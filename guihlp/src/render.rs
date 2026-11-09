use esotereel_lib::{
    project::camera::CameraInfo,
    project::ids::TimelineId,
    render::video::request::request_stream_packets_for_time,
    render::wgpuutil::{OffscreenTarget, WGpuUtil},
    render::{RenderContext, render_frame_offscreen},
    util::result::{EsotereelError, LockExt},
};

use crate::state::ClientState;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameRenderResult {
    pub width: u32,
    pub height: u32,
    pub data: crate::ffi::array::FfiArray<u8>,
}

/// GPUレンダリング（コマンド記録+submit+ネットワーク送信）を実行。
/// readbackは呼び出し側でClientStateロック解放後に実行する。
pub fn render_frame_offscreen_ffi(
    wgpuutil: &mut WGpuUtil,
    offscreen: &OffscreenTarget,
    state: &ClientState,
    camera_info: &CameraInfo,
    timeline_id: TimelineId,
    current_frame: i64,
) -> anyhow::Result<()> {
    let project_guard = state.project.read_or_err()?;

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
        project: &state.project,
        timeline: &timeline,
        camera_info,
        current_frame,
    };

    render_frame_offscreen(wgpuutil, offscreen, &ctx)
        .map_err(|msg| EsotereelError::RenderError(msg))?;

    // 一時停止プレビューでもメディア初期化/フェッチは実行。
    for request in request_stream_packets_for_time(
        &state.stream_state_map,
        &state.stream_players,
        &state.media_fetch_cache,
    ) {
        state.network.send(&request);
    }

    Ok(())
}
