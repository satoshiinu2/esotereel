use std::{
    borrow::Cow,
    collections::{BTreeMap, HashSet},
    str::FromStr,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::plugin::{
    NamespacedID,
    clip::component::ComponentHook,
    property::{
        parse::PropertySchemaRaw,
        value::{FieldTypeKind, FieldValue},
    },
};

/// コンポーネントが何を提供できるか（能力）。
///
/// 本体はこの能力を参照して、Preview・GUI・Timeline・Scheduler などの各システムと
/// 連携する。個々の実装を知らなくても、能力さえ合致すれば利用可能。
///
/// 文字列表現（TOML/API 側）は小文字の kebab-case を用いる
/// （例: `frame-provider`, `prefetchable`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentCapability {
    /// 時刻に対応する映像フレームを供給する。
    FrameProvider,
    /// 音声データを供給する。
    AudioProvider,
    /// 位置・回転・拡大縮小を提供する。
    Transformable,
    /// タイムライン上での位置・長さ編集を提供する。
    TimelinePositionable,
    /// データの先読みを提供する。
    Prefetchable,
    /// キーフレーム編集を提供する。
    Keyframeable,
    /// メディアソースファイルパスを提供する（マップ型プロパティのメンバーとして）。
    MediaSource,
    /// メディアオフセット（ティック単位のソース時間オフセット）を提供する（マップ型プロパティのメンバーとして）。
    MediaOffsetProvider,
    /// ドラッグドロップでファイルパスを書き込む宛先メンバー。
    /// コンポーネントが提供する能力ではなく、GUI からの入力を受け付けるマーカーとして使用する。
    Droppable,
    /// プラグイン独自の能力。
    Custom(String),
}

impl ComponentCapability {
    /// 小文字の kebab-case 文字列表現を返す（API / TOML 向け）。
    ///
    /// 例: `FrameProvider` → `"frame-provider"`。
    pub fn as_str(&self) -> String {
        match self {
            ComponentCapability::FrameProvider => "frame-provider".to_string(),
            ComponentCapability::AudioProvider => "audio-provider".to_string(),
            ComponentCapability::Transformable => "transformable".to_string(),
            ComponentCapability::TimelinePositionable => "timeline-positionable".to_string(),
            ComponentCapability::Prefetchable => "prefetchable".to_string(),
            ComponentCapability::Keyframeable => "keyframeable".to_string(),
            ComponentCapability::MediaSource => "media-source".to_string(),
            ComponentCapability::MediaOffsetProvider => "media-offset-provider".to_string(),
            ComponentCapability::Droppable => "droppable".to_string(),
            ComponentCapability::Custom(s) => s.clone(),
        }
    }

    /// この能力が要求する標準関数名を返す（Rust の trait メソッドに相当）。
    ///
    /// 各コンポーネントスクリプトは、宣言する能力に対応する標準関数を実装する必要がある。
    /// 例: `FrameProvider` → `"render_frame"`、`Prefetchable` → `"prefetch"`。
    /// `Custom` の場合は `None`（従来の hooks 仕組みは廃止）。
    pub fn standard_fn(&self) -> Option<&'static str> {
        match self {
            ComponentCapability::FrameProvider => Some("render_frame"),
            ComponentCapability::AudioProvider => Some("render_audio"),
            ComponentCapability::Transformable => Some("get_transform"),
            ComponentCapability::TimelinePositionable => Some("get_timeline_position"),
            ComponentCapability::Prefetchable => Some("prefetch"),
            ComponentCapability::Keyframeable => Some("get_keyframes"),
            ComponentCapability::MediaSource => None,
            ComponentCapability::MediaOffsetProvider => None,
            ComponentCapability::Droppable => None,
            ComponentCapability::Custom(_) => None,
        }
    }

    /// この能力の関数を呼び出すべきフックタイミング（実行順序決定用）。
    ///
    /// `Scheduler` はこのタイミング順に標準関数を呼び出します。
    /// 順序: `Prepare` < `Prefetch` < `Evaluate` < `Process` < `Finalize`。
    pub fn hook_timing(&self) -> ComponentHook {
        match self {
            ComponentCapability::Prefetchable => ComponentHook::Prefetch,
            ComponentCapability::FrameProvider => ComponentHook::Evaluate,
            ComponentCapability::AudioProvider => ComponentHook::Evaluate,
            ComponentCapability::Transformable => ComponentHook::Process,
            ComponentCapability::TimelinePositionable => ComponentHook::Process,
            ComponentCapability::Keyframeable => ComponentHook::Process,
            ComponentCapability::MediaSource => ComponentHook::Evaluate,
            ComponentCapability::MediaOffsetProvider => ComponentHook::Evaluate,
            ComponentCapability::Droppable => ComponentHook::Evaluate,
            ComponentCapability::Custom(_) => ComponentHook::Evaluate,
        }
    }
}

