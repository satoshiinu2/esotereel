use anyhow::Context;
use directories::ProjectDirs;
use log::{info, warn};
use std::path::PathBuf;

/// アプリケーションが利用するディレクトリパスを管理します。
///
/// `working_dir`（ワークディレクトリ）が指定されている場合はそちらを優先し、
/// 指定されていない場合はプラットフォーム固有の標準パス（`directories` クレート経由）を使用します。
///
/// All rights reserved.
#[derive(Default)]
pub struct Directories {
    /// 標準プラグインディレクトリの明示指定（未指定時はインストールディレクトリから導出）
    pub std_plugin_dir: Option<PathBuf>,
    /// ワークディレクトリ（指定時は `data_dir`/`config_dir`/`cache_dir` のベースとして優先使用）
    pub working_dir: Option<PathBuf>,
}

impl Directories {
    pub fn new(std_plugin_dir: Option<PathBuf>, working_dir: Option<PathBuf>) -> Self {
        Self {
            std_plugin_dir,
            working_dir,
        }
    }

    /// アプリ名・組織・限定子から `ProjectDirs` を構築します。
    ///
    /// ユーザー設定ディレクトリが決定できない場合はエラーを返します。
    fn root(&self) -> anyhow::Result<ProjectDirs> {
        let proj_dirs = ProjectDirs::from("com", "satoshiinu", "esotereel") // qualifier, org, app
            .context("could not determine user config directory")?;
        Ok(proj_dirs)
    }

    /// データディレクトリのパスを返します。
    ///
    /// `working_dir` が設定されていれば `working_dir/data`、そうでなければ
    /// プラットフォーム標準のデータディレクトリを返します。
    pub fn data_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(working_dir) = self.working_dir.as_ref() {
            Ok(working_dir.join("data"))
        } else {
            let proj_dirs = self.root()?;

            Ok(proj_dirs.data_dir().to_path_buf())
        }
    }

    /// 設定ディレクトリのパスを返します。
    ///
    /// `working_dir` が設定されていれば `working_dir/config`、そうでなければ
    /// プラットフォーム標準の設定ディレクトリを返します。
    pub fn config_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(working_dir) = self.working_dir.as_ref() {
            Ok(working_dir.join("config"))
        } else {
            let proj_dirs = self.root()?;

            Ok(proj_dirs.config_dir().to_path_buf())
        }
    }

    /// キャッシュディレクトリのパスを返します。
    ///
    /// `working_dir` が設定されていれば `working_dir/cache`、そうでなければ
    /// プラットフォーム標準のキャッシュディレクトリを返します。
    pub fn cache_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(working_dir) = self.working_dir.as_ref() {
            Ok(working_dir.join("cache"))
        } else {
            let proj_dirs = self.root()?;

            Ok(proj_dirs.cache_dir().to_path_buf())
        }
    }

    /// 標準プラグインディレクトリのパスを返します。
    ///
    /// `std_plugin_dir` が指定されていればそれを返し、そうでなければ
    /// インストールディレクトリ配下の `std_plugins` を返します。
    pub fn std_plugins_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(std_plugin_dir) = self.std_plugin_dir.as_ref() {
            Ok(std_plugin_dir.clone())
        } else {
            let install_dirs = self.install_dir()?;

            Ok(install_dirs.join("std_plugins"))
        }
    }

    /// ユーザープラグインディレクトリのパスを返します。
    ///
    /// データディレクトリ配下の `plugins` を返します。
    pub fn user_plugins_dir(&self) -> anyhow::Result<PathBuf> {
        Ok(self.data_dir()?.join("plugins"))
    }

    /// クライアント設定ファイル（`settings.toml`）のパスを返します。
    pub fn client_settings_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.config_dir()?.join("settings.toml"))
    }

    /// プリブートストラップ設定ファイル（`prebootstrap.toml`）のパスを返します。
    pub fn prebootstrap_settings_path(&self) -> anyhow::Result<PathBuf> {
        Ok(self.config_dir()?.join("prebootstrap.toml"))
    }

    /// インストールディレクトリのパスを返します。
    ///
    /// 実行ファイルの親ディレクトリから導出し、macOS では `Contents/Resources/`、
    /// Windows ではレジストリから、Linux では `APPDIR` 環境変数または実行ファイル
    /// ディレクトリをフォールバックとして使用します。
    pub fn install_dir(&self) -> anyhow::Result<std::path::PathBuf> {
        let exe_dir = std::env::current_exe()
            .context("failed to get exe path")?
            .parent()
            .context("exe has no parent directory")?
            .to_path_buf();

        #[cfg(target_os = "macos")]
        {
            // Contents/MacOS/exe -> Contents/Resources/
            Ok(exe_dir
                .parent()
                .context("exe_dir has no parent directory")?
                .join("Resources"))
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

    /// 各ディレクトリパスをログに出力します。
    ///
    /// 取得に失敗した場合は警告として出力します。
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
