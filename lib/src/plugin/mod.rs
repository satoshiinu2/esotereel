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
    dirs::Directories,
    plugin::{
        clip::{
            ClipKindStore,
            component::{
                ComponentCapability, ComponentDefinition, ComponentRegistry, RuntimeDefinition,
                parse_component_definitions,
            },
        },
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
#[archive_attr(derive(CheckBytes))]
pub struct NamespacedID {
    full: String,
    plugin_id: String,
    local_id: String,
}

static ID_VALIDATION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_.:-]+$").unwrap());

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

// rkyv builds hash-map indices using the original key, then validates and looks up
// entries using the archived key. Both types must hash and compare the same fields.
impl PartialEq for ArchivedNamespacedID {
    fn eq(&self, other: &Self) -> bool {
        self.full == other.full
    }
}
impl Eq for ArchivedNamespacedID {}
impl std::hash::Hash for ArchivedNamespacedID {
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
    pub component_definitions: HashMap<NamespacedID, ComponentDefinition>,
    pub component_scripts: HashMap<NamespacedID, CompiledScript>,
    pub toolbar_buttons: Vec<ToolbarButtonSpec>,
    pub toolbar_button_scripts: HashMap<NamespacedID, CompiledScript>,
    pub script: Option<CompiledScript>,
    pub dir: PathBuf,
}

