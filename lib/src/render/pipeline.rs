use crate::render::RenderBatch;
use glam::Mat4;
use wgpu::util::DeviceExt;

use super::{uniform::ScreenUniform, vertex::Vertex};

pub struct WgpuRenderResources {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_capacity: u64,
    pub transform_capacity: u64,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub transform_bind_group_layout: wgpu::BindGroupLayout,
    pub transform_buffer: wgpu::Buffer,
    pub transform_bind_group: wgpu::BindGroup,
    pub dummy_bind_group: wgpu::BindGroup,
}

impl WgpuRenderResources {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        // Uniform buffer
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("screen_uniform"),
            contents: bytemuck::bytes_of(&ScreenUniform::new(
                [1280.0, 720.0], // 初期画面サイズ
                // 初期ビュープロジェクション行列（単位行列）
                // これは `render` メソッドで毎フレーム更新されます
                glam::Mat4::IDENTITY,
            )),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("screen_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("screen_bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // テクスチャ用の Bind group layout
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // 行列用バッファ（例：最大256個のバッチ分を確保）
        // Uniformのオフセットは256バイトアライメントが必要な場合が多い
        let transform_capacity = 256;
        let transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("transform_buffer"),
            size: transform_capacity * 256, // アライメント(256) * スロット数
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 行列 (Locals) 用の Bind group layout
        let transform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("transform_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true, // ここを true にするのがポイント
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("transform_bg"),
            layout: &transform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &transform_buffer,
                    offset: 0,
                    size: std::num::NonZeroU64::new(64), // mat4x4(64bytes)のみをバインド
                }),
            }],
        });

        // 頂点バッファ（最大1000矩形 = 6000頂点）
        let vertex_capacity = (std::mem::size_of::<Vertex>() * 6000) as u64;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vertex_buffer"),
            size: vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shape"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shape.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shape_layout"),
            bind_group_layouts: &[
                Some(&bind_group_layout),
                Some(&texture_bind_group_layout),
                Some(&transform_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shape"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // 1x1 の白いテクスチャを作成してデフォルトの BindGroup とする
        let dummy_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("dummy_texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 白いピクセルを書き込む
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &dummy_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            dummy_texture.size(),
        );

        let dummy_view = dummy_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        });

        let dummy_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let dummy_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dummy_bg"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&dummy_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&dummy_sampler),
                },
            ],
        });

        Self {
            pipeline,
            vertex_buffer,
            vertex_capacity,
            transform_capacity,
            uniform_buffer,
            bind_group,
            texture_bind_group_layout,
            transform_bind_group_layout,
            transform_buffer,
            transform_bind_group,
            dummy_bind_group,
        }
    }

    /// バッファの内容を更新する
    pub fn update_buffers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_size: [f32; 2],
        view_projection_matrix: Mat4,
        batches: &[RenderBatch],
    ) {
        let total_vertex_count: usize = batches.iter().map(|b| b.vertices.len()).sum();
        self.check_capacity(device, batches.len(), total_vertex_count);

        // Uniform更新
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&ScreenUniform::new(screen_size, view_projection_matrix)),
        );

        // 頂点バッファ更新
        let mut offset = 0;
        for batch in batches {
            let byte_offset = (offset * std::mem::size_of::<Vertex>()) as u64;
            queue.write_buffer(
                &self.vertex_buffer,
                byte_offset,
                bytemuck::cast_slice(batch.vertices.as_slice()),
            );
            offset += batch.vertices.len();
        }

        // 全バッチの行列データを一つのベクタにまとめる
        // Uniformバッファのオフセット制限（通常256バイト単位）に合わせる必要がある
        let alignment = device.limits().min_uniform_buffer_offset_alignment as usize;
        let mut transform_data = Vec::new();
        for batch in batches {
            let start = transform_data.len();
            transform_data
                .extend_from_slice(bytemuck::cast_slice(&batch.transform.to_cols_array()));
            // 次のアライメント位置までパディング
            let padding = (alignment - (transform_data.len() - start) % alignment) % alignment;
            transform_data.extend(std::iter::repeat(0u8).take(padding));
        }
        queue.write_buffer(&self.transform_buffer, 0, &transform_data);
    }

    pub fn record_render_commands(
        &self,
        rpass: &mut wgpu::RenderPass,
        batches: &[RenderBatch],
        device: &wgpu::Device,
    ) {
        let alignment = device.limits().min_uniform_buffer_offset_alignment as usize;

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        // バッチごとに描画
        let mut current_vertex = 0;
        for (i, batch) in batches.iter().enumerate() {
            let count = batch.vertices.len() as u32;
            let dynamic_offset = (i * alignment) as u32;

            rpass.set_bind_group(1, &batch.texture_bind_group, &[]); // テクスチャ
            // 固定の BindGroup を使いつつ、オフセットだけ変えて描画
            rpass.set_bind_group(2, &self.transform_bind_group, &[dynamic_offset]);

            rpass.draw(current_vertex..(current_vertex + count), 0..1);
            current_vertex += count;
        }
    }

    fn check_capacity(
        &mut self,
        device: &wgpu::Device,
        num_batches: usize,
        total_vertex_count: usize,
    ) {
        let needed_size = (total_vertex_count * std::mem::size_of::<Vertex>()) as u64;

        // 容量が足りない場合はバッファを拡張（再作成）
        if needed_size > self.vertex_capacity {
            self.vertex_capacity = needed_size.next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vertex_buffer_resized"),
                size: self.vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        // 行列バッファの容量チェックとリサイズ
        let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let needed_transform_count = num_batches as u64;
        if needed_transform_count > self.transform_capacity || self.transform_capacity == 0 {
            // Added check for initial 0 capacity
            self.transform_capacity = needed_transform_count.next_power_of_two();
            self.transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("transform_buffer_resized"),
                size: self.transform_capacity * alignment,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            // バッファが変わったので BindGroup も作り直す必要がある
            self.transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("transform_bg_resized"),
                layout: &self.transform_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.transform_buffer,
                        offset: 0,
                        size: std::num::NonZeroU64::new(64),
                    }),
                }],
            });
        }
    }
}
