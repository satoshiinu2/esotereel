use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Context, bail};
use rhai::{Dynamic, Engine};
use serde::{Deserialize, Serialize};

pub use crate::plugin::property::{
    ComponentCapability, PropertyKey, PropertySchema, parse::PropertySchemaRaw, value::FieldValue,
};
use crate::plugin::property::path::PropertyPath;
use crate::plugin::script::api::PluginRenderContext;
use crate::plugin::script::props::ComponentProps;
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
/// ネイティブ側では ID による種別判定より `capabilities` を通じた
/// 能力ベースのアクセスを原則とする。
/// コンポーネントは独立した部品であり、どのクリップ種別がどのコンポーネントで
/// 構成されるかはクリップ種別（`ClipKind`）側が宣言する。定義は Kind ごとに1つ、
/// 状態（Runtime State）は Clip Instance ごとに独立する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentDefinition {
    pub id: NamespacedID,
    pub name: String,
    pub capabilities: BTreeSet<ComponentCapability>,
    pub editor: Option<String>,
    /// このコンポーネント専用の Rhai スクリプトファイルパス（プラグインディレクトリからの相対パス）。
    /// プラグイン共通の script.rhai ではなく、コンポーネントごとに個別のスクリプトを持つ。
    #[serde(default)]
    pub script: Option<String>,
    /// このコンポーネントが提供するプロパティスキーマの一覧。
    /// コンポーネントが自身のプロパティを宣言し、ClipInstance は各コンポーネントの
    /// プロパティを保持する。スケジューラがスクリプトへ渡す props は
    /// サンドボックス化されており、自コンポーネントのプロパティ（ローカル名）と、
    /// `[[requires]]` で宣言した依存先の capability に紐づくプロパティだけが解決できる。
    #[serde(default)]
    pub properties: Vec<PropertySchema>,
    /// 他コンポーネントへの依存宣言（`[[requires]]`）。
    /// スクリプトへ渡る props では、自プロパティに加えて、ここで宣言した依存先の
    /// capability に紐づくプロパティだけが `name` で参照できる。
    #[serde(default)]
    pub requires: Vec<ComponentRequire>,
}

/// コンポーネントが他のコンポーネントへ宣言する依存。
///
/// 依存先はコンポーネント ID ではなく capability で解決する（AGENTS.md §9）。
/// デフォルトは任意依存で、満たせない場合は props に現れない。
/// `required = true` の依存が kind の構成で満たせなければ定義解決時にエラーになる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentRequire {
    /// 依存先の値を props に混ぜるときのローカル名。
    pub name: String,
    pub capability: ComponentCapability,
    #[serde(default)]
    pub required: bool,
}

impl ComponentDefinition {
    /// `FrameProvider` 能力を持つプロパティのメンバーのうち、指定能力を
    /// 宣言するメンバーのキーを逆引きする。
    ///
    /// ハードコードされたメンバキー（`"source_offset"` 等）に依存せず、
    /// メンバー自身が宣言する能力から参照する（AGENTS.md 第9条）。
    pub fn member_key_by_capability(&self, capability: &ComponentCapability) -> Option<&str> {
        self.properties
            .iter()
            .find(|p| p.capability == Some(ComponentCapability::FrameProvider))
            .and_then(|p| p.member_key_by_capability(capability))
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
    pub fn member_key_by_capability(&self, capability: &ComponentCapability) -> Option<&str> {
        self.components
            .iter()
            .find_map(|component| component.member_key_by_capability(capability))
    }

    /// プロパティ能力とメンバー能力からプロパティパスを能力ベースで解決する。
    ///
    /// ルートは `property_capability` を持つプロパティのキー、メンバーはその
    /// プロパティのメンバーのうち `member_capability` を宣言するメンバー。
    /// どちらかの宣言が無ければ `None`。
    pub fn member_path(
        &self,
        property_capability: &ComponentCapability,
        member_capability: &ComponentCapability,
    ) -> Option<PropertyPath> {
        let property = self.property_by_capability(property_capability)?;
        let member = property.member_key_by_capability(member_capability)?;
        PropertyPath::from(property.key.clone()).child(member).ok()
    }

    /// `[[requires]]` の依存先を capability で解決する単一の規則。
    ///
    /// 構成コンポーネントを宣言順に（依存元自身を除いて）探し、最初に一致したものを
    /// 返す。Scheduler の props 構築と、必須依存の検証（`resolve_definition`）は
    /// この規則を共有する。満たせない場合は `None`。
    pub fn resolve_requirement<'a>(
        &'a self,
        owner: &ComponentDefinition,
        require: &ComponentRequire,
    ) -> Option<RequirementResolution<'a>> {
        for component in &self.components {
            if component.id == owner.id {
                continue;
            }
            for schema in &component.properties {
                if schema.capability.as_ref() == Some(&require.capability) {
                    return Some(RequirementResolution::Property { component, schema });
                }
                if let Some(member) = schema.member_key_by_capability(&require.capability) {
                    return Some(RequirementResolution::Member {
                        component,
                        schema,
                        member,
                    });
                }
            }
            if component.capabilities.contains(&require.capability) {
                return Some(RequirementResolution::DeclaredOnly { component });
            }
        }
        None
    }
}

