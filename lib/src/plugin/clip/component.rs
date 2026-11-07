use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Context;
use rhai::{Dynamic, Engine};
use serde::{Deserialize, Serialize};

use crate::plugin::{NamespacedID, PluginLoader};
pub use crate::plugin::property::{
    parse::PropertySchemaRaw, value::FieldValue, ComponentCapability, PropertySchema,
};
use crate::plugin::script::api::PluginRenderContext;
use crate::plugin::script::bridge::field_values_to_rhai_map;
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
/// `children` で他コンポーネントを参照でき、読み込み時に `RuntimeDefinition` へ
/// フラット化される。定義は Kind ごとに1つ、状態（Runtime State）は Clip Instance ごとに独立する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentDefinition {
    pub id: NamespacedID,
    pub name: String,
    /// このコンポーネントの種別を参照する NamespacedID。
    /// Capability 経由でのアクセスが基本のため、ここでは単なる参照として保持する。
    pub kind: NamespacedID,
    pub capabilities: HashSet<ComponentCapability>,
    pub editor: Option<String>,
    /// このコンポーネント専用の Rhai スクリプトファイルパス（プラグインディレクトリからの相対パス）。
    /// プラグイン共通の script.rhai ではなく、コンポーネントごとに個別のスクリプトを持つ。
    #[serde(default)]
    pub script: Option<String>,
    /// このコンポーネントが提供するプロパティスキーマの一覧。
    /// コンポーネントが自身のプロパティを宣言し、ClipInstance は各コンポーネントの
    /// プロパティを保持・渡す。スケジューラはフック実行時に当該コンポーネントの
    /// プロパティだけをスクリプトに渡す。
    #[serde(default)]
    pub properties: Vec<PropertySchema>,
    /// このコンポーネントが内部で使う子コンポーネントの id。
    #[serde(default)]
    pub children: Vec<NamespacedID>,
}

/// 読み込み時にフラット化・解決済みの実行時定義。
///
/// `resolve_definition` により階層構造の ComponentDefinition が平坦な実行リスト
/// に展開される。Scheduler はこのリストの順にフックを実行する。
#[derive(Debug, Clone)]
pub struct RuntimeDefinition {
    pub root_id: NamespacedID,
    pub components: Vec<ComponentDefinition>,
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

    /// 階層構造の ComponentDefinition をルートから辿ってフラットな `RuntimeDefinition` に解決する。
    ///
    /// 親 → 子の順（深さ優先）で展開し、同じコンポーネントは二度出さない。
    /// 循環参照や未定義の子参照はエラーとする。
    pub fn resolve_definition(&self, root_id: &NamespacedID) -> anyhow::Result<RuntimeDefinition> {
        let mut components = Vec::new();
        let mut visited = HashSet::new();
        let mut path = HashSet::new();
        self.resolve_definition_inner(root_id, &mut components, &mut visited, &mut path)?;
        Ok(RuntimeDefinition {
            root_id: root_id.clone(),
            components,
        })
    }

    fn resolve_definition_inner(
        &self,
        id: &NamespacedID,
        components: &mut Vec<ComponentDefinition>,
        visited: &mut HashSet<NamespacedID>,
        path: &mut HashSet<NamespacedID>,
    ) -> anyhow::Result<()> {
        if path.contains(id) {
            anyhow::bail!("cyclic component reference detected at `{}`", id.full());
        }
        if visited.contains(id) {
            return Ok(()); // 他の枝ですでに展開済み
        }

        path.insert(id.clone());
        let def = self
            .get(id)
            .with_context(|| format!("component definition `{}` not found", id.full()))?;
        components.push(def.clone());
        for child in &def.children {
            self.resolve_definition_inner(child, components, visited, path)?;
        }
        path.remove(id);
        visited.insert(id.clone());
        Ok(())
    }
}

