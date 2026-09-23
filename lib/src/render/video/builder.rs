use std::sync::Arc;

use crate::{
    plugin::script::{api::PluginRenderContext, bridge::field_values_to_rhai_map},
    project::Clip,
    render::{RenderContext, vertex::Vertex},
};
use glam::{EulerRot, Mat4, Quat, Vec3};

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

        let plugin_id = clip.kind_id.plugin_id();

        let loader = ctx.plugin_loader.read().expect("mutex poisoned");
        let Some(kind) = loader.get_clip_kind(&clip.kind_id) else {
            continue;
        };
        let Some(script) = loader.get_script(plugin_id) else {
            continue;
        };
        // drop(loader);

        let media_time = compute_media_seconds(clip, ctx.timeline.tps, ctx.current_frame);
        let base_transform = clip
            .translates
            .get_translate_at()
            .map(|t| {
                Mat4::from_scale_rotation_translation(
                    Vec3::from_array(t.scale),
                    Quat::from_euler(EulerRot::XYZ, t.rotation[0], t.rotation[1], t.rotation[2]),
                    Vec3::from_array(t.position),
                )
            })
            .unwrap_or(Mat4::IDENTITY);

        let render_ctx = PluginRenderContext::new(
            clip.id,
            media_time,
            base_transform,
            Arc::clone(ctx.path_to_stream),
            Arc::clone(ctx.streams),
            Arc::clone(ctx.media_fetch_cache),
        );
        let props_dynamic = field_values_to_rhai_map(&clip.properties);

        // info!(
        //     "clip.properties: {:?} props_dynamic: {:?}",
        //     clip.properties, props_dynamic
        // );

        let call_result: Result<(), _> =
            script.call(&kind.func_name, (render_ctx.clone(), props_dynamic));

        if let Err(e) = call_result {
            log::warn!(
                "plugin render script `{}` failed: {}",
                clip.kind_id.full(),
                e
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
