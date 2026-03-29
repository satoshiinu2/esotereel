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
    position_moved: i64,
    duration_added: i64,
    layer_moved: isize,
) {
    let lock = PROJECT.read().unwrap();
    let Some(project) = lock.as_ref() else {
        return;
    };
    let Ok(timeline) = project.get_timeline(timeline_type) else {
        return;
    };

    let clip_ids = unsafe { slice::from_raw_parts(ptr, len) }.to_vec();

    let clip_ctxs = clip_ids
        .iter()
        .filter_map(|clip_id| {
            let (layer_idx, _, clip) = timeline.find_clip_by_id(*clip_id)?;

            Some(ClipMoveCtx {
                clip_id: *clip_id,
                new_position: (clip.position + position_moved),
                new_duration: (clip.duration + duration_added),
                new_layer: usize::try_from(layer_idx as isize + layer_moved).ok()?,
            })
        })
        .collect();

    let cmd = Command::ClipsMove {
        timeline_idx: timeline_type,
        clips: clip_ctxs,
    };
    send_command(cmd);
}
