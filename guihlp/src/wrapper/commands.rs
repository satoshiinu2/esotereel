use esotereel_lib::{
    project::{
        clip::ClipData,
        commands::Command,
        ids::TimelineId,
        transform::{ClipTranslate, ClipTranslates},
    },
    util::types::ClipMoveCtx,
};

use crate::{WrapperErrorCode, network::ClientNetworkHandler, slice_from_ptr_safe};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_clip_move_mul(
    ptr_network: *const ClientNetworkHandler,
    timeline_id: TimelineId,
    ptr: *const u64,
    len: usize,
    position_moved: i64,
    duration_added: i64,
    layer_moved: isize,
) -> WrapperErrorCode {
    if ptr_network.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let Some(project_arc) = app_state.project.as_ref() else {
        return WrapperErrorCode::not_found(Some("project not found"));
    };
    let lock = project_arc.read().unwrap();

    // BTreeMap の index (順序) から Timeline を取得する（または u64 ID 直接指定）
    let Some(timeline) = lock.timeline(timeline_id) else {
        return WrapperErrorCode::not_found(Some("timeline not found"));
    };

    let clip_ids = slice_from_ptr_safe(ptr, len);

    let clip_ctxs = clip_ids
        .iter()
        .filter_map(|clip_id| {
            // Timeline 側のメソッドで Clip と Layer (および order) を探す
            let (layer, clip) = timeline.find_clip_by_id(*clip_id)?;
            let current_order = layer.order;

            let new_order = u32::try_from(current_order as isize + layer_moved).ok()?;
            let new_layer_id = timeline.get_layer_id_by_order(new_order)?;

            Some(ClipMoveCtx {
                clip_id: *clip_id,
                new_position: clip.position + position_moved,
                new_duration: clip.duration + duration_added,
                new_layer_id,
            })
        })
        .collect();

    let command = Command::ClipsMove { clips: clip_ctxs };

    network.req_command(timeline_id, command);

    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_add_clip_dummy(
    ptr_network: *const ClientNetworkHandler,
    timeline_id: TimelineId,
    position: i64,
    layer_order: u32,
) -> WrapperErrorCode {
    if ptr_network.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let Some(project_arc) = app_state.project.as_ref() else {
        return WrapperErrorCode::not_found(Some("project not found"));
    };
    let lock = project_arc.read().unwrap();

    // BTreeMap の index から 該当の TimelineId と Timeline を取得
    let Some(timeline) = lock.timeline(timeline_id) else {
        return WrapperErrorCode::not_found(Some("timeline not found"));
    };

    // Timeline から order を元に LayerId を取得
    let Some(layer_id) = timeline.get_layer_id_by_order(layer_order) else {
        return WrapperErrorCode::not_found(Some("layer not found"));
    };

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
        layer_map_key: layer_id,
        position,
        duration: 10000,
        clip_data,
        translates,
    };

    network.req_command(timeline_id, command);

    WrapperErrorCode::ok()
}
