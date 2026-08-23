use std::{
    any::Any,
    fmt,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use anyhow::Result;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Serialize, Deserialize, Debug)]
pub enum EsotereelError {
    LockError(String),
    IoError(String),
    AccessError(String),
    DecodeError(String),
    ProjectNotFound,
    TimelineNotFound(usize),
    LayerNotFound,
    ClipNotFound(u64),
    StreamNotFound(u32),
    InvalidTimeline,
    InvalidCommand,
    ClipOverlap,
    DuplicateLayerOrder,
}

impl fmt::Display for EsotereelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EsotereelError::LockError(msg) => write!(f, "Lock error: {}", msg),
            EsotereelError::IoError(msg) => write!(f, "IO error: {}", msg),
            EsotereelError::AccessError(msg) => write!(f, "Access error: {}", msg),
            EsotereelError::DecodeError(msg) => write!(f, "Decode error: {}", msg),
            EsotereelError::ProjectNotFound => write!(f, "Project not found"),
            EsotereelError::TimelineNotFound(idx) => write!(f, "Timeline {} not found", idx),
            EsotereelError::LayerNotFound => write!(f, "Layer not found"),
            EsotereelError::ClipNotFound(id) => write!(f, "Clip {} not found", id),
            EsotereelError::StreamNotFound(id) => write!(f, "Stream {} not found", id),
            EsotereelError::InvalidTimeline => write!(f, "Invalid timeline"),
            EsotereelError::InvalidCommand => write!(f, "Invalid command"),
            EsotereelError::ClipOverlap => write!(f, "Clip overlap"),
            EsotereelError::DuplicateLayerOrder => write!(f, "Duplicate layer order"),
        }
    }
}

impl std::error::Error for EsotereelError {}

pub type EsotereelResult<T> = Result<T>;

pub fn format_any_error(error: Box<dyn Any + Send>) -> String {
    if let Some(s) = error.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = error.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic or error payload".to_string()
    }
}

pub trait LockExt<T> {
    fn write_or_err(&self) -> Result<RwLockWriteGuard<'_, T>>;
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
