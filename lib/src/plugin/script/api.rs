use std::{
    ops::Range,
    sync::{Arc, Mutex},
};

use dashmap::DashMap;
use glam::Mat4;
use log::info;

use crate::{
    StreamState,
    decode::streamplayer::StreamPlayer,
    project::ids::{ClipId, ResourceId},
    render::{
        vertex::Vertex,
        video::{MediaFetchCache, builder::VertexBatch},
    },
};

#[derive(Clone)]
pub struct PluginRenderContext {
    clip_id: ClipId,
    media_time: f64,
    base_transform: Mat4,

    batches: Arc<Mutex<Vec<VertexBatch>>>,
    /// 未ロードで申告されたpath
    requested_paths: Arc<Mutex<Vec<String>>>,

    path_to_stream: Arc<DashMap<String, StreamState>>,
    streams: Arc<DashMap<ResourceId, StreamPlayer>>,
    plugin_cache: Arc<MediaFetchCache>,
}

impl PluginRenderContext {
    pub fn new(
        clip_id: ClipId,
        media_time: f64,
        base_transform: Mat4,
        path_to_stream: Arc<DashMap<String, StreamState>>,
        streams: Arc<DashMap<ResourceId, StreamPlayer>>,
        plugin_cache: Arc<MediaFetchCache>,
    ) -> Self {
        Self {
            clip_id,
            media_time,
            base_transform,
            batches: Arc::new(Mutex::new(Vec::new())),
            requested_paths: Arc::new(Mutex::new(Vec::new())),
            path_to_stream,
            streams,
            plugin_cache,
        }
    }

    pub fn media_time(&self) -> f64 {
        self.media_time
    }

    pub fn get_texture(&mut self, path: &str, start_seconds: f64, lookahead_seconds: f64) -> i64 {
        let range: Range<f64> = start_seconds..(start_seconds + lookahead_seconds.max(0.0));

        match self.path_to_stream.get(path).map(|s| *s) {
            Some(StreamState::Loaded(resource_id)) => {
                self.plugin_cache
                    .record(self.clip_id, path.to_string(), range);
                let has_frame = self
                    .streams
                    .get(&resource_id)
                    .map(|p| p.get_frame_at(start_seconds).is_some())
                    .unwrap_or(false);
                if has_frame { resource_id as i64 } else { -1 }
            }
            Some(StreamState::Loading) => -1,
            None => {
                self.path_to_stream
                    .insert(path.to_string(), StreamState::Loading);
                self.requested_paths
                    .lock()
                    .expect("mutex poisoned")
                    .push(path.to_string());
                -1
            }
        }
    }

    pub fn render_video(&mut self, texture_id: i64, x: f64, y: f64, width: f64, height: f64) {
        if texture_id < 0 {
            return;
        }
        self.batches
            .lock()
            .expect("mutex poisoned")
            .push(VertexBatch {
                vertices: Vertex::rect(
                    x as f32,
                    y as f32,
                    width as f32,
                    height as f32,
                    [1.0, 1.0, 1.0, 1.0],
                )
                .to_vec(),
                texture_id: texture_id as u32,
                transform: self.base_transform,
            });
    }

    pub fn into_parts(self) -> (Vec<VertexBatch>, Vec<String>) {
        // idc this code wtf
        (
            Arc::try_unwrap(self.batches)
                .map(|m| m.into_inner().unwrap_or_default())
                .unwrap_or_default(),
            Arc::try_unwrap(self.requested_paths)
                .map(|m| m.into_inner().unwrap_or_default())
                .unwrap_or_default(),
        )
    }
}

pub const DEFAULT_LOOKAHEAD_SECONDS: f64 = 1.0;

pub(super) fn register_fn_for(engine: &mut rhai::Engine) {
    engine.register_type_with_name::<PluginRenderContext>("RenderContext");

    engine.register_fn(
        "get_texture",
        |ctx: &mut PluginRenderContext, path: &str, start: f64, lookahead: f64| {
            ctx.get_texture(path, start, lookahead)
        },
    );
    engine.register_fn(
        "get_texture",
        |ctx: &mut PluginRenderContext, path: &str, start: f64| {
            ctx.get_texture(path, start, DEFAULT_LOOKAHEAD_SECONDS)
        },
    );

    engine.register_fn(
        "render_video",
        |ctx: &mut PluginRenderContext, tex: i64, x: f64, y: f64, w: f64, h: f64| {
            ctx.render_video(tex, x, y, w, h)
        },
    );

    engine.register_fn("media_time", |ctx: &mut PluginRenderContext| {
        ctx.media_time()
    });

    engine.register_fn("core_undo", core_undo);
    engine.register_fn("core_redo", core_redo);
}

#[deprecated]
fn core_undo() {
    info!("undo placeholder called");
}

#[deprecated]
fn core_redo() {
    info!("redo placeholder called");
}
