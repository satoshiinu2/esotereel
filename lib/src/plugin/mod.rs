use std::path::{Path, PathBuf};

use anyhow::Context;
use log;

use crate::{HostRole, dirs::Directories, setting::FieldSchema};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
}

pub struct Plugin {
    pub manifest: PluginManifest,
    pub schema: Vec<FieldSchema>,
    pub dir: PathBuf,
}

impl Plugin {
    /// このプラグイン単体のディレクトリからmanifest+settings.tomlを読み込む
    fn load(dir: &Path) -> anyhow::Result<Self> {
        let manifest_path = dir.join("manifest.toml");
        let manifest_text = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read manifest at {}", manifest_path.display()))?;
        let manifest: PluginManifest = toml::from_str(&manifest_text)
            .with_context(|| format!("failed to parse manifest at {}", manifest_path.display()))?;

        log::info!(
            "Loading plugin '{}' v{} (ID: {})",
            manifest.name,
            manifest.version,
            manifest.id
        );

        let settings_path = dir.join("settings.toml");
        let schema = if settings_path.exists() {
            let text = std::fs::read_to_string(&settings_path).with_context(|| {
                format!(
                    "failed to read settings schema at {}",
                    settings_path.display()
                )
            })?;
            FieldSchema::parse_toml(&text, &format!("{}/settings.toml", manifest.id))
                .with_context(|| format!("invalid settings schema for plugin `{}`", manifest.id))?
        } else {
            Vec::new()
        };

        // key衝突防止のためプラグインIDでnamespace化
        let schema = Self::namespaced_schema(&manifest.id, schema);

        Ok(Self {
            manifest,
            schema,
            dir: dir.to_owned(),
        })
    }

    fn namespaced_schema(plugin_id: &str, mut fields: Vec<FieldSchema>) -> Vec<FieldSchema> {
        for f in &mut fields {
            f.key = format!("{plugin_id}.{}", f.key);
            f.category.insert(0, plugin_id.to_string());
        }
        fields
    }
}

pub struct PluginLoadResult {
    pub dir: PathBuf,
    pub result: anyhow::Result<Plugin>,
}

pub struct PluginLoader {
    pub plugins: Vec<Plugin>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
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

        self.plugins = results
            .into_iter()
            .filter_map(|r| r.result.ok())
            .collect::<Vec<_>>();

        // 呼び出し側が個別の成否も見たい場合のために結果自体も返す
        // (現状は上のself.pluginsへの格納で十分ならこの戻り値は無くしてもいい)
        let dirs_again = Self::discover_all_plugin_dirs(dirs_def)?; // 簡略化のための重複呼び出し、下記コメント参照
        Ok(dirs_again
            .into_iter()
            .map(|dir| PluginLoadResult {
                result: Plugin::load(&dir),
                dir,
            })
            .collect())
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
            log::info!("Reloaded plugin '{}'", plugin_id);
        }

        Ok(())
    }

    pub fn collect_all_schemas(&self) -> Vec<FieldSchema> {
        self.plugins.iter().flat_map(|p| p.schema.clone()).collect()
    }
}
