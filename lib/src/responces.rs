use std::ops::Range;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

use crate::project::{
    Clip,
    ids::{ClipId, LayerFolderId, LayerId, ResourceId, TimelineId},
    layer::LayerMeta,
    layer_outline::{Meta, OutlineNode},
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
    },
    RemoveLayer {
        timeline_id: TimelineId,
        layer_ids: Vec<LayerId>,
    },
    UpdateOutline {
        timeline_id: TimelineId,
        folders: Vec<(LayerFolderId, Meta)>,
        children: Vec<(Option<LayerFolderId>, Vec<OutlineNode>)>, // コンテナごとの並びまるごと
    },
    Removes {
        timeline_id: TimelineId,
        folder_ids: Vec<LayerFolderId>,
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
        generation: u64,
    },
    StreamDataEnd {
        resource_id: ResourceId,
        fetched_ranges: Vec<Range<f64>>,
        generation: u64,
    },
    DebugProjectStruct(Option<String>),
}
