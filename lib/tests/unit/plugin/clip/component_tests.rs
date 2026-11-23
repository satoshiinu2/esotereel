use super::*;
use crate::plugin::property::value::FieldTypeKind;

fn make_def(id_local: &str) -> ComponentDefinition {
    ComponentDefinition {
        id: NamespacedID::new("std", id_local).unwrap(),
        name: id_local.to_string(),
        capabilities: BTreeSet::from([ComponentCapability::Transformable]),
        editor: Some("transform".to_string()),
        script: None,
        requires: Vec::new(),
        properties: Vec::new(),
    }
}

#[test]
fn registry_insert_and_lookup() {
    let mut registry = ComponentRegistry::default();
    let def = make_def("transform");
    let id = def.id.clone();
    registry
        .merge_component_definitions(HashMap::from([(id.clone(), def)]))
        .unwrap();
    assert!(registry.contains(&id));
    assert_eq!(registry.len(), 1);
}

#[test]
fn by_capability_filter() {
    let mut registry = ComponentRegistry::default();
    let transform = make_def("transform");
    let transform_id = transform.id.clone();
    let media = ComponentDefinition {
        id: NamespacedID::new("std", "video").unwrap(),
        name: "Video".to_string(),
        capabilities: BTreeSet::from([
            ComponentCapability::FrameProvider,
            ComponentCapability::Prefetchable,
        ]),
        editor: Some("video".to_string()),
        script: None,
        requires: Vec::new(),
        properties: Vec::new(),
    };
    let media_id = media.id.clone();
    registry
        .merge_component_definitions(HashMap::from([
            (transform_id.clone(), transform),
            (media_id.clone(), media),
        ]))
        .unwrap();

    let providers: Vec<_> = registry
        .by_capability(&ComponentCapability::FrameProvider)
        .iter()
        .map(|d| d.id.clone())
        .collect();
    assert_eq!(providers, vec![media_id]);

    let ids: Vec<_> = registry
        .ids_by_capability(&ComponentCapability::Transformable)
        .iter()
        .cloned()
        .collect();
    assert_eq!(ids, vec![transform_id]);
}

#[test]
fn duplicate_id_is_rejected() {
    let mut registry = ComponentRegistry::default();
    let def = make_def("transform");
    let id = def.id.clone();
    registry
        .merge_component_definitions(HashMap::from([(id.clone(), def)]))
        .unwrap();
    assert!(
        registry
            .merge_component_definitions(HashMap::from([(id.clone(), make_def("transform"))]))
            .is_err()
    );
}

#[test]
fn parse_component_definitions_from_toml() {
    let text = r#"
[[components]]
id = "media"
name = "Media"
capabilities = ["frame-provider", "prefetchable"]
editor = "video"

[[components]]
id = "transform"
name = "Transform"
capabilities = ["transformable"]
editor = "transform"
"#;
    let map = parse_component_definitions(text, "std").unwrap();
    assert_eq!(map.len(), 2);

    let media = map
        .get(&NamespacedID::new("std", "media").unwrap())
        .unwrap();
    assert!(
        media
            .capabilities
            .contains(&ComponentCapability::FrameProvider)
    );
    assert!(
        media
            .capabilities
            .contains(&ComponentCapability::Prefetchable)
    );
    assert_eq!(
        media
            .capabilities
            .iter()
            .find(|c| matches!(c, ComponentCapability::FrameProvider))
            .map(|_| "render_frame"),
        Some("render_frame")
    );
    assert_eq!(media.editor.as_deref(), Some("video"));

    let transform = map
        .get(&NamespacedID::new("std", "transform").unwrap())
        .unwrap();
    assert!(
        transform
            .capabilities
            .contains(&ComponentCapability::Transformable)
    );
}

#[test]
fn parse_component_definitions_with_properties() {
    let text = r#"
[[components]]
id = "video"
name = "Video"
capabilities = ["frame-provider"]
properties = [
  { key = "source", label = "Source", kind = { type = "string" } },
  { key = "volume", label = "Volume", kind = { type = "float", min = 0.0, max = 1.0, step = 0.1 }, default = 1.0 },
]
"#;
    let map = parse_component_definitions(text, "std").unwrap();
    let video = map
        .get(&NamespacedID::new("std", "video").unwrap())
        .unwrap();
    assert_eq!(video.properties.len(), 2);

    let source = video
        .properties
        .iter()
        .find(|p| p.key.address() == "std:video#source")
        .unwrap();
    assert_eq!(source.label, "Source");
    assert!(matches!(source.kind, FieldTypeKind::String));
    assert_eq!(source.default, FieldValue::String(String::new()));

    let volume = video
        .properties
        .iter()
        .find(|p| p.key.address() == "std:video#volume")
        .unwrap();
    assert_eq!(volume.label, "Volume");
    assert_eq!(volume.default, FieldValue::Float(1.0));
}

