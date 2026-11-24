use super::*;
use crate::plugin::Plugin;
use crate::plugin::clip::component::{ClipInstance, RuntimeDefinition};
use crate::plugin::script::props::ComponentProps;
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
    let video_component = plugin
        .component_definitions
        .get(&crate::plugin::NamespacedID::new("std", "video").unwrap())
        .unwrap();
    let fields: Vec<_> = media_component
        .properties
        .iter()
        .chain(transform_component.properties.iter())
        .cloned()
        .collect();
    let fields = &fields;
    let media = fields.iter().find(|f| f.key.name() == "media").unwrap();
    let transform = fields.iter().find(|f| f.key.name() == "transform").unwrap();
    let crate::plugin::property::value::FieldTypeKind::Group { .. } = &media.kind else {
        panic!("media property must be a group")
    };
    let crate::plugin::property::value::FieldTypeKind::Group { .. } = &transform.kind else {
        panic!("transform property must be a group")
    };
    assert!(
        !fields
            .iter()
            .any(|field| matches!(field.key.local_name(), "width" | "height"))
    );
    let leaves = crate::plugin::property::PropertySchema::leaf_fields(fields).unwrap();
    assert_eq!(
        leaves
            .iter()
            .map(|(path, _)| path.to_string())
            .collect::<Vec<_>>(),
        [
            "std:media#media/source",
            "std:media#media/source_offset",
            "std:transform#transform/position",
            "std:transform#transform/rotation",
            "std:transform#transform/scale"
        ]
    );
    let mut props = ClipBindingValue::default_properties(fields);
    // コンポーネントのプロパティは所有者付きのキー（std:media#media, std:transform#transform）で配置される。
    // transform デフォルト値をセット（as_mat4 は position/rotation/scale を全て要求する）
    let transform_value = props
        .get_mut(
            &crate::plugin::property::PropertyKey::new(
                crate::plugin::NamespacedID::new("std", "transform").unwrap(),
                "transform",
            )
            .unwrap(),
        )
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

    // Scheduler と同じ経路で props を構築する: render_frame を担う media
    // コンポーネントは、自プロパティと [[requires]] で宣言した transform
    // 依存がローカル名で解決できる。
    let definition = RuntimeDefinition {
        kind_id: crate::plugin::NamespacedID::new("std", "video").unwrap(),
        components: vec![
            media_component.clone(),
            transform_component.clone(),
            video_component.clone(),
        ],
    };
    let mut instance = ClipInstance {
        kind_id: definition.kind_id.clone(),
        properties: props,
        state: std::collections::HashMap::new(),
    };

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
    let engine = &plugin.engine;
    let script = plugin
        .component_scripts
        .get(&crate::plugin::NamespacedID::new("std", "media").unwrap())
        .unwrap();
    script
        .call::<()>(
            &engine,
            "render_frame",
            (
                context.clone(),
                ComponentProps::build(&definition, &instance, media_component).unwrap(),
            ),
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
        let transform_value = instance
            .properties
            .get_mut(
                &crate::plugin::property::PropertyKey::new(
                    crate::plugin::NamespacedID::new("std", "transform").unwrap(),
                    "transform",
                )
                .unwrap(),
            )
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
                (
                    context.clone(),
                    ComponentProps::build(&definition, &instance, media_component).unwrap(),
                ),
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
fn media_api_is_registered_only_with_media_permission() {
    let make_context = || {
        PluginRenderContext::new(
            1,
            0.0,
            Mat4::IDENTITY,
            Arc::new(DashMap::new()),
            Arc::new(DashMap::new()),
            Arc::new(MediaFetchCache::default()),
        )
    };
    let compile_probe = |engine: &rhai::Engine| {
        let ast = engine
            .compile("fn probe(ctx) { ctx.get_texture(\"some.mp4\", 0.0) }")
            .unwrap();
        crate::plugin::script::CompiledScript {
            ast,
            available_functions: vec!["probe".to_string()],
        }
    };

    // 権限なしでは get_texture が登録されず、呼び出しがエラーになる
    let mut denied_engine = rhai::Engine::new();
    register_render_api(&mut denied_engine, &[]);
    let script = compile_probe(&denied_engine);
    let denied = script.call::<i64>(&denied_engine, "probe", (make_context(),));
    assert!(
        denied.is_err(),
        "get_texture must not be callable without the media permission: {denied:?}"
    );

    // media 権限があれば呼び出せる（未ロードのパスは -1）
    let mut allowed_engine = rhai::Engine::new();
    register_render_api(&mut allowed_engine, &[PluginPermission::Media]);
    let script = compile_probe(&allowed_engine);
    let allowed = script.call::<i64>(&allowed_engine, "probe", (make_context(),));
    assert_eq!(allowed.unwrap(), -1);
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
