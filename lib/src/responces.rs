use std::ops::Range;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::plugin::NamespacedID;
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
    ProjectMeta {
        timelines: Vec<TimelineMeta>,
    },
    /// クリップの更新。
    UpdateClip {
        timeline_id: TimelineId,
        clips: Vec<(LayerId, Clip)>,
    },
    /// クリッププロパティの更新。
    UpdateClipProperty {
        timeline_id: TimelineId,
        clip_id: ClipId,
        key: NamespacedID,
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
}
