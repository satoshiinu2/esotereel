use esotereel_lib::project::{
    clip::ClipData,
    command::{ClipMoveCtx, CommandRequest},
    ids::{LayerFolderId, LayerId, TimelineId},
    transform::{ClipTranslate, ClipTranslates},
};

use crate::{
    WrapperErrorCode, ffi::stringview::StringView, network::ClientNetworkHandler,
    slice_from_ptr_or_empty,
};

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

    let clip_data = {
        let app_state = network.app_state.lock().expect("mutex poisoned");
        let project_arc = match app_state.project.as_ref() {
            Some(arc) => arc,
            None => return WrapperErrorCode::not_found(Some("project not found")),
        };

        let lock = project_arc.read().unwrap();
        let timeline = match lock.timeline(timeline_id) {
            Some(tl) => tl,
            None => return WrapperErrorCode::not_found(Some("timeline not found")),
        };

        // 合成順を1回だけ確定させ、位置解決に使う
        let execution_order: Vec<u64> = timeline.outline.iter_execution_order().collect();

        let clip_ids = unsafe { slice_from_ptr_or_empty(ptr, len) };

        clip_ids
            .iter()
            .filter_map(|clip_id| {
                let (clip, layer_id) = timeline.get_clip_and_layer(*clip_id)?;

                let current_index = execution_order.iter().position(|&id| id == layer_id)?;
                let new_index = current_index.checked_add_signed(layer_moved)?;
                let new_layer_id = *execution_order.get(new_index)?;

                Some(ClipMoveCtx {
                    clip_id: *clip_id,
                    new_position: clip.position + position_moved,
                    new_duration: clip.duration + duration_added,
                    new_layer_id,
                })
            })
            .collect()
    };

    let command = CommandRequest::ClipsMove { clips: clip_data };

    network.req_command(timeline_id, command);

    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_add_clip_dummy(
    ptr_network: *const ClientNetworkHandler,
    timeline_id: TimelineId,
    position: i64,
    layer_id: LayerId,
) -> WrapperErrorCode {
    if ptr_network.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = unsafe { &*ptr_network };

    let clip_data = ClipData::Video {
        path: "/home/satoshiinu/Videos/3.mp4".to_string(),
        media_offset: 0.0,
    };

    let translates = ClipTranslates::Normal(ClipTranslate {
        position: [-100.0, -100.0, 0.0],
        rotation: [0.0, 0.0, 0.0],
        scale: [400.0, 300.0, 1.0],
    });

    let command = CommandRequest::AddClip {
        layer_id,
        position,
        duration: 10000,
        clip_data,
        translates,
    };

    network.req_command(timeline_id, command);

    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_add_layer(
    ptr_network: *const ClientNetworkHandler,
    timeline_id: TimelineId,
    has_parent: bool,
    parent_folder_id: LayerFolderId,
    has_insert_index: bool,
    insert_index: usize,
    name: StringView,
) -> WrapperErrorCode {
    if ptr_network.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    let network = unsafe { &*ptr_network };

    let command = CommandRequest::AddLayer {
        parent_folder_id: has_parent.then_some(parent_folder_id),
        insert_index: has_insert_index.then_some(insert_index),
        name: name.as_string_lossy().into_owned(),
    };

    network.req_command(timeline_id, command);
    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_add_folder(
    ptr_network: *const ClientNetworkHandler,
    timeline_id: TimelineId,
    has_parent: bool,
    parent_folder_id: LayerFolderId,
    has_insert_index: bool,
    insert_index: usize,
    name: StringView,
) -> WrapperErrorCode {
    if ptr_network.is_null() {
        return WrapperErrorCode::null_ptr();
    }
    let network = unsafe { &*ptr_network };

    let command = CommandRequest::AddFolder {
        parent_folder_id: has_parent.then_some(parent_folder_id),
        insert_index: has_insert_index.then_some(insert_index),
        name: name.as_string_lossy().into_owned(),
    };

    network.req_command(timeline_id, command);
    WrapperErrorCode::ok()
}
