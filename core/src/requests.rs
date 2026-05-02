use std::collections::HashMap;
use std::sync::atomic::Ordering;

use esotereel_lib::{
    CLIENT_ALL, ServerState,
    decode::videostreamer::VideoStreamer,
    project::{ClipUpdateMap, Project, util::ProjectMutExt},
    requests::ArchivedRequest,
    responces::{Response, send_response},
    util::result::{EsotereelError, EsotereelResult, LockExt},
};

use crate::project::commands::handle_command_action;

pub fn on_request_receive(
    request: &ArchivedRequest,
    client_id: u32,
    app_state: &ServerState,
) -> EsotereelResult<()> {
    match request {
        ArchivedRequest::Test => {}
        ArchivedRequest::NewProject => {
            log::info!("Server: Handling NewProject request from client {}", client_id);
            let mut lock = app_state.project.write_or_err()?;
            let new_project = Project::new();

            // new_project.debug_add_clips(0, 0);

            let cmd = Response::ProjectAll {
                project: new_project.clone(),
            };

            *lock = Some(new_project);
            send_response(client_id, cmd);
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
            send_response(CLIENT_ALL, cmd);
        }
        ArchivedRequest::LoadStream { path } => {
            let path_str = path.as_ref();

            let streamer = VideoStreamer::new(path_str).map_err(|e| {
                EsotereelError::IoError(format!("Failed to open video stream: {:?}", e))
            })?;

            let resource_id = app_state.next_resource_id.fetch_add(1, Ordering::SeqCst);

            let codec_id = streamer.codec_id();
            let width = streamer.width();
            let height = streamer.height();
            let extradata = streamer.extradata().unwrap_or(&[]).to_vec();

            // Store the streamer in the thread-local STREAMS for later packet requests
            app_state.streams.insert(resource_id, streamer);

            let codec_id = unsafe { std::mem::transmute(codec_id) };

            let response = Response::StreamMetadata {
                resource_id,
                codec_id,
                width,
                height,
                extradata,
            };
            send_response(client_id, response);
            log::info!("Sent StreamMetadata for resource_id: {}", resource_id);
        }
        ArchivedRequest::FetchStreamData {
            resource_id,
            seek_seconds,
            count,
        } => {
            let mut is_first_packet_of_clip = true;

            let mut streamer = app_state
                .streams
                .get_mut(resource_id)
                .ok_or_else(|| EsotereelError::StreamNotFound(*resource_id))?;

            streamer
                .seek(*seek_seconds)
                .map_err(|e| EsotereelError::AccessError(e.to_string()))?;

            let video_idx = streamer.video_stream_index;
            let mut sent_count = 0;

            // Iteratorを直接進めて、指定された数(count)だけパケットを送る
            while let Some((stream, packet)) = streamer.ictx.packets().next() {
                if stream.index() == video_idx {
                    send_response(
                        client_id,
                        Response::StreamData {
                            resource_id: *resource_id,
                            data: packet.data().map(|d| d.to_vec()).unwrap_or_default(),
                            pts: packet.pts(),
                            dts: packet.dts(),
                            is_key: packet.is_key(),
                            discontinuous: is_first_packet_of_clip,
                        },
                    );

                    is_first_packet_of_clip = false;
                    sent_count += 1;
                    if sent_count >= *count {
                        break; // 指定数送ったらストップ
                    }
                }
            }
        }
    }
    Ok(())
}
