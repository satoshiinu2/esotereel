use std::sync::RwLockWriteGuard;

use crate::{project::Project, util::error::EsotereelResult};

pub trait ProjectExt {
    fn project_err(&mut self) -> EsotereelResult<&mut Project>;
}

impl ProjectExt for RwLockWriteGuard<'_, Option<Project>> {
    fn project_err(&mut self) -> EsotereelResult<&mut Project> {
        self.as_mut()
            .ok_or(crate::util::error::EsotereelError::ProjectNotFound)
    }
}
