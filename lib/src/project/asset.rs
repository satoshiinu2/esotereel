use std::{
    collections::{HashMap, hash_map},
    path::{Path, PathBuf},
};

use crate::{project::ids::AssetRef, util::result::EsotereelError};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AssetFingerprint {
    pub size: u64,
    pub hash: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AssetLocation {
    External { path: PathBuf },
    Managed { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AssetAvailability {
    Available(AssetLocation),
    Missing { previous_path: PathBuf },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssetEntry {
    pub id: AssetRef,
    pub name: String,

    /// 実ファイルの現在の参照先
    pub source: AssetAvailability,

    /// ファイルの同一性確認用
    pub fingerprint: Option<AssetFingerprint>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AssetRegistry {
    entries: HashMap<AssetRef, AssetEntry>,

    /// UI 表示順
    order: Vec<AssetRef>,
}

impl AssetRegistry {
    pub fn get(&self, id: AssetRef) -> Option<&AssetEntry> {
        self.entries.get(&id)
    }

    /// 構築済みのエントリをそのまま登録します。
    ///
    /// 公開の登録経路（`add_external` / `add_managed`）では作れない
    /// エントリ（例: `Missing`）の復元・取り込み用です。
    pub(crate) fn add(&mut self, entry: AssetEntry) -> anyhow::Result<AssetRef> {
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

    /// 指定 ID で外部ファイル参照を登録します。
    pub fn add_external(&mut self, id: AssetRef, path: &Path) -> anyhow::Result<AssetRef> {
        self.add(AssetEntry {
            id,
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string(),
            source: AssetAvailability::Available(AssetLocation::External {
                path: path.to_path_buf(),
            }),
            fingerprint: fingerprint_for(path),
        })
    }

    /// 指定 ID でプロジェクト管理下のファイルを登録します。
    pub fn add_managed(
        &mut self,
        id: AssetRef,
        name: String,
        path: PathBuf,
    ) -> anyhow::Result<AssetRef> {
        let fingerprint = fingerprint_for(&path);
        self.add(AssetEntry {
            id,
            name,
            source: AssetAvailability::Available(AssetLocation::Managed { path }),
            fingerprint,
        })
    }

    pub fn remove(&mut self, id: AssetRef) -> anyhow::Result<AssetEntry> {
        let entry = self
            .entries
            .remove(&id)
            .ok_or_else(|| EsotereelError::AssetNotFound(id))?;

        self.order.retain(|&ordered_id| ordered_id != id);

        Ok(entry)
    }

    pub fn relink(&mut self, id: AssetRef, new_location: AssetLocation) -> anyhow::Result<()> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or_else(|| EsotereelError::AssetNotFound(id))?;
        entry.source = AssetAvailability::Available(new_location);
        Ok(())
    }

    /// fingerprint の hash がまだ無いエントリについて、ファイルを
    /// 読んで BLAKE3 ハッシュを計算し、埋めます。
    ///
    /// External アセットのバックグラウンドでの後埋め用です。呼び出し側
    /// が同時実行数・優先度・再生中の後回しを制御します（この関数は
    /// 同期的に1ファイル分だけ計算します）。
    /// hash が既に入っている場合は何もせず `false` を返します。
    /// Missing のエントリはファイルが無いためエラーになります。
    pub fn fill_fingerprint_hash(&mut self, id: AssetRef) -> anyhow::Result<bool> {
        let entry = self
            .entries
            .get(&id)
            .ok_or_else(|| EsotereelError::AssetNotFound(id))?;
        if entry.fingerprint.as_ref().and_then(|f| f.hash).is_some() {
            return Ok(false);
        }
        let path = match &entry.source {
            AssetAvailability::Available(AssetLocation::External { path })
            | AssetAvailability::Available(AssetLocation::Managed { path }) => {
                path.clone()
            }
            AssetAvailability::Missing { previous_path } => {
                anyhow::bail!("asset `{id}` is missing (previous path: {previous_path:?})")
            }
        };
        let hash = hash_file_blake3(&path)?;
        self.set_fingerprint_hash(id, hash)?;
        Ok(true)
    }

    /// fingerprint の hash を埋めます（アップロード完了時の BLAKE3 等）。
    ///
    /// size は登録時に記録済みの値を残します。fingerprint 自体が無い
    /// 場合は、エントリのパスを stat して size を補います。
    pub fn set_fingerprint_hash(
        &mut self,
        id: AssetRef,
        hash: [u8; 32],
    ) -> anyhow::Result<()> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or_else(|| EsotereelError::AssetNotFound(id))?;
        match &mut entry.fingerprint {
            Some(fingerprint) => fingerprint.hash = Some(hash),
            None => {
                let size = match &entry.source {
                    AssetAvailability::Available(AssetLocation::External { path })
                    | AssetAvailability::Available(AssetLocation::Managed { path }) => {
                        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
                    }
                    AssetAvailability::Missing { .. } => 0,
                };
                entry.fingerprint = Some(AssetFingerprint { size, hash: Some(hash) });
            }
        }
        Ok(())
    }

    pub fn resolve(&self, id: AssetRef) -> anyhow::Result<&Path> {
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

    /// 外部ファイル（プロジェクト外の実パス）をアセットとして登録し、その `AssetRef` を返す。
    ///
    /// 同一パスが既に登録済みなら既存の ID を返す（重複登録を避ける）。
    /// 登録は `External` として行い、ファイルの移動は `relink` で付け替える。
    pub fn get_or_add_external(&mut self, path: &Path) -> anyhow::Result<AssetRef> {
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
            fingerprint: fingerprint_for(path),
        };
        self.add(entry)
    }

    /// パスからアセットIDを逆引きする。見つからなければ `None` を返す。
    pub fn resolve_by_path(&self, path: &Path) -> Option<AssetRef> {
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

/// ファイル全体の BLAKE3 ハッシュを計算します。
///
/// External アセットのバックグラウンドでの後埋めや、アップロード完了時の
/// フォールバック（順不同の受信で受信時の incremental 計算が使えなかった
/// 場合）で使います。
pub fn hash_file_blake3(path: &Path) -> std::io::Result<[u8; 32]> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(*hasher.finalize().as_bytes())
}

/// 登録時にファイルのサイズを記録します。
///
/// stat に失敗した場合は `None` を返し、登録自体は成功させます
/// （ファイルがまだ無い・移動済みの場合、fingerprint は後から埋める）。
/// hash はここでは計算せず、アップロード経路の incremental 計算や
/// バックグラウンドでの後埋めに任せます。
fn fingerprint_for(path: &Path) -> Option<AssetFingerprint> {
    let size = std::fs::metadata(path).ok()?.len();
    Some(AssetFingerprint { size, hash: None })
}

#[cfg(test)]
#[path = "../../tests/unit/project/asset_tests.rs"]
mod tests;
