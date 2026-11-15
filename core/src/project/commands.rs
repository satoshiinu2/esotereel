use esotereel_lib::{
    plugin::property::{PropertyPath, value::FieldValue},
    project::{
        Project,
        clip::ClipBindingValue,
        command::{ClipMoveHistoryCtx, ClipResizeHistoryCtx, CommandHistory, CommandRequest},
        ids::TimelineId,
        layer::LayerRemoveStrategy,
    },
    util::result::EsotereelError,
};

use crate::project::{MediaPropertyPaths, clip_add_core, clip_move_mul_core, clip_resize_core};

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

                    // ステールネス検出：クライアントが認識しているベース値とサーバー現在値を比較
                    if clip.position != ctx.base_position || clip.duration != ctx.base_duration {
                        anyhow::bail!(
                            "stale move request for clip {}: client base ({}, {}) != server current ({}, {})",
                            ctx.clip_id, ctx.base_position, ctx.base_duration, clip.position, clip.duration
                        );
                    }

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
        CommandRequest::ClipsResize { clips } => {
            let timeline = project
                .timeline_mut(timeline_id)
                .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

            let entries: anyhow::Result<Vec<_>> = clips
                .into_iter()
                .map(|ctx| -> anyhow::Result<ClipResizeHistoryCtx> {
                    let clip = timeline
                        .get_clip(ctx.clip_id)
                        .ok_or(EsotereelError::ClipNotFound(ctx.clip_id))?;

                    // ステールネス検出：クライアントが認識しているベース値とサーバー現在値を比較
                    if clip.position != ctx.base_position || clip.duration != ctx.base_duration {
                        anyhow::bail!(
                            "stale resize request for clip {}: client base ({}, {}) != server current ({}, {})",
                            ctx.clip_id, ctx.base_position, ctx.base_duration, clip.position, clip.duration
                        );
                    }

                    let position_delta = ctx.new_position - clip.position;
                    let duration_delta = ctx.new_duration - clip.duration;

                    Ok(ClipResizeHistoryCtx {
                        clip_id: ctx.clip_id,
                        left_edge: ctx.left_edge,
                        old_position: clip.position,
                        old_duration: clip.duration,
                        new_position: ctx.new_position,
                        new_duration: ctx.new_duration,
                        position_delta,
                        duration_delta,
                    })
                })
                .collect();
            CommandHistory::ClipsResize { clips: entries? }
        }
        CommandRequest::AddClip {
            layer_id,
            position,
            duration,
            kind_id,
            properties,
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
            }
        }
        CommandRequest::AddClipFromFile { .. } => {
            anyhow::bail!("file-drop commands must be resolved by the server before execution")
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

            // Capture the addressed leaf, so undo never overwrites sibling members.
            let old_value = clip
                .get_property_path(&key)
                .cloned()
                .map(ClipBindingValue::Static)
                .ok_or_else(|| anyhow::anyhow!("property path `{key}` not found"))?;

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
    media_paths: Option<&MediaPropertyPaths>,
) -> anyhow::Result<()> {
    match command {
        CommandHistory::ClipsMove { clips } => {
            clip_move_mul_core(project, timeline_id, clips.as_slice())?
        }
        CommandHistory::ClipsResize { clips } => {
            clip_resize_core(project, timeline_id, clips.as_slice(), media_paths)?
        }
        CommandHistory::AddClip {
            clip_id,
            layer_id,
            position,
            duration,
            kind_id,
            properties,
        } => {
            let actual_clip_id = clip_add_core(
                project,
                timeline_id,
                *layer_id,
                *position,
                *duration,
                kind_id.clone(),
                properties.clone(),
                media_paths,
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

            clip.set_property_path(key, value.clone())?;
            let root_value = clip
                .get_property_value(key.root())
                .ok_or(EsotereelError::AccessError(format!(
                    "root value not found for key: {}",
                    key.root()
                )))?
                .clone();
            timeline.touch_property_changed(*clip_id, key.root().clone(), root_value);
        }
    };
    Ok(())
}

pub fn execute_command_undo(
    project: &mut Project,
    timeline_id: TimelineId,
    command: CommandHistory,
    media_paths: Option<&MediaPropertyPaths>,
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
            execute_command(project, timeline_id, &mut undo_command, media_paths)?;
        }
        CommandHistory::ClipsResize { clips } => {
            // クリップリサイズのundoは、古い値に戻す
            let undo_resizes: Vec<_> = clips
                .iter()
                .map(|ctx| ClipResizeHistoryCtx {
                    clip_id: ctx.clip_id,
                    left_edge: ctx.left_edge,
                    old_position: ctx.new_position,
                    old_duration: ctx.new_duration,
                    new_position: ctx.old_position,
                    new_duration: ctx.old_duration,
                    position_delta: -ctx.position_delta,
                    duration_delta: -ctx.duration_delta,
                })
                .collect();

            let mut undo_command = CommandHistory::ClipsResize {
                clips: undo_resizes,
            };
            execute_command(project, timeline_id, &mut undo_command, media_paths)?;
        }
        CommandHistory::AddClip {
            clip_id,
            layer_id: _,
            position: _,
            duration: _,
            kind_id: _,
            properties: _,
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
            clip.set_property_path(&key, old_value.clone())?;
            let root_value = clip
                .get_property_value(key.root())
                .ok_or(EsotereelError::AccessError(format!(
                    "root value not found for key: {}",
                    key.root()
                )))?
                .clone();
            timeline.touch_property_changed(clip_id, key.root().clone(), root_value);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/project/commands_tests.rs"]
mod tests;
