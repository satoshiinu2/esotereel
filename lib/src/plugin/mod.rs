use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::Context;
use log;
use regex::Regex;
use rkyv::{Archive, CheckBytes, bytecheck};

use crate::{
    HostRole,
    dirs::Directories,
    plugin::{
        clip::ClipKindStore,
        property::PropertySchema,
        script::CompiledScript,
        toolbar::ToolbarButtonSpec,
    },
    project::clip::ClipKind,
};

pub mod clip;
pub mod property;
pub mod registry;
pub mod script;
pub mod settings;
pub mod toolbar;

#[derive(
    Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[archive_attr(derive(Hash, Eq, PartialEq, CheckBytes))]
pub struct NamespacedID {
    full: String,
    plugin_id: String,
    local_id: String,
}

static ID_VALIDATION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9._-]+$").unwrap());

impl NamespacedID {
    pub fn new(plugin_id: &str, local_id: &str) -> anyhow::Result<Self> {
        if !ID_VALIDATION_REGEX.is_match(plugin_id) || !ID_VALIDATION_REGEX.is_match(local_id) {
            anyhow::bail!(
                "plugin_id/local_id must contain only ASCII letters, digits, '.', '_' or '-': {plugin_id}:{local_id}"
            );
        }
        Ok(Self {
            full: format!("{plugin_id}:{local_id}"),
            plugin_id: plugin_id.to_string(),
            local_id: local_id.to_string(),
        })
    }

    pub fn parse(full: &str) -> anyhow::Result<Self> {
        let (plugin_id, local_id) = full
            .split_once(':')
            .with_context(|| format!("invalid namespaced id (missing ':'): {full}"))?;
        Ok(Self {
            full: full.to_string(),
            plugin_id: plugin_id.to_string(),
            local_id: local_id.to_string(),
        })
    }

    pub fn full(&self) -> &str {
        &self.full
    }
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }
    pub fn local_id(&self) -> &str {
        &self.local_id
    }
}

impl PartialEq for NamespacedID {
    fn eq(&self, other: &Self) -> bool {
        self.full == other.full
    }
}
impl Eq for NamespacedID {}
impl std::hash::Hash for NamespacedID {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.full.hash(state);
    }
}

impl std::fmt::Display for NamespacedID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(&self.full)
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
}

pub struct Plugin {
    pub manifest: PluginManifest,
    pub setting_schemas: Vec<PropertySchema>,
    pub clip_kinds: HashMap<NamespacedID, ClipKind>,
    pub toolbar_buttons: Vec<ToolbarButtonSpec>,
    pub script: Option<CompiledScript>,
    pub dir: PathBuf,
}

impl Plugin {
    /// このプラグイン単体のディレクトリからmanifest+settings.tomlを読み込む
    fn load(dir: &Path) -> anyhow::Result<Self> {
        let manifest = Self::load_manifest(&dir.join("manifest.toml"))?;

        let setting_schemas = Self::load_settings(&manifest, &dir.join("settings.toml"))?;
        let clip_kinds = Self::load_clip_kinds(&manifest, &dir.join("clips"))?;
        let toolbar_buttons = Self::load_toolbar(&manifest, &dir.join("toolbars.toml"))?;
        let script = Self::load_script(&manifest, &dir, &dir.join("script.rhai"))?;

        let avaliable_functions = script
            .as_ref()
            .map_or_default(|s| s.available_functions.clone());

        log::info!(
            "Plugin '{}' v{} (ID: {}) loaded successfully: Settings schemas: {}, Clip kinds: {}, Toolbar buttons: {}, Available scripts: {}",
            manifest.name,
            manifest.version,
            manifest.id,
            setting_schemas.len(),
            clip_kinds.len(),
            toolbar_buttons.len(),
            avaliable_functions.len()
        );

        log::info!(
            "Plugin '{}' debug info: \nSettings schemas: {:?}, \nClip kinds: {:?}, \nToolbar buttons: {:?}, \nAvailable scripts: {:?}",
            manifest.id,
            setting_schemas
                .iter()
                .map(|s| s.key.full())
                .collect::<Vec<_>>(),
            clip_kinds
                .iter()
                .map(|(id, _)| id.full())
                .collect::<Vec<_>>(),
            toolbar_buttons.iter().map(|b| &b.id).collect::<Vec<_>>(),
            avaliable_functions
        );

        Ok(Self {
            manifest,
            setting_schemas,
            clip_kinds,
            toolbar_buttons,
            script,
            dir: dir.to_owned(),
        })
    }

