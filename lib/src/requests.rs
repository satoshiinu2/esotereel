use std::{collections::VecDeque, ops::Range};

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::project::{
    MediaSec, TimelineTick,
    command::CommandRequest,
    ids::{ResourceId, TimelineId},
};

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
    /// メディアストリームの初期化。
    InitStream {
        path: String,
    },
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
    Undo {
        timeline_id: TimelineId,
    },
    /// 操作をやり直す（Redo）。
    Redo {
        timeline_id: TimelineId,
    },
}
