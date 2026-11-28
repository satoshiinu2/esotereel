use std::collections::VecDeque;

use esotereel_lib::{
    plugin::{NamespacedID, PluginLoader, clip::component::ComponentCapability},
    project::{
        clip::ClipBindingValue,
        command::{ClipMoveCtx, ClipResizeCtx, CommandRequest},
        ids::{AssetRef, LayerFolderId, LayerId, TimelineId},
    },
    requests::{AddClipFromAsset, Request},
    util::result::{EsotereelError, LockExt},
};

use crate::network::ClientNetworkHandler;
use crate::state::ClientState;

#[derive(Debug)]
enum PendingRequest {
    Command((TimelineId, CommandRequest)),
    Request(Request),
}

#[derive(Default, Debug)]
pub struct CommandQueue {
    pending: VecDeque<PendingRequest>,
}

impl CommandQueue {
    pub fn enqueue(&mut self, timeline_id: TimelineId, command: CommandRequest) {
        self.pending
            .push_back(PendingRequest::Command((timeline_id, command)));
    }

    pub fn enqueue_request(&mut self, request: Request) {
        self.pending.push_back(PendingRequest::Request(request));
    }

    pub fn send_all(&mut self, network: &ClientNetworkHandler) {
        // キューに入れた順序を保って送る。コマンドは連続する範囲をまとめて
        // 1つの Command リクエストにし、その他のリクエストは単独で送る。
        let mut commands: VecDeque<(TimelineId, CommandRequest)> = VecDeque::new();
        while let Some(item) = self.pending.pop_front() {
            match item {
                PendingRequest::Command(command) => commands.push_back(command),
                PendingRequest::Request(request) => {
                    if !commands.is_empty() {
                        network.send(&Request::Command {
                            commands: std::mem::take(&mut commands),
                        });
                    }
                    network.send(&request);
                }
            }
        }
        if !commands.is_empty() {
            network.send(&Request::Command { commands });
        }
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
                    base_position: clip.position,
                    base_duration: clip.duration,
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

    pub fn req_cmd_clip_resize_mul(
        &mut self,
        project: &esotereel_lib::project::Project,
        timeline_id: TimelineId,
        clip_ids: &[u64],
        left_edge: bool,
        frame_delta: i64,
    ) -> anyhow::Result<()> {
        let timeline = project
            .timeline_ref(timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

        let clip_data = clip_ids
            .iter()
            .filter_map(|clip_id| {
                let (clip, _) = timeline.get_clip_and_layer(*clip_id)?;
                let (new_position, new_duration) = if left_edge {
                    (clip.position + frame_delta, clip.duration - frame_delta)
                } else {
                    (clip.position, clip.duration + frame_delta)
                };
                Some(ClipResizeCtx {
                    clip_id: *clip_id,
                    left_edge,
                    base_position: clip.position,
                    base_duration: clip.duration,
                    new_position,
                    new_duration,
                })
            })
            .collect();

        let command = CommandRequest::ClipsResize { clips: clip_data };
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
        let loader = state.plugin_loader.read_or_err()?;

        let kind_id = media_clip_kind_id(&loader)?;

        let property_schema = loader
            .get_clip_properties(&kind_id)
            .ok_or(EsotereelError::ClipKindNotFound(kind_id.clone()))?;

        let properties = ClipBindingValue::default_properties(&property_schema);
        drop(loader);

        let command = CommandRequest::AddClip {
            layer_id,
            position,
            duration: 50,
            kind_id,
            properties,
        };

        self.enqueue(timeline_id, command);

        Ok(())
    }

    pub fn req_cmd_add_clip_with_source(
        &mut self,
        state: &ClientState,
        timeline_id: TimelineId,
        position: i64,
        layer_id: LayerId,
        source_path: &str,
    ) -> anyhow::Result<()> {
        let loader = state.plugin_loader.read_or_err()?;
        let kind_id = media_clip_kind_id(&loader)?;
        drop(loader);

        // アセット参照（UUID）はクライアントが採番し、サーバーへの参照登録と
        // クライアント側レジストリへの登録を対にする。こうするとサーバーが
        // クリップに持たせる参照とクライアントがプレビュー描画で解決する参照が
        // 一致する。プロジェクトが無い場合はサーバー側でも失敗するので、
        // ここで同じエラーにする。
        let project_arc = state.project_arc();
        let mut project_guard = project_arc.write_or_err()?;
        let project = project_guard
            .as_mut()
            .ok_or(EsotereelError::ProjectNotFound)?;
        let asset_ref = project.register_asset(std::path::Path::new(source_path))?;
        drop(project_guard);

        self.enqueue_request(Request::RegisterAssetReference {
            asset_id: asset_ref.to_string(),
            path: source_path.to_string(),
            add_clip: Some(AddClipFromAsset {
                timeline_id,
                layer_id,
                position,
                duration: 50,
                kind_id,
            }),
        });

        Ok(())
    }

    /// 登録済みアセットの参照からクリップを追加します。
    ///
    /// パスはクライアント側ミラーのレジストリから ID で解決します。
    /// ミラーのパスはサーバーから同期された表のものと一致するため、
    /// サーバー側の `register_external_asset` は冪等に成功し、クリップ
    /// 追加では `clip_add_core` が同じ ID を `AssetRef` 値として確定させ
    /// ます。二重登録や ID の取り違えは起きません。Missing のアセットは
    /// 解決が失敗してここで見える失敗になります。
    pub fn req_cmd_add_clip_from_asset(
        &mut self,
        state: &ClientState,
        timeline_id: TimelineId,
        position: i64,
        layer_id: LayerId,
        asset_id: AssetRef,
    ) -> anyhow::Result<()> {
        let loader = state.plugin_loader.read_or_err()?;
        let kind_id = media_clip_kind_id(&loader)?;
        drop(loader);

        let project_arc = state.project_arc();
        let project_guard = project_arc.read_or_err()?;
        let project = project_guard
            .as_ref()
            .ok_or(EsotereelError::ProjectNotFound)?;
        let path = project.assets().resolve(asset_id)?.to_path_buf();
        drop(project_guard);

        self.enqueue_request(Request::RegisterAssetReference {
            asset_id: asset_id.to_string(),
            path: path.to_string_lossy().into_owned(),
            add_clip: Some(AddClipFromAsset {
                timeline_id,
                layer_id,
                position,
                duration: 50,
                kind_id,
            }),
        });

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

/// メディアクリップの種別 ID を、`FrameProvider` 能力を持つコンポーネントを
/// 含む種別として PluginLoader から解決します（AGENTS.md 第9条）。
fn media_clip_kind_id(loader: &PluginLoader) -> anyhow::Result<NamespacedID> {
    loader
        .find_clip_kind_by_capability(&ComponentCapability::FrameProvider)
        .ok_or_else(|| {
            anyhow::anyhow!("no clip kind has a component with the frame-provider capability")
        })
}
