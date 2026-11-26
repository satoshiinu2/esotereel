//! アプリ設定です。VS Code 式の階層設定です。
//!
//! 解決順は既定値（コード内蔵）＜ ユーザー設定 ＜ プロジェクト設定 ＜
//! サーバー設定です。形式は TOML で、未知のキーは無視します
//! （警告ログに残します）。
//!
//! - ユーザー設定: `config_dir` の `settings.toml`。
//! - プロジェクト設定: プロジェクトフォルダ内の `settings.toml`。
//! - サーバー設定: core が渡す TOML ファイル（サーバー側のポリシー）。
//!
//! キーはそれぞれ、どのスコープで設定できるかを宣言します
//! （`SettingScope` を参照）。スコープ外の層にある値は無視します。
//!
//! 注意: チャンク幅のように保存形式に効く値は、正はプロジェクト側の
//! 記録（`timeline.json` の `chunk_ticks`）で、ユーザー設定の値は
//! 新規プロジェクト作成時の既定値としてだけ効きます。

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::project::chunk_index::CHUNK_TICKS;

/// キーがどのスコープで設定できるかの宣言です。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingScope {
    /// ユーザー設定でのみ設定できます。プロジェクト／サーバー側の値は無視します。
    UserOnly,
    /// プロジェクト設定・サーバー設定でのみ設定できます。
    /// ユーザー側の値は無視します。
    ProjectOrServerOnly,
    /// どの層でも設定でき、後の層が前の層を上書きします。
    Overridable,
}

/// 解決済みのアプリ設定です。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettings {
    /// 新規プロジェクト作成時のチャンク幅の既定値（tick）。
    /// 既存プロジェクトの幅は `timeline.json` の記録が正です。
    pub chunk_default_width_ticks: i64,
    /// このプロジェクトが次回保存時に使うチャンク幅。
    /// `None` なら記録されている幅を維持します（再レイアウトしない）。
    pub chunk_width_ticks: Option<i64>,
    /// チャンクをアンロードするまでの未使用時間（秒）。
    pub chunk_unload_after_secs: u64,
    /// 頻繁に使われるチャンクをアンロード対象から外すかどうか。
    pub chunk_keep_frequently_used: bool,
    /// オートセーブの間隔（秒）。
    pub autosave_interval_secs: u64,
    /// プロジェクトごとに保持するオートセーブの世代数。
    pub autosave_generations: u32,
    /// オートセーブ全体の容量上限（バイト）。超えたら古い世代から削除します。
    pub autosave_max_total_bytes: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            chunk_default_width_ticks: CHUNK_TICKS,
            chunk_width_ticks: None,
            chunk_unload_after_secs: 600,
            chunk_keep_frequently_used: true,
            autosave_interval_secs: 300,
            autosave_generations: 5,
            autosave_max_total_bytes: 1024 * 1024 * 1024,
        }
    }
}

/// 層（どのファイルから読んだ値か）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    User,
    Project,
    Server,
}

impl SettingScope {
    /// この層がこのスコープのキーを設定できるかどうか。
    fn allows(self, layer: Layer) -> bool {
        match (self, layer) {
            (SettingScope::UserOnly, Layer::User) => true,
            (SettingScope::ProjectOrServerOnly, Layer::Project)
            | (SettingScope::ProjectOrServerOnly, Layer::Server) => true,
            (SettingScope::Overridable, _) => true,
            _ => false,
        }
    }
}

