use esotereel_lib::{
    project::camera::CameraInfo,
    project::ids::TimelineId,
    render::{RenderContext, render_frame_offscreen},
    render::wgpuutil::{OffscreenTarget, WGpuUtil},
    util::result::EsotereelError,
};

use crate::ffi::array::FfiArray;
use crate::state::ClientState;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameRenderResult {
    pub width: u32,
    pub height: u32,
    pub data: FfiArray<u8>,
}

pub fn render_frame_offscreen_ffi(
    wgpuutil: &mut WGpuUtil,
    offscreen: &OffscreenTarget,
    state: &ClientState,
    camera_info: &CameraInfo,
    timeline_id: TimelineId,
    current_frame: i64,
) -> anyhow::Result<FrameRenderResult> {
    let project_guard = state.project.read().expect("mutex poisoned");

    let project = match project_guard.as_ref() {
        Some(arc) => Ok(arc),
        None => Err(EsotereelError::ProjectNotFound),
    }?;

    let timeline = match project.timeline_ref(timeline_id) {
        Some(tl) => Ok(tl),
        None => Err(EsotereelError::TimelineNotFound(timeline_id)),
    }?;

    let ctx = RenderContext {
        path_to_stream: &state.stream_state_map,
        streams: &state.stream_players,

        media_fetch_cache: &state.media_fetch_cache,
        plugin_loader: &state.plugin_loader,
        script_engine: &state.script_engine,

        timeline,
        camera_info,
        current_frame,
    };

    render_frame_offscreen(wgpuutil, offscreen, &ctx)
        .map_err(|msg| EsotereelError::RenderError(msg))?;

    let bytes = offscreen
        .readback(&wgpuutil.device)
        .map_err(|msg| EsotereelError::RenderError(msg))?;

    Ok(FrameRenderResult {
        data: FfiArray::from_vec(bytes),
        width: offscreen.width,
        height: offscreen.height,
    })
}
