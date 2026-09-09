use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use rhai::module_resolvers::FileModuleResolver;

use crate::{plugin::script::core_fn::register_fn_for, util::result::EsotereelError};

mod core_fn;

#[derive(Debug, Clone)]
pub struct CompiledScript {
    pub ast: rhai::AST,

    pub available_functions: Vec<String>,
}

impl CompiledScript {
    pub fn compile(
        engine: &mut rhai::Engine,
        resolve_path: &Path,
        source_path: &PathBuf,
    ) -> anyhow::Result<CompiledScript> {
        let resolver = FileModuleResolver::new_with_path(resolve_path);
        engine.set_module_resolver(resolver);

        let ast = engine
            .compile_file(source_path.clone())
            .map_err(|e| anyhow::anyhow!("Script compile error: {}", e))?;

        let available_functions = ast.iter_functions().map(|f| f.name.to_string()).collect();

        Ok(CompiledScript {
            ast,
            available_functions,
        })
    }

    pub fn call<T: Clone + Send + Sync + 'static>(
        &self,
        engine: &rhai::Engine,
        fn_name: &str,
        args: impl rhai::FuncArgs,
    ) -> anyhow::Result<T> {
        let mut scope = rhai::Scope::new();
        let result = engine
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
    pub engine: rhai::Engine, // 唯一のEngineインスタンス
    scripts: HashMap<String, CompiledScript>,
    has_disk_loaded: bool,
}

impl ScriptStore {
    pub fn new() -> Self {
        let mut engine = rhai::Engine::new();
        register_fn_for(&mut engine);

        Self {
            registry: ScriptRegistry::default(),
            engine,
            scripts: HashMap::new(),
            has_disk_loaded: false,
        }
    }

    pub fn merge_plugin_scripts(
        &mut self,
        plugin_buttons: HashMap<String, CompiledScript>,
    ) -> anyhow::Result<()> {
        self.registry.merge_plugin_scripts(plugin_buttons)
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
            .registry
            .scripts_for_plugin(plugin_id)
            .ok_or(EsotereelError::PluginNotFound(plugin_id.to_string()))?;

        script.call(&self.engine, fn_name, args)
    }
}