#[test]
fn unknown_capability_is_rejected() {
    let text = r#"
[[components]]
id = "custom"
name = "Custom"
capabilities = ["my-custom-cap"]
"#;
    assert!(parse_component_definitions(text, "std").is_err());
}

#[test]
fn capability_as_str_is_lowercase() {
    assert_eq!(
        ComponentCapability::FrameProvider.as_str(),
        "frame-provider"
    );
    assert_eq!(ComponentCapability::Prefetchable.as_str(), "prefetchable");
}

#[test]
fn capability_call_mappings() {
    assert_eq!(
        ComponentCapability::FrameProvider.call(),
        CapabilityCall::Scheduler {
            fn_name: "render_frame",
            hook: ComponentHook::Evaluate
        }
    );
    assert_eq!(
        ComponentCapability::Prefetchable.call(),
        CapabilityCall::Scheduler {
            fn_name: "prefetch",
            hook: ComponentHook::Prefetch
        }
    );
    assert_eq!(
        ComponentCapability::Transformable.call(),
        CapabilityCall::Scheduler {
            fn_name: "get_transform",
            hook: ComponentHook::Process
        }
    );
    assert_eq!(
        ComponentCapability::FileDropHandler.call(),
        CapabilityCall::FileDrop {
            fn_name: "handle_file_drop"
        }
    );
    assert_eq!(
        ComponentCapability::MediaSource.call(),
        CapabilityCall::Marker
    );
}

#[test]
fn file_drop_values_merge_into_defaults_and_reject_unknown_fields() {
    let mut defaults = FieldValue::Map(std::collections::BTreeMap::from([
        ("source".to_string(), FieldValue::String(String::new())),
        (
            "source_offset".to_string(),
            FieldValue::String("0".to_string()),
        ),
    ]));
    let returned = FieldValue::Map(std::collections::BTreeMap::from([(
        "source".to_string(),
        FieldValue::String("/media/clip.mp4".to_string()),
    )]));

    merge_field_value(&mut defaults, returned, "std:media#media").unwrap();

    assert_eq!(
        defaults,
        FieldValue::Map(std::collections::BTreeMap::from([
            (
                "source".to_string(),
                FieldValue::String("/media/clip.mp4".to_string()),
            ),
            (
                "source_offset".to_string(),
                FieldValue::String("0".to_string())
            ),
        ]))
    );

    let unknown_field = FieldValue::Map(std::collections::BTreeMap::from([(
        "missing".to_string(),
        FieldValue::String("value".to_string()),
    )]));
    assert!(
        merge_field_value(&mut defaults, unknown_field, "std:media#media")
            .unwrap_err()
            .to_string()
            .contains("undeclared field")
    );
}

#[test]
fn file_drop_handler_returns_property_map() {
    let engine = Engine::new();
    let ast = engine
        .compile(
            r#"
                    fn handle_file_drop(source_path) {
                        #{
                            "std:media#media": #{
                                source: source_path,
                            },
                        }
                    }
                "#,
        )
        .unwrap();
    let script = crate::plugin::script::CompiledScript {
        ast,
        available_functions: vec!["handle_file_drop".to_string()],
    };

    let result = script
        .call::<Dynamic>(
            &engine,
            "handle_file_drop",
            ("/media/clip.mp4".to_string(),),
        )
        .unwrap();
    let FieldValue::Map(properties) = dynamic_to_field_value(result).unwrap() else {
        panic!("handler should return a property map");
    };
    let FieldValue::Map(media) = properties.get("std:media#media").unwrap() else {
        panic!("media property should be a map");
    };
    assert_eq!(
        media.get("source"),
        Some(&FieldValue::String("/media/clip.mp4".to_string()))
    );
}

