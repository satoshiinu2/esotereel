use std::collections::HashMap;

use super::*;
use crate::project::history::{HistoryStack, redo_command, undo_command};
use esotereel_lib::plugin::{NamespacedID, property::PropertyPath};

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
    let root = NamespacedID::new("test", "text").unwrap();
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
