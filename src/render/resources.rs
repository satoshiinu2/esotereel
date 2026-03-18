// src/render/resources.rs
use egui_wgpu::wgpu;

pub struct WgpuRenderResources {
    pub pipeline: wgpu::RenderPipeline,
    pub offscreen_texture: wgpu::Texture,
    pub offscreen_view: wgpu::TextureView,
    pub output_buffer: wgpu::Buffer, // GPU→CPU転送用
    pub width: u32,
    pub height: u32,
}

impl WgpuRenderResources {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("triangle"),
            source: wgpu::ShaderSource::Wgsl(include_str!("triangle.wgsl").into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            // 既存のまま
            label: Some("triangle"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // オフスクリーンテクスチャ
        let offscreen_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            // RENDER_ATTACHMENT: 描画先として使う
            // COPY_SRC: GPU→CPU転送のためにコピー元として使う
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let offscreen_view = offscreen_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // GPU→CPU転送用バッファ
        // alignmentに注意：1行のバイト数は256の倍数である必要がある
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let bytes_per_row = (width * 4 + align - 1) / align * align;

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output"),
            size: (bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            offscreen_texture,
            offscreen_view,
            output_buffer,
            width,
            height,
        }
    }

    // オフスクリーンに描画してVec<u8>で返す
    pub fn render_to_buffer(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        // オフスクリーンに描画
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.offscreen_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            rpass.set_pipeline(&self.pipeline);
            rpass.draw(0..3, 0..1);
        }

        // テクスチャ→バッファにコピー
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let bytes_per_row = (self.width * 4 + align - 1) / align * align;

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.offscreen_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(Some(encoder.finish()));

        // バッファをCPUにマップして読み取る
        let buffer_slice = self.output_buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::Maintain::Wait);

        let data = buffer_slice.get_mapped_range();

        // alignmentパディングを除去して正しいRGBAデータを作る
        let mut pixels = Vec::with_capacity((self.width * self.height * 4) as usize);
        for row in 0..self.height as usize {
            let start = row * bytes_per_row as usize;
            let end = start + self.width as usize * 4;
            pixels.extend_from_slice(&data[start..end]);
        }
        drop(data);
        self.output_buffer.unmap();

        pixels
    }
}
