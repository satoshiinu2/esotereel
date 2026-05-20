use std::slice;

use esotereel_lib::{
    project::{
        clip_data::ClipData,
        clip_translate::{ClipTranslate, ClipTranslates},
        commands::Command,
    },
    util::types::ClipMoveCtx,
};

use crate::network::ClientNetworkHandler;

#[unsafe(no_mangle)]
pub extern "C" fn req_cmd_clip_move_mul(
    ptr_network: *const ClientNetworkHandler,
    timeline_idx: usize,
    ptr: *const u64,
    len: usize,
    position_moved: i64,
    duration_added: i64,
    layer_moved: isize,
) {
    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

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
            let (_, clip, src_layer_handle) = timeline.layers.find_orderd_clip_by_id(*clip_id)?;
            let new_layer_order = u32::try_from(src_layer_handle as isize + layer_moved).ok()?;
            let new_layer_map_key = timeline
                .layers
                .get_layer_map_key_by_order(new_layer_order)?;

            Some(ClipMoveCtx {
                clip_id: *clip_id,
                new_position: (clip.position() + position_moved),
                new_duration: (clip.duration + duration_added),
                new_layer_map_key,
            })
        })
        .collect();

    let command = Command::ClipsMove { clips: clip_ctxs };

    network.req_command(timeline_idx, command);
}

#[unsafe(no_mangle)]
pub extern "C" fn req_cmd_add_clip_dummy(
    ptr_network: *const ClientNetworkHandler,
    timeline_idx: usize,
    position: i64,
    layer_order: u32,
) {
    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let lock = app_state.project.read().unwrap();
    let Some(project) = lock.as_ref() else {
        return;
    };
    let Ok(timeline) = project.get_timeline(timeline_idx) else {
        return;
    };

    let layer_map_key = timeline.layers.get_layer_map_key_by_order(layer_order);

    let clip_data = ClipData::Video {
        path: "/home/satoshiinu/Videos/3.mp4".to_string(),
        media_offset: 0.0,
    };

    let translates = ClipTranslates::Normal(ClipTranslate {
        position: [-100.0, -100.0, 0.0],
        rotation: [0.0, 0.0, 0.0],
        scale: [400.0, 300.0, 1.0],
    });

    let command = Command::AddClip {
        layer_map_key,
        position,
        duration: 10000,
        clip_data,
        translates,
    };

    network.req_command(timeline_idx, command);
}