#[test]
fn resolve_definition_expands_kind_components_in_order() {
    let mut registry = ComponentRegistry::default();
    let media = ComponentDefinition {
        id: NamespacedID::new("std", "media").unwrap(),
        name: "Media".to_string(),
        capabilities: BTreeSet::from([ComponentCapability::FrameProvider]),
        editor: None,
        script: None,
        requires: Vec::new(),
        properties: Vec::new(),
    };
    let transform = ComponentDefinition {
        id: NamespacedID::new("std", "transform").unwrap(),
        name: "Transform".to_string(),
        capabilities: BTreeSet::from([ComponentCapability::Transformable]),
        editor: None,
        script: None,
        requires: Vec::new(),
        properties: Vec::new(),
    };
    registry
        .merge_component_definitions(HashMap::from([
            (media.id.clone(), media),
            (transform.id.clone(), transform),
        ]))
        .unwrap();

    let kind_id = NamespacedID::new("std", "video").unwrap();
    let def = registry
        .resolve_definition(
            &kind_id,
            &[
                NamespacedID::new("std", "media").unwrap(),
                NamespacedID::new("std", "transform").unwrap(),
                // 重複した宣言は1回だけ現れる。
                NamespacedID::new("std", "media").unwrap(),
            ],
        )
        .unwrap();
    assert_eq!(def.kind_id, kind_id);
    let ids: Vec<_> = def.components.iter().map(|c| c.id.clone()).collect();
    assert_eq!(
        ids,
        vec![
            NamespacedID::new("std", "media").unwrap(),
            NamespacedID::new("std", "transform").unwrap(),
        ]
    );
}

#[test]
fn resolve_definition_rejects_missing_component() {
    let registry = ComponentRegistry::default();
    assert!(
        registry
            .resolve_definition(
                &NamespacedID::new("std", "video").unwrap(),
                &[NamespacedID::new("std", "missing").unwrap()],
            )
            .is_err()
    );
}

#[test]
fn clip_instance_holds_properties_and_state() {
    let instance = ClipInstance::new(NamespacedID::new("std", "video").unwrap(), &[]);
    assert_eq!(instance.kind_id.full(), "std:video");
    assert!(instance.properties.is_empty());
    assert!(instance.state.is_empty());
}

#[test]
fn clip_instance_new_populates_component_property_defaults() {
    let video_def = ComponentDefinition {
        id: NamespacedID::new("std", "video").unwrap(),
        name: "Video".to_string(),
        capabilities: BTreeSet::from([ComponentCapability::FrameProvider]),
        editor: None,
        script: None,
        requires: Vec::new(),
        properties: vec![
            PropertySchema {
                key: PropertyKey::from_namespaced(NamespacedID::new("std", "source").unwrap()),
                category: vec!["Media".to_string()],
                label: "Source".to_string(),
                kind: FieldTypeKind::String,
                default: FieldValue::String("default.mp4".to_string()),
                capability: None,
            },
            PropertySchema {
                key: PropertyKey::from_namespaced(NamespacedID::new("std", "volume").unwrap()),
                category: vec![],
                label: "Volume".to_string(),
                kind: FieldTypeKind::Float {
                    min: Some(0.0),
                    max: Some(1.0),
                    step: 0.1,
                },
                default: FieldValue::Float(1.0),
                capability: None,
            },
        ],
    };

    let instance = ClipInstance::new(NamespacedID::new("std", "video").unwrap(), &[video_def]);
    assert_eq!(instance.properties.len(), 2);

    assert!(matches!(
        instance.properties.get(&PropertyKey::from_namespaced(
            NamespacedID::new("std", "source").unwrap(),
        )),
        Some(ClipBindingValue::Static(FieldValue::String(v))) if v == "default.mp4"
    ));
    assert!(matches!(
        instance.properties.get(&PropertyKey::from_namespaced(
            NamespacedID::new("std", "volume").unwrap(),
        )),
        Some(ClipBindingValue::Static(FieldValue::Float(v))) if *v == 1.0
    ));
    assert!(instance.state.is_empty());
}

