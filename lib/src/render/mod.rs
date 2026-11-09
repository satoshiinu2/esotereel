use std::sync::{Arc, RwLock};

use crate::{
    StreamState,
    decode::streamplayer::StreamPlayer,
    plugin::PluginLoader,
    project::{Project, Timeline, TimelineTick, camera::CameraInfo, ids::ResourceId},
    render::{
        video::{MediaFetchCache, update_timline_clips_texture},
        wgpuutil::OffscreenTarget,
    },
};
use dashmap::DashMap;
use glam::Mat4;

use crate::render::{vertex::Vertex, video::builder::build_vertices, wgpuutil::WGpuUtil};

pub mod pipeline;
pub mod uniform;
pub mod vertex;
pub mod video;
pub mod wgpuutil;

/// 1 フレームのレンダーに必要なコンテキストです。
///
/// 各フィールドはライフタイム `'a` で結びつけられ、レンダースレッドから
/// 参照されます。`plugin_loader` は `Arc<RwLock<>>` で共有され、
/// GUI スレッドとの競合を避けるためロックを適切に取得して使用します。
pub struct RenderContext<'a> {
    /// パス → ストリーム状態のマップ。
    pub path_to_stream: &'a Arc<DashMap<String, StreamState>>,
    /// リソース ID → ストリームプレイヤーのマップ。
    pub streams: &'a Arc<DashMap<ResourceId, StreamPlayer>>,
    /// メディアフェッチキャッシュ。
    pub media_fetch_cache: &'a Arc<MediaFetchCache>,

    /// プラグインローダー。`Arc<RwLock<>>` で共有される。
    pub plugin_loader: &'a Arc<RwLock<PluginLoader>>,
    /// Rhai スクリプトエンジン。
    pub script_engine: &'a rhai::Engine,

    /// プロジェクト。アセットレジストリ（`Project.assets`）へのアクセスに使用し、
    /// 動画ソースがアセットID（UUID）形式の場合に実パスを解決する。
    pub project: &'a Arc<RwLock<Option<Project>>>,
    /// 現在レンダー対象のタイムライン。
    pub timeline: &'a Timeline,
    /// カメラ情報（ビュープロジェクション行列の計算に使用）。
    pub camera_info: &'a CameraInfo,
    /// 現在のフレーム（タイムラインティック）。
    pub current_frame: TimelineTick,
}

/// 1 バッチ分のレンダー命令を表します。
///
/// 頂点データ・テクスチャバインドグループ・変換行列をまとめ、
/// `WgpuRenderResources::record_render_commands` でまとめて描画されます。
#[derive(Debug)]
pub struct RenderBatch {
    pub vertices: Vec<Vertex>,
    pub texture_bind_group: wgpu::BindGroup,
    pub transform: Mat4,
}

/// オフスクリーンテクスチャへ 1 フレームをレンダーします。
///
/// ウィンドウサイズが 0 の場合はエラーを返します。カメラのビュープロジェクション行列を
/// 計算し、頂点を構築してテクスチャを更新し、wgpu のレンダーパスで描画後、
/// 結果を `offscreen.buffer` にコピーして GPU 読み戻しの準備を行います。
///
/// GPU 読み戻しの前には `ClientState` ロックを解放することで GUI フリーズを防ぎます
///（コミット `67b6d06` 参照）。
pub fn render_frame_offscreen(
    util: &mut WGpuUtil,
    offscreen: &OffscreenTarget,
    ctx: &RenderContext,
) -> Result<(), String> {
    if offscreen.width == 0 || offscreen.height == 0 {
        return Err("window size is 0".into());
    }

    let screen_size = [offscreen.width as f32, offscreen.height as f32];

    // ビュープロジェクション行列の計算
    let proj_matrix = ctx.camera_info.get_proj_mat(screen_size);
    let view_matrix = ctx.camera_info.get_view_mat();

    let view_projection_matrix = proj_matrix * view_matrix;

    // スクリプトで必要なフレームを記録してから、テクスチャを更新する。
    let vertices = build_vertices(ctx);
    update_timline_clips_texture(util, ctx);

    let batches: Vec<RenderBatch> = if !vertices.is_empty() {
        vertices
            .into_iter()
            .map(|b| {
                let bind_group = util
                    .textures
                    .get(&b.texture_id)
                    .map(|(_, bg)| bg.clone())
                    .unwrap_or_else(|| util.resources.dummy_bind_group.clone());

                RenderBatch {
                    vertices: b.vertices,
                    texture_bind_group: bind_group,
                    transform: b.transform,
                }
            })
            .collect()
    } else {
        vec![]
    };

    // バッファの更新（RenderPass開始前に行う）
    if !batches.is_empty() {
        util.resources.update_buffers(
            &util.device,
            &util.queue,
            screen_size,
            view_projection_matrix,
            &batches,
        );
    }

    let view = &offscreen.view;
    let mut encoder = util
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Encoder"),
        });

    {
        // RenderPass の作成
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    // 背景色（少し暗いグレー）
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        // 描画コマンドの記録
        if !batches.is_empty() {
            util.resources
                .record_render_commands(&mut rpass, &batches, &util.device);
        }
    }

    // 5. GPUへ送信
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &offscreen.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &offscreen.buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(offscreen.padded_bytes_per_row),
                rows_per_image: Some(offscreen.height),
            },
        },
        wgpu::Extent3d {
            width: offscreen.width,
            height: offscreen.height,
            depth_or_array_layers: 1,
        },
    );

    util.queue.submit(std::iter::once(encoder.finish()));
    Ok(())
}
