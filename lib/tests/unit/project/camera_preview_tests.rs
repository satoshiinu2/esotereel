use super::*;
use crate::{
    StreamState,
    decode::streamplayer::StreamPlayer,
    plugin::{NamespacedID, PluginLoader, property::value::FieldValue},
    project::{
        Project, Timeline,
        clip::{ClipBindingValue, ShaderStackEntry},
        ids::IdGenerator,
    },
    render::{
        RenderContext, render_frame_offscreen,
        video::{MediaFetchCache, builder::build_vertices},
        wgpuutil::{OffscreenTarget, WGpuUtil},
    },
};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

fn with_preview(test: impl FnOnce(&RenderContext)) {
    let mut loader = PluginLoader::new();
    loader
        .load_additional_plugin(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plugins/std"
        )))
        .unwrap();
    let kind = NamespacedID::new("std", "video").unwrap();
    let mut properties =
        ClipBindingValue::default_properties(&loader.get_clip_properties(&kind).unwrap());
    // コンポーネントが提供するプロパティは所有者付きのキーで配置される:
    // std:media#media, std:transform#transform など
    let media_value = properties
        .get_mut(
            &crate::plugin::property::PropertyKey::new(
                NamespacedID::new("std", "media").unwrap(),
                "media",
            )
            .unwrap(),
        )
        .unwrap();
    let ClipBindingValue::Static(FieldValue::Map(media)) = media_value else {
        panic!("media property must be a map")
    };
    // source は AssetRef 型の値で持ち、描画側はプロジェクトのレジストリ経由で
    // 実パス（ここではストリームキャッシュのキー "fixture.mp4"）へ解決する。
    let fixture_ref = uuid::Uuid::from_u128(1);
    media.insert("source".into(), FieldValue::AssetRef(fixture_ref));
    let transform_value = properties
        .get_mut(
            &crate::plugin::property::PropertyKey::new(
                NamespacedID::new("std", "transform").unwrap(),
                "transform",
            )
            .unwrap(),
        )
        .unwrap();
    let ClipBindingValue::Static(FieldValue::Map(transform)) = transform_value else {
        panic!("transform property must be a map")
    };
    transform.insert(
        "scale".into(),
        FieldValue::Array(vec![
            FieldValue::Float(32.0),
            FieldValue::Float(24.0),
            FieldValue::Float(1.0),
        ]),
    );
    let mut ids = IdGenerator::default();
    let mut timeline = Timeline::new_empty(1, 60.0);
    let layer = timeline
        .insert_layer(&mut ids, "Video".into(), None, None)
        .unwrap();
    timeline
        .new_clip_in(layer, &mut ids, 0, 60, kind, properties)
        .unwrap();
    let mut frame =
        ffmpeg_next::util::frame::video::Video::new(ffmpeg_next::format::Pixel::RGBA, 32, 24);
    let stride = frame.stride(0);
    for y in 0..24 {
        for x in 0..32 {
            let offset = y * stride + x * 4;
            let rgba = if y < 12 {
                [255, 0, 0, 255]
            } else {
                [0, 255, 0, 255]
            };
            frame.data_mut(0)[offset..offset + 4].copy_from_slice(&rgba);
        }
    }
    let mut player =
        StreamPlayer::new_from_metadata(ffmpeg_next::codec::Id::H264, 32, 24, 1.0 / 30.0, &[])
            .unwrap();
    player
        .frames
        .insert(ordered_float::OrderedFloat(0.0), frame);
    let streams = Arc::new(DashMap::new());
    streams.insert(1, player);
    let paths = Arc::new(DashMap::new());
    paths.insert("fixture.mp4".into(), StreamState::Loaded(1));
    let cache = Arc::new(MediaFetchCache::default());
    let loader = Arc::new(RwLock::new(loader));
    let camera = CameraInfo {
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        is_orthographic: true,
        orthographic_direction: Direction::Front,
        scale_factor: 1.0,
        fov: 60.0,
    };
    let mut project_inner = Project::new();
    project_inner
        .register_external_asset(fixture_ref, std::path::Path::new("fixture.mp4"))
        .unwrap();
    let project = Arc::new(RwLock::new(Some(project_inner)));
    test(&RenderContext {
        path_to_stream: &paths,
        streams: &streams,
        media_fetch_cache: &cache,
        plugin_loader: &loader,
        timeline: &timeline,
        camera_info: &camera,
        current_frame: 0,
        project: &project,
    });
}

#[test]
fn video_properties_override_legacy_size_and_project_into_view() {
    with_preview(|ctx| {
        let batches = build_vertices(ctx).batches;
        assert_eq!(batches.len(), 1);
        let model = batches[0].transform;
        assert_eq!(
            model.transform_point3(Vec3::ZERO),
            Vec3::new(-16.0, -12.0, 0.0)
        );
        assert_eq!(
            model.transform_point3(Vec3::new(1.0, 1.0, 0.0)),
            Vec3::new(16.0, 12.0, 0.0)
        );
        let mvp =
            ctx.camera_info.get_proj_mat([64.0, 64.0]) * ctx.camera_info.get_view_mat() * model;
        for vertex in &batches[0].vertices {
            let projected = mvp.project_point3(Vec3::from_array(vertex.position));
            assert!(projected.x.abs() <= 1.0 && projected.y.abs() <= 1.0);
            assert!((0.0..=1.0).contains(&projected.z));
        }
    });
}

