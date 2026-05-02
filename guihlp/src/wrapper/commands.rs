use std::slice;

use esotereel_lib::{
    project::{clipdata::ClipData, commands::Command},
    util::types::ClipMoveCtx,
};

use crate::{network::ClientNetworkHandler, wrapper::requests::req_command};

#[unsafe(no_mangle)]
pub extern "C" fn req_cmd_clip_move_mul(
    timeline_idx: usize,
    ptr: *const u64,
    len: usize,
    position_moved: i64,
    duration_added: i64,
    layer_moved: isize,
) {
    let Some(network) = ClientNetworkHandler::get_instance() else {
        return;
    };
    let app_state = &network.app_state;

    let lock = app_state.project.read().unwrap();
    let Some(project) = lock.as_ref() else {
        return;
    };
    let Ok(timeline) = project.get_timeline(timeline_idx) else {
        return;
    };

    let clip_ids = unsafe { slice::from_raw_parts(ptr, len) };

    let clip_ctxs = clip_ids
        .iter()
        .filter_map(|clip_id| {
            let (layer_idx, clip) = timeline.find_clip_by_id(*clip_id)?;

            Some(ClipMoveCtx {
                clip_id: *clip_id,
                new_position: (clip.position + position_moved),
                new_duration: (clip.duration + duration_added),
                new_layer: usize::try_from(layer_idx as isize + layer_moved).ok()?,
            })
        })
        .collect();

    let command = Command::ClipsMove { clips: clip_ctxs };

    req_command(timeline_idx, command);
}

#[unsafe(no_mangle)]
pub extern "C" fn req_cmd_add_clip_dummy(timeline_idx: usize, position: i64, layer_idx: usize) {
    let command = Command::AddClip {
        layer_idx,
        position,
        duration: 10,
        clip_data: ClipData::Dummy,
    };
    req_command(timeline_idx, command);
}
