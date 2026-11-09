//! Esotereel コアライブラリのルートモジュールです。
//!
//! Esotereel は Rust 製のコア＋ C++/Qt GUI + C ABI 構成の動画編集エンジンです。
//! 本クレートはコア部分を担い、プラグインシステム・プロパティシステム・
//! コンポーネント／スケジューラ・プロジェクトモデル・レンダー（wgpu）・
//! デコード（FFmpeg）・ステート・リクエスト／レスポンスを提供します。
//!
//! ## アーキテクチャの境界
//! - ドメインロジックはすべて Rust コアに置きます（`lib/src/`）。
//! - C ABI / FFI は言語間の境界に限定し、コアの内部実装を GUI に直接持ち込みません。
//! - GUI スレッド以外からの UI 更新は避け、重い処理はワーカースレッド／非同期で実行します。
//!
//! ## モジュールマップ
//! - [`decode`] - メディアストリームのデコード・再生管理（FFmpeg）。
//! - [`dirs`] - ディレクトリパス解決。
//! - [`network`] - クライアント／サーバー通信。
//! - [`plugin`] - プラグイン読み込み・`NamespacedID`・プロパティ・コンポーネント定義。
//! - [`prebootstrap`] - 起動前初期化。
//! - [`project`] - プロジェクト・タイムライン・レイヤー・クリップのデータモデル。
//! - [`render`] - wgpu を用いたオフスクリーンレンダー。
//! - [`requests`] / [`responces`] - クライアント⇔サーバー間のリクエスト／レスポンス列挙体。
//! - [`state`] - `CommonState` および `HostState` トレイト（共有ステート）。
//! - [`util`] - 汎用ユーティリティ（エラー型 `EsotereelError`、`LockExt` など）。
//!
//! ## エラー処理の規約
//! 本番コードでは `panic!`、`expect()`、`unwrap()` を多用せず、`Result`/`Option` による
//! 適切なエラー伝播を行います。ID 未発見やプロパティ不在などはアプリをクラッシュさせ
//! ずエラーとして処理します（AGENTS.md 第9条）。

use crate::decode::{streamplayer::StreamPlayer, videostreamer::VideoStreamer};

pub mod decode;
pub mod dirs;
pub mod network;
pub mod plugin;
pub mod prebootstrap;
pub mod project;
pub mod render;
pub mod requests;
pub mod responces;
pub mod state;
pub mod util;

/// ストリームの読み込み状態を表します。
///
/// `Loading` は読み込み中、`Loaded(id)` は読み込み完了で `id` は
/// リソース／ストリームを識別する ID です。
#[derive(Clone, Copy, Debug)]
pub enum StreamState {
    Loading,
    Loaded(u32),
}

impl StreamState {
    /// `Loaded(id)` の場合 `Some(id)` を、それ以外は `None` を返します。
    pub fn as_option(&self) -> Option<u32> {
        if let StreamState::Loaded(id) = self {
            Some(*id)
        } else {
            None
        }
    }
}

/// ホストの役割を表します。
///
/// `Client` はクライアント側、`Server` はサーバー側の役割を担います。
/// `CommonState` や `HostState` トレイトで、役割ごとの振る舞いを分岐する際に使用します。
#[derive(Debug)]
pub enum HostRole {
    Client,
    Server,
}

/// `VideoStreamer` はスレッド間移動自体は安全ですが、
/// 複数スレッドからの同時書き込みは行わないことを前提とします。
///
/// FFmpeg のデコーダ内部状態へのアクセスは単一スレッドから行うため、
/// `Send`/`Sync` は `unsafe` で実装しつつ、呼び出し側で排他を保証します。
unsafe impl Send for VideoStreamer {}
unsafe impl Sync for VideoStreamer {}

/// `StreamPlayer` はスレッド間移動自体は安全ですが、
/// 複数スレッドからの同時書き込みは行わないことを前提とします。
unsafe impl Send for StreamPlayer {}
unsafe impl Sync for StreamPlayer {}
