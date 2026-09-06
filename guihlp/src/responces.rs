use std::{
    ops::Range,
    sync::{Arc, RwLock},
};

use colored::Colorize;
use esotereel_lib::{
    StreamState,
    decode::streamplayer::{FetchState, StreamPlayer},
    project::{
        Project,
        ids::{LayerId, TimelineId},
        layer::LayerMeta,
        timeline::TimelineMeta,
    },
    responces::ArchivedResponse,
    util::result::{EsotereelError, EsotereelResult},
};
use log::info;
use rkyv::Deserialize;

use crate::{mark_dirty_timeline, network::ClientNetworkHandler};

pub(super) fn on_responce_recveve(
    responce: &ArchivedResponse,
    network: &Arc<ClientNetworkHandler>,
) -> EsotereelResult<()> {
    match responce {
        ArchivedResponse::Test => {}
        ArchivedResponse::ProjectMeta { timelines } => {
            // Since TimelineMeta is simple and Copy, we can manually deserialize it
            // by accessing the archived fields directly
            let archived_timelines = timelines.as_slice();
            let timeline_metas: Vec<TimelineMeta> = archived_timelines
                .iter()
                .map(|archived| {
                    // Deserialize the inner Vec<LayerMeta>
                    let archived_layers = archived.layers.as_slice();
                    let layer_metas: Vec<esotereel_lib::project::layer::LayerMeta> =
                        archived_layers
                            .iter()
                            .map(|archived_layer| {
                                // Deserialize ArchivedOption properly
                                let parent: Option<u64> = archived_layer
                                    .parent
                                    .deserialize(&mut rkyv::Infallible)
                                    .unwrap();

                                esotereel_lib::project::layer::LayerMeta {
                                    id: archived_layer.id,
                                    name: archived_layer.name.as_str().to_string(),
                                    enabled: archived_layer.enabled,
                                    parent,
                                    children: archived_layer.children.as_slice().to_vec(),
                                    folder: archived_layer.folder,
                                }
                            })
                            .collect();

                    TimelineMeta {
                        id: archived.id,
                        fps: archived.fps,
                        root_layers: archived.root_layers.as_slice().to_vec(),
                        layers: layer_metas,
                    }
                })
                .collect();

            info!("timeline_metas: {:?}", &timeline_metas);

            let real_project = Project::from_meta(timeline_metas);
            let timeline_len = real_project.timeline_count();
            let project_arc = Arc::new(RwLock::new(real_project));

            {
                let mut app_state = network.app_state.lock().expect("mutex poisoned");
                app_state.project = Some(project_arc);
            }

            for i in 0..timeline_len {
                mark_dirty_timeline(i as TimelineId);
            }
        }
        ArchivedResponse::UpdateClip { timeline_id, clips } => {
            let project_arc = {
                let app_state = network.app_state.lock().expect("mutex poisoned");
                app_state.project.as_ref().cloned()
            };

            if let Some(project_arc) = project_arc {
                {
                    let mut project = project_arc.write().unwrap();
                    let timeline = project
                        .timeline_mut(*timeline_id)
                        .ok_or(EsotereelError::InvalidTimeline)?;

                    for (layer_id, archived_clip) in clips.iter() {
                        let clip = archived_clip.deserialize(&mut rkyv::Infallible).unwrap();
                        timeline.upsert_clip_from_network(*layer_id, clip);
                    }
                }
                // ロックを解放してからC++コールバックを呼び出す（デッドロック回避）
                drop(project_arc);
                mark_dirty_timeline(*timeline_id);
            }
        }
        ArchivedResponse::RemoveClip {
            timeline_id,
            clip_ids,
        } => {
            let project_arc = {
                let app_state = network.app_state.lock().expect("mutex poisoned");
                app_state.project.as_ref().cloned()
            };

            if let Some(project_arc) = project_arc {
                {
                    let mut project = project_arc.write().unwrap();
                    let timeline = project
                        .timeline_mut(*timeline_id)
                        .ok_or(EsotereelError::InvalidTimeline)?;

                    for (layer_id, clip_id) in clip_ids.iter() {
                        timeline.remove_clip_by_id_in(*layer_id, *clip_id);
                    }
                }
                // ロックを解放してからC++コールバックを呼び出す（デッドロック回避）
                drop(project_arc);
                mark_dirty_timeline(*timeline_id);
            }
        }
        ArchivedResponse::UpdateLayer {
            timeline_id,
            layers,
            root_layers,
        } => {
            let project_arc = {
                let app_state = network.app_state.lock().expect("mutex poisoned");
                app_state.project.as_ref().cloned()
            };

            if let Some(project_arc) = project_arc {
                {
                    let mut project = project_arc.write().unwrap();
                    let timeline = project
                        .timeline_mut(*timeline_id)
                        .ok_or(EsotereelError::InvalidTimeline)?;

                    for archived_layer in layers.iter() {
                        let parent = archived_layer
                            .parent
                            .deserialize(&mut rkyv::Infallible)
                            .unwrap();
                        let meta = LayerMeta {
                            id: archived_layer.id,
                            name: archived_layer.name.as_str().to_string(),
                            enabled: archived_layer.enabled,
                            parent,
                            children: archived_layer.children.as_slice().to_vec(),
                            folder: archived_layer.folder,
                        };
                        timeline.apply_layer_meta(meta); // upsert専用の新メソッド
                    }

                    if let Some(root) = root_layers.as_ref() {
                        let root_ids: Vec<LayerId> =
                            root.deserialize(&mut rkyv::Infallible).unwrap();
                        timeline.set_root_layers(root_ids);
                    }
                }
                // ロックを解放してからC++コールバックを呼び出す（デッドロック回避）
                drop(project_arc);
                mark_dirty_timeline(*timeline_id);
            }
        }
        ArchivedResponse::RemoveLayer {
            timeline_id,
            layer_ids,
        } => {
            let project_arc = {
                let app_state = network.app_state.lock().expect("mutex poisoned");
                app_state.project.as_ref().cloned()
            };

            if let Some(project_arc) = project_arc {
                {
                    let mut project = project_arc.write().unwrap();
                    let timeline = project
                        .timeline_mut(*timeline_id)
                        .ok_or(EsotereelError::InvalidTimeline)?;

                    for archived_id in layer_ids.iter() {
                        let id: LayerId = archived_id.deserialize(&mut rkyv::Infallible).unwrap();
                        timeline.remove_layer_local(id); // サーバー側のremove_layerと違い、
                        // parentのchildren書き換えは不要
                        // (親も別途upsertで送られてくるので)
                    }
                }
                // ロックを解放してからC++コールバックを呼び出す（デッドロック回避）
                drop(project_arc);
                mark_dirty_timeline(*timeline_id);
            }
        }

        ArchivedResponse::StreamMetadata {
            path,
            resource_id,
            codec_id,
            width,
            height,
            time_base,
            extradata,
            ..
        } => {
            let codec_id = unsafe { std::mem::transmute(*codec_id) };

            let player = StreamPlayer::new_from_metadata(
                codec_id,
                *width,
                *height,
                *time_base,
                extradata.as_slice(),
            )
            .map_err(|e| esotereel_lib::util::result::EsotereelError::IoError(e.to_string()))?;

            {
                let app_state = network.app_state.lock().expect("mutex poisoned");
                app_state.streams.insert(*resource_id, player);
                app_state
                    .path_to_stream
                    .insert(path.as_ref().to_owned(), StreamState::Loaded(*resource_id));
            }

            log::info!(
                "Player initialized for resource: {} ({})",
                resource_id,
                path
            );
        }
        ArchivedResponse::StreamData {
            resource_id,
            data,
            pts,
            dts,
            is_key,
            discontinuous,
            generation,
        } => {
            let pts: Option<i64> = pts.deserialize(&mut rkyv::Infallible).unwrap();
            let dts: Option<i64> = dts.deserialize(&mut rkyv::Infallible).unwrap();

            {
                let app_state = network.app_state.lock().expect("mutex poisoned");
                if let Some(mut player) = app_state.streams.get_mut(resource_id) {
                    // パケットをデコードしてフレームを取得
                    player
                        .process_packet(data, pts, dts, *is_key, *discontinuous, *generation)
                        .map_err(|e| EsotereelError::DecodeError(e.to_string()))?;
                }
            }
        }
        ArchivedResponse::StreamDataEnd {
            resource_id,
            fetched_ranges,
            generation: _,
        } => {
            let fetched_ranges: Vec<Range<f64>> =
                fetched_ranges.deserialize(&mut rkyv::Infallible).unwrap();

            let app_state = network.app_state.lock().expect("mutex poisoned");
            if let Some(mut player) = app_state.streams.get_mut(resource_id) {
                player.fetch_state = FetchState::Idle;
                player.free_no_needed_frames(&fetched_ranges); // Vec対応版
            }
        }
        ArchivedResponse::DebugProjectStruct(server_str) => {
            let project_arc = {
                let app_state = network.app_state.lock().expect("mutex poisoned");
                app_state.project.as_ref().cloned()
            };

            if let Some(project_arc) = project_arc {
                let project = project_arc.read().unwrap();

                let client_str = format!("{:#?}", project);

                info!("Client: {}", client_str.green());
            } else {
                info!("Client project is None");
            }

            if let Some(server_str) = server_str.as_deref() {
                info!("Server: {}", server_str.red());
            } else {
                info!("Server project is None");
            }
        }
    }
    Ok(())
}
