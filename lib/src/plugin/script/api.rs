use std::{
    ops::Range,
    sync::{Arc, Mutex, RwLock},
};

use dashmap::DashMap;
use glam::{EulerRot, Mat4, Quat, Vec3};

use crate::{
    StreamState,
    decode::streamplayer::StreamPlayer,
    network::NetworkHandler,
    project::{
        Project,
        ids::{ClipId, ResourceId, TimelineId},
    },
    render::{
        vertex::Vertex,
        video::{MediaFetchCache, builder::VertexBatch},
    },
    requests::Request,
    state::HostState,
};

/// プラグインスクリプトからアクションを呼び出すためのコンテキスト
pub struct PluginActionContext<N: NetworkHandler> {
    project: Arc<RwLock<Option<Project>>>,
    pub timeline_id: TimelineId,
    packets: Arc<Mutex<Vec<Request>>>,
    network: Arc<N>,
}

impl<N: NetworkHandler> Clone for PluginActionContext<N> {
    fn clone(&self) -> Self {
        Self {
            project: self.project.clone(),
            timeline_id: self.timeline_id,
            packets: self.packets.clone(),
            network: self.network.clone(),
        }
    }
}

impl<N: NetworkHandler> PluginActionContext<N> {
    pub fn new(
        network: Arc<N>,
        project: Arc<RwLock<Option<Project>>>,
        timeline_id: TimelineId,
    ) -> Self {
        Self {
            project,
            timeline_id,
            network,
            packets: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn network(&self) -> PluginNetworkContext<Request> {
        PluginNetworkContext {
            timeline_id: self.timeline_id,
            packets: self.packets.clone(),
        }
    }

    pub fn take_packets(&self) -> Vec<Request> {
        std::mem::take(&mut *self.packets.lock().expect("mutex poisoned"))
    }
}

/// ネットワーク通信（パケット発行）専用のサブコンテキスト
pub struct PluginNetworkContext<T> {
    pub timeline_id: TimelineId,
    packets: Arc<Mutex<Vec<T>>>,
}

impl<T> Clone for PluginNetworkContext<T> {
    fn clone(&self) -> Self {
        Self {
            timeline_id: self.timeline_id,
            packets: self.packets.clone(),
        }
    }
}

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

    pub fn set_transform(&mut self, transform: Mat4) {
        self.base_transform = transform;
    }

    pub fn apply_transform(&mut self, transform: Mat4) {
        self.base_transform *= transform;
    }

    /// Render at the source resolution, centered on the transform origin.
    pub fn render_video_native(&mut self, texture_id: i64) {
        let Ok(resource_id) = u32::try_from(texture_id) else {
            return;
        };
        let dimensions = self
            .streams
            .get(&resource_id)
            .map(|player| player.dimensions());
        let Some((width, height)) = dimensions else {
            return;
        };
        let (width, height) = (width as f64, height as f64);
        self.render_video(texture_id, -width / 2.0, -height / 2.0, width, height);
    }

    pub fn render_video(&mut self, texture_id: i64, x: f64, y: f64, width: f64, height: f64) {
        if texture_id < 0 {
            return;
        }
        self.batches
            .lock()
            .expect("mutex poisoned")
            .push(VertexBatch {
                vertices: Vertex::rect(0.0, 0.0, 1.0, 1.0, [1.0, 1.0, 1.0, 1.0]).to_vec(),
                texture_id: texture_id as u32,
                transform: self.base_transform
                    * Mat4::from_translation(Vec3::new(x as f32, y as f32, 0.0))
                    * Mat4::from_scale(Vec3::new(width as f32, height as f32, 1.0)),
            });
    }

    pub fn into_parts(self) -> (Vec<VertexBatch>, Vec<String>) {
        // Rhai may retain a cloned context; drain shared output without requiring unique ownership.
        (
            std::mem::take(&mut *self.batches.lock().expect("mutex poisoned")),
            std::mem::take(&mut *self.requested_paths.lock().expect("mutex poisoned")),
        )
    }
}

fn transform_matrix(
    position: rhai::Array,
    rotation: rhai::Array,
    scale: rhai::Array,
) -> Result<Mat4, Box<rhai::EvalAltResult>> {
    fn vector(values: rhai::Array) -> Result<Vec3, Box<rhai::EvalAltResult>> {
        if values.len() != 3 {
            return Err("transform vectors must contain three numbers".into());
        }
        let mut result = [0.0; 3];
        for (out, value) in result.iter_mut().zip(values) {
            let number = value
                .clone()
                .try_cast::<f64>()
                .or_else(|| value.try_cast::<i64>().map(|v| v as f64))
                .ok_or_else(|| {
                    Box::<rhai::EvalAltResult>::from("transform vectors must contain numbers")
                })?;
            *out = number as f32;
            if !out.is_finite() {
                return Err("transform values must be finite".into());
            }
        }
        Ok(Vec3::from_array(result))
    }
    let position = vector(position)?;
    let rotation = vector(rotation)? * (std::f32::consts::PI / 180.0);
    let scale = vector(scale)?;
    Ok(Mat4::from_scale_rotation_translation(
        scale,
        Quat::from_euler(EulerRot::XYZ, rotation.x, rotation.y, rotation.z),
        position,
    ))
}

pub const DEFAULT_LOOKAHEAD_SECONDS: f64 = 1.0;

pub(crate) fn register_render_api(engine: &mut rhai::Engine) {
    engine.register_type_with_name::<PluginRenderContext>("RenderContext");
    engine.register_type_with_name::<Mat4>("Mat4");
    engine.register_fn("mat4_trs", transform_matrix);
    engine.register_fn(
        "set_transform",
        |ctx: &mut PluginRenderContext, matrix: Mat4| ctx.set_transform(matrix),
    );
    engine.register_fn(
        "apply_transform",
        |ctx: &mut PluginRenderContext, matrix: Mat4| ctx.apply_transform(matrix),
    );

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

    engine.register_fn("render_video", |ctx: &mut PluginRenderContext, tex: i64| {
        ctx.render_video_native(tex)
    });
    engine.register_fn(
        "render_video",
        |ctx: &mut PluginRenderContext, tex: i64, x: f64, y: f64, w: f64, h: f64| {
            ctx.render_video(tex, x, y, w, h)
        },
    );

    engine.register_fn("media_time", |ctx: &mut PluginRenderContext| {
        ctx.media_time()
    });
}

/// クライアント/サーバー用の関数を登録
pub fn register_fn_for<T: HostState>(engine: &mut rhai::Engine) {
    type NetworkHandlerT<T: HostState> = <T as HostState>::NetworkHandler;
    register_render_api(engine);

    engine.register_type_with_name::<PluginActionContext<NetworkHandlerT<T>>>("ActionContext");
    engine.register_type_with_name::<PluginNetworkContext<Request>>("NetworkContext");

    // `ctx.network` のゲッター
    engine.register_get(
        "network",
        |ctx: &mut PluginActionContext<NetworkHandlerT<T>>| ctx.network(),
    );

    // NetworkContextのメソッド（オーバーロード）
    engine.register_fn("undo", |net: &mut PluginNetworkContext<Request>| {
        net.packets
            .lock()
            .expect("mutex poisoned")
            .push(Request::Undo {
                timeline_id: net.timeline_id,
            });
    });
    engine.register_fn(
        "undo",
        |net: &mut PluginNetworkContext<Request>, timeline_id: i64| {
            net.packets
                .lock()
                .expect("mutex poisoned")
                .push(Request::Undo {
                    timeline_id: timeline_id as u64,
                });
        },
    );

    engine.register_fn("redo", |net: &mut PluginNetworkContext<Request>| {
        net.packets
            .lock()
            .expect("mutex poisoned")
            .push(Request::Redo {
                timeline_id: net.timeline_id,
            });
    });
    engine.register_fn(
        "redo",
        |net: &mut PluginNetworkContext<Request>, timeline_id: i64| {
            net.packets
                .lock()
                .expect("mutex poisoned")
                .push(Request::Redo {
                    timeline_id: timeline_id as u64,
                });
        },
    );
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::plugin::{Plugin, script::bridge::field_values_to_rhai_map};
    use crate::project::clip::ClipBindingValue;

    #[test]
    fn standard_video_script_uses_source_resolution_and_transform_scale() {
        let plugin = Plugin::load(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plugins/std"
        )))
        .unwrap();
        let schemas = crate::project::clip::ClipKind::resolve_properties(
            &plugin.clip_kinds,
            plugin.pending_properties,
        )
        .unwrap();
        let fields = &schemas[&crate::plugin::NamespacedID::new("std", "video").unwrap()];
        assert!(
            !fields
                .iter()
                .any(|field| matches!(field.key.local_id(), "width" | "height"))
        );
        let mut props = ClipBindingValue::default_properties(fields);
        props.insert(
            crate::plugin::NamespacedID::new("std", "source").unwrap(),
            ClipBindingValue::Static(crate::plugin::property::value::FieldValue::String(
                "test.mp4".into(),
            )),
        );
        let cache = Arc::new(MediaFetchCache::default());
        let context = PluginRenderContext::new(
            1,
            2.0,
            Mat4::IDENTITY,
            Arc::new(DashMap::new()),
            Arc::new(DashMap::new()),
            cache,
        );
        let mut engine = rhai::Engine::new();
        register_render_api(&mut engine);
        let script = plugin.script.unwrap();
        script
            .call::<()>(
                &engine,
                "render_clip_video",
                (context.clone(), field_values_to_rhai_map(&props)),
            )
            .unwrap();
        let (_, paths) = context.clone().into_parts();
        assert_eq!(paths, vec!["test.mp4"]);
        // Supply a decoded frame at the playhead; the script must request 2s, not 0s.
        let mut player =
            StreamPlayer::new_from_metadata(ffmpeg_next::codec::Id::H264, 32, 24, 1.0 / 30.0, &[])
                .unwrap();
        player.frames.insert(
            ordered_float::OrderedFloat(2.0),
            ffmpeg_next::util::frame::video::Video::new(ffmpeg_next::format::Pixel::RGBA, 32, 24),
        );
        context
            .path_to_stream
            .insert("test.mp4".into(), StreamState::Loaded(7));
        context.streams.insert(7, player);
        for factor in [1.0, 0.5, 2.0] {
            props.insert(
                crate::plugin::NamespacedID::new("std", "scale").unwrap(),
                ClipBindingValue::Static(crate::plugin::property::value::FieldValue::Array(
                    [factor, factor, 1.0]
                        .into_iter()
                        .map(crate::plugin::property::value::FieldValue::Float)
                        .collect(),
                )),
            );
            script
                .call::<()>(
                    &engine,
                    "render_clip_video",
                    (context.clone(), field_values_to_rhai_map(&props)),
                )
                .unwrap();
            let (batches, _) = context.clone().into_parts();
            assert_eq!(batches.len(), 1);
            let model = batches[0].transform;
            assert_eq!(
                model.transform_point3(Vec3::ZERO),
                Vec3::new(-16.0 * factor as f32, -12.0 * factor as f32, 0.0)
            );
            assert_eq!(
                model.transform_point3(Vec3::new(1.0, 1.0, 0.0)),
                Vec3::new(16.0 * factor as f32, 12.0 * factor as f32, 0.0)
            );
        }
    }

    #[test]
    fn transform_uses_degrees_and_rejects_invalid_vectors() {
        fn array(v: &[f64]) -> rhai::Array {
            v.iter().map(|v| rhai::Dynamic::from(*v)).collect()
        }
        let matrix = transform_matrix(
            array(&[10.0, 20.0, 0.0]),
            array(&[0.0, 0.0, 90.0]),
            array(&[2.0, 3.0, 1.0]),
        )
        .unwrap();
        assert!((matrix.transform_point3(Vec3::X) - Vec3::new(10.0, 22.0, 0.0)).length() < 0.0001);
        assert!(transform_matrix(array(&[0.0]), array(&[0.0; 3]), array(&[1.0; 3])).is_err());
        assert!(
            transform_matrix(array(&[f64::NAN; 3]), array(&[0.0; 3]), array(&[1.0; 3])).is_err()
        );
    }
}
