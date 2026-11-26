//! プロジェクトの保存・読み込みです（フォルダ形式）。
//!
//! ```text
//! <folder>/
//!   project.json                  — version、プロジェクト ID、timeline 一覧、assets レジストリ
//!   timelines/<id>/timeline.json  — tps、outline、レイヤー定義、チャンクマニフェスト
//!   timelines/<id>/overview.json  — 全クリップの配置（所属レイヤー・position・duration・kind_id）
//!   timelines/<id>/chunks/<n>.json — 開始位置がその時間範囲に入るクリップ実体
//! ```
//!
//! - チャンク幅は実行時の `CHUNK_TICKS`（30_000 tick）に合わせます。
//!   クリップの所有は開始位置が属するチャンクです。
//! - 書き込みは実ファイル（chunks / overview / timeline）を先に書き、
//!   最後に project.json を差し替えるのがコミットポイントです。
//!   各ファイルは一時ファイル＋リネームの原子書き込みです。
//! - ロードは全チャンクを組み立てて `Project` を復元します。
//!   overview は配置のキャッシュですが、所属レイヤーは overview にしか
//!   ないため、overview とチャンク実体の不整合は見える失敗にします。
//! - `IdGenerator` は保存せず、ロード時にリセットして読み込んだ全 ID を
//!   `observe_*` で引き上げます。

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::project::asset::AssetRegistry;
use crate::project::chunk_index::CHUNK_TICKS;
use crate::project::clip::Clip;
use crate::project::ids::{ClipId, LayerId, TimelineId};
use crate::project::layer::LayerMeta;
use crate::project::layer_outline::LayerOutline;
use crate::project::project::Project;
use crate::project::timeline::Timeline;

/// プロジェクトファイル形式の現行バージョンです。
pub const CURRENT_PROJECT_VERSION: ProjectVersion = ProjectVersion {
    major: 1,
    minor: 0,
    revision: 0,
};

/// プロジェクトファイル形式のバージョン（major.minor.revision の3要素）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectVersion {
    pub major: u32,
    pub minor: u32,
    pub revision: u32,
}

impl std::fmt::Display for ProjectVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.revision)
    }
}

/// ファイルのバージョンが読み取り側に対してどういう位置にあるか。
///
/// 判定だけを lib で行い、ダイアログの表示は呼び出し側（GUI）の責務です。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionStatus {
    /// ファイルが読み取り側と同じか古い。通常通り開けます。
    Compatible,
    /// revision だけ新しい。そのまま開けます。
    NewerRevision,
    /// minor が新しい。警告を出します。
    NewerMinor,
    /// major が新しい。強い警告（確認を取るまで進めない形）を出します。
    NewerMajor,
}

impl ProjectVersion {
    /// 読み取り側のバージョンに対するこのバージョンの位置を判定します。
    pub fn status_against(&self, reader: ProjectVersion) -> VersionStatus {
        if self.major > reader.major {
            VersionStatus::NewerMajor
        } else if self.major < reader.major {
            VersionStatus::Compatible
        } else if self.minor > reader.minor {
            VersionStatus::NewerMinor
        } else if self.minor < reader.minor {
            VersionStatus::Compatible
        } else if self.revision > reader.revision {
            VersionStatus::NewerRevision
        } else {
            VersionStatus::Compatible
        }
    }
}

