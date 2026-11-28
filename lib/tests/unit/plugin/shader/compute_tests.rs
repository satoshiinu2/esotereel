use std::{collections::HashMap, path::Path};

use super::*;
use crate::plugin::{
    PluginLoader,
    shader::{CoreParamValue, pack_params},
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

#[test]
#[ignore = "requires a wgpu adapter; run explicitly to verify compute deform dispatch"]
fn corner_compute_deforms_vertices() {
    use crate::render::corner::{CornerOffsets, corner_deform_shader_id};

    let mut loader = PluginLoader::new();
    loader
        .load_additional_plugin(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plugins/std"
        )))
        .unwrap();
    let id = corner_deform_shader_id().unwrap();
    let definition = loader.get_shader(&id).unwrap();

    let (device, queue) = create_device();

    // 単位クワッドの4隅＋中心。tex_coords が正規化座標 (u, v) を兼ねる。
    let mut input = Vertex::rect(0.0, 0.0, 1.0, 1.0, [1.0, 1.0, 1.0, 1.0]).to_vec();
    input.push(Vertex {
        position: [0.5, 0.5, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        tex_coords: [0.5, 0.5],
    });
    let input_bytes = bytemuck::cast_slice(&input);

    let offsets = CornerOffsets {
        top_left: [0.5, 0.0],
        top_right: [0.0, 0.5],
        bottom_right: [-0.5, 0.0],
        bottom_left: [0.0, -0.5],
    };
    let entry = offsets.stack_entry(id);
    let params = pack_params(&definition, &HashMap::new(), &entry.properties).unwrap();

    let mut runner = ComputeStackRunner::new();
    let out = runner
        .run(
            &device,
            &queue,
            input_bytes,
            &[ComputePass {
                definition: &definition,
                params,
            }],
        )
        .unwrap();
    let result = read_vertices(&device, &queue, &out, input.len());
    // rect の頂点順は (0,0), (1,0), (0,1), (1,0), (1,1), (0,1)
    assert_eq!(result[0].position, [0.5, 0.0, 0.0]);
    assert_eq!(result[1].position, [1.0, 0.5, 0.0]);
    assert_eq!(result[2].position, [0.0, 0.5, 0.0]);
    assert_eq!(result[4].position, [0.5, 1.0, 0.0]);
    // 中心は四隅オフセットの平均（この組み合わせでは 0）だけ移動する
    assert_eq!(result[6].position, [0.5, 0.5, 0.0]);
}

