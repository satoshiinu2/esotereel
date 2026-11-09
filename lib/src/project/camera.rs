use glam::{Mat4, Vec3, camera::lh::proj::directx};

/// カメラ情報です。
///
/// 視点・回転・投影種別（正射影/透視投影）を持ち、ビュー行列と射影行列を
/// 生成します。レンダリング時にカメラ変換を適用するために使用されます。
#[repr(C)]
#[derive(Debug)]
pub struct CameraInfo {
    /// カメラ位置
    position: Vec3,
    /// 回転（度）
    rotation: Vec3,
    /// 正射影モードかどうか
    is_orthographic: bool,
    /// 正射影方向
    orthographic_direction: Direction,
    /// スケール係数
    scale_factor: f32,
    /// 視野角（度）
    fov: f32,
}

impl CameraInfo {
    /// ビュー行列を返します。
    ///
    /// 正射影モードの場合は方向に合わせた固定回転を適用し、
    /// 3Dモードの場合はオイラー角から回転を生成します。
    /// 左手系への変換と平行移動を適用した行列を返します。
    pub fn get_view_mat(&self) -> Mat4 {
        let rot_mat = if self.is_orthographic {
            // 2Dモード: 方向に合わせた固定回転を適用
            self.orthographic_direction.get_rotation_mat()
        } else {
            // 3Dモード: オイラー角から回転を生成
            Mat4::from_euler(
                glam::EulerRot::XYZ,
                self.rotation.x.to_radians(),
                self.rotation.y.to_radians(),
                self.rotation.z.to_radians(),
            )
        };

        // 左手系への変換と平行移動を適用
        let trans_mat =
            Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0)) * Mat4::from_translation(-self.position);

        rot_mat * trans_mat
    }

    /// 射影行列を返します。
    ///
    /// 正射影モードの場合は 2D 用の平行投影行列（ズームレベルを考慮）を、
    /// 3Dモードの場合は透視投影行列を返します。
    pub fn get_proj_mat(&self, screen_size: [f32; 2]) -> Mat4 {
        let half_w = screen_size[0] / 2.0;
        let half_h = screen_size[1] / 2.0;
        let aspect = screen_size[0] / screen_size[1];
        let fov = self.fov.to_radians();

        if self.is_orthographic {
            // 2D用の平行投影行列（ズームレベルを考慮）
            let ortho_mat =
                directx::orthographic(-half_w, half_w, half_h, -half_h, -1000.0, 1000.0);
            let scale_mat = Mat4::from_scale(Vec3::new(self.scale_factor, self.scale_factor, 1.0));

            scale_mat * ortho_mat
        } else {
            // 3D用の透視投影行列
            directx::perspective(fov, aspect, 0.1, 1000.0)
        }
    }
}

/// カメラの向きです。
///
/// 正射影モードで使用される固定方向を表現します。
#[derive(Clone, Copy, Debug)]
#[repr(C)]

pub enum Direction {
    /// 前方
    Front,
    /// 後方
    Back,
    /// 上方
    Top,
    /// 下方
    Bottom,
    /// 左方
    Left,
    /// 右方
    Right,
}

impl Direction {
    /// 回転行列を返します。
    ///
    /// 各方向に対応する固定回転行列を返します。
    pub fn get_rotation_mat(&self) -> Mat4 {
        match self {
            Direction::Front => Mat4::IDENTITY,
            Direction::Back => Mat4::from_rotation_y(180.0_f32.to_radians()),
            Direction::Top => Mat4::from_rotation_x(90.0_f32.to_radians()),
            Direction::Bottom => Mat4::from_rotation_x(-90.0_f32.to_radians()),
            Direction::Left => Mat4::from_rotation_y(90.0_f32.to_radians()),
            Direction::Right => Mat4::from_rotation_y(-90.0_f32.to_radians()),
        }
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    use crate::{
        StreamState,
        decode::streamplayer::StreamPlayer,
        plugin::{
            NamespacedID, PluginLoader, property::value::FieldValue,
            script::api::register_render_api,
        },
        project::{Timeline, clip::ClipBindingValue, ids::IdGenerator},
        render::{
            RenderContext, render_frame_offscreen,
            video::{MediaFetchCache, builder::build_vertices},
            wgpuutil::{OffscreenTarget, WGpuUtil},
        },
    };
    use dashmap::DashMap;
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
            ClipBindingValue::default_properties(loader.get_clip_properties(&kind).unwrap());
        // コンポーネントが提供するプロパティはフラットなキーで配置される:
        // std:video:media, std:video:transform など
        let media_value = properties
            .get_mut(&NamespacedID::new("std", "video:media").unwrap())
            .unwrap();
        let ClipBindingValue::Static(FieldValue::Map(media)) = media_value else {
            panic!("media property must be a map")
        };
        media.insert("source".into(), FieldValue::String("fixture.mp4".into()));
        let transform_value = properties
            .get_mut(&NamespacedID::new("std", "video:transform").unwrap())
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
        let layer = timeline.insert_layer(&mut ids, "Video".into(), None, None);
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
        let mut engine = rhai::Engine::new();
        register_render_api(&mut engine);
        let camera = CameraInfo {
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            is_orthographic: true,
            orthographic_direction: Direction::Front,
            scale_factor: 1.0,
            fov: 60.0,
        };
        let project = Arc::new(RwLock::new(None));
        test(&RenderContext {
            path_to_stream: &paths,
            streams: &streams,
            media_fetch_cache: &cache,
            plugin_loader: &loader,
            script_engine: &engine,
            timeline: &timeline,
            camera_info: &camera,
            current_frame: 0,
            project: &project,
        });
    }

    #[test]
    fn video_properties_override_legacy_size_and_project_into_view() {
        with_preview(|ctx| {
            let batches = build_vertices(ctx);
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
    #[ignore = "requires ESOTEREEL_TEST_VIDEO and a wgpu adapter"]
    fn video_file_preview_matches_decoded_frame() {
        let path = std::env::var("ESOTEREEL_TEST_VIDEO")
            .expect("set ESOTEREEL_TEST_VIDEO to a video file");
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
                &crate::plugin::property::PropertyPath::parse("std:video/transform/scale").unwrap(),
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
                        decoded.data(0)[(iy + dy) * decoded.stride(0) + (ix + dx) * 4 + channel]
                            as f64
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
}
