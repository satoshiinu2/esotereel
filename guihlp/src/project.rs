use nomyoedit_lib::project::Project;
use nomyoedit_lib::project::clip::{ArchivedClip, Clip};
use rkyv::Deserialize;
use rkyv::{collections::ArchivedHashMap, vec::ArchivedVec};

pub(crate) fn clip_apply_updates(
    project: &mut Project,
    timeline_type: usize,
    updates: &ArchivedHashMap<u32, ArchivedVec<ArchivedClip>>,
) -> Result<(), String> {
    let timeline = project.get_timeline_mut(timeline_type)?;

    for (_, update_layer) in updates.iter() {
        for archived_clip in update_layer.iter() {
            timeline.remove_clip_by_id(archived_clip.id);
        }
    }

    for (layer_idx, update_layer) in updates.iter() {
        // TODO: warning for non-existent layer
        if let Some(layer) = timeline.layers.get_mut(*layer_idx as usize) {
            for archived_clip in update_layer.iter() {
                let new_clip: Clip = archived_clip.deserialize(&mut rkyv::Infallible).unwrap();
                layer.clips.insert(new_clip);
            }
        }
    }
    Ok(())
}
