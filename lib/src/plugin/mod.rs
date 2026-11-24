//! プラグインシステムのモジュールです。
//!
//! プラグインは `manifest.toml`、`settings.toml`、`clips/`、`components/`、`shaders/`、
//! `toolbars.toml`、`script.rhai` で構成され、`PluginLoader` によりディスクから読み込まれます。
//! 各プラグインは `ClipKind`（クリップ種別）、`ComponentDefinition`（コンポーネント定義）、
//! `PropertySchema`（プロパティスキーマ）、Rhai スクリプトを登録します。
//!
//! ## 設計上の注意
//! - プロパティ ID（`NamespacedID`）はハードコードせず、コンポーネントが宣言する
//!   能力（`ComponentCapability`）から解決して参照します（AGENTS.md 第9条）。
//! - プラグイン読み込みは `tokio::task::spawn_blocking` で並列実行され、
//!   1 つのプラグインの失敗が全体を止めません（`load_from_disk`）。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, RwLock},
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
        property::{PropertyKey, PropertySchema},
        script::CompiledScript,
        shader::{
            ShaderDefinition, ShaderRegistry, compile_and_validate, module_path_for,
            parse_shader_definition,
        },
        toolbar::ToolbarButtonSpec,
    },
    project::clip::ClipKind,
};

pub mod clip;
pub mod property;
pub mod registry;
pub mod script;
pub mod shader;
pub mod settings;
pub mod toolbar;

/// プラグインスコープ下の識別子を表す名前空間付き ID です。
///
/// 形式は `plugin_id:local_id`（例: `std:video`）。プラグイン間で ID が衝突しないよう、
/// プラグイン ID とローカル ID の組み合わせで一意性を保証します。
///
/// プロパティ ID やコンポーネント ID など、システム内の識別子はすべてこの型を介して
/// 表現し、ハードコードされたリテラル ID を使用しないこと（AGENTS.md 第9条）。
#[derive(
    Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub struct NamespacedID {
    /// `plugin_id:local_id` 形式のフル ID 文字列。
    full: String,
    /// プラグイン識別子。
    plugin_id: String,
    /// プラグイン内部でのローカル識別子。
    local_id: String,
}

static ID_VALIDATION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_.:-]+$").unwrap());

impl NamespacedID {
    /// `plugin_id:local_id` 形式の新しい `NamespacedID` を作成します。
    ///
    /// 入力は ASCII 英字・数字・`.` `_` `-` `:` のみを含む必要があります
    /// （`local_id` には `:` を含む階層形も使えます）。
    /// 不正な場合は `anyhow::bail!` でエラーを返します。
    pub fn new(plugin_id: &str, local_id: &str) -> anyhow::Result<Self> {
        if !ID_VALIDATION_REGEX.is_match(plugin_id) || !ID_VALIDATION_REGEX.is_match(local_id) {
            anyhow::bail!(
                "plugin_id/local_id must contain only ASCII letters, digits, '.', '_', '-' or ':': {plugin_id}:{local_id}"
            );
        }
        Ok(Self {
            full: format!("{plugin_id}:{local_id}"),
            plugin_id: plugin_id.to_string(),
            local_id: local_id.to_string(),
        })
    }

    /// `plugin_id:local_id` 形式の文字列をパースして `NamespacedID` を作成します。
    ///
    /// `:` 区切りがなければエラーとなります。各節は `new` と同じ検証を通る
    /// 必要があり、空の節や不正な文字を含む ID はエラーとなります。
    pub fn parse(full: &str) -> anyhow::Result<Self> {
        let (plugin_id, local_id) = full
            .split_once(':')
            .with_context(|| format!("invalid namespaced id (missing ':'): {full}"))?;
        Self::new(plugin_id, local_id)
            .with_context(|| format!("invalid namespaced id: {full}"))
    }

    /// フル ID 文字列（`plugin_id:local_id`）を返します。
    pub fn full(&self) -> &str {
        &self.full
    }

