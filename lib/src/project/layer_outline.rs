use std::collections::HashMap;

use rkyv::CheckBytes;
use rkyv::bytecheck;

use crate::project::layer::LayerRemoveStrategy;
use crate::{
    project::ids::{LayerFolderId, LayerId},
    util::result::{EsotereelError, EsotereelResult},
};

#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
#[archive_attr(derive(CheckBytes))]
pub enum OutlineNode {
    Layer(LayerId),
    Folder(LayerFolderId),
}

/// Folderの中身(名前・開閉状態・並び順)。

#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Debug,
    Default,
)]
#[archive_attr(derive(CheckBytes))]
pub struct LayerFolder {
    pub name: String,
    pub children: Vec<OutlineNode>,
}

/// 表示用の並び・階層を持つ構造。
#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Debug,
    Default,
)]
#[archive_attr(derive(CheckBytes))]
pub struct LayerOutline {
    pub roots: Vec<OutlineNode>,
    folders: HashMap<LayerFolderId, LayerFolder>,
}

pub enum NodeLocation {
    Root,
    InFolder(LayerFolderId),
}

impl NodeLocation {
    pub fn to_option(&self) -> Option<LayerFolderId> {
        match self {
            NodeLocation::Root => None,
            NodeLocation::InFolder(x) => Some(*x),
        }
    }
}

pub struct ExecutionOrderIter<'a> {
    stack: Vec<std::slice::Iter<'a, OutlineNode>>,
    folders: &'a HashMap<LayerFolderId, LayerFolder>,
}

impl<'a> Iterator for ExecutionOrderIter<'a> {
    type Item = LayerId;

    fn next(&mut self) -> Option<LayerId> {
        while let Some(top) = self.stack.last_mut() {
            match top.next() {
                Some(OutlineNode::Layer(id)) => return Some(*id),
                Some(OutlineNode::Folder(fid)) => {
                    // openかどうかは見ない。UIの表示/非表示と合成順は無関係。
                    if let Some(folder) = self.folders.get(fid) {
                        self.stack.push(folder.children.iter());
                    }
                    // 壊れた参照(folders.get失敗)は無視して次のノードへ
                }
                None => {
                    self.stack.pop();
                }
            }
        }
        None
    }
}

impl LayerOutline {
    fn children_mut(&mut self, parent: Option<LayerFolderId>) -> Option<&mut Vec<OutlineNode>> {
        match parent {
            Some(fid) => self.folders.get_mut(&fid).map(|f| &mut f.children),
            None => Some(&mut self.roots),
        }
    }

    fn insert(&mut self, node: OutlineNode, parent: Option<LayerFolderId>, index: Option<usize>) {
        let siblings = self.children_mut(parent).expect("parent folder not found");
        let idx = index.unwrap_or(siblings.len()).min(siblings.len());
        siblings.insert(idx, node);
    }

    pub fn insert_layer(
        &mut self,
        id: LayerId,
        parent: Option<LayerFolderId>,
        index: Option<usize>,
    ) {
        self.insert(OutlineNode::Layer(id), parent, index);
    }

    pub fn insert_folder(
        &mut self,
        id: LayerFolderId,
        name: String,
        parent: Option<LayerFolderId>,
        index: Option<usize>,
    ) {
        self.folders.insert(
            id,
            LayerFolder {
                name,
                children: Vec::new(),
            },
        );
        self.insert(OutlineNode::Folder(id), parent, index);
    }

    /// nodeを木のどこにあっても見つけて取り除く。
    fn remove(&mut self, node: &OutlineNode) -> bool {
        if let Some(pos) = self.roots.iter().position(|n| n == node) {
            self.roots.remove(pos);
            return true;
        }
        for folder in self.folders.values_mut() {
            if let Some(pos) = folder.children.iter().position(|n| n == node) {
                folder.children.remove(pos);
                return true;
            }
        }
        false
    }

