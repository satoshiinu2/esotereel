use std::ops::Range;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::plugin::property::PropertyKey;
use crate::util::result::EsotereelError;
use crate::project::{
    Clip, TimelineTick,
    asset::AssetEntry,
    autosave::AutosaveGeneration,
    clip::ClipBindingValue,
    ids::{AssetRef, ClipId, LayerFolderId, LayerId, ResourceId, TimelineId},
    layer::LayerMeta,
    layer_outline::{Meta, OutlineNode},
    save::{ProjectVersion, VersionStatus},
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
        version: ProjectVersion,
        status: VersionStatus,
        project_id: String,
    },
    /// プロジェクトフォルダを開いた結果。メタ情報と kind 表は
    /// 別途 `ProjectMeta` / `KindTableSync` で届く。
    ProjectOpened {
        path: String,
        version: ProjectVersion,
        status: VersionStatus,
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
        generations: Vec<AutosaveGeneration>,
    },
    /// オートセーブ世代から復元して開いた通知。メタ情報と kind 表は
    /// 別途届く。
    AutosaveRestored {
        generation_id: String,
    },
    /// アセット表の全量配信（プロジェクト開始時）。レジストリの
    /// 表示順そのままの並びです。`in_use` はいずれかのクリップが
    /// メディア source として参照しているアセットの ID 集合です
    /// （使用判定はクリップ側の走査が要るため、エントリ本体とは
    /// 別に載せます）。
    AssetTableSync {
        entries: Vec<AssetEntry>,
        in_use: Vec<AssetRef>,
    },
    /// アセット表への増分配信。`upserts` は新規・変更分、
    /// `in_use` は `upserts` のうち使用中のものの ID 集合、
    /// `removed` は削除されたアセット ID です。
    AssetTableUpdated {
        upserts: Vec<AssetEntry>,
        in_use: Vec<AssetRef>,
        removed: Vec<AssetRef>,
    },
}

#[cfg(test)]
#[path = "../tests/unit/responces_tests.rs"]
mod tests;