    /// プラグイン識別子を返します。
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// プラグイン内部のローカル識別子を返します。
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

/// プラグインのマニフェスト（`manifest.toml` の内容）を表します。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PluginManifest {
    /// プラグイン識別子（例: `std`）。
    pub id: String,
    /// プラグインの表示名。
    pub name: String,
    /// プラグインのバージョン。
    pub version: String,
}

/// 読み込まれたプラグインの実体です。
///
/// プラグインディレクトリからマニフェスト・設定スキーマ・クリップ種別・
/// コンポーネント定義・シェーダ定義・ツールバーボタン・スクリプトをすべて読み込み、保持します。
/// `PluginLoader` が所有し、他モジュールからは `PluginLoader` のアクセサ経由で
/// 間接的に参照することを前提とします。
pub struct Plugin {
    pub manifest: PluginManifest,
    pub setting_schemas: Vec<PropertySchema>,
    pub clip_kinds: HashMap<NamespacedID, ClipKind>,
    pub component_definitions: HashMap<NamespacedID, ComponentDefinition>,
    pub component_scripts: HashMap<NamespacedID, CompiledScript>,
    pub shader_definitions: HashMap<NamespacedID, Arc<ShaderDefinition>>,
    pub toolbar_buttons: Vec<ToolbarButtonSpec>,
    pub toolbar_button_scripts: HashMap<NamespacedID, CompiledScript>,
    pub script: Option<CompiledScript>,
    pub dir: PathBuf,
}