#[test]
fn clip_instance_new_merges_multiple_components_properties() {
    let a = ComponentDefinition {
        id: NamespacedID::new("std", "a").unwrap(),
        name: "A".to_string(),
        capabilities: BTreeSet::new(),
        editor: None,
        script: None,
        requires: Vec::new(),
        properties: vec![PropertySchema {
            key: PropertyKey::from_namespaced(NamespacedID::new("std", "prop_a").unwrap()),
            category: vec![],
            label: "Prop A".to_string(),
            kind: FieldTypeKind::String,
            default: FieldValue::String("a".to_string()),
            capability: None,
        }],
    };
    let b = ComponentDefinition {
        id: NamespacedID::new("std", "b").unwrap(),
        name: "B".to_string(),
        capabilities: BTreeSet::new(),
        editor: None,
        script: None,
        requires: Vec::new(),
        properties: vec![PropertySchema {
            key: PropertyKey::from_namespaced(NamespacedID::new("std", "prop_b").unwrap()),
            category: vec![],
            label: "Prop B".to_string(),
            kind: FieldTypeKind::Int {
                min: None,
                max: None,
            },
            default: FieldValue::Int(42),
            capability: None,
        }],
    };

    let instance = ClipInstance::new(NamespacedID::new("std", "clip").unwrap(), &[a, b]);
    assert_eq!(instance.properties.len(), 2);

    assert!(matches!(
        instance.properties.get(&PropertyKey::from_namespaced(
            NamespacedID::new("std", "prop_a").unwrap(),
        )),
        Some(ClipBindingValue::Static(FieldValue::String(v))) if v == "a"
    ));
    assert!(matches!(
        instance.properties.get(&PropertyKey::from_namespaced(
            NamespacedID::new("std", "prop_b").unwrap(),
        )),
        Some(ClipBindingValue::Static(FieldValue::Int(v))) if *v == 42
    ));
}

#[test]
fn parse_component_definitions_properties_default_empty() {
    let text = r#"
[[components]]
id = "video"
name = "Video"
capabilities = ["frame-provider"]
"#;
    let map = parse_component_definitions(text, "std").unwrap();
    let video = map
        .get(&NamespacedID::new("std", "video").unwrap())
        .unwrap();
    assert!(video.properties.is_empty());
}

#[test]
fn hook_execution_order_is_capability_timing_order() {
    // ComponentHook の Ord に従い、Prefetch < Evaluate < Process の順で実行される。
    let mut hooks = Vec::new();
    hooks.push((ComponentHook::Finalize, "finalize"));
    hooks.push((ComponentHook::Prepare, "prepare"));
    hooks.push((ComponentHook::Evaluate, "evaluate"));
    hooks.sort_by_key(|(hook, _)| hook.clone());
    let order: Vec<_> = hooks.iter().map(|(h, _)| h.clone()).collect();
    assert_eq!(
        order,
        vec![
            ComponentHook::Prepare,
            ComponentHook::Evaluate,
            ComponentHook::Finalize,
        ]
    );
}

#[test]
fn scheduler_runs_rhai_capability_function_via_loaded_plugin() {
    use std::{env, fs};

    let dir = env::temp_dir().join("esotereel_scheduler_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("components")).unwrap();

    fs::write(
        dir.join("manifest.toml"),
        r#"id = "sched-test"
name = "Scheduler Test"
version = "0.1.0"
"#,
    )
    .unwrap();
    fs::write(
        dir.join("components").join("video.rhai"),
        "fn render_frame(ctx, props) { return \"called\"; }\n",
    )
    .unwrap();
    fs::write(
        dir.join("components").join("video.toml"),
        r#"[[components]]
id = "video"
name = "Video"
capabilities = ["frame-provider"]
script = "video.rhai"
"#,
    )
    .unwrap();

    let mut loader = PluginLoader::new();
    loader.load_additional_plugin(&dir).unwrap();
    assert!(
        loader
            .get_component_script(&NamespacedID::new("sched-test", "video").unwrap())
            .is_some()
    );

    let engine = Engine::new();
    let instance = ClipInstance {
        kind_id: NamespacedID::new("sched-test", "video").unwrap(),
        properties: HashMap::new(),
        state: HashMap::new(),
    };
    let def = RuntimeDefinition {
        kind_id: NamespacedID::new("sched-test", "video").unwrap(),
        components: vec![ComponentDefinition {
            id: NamespacedID::new("sched-test", "video").unwrap(),
            name: "Video".to_string(),
            capabilities: BTreeSet::from([ComponentCapability::FrameProvider]),
            editor: None,
            script: Some("video.rhai".to_string()),
            requires: Vec::new(),
            properties: Vec::new(),
        }],
    };

    use crate::render::video::MediaFetchCache;
    use dashmap::DashMap;
    use glam::Mat4;
    use std::sync::Arc;
    let mut ctx = PluginRenderContext::new(
        0,
        0.0,
        Mat4::IDENTITY,
        Arc::new(DashMap::new()),
        Arc::new(DashMap::new()),
        Arc::new(MediaFetchCache::default()),
    );
    let scheduler = Scheduler::new();
    let result = scheduler.run_clip(&engine, &loader, &instance, &def, &mut ctx);
    assert!(
        result.is_ok(),
        "scheduler should run the Rhai capability function: {result:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}
