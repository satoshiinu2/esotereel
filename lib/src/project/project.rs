use std::collections::{BTreeMap, HashMap};

use crate::plugin::NamespacedID;
use crate::project::asset::AssetRegistry;
use crate::project::change::ChangeSet;
use crate::project::clip::ClipBindingValue;
use crate::project::ids::{ClipId, IdGenerator, LayerFolderId, LayerId, TimelineId};
use crate::project::timeline::{Timeline, TimelineMeta};
use crate::util::result::EsotereelError;

/// プロジェクトのルートデータモデルです。
///
/// 複数のタイムライン（`Timeline`）を `TimelineId` でキーに保持し、
/// `IdGenerator` により各種 ID を一意に生成します。
///
/// タイムライン・レイヤー・クリップの挿入・取得・削除、タイムラインの
/// 独立化（ミラーからのコピー）、変更セットの抽出などを提供します。
#[derive(Debug, Default)]
pub struct Project {
    timelines: BTreeMap<TimelineId, Timeline>,
    ids: IdGenerator,
    assets: AssetRegistry,
}

impl Project {
    /// 新しい空のプロジェクトを作成します。
    pub fn new() -> Self {
        Self::default()
    }

    /// 指定 FPS で新しいタイムラインを追加し、その `TimelineId` を返します。
    pub fn insert_timeline(&mut self, fps: f64) -> TimelineId {
        let id = self.ids.next_timeline_id();
        self.timelines
            .insert(id, Timeline::new(id, fps, &mut self.ids));
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
        Ok(timeline.insert_layer(&mut self.ids, name, parent, index))
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
        Ok(timeline.insert_folder(&mut self.ids, name, parent, index))
    }

    pub fn new_clip_in_timeline(
        &mut self,
        timeline_id: TimelineId,
        layer_id: LayerId,
        position: i64,
        duration: i64,
        kind_id: NamespacedID,
        properties: HashMap<NamespacedID, ClipBindingValue>,
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
            .get(&source)
            .ok_or(EsotereelError::TimelineNotFound(source))?
            .deep_clone(&mut self.ids, new_id);
        self.timelines.insert(new_id, cloned);
        Ok(new_id)
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
            project
                .timelines
                .insert(timeline_id, Timeline::from_meta(&meta));
            project.ids.observe_timeline(timeline_id);
            for lm in &meta.layers {
                project.ids.observe_layer(lm.id);
            }
        }
        project
    }
}
