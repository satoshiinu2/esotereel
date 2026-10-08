use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::{
    project::ids::{LayerFolderId, LayerId, TimelineId},
    util::result::EsotereelError,
};

use crate::{
    WrapperErrorCode,
    commands::CommandQueue,
    ffi::{
        result::FfiResultVoid,
        state::{ClientStateHandle, OptionProject},
        stringview::FfiStringView,
    },
    slice_from_ptr_or_empty,
};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn command_queue_new() -> *mut CommandQueue {
    Box::into_raw(Box::new(CommandQueue::default()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn command_queue_drop(ptr: *mut CommandQueue) -> FfiResultVoid {
    fn inner(ptr: *mut CommandQueue) -> anyhow::Result<()> {
        if ptr.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_queue".to_owned()))
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
        if ptr_queue.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_queue".to_owned()))
        }
        if ptr_state.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_state".to_owned()))
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
) -> FfiResultVoid {
    fn inner(
        ptr_queue: *mut CommandQueue,
        ptr_opt_project: *const OptionProject,
        timeline_id: TimelineId,
        ptr: *const u64,
        len: usize,
        position_moved: i64,
        duration_added: i64,
        layer_moved: isize,
    ) -> anyhow::Result<()> {
        if ptr_queue.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_queue".to_owned()));
        }
        if ptr_opt_project.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_opt_project".to_owned()));
        }

        let project = match unsafe { &*ptr_opt_project } {
            Some(arc) => arc,
            None => anyhow::bail!(EsotereelError::ProjectNotFound),
        };

        let clip_ids = unsafe { slice_from_ptr_or_empty(ptr, len) };

        let queue = unsafe { &mut *ptr_queue };
        queue.req_cmd_clip_move_mul(
            project,
            timeline_id,
            clip_ids,
            position_moved,
            duration_added,
            layer_moved,
        )?;

        Ok(())
    }

    FfiResultVoid::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        inner(
            ptr_queue,
            ptr_opt_project,
            timeline_id,
            ptr,
            len,
            position_moved,
            duration_added,
            layer_moved,
        )
    })))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_clip_resize_mul(
    ptr_queue: *mut CommandQueue,
    ptr_opt_project: *const OptionProject,
    timeline_id: TimelineId,
    ptr: *const u64,
    len: usize,
    left_edge: bool,
    frame_delta: i64,
) -> FfiResultVoid {
    fn inner(
        ptr_queue: *mut CommandQueue,
        ptr_opt_project: *const OptionProject,
        timeline_id: TimelineId,
        ptr: *const u64,
        len: usize,
        left_edge: bool,
        frame_delta: i64,
    ) -> anyhow::Result<()> {
        if ptr_queue.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_queue".to_owned()));
        }
        if ptr_opt_project.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_opt_project".to_owned()));
        }

        let project = match unsafe { &*ptr_opt_project } {
            Some(arc) => arc,
            None => anyhow::bail!(EsotereelError::ProjectNotFound),
        };

        let clip_ids = unsafe { slice_from_ptr_or_empty(ptr, len) };

        let queue = unsafe { &mut *ptr_queue };
        queue.req_cmd_clip_resize_mul(
            project,
            timeline_id,
            clip_ids,
            left_edge,
            frame_delta,
        )?;

        Ok(())
    }

    FfiResultVoid::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        inner(
            ptr_queue,
            ptr_opt_project,
            timeline_id,
            ptr,
            len,
            left_edge,
            frame_delta,
        )
    })))
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

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state_guard = state.lock().expect("mutex poisoned");

        let queue = unsafe { &mut *ptr_queue };
        queue.req_cmd_add_clip_dummy(&state_guard, timeline_id, position, layer_id)?;

        Ok(())
    }

    FfiResultVoid::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        inner(ptr_queue, ptr_state, timeline_id, position, layer_id)
    })))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_add_layer(
    ptr_queue: *mut CommandQueue,
    timeline_id: TimelineId,
    has_parent: bool,
    parent_folder_id: LayerFolderId,
    has_insert_index: bool,
    insert_index: usize,
    name: FfiStringView,
) -> FfiResultVoid {
    fn inner(
        ptr_queue: *mut CommandQueue,
        timeline_id: TimelineId,
        has_parent: bool,
        parent_folder_id: LayerFolderId,
        has_insert_index: bool,
        insert_index: usize,
        name: FfiStringView,
    ) -> anyhow::Result<()> {
        if ptr_queue.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_queue".to_string()));
        }

        let queue = unsafe { &mut *ptr_queue };
        queue.req_cmd_add_layer(
            timeline_id,
            has_parent,
            parent_folder_id,
            has_insert_index,
            insert_index,
            name.as_str().unwrap_or(""),
        );
        Ok(())
    }

    FfiResultVoid::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        inner(
            ptr_queue,
            timeline_id,
            has_parent,
            parent_folder_id,
            has_insert_index,
            insert_index,
            name,
        )
    })))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_cmd_add_folder(
    ptr_queue: *mut CommandQueue,
    timeline_id: TimelineId,
    has_parent: bool,
    parent_folder_id: LayerFolderId,
    has_insert_index: bool,
    insert_index: usize,
    name: FfiStringView,
) -> FfiResultVoid {
    fn inner(
        ptr_queue: *mut CommandQueue,
        timeline_id: TimelineId,
        has_parent: bool,
        parent_folder_id: LayerFolderId,
        has_insert_index: bool,
        insert_index: usize,
        name: FfiStringView,
    ) -> anyhow::Result<()> {
        if ptr_queue.is_null() {
            anyhow::bail!(EsotereelError::NullPointer("ptr_queue".to_string()));
        }

        let queue = unsafe { &mut *ptr_queue };
        queue.req_cmd_add_folder(
            timeline_id,
            has_parent,
            parent_folder_id,
            has_insert_index,
            insert_index,
            name.as_str().unwrap_or(""),
        );

        Ok(())
    }

    FfiResultVoid::from_panic_result_result(catch_unwind(AssertUnwindSafe(|| {
        inner(
            ptr_queue,
            timeline_id,
            has_parent,
            parent_folder_id,
            has_insert_index,
            insert_index,
            name,
        )
    })))
}
