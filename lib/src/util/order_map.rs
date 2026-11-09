use std::collections::BTreeMap;

use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

/// 順序付きマップです。
///
/// `BTreeMap<i64, V>` を内部に持ち、要素に「順序」を割り当てて並べ替えを管理します。
/// 項目の挿入・移動時に中間値を計算して順序を付け、ギャップが不足した場合は
/// 全体を再バランスします。
///
/// 主にレイヤーやクリップの表示順序管理に使用されます。
#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct OrderMap<V>
where
    V: PartialEq,
{
    pub layout: BTreeMap<i64, V>,
}

impl<V> OrderMap<V>
where
    V: PartialEq,
{
    /// 新しい空の `OrderMap` を作成します。
    pub fn new() -> Self {
        Self {
            layout: BTreeMap::new(),
        }
    }

    /// 指定値の順序を返します。見つからなければ `None`。
    pub fn get_order_of(&self, value: &V) -> Option<i64> {
        self.layout
            .iter()
            .find(|(_, v)| *v == value)
            .map(|(&order, _)| order)
    }

    /// 値を末尾に追加します。
    pub fn add_last(&mut self, value: V) {
        let order = self.next_order();
        self.layout.insert(order, value);
    }

    /// 対象値を、前後の値の間に移動します。
    ///
    /// 対象が既に存在する場合はまず削除し、前後の順序の中間に再挿入します。
    /// ギャップが不足している場合は全体を再バランスしてから挿入します。
    pub fn move_at(&mut self, target: V, prev: &V, next: &V) {
        // すでに存在するば場合消す
        let current_target_order = self.get_order_of(&target);
        if let Some(order) = current_target_order {
            self.layout.remove(&order);
        }

        // 1. Get the current orders of the previous and next items.
        let mut prev_order = self.get_order_of(&prev).unwrap_or(0);
        let mut next_order = self.get_order_of(&next).unwrap_or(i64::MAX);

        // 2. If there's no sufficient gap, rebalance the entire map.
        if !self.has_gap(prev_order, next_order) {
            self.rebalance();
            // IMPORTANT: After rebalancing, all order keys have changed.
            // We must re-fetch the orders for prev_id and next_id.
            prev_order = self.get_order_of(&prev).unwrap_or(0);
            next_order = self.get_order_of(&next).unwrap_or(i64::MAX);
        }

        // 3. Calculate a new intermediate order for the target_id.
        let new_order = self.mid_order(prev_order, next_order);

        // 4. Insert the target_id at its new order.
        self.layout.insert(new_order, target);
    }

    /// 指定順序の値への参照を返します。
    pub fn get(&self, order: i64) -> Option<&V> {
        self.layout.get(&order)
    }

    /// 値へのイテレータを返します。
    pub fn iter(&self) -> std::collections::btree_map::Values<'_, i64, V> {
        self.layout.values()
    }

    /// 可変な値へのイテレータを返します。
    pub fn iter_mut(&mut self) -> std::collections::btree_map::ValuesMut<'_, i64, V> {
        self.layout.values_mut()
    }

    fn next_order(&self) -> i64 {
        const GAP: i64 = 1i64 << 32;
        self.layout
            .keys()
            .last()
            .map(|&last| last + GAP)
            .unwrap_or(0)
    }

    fn mid_order(&self, prev: i64, next: i64) -> i64 {
        prev + (next - prev) / 2
    }

    fn has_gap(&self, prev: i64, next: i64) -> bool {
        next - prev > 1
    }

    fn rebalance(&mut self) {
        const GAP: i64 = 1i64 << 32;
        let old_layout = std::mem::take(&mut self.layout);
        self.layout = old_layout
            .into_values()
            .enumerate()
            .map(|(i, v)| ((i as i64) * GAP, v))
            .collect();
    }
}

impl<V> IntoIterator for OrderMap<V>
where
    V: PartialEq,
{
    type Item = V;
    type IntoIter = std::collections::btree_map::IntoValues<i64, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.layout.into_values()
    }
}

impl<'a, V> IntoIterator for &'a OrderMap<V>
where
    V: PartialEq,
{
    type Item = &'a V;
    type IntoIter = std::collections::btree_map::Values<'a, i64, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.layout.values()
    }
}

impl<'a, V> IntoIterator for &'a mut OrderMap<V>
where
    V: PartialEq,
{
    type Item = &'a mut V;
    type IntoIter = std::collections::btree_map::ValuesMut<'a, i64, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.layout.values_mut()
    }
}
