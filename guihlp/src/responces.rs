use std::{
    ops::Range,
    sync::{Arc, RwLock},
};

use esotereel_lib::{
    StreamState,
    decode::streamplayer::{FetchState, StreamPlayer},
    project::{Project, ids::TimelineId, model::ProjectModel},
    responces::ArchivedResponse,
    util::{
        result::{EsotereelError, EsotereelResult},
        slot_map::SlotMapKey,
    },
};
use rkyv::{Deserialize, de::deserializers::SharedDeserializeMap};

use crate::{mark_dirty_timeline, network::ClientNetworkHandler, project::clip_apply_updates};

pub(super) fn on_responce_recveve(
    responce: &ArchivedResponse,
    network: &Arc<ClientNetworkHandler>,
) -> EsotereelResult<()> {
    let mut app_state = network.app_state.lock().expect("mutex poisoned");

    match responce {
        ArchivedResponse::Test => {}
        ArchivedResponse::ProjectAll { project } => {
            // log::info!("Client: Received ProjectAll response");

            let real_project: ProjectModel = project
                .deserialize(&mut SharedDeserializeMap::new())
                .unwrap();

            let real_project = Project::from_model(real_project);

            let timeline_len = real_project.timeline_count();
            let project_arc = Arc::new(RwLock::new(real_project));

            app_state.project = Some(project_arc.clone());

            // dbg!(real_project.clone());
            for i in 0..timeline_len {
                mark_dirty_timeline(i as TimelineId);
            }
        }
        ArchivedResponse::ClipUpdates {
            timeline_map_key,
            updates,
        } => {
            if let Some(project_arc) = app_state.project.as_ref() {
                let mut project = project_arc.write().unwrap();
                let timeline = project
                    .timeline_mut(*timeline_map_key)
                    .ok_or(EsotereelError::InvalidTimeline)?;

                clip_apply_updates(timeline, updates)?;
                mark_dirty_timeline(*timeline_map_key);
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

            app_state.streams.insert(*resource_id, player);
            app_state
                .path_to_stream
                .insert(path.as_ref().to_owned(), StreamState::Loaded(*resource_id));

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
        } => {
            let pts: Option<i64> = pts.deserialize(&mut rkyv::Infallible).unwrap();
            let dts: Option<i64> = dts.deserialize(&mut rkyv::Infallible).unwrap();

            if let Some(mut player) = app_state.streams.get_mut(resource_id) {
                // パケットをデコードしてフレームを取得
                player
                    .process_packet(data, pts, dts, *is_key, *discontinuous)
                    .map_err(|e| EsotereelError::DecodeError(e.to_string()))?;
            }
        }
        ArchivedResponse::StreamDataEnd {
            resource_id,
            fetched_range,
        } => {
            if let Some(mut player) = app_state.streams.get_mut(resource_id) {
                let fetched_range: Range<f64> =
                    fetched_range.deserialize(&mut rkyv::Infallible).unwrap();

                // fetch_state を元の状態に戻して待機
                player.fetch_state = FetchState::Idle;

                player.free_no_needed_frames(fetched_range);
            }
        }
    }
    Ok(())
}
