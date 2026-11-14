use std::{
    collections::HashMap,
    fs,
    io::{Seek, Write},
    path::{Path, PathBuf},
};

use esotereel_lib::dirs::Directories;

#[derive(Debug, Clone)]
pub enum AssetSource {
    LocalReference { path: PathBuf },
    ManagedUpload { path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct ManagedAsset {
    pub id: String,
    pub file_name: String,
    pub source: AssetSource,
    pub total_size: Option<u64>,
    pub uploaded_size: u64,
}

#[derive(Default)]
pub struct AssetsManager {
    root_dir: PathBuf,
    assets: HashMap<String, ManagedAsset>,
}

impl AssetsManager {
    pub fn new(dirs_def: Directories) -> Self {
        let root_dir = dirs_def
            .data_dir()
            .unwrap_or_else(|_| std::env::temp_dir().join("esotereel-assets"))
            .join("assets");
        fs::create_dir_all(&root_dir).ok();

        Self {
            root_dir,
            assets: HashMap::new(),
        }
    }

    pub fn register_reference(&mut self, asset_id: &str, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref().to_path_buf();
        self.assets.insert(
            asset_id.to_string(),
            ManagedAsset {
                id: asset_id.to_string(),
                file_name: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("asset")
                    .to_string(),
                source: AssetSource::LocalReference { path: path.clone() },
                total_size: Some(std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0)),
                uploaded_size: 0,
            },
        );
        Ok(())
    }

    pub fn begin_upload(
        &mut self,
        asset_id: &str,
        file_name: &str,
        total_size: u64,
    ) -> anyhow::Result<()> {
        let target_path = self.root_dir.join(file_name);
        fs::create_dir_all(&self.root_dir).ok();

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).ok();
        }

        self.assets.insert(
            asset_id.to_string(),
            ManagedAsset {
                id: asset_id.to_string(),
                file_name: file_name.to_string(),
                source: AssetSource::ManagedUpload {
                    path: target_path.clone(),
                },
                total_size: Some(total_size),
                uploaded_size: 0,
            },
        );

        if target_path.exists() {
            fs::remove_file(&target_path).ok();
        }
        fs::File::create(&target_path)?;
        Ok(())
    }

    pub fn upload_chunk(&mut self, asset_id: &str, offset: u64, data: &[u8]) -> anyhow::Result<()> {
        let entry = self
            .assets
            .get_mut(asset_id)
            .ok_or_else(|| anyhow::anyhow!("asset `{asset_id}` is not registered"))?;

        let path = match &entry.source {
            AssetSource::ManagedUpload { path } => path.clone(),
            AssetSource::LocalReference { .. } => {
                anyhow::bail!("asset `{asset_id}` is not an upload-managed asset")
            }
        };

        let mut file = fs::OpenOptions::new().create(true).write(true).open(&path)?;
        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))?;
        }
        file.write_all(data)?;
        entry.uploaded_size = offset + data.len() as u64;
        Ok(())
    }

    pub fn finish_upload(&mut self, asset_id: &str) -> anyhow::Result<PathBuf> {
        let entry = self
            .assets
            .get(asset_id)
            .ok_or_else(|| anyhow::anyhow!("asset `{asset_id}` is not registered"))?
            .clone();

        let path = match &entry.source {
            AssetSource::ManagedUpload { path } => path.clone(),
            AssetSource::LocalReference { path } => path.clone(),
        };

        self.assets
            .get_mut(asset_id)
            .map(|asset| asset.uploaded_size = asset.total_size.unwrap_or(asset.uploaded_size));

        Ok(path)
    }

    pub fn resolve(&self, asset_id: &str) -> Option<&Path> {
        self.assets
            .get(asset_id)
            .and_then(|asset| match &asset.source {
                AssetSource::LocalReference { path } | AssetSource::ManagedUpload { path } => {
                    Some(path.as_path())
                }
            })
    }
}
