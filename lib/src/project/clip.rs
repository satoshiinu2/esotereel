use rkyv::{
    Archive, CheckBytes, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize, bytecheck,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::project::{
    TimelineTick,
    ids::{ClipId, ScriptId, TimelineId},
    transform::ClipTranslates,
};

#[derive(Archive, RkyvDeserialize, RkyvSerialize, Serialize, Deserialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Clip {
    pub id: ClipId,
    pub position: TimelineTick,
    pub duration: TimelineTick,
    pub data: ClipData,
    pub translates: ClipTranslates,
}

impl Clip {
    pub fn new(
        id: ClipId,
        position: TimelineTick,
        duration: TimelineTick,
        clip_data: ClipData,
        translates: ClipTranslates,
    ) -> Self {
        Self {
            id,
            position,
            duration,
            data: clip_data,
            translates,
        }
    }

    pub fn position(&self) -> i64 {
        self.position
    }

    pub fn set_position(&mut self, new_pos: i64) {
        self.position = new_pos;
    }
}

/// Compositionへの参照。Mirrorは複数Clipが同じTimelineIdを共有し、
/// Independentはこのclip専用のprivate Timelineを指す。
#[derive(
    Archive,
    RkyvDeserialize,
    RkyvSerialize,
    Serialize,
    Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[archive_attr(derive(CheckBytes))]
pub enum CompositionRef {
    Mirror(TimelineId),
    Independent(TimelineId),
}

impl CompositionRef {
    pub fn timeline_id(&self) -> TimelineId {
        match self {
            CompositionRef::Mirror(id) | CompositionRef::Independent(id) => *id,
        }
    }
}

/// Scriptに渡すパラメータ。今はプレースホルダー。
/// rhai等のスクリプトエンジンと繋ぐ際に型を差し替える想定。
#[derive(
    Archive, RkyvDeserialize, RkyvSerialize, Serialize, Deserialize, Debug, Clone, Default,
)]
#[archive_attr(derive(CheckBytes))]
pub struct ScriptParams(pub BTreeMap<String, String>);

#[derive(Archive, RkyvDeserialize, RkyvSerialize, Serialize, Deserialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub enum ClipData {
    Dummy,
    Video {
        path: String,
        media_offset: f64,
    },
    Audio {
        path: String,
        media_offset: f64,
    },
    Composite {
        source: CompositionRef,
    },
    Area2D {
        source: CompositionRef,
    },
    Area3D {
        source: CompositionRef,
    },
    /// スクリプトが生成した構造の出力先Timeline。
    /// 初回評価前はNone、評価後にTimelineIdが入る。
    Script {
        script_id: ScriptId,
        params: ScriptParams,
        generated: Option<TimelineId>,
    },
}

impl ClipData {
    pub fn get_media_seconds(
        global_tps: f64,
        clip_position: i64,
        current_frame: i64,
        media_offset: f64,
    ) -> f64 {
        let relative_frame = current_frame - clip_position;
        if relative_frame < 0 {
            return media_offset;
        }
        (relative_frame as f64 / global_tps) + media_offset
    }

    /// このClipが下位Timelineを参照している場合、そのidを返す。
    /// Composite/Area2D/Area3D/Script(生成済み)すべてで共通に使える。
    pub fn nested_timeline_id(&self) -> Option<TimelineId> {
        match self {
            ClipData::Composite { source }
            | ClipData::Area2D { source }
            | ClipData::Area3D { source } => Some(source.timeline_id()),
            ClipData::Script { generated, .. } => *generated,
            _ => None,
        }
    }
}

impl PartialEq for Clip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Clip {}

impl PartialEq for ArchivedClip {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for ArchivedClip {}
