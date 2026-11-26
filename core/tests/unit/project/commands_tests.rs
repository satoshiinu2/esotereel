use std::collections::HashMap;

use super::*;
use crate::project::history::{HistoryStack, redo_command, undo_command};
use esotereel_lib::plugin::{
    NamespacedID,
    property::{PropertyKey, PropertyPath},
};

#[test]
fn move_history_keeps_duration_on_undo_redo() {
    let mut project = Project::new();
    let timeline_id = project.insert_timeline(60.0);
    let layer_id = *project
        .timeline_ref(timeline_id)
        .unwrap()
        .iter_layers()
        .next()
        .unwrap()
        .0;
    let kind_id = NamespacedID::new("test", "clip").unwrap();
    let clip_id = project
        .new_clip_in_timeline(
            timeline_id,
            layer_id,
            10,
            30,
            kind_id.clone(),
            HashMap::new(),
        )
        .unwrap();

    let mut history = HistoryStack::default();
    let command = CommandHistory::ClipsMove {
        clips: vec![ClipMoveHistoryCtx {
            clip_id,
            old_position: 10,
            old_duration: 30,
            old_layer_id: layer_id,
            new_position: 200,
            new_duration: 80,
            new_layer_id: layer_id,
        }],
    };

    let mut applied = command.clone();
    execute_command(&mut project, timeline_id, &mut applied, None).unwrap();
    history.push(command.clone());

    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    assert_eq!(clip.position, 200);
    assert_eq!(clip.duration, 80);

    undo_command(&mut project, timeline_id, &mut history, None).unwrap();
    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    assert_eq!(clip.position, 10);
    assert_eq!(clip.duration, 30);

    redo_command(&mut project, timeline_id, &mut history, None).unwrap();
    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    assert_eq!(clip.position, 200);
    assert_eq!(clip.duration, 80);
}

#[test]
fn nested_folder_move_history_survives_undo_redo() {
    let mut project = Project::new();
    let timeline_id = project.insert_timeline(60.0);

    let root_folder = project
        .insert_folder_in_timeline(timeline_id, None, None, "root".to_string())
        .unwrap();
    let nested_folder = project
        .insert_folder_in_timeline(timeline_id, Some(root_folder), None, "nested".to_string())
        .unwrap();

    let base_layer = project
        .insert_layer_in_timeline(timeline_id, None, None, "base".to_string())
        .unwrap();
    let nested_layer = project
        .insert_layer_in_timeline(timeline_id, Some(nested_folder), None, "deep".to_string())
        .unwrap();

    let kind_id = NamespacedID::new("test", "clip").unwrap();
    let clip_id = project
        .new_clip_in_timeline(
            timeline_id,
            base_layer,
            10,
            30,
            kind_id.clone(),
            HashMap::new(),
        )
        .unwrap();

    let mut history = HistoryStack::default();
    let command = CommandHistory::ClipsMove {
        clips: vec![ClipMoveHistoryCtx {
            clip_id,
            old_position: 10,
            old_duration: 30,
            old_layer_id: base_layer,
            new_position: 90,
            new_duration: 60,
            new_layer_id: nested_layer,
        }],
    };

    history.push(command.clone());
    let mut applied = command.clone();
    execute_command(&mut project, timeline_id, &mut applied, None).unwrap();

    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    assert_eq!(clip.position, 90);
    assert_eq!(clip.duration, 60);
    assert!(
        project
            .timeline_ref(timeline_id)
            .unwrap()
            .get_layer(nested_layer)
            .unwrap()
            .clips
            .values()
            .any(|&id| id == clip_id)
    );

    undo_command(&mut project, timeline_id, &mut history, None).unwrap();
    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    assert_eq!(clip.position, 10);
    assert_eq!(clip.duration, 30);
    assert!(
        project
            .timeline_ref(timeline_id)
            .unwrap()
            .get_layer(base_layer)
            .unwrap()
            .clips
            .values()
            .any(|&id| id == clip_id)
    );

    redo_command(&mut project, timeline_id, &mut history, None).unwrap();
    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    assert_eq!(clip.position, 90);
    assert_eq!(clip.duration, 60);
    assert!(
        project
            .timeline_ref(timeline_id)
            .unwrap()
            .get_layer(nested_layer)
            .unwrap()
            .clips
            .values()
            .any(|&id| id == clip_id)
    );
}

