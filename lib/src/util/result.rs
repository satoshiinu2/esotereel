use std::{
    any::Any,
    fmt,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use anyhow::Result;
use rkyv::{Archive, Deserialize, Serialize};

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
#[derive(Archive, Serialize, Deserialize, Debug, thiserror::Error)]
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

/// `RwLock<T>` の取得に失敗した際に `EsotereelError::LockError` に変換する拡張トレイトです。
///
/// ミューテックスポイズン（スレッドがパニックを起こしながらロックを保持していた状態）
/// は回復不能な状態とみなし、`LockError` としてエラーを返します。
pub trait LockExt<T> {
    /// 書き込みロックを取得し、失敗時は `EsotereelError::LockError` を返します。
    fn write_or_err(&self) -> Result<RwLockWriteGuard<'_, T>>;
    /// 読み取りロックを取得し、失敗時は `EsotereelError::LockError` を返します。
    fn read_or_err(&self) -> Result<RwLockReadGuard<'_, T>>;
}

impl<T> LockExt<T> for RwLock<T> {
    fn write_or_err(&self) -> Result<RwLockWriteGuard<'_, T>> {
        self.write()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))
    }
    fn read_or_err(&self) -> Result<RwLockReadGuard<'_, T>> {
        self.read()
            .map_err(|e| anyhow::Error::from(EsotereelError::LockError(e.to_string())))
    }
}
