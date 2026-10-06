use std::collections::VecDeque;

use esotereel_lib::{
    project::{
        clip::{ClipBindingValue, ClipData},
        command::{ClipMoveCtx, CommandRequest},
        ids::{LayerFolderId, LayerId, TimelineId},
        transform::{ClipTranslate, ClipTranslates},
    },
    plugin::NamespacedID,
    requests::Request,
    util::result::EsotereelError,
};

use crate::network::ClientNetworkHandler;
use crate::state::ClientState;
use crate::WrapperErrorCode;
use crate::slice_from_ptr_or_empty;

#[derive(Default, Debug)]
pub struct CommandQueue {
    pending: VecDeque<(TimelineId, CommandRequest)>,
}

impl CommandQueue {
    pub fn enqueue(&mut self, timeline_id: TimelineId, command: CommandRequest) {
        self.pending.push_back((timeline_id, command));
    }

    pub fn send_all(&mut self, network: &ClientNetworkHandler) {
        if self.pending.is_empty() {
            return;
        }

        let req = &Request::Command {
            commands: std::mem::take(&mut self.pending),
        };

        network.send(req);
    }

    pub fn req_cmd_clip_move_mul(
        &mut self,
        project: &esotereel_lib::project::Project,
        timeline_id: TimelineId,
        clip_ids: &[u64],
        position_moved: i64,
        duration_added: i64,
        layer_moved: isize,
    ) -> anyhow::Result<()> {
        let timeline = project
            .timeline_ref(timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

        let execution_order: Vec<u64> = timeline.outline.iter_execution_order().collect();

        let clip_data = clip_ids
            .iter()
            .filter_map(|clip_id| {
                let (clip, layer_id) = timeline.get_clip_and_layer(*clip_id)?;

                let current_index = execution_order.iter().position(|&id| id == layer_id)?;
                let new_index = current_index.checked_add_signed(layer_moved)?;
                let new_layer_id = *execution_order.get(new_index)?;

                Some(ClipMoveCtx {
                    clip_id: *clip_id,
                    new_position: clip.position + position_moved,
                    new_duration: clip.duration + duration_added,
                    new_layer_id,
                })
            })
            .collect();

        let command = CommandRequest::ClipsMove { clips: clip_data };
        self.enqueue(timeline_id, command);

        Ok(())
    }

    pub fn req_cmd_add_clip_dummy(
        &mut self,
        state: &ClientState,
        timeline_id: TimelineId,
        position: i64,
        layer_id: LayerId,
    ) -> anyhow::Result<()> {
        let _clip_data = ClipData::Video {
            path: "/home/satoshiinu/Videos/3.mp4".to_string(),
            media_offset: 0.0,
        };

        let translates = ClipTranslates::Normal(ClipTranslate {
            position: [-100.0, -100.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [400.0, 300.0, 1.0],
        });

        let kind_id = NamespacedID::parse("std:video").unwrap();

        let loader = state.plugin_loader.read().expect("mutex poisoned");

        let property_schema = loader
            .get_clip_properties(&kind_id)
            .ok_or(EsotereelError::ClipKindNotFound(kind_id.clone()))?;

        let properties = ClipBindingValue::default_properties(property_schema);
        drop(loader);

        let command = CommandRequest::AddClip {
            layer_id,
            position,
            duration: 10000,
            kind_id,
            properties,
            translates,
        };

        self.enqueue(timeline_id, command);

        Ok(())
    }

    pub fn req_cmd_add_layer(
        &mut self,
        timeline_id: TimelineId,
        has_parent: bool,
        parent_folder_id: LayerFolderId,
        has_insert_index: bool,
        insert_index: usize,
        name: &str,
    ) {
        let command = CommandRequest::AddLayer {
            parent_folder_id: has_parent.then_some(parent_folder_id),
            insert_index: has_insert_index.then_some(insert_index),
            name: name.to_string(),
        };

        self.enqueue(timeline_id, command);
    }

    pub fn req_cmd_add_folder(
        &mut self,
        timeline_id: TimelineId,
        has_parent: bool,
        parent_folder_id: LayerFolderId,
        has_insert_index: bool,
        insert_index: usize,
        name: &str,
    ) {
        let command = CommandRequest::AddFolder {
            parent_folder_id: has_parent.then_some(parent_folder_id),
            insert_index: has_insert_index.then_some(insert_index),
            name: name.to_string(),
        };

        self.enqueue(timeline_id, command);
    }
}