impl FromStr for ComponentCapability {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "frame-provider" => ComponentCapability::FrameProvider,
            "audio-provider" => ComponentCapability::AudioProvider,
            "transformable" => ComponentCapability::Transformable,
            "timeline-positionable" => ComponentCapability::TimelinePositionable,
            "prefetchable" => ComponentCapability::Prefetchable,
            "keyframeable" => ComponentCapability::Keyframeable,
            "media-source" => ComponentCapability::MediaSource,
            "media-offset-provider" => ComponentCapability::MediaOffsetProvider,
            "droppable" => ComponentCapability::Droppable,
            other => ComponentCapability::Custom(other.to_string()),
        })
    }
}

pub mod parse;
pub mod path;
pub use path::PropertyPath;
pub mod value;

#[derive(Debug, serde::Deserialize)]
struct SchemaFile {
    #[serde(default)]
    fields: Vec<PropertySchemaRaw>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PropertySchema {
    pub key: NamespacedID,
    pub category: Vec<String>,
    pub label: String,
    pub kind: FieldTypeKind,
    pub default: FieldValue,
    /// このプロパティが提供する能力。一部のプロパティ（例: メディアソース、変換）は
    /// コンポーネントの能力と紐付けられ、レンダラー等は能力からプロパティを参照する。
    /// 設定スキーマなど能力に関係ないプロパティは None。
    #[serde(default)]
    pub capability: Option<ComponentCapability>,
    /// マップ型プロパティのメンバー（キー名）に対する能力マッピング。
    /// 例: メディアマップ内の `source_offset` メンバーが `media-offset-provider` 能力を持つ場合、
    /// レンダラーは能力からどのメンバーを読むかを解決できる。
    /// ハードコードされたメンバキー文字列（"source", "source_offset" など）に依存しないため、
    /// プラグインは任意のメンバー名を宣言できる。
    #[serde(default)]
    pub member_capabilities: BTreeMap<String, ComponentCapability>,
}

impl PropertySchema {
    /// グループ型（複数フィールドをまとめる）プロパティスキーマを構築します。
    ///
    /// `reference` はグループを参照する `NamespacedID`、`fields` は子フィールドの一覧です。
    /// デフォルト値は子フィールドのデフォルトを `Map` として展開します。
    pub fn group(
        key: NamespacedID,
        label: String,
        reference: NamespacedID,
        fields: Vec<Self>,
    ) -> Self {
        let default = FieldValue::Map(
            fields
                .iter()
                .map(|f| (f.key.local_id().to_owned(), f.default.clone()))
                .collect(),
        );
        Self {
            key,
            category: Vec::new(),
            label,
            kind: FieldTypeKind::Group { reference, fields },
            default,
            capability: None,
            member_capabilities: BTreeMap::new(),
        }
    }

