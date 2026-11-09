use std::{
    collections::{HashMap, hash_map},
    path::{Path, PathBuf},
};

use crate::{project::ids::AssetId, util::result::EsotereelError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetFingerprint {
    pub size: u64,
    pub hash: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetLocation {
    External { path: PathBuf },
    Managed { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetAvailability {
    Available(AssetLocation),
    Missing { previous_path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct AssetEntry {
    pub id: AssetId,
    pub name: String,

    /// 実ファイルの現在の参照先
    pub source: AssetAvailability,

    /// プロジェクト内での論理パス
    pub project_path: Option<PathBuf>,

    /// ファイルの同一性確認用
    pub fingerprint: Option<AssetFingerprint>,
}

#[derive(Debug, Default)]
pub struct AssetRegistry {
    entries: HashMap<AssetId, AssetEntry>,

    /// UI 表示順
    order: Vec<AssetId>,
}

impl AssetRegistry {
    pub fn get(&self, id: AssetId) -> Option<&AssetEntry> {
        self.entries.get(&id)
    }

    pub fn add(&mut self, entry: AssetEntry) -> anyhow::Result<AssetId> {
        let id = entry.id;

        match self.entries.entry(id) {
            hash_map::Entry::Occupied(_) => Err(EsotereelError::AssetAlreadyExists(id).into()),
            hash_map::Entry::Vacant(vacant) => {
                vacant.insert(entry);
                self.order.push(id);
                Ok(id)
            }
        }
    }

    pub fn remove(&mut self, id: AssetId) -> anyhow::Result<AssetEntry> {
        let entry = self
            .entries
            .remove(&id)
            .ok_or_else(|| EsotereelError::AssetNotFound(id))?;

        self.order.retain(|&ordered_id| ordered_id != id);

        Ok(entry)
    }

    pub fn relink(&mut self, id: AssetId, new_location: AssetLocation) -> anyhow::Result<()> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or_else(|| EsotereelError::AssetNotFound(id))?;
        entry.source = AssetAvailability::Available(new_location);
        Ok(())
    }

    pub fn resolve(&self, id: AssetId) -> anyhow::Result<&Path> {
        let entry = self
            .entries
            .get(&id)
            .ok_or_else(|| EsotereelError::AssetNotFound(id))?;
        match &entry.source {
            AssetAvailability::Available(AssetLocation::External { path })
            | AssetAvailability::Available(AssetLocation::Managed { path }) => Ok(path.as_path()),
            AssetAvailability::Missing { previous_path } => Err(anyhow::anyhow!(
                "Asset is missing, previous path: {:?}",
                previous_path
            )),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &AssetEntry> {
        self.order.iter().filter_map(move |id| self.entries.get(id))
    }
}
