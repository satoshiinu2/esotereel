use std::ops::Range;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::plugin::property::PropertyKey;
use crate::util::result::EsotereelError;
use crate::project::{
    Clip, TimelineTick,
    clip::ClipBindingValue,
    ids::{ClipId, LayerFolderId, LayerId, ResourceId, TimelineId},
    layer::LayerMeta,
    layer_outline::{Meta, OutlineNode},
    timeline::TimelineMeta,
};

/// サーバーからクライアントへ送信されるレスポンスの種類です。
///
/// `rkyv` によるアーカイブ化と `serde` によるシリアライゼーションの両方に対応し、
/// ネットワーク経由で転送されます。各バリアントはサーバーからの応答内容を表現します。
#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Response {
    /// 接続テスト用レスポンス。
    Test,
    /// プロジェクト全体のメタ情報。
    ProjectMeta { timelines: Vec<TimelineMeta> },
    /// クリップの更新。
    UpdateClip {
        timeline_id: TimelineId,
        clips: Vec<(LayerId, Clip)>,
    },
    /// クリッププロパティの更新。
    UpdateClipProperty {
        timeline_id: TimelineId,
        clip_id: ClipId,
        key: PropertyKey,
        value: ClipBindingValue,
    },
    /// クリップの削除。
    RemoveClip {
        timeline_id: TimelineId,
        clip_ids: Vec<(LayerId, ClipId)>,
    },
    /// コマンドが拒否された場合の応答。現在のクリップ状態を含む。
    CommandRejected {
        timeline_id: TimelineId,
        clip_id: ClipId,
        current_position: TimelineTick,
        current_duration: TimelineTick,
        current_layer_id: LayerId,
    },
    /// Asset の参照登録またはアップロードが完了しました。
    AssetImported { asset_id: String },
    /// 処理が失敗しました。
    Failed { error: EsotereelError },
    /// レイヤーの更新。
    UpdateLayer {
        timeline_id: TimelineId,
        layers: Vec<LayerMeta>,
    },
    /// レイヤーの削除。
    RemoveLayer {
        timeline_id: TimelineId,
        layer_ids: Vec<LayerId>,
    },
    /// アウトライン（レイヤーフォルダーと子ノード）の更新。
    UpdateOutline {
        timeline_id: TimelineId,
        folders: Vec<(LayerFolderId, Meta)>,
        children: Vec<(Option<LayerFolderId>, Vec<OutlineNode>)>, // コンテナごとの並びまるごと
    },
    /// フォルダーの削除。
    Removes {
        timeline_id: TimelineId,
        folder_ids: Vec<LayerFolderId>,
    },
    /// ストリームのメタデータ（コーデック、解像度、時間基準など）。
    StreamMetadata {
        path: String,
        resource_id: ResourceId,
        codec_id: u16,
        width: u32,
        height: u32,
        time_base: f64,
        extradata: Vec<u8>,
    },
    /// ストリームデータのチャンク（パケット）。
    StreamData {
        resource_id: ResourceId,
        data: Vec<u8>,
        pts: Option<i64>,
        dts: Option<i64>,
        is_key: bool,
        discontinuous: bool,
        generation: u64,
    },
    /// ストリームデータ取得の終了通知。
    StreamDataEnd {
        resource_id: ResourceId,
        fetched_ranges: Vec<Range<f64>>,
        generation: u64,
    },
    /// デバッグ用：プロジェクト構造（オプションの文字列）。
    DebugProjectStruct(Option<String>),
    /// プロジェクトフォルダの事前確認結果（バージョン判定）。
    ProjectInspected {
        path: String,
        version: ProjectVersionWire,
        status: WireVersionStatus,
        project_id: String,
    },
    /// プロジェクトフォルダを開いた結果。メタ情報と kind 表は
    /// 別途 `ProjectMeta` / `KindTableSync` で届く。
    ProjectOpened {
        path: String,
        version: ProjectVersionWire,
        status: WireVersionStatus,
    },
    /// プロジェクトを保存した先のフォルダ。
    ProjectSaved {
        path: String,
    },
    /// kind 表の全量配信（プロジェクト開始時）。インデックスが連番。
    KindTableSync {
        entries: Vec<String>,
    },
    /// kind 表への追加採番の配信。`start` が先頭エントリの連番。
    KindTableAppended {
        start: u32,
        entries: Vec<String>,
    },
    /// オートセーブ世代の一覧。
    AutosaveGenerations {
        generations: Vec<AutosaveGenerationWire>,
    },
    /// オートセーブ世代から復元して開いた通知。メタ情報と kind 表は
    /// 別途届く。
    AutosaveRestored {
        generation_id: String,
    },
}

/// 保存形式のバージョン（major.minor.revision）の転送形です。
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[archive_attr(derive(CheckBytes))]
pub struct ProjectVersionWire {
    pub major: u32,
    pub minor: u32,
    pub revision: u32,
}

/// 読み取り側のバージョンに対する、ファイルのバージョンの新しさです。
/// `save::VersionStatus` の転送形です。
#[derive(Archive, Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[archive_attr(derive(CheckBytes))]
pub enum WireVersionStatus {
    Compatible,
    NewerRevision,
    NewerMinor,
    NewerMajor,
}

impl From<crate::project::save::VersionStatus> for WireVersionStatus {
    fn from(status: crate::project::save::VersionStatus) -> Self {
        use crate::project::save::VersionStatus;
        match status {
            VersionStatus::Compatible => Self::Compatible,
            VersionStatus::NewerRevision => Self::NewerRevision,
            VersionStatus::NewerMinor => Self::NewerMinor,
            VersionStatus::NewerMajor => Self::NewerMajor,
        }
    }
}

impl From<crate::project::save::ProjectVersion> for ProjectVersionWire {
    fn from(version: crate::project::save::ProjectVersion) -> Self {
        Self {
            major: version.major,
            minor: version.minor,
            revision: version.revision,
        }
    }
}

/// オートセーブ世代1件の転送形です（`autosave::AutosaveGeneration` と同内容）。
#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[archive_attr(derive(CheckBytes))]
pub struct AutosaveGenerationWire {
    pub id: String,
    pub project_id: String,
    pub created_at_ms: u64,
    pub size_bytes: u64,
    pub file_count: u64,
}

impl From<crate::project::autosave::AutosaveGeneration> for AutosaveGenerationWire {
    fn from(generation: crate::project::autosave::AutosaveGeneration) -> Self {
        Self {
            id: generation.id,
            project_id: generation.project_id.to_string(),
            created_at_ms: generation.created_at_ms,
            size_bytes: generation.size_bytes,
            file_count: generation.file_count as u64,
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/responces_tests.rs"]
mod tests;
