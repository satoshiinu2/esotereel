use esotereel_lib::project::Project;

use crate::{ON_CONNECTED_CALLBACKS, WrapperErrorCode, network::OnConnectedFn};

pub mod commands;
pub mod internalserver;
pub mod logger;
pub mod network;
pub mod project;
pub mod render;
pub mod requests;
pub mod stringview;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_on_connected_callback(callback: OnConnectedFn) {
    ON_CONNECTED_CALLBACKS.set(callback).ok();
}

#[unsafe(no_mangle)]
pub extern "C" fn get_project() -> *const Project {
    let lock = ProjectHelper::get_project_lock();
    let Ok(lock) = lock else {
        return std::ptr::null();
    };
    let Some(project) = lock.as_ref() else {
        return std::ptr::null();
    };
    project
}

struct ProjectHelper {}

pub type ProjectGuard<'a> = std::sync::RwLockReadGuard<'a, Option<Project>>;

impl ProjectHelper {
    pub(crate) fn get_project_lock() -> Result<ProjectGuard<'static>, WrapperErrorCode> {
        let instance_guard = crate::network::INSTANCE
            .read()
            .map_err(|_| WrapperErrorCode::Error)?;

        let instance = instance_guard.as_ref().ok_or(WrapperErrorCode::NotFound)?;

        // Safety: We are extending the lifetime of the lock to 'static.
        // This is only safe because the ClientNetworkHandler is stored in a global static INSTANCE
        // and is never dropped or replaced during the application's lifetime once initialized.
        let lock = unsafe {
            std::mem::transmute::<
                std::sync::RwLockReadGuard<'_, Option<Project>>,
                std::sync::RwLockReadGuard<'static, Option<Project>>,
            >(
                instance
                    .app_state
                    .project
                    .read()
                    .map_err(|_| WrapperErrorCode::Error)?,
            )
        };

        if lock.is_none() {
            return Err(WrapperErrorCode::NotFound);
        }

        Ok(lock)
    }
}
