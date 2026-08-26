use esotereel_lib::{
    project::{
        clip::ClipData,
        commands::Command,
        ids::{LayerId, TimelineId},
        transform::{ClipTranslate, ClipTranslates},
    },
    util::types::ClipMoveCtx,
};

use crate::{
    WrapperErrorCode, network::ClientNetworkHandler, slice_from_ptr_or_empty,
    wrapper::stringview::StringView,
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

    // Single lock acquisition to get all needed data
    let clip_data = {
        let app_state = network.app_state.lock().expect("mutex poisoned");
        let project_arc = match app_state.project.as_ref() {
            Some(arc) => arc,
            None => return WrapperErrorCode::not_found(Some("project not found")),
        };

        // Get timeline data with minimal lock time
        let lock = project_arc.read().unwrap();
        let timeline = match lock.timeline(timeline_id) {
            Some(tl) => tl,
            None => return WrapperErrorCode::not_found(Some("timeline not found")),
        };

        let clip_ids = unsafe { slice_from_ptr_or_empty(ptr, len) };

        clip_ids
            .iter()
            .filter_map(|clip_id| {
                let (clip, layer_id) = timeline.find_clip_by_id(*clip_id)?;

                // order(数値)は無くなったので、root_layers内のindexで移動量を解決する。
                // 注意: Composite展開行など別Timeline由来のレイヤーはroot_index_ofが
                // Noneを返す(=このtimeline上には存在しない)ため、その場合は移動対象外になる。
                let current_index = timeline.root_index_of(layer_id)?;
                let new_index = current_index.checked_add_signed(layer_moved)?;
                let new_layer_id = timeline.layer_id_at_root_index(new_index)?;

                Some(ClipMoveCtx {
                    clip_id: *clip_id,
                    new_position: clip.position + position_moved,
                    new_duration: clip.duration + duration_added,
                    new_layer_id,
                })
            })
            .collect()
    };

    let command = Command::ClipsMove { clips: clip_data };

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

    let command = Command::AddClip {
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
    parent_layer_id: LayerId,
    has_insert_index: bool,
    insert_index: usize,
    name: StringView,
    is_folder: bool,
) -> WrapperErrorCode {
    if ptr_network.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = unsafe { &*ptr_network };

    let parent_layer_id = if has_parent {
        Some(parent_layer_id)
    } else {
        None
    };

    let insert_index = if has_insert_index {
        Some(insert_index)
    } else {
        None
    };

    let command = Command::AddLayer {
        parent_layer_id,
        insert_index,
        name: name.as_string_lossy().into_owned(),
        is_folder,
    };

    network.req_command(timeline_id, command);

    WrapperErrorCode::ok()
}
