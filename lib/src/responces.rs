use std::ops::Range;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::project::{
    Clip,
    ids::{ClipId, LayerId, ResourceId, TimelineId},
    layer::LayerMeta,
    timeline::TimelineMeta,
};

#[derive(Archive, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Response {
    Test,
    ProjectMeta {
        timelines: Vec<TimelineMeta>,
    },
    UpdateClip {
        timeline_id: TimelineId,
        clips: Vec<(LayerId, Clip)>,
    },
    RemoveClip {
        timeline_id: TimelineId,
        clip_ids: Vec<(LayerId, ClipId)>,
    },
    UpdateLayer {
        timeline_id: TimelineId,
        layers: Vec<LayerMeta>,
        root_layers: Option<Vec<LayerId>>, // 変更があった時だけ
    },
    RemoveLayer {
        timeline_id: TimelineId,
        layer_ids: Vec<LayerId>,
    },
    StreamMetadata {
        path: String,
        resource_id: ResourceId,
        codec_id: u16,
        width: u32,
        height: u32,
        time_base: f64,
        extradata: Vec<u8>,
    },
    StreamData {
        resource_id: ResourceId,
        data: Vec<u8>,
        pts: Option<i64>,
        dts: Option<i64>,
        is_key: bool,
        discontinuous: bool,
    },
    StreamDataEnd {
        resource_id: ResourceId,
        fetched_range: Range<f64>,
    },
    DebugProjectStruct(String),
}
