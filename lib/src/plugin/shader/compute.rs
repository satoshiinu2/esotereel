//! compute シェーダのスタック実行ランナーです。
//!
//! 頂点データを storage buffer に置き、シェーダ 1 つ = compute パス 1 回として
//! バッファを ping-pong しながら順に適用します。各パスの入力は直前のパスの
//! 出力ですが、呼び出し側が常に元の頂点列を渡すため、フレームをまたいだ
//! 変形の累積は起きません。
//!
//! バインドレイアウトはフェーズ 1 の契約どおり、@group(0) が Params uniform、
//! @group(1) の binding 0 / 1 が入力（read）/ 出力（read_write）の頂点バッファです。
//! レンダーループからは `render::deform` 経由で使われます。

use std::collections::HashMap;

use anyhow::{bail, Context};
use wgpu::util::DeviceExt;

use crate::{
    plugin::{
        shader::{ShaderDefinition, ShaderKind},
        NamespacedID,
    },
    render::vertex::Vertex,
};

/// 頂点 1 つ分のバイト数です（`Vertex` は 9 × f32 = 36 バイト）。
const VERTEX_STRIDE: u64 = std::mem::size_of::<Vertex>() as u64;

/// 1 回の compute パス分の入力です。
pub struct ComputePass<'a> {
    /// 適用する compute シェーダ定義。
    pub definition: &'a ShaderDefinition,
    /// `pack_params` で生成した Params uniform のバイト列。
    pub params: Vec<u8>,
}

struct CachedPipeline {
    pipeline: wgpu::ComputePipeline,
    params_layout: wgpu::BindGroupLayout,
    vertices_layout: wgpu::BindGroupLayout,
    workgroup_size: u32,
}

/// compute シェーダのスタックを実行するランナーです。
///
/// パイプラインはシェーダ定義ごとに生成してキャッシュします。
#[derive(Default)]
pub struct ComputeStackRunner {
    pipelines: HashMap<NamespacedID, CachedPipeline>,
}

impl ComputeStackRunner {
    /// 空のランナーを作成します。
    pub fn new() -> Self {
        Self::default()
    }

