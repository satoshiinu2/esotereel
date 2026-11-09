//! プロジェクトデータモデルのモジュール群です。
//!
//! プロジェクト（`Project`）、タイムライン（`Timeline`）、レイヤー（`Layer`）、
//! クリップ（`Clip`）など、動画編集のデータモデルを定義します。
//!
//! ## モジュール一覧
//! - `camera` - カメラ情報（`CameraInfo`）。
//! - `change` - 変更セット（`ChangeSet`）。
//! - `chunk_index` - チャンクインデックス。
//! - `clip` - クリップおよびクリップバインディング値（`ClipBindingValue`）。
//! - `command` - コマンドリクエスト（`CommandRequest`）。
//! - `ids` - 各種 ID 生成器・識別子（`TimelineId`, `LayerId`, `ClipId`, `ResourceId` など）。
//! - `layer` - レイヤー。
//! - `layer_outline` - レイヤーフォルダーとアウトラインノード。
//! - `project` - プロジェクト（`Project`）。
//! - `save` - プロジェクトの保存・読み込み。
//! - `timeline` - タイムライン（`Timeline`, `TimelineMeta`）。
//! - `util` - プロジェクト固有のユーティリティ。
//! - `value` - プロジェクト固有の値型。

pub mod camera;
pub mod change;
pub mod chunk_index;
pub mod clip;
pub mod command;
pub mod ids;
pub mod layer;
pub mod layer_outline;
pub mod project;
pub mod save;
pub mod timeline;
pub mod util;
pub mod value;

pub use {
    clip::Clip, layer::Layer, layer_outline::LayerFolder, project::Project, timeline::Timeline,
};

/// タイムライン上の時刻を表すティック単位の ID（`i64`）です。
///
/// フレームレートに依存しない整数時刻として使用し、クリップの位置・長さに用いられます。
pub type TimelineTick = i64;

/// メディアの秒数を表す浮動小数点数（`f64`）です。
///
/// シーク位置や再生時間など、秒単位の時刻に使用されます。
pub type MediaSec = f64;
