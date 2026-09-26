use std::{collections::BTreeMap, path::PathBuf};

use rkyv::{Archive, CheckBytes, bytecheck, with::Map};

use crate::util::color::RgbaColor;
use crate::util::rkyv_with::PathAsString;

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("型が一致しません: 期待値 {expected}, 実際値 {got}")]
    TypeMismatch { expected: &'static str, got: String },
    #[error("数値が範囲外です: {0}")]
    OutOfRange(String),
    #[error("無効な色フォーマットです: {0}")]
    InvalidColor(String),
    #[error("無効な列挙型(Enum)の値です: {0}")]
    InvalidOption(String),
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum FieldTypeKind {
    Bool,
    Int {
        min: i64,
        max: i64,
    },
    Float {
        min: f64,
        max: f64,
        step: f64,
    },
    Enum {
        options: Vec<String>,
    },
    String,
    FilePath {
        extensions: Option<Vec<String>>,
    },
    Color,
    Array {
        item_kind: Box<FieldTypeKind>,
    },
    Map {
        value_kind: Box<FieldTypeKind>,
        known_keys: Option<Vec<String>>,
    },
}

#[derive(
    Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[archive_attr(
    derive(CheckBytes),
    check_bytes(
        bound = "__C: rkyv::validation::ArchiveContext, <__C as rkyv::Fallible>::Error: std::error::Error"
    )
)]
#[archive(bound(
    serialize = "__S: rkyv::ser::Serializer + rkyv::ser::ScratchSpace",
    deserialize = "__D: rkyv::Fallible"
))]
pub enum FieldValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Enum(String),
    String(String),
    Path(#[with(Map<PathAsString>)] Vec<PathBuf>),
    Color(RgbaColor),
    Array(
        #[omit_bounds]
        #[archive_attr(omit_bounds)]
        Vec<FieldValue>,
    ),

    Map(
        #[omit_bounds]
        #[archive_attr(omit_bounds)]
        BTreeMap<String, FieldValue>,
    ),
}

