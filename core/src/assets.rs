use std::{
    collections::HashMap,
    fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use esotereel_lib::{dirs::Directories, project::ids::AssetId};

/// 進行中のアップロード1件分の転送状態。
struct UploadSession {
    file_name: String,
    path: PathBuf,
    total_size: u64,
    received_size: u64,
}

/// クライアントからのアップロードを受け付ける処理器。
///
/// アセットの台帳は持たない。受け取ったファイルの置き場所と転送の進捗だけを
/// 管理し、完了したファイルは呼び出し側がプロジェクトの `AssetRegistry`
/// （`Project::register_managed_asset`）へ登録する。参照登録についても同様で、
/// 実体は `Project::register_external_asset` が台帳に載せる。
pub struct UploadManager {
    root_dir: PathBuf,
    sessions: HashMap<AssetId, UploadSession>,
}

impl UploadManager {
    pub fn new(dirs_def: &Directories) -> Self {
        let root_dir = dirs_def
            .data_dir()
            .unwrap_or_else(|_| std::env::temp_dir().join("esotereel-assets"))
            .join("assets");

        Self {
            root_dir,
            sessions: HashMap::new(),
        }
    }

    /// アップロードを開始し、保存先の空ファイルを用意する。
    ///
    /// 保存先はアセット ID ごとのディレクトリ配下に固定する。ファイル名は
    /// そのまま使うため、パス区切りを含む名前は受け付けない。
    pub fn begin(
        &mut self,
        asset_id: AssetId,
        file_name: &str,
        total_size: u64,
    ) -> anyhow::Result<()> {
        let file_name = validate_file_name(file_name)?;
        let dir = self.root_dir.join(asset_id.to_string());
        fs::create_dir_all(&dir)?;
        let path = dir.join(&file_name);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        fs::File::create(&path)?;

        self.sessions.insert(
            asset_id,
            UploadSession {
                file_name,
                path,
                total_size,
                received_size: 0,
            },
        );
        Ok(())
    }

    /// アップロードの一部を、指定オフセットへ書き込む。
    pub fn write_chunk(
        &mut self,
        asset_id: AssetId,
        offset: u64,
        data: &[u8],
    ) -> anyhow::Result<()> {
        let session = self
            .sessions
            .get_mut(&asset_id)
            .ok_or_else(|| anyhow::anyhow!("asset `{asset_id}` has no active upload"))?;

        let end = offset
            .checked_add(data.len() as u64)
            .ok_or_else(|| anyhow::anyhow!("upload offset overflow for asset `{asset_id}`"))?;
        if end > session.total_size {
            anyhow::bail!(
                "chunk for asset `{asset_id}` exceeds the declared total size ({} > {})",
                end,
                session.total_size
            );
        }

        let mut file = fs::OpenOptions::new().write(true).open(&session.path)?;
        if offset > 0 {
            file.seek(SeekFrom::Start(offset))?;
        }
        file.write_all(data)?;
        session.received_size = session.received_size.max(end);
        Ok(())
    }

    /// アップロードの完了を検証し、登録用に（ファイル名, 保存先パス）を返す。
    ///
    /// 宣言された総サイズと受信量・実ファイルサイズが一致しない場合はエラーにし、
    /// セッションは残す（不足分のチャンクを送って再試行できる）。
    pub fn complete(&mut self, asset_id: AssetId) -> anyhow::Result<(String, PathBuf)> {
        let session = self
            .sessions
            .get(&asset_id)
            .ok_or_else(|| anyhow::anyhow!("asset `{asset_id}` has no active upload"))?;

        if session.received_size != session.total_size {
            anyhow::bail!(
                "upload for asset `{asset_id}` is incomplete: {} of {} bytes received",
                session.received_size,
                session.total_size
            );
        }
        let actual_size = fs::metadata(&session.path)?.len();
        if actual_size != session.total_size {
            anyhow::bail!(
                "upload for asset `{asset_id}` is incomplete: file holds {} of {} bytes",
                actual_size,
                session.total_size
            );
        }

        let session = self
            .sessions
            .remove(&asset_id)
            .ok_or_else(|| anyhow::anyhow!("asset `{asset_id}` has no active upload"))?;
        Ok((session.file_name, session.path))
    }
}

/// ファイル名として受け付けられるのは、パス区切りや親ディレクトリ参照を
/// 含まない単一の名前だけとする。
fn validate_file_name(file_name: &str) -> anyhow::Result<String> {
    let name = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid file name `{file_name}`"))?;
    if name != file_name {
        anyhow::bail!("invalid file name `{file_name}`");
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager_in_temp_dir(tag: &str) -> (UploadManager, PathBuf) {
        let working_dir =
            std::env::temp_dir().join(format!("esotereel-upload-test-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&working_dir);
        let dirs = Directories::new(None, Some(working_dir.clone()));
        (UploadManager::new(&dirs), working_dir)
    }

    #[test]
    fn begin_rejects_file_names_with_path_separators() {
        let (mut manager, working_dir) = manager_in_temp_dir("traversal");
        let asset_id = AssetId::new_v4();

        assert!(manager.begin(asset_id, "../evil.mp4", 0).is_err());
        assert!(manager.begin(asset_id, "sub/dir.mp4", 0).is_err());
        assert!(manager.begin(asset_id, "clip.mp4", 0).is_ok());

        let _ = fs::remove_dir_all(&working_dir);
    }

    #[test]
    fn complete_requires_the_full_declared_size() {
        let (mut manager, working_dir) = manager_in_temp_dir("size");
        let asset_id = AssetId::new_v4();

        manager.begin(asset_id, "clip.mp4", 5).unwrap();
        manager.write_chunk(asset_id, 0, b"abc").unwrap();
        assert!(manager.complete(asset_id).is_err());

        manager.write_chunk(asset_id, 3, b"de").unwrap();
        let (name, path) = manager.complete(asset_id).unwrap();
        assert_eq!(name, "clip.mp4");
        assert_eq!(fs::metadata(&path).unwrap().len(), 5);

        let _ = fs::remove_dir_all(&working_dir);
    }
}