    /// グループ構造を「エディタ用の平坦な葉（leaf）一覧」に展開します。
    ///
    /// スキーマ自体と保存値はツリー構造のまま残し、表示・編集用にのみ平坦化します。
    /// 戻り値は `(PropertyPath, PropertySchema)` のベクタで、パスは `std:group/leaf` 形式です。
    pub fn leaf_fields(fields: &[Self]) -> anyhow::Result<Vec<(PropertyPath, Self)>> {
        fn visit(
            field: &PropertySchema,
            path: PropertyPath,
            groups: &[String],
            default: &FieldValue,
            out: &mut Vec<(PropertyPath, PropertySchema)>,
        ) -> anyhow::Result<()> {
            if let FieldTypeKind::Group { fields, .. } = &field.kind {
                let mut groups = groups.to_vec();
                groups.push(field.label.clone());
                groups.extend(field.category.iter().cloned());
                for child in fields {
                    let effective = match default {
                        FieldValue::Map(values) => {
                            values.get(child.key.local_id()).unwrap_or(&child.default)
                        }
                        _ => &child.default,
                    };
                    visit(
                        child,
                        path.child(child.key.local_id())?,
                        &groups,
                        effective,
                        out,
                    )?;
                }
            } else {
                let mut leaf = field.clone();
                leaf.default = default.clone();
                leaf.category = groups
                    .iter()
                    .cloned()
                    .chain(field.category.iter().cloned())
                    .collect();
                out.push((path, leaf));
            }
            Ok(())
        }
        let mut out = Vec::new();
        for field in fields {
            visit(
                field,
                field.key.clone().into(),
                &[],
                &field.default,
                &mut out,
            )?;
        }
        Ok(out)
    }

    /// TOML テキストからプロパティスキーマの一覧をパースします。
    ///
    /// 重複するキーや不正な `kind` はエラーとなります（`validate_properties` で検証）。
    pub fn parse_toml(text: &str, plugin_id: &str) -> anyhow::Result<Vec<PropertySchema>> {
        let parsed: SchemaFile = toml::from_str(text).map_err(|e| {
            anyhow::anyhow!(
                "failed to parse settings schema TOML in {}: {}",
                plugin_id,
                e
            )
        })?;

        let properties = parsed
            .fields
            .into_iter()
            .map(|raw| Self::parse_one(raw, plugin_id))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Self::validate_properties(&properties)
            .with_context(|| format!("schema validation failed in `{plugin_id}`"))?;

        Ok(properties)
    }

    /// 単一の生プロパティ定義（`PropertySchemaRaw`）をパースして `PropertySchema` を構築します。
    ///
    /// `kind` は TOML 値から `FieldTypeKind` に変換し、`default` が省略された場合は
    /// `kind` からデフォルト値を導出します。
    pub fn parse_one(raw: PropertySchemaRaw, plugin_id: &str) -> anyhow::Result<PropertySchema> {
        let mut kind = FieldTypeKind::from_toml_value(&raw.kind)
            .with_context(|| format!("invalid `kind` for key `{}` in `{plugin_id}`", raw.key))?;

        // Group 型の場合、子フィールドを再帰的にパースして kind に設定する
        if let FieldTypeKind::Group { fields, .. } = &mut kind {
            match &raw.fields {
                Some(raw_fields) => {
                    *fields = raw_fields
                        .iter()
                        .map(|rf| PropertySchema::parse_one(rf.clone(), plugin_id))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                }
                None => *fields = Vec::new(),
            }
        }

        let capability = match &raw.capability {
            Some(cap_str) => Some(ComponentCapability::from_str(cap_str).with_context(|| {
                format!(
                    "invalid `capability` `{cap_str}` for key `{}` in `{plugin_id}`",
                    raw.key
                )
            })?),
            None => None,
        };

        // メンバーレベルの能力マッピング（マップ型プロパティ用）
        let member_capabilities = raw
            .member_capabilities
            .iter()
            .map(|(member_key, cap_str)| {
                ComponentCapability::from_str(cap_str)
                    .map(|cap| (member_key.clone(), cap))
                    .with_context(|| {
                        format!(
                            "invalid `member_capabilities` capability `{cap_str}` for member `{member_key}` on key `{}` in `{plugin_id}`",
                            raw.key
                        )
                    })
            })
            .collect::<anyhow::Result<BTreeMap<String, ComponentCapability>>>()?;

        // default定義されていなかったらフォールバック
        let default = match raw.default {
            Some(v) => kind.parse_toml_to(&v).with_context(|| {
                format!(
                    "could not derive default for key `{}` in `{plugin_id}`",
                    raw.key
                )
            })?,
            None => kind.to_default_value().with_context(|| {
                format!(
                    "could not derive default for key `{}` in `{plugin_id}`",
                    raw.key
                )
            })?,
        };

        Ok(PropertySchema {
            key: NamespacedID::new(plugin_id, &raw.key)?,
            category: raw.category,
            label: raw.label,
            kind,
            default,
            capability,
            member_capabilities,
        })
    }