    /// 頂点列に compute シェーダのスタックを順に適用し、最終頂点バッファを返します。
    ///
    /// `input` は `Vertex` 列のバイト表現です。返るバッファは入力と同じサイズで、
    /// `VERTEX` 用途を含みます。`passes` が空の場合は入力のコピーを返します。
    pub fn run(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input: &[u8],
        passes: &[ComputePass<'_>],
    ) -> anyhow::Result<wgpu::Buffer> {
        if input.is_empty() || input.len() as u64 % VERTEX_STRIDE != 0 {
            bail!(
                "input must be a non-empty sequence of vertices ({VERTEX_STRIDE} bytes each), got {} bytes",
                input.len()
            );
        }
        for pass in passes {
            if !matches!(pass.definition.kind, ShaderKind::Compute { .. }) {
                bail!(
                    "shader `{}` is not a compute shader",
                    pass.definition.id.full()
                );
            }
        }

        let vertex_count = (input.len() as u64 / VERTEX_STRIDE) as u32;
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        let buffers: [wgpu::Buffer; 2] = std::array::from_fn(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("compute stack vertices"),
                size: input.len() as u64,
                usage,
                mapped_at_creation: false,
            })
        });
        queue.write_buffer(&buffers[0], 0, input);
        if passes.is_empty() {
            return Ok(buffers[0].clone());
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("compute stack"),
        });
        for (i, pass) in passes.iter().enumerate() {
            let cached = self.pipeline_for(device, pass.definition)?;
            // プロパティの無いシェーダでも group(0) のバインドは必要で、
            // サイズ 0 のバッファは作れないため最小サイズで代替する。
            let params_data: &[u8] = if pass.params.is_empty() {
                &[0u8; 4]
            } else {
                &pass.params
            };
            let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("compute stack params"),
                contents: params_data,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compute stack params bind group"),
                layout: &cached.params_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                }],
            });
            let vertices_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("compute stack vertices bind group"),
                layout: &cached.vertices_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffers[i % 2].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buffers[(i + 1) % 2].as_entire_binding(),
                    },
                ],
            });

            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute stack pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&cached.pipeline);
            cpass.set_bind_group(0, &params_bind_group, &[]);
            cpass.set_bind_group(1, &vertices_bind_group, &[]);
            cpass.dispatch_workgroups(vertex_count.div_ceil(cached.workgroup_size), 1, 1);
            drop(cpass);
        }
        queue.submit([encoder.finish()]);

        Ok(buffers[passes.len() % 2].clone())
    }

    fn pipeline_for(
        &mut self,
        device: &wgpu::Device,
        definition: &ShaderDefinition,
    ) -> anyhow::Result<&CachedPipeline> {
        if !self.pipelines.contains_key(&definition.id) {
            let cached = Self::create_pipeline(device, definition)?;
            self.pipelines.insert(definition.id.clone(), cached);
        }
        Ok(self.pipelines.get(&definition.id).unwrap())
    }

    fn create_pipeline(
        device: &wgpu::Device,
        definition: &ShaderDefinition,
    ) -> anyhow::Result<CachedPipeline> {
        let ShaderKind::Compute { entry_compute } = &definition.kind else {
            bail!("shader `{}` is not a compute shader", definition.id.full());
        };

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(definition.id.full()),
            source: wgpu::ShaderSource::Wgsl(definition.compiled_wgsl.as_str().into()),
        });
        let buffer_entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compute stack params layout"),
            entries: &[buffer_entry(0, wgpu::BufferBindingType::Uniform)],
        });
        let vertices_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compute stack vertices layout"),
            entries: &[
                buffer_entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
                buffer_entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compute stack pipeline layout"),
            bind_group_layouts: &[Some(&params_layout), Some(&vertices_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(definition.id.full()),
            layout: Some(&layout),
            module: &module,
            entry_point: Some(entry_compute.as_str()),
            compilation_options: Default::default(),
            cache: None,
        });
        let workgroup_size = Self::workgroup_size(definition, entry_compute)?;

        Ok(CachedPipeline {
            pipeline,
            params_layout,
            vertices_layout,
            workgroup_size,
        })
    }

    /// コンパイル済み WGSL を naga で読み、compute エントリの workgroup サイズ（x）を返します。
    fn workgroup_size(definition: &ShaderDefinition, entry: &str) -> anyhow::Result<u32> {
        let module = naga::front::wgsl::parse_str(&definition.compiled_wgsl).map_err(|e| {
            anyhow::anyhow!(
                "naga parse failed for `{}`:\n{}",
                definition.id.full(),
                e.emit_to_string(&definition.compiled_wgsl)
            )
        })?;
        let entry_point = module
            .entry_points
            .iter()
            .find(|ep| ep.name == entry && ep.stage == naga::ShaderStage::Compute)
            .with_context(|| {
                format!(
                    "compute entry point `{entry}` not found in shader `{}`",
                    definition.id.full()
                )
            })?;
        Ok(entry_point.workgroup_size[0].max(1))
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::Path};

    use super::*;
    use crate::plugin::{
        shader::{pack_params, CoreParamValue},
        PluginLoader,
    };

    /// storage buffer が使える制限でデバイスを作成します。
    /// `WGpuUtil` の `downlevel_webgl2_defaults` は storage buffer を許可しないため、
    /// このテストでは専用のデバイスを用意します。
    fn create_device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .expect("Could not get an adapter (GPU).");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::defaults(),
            memory_hints: Default::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
        }))
        .expect("Failed to create device")
    }

    fn read_vertices(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer: &wgpu::Buffer,
        count: usize,
    ) -> Vec<Vertex> {
        let size = (count * std::mem::size_of::<Vertex>()) as u64;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compute stack readback staging"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("compute stack readback"),
        });
        encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, size);
        queue.submit([encoder.finish()]);

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        rx.recv().unwrap().unwrap();

        let data = slice.get_mapped_range();
        let vertices = bytemuck::cast_slice::<u8, Vertex>(&data).to_vec();
        drop(data);
        staging.unmap();
        vertices
    }

    #[test]
    #[ignore = "requires a wgpu adapter; run explicitly to verify compute deform dispatch"]
    fn wave_compute_deforms_vertices() {
        let mut loader = PluginLoader::new();
        loader
            .load_additional_plugin(Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../plugins/std"
            )))
            .unwrap();
        let id = NamespacedID::new("std", "wave_compute").unwrap();
        let definition = loader.get_shader(&id).unwrap();

        let (device, queue) = create_device();
        let xs = [0.0f32, 10.0, 20.0, 100.0];
        let input: Vec<Vertex> = xs
            .iter()
            .map(|&x| Vertex {
                position: [x, 0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                tex_coords: [0.0, 0.0],
            })
            .collect();
        let input_bytes = bytemuck::cast_slice(&input);

        // core:time = 0、固有プロパティは default（amplitude 10, frequency 0.05, speed 2）を使う。
        // y' = y + sin(x * frequency + time * speed) * amplitude = sin(x * 0.05) * 10。
        let core = HashMap::from([("core:time".to_string(), CoreParamValue::Float32(0.0))]);
        let params = pack_params(&definition, &core, &HashMap::new()).unwrap();
        let expected_y = |x: f32| (x * 0.05).sin() * 10.0;

        let mut runner = ComputeStackRunner::new();

        let out = runner
            .run(
                &device,
                &queue,
                input_bytes,
                &[ComputePass {
                    definition: &definition,
                    params: params.clone(),
                }],
            )
            .unwrap();
        let result = read_vertices(&device, &queue, &out, input.len());
        for (vertex, &x) in result.iter().zip(&xs) {
            assert_eq!(vertex.position[0], x);
            assert!(
                (vertex.position[1] - expected_y(x)).abs() < 1e-4,
                "x={x}: y {} != expected {}",
                vertex.position[1],
                expected_y(x)
            );
            assert_eq!(vertex.color, [1.0, 1.0, 1.0, 1.0]);
        }

        // 2 段重ねると 2 段目の入力は 1 段目の出力になり、x は変わらないため
        // 変形量が 2 倍になる（スタックの ping-pong がつながっていることの確認）。
        let out = runner
            .run(
                &device,
                &queue,
                input_bytes,
                &[
                    ComputePass {
                        definition: &definition,
                        params: params.clone(),
                    },
                    ComputePass {
                        definition: &definition,
                        params,
                    },
                ],
            )
            .unwrap();
        let result = read_vertices(&device, &queue, &out, input.len());
        for (vertex, &x) in result.iter().zip(&xs) {
            assert!(
                (vertex.position[1] - expected_y(x) * 2.0).abs() < 1e-4,
                "x={x}: y {} != expected {}",
                vertex.position[1],
                expected_y(x) * 2.0
            );
        }
    }
}
