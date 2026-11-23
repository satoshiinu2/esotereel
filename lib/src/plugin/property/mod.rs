use std::{
    borrow::Cow,
    collections::HashSet,
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

/// コンポーネント能力の呼び出しの性質。
///
/// 能力ごとに呼び出し規約が異なるため、関数名やタイミングを個別の
/// メソッドで分散させず、この enum で性質ごと表す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityCall {
    /// `Scheduler` が通常実行時に `(ctx, props)` で呼ぶ。
    Scheduler {
        fn_name: &'static str,
        hook: ComponentHook,
    },
    /// ファイルドロップ時に生成専用経路が `(source_path)` で1回だけ呼ぶ。
    /// 戻り値は初期プロパティとして適用される。
    FileDrop { fn_name: &'static str },
    /// 対応する関数を持たない。プロパティのメンバー等を能力で参照するためだけに使う。
    Marker,
}

/// `ComponentCapability` の全バリアントを1つの表から生成するマクロ。
/// `as_str` / `call` / `FromStr` をすべて生成する。
macro_rules! component_capabilities {
    ($(
        $(#[$doc:meta])*
        $variant:ident: $name:literal, $call:expr;
    )*) => {
        /// コンポーネントが何を提供できるか（能力）。
        ///
        /// 本体はこの能力を参照して、Preview・GUI・Timeline・Scheduler などの各システムと
        /// 連携する。個々の実装を知らなくても、能力さえ合致すれば利用可能。
        ///
        /// 文字列表現（TOML/API 側）は小文字の kebab-case を用いる
        /// （例: `frame-provider`, `prefetchable`）。
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub enum ComponentCapability {
            $(
                $(#[$doc])*
                $variant,
            )*
        }

        impl ComponentCapability {
            /// 小文字の kebab-case 文字列表現を返す（API / TOML 向け）。
            pub fn as_str(&self) -> String {
                match self {
                    $(ComponentCapability::$variant => $name.to_string(),)*
                }
            }

            /// この能力の呼び出しの性質を返す。
            /// 各コンポーネントスクリプトは、宣言する能力に対応する標準関数を実装する必要がある。
            pub fn call(&self) -> CapabilityCall {
                match self {
                    $(ComponentCapability::$variant => $call,)*
                }
            }
        }

        impl FromStr for ComponentCapability {
            type Err = anyhow::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(match s {
                    $($name => ComponentCapability::$variant,)*
                    other => anyhow::bail!("unknown component capability `{other}`"),
                })
            }
        }
    };
}

component_capabilities! {
    /// 時刻に対応する映像フレームを供給する。
    FrameProvider: "frame-provider", CapabilityCall::Scheduler { fn_name: "render_frame", hook: ComponentHook::Evaluate };
    /// 音声データを供給する。
    AudioProvider: "audio-provider", CapabilityCall::Scheduler { fn_name: "render_audio", hook: ComponentHook::Evaluate };
    /// 位置・回転・拡大縮小を提供する。
    Transformable: "transformable", CapabilityCall::Scheduler { fn_name: "get_transform", hook: ComponentHook::Process };
    /// タイムライン上での位置・長さ編集を提供する。
    TimelinePositionable: "timeline-positionable", CapabilityCall::Scheduler { fn_name: "get_timeline_position", hook: ComponentHook::Process };
    /// データの先読みを提供する。
    Prefetchable: "prefetchable", CapabilityCall::Scheduler { fn_name: "prefetch", hook: ComponentHook::Prefetch };
    /// キーフレーム編集を提供する。
    Keyframeable: "keyframeable", CapabilityCall::Scheduler { fn_name: "get_keyframes", hook: ComponentHook::Process };
    /// メディアソースファイルパスを提供する（マップ型プロパティのメンバーとして）。
    MediaSource: "media-source", CapabilityCall::Marker;
    /// メディアオフセット（ティック単位のソース時間オフセット）を提供する（マップ型プロパティのメンバーとして）。
    MediaOffsetProvider: "media-offset-provider", CapabilityCall::Marker;
    /// ファイルのドロップ時に初期プロパティを生成する。
    FileDropHandler: "file-drop-handler", CapabilityCall::FileDrop { fn_name: "handle_file_drop" };
}

pub mod key;
pub mod parse;
pub mod path;
pub use key::{PropertyKey, validate_local_name};
pub use path::PropertyPath;
pub mod value;

#[derive(Debug, serde::Deserialize)]
struct SchemaFile {
    #[serde(default)]
    fields: Vec<PropertySchemaRaw>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PropertySchema {
    pub key: PropertyKey,
    pub category: Vec<String>,
    pub label: String,
    pub kind: FieldTypeKind,
    pub default: FieldValue,
    /// このプロパティが提供する能力。一部のプロパティ（例: メディアソース、変換）は
    /// コンポーネントの能力と紐付けられ、レンダラー等は能力からプロパティを参照する。
    /// 設定スキーマなど能力に関係ないプロパティは None。
    #[serde(default)]
    pub capability: Option<ComponentCapability>,
}

/// プロパティキーの名前空間スコープ。パース時にキーの所有者を決めます。
enum KeyScope<'a> {
    /// プラグイン名前空間直下（設定スキーマ等）。
    Namespaced { plugin_id: &'a str },
    /// コンポーネント所有。
    Component { owner: &'a NamespacedID },
}

impl KeyScope<'_> {
    fn plugin_id(&self) -> &str {
        match self {
            KeyScope::Namespaced { plugin_id } => plugin_id,
            KeyScope::Component { owner } => owner.plugin_id(),
        }
    }

    fn key(&self, raw_key: &str) -> anyhow::Result<PropertyKey> {
        match self {
            KeyScope::Namespaced { plugin_id } => Ok(PropertyKey::from_namespaced(
                NamespacedID::new(plugin_id, raw_key)?,
            )),
            KeyScope::Component { owner } => PropertyKey::new((*owner).clone(), raw_key),
        }
    }
}

impl PropertySchema {
    /// グループ型（複数フィールドをまとめる）プロパティスキーマを構築します。
    ///
    /// `fields` は子フィールドの一覧です。グループはフィールド宣言だけで
    /// 構成され、デフォルト値は子フィールドのデフォルトを `Map` として展開します。
    pub fn group(key: PropertyKey, label: String, fields: Vec<Self>) -> Self {
        let default = FieldValue::Map(
            fields
                .iter()
                .map(|f| (f.key.local_name().to_owned(), f.default.clone()))
                .collect(),
        );
        Self {
            key,
            category: Vec::new(),
            label,
            kind: FieldTypeKind::Group { fields },
            default,
            capability: None,
        }
    }

    /// グループ型プロパティのメンバーのうち、指定能力を宣言するメンバーの
    /// ローカル名を逆引きする。メンバー自身が `capability` を宣言するため、
    /// 存在しないメンバーを指すことはない。
    pub fn member_key_by_capability(&self, capability: &ComponentCapability) -> Option<&str> {
        let FieldTypeKind::Group { fields } = &self.kind else {
            return None;
        };
        fields
            .iter()
            .find(|field| field.capability.as_ref() == Some(capability))
            .map(|field| field.key.local_name())
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
                            values.get(child.key.local_name()).unwrap_or(&child.default)
                        }
                        _ => &child.default,
                    };
                    visit(
                        child,
                        path.child(child.key.local_name())?,
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
    /// キーはプラグイン名前空間直下の識別子（`plugin_id:raw.key`）になります。
    /// 設定スキーマやシェーダ固有プロパティなど、コンポーネントに属さない
    /// プロパティ向けです。コンポーネントのプロパティには `parse_for_component` を
    /// 使ってください。
    ///
    /// `kind` は TOML 値から `FieldTypeKind` に変換し、`default` が省略された場合は
    /// `kind` からデフォルト値を導出します。
    pub fn parse_one(raw: PropertySchemaRaw, plugin_id: &str) -> anyhow::Result<PropertySchema> {
        Self::parse_scoped(raw, &KeyScope::Namespaced { plugin_id })
    }

    /// コンポーネントが宣言するプロパティとして、生定義をパースします。
    ///
    /// キーは所有コンポーネントの ID とローカル名の組（アドレスは
    /// `plugin:component#name`）になります。TOML 側のキーにはローカル名のみを
    /// 書きます。Group の子フィールドも同じ所有者の下でパースされます。
    pub fn parse_for_component(
        raw: PropertySchemaRaw,
        owner: &NamespacedID,
    ) -> anyhow::Result<PropertySchema> {
        Self::parse_scoped(raw, &KeyScope::Component { owner })
    }

    fn parse_scoped(raw: PropertySchemaRaw, scope: &KeyScope) -> anyhow::Result<PropertySchema> {
        let plugin_id = scope.plugin_id();
        let key = scope.key(&raw.key)?;
        let mut kind = FieldTypeKind::from_toml_value(&raw.kind)
            .with_context(|| format!("invalid `kind` for key `{}` in `{plugin_id}`", raw.key))?;

        // Group 型の場合、子フィールドを再帰的にパースして kind に設定する
        if let FieldTypeKind::Group { fields, .. } = &mut kind {
            match &raw.fields {
                Some(raw_fields) => {
                    *fields = raw_fields
                        .iter()
                        .map(|rf| PropertySchema::parse_scoped(rf.clone(), scope))
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
            key,
            category: raw.category,
            label: raw.label,
            kind,
            default,
            capability,
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

    fn validate_kind(key: &PropertyKey, kind: &FieldTypeKind) -> anyhow::Result<()> {
        match kind {
            FieldTypeKind::Enum { options } if options.is_empty() => {
                anyhow::bail!("Enum kind for key `{key}` has no options")
            }
            FieldTypeKind::Group { fields, .. } => {
                let mut names = HashSet::new();
                for field in fields {
                    anyhow::ensure!(
                        names.insert(field.key.local_name()),
                        "duplicate group member `{}`",
                        field.key.local_name()
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
    fn get_value(&self, key: &PropertyKey) -> Option<&FieldValue>;
    /// 指定キーのプロパティ値を設定します。
    fn set_value(&mut self, key: PropertyKey, value: FieldValue) -> anyhow::Result<()>;
}

#[cfg(test)]
#[path = "../../../tests/unit/plugin/property/mod_tests.rs"]
mod tests;