impl ToString for FieldValue {
    fn to_string(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Enum(value) | Self::String(value) => value.clone(),
            Self::Path(paths) => paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(", "),
            Self::Color(color) => {
                format!(
                    "#{:02X}{:02X}{:02X}{:02X}",
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                    (color.a * 255.0) as u8
                )
            }
            Self::Array(values) => values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            Self::Map(values) => values
                .iter()
                .map(|(key, value)| format!("{key}: {}", value.to_string()))
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

impl FieldTypeKind {
    pub fn parse_toml_to(&self, value: &toml::Value) -> Result<FieldValue, ConvertError> {
        match (self, value) {
            // --- Bool ---
            (FieldTypeKind::Bool, toml::Value::Boolean(b)) => Ok(FieldValue::Bool(*b)),

            // --- Int ---
            (FieldTypeKind::Int { min, max }, toml::Value::Integer(i)) => {
                if i >= min && i <= max {
                    Ok(FieldValue::Int(*i))
                } else {
                    Err(ConvertError::OutOfRange(format!(
                        "値 {i} は範囲 [{min}, {max}] 外です"
                    )))
                }
            }

            // --- Float ---
            (FieldTypeKind::Float { min, max, .. }, toml::Value::Float(f)) => {
                if f >= min && f <= max {
                    Ok(FieldValue::Float(*f))
                } else {
                    Err(ConvertError::OutOfRange(format!(
                        "値 {f} は範囲 [{min}, {max}] 外です"
                    )))
                }
            }
            // TOMLで 1.0 が Integer(1) と解釈された場合のエラー救済
            (FieldTypeKind::Float { min, max, .. }, toml::Value::Integer(i)) => {
                let f = *i as f64;
                if f >= *min && f <= *max {
                    Ok(FieldValue::Float(f))
                } else {
                    Err(ConvertError::OutOfRange(format!(
                        "値 {f} は範囲 [{min}, {max}] 外です"
                    )))
                }
            }

            // --- Enum ---
            (FieldTypeKind::Enum { options }, toml::Value::String(s)) => {
                if options.contains(s) {
                    Ok(FieldValue::Enum(s.clone()))
                } else {
                    Err(ConvertError::InvalidOption(format!(
                        "\"{s}\" は許可された選択肢 {options:?} に含まれません"
                    )))
                }
            }

            // --- String ---
            (FieldTypeKind::String, toml::Value::String(s)) => Ok(FieldValue::String(s.clone())),

            // --- FilePath ---
            // 単一文字列、または配列形式のどちらの TOML 表現にも対応
            (FieldTypeKind::FilePath { .. }, toml::Value::String(s)) => {
                Ok(FieldValue::Path(vec![PathBuf::from(s)]))
            }
            (FieldTypeKind::FilePath { .. }, toml::Value::Array(arr)) => {
                let mut paths = Vec::new();
                for item in arr {
                    if let toml::Value::String(s) = item {
                        paths.push(PathBuf::from(s));
                    } else {
                        return Err(ConvertError::TypeMismatch {
                            expected: "String (Path)",
                            got: item.type_str().to_string(),
                        });
                    }
                }
                Ok(FieldValue::Path(paths))
            }

            // --- Color ---
            (FieldTypeKind::Color, toml::Value::String(s)) => {
                let color = Self::parse_rgba_color(s)?;
                Ok(FieldValue::Color(color))
            }

            // --- Array ---
            (FieldTypeKind::Array { item_kind }, toml::Value::Array(arr)) => {
                let mut result = Vec::with_capacity(arr.len());
                for item in arr {
                    let parsed_item = FieldTypeKind::parse_toml_to(&item_kind, item)?;
                    result.push(parsed_item);
                }
                Ok(FieldValue::Array(result))
            }

            // --- Map ---
            (
                FieldTypeKind::Map {
                    value_kind,
                    known_keys,
                },
                toml::Value::Table(table),
            ) => {
                let mut result = BTreeMap::new();
                for (key, val) in table {
                    if let Some(keys) = known_keys {
                        if !keys.contains(key) {
                            continue; // 登録外のキーをスキップするか、エラーにするかを選択
                        }
                    }
                    let parsed_val = FieldTypeKind::parse_toml_to(&value_kind, val)?;
                    result.insert(key.clone(), parsed_val);
                }
                Ok(FieldValue::Map(result))
            }

            // --- 型不一致 ---
            (expected_kind, actual_val) => Err(ConvertError::TypeMismatch {
                expected: match expected_kind {
                    FieldTypeKind::Bool => "Boolean",
                    FieldTypeKind::Int { .. } => "Integer",
                    FieldTypeKind::Float { .. } => "Float",
                    FieldTypeKind::Enum { .. } => "String (Enum)",
                    FieldTypeKind::String => "String",
                    FieldTypeKind::FilePath { .. } => "String or Array of Strings",
                    FieldTypeKind::Color => "String (Hex/Color)",
                    FieldTypeKind::Array { .. } => "Array",
                    FieldTypeKind::Map { .. } => "Table",
                },
                got: actual_val.type_str().to_string(),
            }),
        }
    }

    // 補助関数: `#RRGGBBAA` または `#RRGGBB` の文字列から RgbaColor へ変換
    fn parse_rgba_color(hex_str: &str) -> Result<RgbaColor, ConvertError> {
        let hex = hex_str.trim_start_matches('#');
        let (r8, g8, b8, a8) = match hex.len() {
            6 => (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
                Ok(255),
            ),
            8 => (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
                u8::from_str_radix(&hex[6..8], 16),
            ),
            _ => return Err(ConvertError::InvalidColor(hex_str.to_string())),
        };

        match (r8, g8, b8, a8) {
            (Ok(r8), Ok(g8), Ok(b8), Ok(a8)) => Ok(RgbaColor {
                r: r8 as f32 / 255.0,
                g: g8 as f32 / 255.0,
                b: b8 as f32 / 255.0,
                a: a8 as f32 / 255.0,
            }),
            _ => Err(ConvertError::InvalidColor(hex_str.to_string())),
        }
    }
}
