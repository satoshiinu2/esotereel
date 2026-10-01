use esotereel_lib::{
    plugin::property::value::FieldValue,
    project::{
        Project,
        clip::ClipBindingValue,
        command::{ClipMoveHistoryCtx, CommandHistory, CommandRequest},
        ids::{ClipId, LayerFolderId, LayerId, TimelineId},
        layer::LayerRemoveStrategy,
    },
    util::result::EsotereelError,
};

use crate::project::{clip_add_core, clip_move_mul_core};

pub fn command_to_history(
    project: &mut Project,
    timeline_id: TimelineId,
    request: CommandRequest,
) -> anyhow::Result<CommandHistory> {
    let history = match request {
        CommandRequest::ClipsMove { clips } => {
            let timeline = project
                .timeline_mut(timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

            let entries: anyhow::Result<Vec<_>> = clips
                .into_iter()
                .map(|ctx| -> anyhow::Result<ClipMoveHistoryCtx> {
                    let (clip, layer) = timeline
                        .get_clip_and_layer(ctx.clip_id)
                        .ok_or(EsotereelError::ClipNotFound(ctx.clip_id))?;

                    Ok(ClipMoveHistoryCtx {
                        clip_id: ctx.clip_id,
                        old_position: clip.position,
                        old_duration: clip.duration,
                        old_layer_id: layer,
                        new_position: ctx.new_position,
                        new_duration: ctx.new_duration,
                        new_layer_id: ctx.new_layer_id,
                    })
                })
                .collect();
            CommandHistory::ClipsMove { clips: entries? }
        }
        CommandRequest::AddClip {
            layer_id,
            position,
            duration,
            kind_id,
            properties,
            translates,
        } => {
            // AddClipではIDが確定していないので、履歴にはIDを含めない
            // 実際のIDは実行時に確定する
            CommandHistory::AddClip {
                clip_id: None,
                layer_id,
                position,
                duration,
                kind_id,
                properties,
                translates,
            }
        }
        CommandRequest::AddLayer {
            parent_folder_id: parent_layer_id,
            insert_index,
            name,
        } => {
            // AddLayerではIDが確定していないので、履歴にはIDを含めない
            // 実際のIDは実行時に確定する
            CommandHistory::AddLayer {
                layer_id: None,
                parent_layer_id,
                insert_index,
                name,
            }
        }
        CommandRequest::AddFolder {
            parent_folder_id: parent_layer_id,
            insert_index,
            name,
        } => {
            // AddFolderではIDが確定していないので、履歴にはIDを含めない
            // 実際のIDは実行時に確定する
            CommandHistory::AddFolder {
                folder_id: None,
                parent_layer_id,
                insert_index,
                name,
            }
        }
        CommandRequest::SetClipPropertyValue {
            clip_id,
            key,
            value,
        } => {
            let timeline = project
                .timeline_ref(timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

            let clip = timeline
                .get_clip(clip_id)
                .ok_or(EsotereelError::ClipNotFound(clip_id))?;

            // 古い値を取得、存在しない場合はInt(0)をデフォルトとして使用
            let old_value = clip
                .get_property_value(&key)
                .cloned()
                .unwrap_or_else(|| ClipBindingValue::Static(FieldValue::Int(0)));

            CommandHistory::SetClipPropertyValue {
                clip_id,
                key: key.clone(),
                old_value,
                new_value: value.clone(),
            }
        }
    };

    Ok(history)
}

pub fn execute_command(
    project: &mut Project,
    timeline_id: TimelineId,
    command: &mut CommandHistory,
) -> anyhow::Result<()> {
    match command {
        CommandHistory::ClipsMove { clips } => {
            clip_move_mul_core(project, timeline_id, clips.as_slice())?
        }
        CommandHistory::AddClip {
            clip_id,
            layer_id,
            position,
            duration,
            kind_id,
            properties,
            translates,
        } => {
            let actual_clip_id = clip_add_core(
                project,
                timeline_id,
                *layer_id,
                *position,
                *duration,
                kind_id.clone(),
                properties.clone(),
                translates.clone(),
                *clip_id,
            )?;
            // 履歴のIDを実際のIDで更新
            *clip_id = Some(actual_clip_id);
        }
        CommandHistory::AddLayer {
            layer_id,
            parent_layer_id,
            insert_index,
            name,
        } => {
            let parent_layer_id = parent_layer_id.as_ref().copied();
            let insert_index = insert_index.as_ref().map(|x| *x as usize);

            let actual_layer_id = project.insert_layer_in_timeline(
                timeline_id,
                parent_layer_id,
                insert_index,
                name.to_string(),
            )?;
            // 履歴のIDを実際のIDで更新
            *layer_id = Some(actual_layer_id);
        }
        CommandHistory::AddFolder {
            folder_id,
            parent_layer_id,
            insert_index,
            name,
        } => {
            let parent_layer_id = parent_layer_id.as_ref().copied();
            let insert_index = insert_index.as_ref().map(|x| *x as usize);

            let actual_folder_id = project.insert_folder_in_timeline(
                timeline_id,
                parent_layer_id,
                insert_index,
                name.to_string(),
            )?;
            // 履歴のIDを実際のIDで更新
            *folder_id = Some(actual_folder_id);
        }
        CommandHistory::SetClipPropertyValue {
            clip_id,
            key,
            old_value: _,
            new_value: value,
        } => {
            let timeline = project
                .timeline_mut(timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

            let clip = timeline
                .get_clip_mut(*clip_id)
                .ok_or(EsotereelError::ClipNotFound(*clip_id))?;

            clip.set_property_value(key.clone(), value.clone())?;
            timeline.touch_property_changed(*clip_id, key.clone(), value.clone());
        }
    };
    Ok(())
}

pub fn execute_command_undo(
    project: &mut Project,
    timeline_id: TimelineId,
    command: CommandHistory,
) -> anyhow::Result<()> {
    match command {
        CommandHistory::ClipsMove { clips } => {
            // クリップ移動のundoは、古い位置に戻す
            let undo_moves: Vec<_> = clips
                .iter()
                .map(|ctx| ClipMoveHistoryCtx {
                    clip_id: ctx.clip_id,
                    old_position: ctx.new_position,
                    old_duration: ctx.new_duration,
                    old_layer_id: ctx.new_layer_id,
                    new_position: ctx.old_position,
                    new_duration: ctx.old_duration,
                    new_layer_id: ctx.old_layer_id,
                })
                .collect();

            let mut undo_command = CommandHistory::ClipsMove { clips: undo_moves };
            execute_command(project, timeline_id, &mut undo_command)?;
        }
        CommandHistory::AddClip {
            clip_id,
            layer_id: _,
            position: _,
            duration: _,
            kind_id: _,
            properties: _,
            translates: _,
        } => {
            // クリップ追加のundoは、クリップを削除する
            let timeline = project
                .timeline_mut(timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
            if let Some(id) = clip_id {
                timeline.remove_clip_by_id(id);
            }
        }
        CommandHistory::AddLayer {
            layer_id,
            parent_layer_id: _,
            insert_index: _,
            name: _,
        } => {
            // レイヤー追加のundoは、レイヤーを削除する
            let timeline = project
                .timeline_mut(timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
            if let Some(id) = layer_id {
                timeline.remove_layer(id);
            }
        }
        CommandHistory::AddFolder {
            folder_id,
            parent_layer_id: _,
            insert_index: _,
            name: _,
        } => {
            // フォルダー追加のundoは、フォルダーを削除する
            let timeline = project
                .timeline_mut(timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
            if let Some(id) = folder_id {
                timeline.remove_folder(id, LayerRemoveStrategy::Recursive)?;
            }
        }
        CommandHistory::SetClipPropertyValue {
            clip_id,
            key,
            old_value,
            new_value: _,
        } => {
            // プロパティ設定のundoは、古い値に戻す
            let timeline = project
                .timeline_mut(timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

            let clip = timeline
                .get_clip_mut(clip_id)
                .ok_or(EsotereelError::ClipNotFound(clip_id))?;

            // 古い値を設定して通知
            clip.set_property_value(key.clone(), old_value.clone())?;
            timeline.touch_property_changed(clip_id, key.clone(), old_value.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::project::history::{HistoryStack, redo_command, undo_command};
    use esotereel_lib::{plugin::NamespacedID, project::transform::ClipTranslates};

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
                ClipTranslates::None,
            )
            .unwrap();

        let mut history = HistoryStack::new(10);
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
        execute_command(&mut project, timeline_id, &mut applied).unwrap();
        history.push(command.clone());

        let clip = project
            .timeline_ref(timeline_id)
            .unwrap()
            .get_clip(clip_id)
            .unwrap();
        assert_eq!(clip.position, 200);
        assert_eq!(clip.duration, 80);

        undo_command(&mut project, timeline_id, &mut history).unwrap();
        let clip = project
            .timeline_ref(timeline_id)
            .unwrap()
            .get_clip(clip_id)
            .unwrap();
        assert_eq!(clip.position, 10);
        assert_eq!(clip.duration, 30);

        redo_command(&mut project, timeline_id, &mut history).unwrap();
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
                ClipTranslates::None,
            )
            .unwrap();

        let mut history = HistoryStack::new(10);
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
        execute_command(&mut project, timeline_id, &mut applied).unwrap();

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

        undo_command(&mut project, timeline_id, &mut history).unwrap();
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

        redo_command(&mut project, timeline_id, &mut history).unwrap();
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
            translates: ClipTranslates::None,
        };

        let mut add_history = add_command.clone();
        execute_command(&mut project, timeline_id, &mut add_history).unwrap();
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
        execute_command(&mut project, timeline_id, &mut move_history).unwrap();

        let mut history = HistoryStack::new(10);
        history.push(add_history);
        history.push(move_command.clone());

        undo_command(&mut project, timeline_id, &mut history).unwrap();
        undo_command(&mut project, timeline_id, &mut history).unwrap();

        redo_command(&mut project, timeline_id, &mut history).unwrap();
        redo_command(&mut project, timeline_id, &mut history).unwrap();

        let clip = project
            .timeline_ref(timeline_id)
            .unwrap()
            .get_clip(clip_id)
            .unwrap();
        assert_eq!(clip.position, 200);
        assert_eq!(clip.duration, 80);
    }
}
