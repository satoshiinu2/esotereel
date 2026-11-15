use std::{
    any::Any,
    fmt,
    sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use anyhow::Result;

use crate::{
    plugin::NamespacedID,
    project::{
        TimelineTick,
        ids::{AssetId, ClipId, LayerId, ResourceId, TimelineId},
    },
};

// Esotereel コア全体で共通に使用するエラー型と結果型です。
//
// 各エラーは「何をしようとして失敗したか」を文脈付きで表現し、GUI やネットワーク
// 層へエラーとして伝播させます。本番コードでは `panic!`/`expect()`/`unwrap()` を避け、
// ここで定義された `EsotereelError` を返すか `anyhow::Result` で伝播することを原則とします
// （AGENTS.md 第9条：パニックの使用制限）。

// TODO: cow
#[derive(Debug, thiserror::Error)]
pub enum EsotereelError {
    NullPointer(String),
    LockError(String),
    IoError(String),
    AccessError(String),
    DecodeError(String),
    RenderError(String),
    ProjectNotFound,
    TimelineNotFound(TimelineId),
    ClipNotFound(ClipId),
    ClipNotFoundAt(TimelineTick),
    ClipKindNotFound(NamespacedID),
    LayerNotFound(LayerId),
    DuplicateLayerId(LayerId),
    PluginNotFound(String),
    StreamNotFound(ResourceId),
    InvalidString(String),
    InvalidCommand,
    ClipOverlap,
    InvalidLayerMove,
    AssetNotFound(AssetId),
    AssetAlreadyExists(AssetId),
}

impl fmt::Display for EsotereelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EsotereelError::NullPointer(msg) => write!(f, "Null pointer error: {}", msg),
            EsotereelError::LockError(msg) => write!(f, "Lock error: {}", msg),
            EsotereelError::IoError(msg) => write!(f, "IO error: {}", msg),
            EsotereelError::AccessError(msg) => write!(f, "Access error: {}", msg),
            EsotereelError::DecodeError(msg) => write!(f, "Decode error: {}", msg),
            EsotereelError::RenderError(msg) => write!(f, "Render error: {}", msg),
            EsotereelError::ProjectNotFound => write!(f, "Project not found"),
            EsotereelError::TimelineNotFound(id) => write!(f, "Timeline '{}' not found", id),
            EsotereelError::LayerNotFound(id) => write!(f, "Layer '{}' not found", id),
            EsotereelError::DuplicateLayerId(id) => write!(f, "Duplicate layer ID '{}'", id),
            EsotereelError::PluginNotFound(id) => write!(f, "Plugin '{}' not found", id),
            EsotereelError::ClipNotFound(id) => write!(f, "Clip '{}' not found", id),
            EsotereelError::ClipNotFoundAt(tick) => write!(f, "Clip not found at {tick}"),
            EsotereelError::ClipKindNotFound(id) => write!(f, "ClipKind '{}' not found", id),
            EsotereelError::StreamNotFound(id) => write!(f, "Stream '{}' not found", id),
            EsotereelError::InvalidString(name) => write!(f, "Invalid string: {}", name),
            EsotereelError::InvalidCommand => write!(f, "Invalid command"),
            EsotereelError::ClipOverlap => write!(f, "Clip overlap"),
            EsotereelError::InvalidLayerMove => write!(f, "Invalid layer move"),
            EsotereelError::AssetNotFound(id) => write!(f, "Asset '{}' not found", id),
            EsotereelError::AssetAlreadyExists(id) => write!(f, "Asset '{}' already exists", id),
        }
    }
}

/// Esotereel の結果型の省略形です。エラーは `EsotereelError` に統一されます。
pub type EsotereelResult<T> = Result<T>;

/// スパン／スレッド境界で回収されたパニックペイロードを文字列に変換します。
///
/// `Box<dyn Any + Send>` は `std::panic::catch_unwind` 等で回収されたペイロードを想定し、
/// `&str` / `String` の場合はそのまま返し、それ以外は「Unknown panic or error payload」を返します。
pub fn format_any_error(error: Box<dyn Any + Send>) -> String {
    if let Some(s) = error.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = error.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic or error payload".to_string()
    }
}

/// ロック取得時のミューテックスポイズン（スレッドがパニックを起こしながら
/// ロックを保持していた状態）から回復するための拡張トレイトです。
///
/// 毒性は「パニックしたスレッドがデータを不整合な状態で残したかもしれない」
/// という印であり、ロック機構自体の故障ではありません。そのため毒性時は
/// `into_inner` でガードを回復し、`log::error` に回復した事実を残して処理を
/// 継続します。毒性をエラーとして伝播させると、1 回のパニックでそのロックを
/// 使う全操作がプロセス終了まで恒久的に失敗してしまうため、それを避けます。
pub trait LockExt<T> {
    /// 書き込みロックを取得します。毒性時は回復してガードを返します。
    fn write_or_err(&self) -> Result<RwLockWriteGuard<'_, T>>;
    /// 読み取りロックを取得します。毒性時は回復してガードを返します。
    fn read_or_err(&self) -> Result<RwLockReadGuard<'_, T>>;
}

impl<T> LockExt<T> for RwLock<T> {
    fn write_or_err(&self) -> Result<RwLockWriteGuard<'_, T>> {
        Ok(match self.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::error!("RwLock write poisoned; recovering guard via into_inner");
                poisoned.into_inner()
            }
        })
    }

    fn read_or_err(&self) -> Result<RwLockReadGuard<'_, T>> {
        Ok(match self.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::error!("RwLock read poisoned; recovering guard via into_inner");
                poisoned.into_inner()
            }
        })
    }
}

/// `Mutex<T>` 版の毒性回復トレイトです。方針は `LockExt` と同じです。
pub trait MutexExt<T> {
    /// ロックを取得します。毒性時は回復してガードを返します。
    fn lock_or_err(&self) -> Result<MutexGuard<'_, T>>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_or_err(&self) -> Result<MutexGuard<'_, T>> {
        Ok(match self.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::error!("Mutex poisoned; recovering guard via into_inner");
                poisoned.into_inner()
            }
        })
    }
}
