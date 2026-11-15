use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::Context;
use rhai::{Dynamic, Engine};
use serde::{Deserialize, Serialize};

pub use crate::plugin::property::{
    ComponentCapability, PropertySchema, parse::PropertySchemaRaw, value::FieldValue,
};
use crate::plugin::property::path::PropertyPath;
use crate::plugin::script::api::PluginRenderContext;
use crate::plugin::script::bridge::field_values_to_rhai_map;
use crate::plugin::{NamespacedID, PluginLoader};
use crate::project::clip::ClipBindingValue;

/// コンポーネントが実行する処理のタイミング（フック）。
///
/// スケジューラ内部で実行順序を決定するために使用する。
/// `BTreeMap` のキーとして使うため `Ord` も実装している（宣言順 = 実行順）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ComponentHook {
    Prepare,
    Prefetch,
    Evaluate,
    Process,
    Finalize,
}

/// コンポーネントの定義。
///
/// プラグインが TOML で宣言し、Component Registry に登録される。
/// `kind` は `NamespacedID`（例: `std:media-provider`）であり、ネイティブ側では
/// 種別判定より `capabilities` を通じた能力ベースのアクセスを原則とする。
/// コンポーネントは独立した部品であり、どのクリップ種別がどのコンポーネントで
/// 構成されるかはクリップ種別（`ClipKind`）側が宣言する。定義は Kind ごとに1つ、
/// 状態（Runtime State）は Clip Instance ごとに独立する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentDefinition {
    pub id: NamespacedID,
    pub name: String,
    /// このコンポーネントの種別を参照する NamespacedID。
    /// Capability 経由でのアクセスが基本のため、ここでは単なる参照として保持する。
    pub kind: NamespacedID,
    pub capabilities: BTreeSet<ComponentCapability>,
    pub editor: Option<String>,
    /// このコンポーネント専用の Rhai スクリプトファイルパス（プラグインディレクトリからの相対パス）。
    /// プラグイン共通の script.rhai ではなく、コンポーネントごとに個別のスクリプトを持つ。
    #[serde(default)]
    pub script: Option<String>,
    /// このコンポーネントが提供するプロパティスキーマの一覧。
    /// コンポーネントが自身のプロパティを宣言し、ClipInstance は各コンポーネントの
    /// プロパティを保持・渡す。スケジューラはフック実行時にクリップインスタンスの
    /// プロパティ全体をスクリプトに渡すため、兄弟コンポーネントが宣言した
    /// プロパティも参照できる。
    #[serde(default)]
    pub properties: Vec<PropertySchema>,
}

impl ComponentDefinition {
    /// `FrameProvider` 能力を持つプロパティの `member_capabilities` から、
    /// 指定能力を持つメンバーのキーを逆引きする。
    ///
    /// ハードコードされたメンバキー（`"source_offset"` 等）に依存せず、
    /// コンポーネントが宣言する能力からプロパティを参照する（AGENTS.md 第9条）。
    ///
    /// 例: メディアマップ内の `source_offset` メンバーが `media-offset-provider`
    /// 能力を宣言している場合、そのメンバーのキーを返す。
    pub fn member_key_by_capability(&self, capability: &ComponentCapability) -> Option<&String> {
        self.properties
            .iter()
            .find(|p| p.capability == Some(ComponentCapability::FrameProvider))
            .and_then(|p| {
                p.member_capabilities
                    .iter()
                    .find(|(_, cap)| *cap == capability)
                    .map(|(key, _)| key)
            })
    }
}

/// 読み込み時に解決済みの実行時定義。
///
/// `resolve_definition` により、クリップ種別が宣言するコンポーネント列が
/// 実行リストに展開される。Scheduler はこのリストの順にフックを実行する。
#[derive(Debug, Clone)]
pub struct RuntimeDefinition {
    pub kind_id: NamespacedID,
    pub components: Vec<ComponentDefinition>,
}

impl RuntimeDefinition {
    /// 指定された能力を持つプロパティのスキーマを、構成コンポーネント全体から
    /// 逆引きする。プロパティ ID のハードコードを避けるための解決経路（AGENTS.md §9）。
    pub fn property_by_capability(
        &self,
        capability: &ComponentCapability,
    ) -> Option<&PropertySchema> {
        self.components
            .iter()
            .flat_map(|component| component.properties.iter())
            .find(|property| property.capability.as_ref() == Some(capability))
    }

