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
//!   overview は配置のキャッシュで、正はチャンク実体です。overview が
//!   欠落・破損・不整合の場合は、チャンク実体（所属レイヤーはチャンク
//!   ファイル側にも持つ）から配置を再生成して復旧します。
//! - `IdGenerator` は保存せず、ロード時にリセットして読み込んだ全 ID を
//!   `observe_*` で引き上げます。

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::project::asset::AssetRegistry;
use crate::project::chunk::{chunk_of, ChunkId, CHUNK_TICKS};
use crate::project::clip::Clip;
use crate::project::ids::{ClipId, LayerId, TimelineId};
use crate::project::layer::LayerMeta;
use crate::project::layer_outline::LayerOutline;
use crate::project::project::Project;
use crate::project::timeline::Timeline;

/// プロジェクトファイル形式の現行バージョンです。
///
/// 1.1.0: メディア source の値が文字列から `AssetRef` 型の値になった。
/// 旧リーダー（1.0.x）は新しい値表現を読めないため minor を上げています。
/// 旧形式の文字列 source は、開く側（core）が読み込み時に変換します。
pub const CURRENT_PROJECT_VERSION: ProjectVersion = ProjectVersion {
    major: 1,
    minor: 1,
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
pub(crate) struct ChunkFile {
    pub index: i64,
    pub start: i64,
    pub clips: Vec<ChunkClip>,
}

/// チャンクファイル内のクリップ1件です。所属レイヤーは overview にも
/// 持ちますが、overview が再生成可能なキャッシュであるためには正である
/// チャンク側にも必要です（overview 欠落時に配置を再生成するため）。
#[derive(Serialize, Deserialize)]
pub(crate) struct ChunkClip {
    pub layer: LayerId,
    pub clip: Clip,
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
    let mut chunks: BTreeMap<ChunkId, Vec<ChunkClip>> = BTreeMap::new();
    for (clip_id, placement) in timeline.iter_placements() {
        // 削除済みレイヤーに残った配置は、従来（レイヤー駆動の走査）と同じく
        // 保存対象から外れる。
        if timeline.get_layer(placement.layer_id).is_none() {
            continue;
        }
        let Some(clip) = timeline.get_clip(clip_id) else {
            // 配置はあるが実体が引けない = アンロード中。実体が正の保存形式では
            // 書けないため、見える失敗にする。
            return Err(SaveError::Invalid(format!(
                "clip {clip_id} is in an unloaded chunk; load it before saving"
            )));
        };
        if clip.position != placement.position || clip.duration != placement.duration {
            return Err(SaveError::Invalid(format!(
                "clip {clip_id} placement mismatch: placement says ({}, {}), clip says ({}, {})",
                placement.position, placement.duration, clip.position, clip.duration
            )));
        }
        match timeline.kind_table().resolve(placement.kind) {
            Some(kind) if kind.full() == clip.kind_id.full() => {}
            _ => {
                return Err(SaveError::Invalid(format!(
                    "clip {clip_id} kind mismatch between placement and body"
                )));
            }
        }
        placements.push(Placement {
            clip: clip_id,
            layer: placement.layer_id,
            position: placement.position,
            duration: placement.duration,
            kind_id: clip.kind_id.full().to_string(),
        });
        chunks
            .entry(chunk_of(clip.position))
            .or_default()
            .push(ChunkClip {
                layer: placement.layer_id,
                clip: clip.clone(),
            });
    }
    // 決定性を保つため、配置はレイヤー・位置順、チャンク内は位置・ID 順に並べる。
    placements.sort_by_key(|p| (p.layer, p.position, p.clip));

    let mut chunk_files = Vec::new();
    let mut manifest = Vec::new();
    for (index, mut clips) in chunks {
        clips.sort_by_key(|c| (c.clip.position, c.clip.id));
        let max_end = clips
            .iter()
            .map(|c| c.clip.position + c.clip.duration)
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
    let mut bodies: HashMap<ClipId, (LayerId, Clip)> = HashMap::new();
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
        for chunk_clip in chunk.clips {
            let clip = chunk_clip.clip;
            if clip.position < chunk.start
                || clip.position >= chunk.start + timeline_file.chunk_ticks
            {
                return Err(SaveError::Invalid(format!(
                    "clip {} at {} does not belong to chunk {}",
                    clip.id, clip.position, entry.index
                )));
            }
            let clip_id = clip.id;
            if bodies.insert(clip_id, (chunk_clip.layer, clip)).is_some() {
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

    // 配置は overview（キャッシュ）から取るのが基本ですが、正はチャンク
    // 実体なので、overview が欠落・破損・不整合なら実体から再生成します。
    let placements = resolve_placements(&timeline_dir, &bodies, &timeline);
    for placement in &placements {
        let Some((_, clip)) = bodies.remove(&placement.clip) else {
            return Err(SaveError::Invalid(format!(
                "placement references clip {} with no body in chunks",
                placement.clip
            )));
        };
        if timeline.get_layer(placement.layer).is_none() {
            return Err(SaveError::Invalid(format!(
                "placement references missing layer {}",
                placement.layer
            )));
        }
        timeline
            .place_clip(placement.layer, clip)
            .map_err(|e| SaveError::Invalid(format!("failed to place clip: {e:#}")))?;
    }
    if let Some((orphan_id, _)) = bodies.iter().next() {
        return Err(SaveError::Invalid(format!(
            "chunk body for clip {orphan_id} has no placement"
        )));
    }

    // 読み込み時の place_clip が付けた変更マークを捨てる。
    timeline.drain_changes();
    Ok(timeline)
}

/// 配置を決めます。`overview.json` がチャンク実体と完全に一致すれば
/// それを使い、欠落・破損・不整合なら警告ログを残してチャンク実体から
/// 再生成します（overview は再生成可能なキャッシュという設計のため）。
fn resolve_placements(
    timeline_dir: &Path,
    bodies: &HashMap<ClipId, (LayerId, Clip)>,
    timeline: &Timeline,
) -> Vec<Placement> {
    let overview_path = timeline_dir.join("overview.json");
    let overview = fs::read_to_string(&overview_path)
        .map_err(SaveError::from)
        .and_then(|text| serde_json::from_str::<OverviewFile>(&text).map_err(SaveError::from));
    match overview {
        Ok(overview) if overview_matches(&overview, bodies, timeline) => overview.placements,
        Ok(_) => {
            log::warn!(
                "save: {} does not match chunk bodies; regenerating placements from chunks",
                overview_path.display()
            );
            regenerate_placements(bodies)
        }
        Err(error) => {
            log::warn!(
                "save: {} is unreadable ({error}); regenerating placements from chunks",
                overview_path.display()
            );
            regenerate_placements(bodies)
        }
    }
}

/// overview の配置がチャンク実体と完全に一致するか検証します。
fn overview_matches(
    overview: &OverviewFile,
    bodies: &HashMap<ClipId, (LayerId, Clip)>,
    timeline: &Timeline,
) -> bool {
    if overview.placements.len() != bodies.len() {
        return false;
    }
    let mut seen = std::collections::HashSet::new();
    let mut occupied = std::collections::HashSet::new();
    for placement in &overview.placements {
        if !seen.insert(placement.clip) || !occupied.insert((placement.layer, placement.position))
        {
            return false;
        }
        let Some((layer, clip)) = bodies.get(&placement.clip) else {
            return false;
        };
        if *layer != placement.layer
            || clip.position != placement.position
            || clip.duration != placement.duration
            || clip.kind_id.full() != placement.kind_id
        {
            return false;
        }
        if timeline.get_layer(placement.layer).is_none() {
            return false;
        }
    }
    true
}

/// チャンク実体から配置を再生成します（レイヤー・位置・ID 順で決定的に）。
fn regenerate_placements(bodies: &HashMap<ClipId, (LayerId, Clip)>) -> Vec<Placement> {
    let mut placements: Vec<Placement> = bodies
        .values()
        .map(|(layer, clip)| Placement {
            clip: clip.id,
            layer: *layer,
            position: clip.position,
            duration: clip.duration,
            kind_id: clip.kind_id.full().to_string(),
        })
        .collect();
    placements.sort_by_key(|p| (p.layer, p.position, p.clip));
    placements
}

/// `project.json` だけを読んで、バージョンとプロジェクト ID を返します。
///
/// プロジェクトを開く前の事前確認（バージョン警告の段階判定）用です。
/// 本体は読み込みません。
pub fn inspect_project(dir: &Path) -> Result<(ProjectVersion, VersionStatus, Uuid), SaveError> {
    let text = fs::read_to_string(dir.join("project.json"))?;
    let file: ProjectFile = serde_json::from_str(&text)?;
    let status = file.version.status_against(CURRENT_PROJECT_VERSION);
    Ok((file.version, status, file.id))
}

/// プロジェクトフォルダのバックアップを、同じ親フォルダ内に
/// `<フォルダ名>.backup-<UNIX ミリ秒>` として複製し、そのパスを返します。
///
/// 新しいバージョンのプロジェクトを古い読み取り側で開く前の保険です。
pub fn backup_project_folder(dir: &Path) -> Result<PathBuf, SaveError> {
    let name = dir
        .file_name()
        .ok_or_else(|| SaveError::Invalid(format!("invalid project folder {}", dir.display())))?
        .to_string_lossy();
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let parent = dir.parent().unwrap_or_else(|| Path::new("."));
    let mut candidate = parent.join(format!("{name}.backup-{millis}"));
    let mut sequence = 0u32;
    while candidate.exists() {
        sequence += 1;
        candidate = parent.join(format!("{name}.backup-{millis}-{sequence}"));
    }
    copy_dir_recursive(dir, &candidate)?;
    Ok(candidate)
}

/// ディレクトリを再帰的に複製します。
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), SaveError> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// `path` へ一時ファイル＋リネームで原子書き込みします。
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<(), SaveError> {
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