#[test]
#[ignore = "requires a wgpu adapter; run explicitly to verify texture upload and readback"]
fn video_preview_renders_colored_pixels() {
    with_preview(|ctx| {
        let mut gpu = WGpuUtil::new(64, 64);
        let target = OffscreenTarget::new(&gpu.device, gpu.format, 64, 64);
        render_frame_offscreen(&mut gpu, &target, ctx).unwrap();
        let pixels = target.readback(&gpu.device).unwrap();
        let pixel = |x: usize, y: usize| &pixels[(y * 64 + x) * 4..(y * 64 + x + 1) * 4];
        assert_eq!(pixel(32, 24), [255, 0, 0, 255]);
        assert_eq!(pixel(32, 39), [0, 255, 0, 255]);
        assert_eq!(pixel(0, 0), [26, 26, 26, 255]);
    });
}
#[test]
#[ignore = "requires a wgpu adapter; run explicitly to verify compute deform reaches the render loop"]
fn video_preview_with_shader_stack_deforms_pixels() {
    with_preview(|ctx| {
        let mut gpu = WGpuUtil::new(64, 64);
        let target = OffscreenTarget::new(&gpu.device, gpu.format, 64, 64);

        // スタック無し（従来どおり）の描画を基準にする。
        render_frame_offscreen(&mut gpu, &target, ctx).unwrap();
        let baseline = target.readback(&gpu.device).unwrap();

        // 同じクリップに std:wave_compute のスタックを付けてもう一度描く。
        let mut timeline = ctx.timeline.clone();
        let clip = timeline.get_clip_mut(0).unwrap();
        clip.set_shader_stack(vec![ShaderStackEntry {
            shader_id: NamespacedID::new("std", "wave_compute").unwrap(),
            properties: HashMap::new(),
        }]);
        let context = RenderContext {
            timeline: &timeline,
            ..*ctx
        };
        render_frame_offscreen(&mut gpu, &target, &context).unwrap();
        let deformed = target.readback(&gpu.device).unwrap();

        // 変形が描画結果へ反映され、ピクセルが有意に変わることを確認する。
        let differing = baseline
            .chunks_exact(4)
            .zip(deformed.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            differing > 100,
            "expected the wave deform to change many pixels, only {differing} differed"
        );
    });
}

#[test]
#[ignore = "requires ESOTEREEL_TEST_VIDEO and a wgpu adapter"]
fn video_file_preview_matches_decoded_frame() {
    let path =
        std::env::var("ESOTEREEL_TEST_VIDEO").expect("set ESOTEREEL_TEST_VIDEO to a video file");
    with_preview(|ctx| {
        let mut source = crate::decode::videostreamer::VideoStreamer::new(&path).unwrap();
        let mut player = StreamPlayer::new_from_metadata(
            source.codec_id(),
            source.width(),
            source.height(),
            source.time_base,
            source.extradata().unwrap_or(&[]),
        )
        .unwrap();
        for response in source
            .fetch_stream_data_batch(1, vec![5.0..6.0], 1)
            .unwrap()
        {
            if let crate::responces::Response::StreamData {
                data,
                pts,
                dts,
                is_key,
                discontinuous,
                generation,
                ..
            } = response
            {
                player
                    .process_packet(&data, pts, dts, is_key, discontinuous, generation)
                    .unwrap();
            }
        }
        let decoded = player
            .get_frame_at(5.0)
            .expect("a decoded frame near 5 seconds")
            .clone();
        ctx.streams.insert(1, player);
        let mut timeline = ctx.timeline.clone();
        let clip = timeline.get_clip_mut(0).unwrap();
        clip.duration = 600;
        clip.set_property_path(
            &crate::plugin::property::PropertyPath::parse("std:transform#transform/scale").unwrap(),
            ClipBindingValue::Static(FieldValue::Array(
                [
                    32.0 / decoded.width() as f64,
                    24.0 / decoded.height() as f64,
                    1.0,
                ]
                .into_iter()
                .map(FieldValue::Float)
                .collect(),
            )),
        )
        .unwrap();
        let context = RenderContext {
            timeline: &timeline,
            current_frame: 300,
            ..*ctx
        };
        let mut gpu = WGpuUtil::new(64, 64);
        let target = OffscreenTarget::new(&gpu.device, gpu.format, 64, 64);
        render_frame_offscreen(&mut gpu, &target, &context).unwrap();
        let pixels = target.readback(&gpu.device).unwrap();
        // Compare actual readback to bilinear sampling of the decoded RGBA frame.
        for (x, y) in [(24, 24), (32, 32), (40, 39)] {
            let sx = (x as f64 + 0.5 - 16.0) / 32.0 * decoded.width() as f64 - 0.5;
            let sy = (y as f64 + 0.5 - 20.0) / 24.0 * decoded.height() as f64 - 0.5;
            let (ix, iy) = (sx.floor() as usize, sy.floor() as usize);
            let (fx, fy) = (sx.fract(), sy.fract());
            for channel in 0..4 {
                let sample = |dx: usize, dy: usize| {
                    decoded.data(0)[(iy + dy) * decoded.stride(0) + (ix + dx) * 4 + channel] as f64
                };
                let expected = (sample(0, 0) * (1.0 - fx) + sample(1, 0) * fx) * (1.0 - fy)
                    + (sample(0, 1) * (1.0 - fx) + sample(1, 1) * fx) * fy;
                let actual = pixels[(y * 64 + x) * 4 + channel] as f64;
                assert!(
                    (actual - expected).abs() <= 2.0,
                    "pixel ({x}, {y}) channel {channel}: {actual} != {expected}"
                );
            }
        }
    });
}
