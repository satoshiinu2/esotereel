use std::slice;

use nomyoedit_lib::{
    command::{Command, send_command},
    types::ClipMoveCtx,
};

use crate::PROJECT;

#[unsafe(no_mangle)]
pub extern "C" fn cmd_test() {
    let cmd = Command::Test;
    send_command(cmd);
}

#[unsafe(no_mangle)]
pub extern "C" fn cmd_new_project() {
    let cmd = Command::NewProject;
    send_command(cmd);
}

#[unsafe(no_mangle)]
pub extern "C" fn cmd_clip_move_mul(
    timeline_type: usize,
    ptr: *const u64,
    len: usize,
    frame_moved: i32,
    layer_moved: i32,
) {
    let project_guard = PROJECT.read().unwrap();
    let Some(project) = project_guard.as_ref() else {
        return;
    };
    let timeline = project.get_timeline(timeline_type);

    let clip_ids = unsafe { slice::from_raw_parts(ptr, len) }.to_vec();

    let clip_ctxs = clip_ids
        .iter()
        .filter_map(|clip_id| {
            let (layer_idx, _, clip) = timeline.find_clip_by_id(*clip_id)?;

            Some(ClipMoveCtx {
                clip_id: *clip_id,
                new_frame: u64::try_from(clip.position as i64 + frame_moved as i64).ok()?,
                new_layer: u32::try_from(layer_idx as i64 + layer_moved as i64).ok()?,
            })
        })
        .collect();

    let cmd = Command::ClipsMove {
        timeline_type,
        clips: clip_ctxs,
    };
    send_command(cmd);
}
