use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};
use std::{fmt::Debug, marker::PhantomData};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct Slot<T> {
    data: Option<T>,
    generation: u32, // 削除・再利用を検知する
}

pub trait SlotMapKey {
    fn new(index: usize, generation: u32) -> Self;
    fn index(&self) -> usize;
    fn generation(&self) -> u32;
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct SlotMap<K, V> {
    slots: Vec<Slot<V>>,
    free_indices: Vec<usize>, // 空いているインデックスをメモしておく
    _marker: PhantomData<K>,
}

impl<K, V> Default for SlotMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> SlotMap<K, V> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_indices: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn insert(&mut self, val: V) -> K
    where
        K: SlotMapKey,
    {
        if let Some(idx) = self.free_indices.pop() {
            // 1. 空き地を再利用
            let slot = &mut self.slots[idx as usize];
            slot.data = Some(val);
            slot.generation += 1; // 世代を上げる

            K::new(idx, slot.generation)
        } else {
            // 2. 新しく末尾に追加
            let idx = self.slots.len();
            self.slots.push(Slot {
                data: Some(val),
                generation: 1,
            });

            K::new(idx, 1)
        }
    }

    pub fn get(&self, key: K) -> Option<&V>
    where
        K: SlotMapKey,
    {
        let slot = self.slots.get(key.index())?;
        // 世代が一致しているかチェック（これが SlotMap の肝）
        if slot.generation == key.generation() {
            slot.data.as_ref()
        } else {
            None // すでに消されて別のデータになっているか、空っぽ
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &V> {
        self.slots.iter().filter_map(|s| s.data.as_ref())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.slots.iter_mut().filter_map(|s| s.data.as_mut())
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