#[test]
fn add_then_move_undo_redo_maintains_move_target() {
    let mut project = Project::new();
    let timeline_id = project.insert_timeline(60.0);
    let layer_id = *project
        .timeline_ref(timeline_id)
        .unwrap()
        .iter_layers()
        .next()
        .unwrap()
        .0;
    let kind_id = NamespacedID::new("test", "clip").unwrap();

    let add_command = CommandHistory::AddClip {
        clip_id: None,
        layer_id,
        position: 10,
        duration: 30,
        kind_id: kind_id.clone(),
        properties: HashMap::new(),
    };

    let mut add_history = add_command.clone();
    execute_command(&mut project, timeline_id, &mut add_history, None).unwrap();
    let clip_id = match &add_history {
        CommandHistory::AddClip { clip_id, .. } => clip_id.expect("clip id must exist"),
        _ => panic!("unexpected command"),
    };

    let move_command = CommandHistory::ClipsMove {
        clips: vec![ClipMoveHistoryCtx {
            clip_id,
            old_position: 10,
            old_duration: 30,
            old_layer_id: layer_id,
            new_position: 200,
            new_duration: 80,
            new_layer_id: layer_id,
        }],
    };

    let mut move_history = move_command.clone();
    execute_command(&mut project, timeline_id, &mut move_history, None).unwrap();

    let mut history = HistoryStack::default();
    history.push(add_history);
    history.push(move_command.clone());

    undo_command(&mut project, timeline_id, &mut history, None).unwrap();
    undo_command(&mut project, timeline_id, &mut history, None).unwrap();

    redo_command(&mut project, timeline_id, &mut history, None).unwrap();
    redo_command(&mut project, timeline_id, &mut history, None).unwrap();

    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    assert_eq!(clip.position, 200);
    assert_eq!(clip.duration, 80);
}
#[test]
fn nested_property_edits_preserve_siblings_and_undo_redo() {
    let mut project = Project::new();
    let timeline_id = project.insert_timeline(60.0);
    let layer_id = *project
        .timeline_ref(timeline_id)
        .unwrap()
        .iter_layers()
        .next()
        .unwrap()
        .0;
    let root = PropertyKey::from_namespaced(NamespacedID::new("test", "text").unwrap());
    let properties = HashMap::from([(
        root.clone(),
        ClipBindingValue::Static(FieldValue::Map(std::collections::BTreeMap::from([
            ("content".into(), FieldValue::String("Old".into())),
            ("font_size".into(), FieldValue::Float(24.0)),
        ]))),
    )]);
    let clip_id = project
        .new_clip_in_timeline(
            timeline_id,
            layer_id,
            0,
            60,
            NamespacedID::new("test", "clip").unwrap(),
            properties,
        )
        .unwrap();
    let path = PropertyPath::parse("test:text/content").unwrap();
    let request = CommandRequest::SetClipPropertyValue {
        clip_id,
        key: path.clone(),
        value: ClipBindingValue::Static(FieldValue::String("New".into())),
    };
    let wire = esotereel_lib::requests::Request::Command {
        commands: std::collections::VecDeque::from([(timeline_id, request)]),
    };
    let bytes = rkyv::to_bytes::<_, 1024>(&wire).unwrap();
    let decoded = rkyv::from_bytes::<esotereel_lib::requests::Request>(&bytes).unwrap();
    let esotereel_lib::requests::Request::Command { mut commands } = decoded else {
        panic!("command")
    };
    let (_, request) = commands.pop_front().unwrap();
    let mut history = command_to_history(&mut project, timeline_id, request).unwrap();
    execute_command(&mut project, timeline_id, &mut history, None).unwrap();
    // A second editor changes a sibling after the first history entry was captured.
    project
        .timeline_mut(timeline_id)
        .unwrap()
        .get_clip_mut(clip_id)
        .unwrap()
        .set_property_path(
            &PropertyPath::parse("test:text/font_size").unwrap(),
            ClipBindingValue::Static(FieldValue::Float(36.0)),
        )
        .unwrap();
    execute_command_undo(&mut project, timeline_id, history.clone(), None).unwrap();
    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    assert_eq!(
        clip.get_property_path(&path),
        Some(&FieldValue::String("Old".into()))
    );
    assert_eq!(
        clip.get_property_path(&PropertyPath::parse("test:text/font_size").unwrap()),
        Some(&FieldValue::Float(36.0))
    );
    execute_command(&mut project, timeline_id, &mut history, None).unwrap();
    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    assert_eq!(
        clip.get_property_path(&path),
        Some(&FieldValue::String("New".into()))
    );
    assert_eq!(
        clip.get_property_path(&PropertyPath::parse("test:text/font_size").unwrap()),
        Some(&FieldValue::Float(36.0))
    );
    assert!(PropertyPath::parse("test:text//content").is_err());
}

