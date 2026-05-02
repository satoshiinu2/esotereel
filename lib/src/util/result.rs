use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug)]
pub enum EsotereelError {
    LockError(String),
    IoError(String),
    AccessError(String),
    ProjectNotFound,
    TimelineNotFound(usize),
    ClipNotFound(u64),
    StreamNotFound(u32),
    InvalidTimeline,
    InvalidCommand,
    ClipOverlap,
}

pub type EsotereelResult<T> = Result<T, EsotereelError>;

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
