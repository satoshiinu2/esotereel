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

use crate::project::chunk::CHUNK_TICKS;

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
pub enum Layer {
    User,
    Project,
    Server,
}

impl SettingScope {
    /// この層がこのスコープのキーを設定できるかどうか。
    pub fn allows(self, layer: Layer) -> bool {
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

/// 設定値の型です。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingValueKind {
    Int,
    UInt,
    Bool,
}

/// 設定値です（層ファイルの読み書きと、エディタ表示用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingValue {
    Int(i64),
    UInt(u64),
    Bool(bool),
}

impl SettingValue {
    /// この値の型を返します。
    pub fn kind(&self) -> SettingValueKind {
        match self {
            SettingValue::Int(_) => SettingValueKind::Int,
            SettingValue::UInt(_) => SettingValueKind::UInt,
            SettingValue::Bool(_) => SettingValueKind::Bool,
        }
    }

    /// エディタ表示・入力用の文字列表現を返します。
    pub fn to_display_string(&self) -> String {
        match self {
            SettingValue::Int(value) => value.to_string(),
            SettingValue::UInt(value) => value.to_string(),
            SettingValue::Bool(value) => value.to_string(),
        }
    }

    /// 文字列表現から、型に合わせて値を解釈します。
    pub fn parse_as(kind: SettingValueKind, text: &str) -> Option<SettingValue> {
        match kind {
            SettingValueKind::Int => text.parse().ok().map(SettingValue::Int),
            SettingValueKind::UInt => text.parse().ok().map(SettingValue::UInt),
            SettingValueKind::Bool => text.parse().ok().map(SettingValue::Bool),
        }
    }
}

/// 宣言済みキーの情報です（エディタが一覧表示に使います）。
#[derive(Debug, Clone, Copy)]
pub struct SettingKeyInfo {
    /// ドット区切りのキー名（例: `chunk.unload_after_secs`）。
    pub key: &'static str,
    /// このキーが設定できるスコープ。
    pub scope: SettingScope,
    /// 値の型。
    pub kind: SettingValueKind,
    /// 既定値。`chunk.width_ticks` のように「未設定」が既定のキーは `None`。
    pub default_value: Option<SettingValue>,
}

/// 宣言済みの全キーを返します。
pub fn declared_keys() -> Vec<SettingKeyInfo> {
    let defaults = AppSettings::default();
    vec![
        SettingKeyInfo {
            key: "chunk.default_width_ticks",
            scope: SettingScope::UserOnly,
            kind: SettingValueKind::Int,
            default_value: Some(SettingValue::Int(defaults.chunk_default_width_ticks)),
        },
        SettingKeyInfo {
            key: "chunk.width_ticks",
            scope: SettingScope::ProjectOrServerOnly,
            kind: SettingValueKind::Int,
            default_value: None,
        },
        SettingKeyInfo {
            key: "chunk.unload_after_secs",
            scope: SettingScope::Overridable,
            kind: SettingValueKind::UInt,
            default_value: Some(SettingValue::UInt(defaults.chunk_unload_after_secs)),
        },
        SettingKeyInfo {
            key: "chunk.keep_frequently_used",
            scope: SettingScope::Overridable,
            kind: SettingValueKind::Bool,
            default_value: Some(SettingValue::Bool(defaults.chunk_keep_frequently_used)),
        },
        SettingKeyInfo {
            key: "autosave.interval_secs",
            scope: SettingScope::UserOnly,
            kind: SettingValueKind::UInt,
            default_value: Some(SettingValue::UInt(defaults.autosave_interval_secs)),
        },
        SettingKeyInfo {
            key: "autosave.generations",
            scope: SettingScope::Overridable,
            kind: SettingValueKind::UInt,
            default_value: Some(SettingValue::UInt(u64::from(defaults.autosave_generations))),
        },
        SettingKeyInfo {
            key: "autosave.max_total_bytes",
            scope: SettingScope::UserOnly,
            kind: SettingValueKind::UInt,
            default_value: Some(SettingValue::UInt(defaults.autosave_max_total_bytes)),
        },
    ]
}

