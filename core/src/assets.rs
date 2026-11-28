use std::{
    collections::HashMap,
    fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use esotereel_lib::{
    decode::growing::UploadProgress,
    dirs::Directories,
    project::{asset::hash_file_blake3, ids::AssetRef},
};

/// 進行中のアップロード1件分の転送状態。
struct UploadSession {
    file_name: String,
    path: PathBuf,
    total_size: u64,
    /// アップロードを開始したクライアント。切断時の後片付けに使う。
    client_id: u32,
    /// 到着済み接頭辞と完了/中断を読み側と共有する進捗。
    progress: Arc<UploadProgress>,
    /// 受信しながら回す BLAKE3 ハッシュ。先頭から連続して届いた
    /// 接頭辞だけを対象にし、接頭辞が途切れた場合は `None` になり
    /// 完了時にファイルを読み直して計算します。
    hasher: Option<blake3::Hasher>,
    /// ハッシュ済みの先頭からの連続バイト数。
    hashed_len: u64,
}

impl Drop for UploadSession {
    /// 完了せずに破棄されたセッションは中断として扱い、
    /// 部分ファイルを読んでいる側をエラーで目覚めさせます。
    fn drop(&mut self) {
        self.progress.abort();
    }
}

/// クライアントからのアップロードを受け付ける処理器。
///
/// アセットの台帳は持たない。受け取ったファイルの置き場所と転送の進捗だけを
/// 管理し、完了したファイルは呼び出し側がプロジェクトの `AssetRegistry`
/// （`Project::register_managed_asset`）へ登録する。参照登録についても同様で、
/// 実体は `Project::register_external_asset` が台帳に載せる。
pub struct UploadManager {
    root_dir: PathBuf,
    sessions: HashMap<AssetRef, UploadSession>,
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
        asset_id: AssetRef,
        file_name: &str,
        total_size: u64,
        client_id: u32,
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
                client_id,
                progress: Arc::new(UploadProgress::new(total_size)),
                hasher: Some(blake3::Hasher::new()),
                hashed_len: 0,
            },
        );
        Ok(())
    }

    /// 指定クライアントの未完了セッションをすべて破棄します。
    ///
    /// セッションの破棄で読み側は中断として目覚め、部分ファイルも消します。
    /// クライアント切断時に呼び、セッションとファイルが残留しないようにします。
    pub fn abort_client_sessions(&mut self, client_id: u32) {
        let aborted: Vec<UploadSession> = self
            .sessions
            .extract_if(|_, session| session.client_id == client_id)
            .map(|(_, session)| session)
            .collect();
        for session in &aborted {
            if let Err(error) = fs::remove_file(&session.path) {
                log::warn!(
                    "Failed to remove partial upload {}: {error}",
                    session.path.display()
                );
            }
        }
    }

    /// 指定パスへアップロード中のセッションの進捗を返します。
    ///
    /// ストリーム初期化時に、対象ファイルがアップロード中かどうかを
    /// 判定するために使います。該当セッションが無ければ `None` です。
    pub fn progress_for_path(&self, path: &Path) -> Option<Arc<UploadProgress>> {
        self.sessions
            .values()
            .find(|session| session.path == path)
            .map(|session| Arc::clone(&session.progress))
    }

    /// アップロードの一部を、指定オフセットへ書き込む。
    pub fn write_chunk(
        &mut self,
        asset_id: AssetRef,
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
        session.progress.record_range(offset, data.len() as u64);

        // 受信しながら BLAKE3 を回す。対象は先頭から連続した接頭辞だけ。
        // 順不同で後ろが先に届いた分はハッシュせず、完了時の読み直しに任せる。
        // ハッシュ済み範囲への書き込み（再送で内容が変わりうる）は
        // incremental な計算を無効化し、完了時の読み直しへ落とす。
        let mut invalidate = false;
        if let Some(hasher) = session.hasher.as_mut() {
            if offset == session.hashed_len {
                hasher.update(data);
                session.hashed_len += data.len() as u64;
            } else if offset < session.hashed_len {
                invalidate = true;
            }
        }
        if invalidate {
            session.hasher = None;
        }
        Ok(())
    }

    /// アップロードの完了を検証し、登録用に（ファイル名, 保存先パス, BLAKE3 ハッシュ）を返す。
    ///
    /// ハッシュは受信しながら計算したものを使い、順不同の受信で接頭辞が
    /// 途切れた場合だけ完了時にファイルを読み直して計算します
    /// （順序通りの受信では追加 I/O は発生しません）。
    ///
    /// 宣言された総サイズと受信量・実ファイルサイズが一致しない場合はエラーにし、
    /// セッションは残す（不足分のチャンクを送って再試行できる）。
    pub fn complete(
        &mut self,
        asset_id: AssetRef,
    ) -> anyhow::Result<(String, PathBuf, blake3::Hash)> {
        let session = self
            .sessions
            .get(&asset_id)
            .ok_or_else(|| anyhow::anyhow!("asset `{asset_id}` has no active upload"))?;

        // 受信済み範囲の末尾ではなく、先頭から連続して届いた長さで判定する。
        // 末尾だけ届いた場合、ファイル長は total_size と一致しても
        // 途中がゼロ埋めのまま完了扱いになってしまう。
        let received = session.progress.prefix_len();
        if received != session.total_size {
            anyhow::bail!(
                "upload for asset `{asset_id}` is incomplete: {} of {} bytes received",
                received,
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

        session.progress.finish();

        let hash = match &session.hasher {
            Some(hasher) if session.hashed_len == session.total_size => hasher.finalize(),
            _ => hash_file_blake3(&session.path)?,
        };

        let session = self
            .sessions
            .remove(&asset_id)
            .ok_or_else(|| anyhow::anyhow!("asset `{asset_id}` has no active upload"))?;
        Ok((session.file_name.clone(), session.path.clone(), hash))
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
#[path = "../tests/unit/assets_tests.rs"]
mod tests;