/// `plugin:local` 形式、またはプラグイン下のローカル ID を NamespacedID に解決する。
fn resolve_namespaced(value: &str, plugin_id: &str) -> anyhow::Result<NamespacedID> {
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
    pub children: Vec<String>,
    #[serde(default)]
    pub editor: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub properties: Vec<PropertySchemaRaw>,
}

impl ComponentDefinitionRaw {
    pub fn resolve(self, plugin_id: &str) -> anyhow::Result<ComponentDefinition> {
        let id = NamespacedID::new(plugin_id, &self.id)
            .with_context(|| format!("invalid component id `{}` in plugin `{}`", self.id, plugin_id))?;
        let kind = resolve_namespaced(&self.kind, plugin_id)
            .with_context(|| format!("invalid component kind `{}`", self.kind))?;
        let mut capabilities = HashSet::new();
        for cap in &self.capabilities {
            capabilities.insert(cap.parse().with_context(|| {
                format!("invalid component capability `{cap}` in `{}`", id.full())
            })?);
        }
        let mut children = Vec::new();
        for child in &self.children {
            children.push(resolve_namespaced(child, plugin_id).with_context(|| {
                format!("invalid component child id `{child}` in `{}`", id.full())
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
            children,
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
        anyhow::anyhow!("failed to parse component definitions TOML in {}: {}", plugin_id, e)
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

        // コンポーネントが宣言したプロパティだけを抽出して渡す（カプセル化）。
        let component_prop_keys: HashSet<NamespacedID> = component
            .properties
            .iter()
            .map(|p| p.key.clone())
            .collect();
        let component_props: HashMap<NamespacedID, ClipBindingValue> = instance
            .properties
            .iter()
            .filter(|(k, _)| component_prop_keys.contains(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let props = field_values_to_rhai_map(&component_props);
        let props_dynamic = Dynamic::from_map(props);

        // 能力をフックタイミング順にソートし、各標準関数を呼び出す。
        let mut calls: Vec<(ComponentHook, &str)> = Vec::new();
        for cap in &component.capabilities {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::property::value::FieldTypeKind;

    fn make_def(id_local: &str) -> ComponentDefinition {
        ComponentDefinition {
            id: NamespacedID::new("std", id_local).unwrap(),
            name: id_local.to_string(),
            kind: NamespacedID::new("std", "transform").unwrap(),
            capabilities: HashSet::from([ComponentCapability::Transformable]),
            editor: Some("transform".to_string()),
            script: None,
            properties: Vec::new(),
            children: Vec::new(),
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
        assert_eq!(registry.get(&id).unwrap().kind.full(), "std:transform");
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
            kind: NamespacedID::new("std", "media-provider").unwrap(),
            capabilities: HashSet::from([
                ComponentCapability::FrameProvider,
                ComponentCapability::Prefetchable,
            ]),
            editor: Some("video".to_string()),
            script: None,
            properties: Vec::new(),
            children: Vec::new(),
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
        assert!(registry
            .merge_component_definitions(HashMap::from([(id.clone(), make_def("transform"))]))
            .is_err());
    }

    #[test]
    fn parse_component_definitions_from_toml() {
        let text = r#"
[[components]]
id = "video"
name = "Video"
kind = "media-provider"
capabilities = ["frame-provider", "prefetchable"]
editor = "video"
children = ["video-transform"]

[[components]]
id = "video-transform"
name = "Transform"
kind = "transform"
capabilities = ["transformable"]
editor = "transform"
"#;
        let map = parse_component_definitions(text, "std").unwrap();
        assert_eq!(map.len(), 2);

        let video = map
            .get(&NamespacedID::new("std", "video").unwrap())
            .unwrap();
        assert_eq!(video.kind.full(), "std:media-provider");
        assert!(video.capabilities.contains(&ComponentCapability::FrameProvider));
        assert!(video.capabilities.contains(&ComponentCapability::Prefetchable));
        assert_eq!(
            video.capabilities.iter().find(|c| matches!(c, ComponentCapability::FrameProvider)).map(|_| "render_frame"),
            Some("render_frame")
        );
        assert_eq!(video.editor.as_deref(), Some("video"));
        assert_eq!(
            video.children,
            vec![NamespacedID::new("std", "video-transform").unwrap()]
        );

        let transform = map
            .get(&NamespacedID::new("std", "video-transform").unwrap())
            .unwrap();
        assert_eq!(transform.kind.full(), "std:transform");
        assert!(transform.capabilities.contains(&ComponentCapability::Transformable));
    }

    #[test]
    fn parse_component_definitions_with_properties() {
        let text = r#"
[[components]]
id = "video"
name = "Video"
kind = "media-provider"
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

        let source = video.properties.iter().find(|p| p.key.full() == "std:source").unwrap();
        assert_eq!(source.label, "Source");
        assert!(matches!(source.kind, FieldTypeKind::String));
        assert_eq!(source.default, FieldValue::String(String::new()));

        let volume = video.properties.iter().find(|p| p.key.full() == "std:volume").unwrap();
        assert_eq!(volume.label, "Volume");
        assert_eq!(volume.default, FieldValue::Float(1.0));
    }

    #[test]
    fn custom_capability_is_preserved_lowercase() {
        let text = r#"
[[components]]
id = "custom"
name = "Custom"
kind = "custom-kind"
capabilities = ["my-custom-cap"]
"#;
        let map = parse_component_definitions(text, "std").unwrap();
        let def = map
            .get(&NamespacedID::new("std", "custom").unwrap())
            .unwrap();
        assert_eq!(def.kind.full(), "std:custom-kind");
        assert!(def.capabilities.contains(&ComponentCapability::Custom(
            "my-custom-cap".to_string()
        )));
    }

    #[test]
    fn capability_as_str_is_lowercase() {
        assert_eq!(
            ComponentCapability::FrameProvider.as_str(),
            "frame-provider"
        );
        assert_eq!(
            ComponentCapability::Prefetchable.as_str(),
            "prefetchable"
        );
        assert_eq!(
            ComponentCapability::Custom("foo-bar".to_string()).as_str(),
            "foo-bar"
        );
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
            ComponentCapability::Custom("foo".to_string()).standard_fn(),
            None
        );
    }

    #[test]
    fn resolve_definition_flattens_hierarchy_depth_first() {
        let mut registry = ComponentRegistry::default();
        let video = ComponentDefinition {
            id: NamespacedID::new("std", "video").unwrap(),
            name: "Video".to_string(),
            kind: NamespacedID::new("std", "media-provider").unwrap(),
            capabilities: HashSet::from([ComponentCapability::FrameProvider]),
            editor: None,
            script: None,
            properties: Vec::new(),
            children: vec![
                NamespacedID::new("std", "decoder").unwrap(),
                NamespacedID::new("std", "transform").unwrap(),
            ],
        };
        let decoder = ComponentDefinition {
            id: NamespacedID::new("std", "decoder").unwrap(),
            name: "Decoder".to_string(),
            kind: NamespacedID::new("std", "media-provider").unwrap(),
            capabilities: HashSet::from([ComponentCapability::FrameProvider]),
            editor: None,
            script: None,
            properties: Vec::new(),
            children: vec![NamespacedID::new("std", "frame-buffer").unwrap()],
        };
        let frame_buffer = ComponentDefinition {
            id: NamespacedID::new("std", "frame-buffer").unwrap(),
            name: "FrameBuffer".to_string(),
            kind: NamespacedID::new("std", "effect").unwrap(),
            capabilities: HashSet::new(),
            editor: None,
            script: None,
            properties: Vec::new(),
            children: Vec::new(),
        };
        let transform = ComponentDefinition {
            id: NamespacedID::new("std", "transform").unwrap(),
            name: "Transform".to_string(),
            kind: NamespacedID::new("std", "transform").unwrap(),
            capabilities: HashSet::from([ComponentCapability::Transformable]),
            editor: None,
            script: None,
            properties: Vec::new(),
            children: Vec::new(),
        };
        registry
            .merge_component_definitions(HashMap::from([
                (
                    video.id.clone(),
                    video,
                ),
                (decoder.id.clone(), decoder),
                (frame_buffer.id.clone(), frame_buffer),
                (transform.id.clone(), transform),
            ]))
            .unwrap();

        let def = registry
            .resolve_definition(&NamespacedID::new("std", "video").unwrap())
            .unwrap();
        let ids: Vec<_> = def.components.iter().map(|c| c.id.clone()).collect();
        assert_eq!(
            ids,
            vec![
                NamespacedID::new("std", "video").unwrap(),
                NamespacedID::new("std", "decoder").unwrap(),
                NamespacedID::new("std", "frame-buffer").unwrap(),
                NamespacedID::new("std", "transform").unwrap(),
            ]
        );
    }

    #[test]
    fn resolve_definition_rejects_cycle() {
        let mut registry = ComponentRegistry::default();
        let a = ComponentDefinition {
            id: NamespacedID::new("std", "a").unwrap(),
            name: "A".to_string(),
            kind: NamespacedID::new("std", "effect").unwrap(),
            capabilities: HashSet::new(),
            editor: None,
            script: None,
            properties: Vec::new(),
            children: vec![NamespacedID::new("std", "b").unwrap()],
        };
        let b = ComponentDefinition {
            id: NamespacedID::new("std", "b").unwrap(),
            name: "B".to_string(),
            kind: NamespacedID::new("std", "effect").unwrap(),
            capabilities: HashSet::new(),
            editor: None,
            script: None,
            properties: Vec::new(),
            children: vec![NamespacedID::new("std", "a").unwrap()],
        };
        registry
            .merge_component_definitions(HashMap::from([
                (a.id.clone(), a),
                (b.id.clone(), b),
            ]))
            .unwrap();
        assert!(registry
            .resolve_definition(&NamespacedID::new("std", "a").unwrap())
            .is_err());
    }

    #[test]
    fn resolve_definition_rejects_missing_child() {
        let mut registry = ComponentRegistry::default();
        let root = ComponentDefinition {
            id: NamespacedID::new("std", "root").unwrap(),
            name: "Root".to_string(),
            kind: NamespacedID::new("std", "effect").unwrap(),
            capabilities: HashSet::new(),
            editor: None,
            script: None,
            properties: Vec::new(),
            children: vec![NamespacedID::new("std", "missing").unwrap()],
        };
        registry
            .merge_component_definitions(HashMap::from([(root.id.clone(), root)]))
            .unwrap();
        assert!(registry
            .resolve_definition(&NamespacedID::new("std", "root").unwrap())
            .is_err());
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
            kind: NamespacedID::new("std", "media-provider").unwrap(),
            capabilities: HashSet::from([ComponentCapability::FrameProvider]),
            editor: None,
            script: None,
            properties: vec![
                PropertySchema {
                    key: NamespacedID::new("std", "source").unwrap(),
                    category: vec!["Media".to_string()],
                    label: "Source".to_string(),
                    kind: FieldTypeKind::String,
                    default: FieldValue::String("default.mp4".to_string()),
                    capability: None,
                    member_capabilities: BTreeMap::new(),
                                    },
                PropertySchema {
                    key: NamespacedID::new("std", "volume").unwrap(),
                    category: vec![],
                    label: "Volume".to_string(),
                    kind: FieldTypeKind::Float {
                        min: Some(0.0),
                        max: Some(1.0),
                        step: 0.1,
                    },
                    default: FieldValue::Float(1.0),
                    capability: None,
                    member_capabilities: BTreeMap::new(),
                                    },
            ],
            children: Vec::new(),
        };

        let instance =
            ClipInstance::new(NamespacedID::new("std", "video").unwrap(), &[video_def]);
        assert_eq!(instance.properties.len(), 2);

        assert!(matches!(
            instance.properties.get(&NamespacedID::new("std", "source").unwrap()),
            Some(ClipBindingValue::Static(FieldValue::String(v))) if v == "default.mp4"
        ));
        assert!(matches!(
            instance.properties.get(&NamespacedID::new("std", "volume").unwrap()),
            Some(ClipBindingValue::Static(FieldValue::Float(v))) if *v == 1.0
        ));
        assert!(instance.state.is_empty());
    }

    #[test]
    fn clip_instance_new_merges_multiple_components_properties() {
        let a = ComponentDefinition {
            id: NamespacedID::new("std", "a").unwrap(),
            name: "A".to_string(),
            kind: NamespacedID::new("std", "effect").unwrap(),
            capabilities: HashSet::new(),
            editor: None,
            script: None,
            properties: vec![PropertySchema {
                key: NamespacedID::new("std", "prop_a").unwrap(),
                category: vec![],
                label: "Prop A".to_string(),
                kind: FieldTypeKind::String,
                default: FieldValue::String("a".to_string()),
                capability: None,
                member_capabilities: BTreeMap::new(),
                            }],
            children: Vec::new(),
        };
        let b = ComponentDefinition {
            id: NamespacedID::new("std", "b").unwrap(),
            name: "B".to_string(),
            kind: NamespacedID::new("std", "effect").unwrap(),
            capabilities: HashSet::new(),
            editor: None,
            script: None,
            properties: vec![PropertySchema {
                key: NamespacedID::new("std", "prop_b").unwrap(),
                category: vec![],
                label: "Prop B".to_string(),
                kind: FieldTypeKind::Int { min: None, max: None },
                default: FieldValue::Int(42),
                capability: None,
                member_capabilities: BTreeMap::new(),
                            }],
            children: Vec::new(),
        };

        let instance =
            ClipInstance::new(NamespacedID::new("std", "clip").unwrap(), &[a, b]);
        assert_eq!(instance.properties.len(), 2);

        assert!(matches!(
            instance.properties.get(&NamespacedID::new("std", "prop_a").unwrap()),
            Some(ClipBindingValue::Static(FieldValue::String(v))) if v == "a"
        ));
        assert!(matches!(
            instance.properties.get(&NamespacedID::new("std", "prop_b").unwrap()),
            Some(ClipBindingValue::Static(FieldValue::Int(v))) if *v == 42
        ));
    }

    #[test]
    fn parse_component_definitions_properties_default_empty() {
        let text = r#"
[[components]]
id = "video"
name = "Video"
kind = "media-provider"
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
kind = "media-provider"
capabilities = ["frame-provider"]
script = "video.rhai"
"#,
        )
        .unwrap();

        let mut loader = PluginLoader::new();
        loader.load_additional_plugin(&dir).unwrap();
        assert!(loader
            .get_component_script(&NamespacedID::new("sched-test", "video").unwrap())
            .is_some());

        let engine = Engine::new();
        let instance = ClipInstance {
            kind_id: NamespacedID::new("sched-test", "video").unwrap(),
            properties: HashMap::new(),
            state: HashMap::new(),
        };
        let def = RuntimeDefinition {
            root_id: NamespacedID::new("sched-test", "video").unwrap(),
            components: vec![ComponentDefinition {
                id: NamespacedID::new("sched-test", "video").unwrap(),
                name: "Video".to_string(),
                kind: NamespacedID::new("sched-test", "media-provider").unwrap(),
                capabilities: HashSet::from([ComponentCapability::FrameProvider]),
                editor: None,
                script: Some("video.rhai".to_string()),
                properties: Vec::new(),
                children: Vec::new(),
            }],
        };

        use crate::render::video::MediaFetchCache;
        use dashmap::DashMap;
        use std::sync::Arc;
        use glam::Mat4;
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
        assert!(result.is_ok(), "scheduler should run the Rhai capability function: {result:?}");

        let _ = fs::remove_dir_all(&dir);
    }
}