impl AppSettings {
    /// 解決済みの実効値をキー名で引きます。未知のキーは `None` です。
    pub fn value_of(&self, key: &str) -> Option<SettingValue> {
        match key {
            "chunk.default_width_ticks" => Some(SettingValue::Int(self.chunk_default_width_ticks)),
            "chunk.width_ticks" => self.chunk_width_ticks.map(SettingValue::Int),
            "chunk.unload_after_secs" => Some(SettingValue::UInt(self.chunk_unload_after_secs)),
            "chunk.keep_frequently_used" => {
                Some(SettingValue::Bool(self.chunk_keep_frequently_used))
            }
            "autosave.interval_secs" => Some(SettingValue::UInt(self.autosave_interval_secs)),
            "autosave.generations" => Some(SettingValue::UInt(u64::from(self.autosave_generations))),
            "autosave.max_total_bytes" => Some(SettingValue::UInt(self.autosave_max_total_bytes)),
            _ => None,
        }
    }
}

/// 層ファイル（`settings.toml`）から、宣言済みキーの生値を読みます。
///
/// ファイルが無ければ空です。型が宣言と違う値は警告を残して除きます。
pub fn read_layer_values(path: &Path) -> anyhow::Result<Vec<(String, SettingValue)>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&text)?;
    let mut values = Vec::new();
    for info in declared_keys() {
        let Some((section, leaf)) = info.key.split_once('.') else {
            continue;
        };
        let Some(raw) = value.get(section).and_then(|s| s.get(leaf)) else {
            continue;
        };
        let parsed = match info.kind {
            SettingValueKind::Int => raw.as_integer().map(SettingValue::Int),
            SettingValueKind::UInt => raw
                .as_integer()
                .and_then(|v| u64::try_from(v).ok())
                .map(SettingValue::UInt),
            SettingValueKind::Bool => raw.as_bool().map(SettingValue::Bool),
        };
        match parsed {
            Some(parsed) => values.push((info.key.to_string(), parsed)),
            None => {
                log::warn!(
                    "ignoring setting `{}` in {}: type does not match the declaration",
                    info.key,
                    path.display()
                );
            }
        }
    }
    Ok(values)
}

/// 層ファイルへキーを書き込みます（`None` でそのキーを削除＝既定値へ
/// リセット）。他のキー（未知のキーを含む）は保持します。
pub fn write_layer_value(
    path: &Path,
    key: &str,
    value: Option<SettingValue>,
) -> anyhow::Result<()> {
    let (section, leaf) = key
        .split_once('.')
        .ok_or_else(|| anyhow::anyhow!("invalid setting key `{key}`"))?;
    if !declared_keys().iter().any(|info| info.key == key) {
        anyhow::bail!("unknown setting key `{key}`");
    }
    let mut doc = if path.exists() {
        fs::read_to_string(path)?.parse::<toml_edit::DocumentMut>()?
    } else {
        toml_edit::DocumentMut::new()
    };
    match value {
        Some(value) => {
            let toml_value = match value {
                SettingValue::Int(v) => toml_edit::Value::from(v),
                SettingValue::UInt(v) => {
                    let v = i64::try_from(v)
                        .map_err(|_| anyhow::anyhow!("value {v} does not fit in TOML integers"))?;
                    toml_edit::Value::from(v)
                }
                SettingValue::Bool(v) => toml_edit::Value::from(v),
            };
            let table = doc
                .entry(section)
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
                .as_table_mut()
                .ok_or_else(|| anyhow::anyhow!("settings section `{section}` is not a table"))?;
            table[leaf] = toml_edit::Item::Value(toml_value);
        }
        None => {
            if let Some(table) = doc.get_mut(section).and_then(|item| item.as_table_mut()) {
                table.remove(leaf);
            }
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, doc.to_string())?;
    Ok(())
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
