use esotereel_lib::{
    ClientState, decode::videoreciever::StreamReciever, project::Project,
    responces::ArchivedResponse, util::result::EsotereelResult,
};
use rkyv::{Deserialize, de::deserializers::SharedDeserializeMap};

use crate::{project::clip_apply_updates, update_stream_frame, update_timeline};

pub(super) fn on_responce_recveve(
    responce: &ArchivedResponse,
    app_state: &ClientState,
) -> EsotereelResult<()> {
    match responce {
        ArchivedResponse::Test => {}
        ArchivedResponse::ProjectAll { project } => {
            log::info!("Client: Received ProjectAll response");

            let mut real_project: Project = project
                .deserialize(&mut SharedDeserializeMap::new())
                .unwrap();

            real_project.rebuild_id_map()?;

            let timeline_len = real_project.get_timeline_count();

            // dbg!(real_project.clone());
            *app_state.project.write().unwrap() = Some(real_project);
            for i in 0..timeline_len {
                update_timeline(i);
            }
        }
        ArchivedResponse::ClipUpdates {
            timeline_type,
            updates,
        } => {
            if let Some(project) = app_state.project.write().unwrap().as_mut() {
                clip_apply_updates(project, *timeline_type as usize, updates)?;
            };
            update_timeline(*timeline_type as usize);
        }
        ArchivedResponse::StreamMetadata {
            resource_id,
            codec_id,
            width,
            height,
            extradata,
            ..
        } => {
            let codec_id = unsafe { std::mem::transmute(*codec_id) };

            let player =
                StreamReciever::new_from_metadata(codec_id, *width, *height, extradata.as_slice())
                    .map_err(|e| {
                        esotereel_lib::util::result::EsotereelError::IoError(e.to_string())
                    })?;

            app_state.streams.insert(*resource_id, player);
            log::info!("Player initialized for resource: {}", resource_id);
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
                if *discontinuous {
                    player.flush();
                }

                // パケットをデコードしてフレームを取得
                if let Some(frame) = player.process_packet(data, pts, dts, *is_key) {
                    // GUI側のコールバックを呼び出して描画を依頼
                    update_stream_frame(
                        *resource_id,
                        frame.width(),
                        frame.height(),
                        frame.data(0).as_ptr(),
                    );
                }
            }
        }
    }
    Ok(())
}