    /// 構成全体から、指定メンバー能力を持つメンバーキーを逆引きする。
    pub fn member_key_by_capability(&self, capability: &ComponentCapability) -> Option<&String> {
        self.components
            .iter()
            .find_map(|component| component.member_key_by_capability(capability))
    }

    /// プロパティ能力とメンバー能力からプロパティパスを能力ベースで解決する。
    ///
    /// ルートは `property_capability` を持つプロパティのキー、メンバーはその
    /// プロパティの `member_capabilities` で `member_capability` を持つメンバー。
    /// どちらかの宣言が無ければ `None`。
    pub fn member_path(
        &self,
        property_capability: &ComponentCapability,
        member_capability: &ComponentCapability,
    ) -> Option<PropertyPath> {
        let property = self.property_by_capability(property_capability)?;
        let member = property
            .member_capabilities
            .iter()
            .find(|(_, cap)| *cap == member_capability)
            .map(|(key, _)| key)?;
        PropertyPath::from(property.key.clone()).child(member).ok()
    }
}

/// コンポーネントの実行時インスタンス。
///
/// 定義（ComponentDefinition / RuntimeDefinition）は Kind ごとに1つ共有されるが、
/// 状態は Clip Instance ごとに独立する。ネットワーク経由で同期される
/// `crate::project::clip::Clip` とは別物（実行時のみの構造）。
#[derive(Debug, Clone)]
pub struct ClipInstance {
    pub kind_id: NamespacedID,
    pub properties: HashMap<NamespacedID, ClipBindingValue>,
    /// コンポーネントごとの実行時状態。キーはコンポーネント id。
    pub state: HashMap<NamespacedID, FieldValue>,
}

impl ClipInstance {
    /// 指定されたコンポーネント定義からプロパティのデフォルト値を展開して
    /// ClipInstance を作成する。
    pub fn new(kind_id: NamespacedID, components: &[ComponentDefinition]) -> Self {
        let mut schemas = Vec::new();
        for component in components {
            schemas.extend(component.properties.iter().cloned());
        }
        Self {
            kind_id,
            properties: ClipBindingValue::default_properties(&schemas),
            state: HashMap::new(),
        }
    }
}

/// コンポーネント定義の登録・検索・Capability 解決・定義解決を行う。
#[derive(Debug, Default)]
pub struct ComponentRegistry {
    inner: crate::plugin::registry::PluginDefinitionRegistry<NamespacedID, ComponentDefinition>,
}

impl ComponentRegistry {
    pub fn get(&self, id: &NamespacedID) -> Option<&ComponentDefinition> {
        self.inner.get(id)
    }

