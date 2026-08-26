// project/change.rs (新規)
use crate::project::ids::{ClipId, LayerId};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
pub struct RemovedClipInfo {
    pub layer_id: LayerId,
    pub position: i64,
    pub duration: i64,
}

/// 未同期の差分。drain_changes()で取り出されるまで蓄積され続ける。
#[derive(Debug, Default, Clone)]
pub struct ChangeSet {
    pub clips_upserted: HashSet<ClipId>,
    pub clips_removed: HashMap<ClipId, RemovedClipInfo>,

    pub layers_upserted: HashSet<LayerId>,
    pub layers_removed: HashSet<LayerId>,
    pub root_layers_changed: bool,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.is_clip_empty() && self.is_layer_empty()
    }

    pub fn is_clip_empty(&self) -> bool {
        self.clips_upserted.is_empty() && self.clips_removed.is_empty()
    }

    pub(crate) fn mark_clip_upserted(&mut self, id: ClipId) {
        self.clips_removed.remove(&id);
        self.clips_upserted.insert(id);
    }

    pub(crate) fn mark_clip_removed(&mut self, id: ClipId, info: RemovedClipInfo) {
        self.clips_upserted.remove(&id);
        self.clips_removed.insert(id, info);
    }

    pub fn is_layer_empty(&self) -> bool {
        self.layers_upserted.is_empty()
            && self.layers_removed.is_empty()
            && !self.root_layers_changed
    }

    pub(crate) fn mark_root_layers_changed(&mut self) {
        self.root_layers_changed = true;
    }

    pub(crate) fn mark_layer_upserted(&mut self, id: LayerId) {
        self.layers_removed.remove(&id);
        self.layers_upserted.insert(id);
    }

    pub(crate) fn mark_layer_removed(&mut self, id: LayerId) {
        self.layers_upserted.remove(&id);
        self.layers_removed.insert(id);
    }

    pub fn merge(&mut self, other: ChangeSet) {
        for id in other.clips_upserted {
            self.mark_clip_upserted(id);
        }
        for (id, l) in other.clips_removed {
            self.mark_clip_removed(id, l);
        }

        for id in other.layers_upserted {
            self.mark_layer_upserted(id);
        }
        for id in other.layers_removed {
            self.mark_layer_removed(id);
        }
        self.root_layers_changed |= other.root_layers_changed;
    }
}
