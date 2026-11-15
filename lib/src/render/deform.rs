//! クリップのシェーダスタックを描画バッチへ適用するモジュールです。
//!
//! `build_vertices` が記録したクリップ単位のバッチ範囲について、スタックの
//! compute シェーダを順に `ComputeStackRunner` で実行し、変形済み頂点バッファを
//! `RenderBatch::gpu_vertices` に差し替えます。描画側（`pipeline`）はその
//! バッファを共有頂点バッファへコピーしてから通常のパイプラインで描くため、
//! バッチ構造や変換行列の適用（変形後に 1 回だけ）は従来と変わりません。
//!
//! シェーダが見つからない・compute でないエントリは警告を出して読み飛ばし、
//! 残りのエントリで変形を続行します。パラメータのパッキングに失敗した場合は
//! そのバッチの変形全体を中止し、変形なしで描画します。

use std::{collections::HashMap, sync::Arc};

use glam::Mat4;

use crate::{
    plugin::{
        NamespacedID,
        shader::{
            ComputePass, CoreParamValue, ShaderDefinition, ShaderKind, ShaderProperty, pack_params,
        },
    },
    project::clip::ShaderStackEntry,
    render::{RenderBatch, RenderContext, video::builder::ClipDeformation, wgpuutil::WGpuUtil},
};

/// クリップのシェーダスタックを、そのクリップの全バッチへ適用します。
///
/// `core:` パラメータの供給値は、`core:time` がクリップのメディア時刻、
/// `core:transform` が対象バッチの変換行列、`core:view_projection` と
/// `core:screen_size` がフレーム共通の値です。宣言されていない値は渡しません
/// （`pack_params` が未知キーをエラーにするため、定義ごとに必要な値だけを渡します）。
pub fn apply_shader_stacks(
    util: &mut WGpuUtil,
    ctx: &RenderContext,
    batches: &mut [RenderBatch],
    deformations: &[ClipDeformation],
    screen_size: [f32; 2],
    view_projection: Mat4,
) {
    if deformations.is_empty() {
        return;
    }

    // 参照されるシェーダ定義への `Arc` を集め、ロックを解放してから GPU 処理する。
    let definitions: HashMap<NamespacedID, Arc<ShaderDefinition>> = {
        let Ok(loader) = ctx.plugin_loader.read() else {
            log::warn!("plugin_loader lock poisoned, skipping shader stacks");
            return;
        };
        deformations
            .iter()
            .flat_map(|d| d.shader_stack.iter().map(|e| &e.shader_id))
            .filter_map(|id| loader.get_shader(id).map(|def| (id.clone(), def)))
            .collect()
    };

    for deformation in deformations {
        // スタックに載せられるのは compute シェーダだけです。
        // 見つからない定義と render 用シェーダは警告して読み飛ばします。
        let mut entries: Vec<(&ShaderStackEntry, Arc<ShaderDefinition>)> = Vec::new();
        for entry in &deformation.shader_stack {
            let Some(definition) = definitions.get(&entry.shader_id) else {
                log::warn!(
                    "shader `{}` not found, skipping stack entry",
                    entry.shader_id.full()
                );
                continue;
            };
            if !matches!(definition.kind, ShaderKind::Compute { .. }) {
                log::warn!(
                    "shader `{}` is not a compute shader, skipping stack entry",
                    entry.shader_id.full()
                );
                continue;
            }
            entries.push((entry, definition.clone()));
        }
        if entries.is_empty() {
            continue;
        }

        for batch_index in deformation.batch_range.clone() {
            let Some(batch) = batches.get_mut(batch_index) else {
                continue;
            };
            if batch.vertices.is_empty() {
                continue;
            }

            // バッチの変換行列が `core:transform` になるため、パスはバッチごとに組む。
            let mut passes = Vec::with_capacity(entries.len());
            let mut pack_failed = false;
            for (entry, definition) in &entries {
                let core_values = core_values_for(
                    definition,
                    deformation.media_time,
                    screen_size,
                    view_projection,
                    batch.transform,
                );
                match pack_params(definition, &core_values, &entry.properties) {
                    Ok(params) => passes.push(ComputePass { definition, params }),
                    Err(e) => {
                        log::warn!(
                            "failed to pack params for shader `{}`: {e}",
                            definition.id.full()
                        );
                        pack_failed = true;
                        break;
                    }
                }
            }
            if pack_failed || passes.is_empty() {
                continue;
            }

            let input = bytemuck::cast_slice(batch.vertices.as_slice());
            match util
                .compute_runner
                .run(&util.device, &util.queue, input, &passes)
            {
                Ok(buffer) => batch.gpu_vertices = Some(buffer),
                Err(e) => log::warn!("shader stack failed for clip batch, drawing undeformed: {e}"),
            }
        }
    }
}

/// シェーダ定義が宣言する `core:` パラメータについてだけ、供給値のマップを作ります。
fn core_values_for(
    definition: &ShaderDefinition,
    media_time: f64,
    screen_size: [f32; 2],
    view_projection: Mat4,
    transform: Mat4,
) -> HashMap<String, CoreParamValue> {
    let mut values = HashMap::new();
    for property in &definition.properties {
        let ShaderProperty::Core { key, .. } = property else {
            continue;
        };
        let value = match key.as_str() {
            "core:time" => CoreParamValue::Float32(media_time as f32),
            "core:screen_size" => CoreParamValue::Vec2F32(screen_size),
            "core:view_projection" => CoreParamValue::Mat4x4F32(view_projection.to_cols_array()),
            "core:transform" => CoreParamValue::Mat4x4F32(transform.to_cols_array()),
            // 宣言可能な `core:` キーは読み込み時に検証済みで、上記以外は存在しない。
            _ => continue,
        };
        values.insert(key.clone(), value);
    }
    values
}