    pub fn contains(&self, id: &NamespacedID) -> bool {
        self.inner.contains(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&NamespacedID, &ComponentDefinition)> {
        self.inner.iter()
    }

    pub fn len(&self) -> usize {
        self.inner.iter().count()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.iter().count() == 0
    }

    /// 既存定義と id 衝突があればエラーにして合流させる。
    pub fn merge_component_definitions(
        &mut self,
        definitions: HashMap<NamespacedID, ComponentDefinition>,
    ) -> anyhow::Result<()> {
        self.inner.merge(definitions)
    }

    /// 指定 Capability を提供するコンポーネント定義をすべて返す。
    pub fn by_capability(&self, capability: &ComponentCapability) -> Vec<&ComponentDefinition> {
        self.inner
            .iter()
            .filter(|(_, def)| def.capabilities.contains(capability))
            .map(|(_, def)| def)
            .collect()
    }

    /// 指定 Capability を提供するコンポーネントの id をすべて返す。
    pub fn ids_by_capability(&self, capability: &ComponentCapability) -> Vec<NamespacedID> {
        self.inner
            .iter()
            .filter(|(_, def)| def.capabilities.contains(capability))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// クリップ種別が宣言するコンポーネント列を `RuntimeDefinition` に解決する。
    ///
    /// 宣言順に展開し、同じコンポーネントは二度出さない。
    /// 未定義のコンポーネント参照はエラーとする。
    pub fn resolve_definition(
        &self,
        kind_id: &NamespacedID,
        component_ids: &[NamespacedID],
    ) -> anyhow::Result<RuntimeDefinition> {
        let mut components = Vec::new();
        let mut visited = HashSet::new();
        for id in component_ids {
            if !visited.insert(id.clone()) {
                continue;
            }
            let def = self
                .get(id)
                .with_context(|| format!("component definition `{}` not found", id.full()))?;
            components.push(def.clone());
        }
        Ok(RuntimeDefinition {
            kind_id: kind_id.clone(),
            components,
        })
    }
}

/// `plugin:local` 形式、またはプラグイン下のローカル ID を NamespacedID に解決する。
pub(crate) fn resolve_namespaced(value: &str, plugin_id: &str) -> anyhow::Result<NamespacedID> {
    if value.contains(':') {
        NamespacedID::parse(value)
    } else {
        NamespacedID::new(plugin_id, value)
    }
}

/// TOML からパースするための中間表現。
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ComponentDefinitionRaw {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub editor: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub properties: Vec<PropertySchemaRaw>,
}

impl ComponentDefinitionRaw {
    pub fn resolve(self, plugin_id: &str) -> anyhow::Result<ComponentDefinition> {
        let id = NamespacedID::new(plugin_id, &self.id).with_context(|| {
            format!(
                "invalid component id `{}` in plugin `{}`",
                self.id, plugin_id
            )
        })?;
        let kind = resolve_namespaced(&self.kind, plugin_id)
            .with_context(|| format!("invalid component kind `{}`", self.kind))?;
        let mut capabilities = BTreeSet::new();
        for cap in &self.capabilities {
            capabilities.insert(cap.parse().with_context(|| {
                format!("invalid component capability `{cap}` in `{}`", id.full())
            })?);
        }
        let mut properties = Vec::new();
        for raw in &self.properties {
            properties.push(PropertySchema::parse_one(raw.clone(), plugin_id)?);
        }
        Ok(ComponentDefinition {
            id,
            name: self.name,
            kind,
            capabilities,
            editor: self.editor,
            script: self.script,
            properties,
        })
    }
}

/// TOML テキストから `[[components]]` 配列をパースし、プラグインごとの ComponentDefinition マップを返す。
pub fn parse_component_definitions(
    text: &str,
    plugin_id: &str,
) -> anyhow::Result<HashMap<NamespacedID, ComponentDefinition>> {
    #[derive(Debug, Deserialize, Default)]
    struct ComponentsFile {
        #[serde(default)]
        components: Vec<ComponentDefinitionRaw>,
    }

    let parsed: ComponentsFile = toml::from_str(text).map_err(|e| {
        anyhow::anyhow!(
            "failed to parse component definitions TOML in {}: {}",
            plugin_id,
            e
        )
    })?;

    let mut map = HashMap::new();
    for raw in parsed.components {
        let def = raw.resolve(plugin_id)?;
        let id_full = def.id.full().to_string();
        if map.insert(def.id.clone(), def).is_some() {
            anyhow::bail!("duplicate component definition id `{id_full}` in plugin `{plugin_id}`");
        }
    }
    Ok(map)
}

/// コンポーネントの能力を順に実行するスケジューラ。
///
/// ComponentDefinition に登録された能力（frame-provider, prefetchable など）を、
/// プラグインの Rhai スクリプトの標準関数として呼び出す。
/// 各能力は標準関数名（render_frame, prefetch など）とフックタイミングを持ち、
/// フックタイミング順に実行される。
#[derive(Debug, Default)]
pub struct Scheduler;

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// `definition` の各コンポーネントについて、能力に基づく標準関数をフックタイミング順に実行する。
    pub fn run_clip(
        &self,
        engine: &Engine,
        loader: &PluginLoader,
        instance: &ClipInstance,
        definition: &RuntimeDefinition,
        ctx: &mut PluginRenderContext,
    ) -> anyhow::Result<()> {
        for component in &definition.components {
            self.run_component(engine, loader, instance, component, ctx)?;
        }
        Ok(())
    }

    /// FileDropHandler の戻り値を初期プロパティとして適用します。
    pub fn file_drop_properties(
        &self,
        engine: &Engine,
        loader: &PluginLoader,
        kind_id: &NamespacedID,
        source_path: &str,
    ) -> anyhow::Result<HashMap<NamespacedID, ClipBindingValue>> {
        let definition = loader.resolve_component_definition(kind_id)?;
        let handlers: Vec<_> = definition
            .components
            .iter()
            .filter(|component| {
                component
                    .capabilities
                    .contains(&ComponentCapability::FileDropHandler)
            })
            .collect();
        let [handler] = handlers.as_slice() else {
            anyhow::bail!(
                "clip kind `{}` must define exactly one file-drop-handler capability",
                kind_id.full()
            );
        };

        let fn_name = ComponentCapability::FileDropHandler
            .standard_fn()
            .context("file-drop-handler capability has no function mapping")?;
        let script = loader.get_component_script(&handler.id).with_context(|| {
            format!(
                "file-drop-handler component `{}` has no script",
                handler.id.full()
            )
        })?;
        let result = script
            .call::<Dynamic>(engine, fn_name, (source_path.to_owned(),))
            .with_context(|| format!("file-drop-handler `{}` failed", handler.id.full()))?;
        let FieldValue::Map(returned_properties) = dynamic_to_field_value(result)? else {
            anyhow::bail!(
                "file-drop-handler `{}` must return a map of property IDs to values",
                handler.id.full()
            );
        };

        let schemas: HashMap<_, _> = definition
            .components
            .iter()
            .flat_map(|component| component.properties.iter())
            .map(|schema| (schema.key.clone(), schema))
            .collect();
        let mut properties = ClipBindingValue::default_properties(
            &schemas
                .values()
                .map(|schema| (*schema).clone())
                .collect::<Vec<_>>(),
        );
        if returned_properties.is_empty() {
            anyhow::bail!(
                "file-drop-handler `{}` returned no property values",
                handler.id.full()
            );
        }

        for (property_id, value) in returned_properties {
            let property_id = NamespacedID::parse(&property_id).with_context(|| {
                format!("invalid property ID returned by `{}`", handler.id.full())
            })?;
            let schema = schemas.get(&property_id).with_context(|| {
                format!(
                    "file-drop-handler `{}` returned undeclared property `{property_id}`",
                    handler.id.full()
                )
            })?;
            let Some(ClipBindingValue::Static(default_value)) = properties.get_mut(&property_id)
            else {
                anyhow::bail!(
                    "default value for property `{property_id}` is missing or unsupported"
                );
            };
            merge_field_value(default_value, value, &schema.key.to_string())?;
        }

        Ok(properties)
    }

    fn run_component(
        &self,
        engine: &Engine,
        loader: &PluginLoader,
        instance: &ClipInstance,
        component: &ComponentDefinition,
        ctx: &mut PluginRenderContext,
    ) -> anyhow::Result<()> {
        // スクリプトがないコンポーネントはスキップ（GUI 専用定義など）。
        let Some(script) = loader.get_component_script(&component.id) else {
            return Ok(());
        };

        // クリップインスタンスのプロパティ全体を渡す。コンポーネントはクリップ
        // 種別の中で組み合わされる独立部品であり、ある部品のスクリプトが兄弟部品の
        // 宣言したプロパティを参照するのは正規の使い方である（例: media の描画
        // スクリプトが transform コンポーネントの変換プロパティを読む）。
        let props = field_values_to_rhai_map(&instance.properties);
        let props_dynamic = Dynamic::from_map(props);

        // 能力をフックタイミング順にソートし、各標準関数を呼び出す。
        let mut calls: Vec<(ComponentHook, &str)> = Vec::new();
        for cap in &component.capabilities {
            if *cap == ComponentCapability::FileDropHandler {
                continue;
            }
            if let Some(fn_name) = cap.standard_fn() {
                calls.push((cap.hook_timing(), fn_name));
            }
        }
        calls.sort_by_key(|(hook, _)| hook.clone());

        for (_, fn_name) in calls {
            script
                .call::<Dynamic>(engine, fn_name, (ctx.clone(), props_dynamic.clone()))
                .with_context(|| {
                    format!(
                        "capability function `{fn_name}` of component `{}` failed",
                        component.id.full()
                    )
                })?;
        }
        Ok(())
    }
}

fn merge_field_value(
    default: &mut FieldValue,
    returned: FieldValue,
    property_path: &str,
) -> anyhow::Result<()> {
    match (default, returned) {
        (FieldValue::Map(default_values), FieldValue::Map(returned_values)) => {
            for (key, returned_value) in returned_values {
                let default_value = default_values.get_mut(&key).with_context(|| {
                    format!("handler returned undeclared field `{property_path}/{key}`")
                })?;
                merge_field_value(
                    default_value,
                    returned_value,
                    &format!("{property_path}/{key}"),
                )?;
            }
        }
        (FieldValue::Array(default_values), FieldValue::Array(returned_values)) => {
            if let Some(default_item) = default_values.first() {
                for (index, returned_item) in returned_values.iter().cloned().enumerate() {
                    let mut expected_item = default_item.clone();
                    merge_field_value(
                        &mut expected_item,
                        returned_item,
                        &format!("{property_path}/{index}"),
                    )?;
                }
            }
            *default_values = returned_values;
        }
        (FieldValue::Bool(default_value), FieldValue::Bool(returned_value)) => {
            *default_value = returned_value;
        }
        (FieldValue::Int(default_value), FieldValue::Int(returned_value)) => {
            *default_value = returned_value;
        }
        (FieldValue::Float(default_value), FieldValue::Float(returned_value)) => {
            *default_value = returned_value;
        }
        (FieldValue::Enum(default_value), FieldValue::Enum(returned_value))
        | (FieldValue::String(default_value), FieldValue::String(returned_value)) => {
            *default_value = returned_value;
        }
        (FieldValue::Path(default_value), FieldValue::Path(returned_value)) => {
            *default_value = returned_value;
        }
        (FieldValue::Color(default_value), FieldValue::Color(returned_value)) => {
            *default_value = returned_value;
        }
        (default, returned) => anyhow::bail!(
            "handler returned incompatible value for `{property_path}`: expected {}, got {}",
            field_value_kind(default),
            field_value_kind(&returned)
        ),
    }
    Ok(())
}

fn field_value_kind(value: &FieldValue) -> &'static str {
    match value {
        FieldValue::Bool(_) => "bool",
        FieldValue::Int(_) => "int",
        FieldValue::Float(_) => "float",
        FieldValue::Enum(_) => "enum",
        FieldValue::String(_) => "string",
        FieldValue::Path(_) => "path",
        FieldValue::Color(_) => "color",
        FieldValue::Array(_) => "array",
        FieldValue::Map(_) => "map",
    }
}

