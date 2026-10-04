use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use log::warn;

use crate::{
    HostRole, plugin::registry::PluginDefinitionRegistry, plugin::script::api::register_fn_for,
    util::result::EsotereelError,
};

pub mod api;
pub mod bridge;

thread_local! {
    /// for compile only
     static SCRIPT_ENGINE: rhai::Engine = rhai::Engine::new();
}

#[derive(Debug, Clone)]
pub struct CompiledScript {
    pub ast: rhai::AST,

    pub available_functions: Vec<String>,
}

impl CompiledScript {
    pub fn compile(resolve_path: &Path, source_path: &Path) -> anyhow::Result<CompiledScript> {
        SCRIPT_ENGINE.with(|engine| {
            // AST のコンパイルと実行で同じ共有エンジン群を使う。

            // rhai を全部フラットに合流させる
            let mut paths: Vec<_> = walkdir::WalkDir::new(resolve_path)
                .into_iter()
                .filter_map(|e| match e {
                    Ok(entry) => Some(entry.into_path()),
                    Err(err) => {
                        warn!("Failed to read plugin directory entry: {err}");
                        None
                    }
                })
                .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "rhai"))
                .collect();

            // source_pathは優先
            paths.retain(|p| p != source_path);
            paths.sort();
            paths.insert(0, source_path.to_path_buf());

            // (関数名, 引数の数) -> 最初に定義されていたファイル
            // オーバーロード(同名・異なるarity)は許容し、
            // 同じシグネチャの重複だけを衝突として扱う
            let mut functions: HashMap<(String, usize), PathBuf> = HashMap::new();
            let mut ast = rhai::AST::empty();

            for path in paths {
                let clip_ast = engine.compile_file(path.clone())?;

                for func in clip_ast.iter_functions() {
                    let key = (func.name.to_string(), func.params.len());

                    if let Some(previous_path) = functions.insert(key.clone(), path.clone()) {
                        // 同一ファイル内での再定義はコンパイラ側で弾かれるはずなので、
                        // ここに来るのは異なるファイル間の衝突のみ
                        anyhow::bail!(
                            "Duplicate function '{}' ({} args): '{}' and '{}'",
                            key.0,
                            key.1,
                            previous_path.display(),
                            path.display()
                        );
                    }
                }

                ast = ast.merge(&clip_ast);
            }

            let available_functions = ast.iter_functions().map(|f| f.name.to_string()).collect();
            Ok(CompiledScript {
                ast,
                available_functions,
            })
        })
    }

    pub fn call<R: Clone + Send + Sync + 'static>(
        &self,
        engine: &rhai::Engine,
        fn_name: &str,
        args: impl rhai::FuncArgs,
    ) -> anyhow::Result<R> {
        let mut scope = rhai::Scope::new();
        let result = engine
            .call_fn::<R>(&mut scope, &self.ast, fn_name, args)
            .map_err(|e| anyhow::anyhow!("Function '{}' call error: {}", fn_name, e))?;
        Ok(result)
    }
}

/// プラグインから供給されるスクリプトをid衝突チェック付きで集約するレジストリ。
#[derive(Debug, Default)]
pub struct ScriptRegistry {
    inner: PluginDefinitionRegistry<String, CompiledScript>,
}

impl ScriptRegistry {
    pub fn get(&self, id: &str) -> Option<&CompiledScript> {
        self.inner.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.inner.contains(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &CompiledScript)> {
        self.inner.iter()
    }

    /// プラグイン由来のスクリプトを合流させる。
    /// 既存(組み込み/他プラグイン)とid衝突があればエラーにする。
    pub fn merge_plugin_scripts(
        &mut self,
        plugin_scripts: HashMap<String, CompiledScript>,
    ) -> anyhow::Result<()> {
        self.inner.merge(plugin_scripts)
    }
}

/// スクリプトの唯一の保管場所。
/// 以前は `ScriptRegistry` と `ScriptStore` の2箇所に同じ内容を持たせていたが、
/// 状態不整合のリスクがあるため単一の HashMap に統一した。
/// 現在は ScriptRegistry を薄くラップする構造に戻している。
#[derive(Debug, Default)]
pub struct ScriptStore {
    registry: ScriptRegistry,
}

impl ScriptStore {
    pub fn new() -> Self {
        Self {
            registry: ScriptRegistry::default(),
        }
    }

    pub fn scripts_for_plugin(&self, id: &str) -> Option<&CompiledScript> {
        self.registry.get(id)
    }

    pub fn merge_plugin_scripts(
        &mut self,
        plugin_scripts: HashMap<String, CompiledScript>,
    ) -> anyhow::Result<()> {
        self.registry.merge_plugin_scripts(plugin_scripts)
    }

    pub fn apply_loaded_script(
        &mut self,
        loaded: HashMap<String, CompiledScript>,
    ) -> anyhow::Result<()> {
        self.registry.merge_plugin_scripts(loaded)
    }

    pub fn call<T: Clone + Send + Sync + 'static>(
        &self,
        engine: &rhai::Engine,
        plugin_id: &str,
        fn_name: &str,
        args: impl rhai::FuncArgs,
    ) -> anyhow::Result<T> {
        let script = self
            .registry
            .get(plugin_id)
            .ok_or(EsotereelError::PluginNotFound(plugin_id.to_string()))?;
        script.call(engine, fn_name, args)
    }
}