    pub fn remove_layer(&mut self, id: LayerId) -> bool {
        self.remove(&OutlineNode::Layer(id))
    }

    pub fn remove_folder(
        &mut self,
        id: LayerFolderId,
        strategy: LayerRemoveStrategy,
    ) -> Vec<OutlineNode> {
        let parent = self.parent_of(&OutlineNode::Folder(id));
        self.remove(&OutlineNode::Folder(id));
        let removed = self
            .folders
            .remove(&id)
            .map(|f| f.children)
            .unwrap_or_default();

        match strategy {
            LayerRemoveStrategy::PromoteChildren => {
                let flatten_parent = parent.map(|p| p.to_option()).flatten();
                for (_, child) in removed.iter().cloned().enumerate() {
                    self.insert(child, flatten_parent, None); // 末尾に順次挿入、siblings順は維持
                }
                Vec::new() // 何も実体削除しない
            }
            LayerRemoveStrategy::Recursive => removed, // 呼び出し側(Timeline)がこれを見てLayer実体も消す
        }
    }

    pub fn move_node(
        &mut self,
        node: OutlineNode,
        new_parent: Option<LayerFolderId>,
        index: Option<usize>,
    ) -> EsotereelResult<()> {
        // 循環防止: Folderを、自分自身 or 自分の子孫の下には移動できない
        if let (OutlineNode::Folder(moving), Some(target)) = (&node, new_parent) {
            if *moving == target || self.is_descendant_folder(target, *moving) {
                anyhow::bail!(EsotereelError::InvalidLayerMove);
            }
        }
        self.remove(&node);
        self.insert(node, new_parent, index);
        Ok(())
    }

    fn is_descendant_folder(&self, candidate: LayerFolderId, ancestor: LayerFolderId) -> bool {
        let Some(folder) = self.folders.get(&ancestor) else {
            return false;
        };
        folder.children.iter().any(|c| match c {
            OutlineNode::Folder(fid) => {
                *fid == candidate || self.is_descendant_folder(candidate, *fid)
            }
            OutlineNode::Layer(_) => false,
        })
    }

    pub fn get_folder(&self, id: LayerFolderId) -> Option<&LayerFolder> {
        self.folders.get(&id)
    }

    pub fn get_folder_mut(&mut self, id: LayerFolderId) -> Option<&mut LayerFolder> {
        self.folders.get_mut(&id)
    }

    pub fn children_of(&self, parent: Option<LayerFolderId>) -> &[OutlineNode] {
        match parent {
            Some(fid) => self
                .folders
                .get(&fid)
                .map(|f| f.children.as_slice())
                .unwrap_or(&[]),
            None => &self.roots,
        }
    }

    pub fn parent_of(&self, node: &OutlineNode) -> Option<NodeLocation> {
        if self.roots.contains(node) {
            return Some(NodeLocation::Root);
        }
        self.folders.iter().find_map(|(&fid, f)| {
            f.children
                .contains(node)
                .then_some(NodeLocation::InFolder(fid))
        })
    }

    pub fn upsert_folder_meta(&mut self, id: LayerFolderId, name: String) {
        self.folders
            .entry(id)
            .and_modify(|f| {
                f.name = name.clone();
            })
            .or_insert(LayerFolder {
                name,
                children: Vec::new(),
            });
    }

    pub fn set_children(&mut self, parent: Option<LayerFolderId>, children: Vec<OutlineNode>) {
        match parent {
            Some(fid) => {
                if let Some(f) = self.folders.get_mut(&fid) {
                    f.children = children;
                }
            }
            None => self.roots = children,
        }
    }

    pub fn remove_folder_entry_only(&mut self, id: LayerFolderId) {
        self.folders.remove(&id);
    }

    pub fn iter_execution_order(&self) -> ExecutionOrderIter<'_> {
        ExecutionOrderIter {
            stack: vec![self.roots.iter()],
            folders: &self.folders,
        }
    }
}

#[derive(
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct Meta {
    pub name: String,
}
