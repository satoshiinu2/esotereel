use anyhow::Context;
use directories::ProjectDirs;
use std::path::PathBuf;

fn root() -> anyhow::Result<ProjectDirs> {
    let proj_dirs = ProjectDirs::from("com", "esotereel", "esotereel") // qualifier, org, app
        .context("could not determine user config directory")?;
    Ok(proj_dirs)
}

pub fn plugins_dir() -> anyhow::Result<PathBuf> {
    let proj_dirs = root()?;
    Ok(proj_dirs.config_dir().join("plugins"))
}

pub fn client_settings_path() -> anyhow::Result<PathBuf> {
    let proj_dirs = root()?;
    Ok(proj_dirs.config_dir().join("settings.toml"))
}