/// TOML から読む部分設定です。すべてのフィールドが `Option` で、
/// 指定されたキーだけが上書きします。未知のキーは無視されます。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PartialAppSettings {
    chunk: Option<PartialChunkSettings>,
    autosave: Option<PartialAutosaveSettings>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PartialChunkSettings {
    default_width_ticks: Option<i64>,
    width_ticks: Option<i64>,
    unload_after_secs: Option<u64>,
    keep_frequently_used: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PartialAutosaveSettings {
    interval_secs: Option<u64>,
    generations: Option<u32>,
    max_total_bytes: Option<u64>,
}

impl AppSettings {
    /// 既定値だけの設定を返します。
    pub fn new() -> Self {
        Self::default()
    }

    /// ユーザー・プロジェクト・サーバーの設定ファイルを読み込み、
    /// 階層解決した設定を返します。
    ///
    /// 存在しないファイルは飛ばします。TOML のパースに失敗した場合は
    /// エラーを返します（黙って既定値に落ちません）。
    pub fn load(
        user_path: Option<&Path>,
        project_dir: Option<&Path>,
        server_path: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let mut settings = Self::default();
        if let Some(path) = user_path {
            let partial = load_partial(path)?;
            settings.apply(partial, Layer::User, path);
        }
        if let Some(dir) = project_dir {
            let path = dir.join("settings.toml");
            let partial = load_partial(&path)?;
            settings.apply(partial, Layer::Project, &path);
        }
        if let Some(path) = server_path {
            let partial = load_partial(path)?;
            settings.apply(partial, Layer::Server, path);
        }
        Ok(settings)
    }

    fn apply(&mut self, partial: PartialAppSettings, layer: Layer, path: &Path) {
        if let Some(chunk) = partial.chunk {
            self.apply_if_allowed(
                SettingScope::UserOnly,
                layer,
                "chunk.default_width_ticks",
                path,
                chunk.default_width_ticks,
                |settings, value| settings.chunk_default_width_ticks = value,
            );
            self.apply_if_allowed(
                SettingScope::ProjectOrServerOnly,
                layer,
                "chunk.width_ticks",
                path,
                chunk.width_ticks,
                |settings, value| settings.chunk_width_ticks = Some(value),
            );
            self.apply_if_allowed(
                SettingScope::Overridable,
                layer,
                "chunk.unload_after_secs",
                path,
                chunk.unload_after_secs,
                |settings, value| settings.chunk_unload_after_secs = value,
            );
            self.apply_if_allowed(
                SettingScope::Overridable,
                layer,
                "chunk.keep_frequently_used",
                path,
                chunk.keep_frequently_used,
                |settings, value| settings.chunk_keep_frequently_used = value,
            );
        }
        if let Some(autosave) = partial.autosave {
            self.apply_if_allowed(
                SettingScope::UserOnly,
                layer,
                "autosave.interval_secs",
                path,
                autosave.interval_secs,
                |settings, value| settings.autosave_interval_secs = value,
            );
            self.apply_if_allowed(
                SettingScope::Overridable,
                layer,
                "autosave.generations",
                path,
                autosave.generations,
                |settings, value| settings.autosave_generations = value,
            );
            self.apply_if_allowed(
                SettingScope::UserOnly,
                layer,
                "autosave.max_total_bytes",
                path,
                autosave.max_total_bytes,
                |settings, value| settings.autosave_max_total_bytes = value,
            );
        }
    }

    /// スコープが許す場合だけ値を適用します。
    /// スコープ外の値は警告ログに残して無視します。
    fn apply_if_allowed<T>(
        &mut self,
        scope: SettingScope,
        layer: Layer,
        key: &'static str,
        path: &Path,
        value: Option<T>,
        apply: impl FnOnce(&mut Self, T),
    ) {
        let Some(value) = value else {
            return;
        };
        if !scope.allows(layer) {
            log::warn!(
                "ignoring setting `{key}` in {}: not allowed in the {:?} scope",
                path.display(),
                layer
            );
            return;
        }
        apply(self, value);
    }
}

/// TOML ファイルを部分設定として読み込みます。
///
/// 存在しないファイルは空（何も指定なし）として扱います。
/// 未知のキーは無視します（警告ログに残します）。
fn load_partial(path: &Path) -> anyhow::Result<PartialAppSettings> {
    if !path.exists() {
        return Ok(PartialAppSettings::default());
    }
    let text = fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&text)?;
    if let Some(table) = value.as_table() {
        for key in table.keys() {
            if key != "chunk" && key != "autosave" {
                log::warn!("ignoring unknown settings section `{key}` in {}", path.display());
            }
        }
        if let Some(chunk) = table.get("chunk").and_then(|v| v.as_table()) {
            for key in chunk.keys() {
                if !matches!(
                    key.as_str(),
                    "default_width_ticks"
                        | "width_ticks"
                        | "unload_after_secs"
                        | "keep_frequently_used"
                ) {
                    log::warn!(
                        "ignoring unknown setting `chunk.{key}` in {}",
                        path.display()
                    );
                }
            }
        }
        if let Some(autosave) = table.get("autosave").and_then(|v| v.as_table()) {
            for key in autosave.keys() {
                if !matches!(key.as_str(), "interval_secs" | "generations" | "max_total_bytes")
                {
                    log::warn!(
                        "ignoring unknown setting `autosave.{key}` in {}",
                        path.display()
                    );
                }
            }
        }
    }
    let partial: PartialAppSettings = value.try_into()?;
    Ok(partial)
}

#[cfg(test)]
#[path = "../tests/unit/settings_tests.rs"]
mod tests;
