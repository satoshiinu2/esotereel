use std::{
    ops::Range,
    sync::{Arc, Mutex, RwLock},
};

use dashmap::DashMap;
use glam::{Mat4, Vec3};

use crate::{
    StreamState,
    decode::streamplayer::StreamPlayer,
    network::NetworkHandler,
    plugin::property::special::{Transform, TransformValues},
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
        match self.packets.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(_) => {
                log::warn!("packets lock poisoned");
                Vec::new()
            }
        }
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
    video_source: Option<String>,

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
            video_source: None,
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

    pub fn set_video_source(&mut self, source: String) {
        self.video_source = Some(source);
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
                    .map(|mut g| g.push(path.to_string()))
                    .unwrap_or_else(|_| log::warn!("requested_paths lock poisoned"));
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

    pub fn render_video_with_transform(&mut self, texture_id: i64, transform: Mat4) {
        let Ok(resource_id) = u32::try_from(texture_id) else {
            return;
        };
        let has_dimensions = self
            .streams
            .get(&resource_id)
            .map(|player| player.dimensions())
            .is_some_and(|(width, height)| width > 0 && height > 0);
        if !has_dimensions {
            return;
        }
        if texture_id < 0 {
            return;
        }
        if let Ok(mut guard) = self.batches.lock() {
            guard.push(VertexBatch {
                vertices: Vertex::rect(0.0, 0.0, 1.0, 1.0, [1.0, 1.0, 1.0, 1.0]).to_vec(),
                texture_id: resource_id,
                transform: self.base_transform
                    * transform
                    * Mat4::from_translation(Vec3::new(-0.5, -0.5, 0.0)),
            });
        } else {
            log::warn!("batches lock poisoned");
        }
    }

    pub fn render_video(&mut self, texture_id: i64, x: f64, y: f64, width: f64, height: f64) {
        if texture_id < 0 {
            return;
        }
        if let Ok(mut guard) = self.batches.lock() {
            guard.push(VertexBatch {
                vertices: Vertex::rect(0.0, 0.0, 1.0, 1.0, [1.0, 1.0, 1.0, 1.0]).to_vec(),
                texture_id: texture_id as u32,
                transform: self.base_transform
                    * Mat4::from_translation(Vec3::new(x as f32, y as f32, 0.0))
                    * Mat4::from_scale(Vec3::new(width as f32, height as f32, 1.0)),
            });
        } else {
            log::warn!("batches lock poisoned");
        }
    }

    pub fn into_parts(self) -> (Vec<VertexBatch>, Vec<String>) {
        // Rhai may retain a cloned context; drain shared output without requiring unique ownership.
        let batches = self
            .batches
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default();
        let paths = self
            .requested_paths
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default();
        (batches, paths)
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
    Ok(TransformValues {
        position: vector(position)?.to_array(),
        rotation: vector(rotation)?.to_array(),
        scale: vector(scale)?.to_array(),
    }
    .matrix())
}

pub const DEFAULT_LOOKAHEAD_SECONDS: f64 = 1.0;

pub(crate) fn register_render_api(engine: &mut rhai::Engine) {
    engine.register_type_with_name::<PluginRenderContext>("RenderContext");
    engine.register_type_with_name::<Mat4>("Mat4");
    engine.register_fn("mat4_trs", transform_matrix);
    engine.register_fn("as_mat4", |fields: rhai::Map| {
        Transform::from_rhai_map(&fields).map(TransformValues::matrix)
    });
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
    engine.register_fn(
        "get_video_texture",
        |ctx: &mut PluginRenderContext| -> Result<i64, Box<rhai::EvalAltResult>> {
            let source = ctx
                .video_source
                .clone()
                .ok_or_else(|| Box::<rhai::EvalAltResult>::from("video source is not set"))?;
            Ok(ctx.get_texture(&source, ctx.media_time(), DEFAULT_LOOKAHEAD_SECONDS))
        },
    );

    engine.register_fn(
        "render_video",
        |ctx: &mut PluginRenderContext, tex: i64, transform: Mat4| {
            ctx.render_video_with_transform(tex, transform)
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
        if let Ok(mut guard) = net.packets.lock() {
            guard.push(Request::Undo {
                timeline_id: net.timeline_id,
            });
        } else {
            log::warn!("packets lock poisoned");
        }
    });
    engine.register_fn(
        "undo",
        |net: &mut PluginNetworkContext<Request>, timeline_id: i64| {
            if let Ok(mut guard) = net.packets.lock() {
                guard.push(Request::Undo {
                    timeline_id: timeline_id as u64,
                });
            } else {
                log::warn!("packets lock poisoned");
            }
        },
    );

    engine.register_fn("redo", |net: &mut PluginNetworkContext<Request>| {
        if let Ok(mut guard) = net.packets.lock() {
            guard.push(Request::Redo {
                timeline_id: net.timeline_id,
            });
        } else {
            log::warn!("packets lock poisoned");
        }
    });
    engine.register_fn(
        "redo",
        |net: &mut PluginNetworkContext<Request>, timeline_id: i64| {
            if let Ok(mut guard) = net.packets.lock() {
                guard.push(Request::Redo {
                    timeline_id: timeline_id as u64,
                });
            } else {
                log::warn!("packets lock poisoned");
            }
        },
    );
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::plugin::{Plugin, script::bridge::field_values_to_rhai_map};
    use crate::project::clip::ClipBindingValue;
    use crate::project::video_clip::VideoClipProperties;

    #[test]
    fn standard_video_script_uses_pixel_dimensions_from_transform_scale() {
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
        let [
            crate::plugin::property::PropertySchema {
                kind:
                    crate::plugin::property::value::FieldTypeKind::Group {
                        fields: video_fields,
                        ..
                    },
                ..
            },
        ] = fields.as_slice()
        else {
            panic!("video property root")
        };
        let [
            crate::plugin::property::PropertySchema {
                kind:
                    crate::plugin::property::value::FieldTypeKind::Special {
                        reference: media_reference,
                        ..
                    },
                ..
            },
            crate::plugin::property::PropertySchema {
                kind:
                    crate::plugin::property::value::FieldTypeKind::Special {
                        reference: transform_reference,
                        ..
                    },
                ..
            },
        ] = video_fields.as_slice()
        else {
            panic!("video schema must use core special properties")
        };
        assert_eq!(media_reference.full(), "core:media");
        assert_eq!(transform_reference.full(), "core:transform");
        assert!(
            !fields
                .iter()
                .any(|field| matches!(field.key.local_id(), "width" | "height"))
        );
        let leaves = crate::plugin::property::PropertySchema::leaf_fields(fields).unwrap();
        assert_eq!(
            leaves
                .iter()
                .map(|(path, _)| path.to_string())
                .collect::<Vec<_>>(),
            [
                "std:video/media/source",
                "std:video/media/source_offset",
                "std:video/transform/position",
                "std:video/transform/rotation",
                "std:video/transform/scale",
            ]
        );
        let mut props = ClipBindingValue::default_properties(fields);
        let ClipBindingValue::Static(crate::plugin::property::value::FieldValue::Map(video)) =
            props
                .get_mut(&crate::plugin::NamespacedID::new("std", "video").unwrap())
                .unwrap()
        else {
            panic!("video group")
        };
        let Some(crate::plugin::property::value::FieldValue::Map(media)) = video.get_mut("media")
        else {
            panic!("media group")
        };
        media.insert(
            "source".into(),
            crate::plugin::property::value::FieldValue::String("test.mp4".into()),
        );
        let cache = Arc::new(MediaFetchCache::default());
        let path_to_stream = Arc::new(DashMap::new());
        let streams = Arc::new(DashMap::new());
        let mut context = PluginRenderContext::new(
            1,
            2.0,
            Mat4::IDENTITY,
            Arc::clone(&path_to_stream),
            Arc::clone(&streams),
            Arc::clone(&cache),
        );
        context.set_video_source("test.mp4".into());
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
        path_to_stream.insert("test.mp4".into(), StreamState::Loaded(7));
        streams.insert(7, player);
        for factor in [1.0, 0.5, 2.0] {
            let ClipBindingValue::Static(crate::plugin::property::value::FieldValue::Map(video)) =
                props
                    .get_mut(&crate::plugin::NamespacedID::new("std", "video").unwrap())
                    .unwrap()
            else {
                panic!("video group")
            };
            let Some(crate::plugin::property::value::FieldValue::Map(transform)) =
                video.get_mut("transform")
            else {
                panic!("transform group")
            };
            transform.insert(
                "scale".into(),
                crate::plugin::property::value::FieldValue::Array(
                    [32.0 * factor, 24.0 * factor, 1.0]
                        .into_iter()
                        .map(crate::plugin::property::value::FieldValue::Float)
                        .collect(),
                ),
            );
            let video_properties = VideoClipProperties::from_value(
                &crate::plugin::property::value::FieldValue::Map(video.clone()),
            )
            .unwrap();
            let mut context = PluginRenderContext::new(
                1,
                2.0,
                Mat4::IDENTITY,
                Arc::clone(&path_to_stream),
                Arc::clone(&streams),
                Arc::clone(&cache),
            );
            context.set_video_source(video_properties.media.source);
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