    /// プロパティ一覧を検証します。
    ///
    /// 重複するキーや、空の `Enum` オプション、`Group` 内の重複メンバー名を拒否します。
    pub fn validate_properties(properties: &[PropertySchema]) -> anyhow::Result<()> {
        let mut seen = HashSet::new();
        for property in properties {
            if !seen.insert(&property.key) {
                anyhow::bail!("duplicate property key `{}`", property.key,);
            }
            Self::validate_kind(&property.key, &property.kind)?;
        }
        Ok(())
    }

    fn validate_kind(key: &NamespacedID, kind: &FieldTypeKind) -> anyhow::Result<()> {
        match kind {
            FieldTypeKind::Enum { options } if options.is_empty() => {
                anyhow::bail!("Enum kind for key `{key}` has no options")
            }
            FieldTypeKind::Group { fields, .. } => {
                let mut names = HashSet::new();
                for field in fields {
                    anyhow::ensure!(
                        names.insert(field.key.local_id()),
                        "duplicate group member `{}`",
                        field.key.local_id()
                    );
                }
                Self::validate_properties(fields)
            }
            FieldTypeKind::Array { item_kind } => Self::validate_kind(key, item_kind)
                .with_context(|| format!("invalid Array item_kind for key `{key}`")),
            FieldTypeKind::Map { value_kind, .. } => Self::validate_kind(key, value_kind)
                .with_context(|| format!("invalid Map value_kind for key `{key}`")),
            _ => Ok(()),
        }
    }
}

/// プロパティを保持・読み書きするホスト（所有者）を表すトレイトです。
///
/// コンポーネントやクリップインスタンスがこのトレイトを実装し、
/// スケジューラやレンダラーは `key` 経由でプロパティ値にアクセスします。
/// ID が見つからない場合は `None` やエラーを返し、アプリをクラッシュさせません
/// （AGENTS.md 第9条：パニックの使用制限）。
pub trait PropertyHost {
    /// このホストが宣言するプロパティスキーマの一覧を返します。
    fn properties(&self) -> Cow<'_, [PropertySchema]>;
    /// 指定キーのプロパティ値を返します。存在しなければ `None` を返します。
    fn get_value(&self, key: &NamespacedID) -> Option<&FieldValue>;
    /// 指定キーのプロパティ値を設定します。
    fn set_value(&mut self, key: NamespacedID, value: FieldValue) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_one_string_with_default() {
        let raw = PropertySchemaRaw {
            key: "source".to_string(),
            label: "Source".to_string(),
            category: vec!["Media".to_string()],
            kind: toml::Value::Table(
                [(
                    "type".to_string(),
                    toml::Value::String("string".to_string()),
                )]
                .into_iter()
                .collect(),
            ),
            default: Some(toml::Value::String("default.mp4".to_string())),
            capability: None,
            member_capabilities: BTreeMap::new(),
                        fields: None,
        };
        let schema = PropertySchema::parse_one(raw, "std").unwrap();
        assert_eq!(schema.key.full(), "std:source");
        assert_eq!(schema.label, "Source");
        assert_eq!(schema.category, vec!["Media"]);
        assert!(matches!(schema.kind, FieldTypeKind::String));
        assert_eq!(
            schema.default,
            FieldValue::String("default.mp4".to_string())
        );
    }

