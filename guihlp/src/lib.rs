use std::sync::{OnceLock, RwLock};

pub use nomyoedit_lib::project::Project;
pub use nomyoedit_lib::project::clip::Clip;
pub use nomyoedit_lib::project::layer::Layer;
pub use nomyoedit_lib::project::timeline::Timeline;
use nomyoedit_lib::responce::{ArchivedResponse, set_responce_callbacks};
use rkyv::Deserialize;

use crate::project::clip_apply_updates;

pub mod project;
pub mod wrapper;

pub(crate) static PROJECT: RwLock<Option<Project>> = RwLock::new(None);
static GUI_CALLBACKS: OnceLock<GuiCallbacks> = OnceLock::new();

#[repr(C)]
pub struct GuiCallbacks {
    pub on_test: extern "C" fn(),
    pub on_update_timeline: extern "C" fn(timeline_type: usize),
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    set_responce_callbacks(on_responce_recveve);
}

#[unsafe(no_mangle)]
pub extern "C" fn set_gui_callbacks(callbacks: GuiCallbacks) {
    GUI_CALLBACKS.set(callbacks).ok();
}

fn on_responce_recveve(responce: &ArchivedResponse) {
    match responce {
        ArchivedResponse::Test => {}
        ArchivedResponse::ProjectAll { project } => {
            let real_project: Project = project.deserialize(&mut rkyv::Infallible).unwrap();
            // dbg!(real_project.clone());
            *PROJECT.write().unwrap() = Some(real_project);
        }
        ArchivedResponse::ClipUpdates {
            timeline_type,
            updates,
        } => {
            if let Some(project) = PROJECT.write().unwrap().as_mut() {
                clip_apply_updates(project, *timeline_type as usize, updates);
            };
            if let Some(cb) = GUI_CALLBACKS.get() {
                (cb.on_update_timeline)(*timeline_type as usize);
            }
        }
    }
}
