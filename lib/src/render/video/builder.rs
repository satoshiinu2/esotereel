use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    plugin::{
        clip::component::{ClipInstance, Scheduler},
        property::{
            value::FieldValue,
            ComponentCapability,
        },
        script::api::PluginRenderContext,
    },
    project::Clip,
    render::{RenderContext, vertex::Vertex},
};
use glam::Mat4;

pub struct VertexBatch {
    pub vertices: Vec<Vertex>,
    pub texture_id: u32, // ここでどのテクスチャを使用するか識別する
    pub transform: Mat4,
}

pub fn build_vertices(ctx: &RenderContext) -> Vec<VertexBatch> {
    let mut batches = vec![];

    for layer in ctx.timeline.iter_execution_order() {
        if !layer.enabled {
            continue;
        }

        let Some(clip_id) = layer.get_clip_id_at(ctx.current_frame) else {
            continue;
        };
        let Some(clip) = ctx.timeline.get_clip(clip_id) else {
            continue;
        };

        let loader = match ctx.plugin_loader.read() {
            Ok(loader) => loader,
            Err(e) => {
                log::warn!("plugin lock is poisoned: {e}",);
                continue;
            }
        };

        // clip → kind → components: 能力ベースのレンダリングをスケジューラで実行。
        // 従来の script.rhai + kind.render_script 経由ではなく、コンポーネントが
        // 宣言する能力（frame-provider, prefetchable, transformable など）に基づき、
        // 各標準関数（render_frame, prefetch, get_transform …）をフックタイミング順に呼び出す。
        let Ok(runtime_def) = loader.resolve_component_definition(&clip.kind_id) else {
            log::warn!(
                "no component definition for clip kind {}, skipping render",
                clip.kind_id.full()
            );
            continue;
        };

        // コンポーネントが宣言する能力から、どのプロパティを参照すべきかを解決する。
        // プロパティID（std:video:media など）をコード中にハードコードせず、
        // コンポーネント定義の `capability` フィールドから逆引きする。
        let media_property_key = runtime_def
            .components
            .iter()
            .find(|c| c.id == clip.kind_id)
            .and_then(|c| {
                c.properties
                    .iter()
                    .find(|p| p.capability == Some(ComponentCapability::FrameProvider))
                    .map(|p| p.key.clone())
            });

        // メンバキー（"source", "source_offset" 等）もハードコードせず、
        // プロパティ定義の `member_capabilities` から能力ベースで解決する。
        // 未宣言の場合は既定のメンバー名にフォールバックする。
        let media_component_def = runtime_def
            .components
            .iter()
            .find(|c| c.id == clip.kind_id);
        let source_member_key = media_component_def
            .and_then(|c| {
                c.properties.iter().find(|p| {
                    p.capability == Some(ComponentCapability::FrameProvider)
                }).and_then(|p| {
                    p.member_capabilities.iter()
                        .find(|(_, cap)| **cap == ComponentCapability::MediaSource)
                        .map(|(key, _)| key.as_str())
                })
            })
            .unwrap_or("source");
        let offset_member_key = media_component_def
            .and_then(|c| {
                c.properties.iter().find(|p| {
                    p.capability == Some(ComponentCapability::FrameProvider)
                }).and_then(|p| {
                    p.member_capabilities.iter()
                        .find(|(_, cap)| **cap == ComponentCapability::MediaOffsetProvider)
                        .map(|(key, _)| key.as_str())
                })
            })
            .unwrap_or("source_offset");

        let mut media_time = compute_media_seconds(clip, ctx.timeline.tps, ctx.current_frame);
        let mut video_source: Option<String> = None;

        if let Some(media_key) = media_property_key {
            if let Some(FieldValue::Map(media_map)) =
                clip.properties.get(&media_key).and_then(|v| v.in_frame_of().cloned())
            {
                let source_offset = media_map
                    .get(offset_member_key)
                    .and_then(|v| match v {
                        FieldValue::Int(i) => Some(*i as f64),
                        FieldValue::Float(f) => Some(*f),
                        _ => None,
                    })
                    .unwrap_or(0.0);
                let relative_frame = ctx.current_frame.saturating_sub(clip.position);
                let seconds = (relative_frame as f64 + source_offset) / ctx.timeline.tps;
                if seconds.is_finite() {
                    media_time = seconds;
                }

                // 同じメディアプロパティから source 文字列も取得し、ビデオソースとして設定する。
                // 空文字列の場合はソース未設定とみなし、ビデオをレンダリングしない。
                if let Some(FieldValue::String(source)) = media_map.get(source_member_key) {
                    if !source.is_empty() {
                        video_source = Some(source.clone());
                    }
                }
            }
        }

        let mut render_ctx = PluginRenderContext::new(
            clip.id,
            media_time,
            Mat4::IDENTITY,
            Arc::clone(ctx.path_to_stream),
            Arc::clone(ctx.streams),
            Arc::clone(ctx.media_fetch_cache),
        );

        if let Some(source) = video_source {
            render_ctx.set_video_source(source);
        }

        let instance = ClipInstance {
            kind_id: clip.kind_id.clone(),
            properties: clip.properties.clone(),
            state: HashMap::new(),
        };
        let scheduler = Scheduler::new();
        if let Err(e) = scheduler.run_clip(ctx.script_engine, &*loader, &instance, &runtime_def, &mut render_ctx) {
            log::warn!(
                "component scheduler failed for clip {}: {e}",
                clip.id
            );
        }

        // スクリプトが core_render_video を何回呼んでもここで全部回収される
        let (plugin_batches, requested_paths) = render_ctx.into_parts();
        batches.extend(plugin_batches);
        for path in requested_paths {
            ctx.media_fetch_cache.record_init_request(path);
        }
    }
    batches
}

fn compute_media_seconds(clip: &Clip, global_tps: f64, current_frame: i64) -> f64 {
    let relative_frame = current_frame - clip.position;
    if relative_frame < 0 {
        return 0.0;
    }
    relative_frame as f64 / global_tps
}
