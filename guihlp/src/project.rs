use esotereel_lib::project::clip::Clip;
use esotereel_lib::project::{ClipUpdateMap, Project};
use esotereel_lib::util::result::EsotereelResult;
use esotereel_lib::util::slot_map::SlotMapKey;
use rkyv::Deserialize;

pub(crate) fn clip_apply_updates(
    project: &mut Project,
    timeline_type: usize,
    updates: &rkyv::Archived<ClipUpdateMap>,
) -> EsotereelResult<()> {
    let timeline = project.get_timeline_mut(timeline_type)?;

    for (_, update_clips) in updates.iter() {
        for clip in update_clips.iter() {
            timeline.layers.remove_clip_by_id(clip.id);
        }
    }

    for (new_layer_map_key, update_clips) in updates.iter() {
        // TODO: warning for non-existent layer

        for archived_clip in update_clips.iter() {
            let new_clip: Clip = archived_clip
                .as_ref()
                .deserialize(&mut rkyv::Infallible)
                .unwrap();
            let new_layer: SlotMapKey = new_layer_map_key
                .deserialize(&mut rkyv::Infallible)
                .unwrap();

            timeline.layers.update_layer_clip(&new_layer, new_clip);
        }
    }
    Ok(())
}
