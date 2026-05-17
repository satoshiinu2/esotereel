use std::{ops::Range, sync::Arc};

use esotereel_lib::{
    StreamState,
    decode::streamplayer::{FetchState, StreamPlayer},
    project::Project,
    responces::ArchivedResponse,
    util::result::{EsotereelError, EsotereelResult},
};
use rkyv::{Deserialize, de::deserializers::SharedDeserializeMap};

use crate::{network::ClientNetworkHandler, project::clip_apply_updates, update_timeline};

pub(super) fn on_responce_recveve(
    responce: &ArchivedResponse,
    network: &Arc<ClientNetworkHandler>,
) -> EsotereelResult<()> {
    let app_state = network.app_state.lock().expect("mutex poisoned");

    match responce {
        ArchivedResponse::Test => {}
        ArchivedResponse::ProjectAll { project } => {
            // log::info!("Client: Received ProjectAll response");

            let mut real_project: Project = project
                .deserialize(&mut SharedDeserializeMap::new())
                .unwrap();

            real_project.rebuild_id_map()?;

            let timeline_len = real_project.get_timeline_count();
            let project_arc = Arc::new(real_project);

            *app_state.project.write().unwrap() = Some(project_arc.clone());

            // dbg!(real_project.clone());
            for i in 0..timeline_len {
                update_timeline(i);
            }
        }
        ArchivedResponse::ClipUpdates {
            timeline_type,
            updates,
        } => {
            if let Some(project) = app_state.project.write().unwrap().as_mut() {
                let project = Arc::make_mut(project);
                clip_apply_updates(project, *timeline_type as usize, updates)?;
            };
            update_timeline(*timeline_type as usize);
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
