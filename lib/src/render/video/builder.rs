use std::ops::Range;
use std::sync::{Arc, RwLock};

use crate::{
    plugin::{
        clip::component::Scheduler,
        property::{ComponentCapability, value::FieldValue},
        script::api::PluginRenderContext,
    },
    project::{Clip, Project, clip::ShaderStackEntry, ids::AssetRef},
    render::{RenderContext, vertex::Vertex},
    util::result::LockExt,
};
use glam::Mat4;

/// 1 つのテクスチャと変換行列をまとめた頂点バッチです。
///
/// `texture_id` でどのビデオテクスチャを使用するかを識別します。
pub struct VertexBatch {
    pub vertices: Vec<Vertex>,
    pub texture_id: u32,
    pub transform: Mat4,
}

/// `build_vertices` の結果です。
///
/// `batches` が描画対象の頂点バッチ列、`deformations` がそのうち
/// シェーダスタックを持つクリップのバッチ範囲と変形入力です。
/// スタックが空のクリップは `deformations` に現れず、従来どおり描画されます。
pub struct VertexBuildOutput<'a> {
    pub batches: Vec<VertexBatch>,
    pub deformations: Vec<ClipDeformation<'a>>,
}

/// クリップ 1 件分の頂点変形入力です。
///
/// `batch_range` は `VertexBuildOutput::batches` のうちこのクリップが
/// 生成した範囲を指します。変形は常にこのフレームで構築された元の
/// 頂点列を入力とするため、フレームをまたいだ累積は起きません。
pub struct ClipDeformation<'a> {
    pub batch_range: Range<usize>,
    /// クリップのメディア時刻（秒）。`core:time` の供給値です。
    pub media_time: f64,
    /// クリップのシェーダスタック（適用順）。クリップ本体からの借用で、
    /// フレーム構築中だけ使われる。
    pub shader_stack: &'a [ShaderStackEntry],
}