    fn load_manifest(manifest_path: &Path) -> anyhow::Result<PluginManifest> {
        let manifest_text = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read manifest at {}", manifest_path.display()))?;
        let manifest: PluginManifest = toml::from_str(&manifest_text)
            .with_context(|| format!("failed to parse manifest at {}", manifest_path.display()))?;

        Ok(manifest)
    }

    fn load_settings(
        manifest: &PluginManifest,
        settings_path: &Path,
    ) -> anyhow::Result<Vec<PropertySchema>> {
        let schema = if settings_path.exists() {
            let text = std::fs::read_to_string(&settings_path).with_context(|| {
                format!(
                    "failed to read settings schema at {}",
                    settings_path.display()
                )
            })?;
            PropertySchema::parse_toml(&text, &manifest.id)
                .with_context(|| format!("invalid settings schema for plugin `{}`", manifest.id))?
        } else {
            Vec::new()
        };

        Ok(schema)
    }

    fn load_toolbar(
        manifest: &PluginManifest,
        toolbars_path: &Path,
    ) -> anyhow::Result<Vec<ToolbarButtonSpec>> {
        let toolbar_buttons = if toolbars_path.exists() {
            let text = std::fs::read_to_string(&toolbars_path).with_context(|| {
                format!(
                    "failed to read toolbars schema at {}",
                    toolbars_path.display()
                )
            })?;
            ToolbarButtonSpec::parse_toml(&text, &manifest.id)
                .with_context(|| format!("invalid toolbars schema for plugin `{}`", manifest.id))?
        } else {
            Vec::new()
        };
        let toolbar_buttons = Self::namespaced_toolbar_buttons(&manifest.id, toolbar_buttons);

        Ok(toolbar_buttons)
    }

    fn load_script(
        _manifest: &PluginManifest,
        plugin_path: &Path,
        script_path: &Path,
    ) -> anyhow::Result<Option<CompiledScript>> {
        if script_path.exists() {
            // 並列で動かしているので毎回作成
            let compiled = CompiledScript::compile(
                plugin_path, // モジュールの解決パス (プラグインのルートディレクトリ)
                &script_path.to_path_buf(),
            )?;
            Ok(Some(compiled))
        } else {
            Ok(None)
        }
    }

    fn load_clip_kinds(
        manifest: &PluginManifest,
        clips_dir: &Path,
    ) -> anyhow::Result<HashMap<NamespacedID, ClipKind>> {
        if !clips_dir.exists() {
            return Ok(HashMap::new());
        }

        let mut toml_paths: Vec<PathBuf> = std::fs::read_dir(clips_dir)
            .with_context(|| format!("failed to read clips directory {}", clips_dir.display()))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();

        // OS依存の走査順に結果が左右されないよう固定順にしておく
        toml_paths.sort();

        let mut all_kinds = HashMap::new();
        for path in toml_paths {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read clip kind file at {}", path.display()))?;
            let kinds = ClipKind::parse_toml(&text, &manifest.id).with_context(|| {
                format!(
                    "invalid clip kinds in `{}` for plugin `{}`",
                    path.display(),
                    manifest.id
                )
            })?;
            all_kinds.extend(kinds);
        }

        // 個別ファイル内の重複はparse_toml側で検証済みなので、
        // ここではファイルをまたいだプラグイン全体でのid重複だけを見る
        ClipKind::validate_kinds(&all_kinds)
            .with_context(|| format!("duplicate clip kind id across files in `{}`", manifest.id))?;

        Ok(all_kinds)
    }

