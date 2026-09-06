use anyhow::Context;
use directories::ProjectDirs;
use log::{info, warn};
use std::path::PathBuf;

#[derive(Default)]
pub struct Directories {
    // custon dir
    pub std_plugin_dir: Option<PathBuf>,
    pub working_dir: Option<PathBuf>,
}

impl Directories {
    pub fn new(std_plugin_dir: Option<PathBuf>, working_dir: Option<PathBuf>) -> Self {
        Self {
            std_plugin_dir,
            working_dir,
        }
    }

    fn root(&self) -> anyhow::Result<ProjectDirs> {
        let proj_dirs = ProjectDirs::from("com", "satoshiinu", "esotereel") // qualifier, org, app
            .context("could not determine user config directory")?;
        Ok(proj_dirs)
    }

    pub fn data_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(working_dir) = self.working_dir.clone() {
            Ok(working_dir.join("data"))
        } else {
            let proj_dirs = self.root()?;

            Ok(proj_dirs.data_dir().to_path_buf())
        }
    }

    pub fn config_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(working_dir) = self.working_dir.clone() {
            Ok(working_dir.join("config"))
        } else {
            let proj_dirs = self.root()?;

            Ok(proj_dirs.config_dir().to_path_buf())
        }
    }

    pub fn cache_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(working_dir) = self.working_dir.clone() {
            Ok(working_dir.join("cache"))
        } else {
            let proj_dirs = self.root()?;

            Ok(proj_dirs.cache_dir().to_path_buf())
        }
    }

    pub fn std_plugins_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(std_plugin_dir) = self.std_plugin_dir.clone() {
            Ok(std_plugin_dir)
        } else {
            let install_dirs = self.install_dir()?;

            Ok(install_dirs.join("std_plugins"))
        }
    }

    pub fn user_plugins_dir(&self) -> anyhow::Result<PathBuf> {
        Ok(self.data_dir()?.join("plugins"))
    }

    pub fn client_settings_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.config_dir()?.join("settings.toml"))
    }

    pub fn install_dir(&self) -> anyhow::Result<std::path::PathBuf> {
        let exe_dir = std::env::current_exe()
            .expect("failed to get exe path")
            .parent()
            .unwrap()
            .to_path_buf();

        #[cfg(target_os = "macos")]
        {
            // Contents/MacOS/exe -> Contents/Resources/
            Ok(exe_dir.parent().unwrap().join("Resources"))
        }

        #[cfg(target_os = "windows")]
        {
            use winreg::RegKey;
            use winreg::enums::HKEY_LOCAL_MACHINE;

            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            let key = hklm.open_subkey("Software\\Esotereel")?;
            let path: String = key.get_value("InstallLocation")?;
            Ok(std::path::PathBuf::from(path))
        }

        #[cfg(target_os = "linux")]
        {
            Ok(
                std::env::var("APPDIR")
                    .map(PathBuf::from)
                    .map(|p| p.join("usr/share/esotereel"))
                    .unwrap_or(exe_dir), // AppImage外での実行(開発時等)のフォールバック
            )
        }

        // unreachable!("unknown os detected")
    }

    pub fn log_info(&self) {
        match self.data_dir() {
            Ok(path) => info!("data directory: {}", path.display()),
            Err(error) => warn!("failed to determine data directory: {error}"),
        }
        match self.config_dir() {
            Ok(path) => info!("config directory: {}", path.display()),
            Err(error) => warn!("failed to determine config directory: {error}"),
        }
        match self.cache_dir() {
            Ok(path) => info!("cache directory: {}", path.display()),
            Err(error) => warn!("failed to determine cache directory: {error}"),
        }
    }
}
