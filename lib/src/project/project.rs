use std::collections::BTreeMap;

use crate::project::change::ChangeSet;
use crate::project::clip::ClipData;
use crate::project::ids::{ClipId, IdGenerator, LayerId, TimelineId};
use crate::project::layer::Layer;
use crate::project::timeline::{Timeline, TimelineMeta};
use crate::project::transform::ClipTranslates;
use crate::util::result::EsotereelError;

#[derive(Debug, Default)]
pub struct Project {
    timelines: BTreeMap<TimelineId, Timeline>,
    ids: IdGenerator,
}

impl Project {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_timeline(&mut self, fps: f64) -> TimelineId {
        let id = self.ids.next_timeline_id();
        self.timelines
            .insert(id, Timeline::new(id, fps, &mut self.ids));
        id
    }

    pub fn timeline(&self, id: TimelineId) -> Option<&Timeline> {
        self.timelines.get(&id)
    }

    pub fn timeline_mut(&mut self, id: TimelineId) -> Option<&mut Timeline> {
        self.timelines.get_mut(&id)
    }

    pub fn timeline_count(&self) -> usize {
        self.timelines.len()
    }

    pub fn id_generator_mut(&mut self) -> &mut IdGenerator {
        &mut self.ids
    }

    /// 指定Timelineにレイヤー(またはフォルダー)を新規挿入する。
    /// parent が Some の場合はそのレイヤーの子として、None の場合はroot_layers直下に追加する。
    /// index を省略すると末尾に追加される。
    /// 注意: IdGenerator に next_layer_id が無ければ追加が必要。
    pub fn insert_layer_in_timeline(
        &mut self,
        timeline_id: TimelineId,
        parent: Option<LayerId>,
        index: Option<usize>,
        name: String,
        is_folder: bool,
    ) -> anyhow::Result<LayerId> {
        let timeline = self
            .timelines
            .get_mut(&timeline_id)
            .ok_or(EsotereelError::InvalidTimeline)?;

        let id = self.ids.next_layer_id();
        let layer = if is_folder {
            Layer::new_folder(id, name)
        } else {
            Layer::new(id, name)
        };

        timeline.insert_layer(layer, parent, index)?;
        Ok(id)
    }

    pub fn new_clip_in_timeline(
        &mut self,
        timeline_id: TimelineId,
        layer_id: LayerId,
        position: i64,
        duration: i64,
        clip_data: ClipData,
        translates: ClipTranslates,
    ) -> anyhow::Result<ClipId> {
        let timeline = self
            .timelines
            .get_mut(&timeline_id)
            .ok_or(EsotereelError::InvalidTimeline)?;

        timeline.new_clip_in(
            layer_id,
            &mut self.ids,
            position,
            duration,
            clip_data,
            translates,
        )
    }

    /// Mirror参照されているTimelineを独立コピーし、新しいTimelineIdを返す。
    /// 呼び出し側でClipのCompositionRefをIndependent(new_id)に差し替えること。
    pub fn make_independent(&mut self, source: TimelineId) -> anyhow::Result<TimelineId> {
        let new_id = self.ids.next_timeline_id();
        let cloned = self
            .timelines
            .get(&source)
            .ok_or(EsotereelError::InvalidTimeline)?
            .deep_clone(&mut self.ids, new_id);
        self.timelines.insert(new_id, cloned);
        Ok(new_id)
    }

    /// 対応するClipが存在しなくなったIndependent Timelineの掃除。
    /// 厳密な参照カウントは今はせず、呼び出し側(Clip削除時)から明示的に呼ぶ。
    pub fn remove_timeline(&mut self, id: TimelineId) -> Option<Timeline> {
        self.timelines.remove(&id)
    }

    pub fn drain_changes(&mut self) -> Vec<(TimelineId, ChangeSet)> {
        self.timelines
            .iter_mut()
            .filter_map(|(&id, tl)| {
                let cs = tl.drain_changes();
                (!cs.is_empty()).then_some((id, cs))
            })
            .collect()
    }

    // TODO: clip -> nested_timeline_id の逆引きマップ
    pub fn propagate_nested_dirty(&mut self, changed: &[(TimelineId, ChangeSet)]) {
        let changed_ids: std::collections::HashSet<TimelineId> =
            changed.iter().map(|(id, _)| *id).collect();

        // 全timeline全clipを舐めて、nested_timeline_idがchanged_idsに含まれるものを探す
        let mut to_touch: Vec<(TimelineId, ClipId)> = Vec::new();
        for (&tid, tl) in self.timelines.iter() {
            for (&cid, clip) in tl.iter_clips() {
                // ※iter_clipsは公開メソッドとして追加要
                if let Some(nested) = clip.data.nested_timeline_id() {
                    if changed_ids.contains(&nested) {
                        to_touch.push((tid, cid));
                    }
                }
            }
        }
        for (tid, cid) in to_touch {
            if let Some(tl) = self.timelines.get_mut(&tid) {
                tl.touch_upsert(cid); // touch_upsertをpub(crate)にして呼ぶ
            }
        }
    }

    /// ProjectAll用の軽量メタ情報。Clip本体を含まない。
    pub fn timelines_meta(&self) -> Vec<TimelineMeta> {
        self.timelines.values().map(TimelineMeta::from).collect()
    }

    pub fn from_meta(timelines: Vec<TimelineMeta>) -> Self {
        let mut project = Self::new();
        for meta in timelines {
            let timeline_id = meta.id;
            project
                .timelines
                .insert(timeline_id, Timeline::from_meta(&meta));
            project.ids.observe_timeline(timeline_id);
            for lm in &meta.layers {
                project.ids.observe_layer(lm.id);
            }
        }
        project
    }
}
