use std::sync::RwLockReadGuard;

use crate::{
    project::Project,
    util::result::{EsotereelError, EsotereelResult},
};

pub trait ProjectExt {
    fn project_err(&self) -> EsotereelResult<&Project>;
}

impl ProjectExt for RwLockReadGuard<'_, Option<Project>> {
    fn project_err(&self) -> EsotereelResult<&Project> {
        self.as_ref()
            .ok_or_else(|| anyhow::Error::from(EsotereelError::ProjectNotFound))
    }
}

pub trait ProjectMutExt {
    fn project_mut_err(&mut self) -> EsotereelResult<&mut Project>;
}

impl ProjectMutExt for Option<Project> {
    fn project_mut_err(&mut self) -> EsotereelResult<&mut Project> {
        self.as_mut()
            .ok_or_else(|| anyhow::Error::from(EsotereelError::ProjectNotFound))
    }
}
