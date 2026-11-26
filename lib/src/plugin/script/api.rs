use std::{
    ops::Range,
    sync::{Arc, Mutex, RwLock},
};

use dashmap::DashMap;
use glam::{EulerRot, Mat4, Quat, Vec3};

use std::collections::BTreeMap;

use crate::{
    plugin::{
        PluginPermission,
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
    util::result::{LockExt, MutexExt},
};

/// 権限リストが指定した権限を含むか（`*` は全てを含む）。
fn allows(permissions: &[PluginPermission], permission: PluginPermission) -> bool {
    permissions.contains(&PluginPermission::All) || permissions.contains(&permission)
}

/// プラグインスクリプトからアクションを呼び出すためのコンテキスト
pub struct PluginActionContext<N: NetworkHandler> {
    project: Arc<RwLock<Option<Project>>>,
    pub timeline_id: TimelineId,
    packets: Arc<Mutex<Vec<Request>>>,
    network: Arc<N>,
    /// スクリプトを実行しているプラグインの ID（権限チェック用）。
    plugin_id: String,
    /// `cross_plugin` 権限の有無。他プラグイン所有のプロパティへの
    /// 書き込みを許可するか。
    allow_cross_plugin: bool,
}

impl<N: NetworkHandler> Clone for PluginActionContext<N> {
    fn clone(&self) -> Self {
        Self {
            project: Arc::clone(&self.project),
            timeline_id: self.timeline_id,
            packets: Arc::clone(&self.packets),
            network: Arc::clone(&self.network),
            plugin_id: self.plugin_id.clone(),
            allow_cross_plugin: self.allow_cross_plugin,
        }
    }
}

impl<N: NetworkHandler> PluginActionContext<N> {
    pub fn new(
        network: Arc<N>,
        project: Arc<RwLock<Option<Project>>>,
        timeline_id: TimelineId,
        plugin_id: String,
        allow_cross_plugin: bool,
    ) -> Self {
        Self {
            project,
            timeline_id,
            network,
            packets: Arc::new(Mutex::new(Vec::new())),
            plugin_id,
            allow_cross_plugin,
        }
    }

    pub fn network(&self) -> PluginNetworkContext<Request> {
        PluginNetworkContext {
            timeline_id: self.timeline_id,
            packets: Arc::clone(&self.packets),
        }
    }

    pub fn take_packets(&self) -> Vec<Request> {
        match self.packets.lock_or_err() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(_) => {
                log::warn!("packets lock poisoned");
                Vec::new()
            }
        }
    }

    /// クリップのプロパティをパス指定で設定します。
    ///
    /// Rhai スクリプトから `set_property(clip_id, "std:media#media/source_offset", 100)` のように
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

        // 他プラグインが所有するプロパティへの書き込みは `cross_plugin`
        // 権限がなければ拒否する（自プラグインのプロパティは対象外）。
        let owner_plugin = prop_path.root().owner().plugin_id();
        if owner_plugin != self.plugin_id && !self.allow_cross_plugin {
            return Err(Box::<rhai::EvalAltResult>::from(format!(
                "permission denied: writing properties of plugin `{owner_plugin}` requires the `cross_plugin` permission"
            )));
        }

        let normalized = dynamic_to_field_value(value).map_err(|e| {
            Box::<rhai::EvalAltResult>::from(format!("value normalization failed: {e}"))
        })?;

        let mut project_guard = self.project.write_or_err().map_err(|_| {
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
/// スクリプト値の入れ子の深さの上限（スタック溢れの防止）。
const MAX_VALUE_DEPTH: usize = 64;

fn dynamic_to_field_value(
    value: rhai::Dynamic,
) -> Result<FieldValue, Box<rhai::EvalAltResult>> {
    dynamic_to_field_value_with_depth(value, 0)
}

fn dynamic_to_field_value_with_depth(
    value: rhai::Dynamic,
    depth: usize,
) -> Result<FieldValue, Box<rhai::EvalAltResult>> {
    if depth > MAX_VALUE_DEPTH {
        return Err(Box::<rhai::EvalAltResult>::from(
            "value nested too deeply for property setting",
        ));
    }
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
            result.push(dynamic_to_field_value_with_depth(item, depth + 1)?);
        }
        return Ok(FieldValue::Array(result));
    }
    if let Some(map) = value.try_cast::<rhai::Map>() {
        let mut result = BTreeMap::new();
        for (k, v) in map {
            result.insert(k.to_string(), dynamic_to_field_value_with_depth(v, depth + 1)?);
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
            packets: Arc::clone(&self.packets),
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
                    .lock_or_err()
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
        if let Ok(mut guard) = self.batches.lock_or_err() {
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
        if let Ok(mut guard) = self.batches.lock_or_err() {
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
        let batches = match self.batches.lock_or_err() {
            Ok(mut g) => std::mem::take(&mut *g),
            Err(error) => {
                log::error!("render batches lock poisoned, dropping output: {error}");
                Vec::new()
            }
        };
        let paths = match self.requested_paths.lock_or_err() {
            Ok(mut g) => std::mem::take(&mut *g),
            Err(error) => {
                log::error!("requested paths lock poisoned, dropping output: {error}");
                Vec::new()
            }
        };
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

/// 描画系 API をエンジンへ登録する。
///
/// `get_texture` 等のメディア読み取りは `media` 権限がある場合だけ登録する。
/// それ以外（変換・描画・時刻）は描画の本体として常時登録する。
pub(crate) fn register_render_api(engine: &mut rhai::Engine, permissions: &[PluginPermission]) {
    super::props::register_props_api(engine);
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

    if allows(permissions, PluginPermission::Media) {
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
    }

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
///
/// アクション系は権限でゲートする: `set_property` は `property_write`、
/// `undo` / `redo` は `history` がある場合だけ登録する。どちらの権限も
/// 無いプラグインには ActionContext 自体を登録しない。
pub fn register_fn_for<T: HostState>(engine: &mut rhai::Engine, permissions: &[PluginPermission]) {
    type NetworkHandlerT<T: HostState> = <T as HostState>::NetworkHandler;
    register_render_api(engine, permissions);

    let property_write = allows(permissions, PluginPermission::PropertyWrite);
    let history = allows(permissions, PluginPermission::History);
    if !property_write && !history {
        return;
    }

    engine.register_type_with_name::<PluginActionContext<NetworkHandlerT<T>>>("ActionContext");
    engine.register_type_with_name::<PluginNetworkContext<Request>>("NetworkContext");

    // `ctx.network` のゲッター
    engine.register_get(
        "network",
        |ctx: &mut PluginActionContext<NetworkHandlerT<T>>| ctx.network(),
    );

    if history {
        register_history_api(engine);
    }
    if property_write {
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
}

/// `history` 権限の API（NetworkContext の undo / redo）を登録する。
fn register_history_api(engine: &mut rhai::Engine) {
    engine.register_fn("undo", |net: &mut PluginNetworkContext<Request>| {
        if let Ok(mut guard) = net.packets.lock_or_err() {
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
            if let Ok(mut guard) = net.packets.lock_or_err() {
                guard.push(Request::Undo {
                    timeline_id: timeline_id as u64,
                });
            } else {
                log::warn!("packets lock poisoned");
            }
        },
    );

    engine.register_fn("redo", |net: &mut PluginNetworkContext<Request>| {
        if let Ok(mut guard) = net.packets.lock_or_err() {
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
            if let Ok(mut guard) = net.packets.lock_or_err() {
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
#[path = "../../../tests/unit/plugin/script/api_render_tests.rs"]
mod render_tests;

#[cfg(test)]
#[path = "../../../tests/unit/plugin/script/api_normalization_tests.rs"]
mod normalization_tests;
