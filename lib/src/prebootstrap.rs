use std::{collections::BTreeMap, fs, path::Path};

use anyhow::Context;

/// ログレベルの列挙です。
///
/// 設定ファイル（TOML）から小文字でデシリアライズされます。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// ログ出力なし
    Off,
    /// エラーのみ
    Error,
    /// 警告以上
    Warn,
    /// 情報以上（既定値）
    #[default]
    Info,
    /// デバッグ以上
    Debug,
    /// トレース以上
    Trace,
}

impl LogLevel {
    /// 文字列から `LogLevel` をパースします。大文字小文字は区別しません。
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "error" => Some(Self::Error),
            "warn" | "warning" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }

    /// `log::LevelFilter` に変換します。
    pub fn as_filter(self) -> log::LevelFilter {
        match self {
            Self::Off => log::LevelFilter::Off,
            Self::Error => log::LevelFilter::Error,
            Self::Warn => log::LevelFilter::Warn,
            Self::Info => log::LevelFilter::Info,
            Self::Debug => log::LevelFilter::Debug,
            Self::Trace => log::LevelFilter::Trace,
        }
    }

    /// 表示用の静的な文字列名を返します。
    pub fn as_name(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Error => "Error",
            Self::Warn => "Warn",
            Self::Info => "Info",
            Self::Debug => "Debug",
            Self::Trace => "Trace",
        }
    }
}

/// ログ設定です。
///
/// `#[serde(default)]` を付与し、未指定のフィールドは既定値で補完されます。
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct LoggingSettings {
    /// 基準となるログレベル
    pub level: LogLevel,
    /// モジュール/タグごとの個別フィルタ
    pub filters: BTreeMap<String, LogLevel>,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            filters: BTreeMap::new(),
        }
    }
}

/// 起動時に読み込まれる小さな設定ファイルです。
///
/// 設定ファイルが存在しない場合は既定値が使用されます。
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct PreBootstrapSettings {
    /// ログ設定
    pub logging: LoggingSettings,
}

impl PreBootstrapSettings {
    /// 設定ファイルを読み込みます。ファイルが存在しない場合は既定値を返します。
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read prebootstrap settings {}", path.display()))?;
        toml::from_str(&text)
            .with_context(|| format!("failed to parse prebootstrap settings {}", path.display()))
    }

    /// 設定ファイルを保存します。親ディレクトリが存在しない場合は作成します。
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(path, text)
            .with_context(|| format!("failed to write prebootstrap settings {}", path.display()))
    }
}
