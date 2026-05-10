use std::sync::atomic::Ordering;
use std::{collections::HashMap, sync::Arc};

use esotereel_lib::{
    StreamState,
    decode::videostreamer::VideoStreamer,
    project::{ClipUpdateMap, Project, util::ProjectMutExt},
    requests::ArchivedRequest,
    responces::Response,
    util::result::{EsotereelError, EsotereelResult, LockExt},
};

use crate::{network::ServerNetworkHandler, project::commands::handle_command_action};

pub fn on_request_receive(
    request: &ArchivedRequest,
    client_id: u32,
    network: &Arc<ServerNetworkHandler>,
) -> EsotereelResult<()> {
    let app_state = &network.app_state;

    match request {
        ArchivedRequest::Test => {}
        ArchivedRequest::NewProject => {
            log::info!(
                "Server: Handling NewProject request from client {}",
                client_id
            );
            let mut lock = app_state.project.write_or_err()?;
            let new_project = Project::new();

            // new_project.debug_add_clips(0, 0);

            let cmd = Response::ProjectAll {
                project: new_project.clone(),
            };

            *lock = Some(new_project);
            network.send(client_id, &cmd);
        }
        ArchivedRequest::ProjectAll => {
            let mut lock = app_state.project.write_or_err()?;
            let project = lock.project_mut_err()?;

            let cmd = Response::ProjectAll {
                project: project.clone(),
            };

            network.send(client_id, &cmd);
        }
        ArchivedRequest::Command {
            command,
            timeline_idx,
        } => {
            let timeline_idx = *timeline_idx as usize;

            let mut lock = app_state.project.write_or_err()?;
            let project = lock.project_mut_err()?;

            let mut updates: Option<ClipUpdateMap> = Some(HashMap::new());
            let timeline = project.get_timeline_mut(timeline_idx)?;

            handle_command_action(command, timeline, &mut updates)?;

            // send updates
            let updates = updates.unwrap_or_default();
            let cmd = Response::ClipUpdates {
                timeline_type: timeline_idx,
                updates,
            };
            network.send_all(&cmd);
        }
        ArchivedRequest::LoadStream { path } => {
            let path_str = path.as_ref();

            let streamer = VideoStreamer::new(path_str).map_err(|e| {
                EsotereelError::IoError(format!("Failed to open video stream: {:?}", e))
            })?;

            let resource_id = app_state
                .path_to_stream
                .get(path_str)
                .and_then(|s| s.as_option())
                .unwrap_or_else(|| {
                    network
                        .app_state
                        .next_resource_id
                        .fetch_add(1, Ordering::SeqCst)
                });

            let codec_id = streamer.codec_id();
            let width = streamer.width();
            let height = streamer.height();
            let extradata = streamer.extradata().unwrap_or(&[]).to_vec();
            let time_base = streamer.time_base;

            // Store the streamer in the thread-local STREAMS for later packet requests
            app_state.streams.insert(resource_id, streamer);
            app_state
                .path_to_stream
                .insert(path_str.to_owned(), StreamState::Loaded(resource_id));

            let codec_id = unsafe { std::mem::transmute(codec_id) };

            let res = Response::StreamMetadata {
                path: path_str.to_owned(),
                resource_id,
                codec_id,
                width,
                height,
                time_base,
                extradata,
            };
            network.send(client_id, &res);

            log::info!(
                "Sent StreamMetadata for resource_id: {} ({})",
                resource_id,
                path
            );
        }
        ArchivedRequest::FetchStreamData {
            resource_id,
            seek_seconds,
            count,
        } => {
            log::info!(
                "Received FetchStreamData request for resource_id: {} at:{}",
                resource_id,
                seek_seconds
            );
            let mut is_first_packet_of_clip = true;

            let mut streamer = app_state
                .streams
                .get_mut(resource_id)
                .ok_or_else(|| EsotereelError::StreamNotFound(*resource_id))?;

            streamer
                .seek(*seek_seconds)
                .map_err(|e| EsotereelError::AccessError(e.to_string()))?;

            let video_idx = streamer.video_stream_index;
            let time_base = streamer.time_base;
            let mut sent_count = 0;
            let mut reached_target_time = false;
            let mut packets_after_target = 0;
            let mut packets = streamer.ictx.packets();

            // Iteratorを直接進めて、指定された数(count)だけパケットを送る
            while let Some((stream, packet)) = packets.next() {
                if stream.index() == video_idx {
                    let data = packet.data().map(|d| d.to_vec()).unwrap_or_default();
                    let pts = packet.pts();
                    let dts = packet.dts();
                    // パケットのPTSを秒数に変換
                    let packet_time = pts.map(|p| p as f64 * time_base).unwrap_or(0.0);

                    let res = Response::StreamData {
                        resource_id: *resource_id,
                        data,
                        pts,
                        dts,
                        is_key: packet.is_key(),
                        discontinuous: is_first_packet_of_clip,
                    };

                    network.send(client_id, &res);

                    is_first_packet_of_clip = false;
                    sent_count += 1;

                    if packet_time >= *seek_seconds {
                        reached_target_time = true;
                    }

                    if reached_target_time {
                        packets_after_target += 1;
                    }

                    // リクエストされた秒数に到達するまで、あるいは最低でも count 個送る
                    // 際限なく送るのを防ぐため、安全策として200パケット程度で打ち切る
                    if (reached_target_time && packets_after_target >= *count) || sent_count >= 500
                    {
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}
