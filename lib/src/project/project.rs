use std::collections::{BTreeMap, HashMap};

use crate::plugin::NamespacedID;
use crate::project::change::ChangeSet;
use crate::project::clip::ClipBindingValue;
use crate::project::ids::{ClipId, IdGenerator, LayerFolderId, LayerId, TimelineId};
use crate::project::timeline::{Timeline, TimelineMeta};
use crate::project::transform::ClipTranslates;
use crate::util::result::EsotereelError;

#[derive(Debug)]
pub struct HistoryStack {
    undo_stack: Vec<String>, // 簡易実装のためコマンドの文字列表現
    redo_stack: Vec<String>,
    max_size: usize,
}

impl HistoryStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
        }
    }

    pub fn push(&mut self, command: String) {
        self.redo_stack.clear();
        
        if self.undo_stack.len() >= self.max_size {
            self.undo_stack.remove(0);
        }
        
        self.undo_stack.push(command);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> Option<String> {
        if let Some(command) = self.undo_stack.pop() {
            self.redo_stack.push(command.clone());
            Some(command)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<String> {
        if let Some(command) = self.redo_stack.pop() {
            self.undo_stack.push(command.clone());
            Some(command)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for HistoryStack {
    fn default() -> Self {
        Self::new(100) // デフォルトで100個のコマンドを保持
    }
}

#[derive(Debug, Default)]
pub struct Project {
    timelines: BTreeMap<TimelineId, Timeline>,
    ids: IdGenerator,
    history: HistoryStack,
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

    pub fn timeline_ref(&self, id: TimelineId) -> Option<&Timeline> {
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
    pub fn insert_layer_in_timeline(
        &mut self,
        timeline_id: TimelineId,
        parent: Option<LayerFolderId>,
        index: Option<usize>,
        name: String,
    ) -> anyhow::Result<LayerId> {
        let timeline = self
            .timelines
            .get_mut(&timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
        Ok(timeline.insert_layer(&mut self.ids, name, parent, index))
    }

    pub fn insert_folder_in_timeline(
        &mut self,
        timeline_id: TimelineId,
        parent: Option<LayerFolderId>,
        index: Option<usize>,
        name: String,
    ) -> anyhow::Result<LayerFolderId> {
        let timeline = self
            .timelines
            .get_mut(&timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;
        Ok(timeline.insert_folder(&mut self.ids, name, parent, index))
    }

    pub fn new_clip_in_timeline(
        &mut self,
        timeline_id: TimelineId,
        layer_id: LayerId,
        position: i64,
        duration: i64,
        kind_id: NamespacedID,
        properties: HashMap<NamespacedID, ClipBindingValue>,
        translates: ClipTranslates,
    ) -> anyhow::Result<ClipId> {
        let timeline = self
            .timelines
            .get_mut(&timeline_id)
            .ok_or(EsotereelError::TimelineNotFound(timeline_id))?;

        timeline.new_clip_in(
            layer_id,
            &mut self.ids,
            position,
            duration,
            kind_id,
            properties,
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
            .ok_or(EsotereelError::TimelineNotFound(source))?
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
