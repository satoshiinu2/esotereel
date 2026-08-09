use esotereel_lib::project::ClipUpdateMap;
use esotereel_lib::project::Timeline;
use esotereel_lib::project::clip::Clip;
use esotereel_lib::util::result::EsotereelResult;
use esotereel_lib::util::slot_map::SlotMapKey;
use rkyv::Deserialize;

pub(crate) fn clip_apply_updates(
    timeline: &mut Timeline,
    updates: &rkyv::Archived<ClipUpdateMap>,
) -> EsotereelResult<()> {
    for (_, update_clips) in updates.iter() {
        for clip in update_clips.iter() {
            timeline.remove_clip_by_id(clip.id);
        }
    }

    for (new_layer_map_key, update_clips) in updates.iter() {
        // TODO: warning for non-existent layer

        for archived_clip in update_clips.iter() {
            let new_clip: Clip = archived_clip.deserialize(&mut rkyv::Infallible).unwrap();

            timeline.modify_layer(*new_layer_map_key, |layer| {
                layer.clips.insert(new_clip);
            })?;
        }
    }
    Ok(())
}