#[test]
fn clip_add_rewrites_media_source_to_asset_ref() {
    use std::collections::BTreeMap;

    use esotereel_lib::plugin::property::value::FieldValue;

    let mut project = Project::new();
    let timeline_id = project.insert_timeline(60.0);
    let layer_id = *project
        .timeline_ref(timeline_id)
        .unwrap()
        .iter_layers()
        .next()
        .unwrap()
        .0;
    let kind_id = NamespacedID::new("std", "video").unwrap();

    let source_path = PropertyPath::parse("std:media#media/source").unwrap();
    let media_map = BTreeMap::from([(
        "source".to_string(),
        FieldValue::String("/tmp/movie.mp4".to_string()),
    )]);
    let properties = HashMap::from([(
        source_path.root().clone(),
        ClipBindingValue::Static(FieldValue::Map(media_map)),
    )]);
    let media_paths = MediaPropertyPaths {
        source: Some(source_path.clone()),
        offset: None,
    };

    let clip_id = clip_add_core(
        &mut project,
        timeline_id,
        layer_id,
        0,
        50,
        kind_id,
        properties,
        Some(&media_paths),
        None,
    )
    .unwrap();

    let clip = project
        .timeline_ref(timeline_id)
        .unwrap()
        .get_clip(clip_id)
        .unwrap();
    let FieldValue::AssetRef(asset_ref) = clip.get_property_path(&source_path).unwrap() else {
        panic!("media source must be an asset reference value");
    };
    // 生パスではなく、レジストリで実パスへ解決できる参照になっていること。
    assert_eq!(
        project.assets().resolve(*asset_ref).unwrap(),
        std::path::Path::new("/tmp/movie.mp4")
    );
}

#[test]
fn legacy_source_strings_convert_to_asset_ref_values() {
    use std::collections::BTreeMap;

    use esotereel_lib::plugin::property::value::FieldValue;

    // 旧形式（文字列）のメディア source を開くときの変換契約を、
    // 1クリップ分の変換処理（resolve_source_to_asset_ref）で検証する。
    let mut project = Project::new();
    let registered_id = uuid::Uuid::new_v4();
    project
        .register_external_asset(registered_id, std::path::Path::new("/tmp/registered.mp4"))
        .unwrap();

    let source_path = PropertyPath::parse("std:media#media/source").unwrap();
    let properties_with = |source: FieldValue| {
        HashMap::from([(
            source_path.root().clone(),
            ClipBindingValue::Static(FieldValue::Map(BTreeMap::from([(
                "source".to_string(),
                source,
            )]))),
        )])
    };
    let source_of = |properties: &HashMap<PropertyKey, ClipBindingValue>| {
        let map = properties.get(source_path.root()).unwrap();
        let ClipBindingValue::Static(FieldValue::Map(map)) = map else {
            panic!("media property must be a map");
        };
        map.get("source").unwrap().clone()
    };

    // UUID 文字列（登録済み）→ その参照値になる。
    let mut properties = properties_with(FieldValue::String(registered_id.to_string()));
    assert!(crate::project::resolve_source_to_asset_ref(
        project.assets_mut(),
        &mut properties,
        &source_path
    ));
    assert_eq!(
        source_of(&properties),
        FieldValue::AssetRef(registered_id)
    );

    // 生パス → 登録してから参照値になり、レジストリで実パスへ解決できる。
    let mut properties = properties_with(FieldValue::String("/tmp/legacy.mp4".to_string()));
    assert!(crate::project::resolve_source_to_asset_ref(
        project.assets_mut(),
        &mut properties,
        &source_path
    ));
    let FieldValue::AssetRef(new_ref) = source_of(&properties) else {
        panic!("raw path source must become an asset reference value");
    };
    assert_eq!(
        project.assets().resolve(new_ref).unwrap(),
        std::path::Path::new("/tmp/legacy.mp4")
    );

    // 空文字列 → 未設定（nil UUID）になる。
    let mut properties = properties_with(FieldValue::String(String::new()));
    assert!(crate::project::resolve_source_to_asset_ref(
        project.assets_mut(),
        &mut properties,
        &source_path
    ));
    assert_eq!(
        source_of(&properties),
        FieldValue::AssetRef(uuid::Uuid::nil())
    );

    // UUID 文字列（未登録）→ 参照値になる（解決できない参照として残る）。
    let unknown_id = uuid::Uuid::new_v4();
    let mut properties = properties_with(FieldValue::String(unknown_id.to_string()));
    assert!(crate::project::resolve_source_to_asset_ref(
        project.assets_mut(),
        &mut properties,
        &source_path
    ));
    assert_eq!(source_of(&properties), FieldValue::AssetRef(unknown_id));

    // 既に AssetRef 値なら触らない。
    let mut properties = properties_with(FieldValue::AssetRef(registered_id));
    assert!(!crate::project::resolve_source_to_asset_ref(
        project.assets_mut(),
        &mut properties,
        &source_path
    ));
    assert_eq!(
        source_of(&properties),
        FieldValue::AssetRef(registered_id)
    );
}