/// プロジェクトの保存・読み込みで発生するエラーです。
#[derive(Debug, Error)]
pub enum SaveError {
    /// ファイルの読み書きに失敗した（ファイルが存在しない場合を含む）。
    #[error("project I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON として壊れている、または必須フィールドが欠けている。
    #[error("project file format error: {0}")]
    Format(#[from] serde_json::Error),
    /// このビルドが対応していないファイルバージョンだった。
    #[error("unsupported project file version: {found} (supported: {supported})")]
    UnsupportedVersion {
        found: ProjectVersion,
        supported: ProjectVersion,
    },
    /// フォルダ内のファイル同士が整合していない。
    #[error("project folder is structurally invalid: {0}")]
    Invalid(String),
}

/// `load_project` の結果です。復元したプロジェクトと、開いたファイルの
/// バージョンが読み取り側に対してどういう位置にあったかを返します。
#[derive(Debug)]
pub struct LoadedProject {
    pub project: Project,
    pub version: ProjectVersion,
    pub status: VersionStatus,
}

/// `project.json` の内容です。
///
/// `assets` は `AssetRegistry` の JSON 値そのものです。`Project` は
/// レジストリを借用でしか渡せないため、値として保持します。
#[derive(Serialize, Deserialize)]
struct ProjectFile {
    version: ProjectVersion,
    id: Uuid,
    timelines: Vec<TimelineId>,
    assets: serde_json::Value,
}

/// `timelines/<id>/timeline.json` の内容です。
#[derive(Serialize, Deserialize)]
struct TimelineFile {
    id: TimelineId,
    tps: f64,
    layers: Vec<LayerMeta>,
    outline: LayerOutline,
    chunk_ticks: i64,
    chunks: Vec<ChunkManifestEntry>,
}

/// チャンクマニフェストの1件です。`max_end` はそのチャンク内のクリップの
/// 終了 tick の最大値で、範囲読み込みがどこまで遡るかの判定に使います。
#[derive(Serialize, Deserialize)]
struct ChunkManifestEntry {
    index: i64,
    start: i64,
    max_end: i64,
}

/// `timelines/<id>/overview.json` の内容です。
#[derive(Serialize, Deserialize)]
struct OverviewFile {
    placements: Vec<Placement>,
}

/// クリップ1件分の配置です。実体はチャンク側が正で、こちらは配置の
/// キャッシュ（所属レイヤー・position・duration・kind 参照）です。
#[derive(Serialize, Deserialize)]
struct Placement {
    clip: ClipId,
    layer: LayerId,
    position: i64,
    duration: i64,
    kind_id: String,
}

/// `timelines/<id>/chunks/<n>.json` の内容です。
#[derive(Serialize, Deserialize)]
struct ChunkFile {
    index: i64,
    start: i64,
    clips: Vec<Clip>,
}

/// プロジェクトを `dir` へフォルダ形式で保存します。
///
/// 存在しないディレクトリは作成します。書き込みは実ファイルを先に行い、
/// 最後に `project.json` を差し替えるのがコミットポイントです。
/// 各ファイルは同一ディレクトリ内の一時ファイル経由で原子書き込みします。
pub fn save_project(project: &Project, dir: &Path) -> Result<(), SaveError> {
    // 先に全ファイルの内容を直列化する。直列化に失敗しても
    // ディスクには何も書かれていない状態を保つ。
    let timeline_files = project
        .iter_timelines()
        .map(|(&id, timeline)| build_timeline_files(id, timeline))
        .collect::<Result<Vec<(TimelineFile, OverviewFile, Vec<ChunkFile>)>, SaveError>>()?;
    let project_file = ProjectFile {
        version: CURRENT_PROJECT_VERSION,
        id: project.id(),
        timelines: project.iter_timelines().map(|(&id, _)| id).collect(),
        assets: serde_json::to_value(project.assets())?,
    };

    fs::create_dir_all(dir)?;
    for (timeline_file, overview, chunks) in &timeline_files {
        let timeline_dir = dir.join("timelines").join(timeline_file.id.to_string());
        let chunks_dir = timeline_dir.join("chunks");
        fs::create_dir_all(&chunks_dir)?;

        for chunk in chunks {
            let path = chunks_dir.join(format!("{}.json", chunk.index));
            write_atomic(&path, &serde_json::to_string_pretty(chunk)?)?;
        }
        remove_stale_chunks(&chunks_dir, chunks.iter().map(|c| c.index).collect());
        write_atomic(
            &timeline_dir.join("overview.json"),
            &serde_json::to_string_pretty(overview)?,
        )?;
        write_atomic(
            &timeline_dir.join("timeline.json"),
            &serde_json::to_string_pretty(timeline_file)?,
        )?;
    }

    // コミットポイント: ここまで成功してから project.json を差し替える。
    write_atomic(
        &dir.join("project.json"),
        &serde_json::to_string_pretty(&project_file)?,
    )?;
    Ok(())
}

/// 1タイムライン分の保存ファイル内容を組み立てます。
fn build_timeline_files(
    id: TimelineId,
    timeline: &Timeline,
) -> Result<(TimelineFile, OverviewFile, Vec<ChunkFile>), SaveError> {
    let mut placements: Vec<Placement> = Vec::new();
    let mut chunks: BTreeMap<i64, Vec<Clip>> = BTreeMap::new();
    for (&layer_id, layer) in timeline.iter_layers() {
        for (&position, &clip_id) in &layer.clips {
            let Some(clip) = timeline.get_clip(clip_id) else {
                return Err(SaveError::Invalid(format!(
                    "layer {layer_id} references missing clip {clip_id}"
                )));
            };
            if clip.position != position {
                return Err(SaveError::Invalid(format!(
                    "clip {clip_id} position mismatch: layer says {position}, clip says {}",
                    clip.position
                )));
            }
            placements.push(Placement {
                clip: clip_id,
                layer: layer_id,
                position: clip.position,
                duration: clip.duration,
                kind_id: clip.kind_id.full().to_string(),
            });
            chunks
                .entry(chunk_index_of(clip.position))
                .or_default()
                .push(clip.clone());
        }
    }
    // 決定性を保つため、配置はレイヤー・位置順、チャンク内は位置・ID 順に並べる。
    placements.sort_by_key(|p| (p.layer, p.position, p.clip));

    let mut chunk_files = Vec::new();
    let mut manifest = Vec::new();
    for (index, mut clips) in chunks {
        clips.sort_by_key(|c| (c.position, c.id));
        let max_end = clips
            .iter()
            .map(|c| c.position + c.duration)
            .max()
            .unwrap_or(index * CHUNK_TICKS);
        manifest.push(ChunkManifestEntry {
            index,
            start: index * CHUNK_TICKS,
            max_end,
        });
        chunk_files.push(ChunkFile {
            index,
            start: index * CHUNK_TICKS,
            clips,
        });
    }

    // 決定性を保つため、レイヤーは ID 順に並べる（`layers` は HashMap 由来で順序不定）。
    let mut layers = timeline.layers_meta();
    layers.sort_by_key(|layer| layer.id);

    let timeline_file = TimelineFile {
        id,
        tps: timeline.tps,
        layers,
        outline: timeline.outline.clone(),
        chunk_ticks: CHUNK_TICKS,
        chunks: manifest,
    };
    let overview = OverviewFile { placements };
    Ok((timeline_file, overview, chunk_files))
}

/// `dir` からプロジェクトを読み込みます。
///
/// 全チャンクを組み立てて `Project` を復元します。`IdGenerator` は
/// 保存されないためリセットし、読み込んだ全 ID を `observe_*` で
/// 引き上げます（`Project::from_loaded` を参照）。
pub fn load_project(dir: &Path) -> Result<LoadedProject, SaveError> {
    let text = fs::read_to_string(dir.join("project.json"))?;
    let file: ProjectFile = serde_json::from_str(&text)?;
    if file.version.major == 0 {
        return Err(SaveError::UnsupportedVersion {
            found: file.version,
            supported: CURRENT_PROJECT_VERSION,
        });
    }
    let status = file.version.status_against(CURRENT_PROJECT_VERSION);
    let assets: AssetRegistry = serde_json::from_value(file.assets)?;

    let mut seen_timelines = std::collections::HashSet::new();
    let mut timelines = BTreeMap::new();
    for timeline_id in &file.timelines {
        if !seen_timelines.insert(*timeline_id) {
            return Err(SaveError::Invalid(format!(
                "duplicate timeline {timeline_id} in manifest"
            )));
        }
        let timeline = load_timeline(dir, *timeline_id)?;
        timelines.insert(*timeline_id, timeline);
    }

    Ok(LoadedProject {
        project: Project::from_loaded(file.id, timelines, assets),
        version: file.version,
        status,
    })
}

/// 1タイムライン分のファイルを読み込み、`Timeline` を組み立てます。
fn load_timeline(dir: &Path, timeline_id: TimelineId) -> Result<Timeline, SaveError> {
    let timeline_dir = dir.join("timelines").join(timeline_id.to_string());
    let read = |name: &str| -> Result<String, SaveError> {
        Ok(fs::read_to_string(timeline_dir.join(name))?)
    };
    let timeline_file: TimelineFile = serde_json::from_str(&read("timeline.json")?)?;
    if timeline_file.id != timeline_id {
        return Err(SaveError::Invalid(format!(
            "timeline file id {} does not match manifest id {timeline_id}",
            timeline_file.id
        )));
    }
    let overview: OverviewFile = serde_json::from_str(&read("overview.json")?)?;

    let mut bodies: HashMap<ClipId, Clip> = HashMap::new();
    for entry in &timeline_file.chunks {
        let chunk_path = timeline_dir
            .join("chunks")
            .join(format!("{}.json", entry.index));
        let chunk: ChunkFile = serde_json::from_str(&fs::read_to_string(&chunk_path)?)?;
        if chunk.index != entry.index || chunk.start != entry.start {
            return Err(SaveError::Invalid(format!(
                "chunk file {} does not match manifest (index {}, start {})",
                chunk_path.display(),
                entry.index,
                entry.start
            )));
        }
        if chunk.start != entry.index * timeline_file.chunk_ticks {
            return Err(SaveError::Invalid(format!(
                "chunk {} start {} does not match chunk_ticks {}",
                entry.index, chunk.start, timeline_file.chunk_ticks
            )));
        }
        for clip in chunk.clips {
            if clip.position < chunk.start
                || clip.position >= chunk.start + timeline_file.chunk_ticks
            {
                return Err(SaveError::Invalid(format!(
                    "clip {} at {} does not belong to chunk {}",
                    clip.id, clip.position, entry.index
                )));
            }
            let clip_id = clip.id;
            if bodies.insert(clip_id, clip).is_some() {
                return Err(SaveError::Invalid(format!(
                    "duplicate clip {clip_id} across chunks"
                )));
            }
        }
    }

    let mut timeline = Timeline::new_empty(timeline_id, timeline_file.tps);
    timeline.outline = timeline_file.outline;
    let mut seen_layers = std::collections::HashSet::new();
    for layer_meta in &timeline_file.layers {
        if !seen_layers.insert(layer_meta.id) {
            return Err(SaveError::Invalid(format!(
                "duplicate layer {} in timeline {timeline_id}",
                layer_meta.id
            )));
        }
        timeline.apply_layer_meta(layer_meta.clone());
    }
    for layer_id in timeline.outline.iter_execution_order() {
        if timeline.get_layer(layer_id).is_none() {
            return Err(SaveError::Invalid(format!(
                "outline references missing layer {layer_id}"
            )));
        }
    }

    let mut seen_placements = std::collections::HashSet::new();
    let mut occupied_positions = std::collections::HashSet::new();
    for placement in &overview.placements {
        if !seen_placements.insert(placement.clip) {
            return Err(SaveError::Invalid(format!(
                "duplicate placement for clip {}",
                placement.clip
            )));
        }
        if !occupied_positions.insert((placement.layer, placement.position)) {
            return Err(SaveError::Invalid(format!(
                "two clips at position {} in layer {}",
                placement.position, placement.layer
            )));
        }
        let Some(clip) = bodies.remove(&placement.clip) else {
            return Err(SaveError::Invalid(format!(
                "overview references clip {} with no body in chunks",
                placement.clip
            )));
        };
        if clip.position != placement.position
            || clip.duration != placement.duration
            || clip.kind_id.full() != placement.kind_id
        {
            return Err(SaveError::Invalid(format!(
                "overview placement for clip {} does not match chunk body",
                placement.clip
            )));
        }
        if timeline.get_layer(placement.layer).is_none() {
            return Err(SaveError::Invalid(format!(
                "overview references missing layer {}",
                placement.layer
            )));
        }
        timeline
            .place_clip(placement.layer, clip)
            .map_err(|e| SaveError::Invalid(format!("failed to place clip: {e:#}")))?;
    }
    if let Some((orphan_id, _)) = bodies.iter().next() {
        return Err(SaveError::Invalid(format!(
            "chunk body for clip {orphan_id} has no overview placement"
        )));
    }

    // 読み込み時の place_clip が付けた変更マークを捨てる。
    timeline.drain_changes();
    Ok(timeline)
}

/// 開始位置が属するチャンクの連番です。実行時の `ChunkIndex` と同じ規則です。
fn chunk_index_of(position: i64) -> i64 {
    position.div_euclid(CHUNK_TICKS)
}

/// `path` へ一時ファイル＋リネームで原子書き込みします。
fn write_atomic(path: &Path, text: &str) -> Result<(), SaveError> {
    let tmp_path = tmp_path_for(path);
    fs::write(&tmp_path, text)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// マニフェストに無い古いチャンクファイルを消します。
///
/// 保存後に残った古いチャンクはロードでは読まれませんが、フォルダ内に
/// 溜まり続けるため掃除します。削除に失敗しても保存自体は成功しているので、
/// 警告ログに留めます。
fn remove_stale_chunks(chunks_dir: &Path, live: std::collections::HashSet<i64>) {
    let Ok(entries) = fs::read_dir(chunks_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(stem) = name.to_str().and_then(|n| n.strip_suffix(".json")) else {
            continue;
        };
        let Ok(index) = stem.parse::<i64>() else {
            continue;
        };
        if live.contains(&index) {
            continue;
        }
        if let Err(e) = fs::remove_file(entry.path()) {
            log::warn!(
                "save: failed to remove stale chunk file {}: {e}",
                entry.path().display()
            );
        }
    }
}

/// 保存用の一時ファイルパス（対象と同一ディレクトリ、`*.tmp` 付け替え）です。
fn tmp_path_for(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
#[path = "../../tests/unit/project/save_tests.rs"]
mod tests;