    #[test]
    fn parse_one_derives_default_when_omitted() {
        let raw = PropertySchemaRaw {
            key: "count".to_string(),
            label: "Count".to_string(),
            category: vec![],
            kind: toml::Value::Table(
                [("type".to_string(), toml::Value::String("int".to_string()))]
                    .into_iter()
                    .collect(),
            ),
            default: None,
            capability: None,
            member_capabilities: BTreeMap::new(),
                        fields: None,
        };
        let schema = PropertySchema::parse_one(raw, "std").unwrap();
        assert_eq!(schema.default, FieldValue::Int(0));
    }

    #[test]
    fn parse_one_rejects_unknown_kind() {
        let raw = PropertySchemaRaw {
            key: "bad".to_string(),
            label: "Bad".to_string(),
            category: vec![],
            kind: toml::Value::String("not-a-kind".to_string()),
            default: None,
            capability: None,
            member_capabilities: BTreeMap::new(),
                        fields: None,
        };
        assert!(PropertySchema::parse_one(raw, "std").is_err());
    }

    #[test]
    fn validate_properties_rejects_duplicate_keys() {
        let key = NamespacedID::new("std", "dup").unwrap();
        let props = vec![
            PropertySchema {
                key: key.clone(),
                category: vec![],
                label: "First".to_string(),
                kind: FieldTypeKind::String,
                default: FieldValue::String("a".to_string()),
                capability: None,
                member_capabilities: BTreeMap::new(),
                            },
            PropertySchema {
                key,
                category: vec![],
                label: "Second".to_string(),
                kind: FieldTypeKind::String,
                default: FieldValue::String("b".to_string()),
                capability: None,
                member_capabilities: BTreeMap::new(),
                            },
        ];
        assert!(PropertySchema::validate_properties(&props).is_err());
    }

    #[test]
    fn group_constructor_builds_nested_default() {
        let child = PropertySchema {
            key: NamespacedID::new("std", "font_size").unwrap(),
            category: vec![],
            label: "Font Size".to_string(),
            kind: FieldTypeKind::Float {
                min: None,
                max: None,
                step: 1.0,
            },
            default: FieldValue::Float(24.0),
            capability: None,
            member_capabilities: BTreeMap::new(),
                    };
        let group = PropertySchema::group(
            NamespacedID::new("std", "style").unwrap(),
            "Style".to_string(),
            NamespacedID::new("std", "style-ref").unwrap(),
            vec![child],
        );
        assert!(matches!(
            group.kind,
            FieldTypeKind::Group { ref reference, .. } if reference.full() == "std:style-ref"
        ));
        assert!(matches!(
            group.default,
            FieldValue::Map(ref m) if m.get("font_size") == Some(&FieldValue::Float(24.0))
        ));
    }

    #[test]
    fn parse_toml_rejects_duplicate_property_keys() {
        let text = r#"
[[fields]]
key = "dup"
label = "Dup"
kind = { type = "string" }
[[fields]]
key = "dup"
label = "Dup again"
kind = { type = "string" }
"#;
        assert!(PropertySchema::parse_toml(text, "std").is_err());
    }

    #[test]
    fn leaf_fields_flattens_group_members() {
        let inner = PropertySchema {
            key: NamespacedID::new("std", "x").unwrap(),
            category: vec![],
            label: "X".to_string(),
            kind: FieldTypeKind::Int {
                min: None,
                max: None,
            },
            default: FieldValue::Int(1),
            capability: None,
            member_capabilities: BTreeMap::new(),
                    };
        let group = PropertySchema::group(
            NamespacedID::new("std", "g").unwrap(),
            "Group".to_string(),
            NamespacedID::new("std", "g-ref").unwrap(),
            vec![inner],
        );
        let leaves = PropertySchema::leaf_fields(&[group]).unwrap();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].0.to_string(), "std:g/x");
        assert_eq!(leaves[0].1.label, "X");
    }
}
