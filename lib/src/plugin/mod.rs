use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Context;
use log;

use crate::{
    HostRole,
    dirs::Directories,
    plugin::{script::CompiledScript, setting::SettingFieldSchema, toolbar::ToolbarButtonSpec},
};

pub mod script;
pub mod setting;
pub mod toolbar;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone)]
pub struct Plugin {
    pub manifest: PluginManifest,
    pub setting_schema: Vec<SettingFieldSchema>,
    pub toolbar_buttons: Vec<ToolbarButtonSpec>,
    pub script: Option<CompiledScript>,
    pub dir: PathBuf,
}

impl Plugin {
    /// このプラグイン単体のディレクトリからmanifest+settings.tomlを読み込む
    fn load(dir: &Path) -> anyhow::Result<Self> {
        let manifest = Self::load_manifest(&dir.join("manifest.toml"))?;

        log::info!(
            "Loading plugin '{}' v{} (ID: {})",
            manifest.name,
            manifest.version,
            manifest.id
        );

        let setting_schema = Self::load_settings(&manifest, &dir.join("settings.toml"))?;
        let toolbar_buttons = Self::load_toolbar(&manifest, &dir.join("toolbars.toml"))?;
        let script = Self::load_script(&manifest, &dir, &dir.join("script.rhai"))?;
        let avaliable_functions = script
            .as_ref()
            .map_or_default(|s| s.available_functions.clone());

        log::info!(
            "Plugin '{}' loaded successfully: Settings schemas: {}, Toolbar buttons: {}, Available scripts: {}",
            manifest.id,
            setting_schema.len(),
            toolbar_buttons.len(),
            avaliable_functions.len()
        );

        Ok(Self {
            manifest,
            setting_schema,
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
    ) -> anyhow::Result<Vec<SettingFieldSchema>> {
        let schema = if settings_path.exists() {
            let text = std::fs::read_to_string(&settings_path).with_context(|| {
                format!(
                    "failed to read settings schema at {}",
                    settings_path.display()
                )
            })?;
            SettingFieldSchema::parse_toml(&text, &manifest.id)
                .with_context(|| format!("invalid settings schema for plugin `{}`", manifest.id))?
        } else {
            Vec::new()
        };

        // key衝突防止のためプラグインIDでnamespace化
        let schema = Self::namespaced_schema(&manifest.id, schema);

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
        manifest: &PluginManifest,
        plugin_path: &Path,
        script_path: &Path,
    ) -> anyhow::Result<Option<CompiledScript>> {
        if script_path.exists() {
            // 並列で動かしているので毎回作成
            let mut engine = rhai::Engine::new();
            let compiled = CompiledScript::compile(
                &mut engine,
                plugin_path, // モジュールの解決パス (プラグインのルートディレクトリ)
                &script_path.to_path_buf(),
            )?;
            Ok(Some(compiled))
        } else {
            Ok(None)
        }
    }

    fn namespaced_schema(
        plugin_id: &str,
        mut fields: Vec<SettingFieldSchema>,
    ) -> Vec<SettingFieldSchema> {
        for f in &mut fields {
            f.key = format!("{plugin_id}.{}", f.key);
            f.category.insert(0, plugin_id.to_string());
        }
        fields
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

pub struct PluginLoadResult {
    pub dir: PathBuf,
    pub result: anyhow::Result<Plugin>,
}

pub struct PluginLoader {
    pub p: Vec<Plugin>,
    is_loaded: bool,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            p: Vec::new(),
            is_loaded: false,
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
    ) -> anyhow::Result<Vec<PluginLoadResult>> {
        // 既に読み込み済みならキャッシュを返す
        if self.is_loaded {
            log::info!("Using cached plugins for {:?}", role);
            let results = self
                .p
                .iter()
                .map(|plugin| PluginLoadResult {
                    dir: plugin.dir.clone(),
                    result: Ok(plugin.clone()),
                })
                .collect();
            return Ok(results);
        }

        log::info!("Starting plugin loading for {:?}", role);
        let plugin_dirs = Self::discover_all_plugin_dirs(dirs_def)?;
        log::info!("Discovered {} plugin directories", plugin_dirs.len());

        let mut tasks = vec![];
        for dir in plugin_dirs {
            let task = tokio::task::spawn_blocking(move || PluginLoadResult {
                result: Plugin::load(&dir),
                dir,
            });

            tasks.push(task);
        }

        let mut results = Vec::with_capacity(tasks.len());
        for task in tasks {
            if let Ok(res) = task.await {
                results.push(res);
            }
        }

        let successful_count = results.iter().filter(|r| r.result.is_ok()).count();
        log::info!(
            "Loaded {}/{} plugins successfully",
            successful_count,
            results.len()
        );

        // 失敗したプラグインのログを出力
        for result in &results {
            if let Err(e) = &result.result {
                log::error!("Failed to load plugin from {}: {}", result.dir.display(), e);
            }
        }

        self.p = results
            .into_iter()
            .filter_map(|r| r.result.ok())
            .collect::<Vec<_>>();

        self.is_loaded = true;

        // 呼び出し側が個別の成否も見たい場合のために結果自体も返す
        // キャッシュ済みのプラグインからPluginLoadResultを再構築
        Ok(self
            .p
            .iter()
            .map(|plugin| PluginLoadResult {
                dir: plugin.dir.clone(),
                result: Ok(plugin.clone()),
            })
            .collect())
    }

    pub fn reload_plugin_by_id(&mut self, plugin_id: &str) -> anyhow::Result<()> {
        // 対象プラグインのディレクトリを取得
        let dir = self
            .p
            .iter()
            .find(|p| p.manifest.id == plugin_id)
            .map(|p| p.dir.clone())
            .ok_or_else(|| anyhow::anyhow!("Plugin `{}` not found in loaded plugins", plugin_id))?;

        // ディレクトリから再度読み込み (マニフェスト・設定の再パース)
        let reloaded_plugin = Plugin::load(&dir)
            .with_context(|| format!("Failed to hot-reload plugin `{}`", plugin_id))?;

        // 成功したら配列内の古いインスタンスを差し替え
        if let Some(index) = self.p.iter().position(|p| p.manifest.id == plugin_id) {
            self.p[index] = reloaded_plugin;
            log::info!("Reloaded plugin '{}'", plugin_id);
        }

        Ok(())
    }

    pub fn collect_all_schemas(&self) -> Vec<SettingFieldSchema> {
        self.p
            .iter()
            .flat_map(|p| p.setting_schema.clone())
            .collect()
    }

    pub fn collect_all_toolbars(&self) -> Vec<(String, ToolbarButtonSpec)> {
        self.p
            .iter()
            .flat_map(|p| {
                p.toolbar_buttons
                    .iter()
                    .map(|b| (p.manifest.id.clone(), b.clone()))
            })
            .collect()
    }

    pub fn collect_all_scripts(&self) -> HashMap<String, CompiledScript> {
        self.p
            .iter()
            .filter_map(|plugin| {
                plugin
                    .script
                    .clone()
                    .map(|script| (plugin.manifest.id.clone(), script))
            })
            .collect()
    }
}