impl Plugin {
    /// このプラグイン単体のディレクトリからmanifest+settings.tomlを読み込む
    fn load(dir: &Path) -> anyhow::Result<Self> {
        let manifest = Self::load_manifest(&dir.join("manifest.toml"))?;

        let setting_schemas = Self::load_settings(&manifest, &dir.join("settings.toml"))?;
        let clip_kinds = Self::load_clip_kinds(&manifest, &dir.join("clips"))?;
        let (component_definitions, mut component_scripts) =
            Self::load_components(&manifest, &dir.join("components"))?;
        let toolbar_buttons = Self::load_toolbar(&manifest, &dir.join("toolbars.toml"))?;
        let mut toolbar_button_scripts = HashMap::new();
        for button in &toolbar_buttons {
            let path = dir.join(&button.script);
            if path.exists() {
                let compiled = CompiledScript::compile(dir, &path)?;
                toolbar_button_scripts.insert(button.id.clone(), compiled);
            }
        }
        let script = Self::load_script(&manifest, &dir, &dir.join("script.rhai"))?;

        Ok(Self {
            manifest,
            setting_schemas,
            clip_kinds,
            component_definitions,
            component_scripts,
            toolbar_buttons,
            toolbar_button_scripts,
            script,
            dir: dir.to_path_buf(),
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

        Ok(toolbar_buttons)
    }

    fn load_script(
        _manifest: &PluginManifest,
        plugin_path: &Path,
        script_path: &Path,
    ) -> anyhow::Result<Option<CompiledScript>> {
        if script_path.exists() {
            // エンジンは ScriptEngines のプロセス共有キャッシュから取得する。
            let compiled = CompiledScript::compile(
                plugin_path, // モジュールの解決パス (プラグインのルートディレクトリ)
                &script_path.to_path_buf(),
            )?;
            Ok(Some(compiled))
        } else {
            Ok(None)
        }
    }

    /// clips/ 配下の全TOMLからkindsを集める。
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

        toml_paths.sort();

        let mut all_kinds = HashMap::new();
        for path in toml_paths {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read clip kind file at {}", path.display()))?;
            let kinds = ClipKind::parse_toml(&text, &manifest.id)
                .with_context(|| {
                    format!(
                        "failed parse clip kinds in `{}` for plugin `{}`",
                        path.display(),
                        manifest.id
                    )
                })?;

            for (id, kind) in kinds {
                if all_kinds.insert(id.clone(), kind).is_some() {
                    anyhow::bail!(
                        "duplicate clip kind id `{}` across files in `{}`",
                        id,
                        manifest.id
                    );
                }
            }
        }

        Ok(all_kinds)
    }

    /// components/ 配下の全TOMLからコンポーネント定義とそのスクリプトを集める。
    fn load_components(
        manifest: &PluginManifest,
        components_dir: &Path,
    ) -> anyhow::Result<(
        HashMap<NamespacedID, ComponentDefinition>,
        HashMap<NamespacedID, CompiledScript>,
    )> {
        if !components_dir.exists() {
            return Ok((HashMap::new(), HashMap::new()));
        }

        let mut toml_paths: Vec<PathBuf> = std::fs::read_dir(components_dir)
            .with_context(|| {
                format!(
                    "failed to read components directory {}",
                    components_dir.display()
                )
            })?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();

        toml_paths.sort();

        let mut all_components = HashMap::new();
        let mut all_scripts = HashMap::new();
        for path in toml_paths {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read component file at {}", path.display()))?;
            let components = parse_component_definitions(&text, &manifest.id)
                .with_context(|| {
                    format!(
                        "failed parse components in `{}` for plugin `{}`",
                        path.display(),
                        manifest.id
                    )
                })?;

            for (id, def) in &components {
                if all_components.insert(id.clone(), def.clone()).is_some() {
                    anyhow::bail!(
                        "duplicate component definition id `{}` across files in `{}`",
                        id,
                        manifest.id
                    );
                }
                if let Some(script_path) = &def.script {
                    let script_path = components_dir.join(script_path);
                    if script_path.exists() {
                        let compiled = CompiledScript::compile(components_dir, &script_path)?;
                        all_scripts.insert(id.clone(), compiled);
                    }
                }
            }
        }

        Ok((all_components, all_scripts))
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
    clip_kinds: ClipKindStore,
    component_registry: ComponentRegistry,

    // インデックスで検索コストを最小化
    scripts_index: HashMap<String, CompiledScript>,
    component_scripts_index: HashMap<NamespacedID, CompiledScript>,
    toolbar_button_scripts_index: HashMap<NamespacedID, CompiledScript>,
    schemas_index: Vec<PropertySchema>,
    toolbars_index: Vec<(String, ToolbarButtonSpec)>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            is_loaded: false,
            clip_kinds: ClipKindStore::new(),
            component_registry: ComponentRegistry::default(),
            scripts_index: HashMap::new(),
            component_scripts_index: HashMap::new(),
            toolbar_button_scripts_index: HashMap::new(),
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
    ) -> anyhow::Result<Vec<PluginLoadedResult>> {
        // 既に読み込み済みならキャッシュを返す
        // ScriptEnginesはClient/Server両方を持っているので共有OK
        if self.is_loaded {
            log::info!("Using cached plugins");
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

        log::info!("Starting plugin loading");
        let plugin_dirs = Self::discover_all_plugin_dirs(dirs_def)?;
        log::info!("Discovered {} plugin directories", plugin_dirs.len());

        let mut tasks = vec![];
        for dir in plugin_dirs {
            let dir_clone = dir.clone();
            let task = tokio::task::spawn_blocking(move || {
                let result = Plugin::load(&dir_clone);
                PluginLoadingResult {
                    result,
                    dir: dir_clone,
                }
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

        self.rebuild_indices()?;
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

    /// 全プラグインのclip_kindsをマージし、その後で全プラグインの
    /// 全プラグインのkindsとcomponent_definitionsをマージし、インデックスを再構築する。
    fn rebuild_indices(&mut self) -> anyhow::Result<()> {
        self.scripts_index.clear();
        self.component_scripts_index.clear();
        self.toolbar_button_scripts_index.clear();
        self.schemas_index.clear();
        self.toolbars_index.clear();
        self.component_registry = ComponentRegistry::default();

        // 1段階目: 全プラグインのkindsをマージ
        let mut merged_kinds: HashMap<NamespacedID, ClipKind> = HashMap::new();
        for plugin in &self.plugins {
            for (id, kind) in &plugin.clip_kinds {
                if merged_kinds.insert(id.clone(), kind.clone()).is_some() {
                    anyhow::bail!("duplicate clip kind id `{}` across plugins", id);
                }
            }
        }

        // 全プラグインのcomponent_definitionsをComponent Registry に登録する。
        // id 衝突があればエラーとし、どのプラグインがどのコンポーネントを
        // 所有するかを明確にする。
        let mut merged_components: HashMap<NamespacedID, ComponentDefinition> = HashMap::new();
        for plugin in &self.plugins {
            for (id, def) in &plugin.component_definitions {
                if merged_components.insert(id.clone(), def.clone()).is_some() {
                    anyhow::bail!("duplicate component definition id `{}` across plugins", id);
                }
            }
        }
        self.component_registry
            .merge_component_definitions(merged_components)?;

        let mut clip_kinds = ClipKindStore::new();
        clip_kinds
            .merge_plugin_kinds(merged_kinds)
            .context("clip kind conflict during rebuild_indices")?;
        self.clip_kinds = clip_kinds;

        for plugin in &self.plugins {
            // scripts index (プラグイン共通 script.rhai)
            if let Some(script) = plugin.script.as_ref() {
                self.scripts_index
                    .insert(plugin.manifest.id.clone(), script.clone());
            }
            // コンポーネントごとの個別スクリプト
            for (id, script) in &plugin.component_scripts {
                self.component_scripts_index.insert(id.clone(), script.clone());
            }
            // ツールバーボタンごとの個別スクリプト
            for (id, script) in &plugin.toolbar_button_scripts {
                self.toolbar_button_scripts_index.insert(id.clone(), script.clone());
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

        log::info!(
            "Plugin indices rebuilt: clip_kinds={}, scripts={}, schemas={}, toolbars={}",
            self.clip_kinds.registry.iter().count(),
            self.scripts_index.len(),
            self.schemas_index.len(),
            self.toolbars_index.len()
        );

        Ok(())
    }

    pub fn load_additional_plugin(&mut self, dir: &Path) -> anyhow::Result<PluginManifest> {
        let plugin = Plugin::load(dir)
            .with_context(|| format!("Failed to load plugin at {}", dir.display()))?;
        let manifest = plugin.manifest.clone();
        self.plugins.push(plugin);
        self.rebuild_indices()?; // 新規分も含めて全体を組み直す
        log::info!("Loaded additional plugin '{}'", manifest.id);
        Ok(manifest)
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
            // 他プラグインのpending propertiesが差し替え対象のkindを参照している
            // 可能性があるため、必ず全プラグイン分を再解決する
            self.rebuild_indices()?;
            log::info!("Reloaded plugin '{}'", plugin_id);
        }

        Ok(())
    }

    pub fn unload_plugin_by_id(&mut self, plugin_id: &str) -> anyhow::Result<()> {
        let before = self.plugins.len();
        self.plugins.retain(|p| p.manifest.id != plugin_id);
        if self.plugins.len() == before {
            anyhow::bail!("Plugin `{}` not found in loaded plugins", plugin_id);
        }
        self.rebuild_indices()?; // 依存解決のため全体を組み直す
        log::info!("Unloaded plugin '{}'", plugin_id);
        Ok(())
    }

    pub fn collect_all_schemas(&self) -> Vec<PropertySchema> {
        self.schemas_index.clone()
    }

    pub fn collect_all_toolbars(&self) -> Vec<(String, ToolbarButtonSpec)> {
        self.toolbars_index.clone()
    }

    // ホットパス用の直接アクセスメソッド
    pub fn get_clip_kind(&self, id: &NamespacedID) -> Option<&ClipKind> {
        self.clip_kinds.get(id)
    }

    /// 指定 id のクリップ（ルートコンポーネント）のプロパティスキーマを返す。
    /// プロパティはクリップ側ではなく、ルートコンポーネントが提供する。
    pub fn get_clip_properties(&self, id: &NamespacedID) -> Option<&[PropertySchema]> {
        self.component_registry
            .get(id)
            .map(|c| c.properties.as_slice())
    }

    /// 指定 id の ComponentDefinition を返す。
    pub fn get_component(&self, id: &NamespacedID) -> Option<&ComponentDefinition> {
        self.component_registry.get(id)
    }

    /// ルートコンポーネント id から階層を解決し、フラットな RuntimeDefinition を返す。
    pub fn resolve_component_definition(
        &self,
        root_id: &NamespacedID,
    ) -> anyhow::Result<RuntimeDefinition> {
        self.component_registry.resolve_definition(root_id)
    }

    /// 指定 Capability を提供する ComponentDefinition をすべて返す。
    pub fn get_components_by_capability(
        &self,
        capability: &ComponentCapability,
    ) -> Vec<&ComponentDefinition> {
        self.component_registry.by_capability(capability)
    }

    pub fn get_settings_schemas(&self) -> &[PropertySchema] {
        &self.schemas_index
    }

    pub fn get_toolbar_buttons(&self) -> &[(String, ToolbarButtonSpec)] {
        &self.toolbars_index
    }

    pub fn get_script(&self, plugin_id: &str) -> Option<&CompiledScript> {
        self.scripts_index.get(plugin_id)
    }

    /// コンポーネントごとの個別スクリプトを返す。
    pub fn get_component_script(&self, id: &NamespacedID) -> Option<&CompiledScript> {
        self.component_scripts_index.get(id)
    }

    /// ツールバーボタンごとの個別スクリプトを返す。
    pub fn get_toolbar_button_script(&self, id: &NamespacedID) -> Option<&CompiledScript> {
        self.toolbar_button_scripts_index.get(id)
    }

    pub fn call_script<R: Clone + Send + Sync + 'static>(
        &self,
        engine: &rhai::Engine,
        plugin_id: &str,
        fn_name: &str,
        args: impl rhai::FuncArgs,
    ) -> anyhow::Result<R> {
        let script = self
            .scripts_index
            .get(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_id))?;

        script.call(engine, fn_name, args)
    }
}

#[cfg(test)]
mod archive_tests {
    use super::*;
    use crate::plugin::property::value::FieldValue;
    use crate::{
        project::{
            clip::{ArchivedClipBindingValue, ClipBindingValue},
            command::{ArchivedCommandRequest, CommandRequest},
        },
        requests::{ArchivedRequest, Request},
    };
    use std::{
        collections::VecDeque,
        hash::{Hash, Hasher},
    };

    #[test]
    fn namespaced_id_has_same_hash_before_and_after_archiving() {
        let id = NamespacedID::new("std", "source").unwrap();
        let bytes = rkyv::to_bytes::<_, 256>(&id).unwrap();
        let archived = rkyv::check_archived_root::<NamespacedID>(&bytes).unwrap();
        let mut original_hash = std::collections::hash_map::DefaultHasher::new();
        let mut archived_hash = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut original_hash);
        archived.hash(&mut archived_hash);
        assert_eq!(original_hash.finish(), archived_hash.finish());
    }

    #[test]
    fn add_clip_properties_survive_network_validation_and_lookup() {
        for count in [1, 2, 7, 32] {
            let properties = (0..count)
                .map(|i| {
                    (
                        NamespacedID::new("std", &format!("field_{i}")).unwrap(),
                        ClipBindingValue::Static(FieldValue::Int(i)),
                    )
                })
                .collect();
            let request = Request::Command {
                commands: VecDeque::from([(
                    1,
                    CommandRequest::AddClip {
                        layer_id: 1,
                        position: 0,
                        duration: 60,
                        kind_id: NamespacedID::new("std", "video").unwrap(),
                        properties,
                    },
                )]),
            };
            let bytes = rkyv::to_bytes::<_, 1024>(&request).unwrap();
            let archived = rkyv::check_archived_root::<Request>(&bytes).unwrap();
            let ArchivedRequest::Command { commands } = archived else {
                panic!("expected command")
            };
            let ArchivedCommandRequest::AddClip { properties, .. } = &commands[0].1 else {
                panic!("expected AddClip")
            };
            assert_eq!(properties.len(), count as usize);
            for i in 0..count {
                let key = NamespacedID::new("std", &format!("field_{i}")).unwrap();
                let key_bytes = rkyv::to_bytes::<_, 256>(&key).unwrap();
                let archived_key = rkyv::check_archived_root::<NamespacedID>(&key_bytes).unwrap();
                assert!(
                    matches!(properties.get(archived_key), Some(ArchivedClipBindingValue::Static(crate::plugin::property::value::ArchivedFieldValue::Int(value))) if *value == i)
                );
            }
            let decoded = rkyv::from_bytes::<Request>(&bytes).unwrap();
            let Request::Command { commands } = decoded else {
                panic!("expected command")
            };
            let CommandRequest::AddClip { properties, .. } = &commands[0].1 else {
                panic!("expected AddClip")
            };
            assert_eq!(properties.len(), count as usize);
        }
    }
}
