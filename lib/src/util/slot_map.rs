use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};
use std::fmt::Debug;

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Slot<T> {
    data: Option<T>,
    generation: u32, // 削除・再利用を検知する
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Eq, Hash)]
#[archive_attr(derive(CheckBytes, Eq, Hash))]
#[repr(C)]
pub struct SlotMapKey {
    pub index: usize,
    generation: u32,
}

impl SlotMapKey {
    pub fn new(index: usize, generation: u32) -> Self {
        Self { index, generation }
    }
}
impl PartialEq for SlotMapKey {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl PartialEq for ArchivedSlotMapKey {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct SlotMap<V> {
    slots: Vec<Slot<V>>,
    free_indices: Vec<usize>, // 空いているインデックスをメモしておく
}

impl<V> Default for SlotMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> SlotMap<V> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_indices: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len() - self.free_indices.len()
    }

    pub fn get_cureent_new_key(&self, idx: usize) -> SlotMapKey {
        let slot = &self.slots[idx];

        SlotMapKey::new(idx, slot.generation)
    }

    pub fn insert(&mut self, val: V) -> SlotMapKey {
        if let Some(idx) = self.free_indices.pop() {
            // 1. 空き地を再利用
            let slot = &mut self.slots[idx];
            slot.data = Some(val);
            slot.generation += 1; // 世代を上げる

            SlotMapKey::new(idx, slot.generation)
        } else {
            // 2. 新しく末尾に追加
            let idx = self.slots.len();
            self.slots.push(Slot {
                data: Some(val),
                generation: 1,
            });

            SlotMapKey::new(idx, 1)
        }
    }

    pub fn get(&self, key: &SlotMapKey) -> Option<&V> {
        let slot = self.slots.get(key.index)?;
        // 世代が一致しているかチェック（これが SlotMap の肝）
        if slot.generation == key.generation {
            slot.data.as_ref()
        } else {
            None // すでに消されて別のデータになっているか、空っぽ
        }
    }

    pub fn get_mut(&mut self, key: &SlotMapKey) -> Option<&mut V> {
        let slot = self.slots.get_mut(key.index)?;
        // 世代が一致しているかチェック（これが SlotMap の肝）
        if slot.generation == key.generation {
            slot.data.as_mut()
        } else {
            None // すでに消されて別のデータになっているか、空っぽ
        }
    }

    pub fn iter(&self) -> Iter<'_, V> {
        Iter {
            inner: self.slots.iter(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, V> {
        IterMut {
            inner: self.slots.iter_mut(),
        }
    }

    pub fn into_iter(self) -> IntoIter<V> {
        IntoIter {
            inner: self.slots.into_iter(),
        }
    }

    pub fn iter_with_key(&self) -> impl Iterator<Item = (SlotMapKey, &V)> {
        self.slots.iter().enumerate().filter_map(|(idx, slot)| {
            slot.data
                .as_ref()
                .map(|value| (SlotMapKey::new(idx, slot.generation), value))
        })
    }

    pub fn get_layers_sorted_mut(&mut self) -> Vec<&mut V>
    where
        V: Ord,
    {
        let mut valid_data: Vec<&mut V> = self
            .slots
            .iter_mut()
            .filter_map(|s| s.data.as_mut())
            .collect();

        valid_data.sort();

        valid_data
    }
}

// iterators
pub struct Iter<'a, V> {
    inner: std::slice::Iter<'a, Slot<V>>,
}

impl<'a, V> Iterator for Iter<'a, V> {
    type Item = &'a V;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.by_ref().find_map(|s| s.data.as_ref())
    }
}

pub struct IterMut<'a, V> {
    inner: std::slice::IterMut<'a, Slot<V>>,
}

impl<'a, V> Iterator for IterMut<'a, V> {
    type Item = &'a mut V;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.by_ref().find_map(|s| s.data.as_mut())
    }
}

pub struct IntoIter<V> {
    inner: std::vec::IntoIter<Slot<V>>,
}

impl<V> Iterator for IntoIter<V> {
    type Item = V;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.by_ref().find_map(|s| s.data)
    }
}
