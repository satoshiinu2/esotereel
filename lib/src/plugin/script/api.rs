use std::{
    ops::Range,
    sync::{Arc, Mutex, RwLock},
};

use dashmap::DashMap;
use glam::{EulerRot, Mat4, Quat, Vec3};

use std::collections::BTreeMap;

use crate::{
    plugin::{
        property::path::PropertyPath,
        property::value::FieldValue,
    },
    project::clip::ClipBindingValue,
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
        match self.packets.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(_) => {
                log::warn!("packets lock poisoned");
                Vec::new()
            }
        }
    }

    /// クリップのプロパティをパス指定で設定します。
    ///
    /// Rhai スクリプトから `set_property(clip_id, "std:video:media/source_offset", 100)` のように
    /// 呼び出し、値を型に応じて正規化（int⇄float 相互変換、文字列→数値パース、bool パースなど）
    /// した上でクリッププロパティに書き込みます。
    pub fn set_clip_property(
        &self,
        clip_id: u64,
        path: &str,
        value: rhai::Dynamic,
    ) -> Result<(), Box<rhai::EvalAltResult>> {
        let prop_path = PropertyPath::parse(path).map_err(|e| {
            Box::<rhai::EvalAltResult>::from(format!("invalid property path `{path}`: {e}"))
        })?;

        let normalized = dynamic_to_field_value(value).map_err(|e| {
            Box::<rhai::EvalAltResult>::from(format!("value normalization failed: {e}"))
        })?;

        let mut project_guard = self.project.write().map_err(|_| {
            Box::<rhai::EvalAltResult>::from("project lock poisoned")
        })?;
        let project = project_guard
            .as_mut()
            .ok_or_else(|| Box::<rhai::EvalAltResult>::from("project not found"))?;

        let timeline = project
            .timeline_mut(self.timeline_id)
            .ok_or_else(|| Box::<rhai::EvalAltResult>::from("timeline not found"))?;

        let clip = timeline
            .get_clip_mut(clip_id)
            .ok_or_else(|| Box::<rhai::EvalAltResult>::from(format!("clip {clip_id} not found")))?;

        clip.set_property_path(&prop_path, ClipBindingValue::Static(normalized))
            .map_err(|e| Box::<rhai::EvalAltResult>::from(format!("failed to set property: {e}")))
    }
}

