use std::{
    any::Any,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

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

pub type EsotereelResult<T> = Result<T, EsotereelError>;

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
    fn write_or_err(&self) -> EsotereelResult<RwLockWriteGuard<'_, T>>;
    fn read_or_err(&self) -> EsotereelResult<RwLockReadGuard<'_, T>>;
}

impl<T> LockExt<T> for RwLock<T> {
    fn write_or_err(&self) -> EsotereelResult<RwLockWriteGuard<'_, T>> {
        self.write()
            .map_err(|_| EsotereelError::LockError("Poisoned lock".into()))
    }
    fn read_or_err(&self) -> EsotereelResult<RwLockReadGuard<'_, T>> {
        self.read()
            .map_err(|_| EsotereelError::LockError("Poisoned lock".into()))
    }
}