fn dynamic_to_field_value(value: Dynamic) -> anyhow::Result<FieldValue> {
    if let Some(value) = value.clone().try_cast::<bool>() {
        return Ok(FieldValue::Bool(value));
    }
    if let Some(value) = value.clone().try_cast::<i64>() {
        return Ok(FieldValue::Int(value));
    }
    if let Some(value) = value.clone().try_cast::<f64>() {
        return Ok(FieldValue::Float(value));
    }
    if let Some(value) = value.clone().try_cast::<String>() {
        return Ok(FieldValue::String(value));
    }
    if let Some(values) = value.clone().try_cast::<rhai::Array>() {
        return values
            .into_iter()
            .map(dynamic_to_field_value)
            .collect::<anyhow::Result<Vec<_>>>()
            .map(FieldValue::Array);
    }
    if let Some(values) = value.try_cast::<rhai::Map>() {
        return values
            .into_iter()
            .map(|(key, value)| Ok((key.to_string(), dynamic_to_field_value(value)?)))
            .collect::<anyhow::Result<std::collections::BTreeMap<_, _>>>()
            .map(FieldValue::Map);
    }
    anyhow::bail!("file-drop-handler returned an unsupported value type")
}

#[cfg(test)]
#[path = "../../../tests/unit/plugin/clip/component_tests.rs"]
mod tests;
