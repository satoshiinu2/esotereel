use std::sync::RwLockReadGuard;

use crate::{
    project::Project,
    util::result::{EsotereelError, EsotereelResult},
};

/// プロジェクト取得拡張トレイトです。
///
/// `RwLockReadGuard<Option<Project>>` に対して、プロジェクトが存在しない場合に
/// エラーを返すメソッドを提供します。
pub trait ProjectExt {
    /// プロジェクトへの参照を返します。存在しない場合は `ProjectNotFound` エラー。
    fn project_err(&self) -> EsotereelResult<&Project>;
}

impl ProjectExt for RwLockReadGuard<'_, Option<Project>> {
    fn project_err(&self) -> EsotereelResult<&Project> {
        self.as_ref()
            .ok_or_else(|| anyhow::Error::from(EsotereelError::ProjectNotFound))
    }
}

/// 可変プロジェクト取得拡張トレイトです。
///
/// `Option<Project>` に対して、プロジェクトが存在しない場合に
/// エラーを返すメソッドを提供します。
pub trait ProjectMutExt {
    /// プロジェクトへの可変参照を返します。存在しない場合は `ProjectNotFound` エラー。
    fn project_mut_err(&mut self) -> EsotereelResult<&mut Project>;
}

impl ProjectMutExt for Option<Project> {
    fn project_mut_err(&mut self) -> EsotereelResult<&mut Project> {
        self.as_mut()
            .ok_or_else(|| anyhow::Error::from(EsotereelError::ProjectNotFound))
    }
}