impl Plugin {
    /// このプラグイン単体のディレクトリから `manifest.toml` + `settings.toml` などを読み込み、
    /// `Plugin` インスタンスを構築します。
    fn load(dir: &Path) -> anyhow::Result<Self> {
        let manifest = Self::load_manifest(&dir.join("manifest.toml"))?;

        let setting_schemas = Self::load_settings(&manifest, &dir.join("settings.toml"))?;
        let clip_kinds = Self::load_clip_kinds(&manifest, &dir.join("clips"))?;
        let (component_definitions, mut component_scripts) =
            Self::load_components(&manifest, &dir.join("components"))?;
        let shader_definitions = Self::load_shaders(&manifest, &dir.join("shaders"))?;
        let toolbar_buttons = Self::load_toolbar(&manifest, &dir.join("toolbars.toml"))?;
        let mut toolbar_button_scripts = HashMap::new();
        for button in &toolbar_buttons {
            let path = dir.join(&button.script);
            if path.exists() {
                match CompiledScript::compile(dir, &path) {
                    Ok(compiled) => {
                        toolbar_button_scripts.insert(button.id.clone(), compiled);
                    }
                    Err(error) => {
                        log::error!(
                            "failed to compile toolbar script `{}` for plugin `{}`: {error:#}",
                            path.display(),
                            manifest.id
                        );
                    }
                }
            }
        }
        let script = Self::load_script(&manifest, &dir, &dir.join("script.rhai"))?;

        Ok(Self {
            manifest,
            setting_schemas,
            clip_kinds,
            component_definitions,
            component_scripts,
            shader_definitions,
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
            // 1ファイルの失敗でプラグイン全体を落とさない。壊れたファイルは
            // ログに残して読み飛ばす。
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) => {
                    log::error!("failed to read clip kind file at {}: {error}", path.display());
                    continue;
                }
            };
            let kinds = match ClipKind::parse_toml(&text, &manifest.id) {
                Ok(kinds) => kinds,
                Err(error) => {
                    log::error!(
                        "failed parse clip kinds in `{}` for plugin `{}`: {error:#}",
                        path.display(),
                        manifest.id
                    );
                    continue;
                }
            };

            for (id, kind) in kinds {
                if all_kinds.insert(id.clone(), kind).is_some() {
                    log::error!(
                        "duplicate clip kind id `{id}` across files in `{}`; keeping the first",
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
            // 1ファイルの失敗でプラグイン全体を落とさない（load_clip_kinds と同じ方針）。
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) => {
                    log::error!("failed to read component file at {}: {error}", path.display());
                    continue;
                }
            };
            let components = match parse_component_definitions(&text, &manifest.id) {
                Ok(components) => components,
                Err(error) => {
                    log::error!(
                        "failed parse components in `{}` for plugin `{}`: {error:#}",
                        path.display(),
                        manifest.id
                    );
                    continue;
                }
            };

            for (id, def) in &components {
                if all_components.contains_key(id) {
                    log::error!(
                        "duplicate component definition id `{id}` across files in `{}`; keeping the first",
                        manifest.id
                    );
                    continue;
                }
                all_components.insert(id.clone(), def.clone());
                if let Some(script_path) = &def.script {
                    let script_path = components_dir.join(script_path);
                    if script_path.exists() {
                        match CompiledScript::compile(components_dir, &script_path) {
                            Ok(compiled) => {
                                all_scripts.insert(id.clone(), compiled);
                            }
                            Err(error) => {
                                log::error!(
                                    "failed to compile component script `{}` for plugin `{}`: {error:#}",
                                    script_path.display(),
                                    manifest.id
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok((all_components, all_scripts))
    }

    /// shaders/ 配下の全TOMLからシェーダ定義を集め、WGSLをコンパイルする。
    fn load_shaders(
        manifest: &PluginManifest,
        shaders_dir: &Path,
    ) -> anyhow::Result<HashMap<NamespacedID, Arc<ShaderDefinition>>> {
        if !shaders_dir.exists() {
            return Ok(HashMap::new());
        }

        // import 解決用に、配下の全 WGSL を package:: モジュールとして登録する。
        // 定義 TOML の無いファイルは import 専用のヘルパモジュールとして扱う。
        let mut module_files: Vec<PathBuf> = walkdir::WalkDir::new(shaders_dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.path().to_path_buf())
            .filter(|path| path.extension().is_some_and(|ext| ext == "wgsl" || ext == "wesl"))
            .collect();
        module_files.sort();

        let mut package_modules: Vec<(String, String)> = Vec::new();
        for path in &module_files {
            // モジュール1つの失敗は、そのモジュールを import するシェーダ側の
            // コンパイルエラーとして表面化する。ここでは読み飛ばす。
            let loaded = (|| -> anyhow::Result<(String, String)> {
                let relative = path.strip_prefix(shaders_dir).with_context(|| {
                    format!("failed to resolve shader module path {}", path.display())
                })?;
                let module_path = module_path_for(relative)
                    .with_context(|| format!("invalid shader module path {}", path.display()))?;
                let source = std::fs::read_to_string(path).with_context(|| {
                    format!("failed to read shader source at {}", path.display())
                })?;
                Ok((module_path, source))
            })();
            match loaded {
                Ok(module) => package_modules.push(module),
                Err(error) => {
                    log::error!("skipping shader module {}: {error:#}", path.display());
                }
            }
        }

        let mut toml_paths: Vec<PathBuf> = std::fs::read_dir(shaders_dir)
            .with_context(|| format!("failed to read shaders directory {}", shaders_dir.display()))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        toml_paths.sort();

        let mut all_shaders = HashMap::new();
        for path in toml_paths {
            // 1ファイルの失敗でプラグイン全体を落とさない（load_clip_kinds と同じ方針）。
            let loaded = (|| -> anyhow::Result<ShaderDefinition> {
                let text = std::fs::read_to_string(&path).with_context(|| {
                    format!("failed to read shader file at {}", path.display())
                })?;
                let mut def = parse_shader_definition(&text, &manifest.id).with_context(|| {
                    format!(
                        "failed parse shader in `{}` for plugin `{}`",
                        path.display(),
                        manifest.id
                    )
                })?;

                let main_module = module_path_for(Path::new(&def.source)).with_context(|| {
                    format!(
                        "invalid shader source path `{}` for plugin `{}`",
                        def.source, manifest.id
                    )
                })?;
                def.compiled_wgsl = compile_and_validate(&main_module, &package_modules)
                    .with_context(|| {
                        format!(
                            "failed to compile shader `{}` for plugin `{}`",
                            def.id, manifest.id
                        )
                    })?;
                Ok(def)
            })();
            let def = match loaded {
                Ok(def) => def,
                Err(error) => {
                    log::error!("skipping shader file {}: {error:#}", path.display());
                    continue;
                }
            };

            let id = def.id.clone();
            if all_shaders.insert(id.clone(), Arc::new(def)).is_some() {
                log::error!(
                    "duplicate shader definition id `{id}` across files in `{}`; keeping the first",
                    manifest.id
                );
            }
        }

        Ok(all_shaders)
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

/// 全プラグインの読み込み・インデックス構築・アクセスを管理するローダーです。
///
/// `load_from_disk` で全プラグインを並列読み込みし、`rebuild_indices` で
/// `clip_kinds`・`component_definitions`・スクリプト・スキーマ・ツールバーを
/// マージ・インデックス化します。
///
/// ## スレッドモデル
/// プラグインの読み込みは `spawn_blocking` で並列化されますが、構築後の
/// `PluginLoader` 自体は `Arc<RwLock<PluginLoader>>` として共有され、
/// 読み取りは `read`、書き込み（再読み込み・追加・削除）は `write` で保護されます。
pub struct PluginLoader {
    pub plugins: Vec<Plugin>,
    is_loaded: bool,
    clip_kinds: ClipKindStore,
    component_registry: ComponentRegistry,
    shader_registry: ShaderRegistry,

    /// 解決済み RuntimeDefinition のキャッシュ。レジストリ再構築時にクリアする。
    resolved_definitions: RwLock<HashMap<NamespacedID, Arc<RuntimeDefinition>>>,

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
            shader_registry: ShaderRegistry::default(),
            resolved_definitions: RwLock::new(HashMap::new()),
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

    /// ディスクから全プラグインを読み込みます。
    ///
    /// 1 つのプラグインの失敗が全体を止めないよう、個別に `Result` を保持したまま返します。
    /// 既に読み込み済みの場合はキャッシュを返します。
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

    /// 全プラグインの `clip_kinds` と `component_definitions` をマージし、インデックスを再構築します。
    ///
    /// ID 衝突がある場合は先に読み込まれたプラグインを優先し、衝突したエントリだけを
    /// ログに残して落とします（1つの衝突で全プラグインが使えなくならないように）。
    /// 新しいインデックス一式はローカルに構築し、全部成功した場合にだけ現在の
    /// インデックスと差し替えます。失敗時は既存のインデックスが無傷で残ります。
    fn rebuild_indices(&mut self) -> anyhow::Result<()> {
        // 1段階目: 全プラグインのkindsをマージ
        // ID が衝突した場合は先勝ちとし、衝突した側だけを落とす。
        // 1つの衝突で全プラグインが使えなくなる事態を避けるため。
        let mut merged_kinds: HashMap<NamespacedID, ClipKind> = HashMap::new();
        for plugin in &self.plugins {
            for (id, kind) in &plugin.clip_kinds {
                if merged_kinds.contains_key(id) {
                    log::error!(
                        "duplicate clip kind id `{id}` from plugin `{}`; keeping the first",
                        plugin.manifest.id
                    );
                    continue;
                }
                merged_kinds.insert(id.clone(), kind.clone());
            }
        }

        // 全プラグインのcomponent_definitionsをComponent Registry に登録する。
        // id 衝突があればエラーとし、どのプラグインがどのコンポーネントを
        // 所有するかを明確にする。
        let mut merged_components: HashMap<NamespacedID, ComponentDefinition> = HashMap::new();
        for plugin in &self.plugins {
            for (id, def) in &plugin.component_definitions {
                if merged_components.contains_key(id) {
                    log::error!(
                        "duplicate component definition id `{id}` from plugin `{}`; keeping the first",
                        plugin.manifest.id
                    );
                    continue;
                }
                merged_components.insert(id.clone(), def.clone());
            }
        }
        let mut component_registry = ComponentRegistry::default();
        component_registry.merge_component_definitions(merged_components)?;

        // 全プラグインのshader_definitionsをマージする。id 衝突があればエラー。
        let mut merged_shaders: HashMap<NamespacedID, Arc<ShaderDefinition>> = HashMap::new();
        for plugin in &self.plugins {
            for (id, def) in &plugin.shader_definitions {
                if merged_shaders.contains_key(id) {
                    log::error!(
                        "duplicate shader definition id `{id}` from plugin `{}`; keeping the first",
                        plugin.manifest.id
                    );
                    continue;
                }
                merged_shaders.insert(id.clone(), Arc::clone(def));
            }
        }
        let mut shader_registry = ShaderRegistry::default();
        shader_registry.merge_shader_definitions(merged_shaders)?;

        let mut clip_kinds = ClipKindStore::new();
        clip_kinds
            .merge_plugin_kinds(merged_kinds)
            .context("clip kind conflict during rebuild_indices")?;

        let mut scripts_index: HashMap<String, CompiledScript> = HashMap::new();
        let mut component_scripts_index: HashMap<NamespacedID, CompiledScript> = HashMap::new();
        let mut toolbar_button_scripts_index: HashMap<NamespacedID, CompiledScript> =
            HashMap::new();
        let mut schemas_index: Vec<PropertySchema> = Vec::new();
        let mut toolbars_index: Vec<(String, ToolbarButtonSpec)> = Vec::new();

        for plugin in &self.plugins {
            // scripts index (プラグイン共通 script.rhai)
            if let Some(script) = plugin.script.as_ref() {
                scripts_index.insert(plugin.manifest.id.clone(), script.clone());
            }
            // コンポーネントごとの個別スクリプト（定義側の先勝ちに合わせる）
            for (id, script) in &plugin.component_scripts {
                if !component_scripts_index.contains_key(id) {
                    component_scripts_index.insert(id.clone(), script.clone());
                }
            }
            // ツールバーボタンごとの個別スクリプト
            for (id, script) in &plugin.toolbar_button_scripts {
                if !toolbar_button_scripts_index.contains_key(id) {
                    toolbar_button_scripts_index.insert(id.clone(), script.clone());
                }
            }

            // schemas index
            for schema in plugin.setting_schemas.iter() {
                schemas_index.push(schema.clone());
            }

            // toolbars index
            for button in plugin.toolbar_buttons.iter() {
                toolbars_index.push((plugin.manifest.id.clone(), button.clone()));
            }
        }

        // 全部成功した場合にだけ、現在のインデックスと差し替える
        self.clip_kinds = clip_kinds;
        self.component_registry = component_registry;
        self.shader_registry = shader_registry;
        self.scripts_index = scripts_index;
        self.component_scripts_index = component_scripts_index;
        self.toolbar_button_scripts_index = toolbar_button_scripts_index;
        self.schemas_index = schemas_index;
        self.toolbars_index = toolbars_index;
        if let Ok(mut cache) = self.resolved_definitions.write() {
            cache.clear();
        }

        log::info!(
            "Plugin indices rebuilt: clip_kinds={}, scripts={}, schemas={}, toolbars={}, shaders={}",
            self.clip_kinds.registry.iter().count(),
            self.scripts_index.len(),
            self.schemas_index.len(),
            self.toolbars_index.len(),
            self.shader_registry.len()
        );

        Ok(())
    }

    /// 追加のプラグインをディスクから読み込み、インデックスを再構築します。
    ///
    /// ホットロード（実行時追加）に使用します。
    pub fn load_additional_plugin(&mut self, dir: &Path) -> anyhow::Result<PluginManifest> {
        let plugin = Plugin::load(dir)
            .with_context(|| format!("Failed to load plugin at {}", dir.display()))?;
        let manifest = plugin.manifest.clone();
        self.plugins.push(plugin);
        // 再構築に失敗したら追加を取り消し、plugins とインデックスの乖離を防ぐ
        if let Err(error) = self.rebuild_indices() {
            self.plugins.pop();
            return Err(error);
        }
        log::info!("Loaded additional plugin '{}'", manifest.id);
        Ok(manifest)
    }

    /// 指定 ID のプラグインをディスクから再読み込み（ホットリロード）します。
    ///
    /// 他プラグインの pending properties が差し替え対象の kind を参照している
    /// 可能性があるため、必ず全プラグイン分を再解決します。
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
            let old_plugin = std::mem::replace(&mut self.plugins[index], reloaded_plugin);
            // 他プラグインのpending propertiesが差し替え対象のkindを参照している
            // 可能性があるため、必ず全プラグイン分を再解決する
            if let Err(error) = self.rebuild_indices() {
                // 再構築に失敗したら元のインスタンスに戻す
                self.plugins[index] = old_plugin;
                return Err(error);
            }
            log::info!("Reloaded plugin '{}'", plugin_id);
        }

        Ok(())
    }

    /// 指定 ID のプラグインをアンロードし、インデックスを再構築します。
    pub fn unload_plugin_by_id(&mut self, plugin_id: &str) -> anyhow::Result<()> {
        let Some(index) = self.plugins.iter().position(|p| p.manifest.id == plugin_id) else {
            anyhow::bail!("Plugin `{}` not found in loaded plugins", plugin_id);
        };
        let removed = self.plugins.remove(index);
        // 再構築に失敗したら元に戻す
        if let Err(error) = self.rebuild_indices() {
            self.plugins.insert(index, removed);
            return Err(error);
        }
        log::info!("Unloaded plugin '{}'", plugin_id);
        Ok(())
    }

    /// 全設定スキーマのコピーを返します。
    pub fn collect_all_schemas(&self) -> Vec<PropertySchema> {
        self.schemas_index.clone()
    }

    /// 全ツールバーボタン仕様のコピーを返します。
    pub fn collect_all_toolbars(&self) -> Vec<(String, ToolbarButtonSpec)> {
        self.toolbars_index.clone()
    }

    // ホットパス用の直接アクセスメソッド
    /// 指定 ID の `ClipKind` を返します。
    pub fn get_clip_kind(&self, id: &NamespacedID) -> Option<&ClipKind> {
        self.clip_kinds.get(id)
    }

    /// 指定 ID のクリップ種別が提供するプロパティスキーマを返します。
    ///
    /// プロパティはクリップ種別側ではなく、その種別を構成する各コンポーネントが
    /// 提供します。種別の宣言順にコンポーネントをたどり、スキーマを合成します。
    /// 種別または構成コンポーネントが存在しない場合は `None` を返します。
    pub fn get_clip_properties(&self, id: &NamespacedID) -> Option<Vec<PropertySchema>> {
        let kind = self.clip_kinds.get(id)?;
        let mut schemas = Vec::new();
        for component_id in &kind.components {
            let component = self.component_registry.get(component_id)?;
            schemas.extend(component.properties.iter().cloned());
        }
        Some(schemas)
    }

    /// 指定 ID の `ComponentDefinition` を返します。
    pub fn get_component(&self, id: &NamespacedID) -> Option<&ComponentDefinition> {
        self.component_registry.get(id)
    }

    /// 指定 ID の `ShaderDefinition` を返します。
    pub fn get_shader(&self, id: &NamespacedID) -> Option<Arc<ShaderDefinition>> {
        self.shader_registry.get(id)
    }

    /// クリップ種別 ID から、その種別が宣言するコンポーネント列を解決し、
    /// フラットな `RuntimeDefinition` を返します。
    ///
    /// 解決結果はキャッシュし、同じ種別への再解決では再構築しません。
    /// キャッシュはレジストリ再構築（`rebuild_indices`）時にクリアされます。
    pub fn resolve_component_definition(
        &self,
        kind_id: &NamespacedID,
    ) -> anyhow::Result<Arc<RuntimeDefinition>> {
        if let Ok(cache) = self.resolved_definitions.read() {
            if let Some(definition) = cache.get(kind_id) {
                return Ok(Arc::clone(definition));
            }
        }
        let kind = self
            .clip_kinds
            .get(kind_id)
            .with_context(|| format!("clip kind `{}` not found", kind_id.full()))?;
        let definition = Arc::new(
            self.component_registry
                .resolve_definition(kind_id, &kind.components)?,
        );
        if let Ok(mut cache) = self.resolved_definitions.write() {
            cache.insert(kind_id.clone(), Arc::clone(&definition));
        }
        Ok(definition)
    }

    /// 指定 `Capability` を提供する `ComponentDefinition` をすべて返します。
    ///
    /// レンダラー等はこのメソッドで能力からコンポーネントを逆引きし、
    /// ハードコードされた ID に依存しない設計とします（AGENTS.md 第9条）。
    pub fn get_components_by_capability(
        &self,
        capability: &ComponentCapability,
    ) -> Vec<&ComponentDefinition> {
        self.component_registry.by_capability(capability)
    }

    /// 指定 `Capability` を持つコンポーネントを含むクリップ種別を返します。
    ///
    /// 複数の種別が該当する場合は ID の辞書順で最小のものを返し、選択を
    /// 決定的にします。該当する種別が無ければ `None` を返します。
    pub fn find_clip_kind_by_capability(
        &self,
        capability: &ComponentCapability,
    ) -> Option<NamespacedID> {
        self.clip_kinds
            .iter()
            .filter(|(_, kind)| {
                kind.components.iter().any(|component_id| {
                    self.component_registry
                        .get(component_id)
                        .is_some_and(|component| component.capabilities.contains(capability))
                })
            })
            .map(|(id, _)| id)
            .min_by(|a, b| a.full().cmp(b.full()))
            .cloned()
    }

    /// 全設定スキーマへのスライス参照を返します。
    pub fn get_settings_schemas(&self) -> &[PropertySchema] {
        &self.schemas_index
    }

    /// 全ツールバーボタン仕様へのスライス参照を返します。
    pub fn get_toolbar_buttons(&self) -> &[(String, ToolbarButtonSpec)] {
        &self.toolbars_index
    }

    /// プラグイン共通の Rhai スクリプトを返します。
    pub fn get_script(&self, plugin_id: &str) -> Option<&CompiledScript> {
        self.scripts_index.get(plugin_id)
    }

    /// コンポーネントごとの個別 Rhai スクリプトを返します。
    pub fn get_component_script(&self, id: &NamespacedID) -> Option<&CompiledScript> {
        self.component_scripts_index.get(id)
    }

    /// ツールバーボタンごとの個別 Rhai スクリプトを返します。
    pub fn get_toolbar_button_script(&self, id: &NamespacedID) -> Option<&CompiledScript> {
        self.toolbar_button_scripts_index.get(id)
    }

    /// プラグイン共通スクリプトの関数を呼び出します。
    ///
    /// プラグイン ID が見つからない場合はエラーを返します。
    pub fn call_script<R: Clone + Send + Sync + 'static>(
        &self,
        engine: &rhai::Engine,
        plugin_id: &str,
        fn_name: &str,
        args: impl rhai::FuncArgs,
    ) -> anyhow::Result<R> {
        let script = self
            .get_plugin_script(plugin_id)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_id))?;

        script.call(engine, fn_name, args)
    }

    /// プラグイン共通スクリプトの複製を返します。
    ///
    /// 呼び出し側がローダーのガードを解放してから実行できるようにするための
    /// アクセサです（実行中に再読み込みが走ってもデッドロックしません）。
    pub fn get_plugin_script(&self, plugin_id: &str) -> Option<CompiledScript> {
        self.scripts_index.get(plugin_id).cloned()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/plugin/archive_tests.rs"]
mod archive_tests;

#[cfg(test)]
#[path = "../../tests/unit/plugin/shader_loading_tests.rs"]
mod shader_loading_tests;
