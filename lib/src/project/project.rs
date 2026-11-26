use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::plugin::{NamespacedID, property::PropertyKey};
use crate::project::asset::AssetRegistry;
use crate::project::change::ChangeSet;
use crate::project::clip::ClipBindingValue;
use crate::project::ids::{AssetRef, ClipId, IdGenerator, LayerFolderId, LayerId, TimelineId};
use crate::project::kind_table::{KindTable, SharedKindTable, new_shared_kind_table};
use crate::project::timeline::{Timeline, TimelineMeta};
use crate::util::result::EsotereelError;

/// プロジェクトのルートデータモデルです。
///
/// 複数のタイムライン（`Timeline`）を `TimelineId` でキーに保持し、
/// `IdGenerator` により各種 ID を一意に生成します。
///
/// `id` はプロジェクトを識別する UUID で、作成時に新規採番されます。
/// 保存ファイルの移動・リネームでも変わらず、オートセーブ世代との
/// 紐付けにも使われます。`IdGenerator` とは異なり保存されます。
///
/// タイムライン・レイヤー・クリップの挿入・取得・削除、タイムラインの
/// 独立化（ミラーからのコピー）、変更セットの抽出などを提供します。
#[derive(Debug)]
pub struct Project {
    id: Uuid,
    timelines: BTreeMap<TimelineId, Timeline>,
    ids: IdGenerator,
    assets: AssetRegistry,
    /// kind 連番の intern テーブル。全タイムラインで共有し、採番の
    /// 権威はプロジェクト（サーバー側では core）です。保存はしません。
    kind_table: SharedKindTable,
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

impl Project {
    /// 新しい空のプロジェクトを作成します。プロジェクト ID を新規採番します。
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            timelines: BTreeMap::new(),
            ids: IdGenerator::default(),
            assets: AssetRegistry::default(),
            kind_table: new_shared_kind_table(),
        }
    }

    /// kind intern テーブルのスナップショットを返します。
    pub fn kind_table(&self) -> KindTable {
        self.kind_table
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// プロジェクト共有の intern テーブルへのハンドルを返します。
    pub fn shared_kind_table(&self) -> SharedKindTable {
        self.kind_table.clone()
    }

    /// プロジェクト ID を返します。
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// 保持するタイムラインを ID 順に反復します。
    pub fn iter_timelines(&self) -> impl Iterator<Item = (&TimelineId, &Timeline)> {
        self.timelines.iter()
    }

    /// 読み込んだ内容からプロジェクトを組み立てます。
    ///
    /// `IdGenerator` は保存されないためリセットし、読み込んだ全 ID を
    /// `observe_*` で引き上げて新規発行との衝突を避けます。
    pub(crate) fn from_loaded(
        id: Uuid,
        timelines: BTreeMap<TimelineId, Timeline>,
        assets: AssetRegistry,
    ) -> Self {
        // 保存形式はタイムライン単位で kind を intern するため、ロード時は
        // 各タイムラインの表をプロジェクト表へ統合し、配置の連番を
        // 振り直してから全タイムラインで表を共有します。
        let shared = new_shared_kind_table();
        let mut timelines = timelines;
        for timeline in timelines.values_mut() {
            let local = timeline.kind_table();
            let mut remap = HashMap::new();
            {
                let mut table = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                for (old_id, kind) in local.entries().iter().enumerate() {
                    let new_id = table.intern(kind);
                    remap.insert(old_id as u32, new_id);
                }
            }
            timeline.remap_kind_ids(&remap);
            timeline.set_shared_kind_table(shared.clone());
        }
        let mut project = Self {
            id,
            timelines,
            ids: IdGenerator::default(),
            assets,
            kind_table: shared,
        };
        for (&timeline_id, timeline) in &project.timelines {
            project.ids.observe_timeline(timeline_id);
            for (&layer_id, _) in timeline.iter_layers() {
                project.ids.observe_layer(layer_id);
            }
            for folder_id in timeline.outline.folder_ids() {
                project.ids.observe_folder(folder_id);
            }
            for (&clip_id, _) in timeline.iter_clips() {
                project.ids.observe_clip(clip_id);
            }
        }
        project
    }

    /// 指定 FPS で新しいタイムラインを追加し、その `TimelineId` を返します。
    pub fn insert_timeline(&mut self, fps: f64) -> TimelineId {
        let id = self.ids.next_timeline_id();
        let mut timeline = Timeline::new(id, fps, &mut self.ids);
        timeline.set_shared_kind_table(self.kind_table.clone());
        self.timelines.insert(id, timeline);
        id
    }

    /// 指定 ID のタイムラインへの不変参照を返します。存在しなければ `None`。
    pub fn timeline_ref(&self, id: TimelineId) -> Option<&Timeline> {
        self.timelines.get(&id)
    }

    /// 指定 ID のタイムラインへの可変参照を返します。存在しなければ `None`。
    pub fn timeline_mut(&mut self, id: TimelineId) -> Option<&mut Timeline> {
        self.timelines.get_mut(&id)
    }

    /// 保持するタイムラインの数を返します。
    pub fn timeline_count(&self) -> usize {
        self.timelines.len()
    }

    /// ID 生成器への可変参照を返します。
    pub fn id_generator_mut(&mut self) -> &mut IdGenerator {
        &mut self.ids
    }

    /// 保持するアセットレジストリへの不変参照を返す。
    pub fn assets(&self) -> &AssetRegistry {
        &self.assets
    }

    /// 保持するアセットレジストリへの可変参照を返す。
    pub fn assets_mut(&mut self) -> &mut AssetRegistry {
        &mut self.assets
    }

    /// 外部ファイルをプロジェクトのアセットとして登録し、その `AssetRef` を返す。
    ///
    /// 同一パスが既に登録済みなら既存の ID を返す。クリップが参照する動画リソース
    /// は、クリップ追加時にここで登録され、`Project.assets` がそのカタログとなる。
    pub fn register_asset(&mut self, path: &Path) -> anyhow::Result<AssetRef> {
        self.assets.get_or_add_external(path)
    }

    /// ID を指定して外部ファイル参照を登録します。
    pub fn register_external_asset(
        &mut self,
        id: AssetRef,
        path: &Path,
    ) -> anyhow::Result<AssetRef> {
        if self.assets.get(id).is_some() {
            if let Ok(existing_path) = self.assets.resolve(id) {
                if existing_path == path {
                    return Ok(id);
                }
            }
            anyhow::bail!("asset ID `{id}` is already registered to another file");
        }
        self.assets.add_external(id, path)
    }

    /// ID を指定して管理対象ファイルを登録します。
    pub fn register_managed_asset(
        &mut self,
        id: AssetRef,
        name: String,
        path: PathBuf,
    ) -> anyhow::Result<AssetRef> {
        if self.assets.get(id).is_some() {
            if let Ok(existing_path) = self.assets.resolve(id) {
                if existing_path == path.as_path() {
                    return Ok(id);
                }
            }
            anyhow::bail!("asset ID `{id}` is already registered to another file");
        }
        self.assets.add_managed(id, name, path)
    }

    /// 指定タイムラインにレイヤー（またはフォルダー）を新規挿入します。
    ///
    /// `parent` が `Some` の場合はそのレイヤーの子として、`None` の場合はルート直下に追加します。
    /// `index` を省略すると末尾に追加されます。
    pub fn insert_layer_in_timeline(
        &mut self,
        timeline_id: TimelineId,
        parent: Option<LayerFolderId>,
        index: Option<usize>,
        name: String,
    ) -> anyhow::Result<LayerId> {
        let timeline = self
            .timelines
            .get_mut(&timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
        timeline.insert_layer(&mut self.ids, name, parent, index)
    }

    pub fn insert_folder_in_timeline(
        &mut self,
        timeline_id: TimelineId,
        parent: Option<LayerFolderId>,
        index: Option<usize>,
        name: String,
    ) -> anyhow::Result<LayerFolderId> {
        let timeline = self
            .timelines
            .get_mut(&timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
        timeline.insert_folder(&mut self.ids, name, parent, index)
    }

    pub fn new_clip_in_timeline(
        &mut self,
        timeline_id: TimelineId,
        layer_id: LayerId,
        position: i64,
        duration: i64,
        kind_id: NamespacedID,
        properties: HashMap<PropertyKey, ClipBindingValue>,
    ) -> anyhow::Result<ClipId> {
        let timeline = self
            .timelines
            .get_mut(&timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

        timeline.new_clip_in(
            layer_id,
            &mut self.ids,
            position,
            duration,
            kind_id,
            properties,
        )
    }

    /// ミラー参照されているタイムラインを独立コピーし、新しい `TimelineId` を返します。
    ///
    /// 呼び出し側で `Clip` の `CompositionRef` を `Independent(new_id)` に差し替えること。
    pub fn make_independent(&mut self, source: TimelineId) -> anyhow::Result<TimelineId> {
        let new_id = self.ids.next_timeline_id();
        let cloned = self
            .timelines
            .get_mut(&source)
            .ok_or(EsotereelError::TimelineNotFound(source))?
            .deep_clone(&mut self.ids, new_id)?;
        self.timelines.insert(new_id, cloned);
        Ok(new_id)
    }

    /// 全タイムラインの全チャンクが読み込み済みであることを保証します。
    ///
    /// 保存など、実体が全部必要な処理の前に呼びます。
    pub fn ensure_all_chunks_loaded(&mut self) -> crate::util::result::EsotereelResult<()> {
        for timeline in self.timelines.values_mut() {
            timeline.ensure_all_chunks_loaded()?;
        }
        Ok(())
    }

    /// プロジェクトフォルダのチャンクファイルを読み書きするストアを、
    /// 全タイムラインへ付けます。
    ///
    /// プロジェクトフォルダを開いた・作ったタイミングで呼びます。
    /// フォルダを移動したら、新しい場所で付け直す必要があります。
    pub fn attach_folder_chunk_stores(&mut self, project_dir: &Path) {
        for (timeline_id, timeline) in self.timelines.iter_mut() {
            let timeline_dir = project_dir.join("timelines").join(timeline_id.to_string());
            timeline.set_chunk_store(Some(Box::new(
                crate::project::chunk_store::FolderChunkStore::new(timeline_dir),
            )));
        }
    }

    /// 対応する `Clip` が存在しなくなった Independent Timeline を掃除します。
    ///
    /// 厳密な参照カウントは行わず、呼び出し側（クリップ削除時）から明示的に呼び出します。
    pub fn remove_timeline(&mut self, id: TimelineId) -> Option<Timeline> {
        self.timelines.remove(&id)
    }

    pub fn drain_changes(&mut self) -> Vec<(TimelineId, ChangeSet)> {
        self.timelines
            .iter_mut()
            .filter_map(|(&id, tl)| {
                let cs = tl.drain_changes();
                (!cs.is_empty()).then_some((id, cs))
            })
            .collect()
    }

    /// `ProjectAll` 用の軽量メタ情報一覧を返します。クリップ本体は含みません。
    pub fn timelines_meta(&self) -> Vec<TimelineMeta> {
        self.timelines.values().map(TimelineMeta::from).collect()
    }

    /// メタ情報一覧からプロジェクトを復元します。
    ///
    /// 各タイムライン・レイヤーの ID を `IdGenerator` に記録し、
    /// 整合性を保った状態で再構築します。
    pub fn from_meta(timelines: Vec<TimelineMeta>) -> Self {
        let mut project = Self::new();
        for meta in timelines {
            let timeline_id = meta.id;
            let mut timeline = Timeline::from_meta(&meta);
            timeline.set_shared_kind_table(project.kind_table.clone());
            project.timelines.insert(timeline_id, timeline);
            project.ids.observe_timeline(timeline_id);
            for lm in &meta.layers {
                project.ids.observe_layer(lm.id);
            }
        }
        project
    }

    /// 権威側（core）から配信された kind 表でミラーを置き換えます。
    ///
    /// プロジェクト開始時に全表を受け取るためのもので、既存の配置が
    /// 持つ連番は配信側の採番と一致している前提です。
    pub fn replace_kind_table(&self, entries: Vec<NamespacedID>) {
        let table = KindTable::from_order(entries);
        let mut guard = self
            .kind_table
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = table;
    }

    /// 権威側から配信された続きの採番をミラーへ追加します。
    /// 整合しない追記は取り込まず `false` を返します。
    pub fn append_kind_entries(
        &self,
        start: crate::project::kind_table::KindId,
        entries: &[NamespacedID],
    ) -> bool {
        let mut guard = self
            .kind_table
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.append_entries(start, entries)
    }
}