/// レンダリングコンテキストから頂点バッチを構築します。
///
/// 各レイヤーの実行順序を走査し、有効なレイヤーに配置されたクリップについて
/// コンポーネントスケジューラを実行し、プラグインから頂点バッチを取得します。
/// シェーダスタックを持つクリップについては、そのバッチ範囲とメディア時刻を
/// `VertexBuildOutput::deformations` に記録し、描画側で compute 変形を適用します。
///
/// # 能力ベースプロパティ参照
///
/// プロパティ ID（`std:media#media` 等）をコード中にハードコードせず、
/// コンポーネント定義の `capability` フィールドから `FrameProvider` などの能力を
/// 逆引きしてプロパティキーを解決します。メディアプロパティのメンバー能力が
/// 未宣言の場合は宣言不備として、そのクリップを描画しません。
///
/// # スケジューラ
///
/// 従来の `script.rhai` + `kind.render_script` 経由ではなく、コンポーネントが
/// 宣言する能力（`frame-provider`, `prefetchable`, `transformable` 等）に基づき、
/// 各標準関数（`render_frame`, `prefetch`, `get_transform` …）をフックタイミング順に
/// 呼び出します。
pub fn build_vertices<'a>(ctx: &RenderContext<'a>) -> VertexBuildOutput<'a> {
    let mut batches = vec![];
    let mut deformations = vec![];

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

        let Ok(loader) = ctx.plugin_loader.read_or_err() else {
            log::warn!("plugin_loader lock poisoned, skipping clip");
            continue;
        };

        let Ok(runtime_def) = loader.resolve_component_definition(&clip.kind_id) else {
            log::warn!(
                "no component definition for clip kind {}, skipping render",
                clip.kind_id.full()
            );
            continue;
        };

        // コンポーネントが宣言する能力から、どのプロパティを参照すべきかを解決する。
        // メディアプロパティはクリップ種別を構成するいずれかのコンポーネントが
        // 提供するため、構成全体から `FrameProvider` 能力を持つプロパティを逆引きする。
        let media_property = runtime_def.property_by_capability(&ComponentCapability::FrameProvider);
        let media_property_key = media_property.map(|p| &p.key);

        // メンバキーもハードコードせず、メンバー自身が宣言する能力から
        // 解決する。メディアプロパティがあるのにメンバー能力が宣言されて
        // いない場合は宣言不備として、そのクリップを描画しない。
        // メンバー名を推測して続行はしない。
        let member_key = |capability: ComponentCapability| -> Option<&str> {
            media_property.and_then(|p| p.member_key_by_capability(&capability))
        };
        let source_member_key = member_key(ComponentCapability::MediaSource);
        let offset_member_key = member_key(ComponentCapability::MediaOffsetProvider);
        if media_property.is_some()
            && (source_member_key.is_none() || offset_member_key.is_none())
        {
            log::warn!(
                "media property of clip kind {} does not declare media-source / media-offset-provider members, skipping render",
                clip.kind_id.full()
            );
            continue;
        }

        let mut media_time = compute_media_seconds(clip, ctx.timeline.tps, ctx.current_frame);
        let mut video_source: Option<AssetRef> = None;

        if let (Some(media_key), Some(source_member_key), Some(offset_member_key)) =
            (media_property_key, source_member_key, offset_member_key)
        {
            if let Some(FieldValue::Map(media_map)) = clip
                .properties
                .get(media_key)
                .and_then(|v| v.in_frame_of())
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

                if let Some(FieldValue::AssetRef(asset_ref)) = media_map.get(source_member_key) {
                    if !asset_ref.is_nil() {
                        video_source = Some(*asset_ref);
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

        if let Some(asset_ref) = video_source {
            match resolve_video_source(asset_ref, ctx.project) {
                Some(resolved) => render_ctx.set_video_source(resolved),
                None => {
                    // 参照先がレジストリで解決できない（未登録または Missing）。
                    // 存在しないパスをストリーム初期化に回さないよう、映像を供給しない。
                    log::warn!(
                        "video source asset `{asset_ref}` could not be resolved in the project asset registry, skipping"
                    );
                }
            }
        }

        let scheduler = Scheduler::new();
        if let Err(e) = scheduler.run_clip_with_properties(
            &*loader,
            &clip.properties,
            &runtime_def,
            &mut render_ctx,
        ) {
            log::warn!("component scheduler failed for clip {}: {e}", clip.id);
        }

        let batch_start = batches.len();
        let (plugin_batches, requested_paths) = render_ctx.into_parts();
        batches.extend(plugin_batches);
        for path in requested_paths {
            ctx.media_fetch_cache.record_init_request(path);
        }

        // シェーダスタックを持つクリップは、生成したバッチ範囲を変形対象として記録する。
        if !clip.shader_stack.is_empty() && batches.len() > batch_start {
            deformations.push(ClipDeformation {
                batch_range: batch_start..batches.len(),
                media_time,
                shader_stack: &clip.shader_stack,
            });
        }
    }
    VertexBuildOutput {
        batches,
        deformations,
    }
}

fn compute_media_seconds(clip: &Clip, global_tps: f64, current_frame: i64) -> f64 {
    let relative_frame = current_frame - clip.position;
    if relative_frame < 0 {
        return 0.0;
    }
    relative_frame as f64 / global_tps
}

/// アセット参照をレンダリング用のファイルパスに解決する。
///
/// `Project.assets`（アセットレジストリ）経由で実パスを解決する。
/// 未登録・Missing の場合は `None` を返し、存在しないパスを
/// ストリーム初期化に回さない。
fn resolve_video_source(
    asset_ref: AssetRef,
    project: &Arc<RwLock<Option<Project>>>,
) -> Option<String> {
    let guard = project.read_or_err().ok()?;
    let project = guard.as_ref()?;
    let path = project.assets().resolve(asset_ref).ok()?;
    Some(path.to_string_lossy().to_string())
}

#[cfg(test)]
#[path = "../../../tests/unit/render/video/builder_tests.rs"]
mod tests;