/// `[[requires]]` の解決結果。依存先コンポーネントと、capability に紐づく対象を示す。
pub enum RequirementResolution<'a> {
    /// プロパティ自体が capability を宣言している。値全体が依存先として渡る。
    Property {
        component: &'a ComponentDefinition,
        schema: &'a PropertySchema,
    },
    /// グループプロパティのメンバーが capability を宣言している。
    /// そのメンバーの値だけが依存先として渡る。
    Member {
        component: &'a ComponentDefinition,
        schema: &'a PropertySchema,
        member: &'a str,
    },
    /// コンポーネント自体が capability を持つ。渡す値はないが必須依存は満たす。
    DeclaredOnly { component: &'a ComponentDefinition },
}

/// コンポーネントの実行時インスタンス。
///
/// 定義（ComponentDefinition / RuntimeDefinition）は Kind ごとに1つ共有されるが、
/// 状態は Clip Instance ごとに独立する。ネットワーク経由で同期される
/// `crate::project::clip::Clip` とは別物（実行時のみの構造）。
#[derive(Debug, Clone)]
pub struct ClipInstance {
    pub kind_id: NamespacedID,
    pub properties: HashMap<PropertyKey, ClipBindingValue>,
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
        let definition = RuntimeDefinition {
            kind_id: kind_id.clone(),
            components,
        };
        for component in &definition.components {
            for require in &component.requires {
                if require.required
                    && definition.resolve_requirement(component, require).is_none()
                {
                    bail!(
                        "component `{}` requires capability `{:?}` as `{}` but no component of kind `{}` provides it",
                        component.id.full(),
                        require.capability,
                        require.name,
                        kind_id.full()
                    );
                }
            }
        }
        Ok(definition)
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
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub editor: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub properties: Vec<PropertySchemaRaw>,
    #[serde(default)]
    pub requires: Vec<ComponentRequireRaw>,
}

/// TOML の `[[requires]]` の中間表現。
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ComponentRequireRaw {
    pub name: String,
    pub capability: String,
    #[serde(default)]
    pub required: bool,
}