    fn namespaced_toolbar_buttons(
        plugin_id: &str,
        mut buttons: Vec<crate::plugin::toolbar::ToolbarButtonSpec>,
    ) -> Vec<crate::plugin::toolbar::ToolbarButtonSpec> {
        for b in &mut buttons {
            b.id = format!("{plugin_id}.{}", b.id);
        }
        buttons
    }
}

// 内部で使う読み込みのための結果
struct PluginLoadingResult {
    pub dir: PathBuf,
    pub result: anyhow::Result<Plugin>,
}

// 読み込みの結果を渡すための結果
pub struct PluginLoadedResult {
    pub dir: PathBuf,
    pub result: anyhow::Result<PluginManifest>,
}

pub struct PluginLoader {
    pub plugins: Vec<Plugin>,
    is_loaded: bool,
    // インデックスで検索コストを最小化
    clip_kinds: ClipKindStore,
    scripts_index: HashMap<String, CompiledScript>,
    schemas_index: Vec<PropertySchema>,
    toolbars_index: Vec<(String, ToolbarButtonSpec)>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            is_loaded: false,
            clip_kinds: ClipKindStore::new(),
            scripts_index: HashMap::new(),
            schemas_index: Vec::new(),
            toolbars_index: Vec::new(),
        }
    }

    fn discover_all_plugin_dirs(dirs_def: &Directories) -> anyhow::Result<Vec<PathBuf>> {
        let mut dirs: Vec<PathBuf> = Vec::new();

        // 1. アプリ同梱の組み込みプラグイン(実行ファイル隣接)
        let std_plugins = dirs_def.std_plugins_dir()?;
        if std_plugins.exists() {
            dirs.extend(Self::discover_plugins(&std_plugins)?);
        } else {
            std::fs::create_dir_all(&std_plugins)?;
        }

        // 2. ユーザーディレクトリのプラグイン
        let user_plugins = dirs_def.user_plugins_dir()?;
        if user_plugins.exists() {
            dirs.extend(Self::discover_plugins(&user_plugins)?);
        } else {
            std::fs::create_dir_all(&user_plugins)?;
        }

        Ok(dirs)
    }

    fn discover_plugins(plugins_root: &Path) -> anyhow::Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        for entry in std::fs::read_dir(plugins_root).with_context(|| {
            format!(
                "failed to read plugins directory {}",
                plugins_root.display()
            )
        })? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                dirs.push(entry.path());
            }
        }
        Ok(dirs)
    }

    /// ディスクから全プラグインを読み込む。1つの失敗が全体を止めないよう、
    /// 個別にResultを保持したまま返す。
    pub async fn load_from_disk(
        &mut self,
        dirs_def: &Directories,
        role: HostRole,
    ) -> anyhow::Result<Vec<PluginLoadedResult>> {
        // 既に読み込み済みならキャッシュを返す
        if self.is_loaded {
            log::info!("Using cached plugins for {:?}", role);
            let results = self
                .plugins
                .iter()
                .map(|plugin| PluginLoadedResult {
                    dir: plugin.dir.clone(),
                    result: Ok(plugin.manifest.clone()),
                })
                .collect();
            return Ok(results);
        }

        log::info!("Starting plugin loading for {:?}", role);
        let plugin_dirs = Self::discover_all_plugin_dirs(dirs_def)?;
        log::info!("Discovered {} plugin directories", plugin_dirs.len());

        let mut tasks = vec![];
        for dir in plugin_dirs {
            let task = tokio::task::spawn_blocking(move || PluginLoadingResult {
                result: Plugin::load(&dir),
                dir,
            });

            tasks.push(task);
        }

        let mut results = Vec::with_capacity(tasks.len());
        for task in tasks {
            results.push(task.await);
        }

        let successful_count = results
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .filter(|r| r.result.is_ok())
            .count();
        log::info!(
            "Loaded {}/{} plugins successfully",
            successful_count,
            results.len()
        );

        // 失敗したプラグインのログを出力
        for result in &results {
            match result {
                Ok(result) => {
                    if let Err(e) = &result.result {
                        log::error!("Failed to load plugin from {}: {}", result.dir.display(), e);
                    }
                }
                Err(e) => {
                    log::error!("Failed to join plugin loading task: {}", e);
                }
            }
        }

        self.plugins = results
            .into_iter()
            .filter_map(|r| r.ok())
            .filter_map(|r| r.result.ok())
            .collect::<Vec<_>>();

        self.rebuild_indices();
        self.is_loaded = true;

        // 呼び出し側が個別の成否も見たい場合のために結果自体も返す
        // キャッシュ済みのプラグインからPluginLoadResultを再構築
        Ok(self
            .plugins
            .iter()
            .map(|plugin| PluginLoadedResult {
                dir: plugin.dir.clone(),
                result: Ok(plugin.manifest.clone()),
            })
            .collect())
    }

    fn rebuild_indices(&mut self) {
        self.clip_kinds = ClipKindStore::new();
        self.scripts_index.clear();
        self.schemas_index.clear();
        self.toolbars_index.clear();

        for plugin in &self.plugins {
            // clip_kinds
            self.clip_kinds
                .merge_plugin_kinds(plugin.clip_kinds.clone())
                .expect("clip kind conflict during rebuild_indices");

            // scripts index
            if let Some(script) = plugin.script.as_ref() {
                self.scripts_index
                    .insert(plugin.manifest.id.clone(), script.clone());
            }

            // schemas index
            for schema in plugin.setting_schemas.iter() {
                self.schemas_index.push(schema.clone());
            }

            // toolbars index
            for button in plugin.toolbar_buttons.iter() {
                self.toolbars_index
                    .push((plugin.manifest.id.clone(), button.clone()));
            }
        }
    }

    pub fn reload_plugin_by_id(&mut self, plugin_id: &str) -> anyhow::Result<()> {
        // 対象プラグインのディレクトリを取得
        let dir = self
            .plugins
            .iter()
            .find(|p| p.manifest.id == plugin_id)
            .map(|p| p.dir.clone())
            .ok_or_else(|| anyhow::anyhow!("Plugin `{}` not found in loaded plugins", plugin_id))?;

        // ディレクトリから再度読み込み (マニフェスト・設定の再パース)
        let reloaded_plugin = Plugin::load(&dir)
            .with_context(|| format!("Failed to hot-reload plugin `{}`", plugin_id))?;

        // 成功したら配列内の古いインスタンスを差し替え
        if let Some(index) = self.plugins.iter().position(|p| p.manifest.id == plugin_id) {
            self.plugins[index] = reloaded_plugin;
            self.rebuild_indices(); // インデックスを再構築
            log::info!("Reloaded plugin '{}'", plugin_id);
        }

        Ok(())
    }

    pub fn collect_all_schemas(&self) -> Vec<PropertySchema> {
        self.schemas_index.clone()
    }

    pub fn collect_all_clip_kinds(&self) -> HashMap<NamespacedID, ClipKind> {
        self.clip_kinds
            .registry
            .iter()
            .map(|(id, kind)| (id.clone(), kind.clone()))
            .collect()
    }

    pub fn collect_all_toolbars(&self) -> Vec<(String, ToolbarButtonSpec)> {
        self.toolbars_index.clone()
    }

    pub fn collect_all_scripts(&self) -> HashMap<String, CompiledScript> {
        self.scripts_index.clone()
    }

    // ホットパス用の直接アクセスメソッド
    pub fn get_clip_kind(&self, id: &NamespacedID) -> Option<&ClipKind> {
        self.clip_kinds.get(id)
    }

    pub fn get_script(&self, plugin_id: &str) -> Option<&CompiledScript> {
        self.scripts_index.get(plugin_id)
    }
}
