use std::{collections::VecDeque, ops::Range};

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::plugin::NamespacedID;
use crate::project::{
    MediaSec, TimelineTick,
    command::CommandRequest,
    ids::{LayerId, ResourceId, TimelineId},
};

/// Asset を利用して新しいクリップを作成するための情報です。
#[derive(Debug, Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub struct AddClipFromAsset {
    pub timeline_id: TimelineId,
    pub layer_id: LayerId,
    pub position: TimelineTick,
    pub duration: TimelineTick,
    pub kind_id: NamespacedID,
}

/// クライアントからサーバーへ送信されるリクエストの種類です。
///
/// `rkyv` によるアーカイブ化と `serde` によるシリアライゼーションの両方に対応し、
/// ネットワーク経由で転送されます。各バリアントは「何をしようとしているか」を
/// 表現し、サーバー側でディスパッチされます。
#[derive(Debug, Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Request {
    /// 接続テスト用リクエスト。
    Test,
    /// 新規プロジェクト作成。
    NewProject,
    /// プロジェクト全体のメタ情報取得。
    ProjectAll,
    /// 複数のコマンドをキュー付きで実行。
    Command {
        commands: VecDeque<(TimelineId, CommandRequest)>,
    },
    /// サーバー上にあるファイルを外部参照として登録します。
    RegisterAssetReference {
        asset_id: String,
        path: String,
        add_clip: Option<AddClipFromAsset>,
    },
    /// 管理対象 Asset のアップロードを開始します。
    BeginAssetUpload {
        asset_id: String,
        file_name: String,
        total_size: u64,
    },
    /// 管理対象 Asset の一部を送信します。
    AssetUploadChunk {
        asset_id: String,
        offset: u64,
        data: Vec<u8>,
    },
    /// アップロードを完了し、必要ならその Asset からクリップを追加します。
    FinishAssetUpload {
        asset_id: String,
        add_clip: Option<AddClipFromAsset>,
    },
    /// メディアストリームの初期化。
    InitStream { path: String },
    /// 指定範囲のストリームデータを取得。
    FetchStreamData {
        resource_id: ResourceId,
        ranges: Vec<Range<MediaSec>>,
    },
    /// 指定範囲内のクリップを取得。
    FetchClipsInRange {
        timeline_id: TimelineId,
        range: Range<TimelineTick>,
    },
    /// デバッグ用：プロジェクト構造を取得。
    DebugFetchProjectStruct,
    /// ツールバーボタンのアクションを実行。
    ToolbarAction {
        button_id: String,
        func_name: String,
        run_on: Option<TimelineId>,
        timeline_id: TimelineId,
    },
    /// 操作を元に戻す（Undo）。
    Undo { timeline_id: TimelineId },
    /// 操作をやり直す（Redo）。
    Redo { timeline_id: TimelineId },
    /// プロジェクトフォルダを開かずにバージョンだけを確認。
    InspectProject { path: String },
    /// プロジェクトフォルダを開く。`backup` が真なら開く前に
    /// フォルダのバックアップ複製を作る。
    OpenProject { path: String, backup: bool },
    /// 現在のプロジェクトフォルダへ保存する。フォルダに別の
    /// プロジェクトが入っている場合は `overwrite` が無いと失敗する。
    SaveProject { overwrite: bool },
    /// 指定フォルダへ名前を付けて保存し、以降はそのフォルダを
    /// プロジェクトフォルダとする。空でないフォルダへは
    /// `overwrite` が無いと失敗する。
    SaveProjectAs { path: String, overwrite: bool },
    /// オートセーブ世代の一覧を取得（全プロジェクト分）。
    ListAutosaveGenerations,
    /// オートセーブ世代から復元して開く。
    RestoreAutosaveGeneration { generation_id: String },
    /// アセットの表示名を変更する。
    RenameAsset { asset_id: String, name: String },
    /// アセットをレジストリから削除する。クリップが参照している
    /// アセットは失敗する。
    RemoveAsset { asset_id: String },
    /// アセットの参照先を別の外部ファイルへ付け替える。
    RelinkAsset { asset_id: String, path: String },
}
