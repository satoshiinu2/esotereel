//! クリップ種別（kind）の intern テーブルです。
//!
//! 配置（`ChunkPlacement`）は `kind_id` の文字列をそのまま持たず、ここで
//! 採番した数値連番（`KindId`）だけを持ちます。保存ファイルには文字列の
//! まま書き、連番は保存しません（`AssetRef` → `ResourceId` と同じ二層化）。
//!
//! テーブルはセッション内 append-only で、一度採番した番号は kind が
//! 使われなくなっても再利用しません（欠番になります）。

use std::collections::HashMap;

use rkyv::{Archive, CheckBytes, bytecheck};

use crate::plugin::NamespacedID;

/// 実行時のクリップ種別 ID（数値連番）です。セッション内でのみ安定します。
pub type KindId = u32;

/// 文字列の `kind_id` と `KindId` の相互変換テーブルです。
#[derive(
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Default,
)]
#[archive_attr(derive(CheckBytes))]
pub struct KindTable {
    /// 連番順の kind 一覧。インデックスがそのまま `KindId` です。
    order: Vec<NamespacedID>,
    /// kind → 連番の逆引きです。
    index: HashMap<NamespacedID, KindId>,
}

impl KindTable {
    /// kind を intern し、連番を返します。
    ///
    /// 未登録なら末尾に採番し、登録済みなら既存の番号を返します。
    /// 番号が再利用されることはありません。
    pub fn intern(&mut self, kind_id: &NamespacedID) -> KindId {
        if let Some(&id) = self.index.get(kind_id) {
            return id;
        }
        let id = self.order.len() as KindId;
        self.order.push(kind_id.clone());
        self.index.insert(kind_id.clone(), id);
        id
    }

    /// 連番から kind を引きます。未採番なら `None`。
    pub fn resolve(&self, id: KindId) -> Option<&NamespacedID> {
        self.order.get(id as usize)
    }

    /// kind から連番を引きます。未登録なら `None`（採番はしません）。
    pub fn lookup(&self, kind_id: &NamespacedID) -> Option<KindId> {
        self.index.get(kind_id).copied()
    }

    /// 採番済みの kind 数を返します。
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// 採番済みの kind が無いかどうかを返します。
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}
