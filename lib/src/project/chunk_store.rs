//! プロジェクトフォルダの `chunks/<n>.json` を読み書きする
//! `ChunkStore` の実装です。直列化形式は保存形式（`save` モジュール）と
//! 同じ `ChunkFile` を使います。
//!
//! アンロード時の書き出し先は、開いているプロジェクトフォルダの
//! チャンクファイルそのものです。配置とマニフェストの正は配置側に
//! あり、実体の書き出しは配置（位置・duration・kind・レイヤー）も
//! マニフェスト（index・start・max_end は配置から導出）も変えないため、
//! `project.json` を最後に差し替える保存のコミットポイントと競合しません。
//! 書き出された実体は、次の保存・オートセーブ世代がそのまま取り込みます
//! （アンロード書き出しはオートセーブ経路に乗る、という設計決定の実装）。

use std::fs;
use std::path::PathBuf;

use crate::project::chunk::{CHUNK_TICKS, ChunkId, ChunkStore};
use crate::project::clip::Clip;
use crate::project::ids::LayerId;
use crate::project::save::{ChunkClip, ChunkFile, write_atomic};
use crate::util::result::EsotereelResult;

/// 1タイムライン分のフォルダチャンクストアです。
#[derive(Debug)]
pub struct FolderChunkStore {
    chunks_dir: PathBuf,
}

impl FolderChunkStore {
    /// タイムラインのフォルダ（`timelines/<id>/`）を指定して作ります。
    pub fn new(timeline_dir: PathBuf) -> Self {
        Self {
            chunks_dir: timeline_dir.join("chunks"),
        }
    }

    fn chunk_path(&self, chunk_id: ChunkId) -> PathBuf {
        self.chunks_dir.join(format!("{chunk_id}.json"))
    }
}

impl ChunkStore for FolderChunkStore {
    fn load_chunk(&mut self, chunk_id: ChunkId) -> EsotereelResult<Vec<(LayerId, Clip)>> {
        let path = self.chunk_path(chunk_id);
        let text = fs::read_to_string(&path)?;
        let file: ChunkFile = serde_json::from_str(&text)?;
        if file.index != chunk_id || file.start != chunk_id * CHUNK_TICKS {
            anyhow::bail!(
                "chunk file {} does not match chunk {chunk_id}",
                path.display()
            );
        }
        Ok(file
            .clips
            .into_iter()
            .map(|entry| (entry.layer, entry.clip))
            .collect())
    }

    fn store_chunk(
        &mut self,
        chunk_id: ChunkId,
        clips: &[(LayerId, Clip)],
    ) -> EsotereelResult<()> {
        fs::create_dir_all(&self.chunks_dir)?;
        let mut entries: Vec<ChunkClip> = clips
            .iter()
            .map(|(layer, clip)| ChunkClip {
                layer: *layer,
                clip: clip.clone(),
            })
            .collect();
        entries.sort_by_key(|entry| (entry.clip.position, entry.clip.id));
        let file = ChunkFile {
            index: chunk_id,
            start: chunk_id * CHUNK_TICKS,
            clips: entries,
        };
        write_atomic(
            &self.chunk_path(chunk_id),
            &serde_json::to_string_pretty(&file)?,
        )?;
        Ok(())
    }
}
