use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use rhai::module_resolvers::FileModuleResolver;

use crate::{plugin::script::api::register_fn_for, util::result::EsotereelError};

pub mod api;
pub mod bridge;

#[derive(Debug, Clone)]
pub struct CompiledScript {
    pub ast: rhai::AST,

    pub available_functions: Vec<String>,

    engine: Arc<rhai::Engine>,
}

impl CompiledScript {
    pub fn compile(resolve_path: &Path, source_path: &PathBuf) -> anyhow::Result<CompiledScript> {
        let mut engine = rhai::Engine::new();
        register_fn_for(&mut engine);

        // rhai を全部フラットに合流させる
        let mut paths: Vec<_> = walkdir::WalkDir::new(resolve_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| e.into_path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "rhai"))
            .collect();

        // source_pathは優先
        paths.retain(|p| p != source_path);
        paths.sort();
        paths.insert(0, source_path.clone());

        // 関数名 -> 最初に定義されていたファイル
        let mut functions = HashMap::new();
        let mut ast = rhai::AST::empty();

        for path in paths {
            let clip_ast = engine.compile_file(path.clone())?;

            for func in clip_ast.iter_functions() {
                let name = func.name.to_string();

                if let Some(previous_path) = functions.insert(name.clone(), path.clone()) {
                    anyhow::bail!(
                        "Duplicate function '{}': '{}' and '{}'",
                        name,
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
            engine: Arc::new(engine),
        })
    }

    pub fn call<T: Clone + Send + Sync + 'static>(
        &self,
        fn_name: &str,
        args: impl rhai::FuncArgs,
    ) -> anyhow::Result<T> {
        let mut scope = rhai::Scope::new();
        let result = self
            .engine
            .call_fn::<T>(&mut scope, &self.ast, fn_name, args)
            .map_err(|e| anyhow::anyhow!("Function '{}' call error: {}", fn_name, e))?;
        Ok(result)
    }
}

#[derive(Debug, Default)]
pub struct ScriptRegistry {
    scripts: HashMap<String, CompiledScript>,
}

impl ScriptRegistry {
    pub fn scripts_for_plugin(&self, id: &str) -> Option<&CompiledScript> {
        self.scripts.get(id)
    }

    pub fn merge_plugin_scripts(
        &mut self,
        plugin_buttons: HashMap<String, CompiledScript>,
    ) -> anyhow::Result<()> {
        let mut merged = self.scripts.clone();
        merged.extend(plugin_buttons);
        self.scripts = merged;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ScriptStore {
    pub registry: ScriptRegistry,
    scripts: HashMap<String, CompiledScript>,
    has_disk_loaded: bool,
}

impl ScriptStore {
    pub fn new() -> Self {
        Self {
            registry: ScriptRegistry::default(),
            scripts: HashMap::new(),
            has_disk_loaded: false,
        }
    }

    pub fn merge_plugin_scripts(
        &mut self,
        plugin_buttons: HashMap<String, CompiledScript>,
    ) -> anyhow::Result<()> {
        self.registry.merge_plugin_scripts(plugin_buttons.clone())?;
        self.scripts.extend(plugin_buttons);
        Ok(())
    }

    pub fn apply_loaded_script(
        &mut self,
        loaded: HashMap<String, CompiledScript>,
    ) -> anyhow::Result<()> {
        for (target, source) in loaded {
            self.scripts.insert(target, source);
        }
        self.has_disk_loaded = true;

        Ok(())
    }

    pub fn call<T: Clone + Send + Sync + 'static>(
        &self,
        plugin_id: &str,
        fn_name: &str,
        args: impl rhai::FuncArgs,
    ) -> anyhow::Result<T> {
        let script = self
            .scripts
            .get(plugin_id)
            .ok_or(EsotereelError::PluginNotFound(plugin_id.to_string()))?;

        script.call(fn_name, args)
    }
}