impl ComponentDefinitionRaw {
    pub fn resolve(self, plugin_id: &str) -> anyhow::Result<ComponentDefinition> {
        let id = NamespacedID::new(plugin_id, &self.id).with_context(|| {
            format!(
                "invalid component id `{}` in plugin `{}`",
                self.id, plugin_id
            )
        })?;
        let mut capabilities = BTreeSet::new();
        for cap in &self.capabilities {
            capabilities.insert(cap.parse().with_context(|| {
                format!("invalid component capability `{cap}` in `{}`", id.full())
            })?);
        }
        let mut properties = Vec::new();
        for raw in &self.properties {
            properties.push(PropertySchema::parse_for_component(raw.clone(), &id)?);
        }
        let mut requires = Vec::new();
        let mut require_names = HashSet::new();
        for raw in &self.requires {
            crate::plugin::property::validate_local_name(&raw.name).with_context(|| {
                format!("invalid require name `{}` in `{}`", raw.name, id.full())
            })?;
            if properties.iter().any(|p| p.key.name() == raw.name) {
                bail!(
                    "require name `{}` collides with a property of `{}`",
                    raw.name,
                    id.full()
                );
            }
            if !require_names.insert(raw.name.clone()) {
                bail!("duplicate require name `{}` in `{}`", raw.name, id.full());
            }
            let capability = raw.capability.parse().with_context(|| {
                format!(
                    "invalid require capability `{}` in `{}`",
                    raw.capability,
                    id.full()
                )
            })?;
            requires.push(ComponentRequire {
                name: raw.name.clone(),
                capability,
                required: raw.required,
            });
        }
        Ok(ComponentDefinition {
            id,
            name: self.name,
            capabilities,
            editor: self.editor,
            script: self.script,
            properties,
            requires,
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
            self.run_component(engine, loader, instance, definition, component, ctx)?;
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
    ) -> anyhow::Result<HashMap<PropertyKey, ClipBindingValue>> {
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
            // 戻り値のキーはプロパティアドレス。ハンドラ自身のプロパティは
            // 所有者を省略した相対形（`:#name`）でも書ける。
            let property_id = PropertyKey::parse_in_context(&property_id, &handler.id)
                .with_context(|| {
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
        definition: &RuntimeDefinition,
        component: &ComponentDefinition,
        ctx: &mut PluginRenderContext,
    ) -> anyhow::Result<()> {
        // スクリプトがないコンポーネントはスキップ（GUI 専用定義など）。
        let Some(script) = loader.get_component_script(&component.id) else {
            return Ok(());
        };

        // props はサンドボックス化する: 自プロパティと [[requires]] 宣言分の
        // 依存だけが解決できる（例: video の描画スクリプトは requires 経由で
        // transform の変換プロパティを読む）。
        let props = ComponentProps::build(definition, instance, component)?;

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
                .call::<Dynamic>(engine, fn_name, (ctx.clone(), props.clone()))
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
mod tests {
    use super::*;
    use crate::plugin::property::value::FieldTypeKind;

    fn make_def(id_local: &str) -> ComponentDefinition {
        ComponentDefinition {
            id: NamespacedID::new("std", id_local).unwrap(),
            name: id_local.to_string(),
            capabilities: BTreeSet::from([ComponentCapability::Transformable]),
            editor: Some("transform".to_string()),
            script: None,
            requires: Vec::new(),
            properties: Vec::new(),
        }
    }

    #[test]
    fn registry_insert_and_lookup() {
        let mut registry = ComponentRegistry::default();
        let def = make_def("transform");
        let id = def.id.clone();
        registry
            .merge_component_definitions(HashMap::from([(id.clone(), def)]))
            .unwrap();
        assert!(registry.contains(&id));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn by_capability_filter() {
        let mut registry = ComponentRegistry::default();
        let transform = make_def("transform");
        let transform_id = transform.id.clone();
        let media = ComponentDefinition {
            id: NamespacedID::new("std", "video").unwrap(),
            name: "Video".to_string(),
            capabilities: BTreeSet::from([
                ComponentCapability::FrameProvider,
                ComponentCapability::Prefetchable,
            ]),
            editor: Some("video".to_string()),
            script: None,
            requires: Vec::new(),
            properties: Vec::new(),
        };
        let media_id = media.id.clone();
        registry
            .merge_component_definitions(HashMap::from([
                (transform_id.clone(), transform),
                (media_id.clone(), media),
            ]))
            .unwrap();

        let providers: Vec<_> = registry
            .by_capability(&ComponentCapability::FrameProvider)
            .iter()
            .map(|d| d.id.clone())
            .collect();
        assert_eq!(providers, vec![media_id]);

        let ids: Vec<_> = registry
            .ids_by_capability(&ComponentCapability::Transformable)
            .iter()
            .cloned()
            .collect();
        assert_eq!(ids, vec![transform_id]);
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let mut registry = ComponentRegistry::default();
        let def = make_def("transform");
        let id = def.id.clone();
        registry
            .merge_component_definitions(HashMap::from([(id.clone(), def)]))
            .unwrap();
        assert!(
            registry
                .merge_component_definitions(HashMap::from([(id.clone(), make_def("transform"))]))
                .is_err()
        );
    }

    #[test]
    fn parse_component_definitions_from_toml() {
        let text = r#"
[[components]]
id = "media"
name = "Media"
capabilities = ["frame-provider", "prefetchable"]
editor = "video"

[[components]]
id = "transform"
name = "Transform"
capabilities = ["transformable"]
editor = "transform"
"#;
        let map = parse_component_definitions(text, "std").unwrap();
        assert_eq!(map.len(), 2);

        let media = map
            .get(&NamespacedID::new("std", "media").unwrap())
            .unwrap();
        assert!(
            media
                .capabilities
                .contains(&ComponentCapability::FrameProvider)
        );
        assert!(
            media
                .capabilities
                .contains(&ComponentCapability::Prefetchable)
        );
        assert_eq!(
            media
                .capabilities
                .iter()
                .find(|c| matches!(c, ComponentCapability::FrameProvider))
                .map(|_| "render_frame"),
            Some("render_frame")
        );
        assert_eq!(media.editor.as_deref(), Some("video"));

        let transform = map
            .get(&NamespacedID::new("std", "transform").unwrap())
            .unwrap();
        assert!(
            transform
                .capabilities
                .contains(&ComponentCapability::Transformable)
        );
    }

    #[test]
    fn parse_component_definitions_with_properties() {
        let text = r#"
[[components]]
id = "video"
name = "Video"
capabilities = ["frame-provider"]
properties = [
  { key = "source", label = "Source", kind = { type = "string" } },
  { key = "volume", label = "Volume", kind = { type = "float", min = 0.0, max = 1.0, step = 0.1 }, default = 1.0 },
]
"#;
        let map = parse_component_definitions(text, "std").unwrap();
        let video = map
            .get(&NamespacedID::new("std", "video").unwrap())
            .unwrap();
        assert_eq!(video.properties.len(), 2);

        let source = video
            .properties
            .iter()
            .find(|p| p.key.address() == "std:video#source")
            .unwrap();
        assert_eq!(source.label, "Source");
        assert!(matches!(source.kind, FieldTypeKind::String));
        assert_eq!(source.default, FieldValue::String(String::new()));

        let volume = video
            .properties
            .iter()
            .find(|p| p.key.address() == "std:video#volume")
            .unwrap();
        assert_eq!(volume.label, "Volume");
        assert_eq!(volume.default, FieldValue::Float(1.0));
    }

    #[test]
    fn unknown_capability_is_rejected() {
        let text = r#"
[[components]]
id = "custom"
name = "Custom"
capabilities = ["my-custom-cap"]
"#;
        assert!(parse_component_definitions(text, "std").is_err());
    }

    #[test]
    fn capability_as_str_is_lowercase() {
        assert_eq!(
            ComponentCapability::FrameProvider.as_str(),
            "frame-provider"
        );
        assert_eq!(ComponentCapability::Prefetchable.as_str(), "prefetchable");
    }

    #[test]
    fn capability_standard_fn_mappings() {
        assert_eq!(
            ComponentCapability::FrameProvider.standard_fn(),
            Some("render_frame")
        );
        assert_eq!(
            ComponentCapability::Prefetchable.standard_fn(),
            Some("prefetch")
        );
        assert_eq!(
            ComponentCapability::Transformable.standard_fn(),
            Some("get_transform")
        );
        assert_eq!(
            ComponentCapability::FileDropHandler.standard_fn(),
            Some("handle_file_drop")
        );
    }

    #[test]
    fn file_drop_values_merge_into_defaults_and_reject_unknown_fields() {
        let mut defaults = FieldValue::Map(std::collections::BTreeMap::from([
            ("source".to_string(), FieldValue::String(String::new())),
            (
                "source_offset".to_string(),
                FieldValue::String("0".to_string()),
            ),
        ]));
        let returned = FieldValue::Map(std::collections::BTreeMap::from([(
            "source".to_string(),
            FieldValue::String("/media/clip.mp4".to_string()),
        )]));

        merge_field_value(&mut defaults, returned, "std:media#media").unwrap();

        assert_eq!(
            defaults,
            FieldValue::Map(std::collections::BTreeMap::from([
                (
                    "source".to_string(),
                    FieldValue::String("/media/clip.mp4".to_string()),
                ),
                (
                    "source_offset".to_string(),
                    FieldValue::String("0".to_string())
                ),
            ]))
        );

        let unknown_field = FieldValue::Map(std::collections::BTreeMap::from([(
            "missing".to_string(),
            FieldValue::String("value".to_string()),
        )]));
        assert!(
            merge_field_value(&mut defaults, unknown_field, "std:media#media")
                .unwrap_err()
                .to_string()
                .contains("undeclared field")
        );
    }

    #[test]
    fn file_drop_handler_returns_property_map() {
        let engine = Engine::new();
        let ast = engine
            .compile(
                r#"
                    fn handle_file_drop(source_path) {
                        #{
                            "std:media#media": #{
                                source: source_path,
                            },
                        }
                    }
                "#,
            )
            .unwrap();
        let script = crate::plugin::script::CompiledScript {
            ast,
            available_functions: vec!["handle_file_drop".to_string()],
        };

        let result = script
            .call::<Dynamic>(
                &engine,
                "handle_file_drop",
                ("/media/clip.mp4".to_string(),),
            )
            .unwrap();
        let FieldValue::Map(properties) = dynamic_to_field_value(result).unwrap() else {
            panic!("handler should return a property map");
        };
        let FieldValue::Map(media) = properties.get("std:media#media").unwrap() else {
            panic!("media property should be a map");
        };
        assert_eq!(
            media.get("source"),
            Some(&FieldValue::String("/media/clip.mp4".to_string()))
        );
    }

    #[test]
    fn resolve_definition_expands_kind_components_in_order() {
        let mut registry = ComponentRegistry::default();
        let media = ComponentDefinition {
            id: NamespacedID::new("std", "media").unwrap(),
            name: "Media".to_string(),
            capabilities: BTreeSet::from([ComponentCapability::FrameProvider]),
            editor: None,
            script: None,
            requires: Vec::new(),
            properties: Vec::new(),
        };
        let transform = ComponentDefinition {
            id: NamespacedID::new("std", "transform").unwrap(),
            name: "Transform".to_string(),
            capabilities: BTreeSet::from([ComponentCapability::Transformable]),
            editor: None,
            script: None,
            requires: Vec::new(),
            properties: Vec::new(),
        };
        registry
            .merge_component_definitions(HashMap::from([
                (media.id.clone(), media),
                (transform.id.clone(), transform),
            ]))
            .unwrap();

        let kind_id = NamespacedID::new("std", "video").unwrap();
        let def = registry
            .resolve_definition(
                &kind_id,
                &[
                    NamespacedID::new("std", "media").unwrap(),
                    NamespacedID::new("std", "transform").unwrap(),
                    // 重複した宣言は1回だけ現れる。
                    NamespacedID::new("std", "media").unwrap(),
                ],
            )
            .unwrap();
        assert_eq!(def.kind_id, kind_id);
        let ids: Vec<_> = def.components.iter().map(|c| c.id.clone()).collect();
        assert_eq!(
            ids,
            vec![
                NamespacedID::new("std", "media").unwrap(),
                NamespacedID::new("std", "transform").unwrap(),
            ]
        );
    }

    #[test]
    fn resolve_definition_rejects_missing_component() {
        let registry = ComponentRegistry::default();
        assert!(
            registry
                .resolve_definition(
                    &NamespacedID::new("std", "video").unwrap(),
                    &[NamespacedID::new("std", "missing").unwrap()],
                )
                .is_err()
        );
    }

    #[test]
    fn clip_instance_holds_properties_and_state() {
        let instance = ClipInstance::new(NamespacedID::new("std", "video").unwrap(), &[]);
        assert_eq!(instance.kind_id.full(), "std:video");
        assert!(instance.properties.is_empty());
        assert!(instance.state.is_empty());
    }

    #[test]
    fn clip_instance_new_populates_component_property_defaults() {
        let video_def = ComponentDefinition {
            id: NamespacedID::new("std", "video").unwrap(),
            name: "Video".to_string(),
            capabilities: BTreeSet::from([ComponentCapability::FrameProvider]),
            editor: None,
            script: None,
            requires: Vec::new(),
            properties: vec![
                PropertySchema {
                    key: PropertyKey::from_namespaced(NamespacedID::new("std", "source").unwrap()),
                    category: vec!["Media".to_string()],
                    label: "Source".to_string(),
                    kind: FieldTypeKind::String,
                    default: FieldValue::String("default.mp4".to_string()),
                    capability: None,
                },
                PropertySchema {
                    key: PropertyKey::from_namespaced(NamespacedID::new("std", "volume").unwrap()),
                    category: vec![],
                    label: "Volume".to_string(),
                    kind: FieldTypeKind::Float {
                        min: Some(0.0),
                        max: Some(1.0),
                        step: 0.1,
                    },
                    default: FieldValue::Float(1.0),
                    capability: None,
                },
            ],
        };

        let instance = ClipInstance::new(NamespacedID::new("std", "video").unwrap(), &[video_def]);
        assert_eq!(instance.properties.len(), 2);

        assert!(matches!(
            instance.properties.get(&PropertyKey::from_namespaced(
                NamespacedID::new("std", "source").unwrap(),
            )),
            Some(ClipBindingValue::Static(FieldValue::String(v))) if v == "default.mp4"
        ));
        assert!(matches!(
            instance.properties.get(&PropertyKey::from_namespaced(
                NamespacedID::new("std", "volume").unwrap(),
            )),
            Some(ClipBindingValue::Static(FieldValue::Float(v))) if *v == 1.0
        ));
        assert!(instance.state.is_empty());
    }

    #[test]
    fn clip_instance_new_merges_multiple_components_properties() {
        let a = ComponentDefinition {
            id: NamespacedID::new("std", "a").unwrap(),
            name: "A".to_string(),
            capabilities: BTreeSet::new(),
            editor: None,
            script: None,
            requires: Vec::new(),
            properties: vec![PropertySchema {
                key: PropertyKey::from_namespaced(NamespacedID::new("std", "prop_a").unwrap()),
                category: vec![],
                label: "Prop A".to_string(),
                kind: FieldTypeKind::String,
                default: FieldValue::String("a".to_string()),
                capability: None,
            }],
        };
        let b = ComponentDefinition {
            id: NamespacedID::new("std", "b").unwrap(),
            name: "B".to_string(),
            capabilities: BTreeSet::new(),
            editor: None,
            script: None,
            requires: Vec::new(),
            properties: vec![PropertySchema {
                key: PropertyKey::from_namespaced(NamespacedID::new("std", "prop_b").unwrap()),
                category: vec![],
                label: "Prop B".to_string(),
                kind: FieldTypeKind::Int {
                    min: None,
                    max: None,
                },
                default: FieldValue::Int(42),
                capability: None,
            }],
        };

        let instance = ClipInstance::new(NamespacedID::new("std", "clip").unwrap(), &[a, b]);
        assert_eq!(instance.properties.len(), 2);

        assert!(matches!(
            instance.properties.get(&PropertyKey::from_namespaced(
                NamespacedID::new("std", "prop_a").unwrap(),
            )),
            Some(ClipBindingValue::Static(FieldValue::String(v))) if v == "a"
        ));
        assert!(matches!(
            instance.properties.get(&PropertyKey::from_namespaced(
                NamespacedID::new("std", "prop_b").unwrap(),
            )),
            Some(ClipBindingValue::Static(FieldValue::Int(v))) if *v == 42
        ));
    }

    #[test]
    fn parse_component_definitions_properties_default_empty() {
        let text = r#"
[[components]]
id = "video"
name = "Video"
capabilities = ["frame-provider"]
"#;
        let map = parse_component_definitions(text, "std").unwrap();
        let video = map
            .get(&NamespacedID::new("std", "video").unwrap())
            .unwrap();
        assert!(video.properties.is_empty());
    }

    #[test]
    fn hook_execution_order_is_capability_timing_order() {
        // ComponentHook の Ord に従い、Prefetch < Evaluate < Process の順で実行される。
        let mut hooks = Vec::new();
        hooks.push((ComponentHook::Finalize, "finalize"));
        hooks.push((ComponentHook::Prepare, "prepare"));
        hooks.push((ComponentHook::Evaluate, "evaluate"));
        hooks.sort_by_key(|(hook, _)| hook.clone());
        let order: Vec<_> = hooks.iter().map(|(h, _)| h.clone()).collect();
        assert_eq!(
            order,
            vec![
                ComponentHook::Prepare,
                ComponentHook::Evaluate,
                ComponentHook::Finalize,
            ]
        );
    }

    #[test]
    fn scheduler_runs_rhai_capability_function_via_loaded_plugin() {
        use std::{env, fs};

        let dir = env::temp_dir().join("esotereel_scheduler_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("components")).unwrap();

        fs::write(
            dir.join("manifest.toml"),
            r#"id = "sched-test"
name = "Scheduler Test"
version = "0.1.0"
"#,
        )
        .unwrap();
        fs::write(
            dir.join("components").join("video.rhai"),
            "fn render_frame(ctx, props) { return \"called\"; }\n",
        )
        .unwrap();
        fs::write(
            dir.join("components").join("video.toml"),
            r#"[[components]]
id = "video"
name = "Video"
capabilities = ["frame-provider"]
script = "video.rhai"
"#,
        )
        .unwrap();

        let mut loader = PluginLoader::new();
        loader.load_additional_plugin(&dir).unwrap();
        assert!(
            loader
                .get_component_script(&NamespacedID::new("sched-test", "video").unwrap())
                .is_some()
        );

        let engine = Engine::new();
        let instance = ClipInstance {
            kind_id: NamespacedID::new("sched-test", "video").unwrap(),
            properties: HashMap::new(),
            state: HashMap::new(),
        };
        let def = RuntimeDefinition {
            kind_id: NamespacedID::new("sched-test", "video").unwrap(),
            components: vec![ComponentDefinition {
                id: NamespacedID::new("sched-test", "video").unwrap(),
                name: "Video".to_string(),
                capabilities: BTreeSet::from([ComponentCapability::FrameProvider]),
                editor: None,
                script: Some("video.rhai".to_string()),
                requires: Vec::new(),
                properties: Vec::new(),
            }],
        };

        use crate::render::video::MediaFetchCache;
        use dashmap::DashMap;
        use glam::Mat4;
        use std::sync::Arc;
        let mut ctx = PluginRenderContext::new(
            0,
            0.0,
            Mat4::IDENTITY,
            Arc::new(DashMap::new()),
            Arc::new(DashMap::new()),
            Arc::new(MediaFetchCache::default()),
        );
        let scheduler = Scheduler::new();
        let result = scheduler.run_clip(&engine, &loader, &instance, &def, &mut ctx);
        assert!(
            result.is_ok(),
            "scheduler should run the Rhai capability function: {result:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
