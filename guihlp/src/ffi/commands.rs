use std::{
    collections::{BTreeMap, VecDeque},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use esotereel_lib::{
    plugin::{NamespacedID, property::PropertySchema},
    project::{
        Project,
        clip::{ClipBindingValue, ClipData},
        command::{ClipMoveCtx, CommandRequest},
        ids::{LayerFolderId, LayerId, TimelineId},
        transform::{ClipTranslate, ClipTranslates},
    },
    requests::Request,
    util::result::EsotereelError,
};

use crate::{
    WrapperErrorCode,
    ffi::{
        result::{FfiResult, FfiResultVoid},
        state::{ClientStateHandle, OptionProject},
        stringview::StringView,
    },
    network::ClientNetworkHandler,
    slice_from_ptr_or_empty,
};

#[derive(Default, Debug)]
pub struct CommandQueue {
    pending: VecDeque<(TimelineId, CommandRequest)>,
}

impl CommandQueue {
    pub fn enqueue(&mut self, timeline_id: TimelineId, command: CommandRequest) {
        self.pending.push_back((timeline_id, command));
    }

    pub(super) fn send_all(&mut self, network: &ClientNetworkHandler) {
        if self.pending.is_empty() {
            return;
        }

        let req = &Request::Command {
            commands: std::mem::take(&mut self.pending),
        };

        network.send(req);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn command_queue_new() -> *mut CommandQueue {
    Box::into_raw(Box::new(CommandQueue::default()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn command_queue_drop(ptr: *mut CommandQueue) -> FfiResultVoid {
    fn inner(ptr: *mut CommandQueue) -> anyhow::Result<()> {
        if ptr.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("command_queue_drop".to_owned()))
        }
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { drop(Box::from_raw(ptr)) };
        }))
        .map_err(|_| anyhow::anyhow!("panic while dropping command queue"))
    }
    FfiResultVoid::from_result(inner(ptr))
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn command_queue_send_all(
    ptr_queue: *mut CommandQueue,
    ptr_state: *const ClientStateHandle,
) -> FfiResultVoid {
    fn inner(
        ptr_queue: *mut CommandQueue,
        ptr_state: *const ClientStateHandle,
    ) -> anyhow::Result<()> {
        if ptr_queue.is_null() || ptr_state.is_null() {
            anyhow::bail!(EsotereelError::NullPointer(
                "command_queue_send_all".to_owned()
            ))
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state = state.lock().expect("mutex poisoned");
        let network = &state.network;

        catch_unwind(AssertUnwindSafe(|| {
            let queue = unsafe { &mut *ptr_queue };
            queue.send_all(network);
        }))
        .map_err(|_| anyhow::anyhow!("panic while sending command queue"))
    }
    FfiResultVoid::from_result(inner(ptr_queue, ptr_state))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_clip_move_mul(
    ptr_queue: *mut CommandQueue,
    ptr_opt_project: *const OptionProject,
    timeline_id: TimelineId,
    ptr: *const u64,
    len: usize,
    position_moved: i64,
    duration_added: i64,
    layer_moved: isize,
) -> WrapperErrorCode {
    if ptr_queue.is_null() || ptr_opt_project.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let clip_data = {
        let project = match unsafe { &*ptr_opt_project } {
            Some(arc) => arc,
            None => return WrapperErrorCode::not_found(Some("project not found")),
        };

        let timeline = match project.timeline_ref(timeline_id) {
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

    let queue = unsafe { &mut *ptr_queue };
    queue.enqueue(timeline_id, command);

    WrapperErrorCode::ok()
}

/// be careful of deadlock
#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_add_clip_dummy(
    ptr_queue: *mut CommandQueue,
    ptr_state: *const ClientStateHandle,
    timeline_id: TimelineId,
    position: i64,
    layer_id: LayerId,
) -> FfiResultVoid {
    fn inner(
        ptr_queue: *mut CommandQueue,
        ptr_state: *const ClientStateHandle,
        timeline_id: TimelineId,
        position: i64,
        layer_id: LayerId,
    ) -> anyhow::Result<()> {
        if ptr_queue.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_queue".to_string()));
        }
        if ptr_state.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_state".to_string()));
        }

        // lock here
        let state = ClientStateHandle::from_ptr(ptr_state);
        let state_guard = state.lock().expect("mutex poisoned");

        let clip_data = ClipData::Video {
            path: "/home/satoshiinu/Videos/3.mp4".to_string(),
            media_offset: 0.0,
        };

        let translates = ClipTranslates::Normal(ClipTranslate {
            position: [-100.0, -100.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [400.0, 300.0, 1.0],
        });

        let kind_id = NamespacedID::parse("std:video").unwrap();

        // TODO: server side default_properties
        let loader = state_guard
            .common
            .plugin_loader
            .read()
            .expect("mutex poisoned");

        let property_schema = loader
            .get_clip_properties(&kind_id)
            .ok_or(EsotereelError::ClipKindNotFound(kind_id.clone()))?;

        let properties = ClipBindingValue::default_properties(property_schema);
        drop(loader);

        let command = CommandRequest::AddClip {
            layer_id,
            position,
            duration: 10000,
            kind_id,
            properties,
            translates,
        };

        let queue = unsafe { &mut *ptr_queue };
        queue.enqueue(timeline_id, command);

        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| {
        inner(ptr_queue, ptr_state, timeline_id, position, layer_id)
    })) {
        Ok(Ok(_)) => FfiResultVoid::ok(),
        Ok(Err(e)) => FfiResultVoid::err(e),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_add_layer(
    ptr_queue: *mut CommandQueue,
    timeline_id: TimelineId,
    has_parent: bool,
    parent_folder_id: LayerFolderId,
    has_insert_index: bool,
    insert_index: usize,
    name: StringView,
) -> WrapperErrorCode {
    if ptr_queue.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let command = CommandRequest::AddLayer {
        parent_folder_id: has_parent.then_some(parent_folder_id),
        insert_index: has_insert_index.then_some(insert_index),
        name: name.as_string_lossy().into_owned(),
    };

    let queue = unsafe { &mut *ptr_queue };
    queue.enqueue(timeline_id, command);

    WrapperErrorCode::ok()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_add_folder(
    ptr_queue: *mut CommandQueue,
    timeline_id: TimelineId,
    has_parent: bool,
    parent_folder_id: LayerFolderId,
    has_insert_index: bool,
    insert_index: usize,
    name: StringView,
) -> WrapperErrorCode {
    if ptr_queue.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let command = CommandRequest::AddFolder {
        parent_folder_id: has_parent.then_some(parent_folder_id),
        insert_index: has_insert_index.then_some(insert_index),
        name: name.as_string_lossy().into_owned(),
    };

    let queue = unsafe { &mut *ptr_queue };
    queue.enqueue(timeline_id, command);

    WrapperErrorCode::ok()
}
