use std::sync::Arc;

use esotereel_lib::project::clip::Clip;
use esotereel_lib::project::{ClipUpdateMap, Project};
use esotereel_lib::util::error::EsotereelResult;
use rkyv::Deserialize;

pub(crate) fn clip_apply_updates(
    project: &mut Project,
    timeline_type: usize,
    updates: &rkyv::Archived<ClipUpdateMap>,
) -> EsotereelResult<()> {
    let timeline = project.get_timeline_mut(timeline_type)?;

    for (_, update_clips) in updates.iter() {
        for clip in update_clips.iter() {
            timeline.remove_clip_by_id(clip.id);
        }
    }

    for (layer_idx, update_clips) in updates.iter() {
        // TODO: warning for non-existent layer
        if let Some(layer) = timeline.layers.get_mut(*layer_idx as usize) {
            for archived_clip in update_clips.iter() {
                let new_clip: Clip = archived_clip
                    .as_ref()
                    .deserialize(&mut rkyv::Infallible)
                    .expect("Failed to deserialize clip");

                layer.clips.insert(Arc::new(new_clip));
            }
        }
    }
    Ok(())
}
