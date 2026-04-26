use esotereel_lib::{project::Project, responces::ArchivedResponse, util::error::EsotereelResult};
use rkyv::{Deserialize, de::deserializers::SharedDeserializeMap};

use crate::{PROJECT, project::clip_apply_updates, update_timeline};

pub(super) fn on_responce_recveve(responce: &ArchivedResponse) -> EsotereelResult<()> {
    match responce {
        ArchivedResponse::Test => {}
        ArchivedResponse::ProjectAll { project } => {
            let mut real_project: Project = project
                .deserialize(&mut SharedDeserializeMap::new())
                .unwrap();

            for i in 0..real_project.get_timeline_count() {
                let timeline = real_project.get_timeline_mut(i)?;
                timeline
                    .layers
                    .iter_mut()
                    .for_each(|l| l.clips.rebuild_id_map());
            }
            let timeline_len = real_project.get_timeline_count();

            // dbg!(real_project.clone());
            *PROJECT.write().unwrap() = Some(real_project);
            for i in 0..timeline_len {
                update_timeline(i);
            }
        }
        ArchivedResponse::ClipUpdates {
            timeline_type,
            updates,
        } => {
            if let Some(project) = PROJECT.write().unwrap().as_mut() {
                clip_apply_updates(project, *timeline_type as usize, updates)?;
            };
            update_timeline(*timeline_type as usize);
        }
    }
    Ok(())
}
