use super::*;
use crate::plugin::{Plugin, script::bridge::field_values_to_rhai_map};
use crate::project::clip::ClipBindingValue;

#[test]
fn standard_video_script_uses_pixel_dimensions_from_transform_scale() {
    let plugin = Plugin::load(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plugins/std"
    )))
    .unwrap();
    // プロパティはコンポーネントが提供する。クリップ種別 std:video を構成する
    // std:media と std:transform のスキーマを合成し、media / transform
    // プロパティを含むことを確認する。
    let media_component = plugin
        .component_definitions
        .get(&crate::plugin::NamespacedID::new("std", "media").unwrap())
        .unwrap();
    let transform_component = plugin
        .component_definitions
        .get(&crate::plugin::NamespacedID::new("std", "transform").unwrap())
        .unwrap();
    let fields: Vec<_> = media_component
        .properties
        .iter()
        .chain(transform_component.properties.iter())
        .cloned()
        .collect();
    let fields = &fields;
    let media = fields
        .iter()
        .find(|f| f.key.local_id() == "video:media")
        .unwrap();
    let transform = fields
        .iter()
        .find(|f| f.key.local_id() == "video:transform")
        .unwrap();
    let crate::plugin::property::value::FieldTypeKind::Group { .. } = &media.kind else {
        panic!("media property must be a group")
    };
    let crate::plugin::property::value::FieldTypeKind::Group { .. } = &transform.kind else {
        panic!("transform property must be a group")
    };
    assert!(
        !fields
            .iter()
            .any(|field| matches!(field.key.local_id(), "width" | "height"))
    );
    let leaves = crate::plugin::property::PropertySchema::leaf_fields(fields).unwrap();
    assert_eq!(
        leaves
            .iter()
            .map(|(path, _)| path.to_string())
            .collect::<Vec<_>>(),
        [
            "std:video:media/source",
            "std:video:media/source_offset",
            "std:video:transform/position",
            "std:video:transform/rotation",
            "std:video:transform/scale"
        ]
    );
    let mut props = ClipBindingValue::default_properties(fields);
    // コンポーネントのプロパティはフラットなキー（std:video:media, std:video:transform）で配置される。
    // transform デフォルト値をセット（as_mat4 は position/rotation/scale を全て要求する）
    let transform_value = props
        .get_mut(&crate::plugin::NamespacedID::new("std", "video:transform").unwrap())
        .unwrap();
    let ClipBindingValue::Static(crate::plugin::property::value::FieldValue::Map(transform_map)) =
        transform_value
    else {
        panic!("transform property must be a map")
    };
    let vec = |v: [f64; 3]| {
        crate::plugin::property::value::FieldValue::Array(
            v.into_iter()
                .map(crate::plugin::property::value::FieldValue::Float)
                .collect(),
        )
    };
    transform_map.insert("position".into(), vec([0.0, 0.0, 0.0]));
    transform_map.insert("rotation".into(), vec([0.0, 0.0, 0.0]));
    transform_map.insert("scale".into(), vec([1.0, 1.0, 1.0]));

    let cache = Arc::new(MediaFetchCache::default());
    let path_to_stream = Arc::new(DashMap::new());
    let streams = Arc::new(DashMap::new());
    let mut context = PluginRenderContext::new(
        1,
        2.0,
        Mat4::IDENTITY,
        Arc::clone(&path_to_stream),
        Arc::clone(&streams),
        Arc::clone(&cache),
    );
    context.set_video_source("test.mp4".into());
    let mut engine = rhai::Engine::new();
    register_render_api(&mut engine);
    let script = plugin
        .component_scripts
        .get(&crate::plugin::NamespacedID::new("std", "media").unwrap())
        .unwrap();
    script
        .call::<()>(
            &engine,
            "render_frame",
            (context.clone(), field_values_to_rhai_map(&props)),
        )
        .unwrap();
    let (_, paths) = context.clone().into_parts();
    assert_eq!(paths, vec!["test.mp4"]);
    // Supply a decoded frame at the playhead; the script must request 2s, not 0s.
    let mut player =
        StreamPlayer::new_from_metadata(ffmpeg_next::codec::Id::H264, 32, 24, 1.0 / 30.0, &[])
            .unwrap();
    player.frames.insert(
        ordered_float::OrderedFloat(2.0),
        ffmpeg_next::util::frame::video::Video::new(ffmpeg_next::format::Pixel::RGBA, 32, 24),
    );
    path_to_stream.insert("test.mp4".into(), StreamState::Loaded(7));
    streams.insert(7, player);
    for factor in [1.0, 0.5, 2.0] {
        let transform_value = props
            .get_mut(&crate::plugin::NamespacedID::new("std", "video:transform").unwrap())
            .unwrap();
        let ClipBindingValue::Static(crate::plugin::property::value::FieldValue::Map(
            transform_map,
        )) = transform_value
        else {
            panic!("transform property must be a map")
        };
        transform_map.insert(
            "scale".into(),
            crate::plugin::property::value::FieldValue::Array(
                [32.0 * factor, 24.0 * factor, 1.0]
                    .into_iter()
                    .map(crate::plugin::property::value::FieldValue::Float)
                    .collect(),
            ),
        );
        let mut context = PluginRenderContext::new(
            1,
            2.0,
            Mat4::IDENTITY,
            Arc::clone(&path_to_stream),
            Arc::clone(&streams),
            Arc::clone(&cache),
        );
        context.set_video_source("test.mp4".into());
        script
            .call::<()>(
                &engine,
                "render_frame",
                (context.clone(), field_values_to_rhai_map(&props)),
            )
            .unwrap();
        let (batches, _) = context.clone().into_parts();
        assert_eq!(batches.len(), 1);
        let model = batches[0].transform;
        assert_eq!(
            model.transform_point3(Vec3::ZERO),
            Vec3::new(-16.0 * factor as f32, -12.0 * factor as f32, 0.0)
        );
        assert_eq!(
            model.transform_point3(Vec3::new(1.0, 1.0, 0.0)),
            Vec3::new(16.0 * factor as f32, 12.0 * factor as f32, 0.0)
        );
    }
}

#[test]
fn transform_uses_degrees_and_rejects_invalid_vectors() {
    fn array(v: &[f64]) -> rhai::Array {
        v.iter().map(|v| rhai::Dynamic::from(*v)).collect()
    }
    let matrix = transform_matrix(
        array(&[10.0, 20.0, 0.0]),
        array(&[0.0, 0.0, 90.0]),
        array(&[2.0, 3.0, 1.0]),
    )
    .unwrap();
    assert!((matrix.transform_point3(Vec3::X) - Vec3::new(10.0, 22.0, 0.0)).length() < 0.0001);
    assert!(transform_matrix(array(&[0.0]), array(&[0.0; 3]), array(&[1.0; 3])).is_err());
    assert!(transform_matrix(array(&[f64::NAN; 3]), array(&[0.0; 3]), array(&[1.0; 3])).is_err());
}
