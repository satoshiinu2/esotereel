use std::{
    collections::{HashMap, hash_map},
    path::{Path, PathBuf},
};

use crate::{project::ids::AssetId, util::result::EsotereelError};
use uuid::Uuid;

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

    /// 外部ファイル（プロジェクト外の実パス）をアセットとして登録し、その `AssetId` を返す。
    ///
    /// 同一パスが既に登録済みなら既存の ID を返す（重複登録を避ける）。
    /// 登録は `External` として行い、ファイルの移動は `relink` で付け替える。
    pub fn get_or_add_external(&mut self, path: &Path) -> anyhow::Result<AssetId> {
        if let Some(id) = self
            .iter()
            .find(|entry| {
                matches!(
                    &entry.source,
                    AssetAvailability::Available(AssetLocation::External { path: p })
                        | AssetAvailability::Available(AssetLocation::Managed { path: p })
                        if p == path
                )
            })
            .map(|entry| entry.id)
        {
            return Ok(id);
        }

        let id = Uuid::new_v4();
        let entry = AssetEntry {
            id,
            name: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string(),
            source: AssetAvailability::Available(AssetLocation::External {
                path: path.to_path_buf(),
            }),
            project_path: None,
            fingerprint: None,
        };
        self.add(entry)
    }

    /// パスからアセットIDを逆引きする。見つからなければ `None` を返す。
    pub fn resolve_by_path(&self, path: &Path) -> Option<AssetId> {
        self.iter()
            .find(|entry| {
                matches!(
                    &entry.source,
                    AssetAvailability::Available(AssetLocation::External { path: p })
                        | AssetAvailability::Available(AssetLocation::Managed { path: p })
                        if p == path
                )
            })
            .map(|entry| entry.id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &AssetEntry> {
        self.order.iter().filter_map(move |id| self.entries.get(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_or_add_external_deduplicates_by_path() {
        let mut registry = AssetRegistry::default();
        let path = Path::new("/tmp/video.mp4");

        let id1 = registry.get_or_add_external(path).unwrap();
        let id2 = registry.get_or_add_external(path).unwrap();
        assert_eq!(id1, id2, "同一パスは同じ AssetId を返すべき");
        assert_eq!(registry.iter().count(), 1, "重複登録されない");
    }

    #[test]
    fn get_or_add_external_resolve_roundtrip() {
        let mut registry = AssetRegistry::default();
        let path = Path::new("/tmp/video.mp4");
        let id = registry.get_or_add_external(path).unwrap();

        let resolved = registry.resolve(id).unwrap();
        assert_eq!(resolved, path);
    }

    #[test]
    fn resolve_by_path_returns_none_for_unknown_path() {
        let registry = AssetRegistry::default();
        assert!(registry.resolve_by_path(Path::new("/tmp/unknown.mp4")).is_none());
    }

    #[test]
    fn resolve_by_path_returns_id_for_registered_path() {
        let mut registry = AssetRegistry::default();
        let path = Path::new("/tmp/video.mp4");
        let id = registry.get_or_add_external(path).unwrap();
        assert_eq!(registry.resolve_by_path(path), Some(id));
    }
}