/// Rhai の動的値を `FieldValue` に正規化します。
///
/// 型に応じて `Int`/`Float`/`Bool`/`String`/`Array`/`Map` に変換し、
/// 文字列からの数値パースや int⇄float 相互変換などを行います。
fn dynamic_to_field_value(
    value: rhai::Dynamic,
) -> Result<FieldValue, Box<rhai::EvalAltResult>> {
    // まず現在の型をチェックし、そのまま変換できる場合はそのまま使う
    if let Some(b) = value.clone().try_cast::<bool>() {
        return Ok(FieldValue::Bool(b));
    }
    if let Some(i) = value.clone().try_cast::<i64>() {
        return Ok(FieldValue::Int(i));
    }
    if let Some(f) = value.clone().try_cast::<f64>() {
        return Ok(FieldValue::Float(f));
    }
    if let Some(s) = value.clone().try_cast::<String>() {
        // 文字列の場合：bool / int / float のパースを試行し、すべて失敗したら String として扱う
        if let Ok(b) = s.parse::<bool>() {
            return Ok(FieldValue::Bool(b));
        }
        if let Ok(i) = s.parse::<i64>() {
            return Ok(FieldValue::Int(i));
        }
        if let Ok(f) = s.parse::<f64>() {
            return Ok(FieldValue::Float(f));
        }
        return Ok(FieldValue::String(s));
    }
    if let Some(arr) = value.clone().try_cast::<rhai::Array>() {
        let mut result = Vec::with_capacity(arr.len());
        for item in arr {
            result.push(dynamic_to_field_value(item)?);
        }
        return Ok(FieldValue::Array(result));
    }
    if let Some(map) = value.clone().try_cast::<rhai::Map>() {
        let mut result = BTreeMap::new();
        for (k, v) in map {
            result.insert(k.to_string(), dynamic_to_field_value(v)?);
        }
        return Ok(FieldValue::Map(result));
    }

    Err(Box::<rhai::EvalAltResult>::from(
        "unsupported value type for property setting",
    ))
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

/// プラグインレンダリングコンテキストです。
///
/// スクリプトからビデオテクスチャの取得、変換の設定、頂点バッチの生成を行うための
/// API を提供します。`render_video` / `render_video_with_transform` で頂点バッチを
/// 積み上げ、`into_parts` で回収します。
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformValues {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

impl TransformValues {
    pub fn matrix(self) -> Mat4 {
        let rotation = Vec3::from_array(self.rotation) * (std::f32::consts::PI / 180.0);
        Mat4::from_scale_rotation_translation(
            Vec3::from_array(self.scale),
            Quat::from_euler(EulerRot::XYZ, rotation.x, rotation.y, rotation.z),
            Vec3::from_array(self.position),
        )
    }
}

fn read_rhai_vector(
    fields: &rhai::Map,
    key: &str,
) -> Result<[f32; 3], Box<rhai::EvalAltResult>> {
    let values = fields
        .get(key)
        .cloned()
        .and_then(rhai::Dynamic::try_cast::<rhai::Array>)
        .ok_or_else(|| {
            Box::<rhai::EvalAltResult>::from(format!("transform property `{key}` must be an array"))
        })?;
    if values.len() != 3 {
        return Err(
            format!("transform property `{key}` must contain exactly three numbers").into(),
        );
    }

    let mut vector = [0.0; 3];
    for (output, value) in vector.iter_mut().zip(values) {
        let number = value
            .clone()
            .try_cast::<f64>()
            .or_else(|| value.try_cast::<i64>().map(|v| v as f64))
            .ok_or_else(|| {
                Box::<rhai::EvalAltResult>::from(format!(
                    "transform property `{key}` must contain numbers"
                ))
            })?;
        *output = number as f32;
        if !number.is_finite() || !output.is_finite() {
            return Err(format!("transform property `{key}` must contain finite numbers").into());
        }
    }
    Ok(vector)
}

pub const DEFAULT_LOOKAHEAD_SECONDS: f64 = 1.0;

pub(crate) fn register_render_api(engine: &mut rhai::Engine) {
    engine.register_type_with_name::<PluginRenderContext>("RenderContext");
    engine.register_type_with_name::<Mat4>("Mat4");
    engine.register_fn("mat4_trs", transform_matrix);
    engine.register_fn("as_mat4", |fields: rhai::Map| -> Result<Mat4, Box<rhai::EvalAltResult>> {
        let position = read_rhai_vector(&fields, "position")?;
        let rotation = read_rhai_vector(&fields, "rotation")?;
        let scale = read_rhai_vector(&fields, "scale")?;
        Ok(TransformValues {
            position,
            rotation,
            scale,
        }
        .matrix())
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
            let Some(source) = ctx.video_source.clone() else {
                return Ok(-1);
            };
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

    // `ctx.set_property(clip_id, path, value)` — クリッププロパティを正規化付きで設定
    engine.register_fn(
        "set_property",
        |ctx: &mut PluginActionContext<NetworkHandlerT<T>>,
         clip_id: u64,
         path: &str,
         value: rhai::Dynamic|
         -> Result<(), Box<rhai::EvalAltResult>> {
            ctx.set_clip_property(clip_id, path, value)
        },
    );
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::plugin::{Plugin, script::bridge::field_values_to_rhai_map};
    use crate::project::clip::ClipBindingValue;

    #[test]
    fn standard_video_script_uses_pixel_dimensions_from_transform_scale() {
        let plugin = Plugin::load(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plugins/std"
        )))
        .unwrap();
        // コンポーネントがプロパティを提供する。ルートコンポーネント std:video の
        // プロパティスキーマを取得し、core:media / core:transform 特別プロパティを含むことを確認する。
        let component = plugin
            .component_definitions
            .get(&crate::plugin::NamespacedID::new("std", "video").unwrap())
            .unwrap();
        let fields = &component.properties;
        let media = fields
            .iter()
            .find(|f| f.key.local_id() == "video:media")
            .unwrap();
        let transform = fields
            .iter()
            .find(|f| f.key.local_id() == "video:transform")
            .unwrap();
        let crate::plugin::property::value::FieldTypeKind::Group { .. } = &media.kind
        else {
            panic!("media property must be a group")
        };
        let crate::plugin::property::value::FieldTypeKind::Map { .. } = &transform.kind
        else {
            panic!("transform property must be a map")
        };
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
            ["std:video:media/source", "std:video:media/source_offset", "std:video:transform"]
        );
        let mut props = ClipBindingValue::default_properties(fields);
        // コンポーネントのプロパティはフラットなキー（std:video:media, std:video:transform）で配置される。
        // transform デフォルト値をセット（as_mat4 は position/rotation/scale を全て要求する）
        let transform_value = props
            .get_mut(&crate::plugin::NamespacedID::new("std", "video:transform").unwrap())
            .unwrap();
        let ClipBindingValue::Static(
            crate::plugin::property::value::FieldValue::Map(transform_map),
        ) = transform_value
        else {
            panic!("transform property must be a map")
        };
        let vec = |v: [f64; 3]| {
            crate::plugin::property::value::FieldValue::Array(
                v.into_iter().map(crate::plugin::property::value::FieldValue::Float).collect(),
            )
        };
        transform_map.insert("position".into(), vec([0.0, 0.0, 0.0]));
        transform_map.insert("rotation".into(), vec([0.0, 0.0, 0.0]));
        transform_map.insert("scale".into(), vec([1.0, 1.0, 1.0]));

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
        let script = plugin
            .component_scripts
            .get(&crate::plugin::NamespacedID::new("std", "video").unwrap())
            .unwrap();
        script
            .call::<()>(
                &engine,
                "render_frame",
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
            let transform_value = props
                .get_mut(&crate::plugin::NamespacedID::new("std", "video:transform").unwrap())
                .unwrap();
            let ClipBindingValue::Static(
                crate::plugin::property::value::FieldValue::Map(transform_map),
            ) = transform_value
            else {
                panic!("transform property must be a map")
            };
            transform_map.insert(
                "scale".into(),
                crate::plugin::property::value::FieldValue::Array(
                    [32.0 * factor, 24.0 * factor, 1.0]
                        .into_iter()
                        .map(crate::plugin::property::value::FieldValue::Float)
                        .collect(),
                ),
            );
            let mut context = PluginRenderContext::new(
                1,
                2.0,
                Mat4::IDENTITY,
                Arc::clone(&path_to_stream),
                Arc::clone(&streams),
                Arc::clone(&cache),
            );
            context.set_video_source("test.mp4".into());
            script
                .call::<()>(
                    &engine,
                    "render_frame",
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

#[cfg(test)]
mod normalization_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn dynamic_to_field_value_int_stays_int() {
        assert_eq!(
            dynamic_to_field_value(42i64.into()).unwrap(),
            FieldValue::Int(42)
        );
    }

    #[test]
    fn dynamic_to_field_value_float_stays_float() {
        assert_eq!(
            dynamic_to_field_value(3.14f64.into()).unwrap(),
            FieldValue::Float(3.14)
        );
    }

    #[test]
    fn dynamic_to_field_value_bool_stays_bool() {
        assert_eq!(
            dynamic_to_field_value(true.into()).unwrap(),
            FieldValue::Bool(true)
        );
    }

    #[test]
    fn dynamic_to_field_value_string_stays_string() {
        assert_eq!(
            dynamic_to_field_value("hello".into()).unwrap(),
            FieldValue::String("hello".to_string())
        );
    }

    #[test]
    fn dynamic_to_field_value_numeric_string_parses() {
        assert_eq!(
            dynamic_to_field_value("42".into()).unwrap(),
            FieldValue::Int(42)
        );
        assert_eq!(
            dynamic_to_field_value("3.14".into()).unwrap(),
            FieldValue::Float(3.14)
        );
        assert_eq!(
            dynamic_to_field_value("true".into()).unwrap(),
            FieldValue::Bool(true)
        );
    }

    #[test]
    fn dynamic_to_field_value_array() {
        let arr = rhai::Array::from(vec![
            rhai::Dynamic::from(1i64),
            rhai::Dynamic::from(2.5f64),
            rhai::Dynamic::from("three".to_string()),
        ]);
        assert_eq!(
            dynamic_to_field_value(arr.into()).unwrap(),
            FieldValue::Array(vec![
                FieldValue::Int(1),
                FieldValue::Float(2.5),
                FieldValue::String("three".to_string()),
            ])
        );
    }

    #[test]
    fn dynamic_to_field_value_map() {
        let mut map = rhai::Map::new();
        map.insert("count".into(), rhai::Dynamic::from(10i64));
        map.insert("name".into(), rhai::Dynamic::from("test".to_string()));
        assert_eq!(
            dynamic_to_field_value(map.into()).unwrap(),
            FieldValue::Map(
                BTreeMap::from([
                    ("count".to_string(), FieldValue::Int(10)),
                    ("name".to_string(), FieldValue::String("test".to_string())),
                ])
            )
        );
    }
}
