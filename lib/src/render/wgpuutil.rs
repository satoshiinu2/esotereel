use std::collections::HashMap;

use wgpu::ExperimentalFeatures;

use crate::render::pipeline::WgpuRenderResources;

/// wgpu のデバイス・キュー・リソースをまとめたユーティリティ構造体です。
///
/// オフスクリーンレンダリング専用で、ウィンドウサーフェスは持たず、
/// オフスクリーンテクスチャへのレンダリングと GPU 読み戻しを行います。
pub struct WGpuUtil {
    pub instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
    pub config_width: u32,
    pub config_height: u32,
    pub resources: WgpuRenderResources,
    pub textures: HashMap<u32, (wgpu::Texture, wgpu::BindGroup)>,
}

impl WGpuUtil {
    /// 新しい `WGpuUtil` を作成します。
    ///
    /// ハイパフォーマンス電源設定のアダプターを選択し、デバイスとキュー、
    /// レンダーリソースを初期化します。
    pub fn new(width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::default();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None, // Offscreen専用なのでウィンドウ互換性は不要
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .expect("Could not get an adapter (GPU).");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            memory_hints: Default::default(),
            experimental_features: ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
        }))
        .expect("Failed to create device");

        let format = wgpu::TextureFormat::Rgba8Unorm;
        let resources = WgpuRenderResources::new(&device, &queue, format);

        Self {
            instance,
            device,
            queue,
            format,
            config_width: width,
            config_height: height,
            resources,
            textures: HashMap::new(),
        }
    }
}

/// オフスクリーンレンダリングターゲットです。
///
/// テクスチャ、ビュー、読み戻し用バッファを保持し、GPU 上で描画した結果を
/// CPU 側の紧密パックされた RGBA バイト列として取得できます。
pub struct OffscreenTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub buffer: wgpu::Buffer,
    pub width: u32,
    pub height: u32,
    pub padded_bytes_per_row: u32,
    pub unpadded_bytes_per_row: u32,
}

const BYTES_PER_PIXEL: u32 = 4; // RGBA8

impl OffscreenTarget {
    /// 新しい `OffscreenTarget` を作成します。
    ///
    /// レンダーアタッチメントおよびコピー元として使用可能なテクスチャと、
    /// CPU 読み戻し用のマップ可能バッファを作成します。
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen render target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // wgpuはcopy_texture_to_bufferで各行を256バイト境界に揃える必要がある
        let unpadded_bytes_per_row = width * BYTES_PER_PIXEL;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            texture,
            view,
            buffer,
            width,
            height,
            padded_bytes_per_row,
            unpadded_bytes_per_row,
        }
    }

    /// コピー済みのバッファを CPU 側に読み戻し、パディングを除去した
    /// 紧密パックされた RGBA バイト列を返します。
    ///
    /// 各行が `COPY_BYTES_PER_ROW_ALIGNMENT`（256 バイト）境界に揃えられているため、
    /// 不要なパディング部分を取り除いて返します。
    pub fn readback(&self, device: &wgpu::Device) -> Result<Vec<u8>, String> {
        let slice = self.buffer.slice(..);

        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });

        // GPU側の完了を待つ(copy_texture_to_bufferのコマンドがsubmit済みである前提)
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None, // 直近のsubmissionを待つ
                timeout: None,          // 無期限に待つ(エラー検知するまで)
            })
            .map_err(|e| format!("device poll failed: {e:?}"))?;

        rx.recv()
            .map_err(|e| format!("map_async channel recv failed: {e}"))?
            .map_err(|e| format!("buffer map failed: {e:?}"))?;

        let data = slice.get_mapped_range();

        // 各行がpadded_bytes_per_rowでアラインされているので、
        // unpadded_bytes_per_row分だけ取り出して詰め直す
        let mut out = Vec::with_capacity((self.unpadded_bytes_per_row * self.height) as usize);
        for row in 0..self.height {
            let start = (row * self.padded_bytes_per_row) as usize;
            let end = start + self.unpadded_bytes_per_row as usize;
            out.extend_from_slice(&data[start..end]);
        }

        drop(data);
        self.buffer.unmap();

        Ok(out)
    }
}
