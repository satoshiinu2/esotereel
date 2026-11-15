use std::collections::HashMap;

use anyhow::bail;
use rhai::{Dynamic, Engine, EvalAltResult, ImmutableString};

use crate::plugin::clip::component::{
    ClipInstance, ComponentDefinition, RequirementResolution, RuntimeDefinition,
};
use crate::plugin::property::value::FieldValue;
use crate::plugin::NamespacedID;
use crate::project::clip::ClipBindingValue;

use super::bridge::{binding_value_to_dynamic, field_value_to_dynamic};

/// スクリプトへ渡す props のハンドル。
///
/// 事前に Map 化して配るのではなく、呼び出し時点のプロパティ値の
/// スナップショットを保持し、スクリプトからのキーアクセスのたびに解決する。
/// 見える名前は自コンポーネントのプロパティと `[[requires]]` で宣言した
/// 依存だけ（サンドボックス仕様）。宣言されていない名前へのアクセスは、
/// 黙って空を返さずキー名を含むエラーにする。
#[derive(Clone)]
pub struct ComponentProps {
    component_id: NamespacedID,
    own: HashMap<String, Option<PropsValue>>,
    dependencies: HashMap<String, Dependency>,
}

/// アクセス時に Rhai の値へ変換する保持値。
#[derive(Clone)]
enum PropsValue {
    Binding(ClipBindingValue),
    Field(FieldValue),
}

impl PropsValue {
    fn to_dynamic(&self) -> Dynamic {
        match self {
            PropsValue::Binding(value) => binding_value_to_dynamic(value),
            PropsValue::Field(value) => field_value_to_dynamic(value),
        }
    }
}

/// `[[requires]]` 依存の解決状態。
#[derive(Clone)]
enum Dependency {
    /// capability に紐づく値がある。
    Value(PropsValue),
    /// コンポーネント自体が capability を持つ。満たされているが渡す値はない。
    DeclaredOnly,
    /// 任意依存が満たされていない（必須なら構築時にエラーになる）。
    Unsatisfied,
}

impl ComponentProps {
    /// `component` のスクリプトへ渡す props を構築する。
    ///
    /// 自コンポーネントのプロパティをローカル名で、`[[requires]]` で宣言した
    /// 依存先は capability に紐づく値だけを依存名で解決できるようにする
    /// （依存先の全プロパティは渡らない）。必須依存が満たせなければエラー。
    pub fn build(
        definition: &RuntimeDefinition,
        instance: &ClipInstance,
        component: &ComponentDefinition,
    ) -> anyhow::Result<Self> {
        let mut own = HashMap::new();
        for schema in &component.properties {
            let value = instance
                .properties
                .get(&schema.key)
                .cloned()
                .map(PropsValue::Binding);
            own.insert(schema.key.name().to_owned(), value);
        }
        let mut dependencies = HashMap::new();
        for require in &component.requires {
            let dependency = match definition.resolve_requirement(component, require) {
                Some(RequirementResolution::Property { schema, .. }) => {
                    match instance.properties.get(&schema.key) {
                        Some(value) => Dependency::Value(PropsValue::Binding(value.clone())),
                        None => Dependency::Unsatisfied,
                    }
                }
                Some(RequirementResolution::Member { schema, member, .. }) => {
                    let member_value = match instance.properties.get(&schema.key) {
                        Some(ClipBindingValue::Static(FieldValue::Map(values))) => {
                            values.get(member).cloned()
                        }
                        _ => None,
                    };
                    match member_value {
                        Some(value) => Dependency::Value(PropsValue::Field(value)),
                        None => Dependency::Unsatisfied,
                    }
                }
                Some(RequirementResolution::DeclaredOnly { .. }) => Dependency::DeclaredOnly,
                None => {
                    if require.required {
                        bail!(
                            "component `{}` requires capability `{:?}` as `{}` but it is not satisfied",
                            component.id.full(),
                            require.capability,
                            require.name
                        );
                    }
                    Dependency::Unsatisfied
                }
            };
            dependencies.insert(require.name.clone(), dependency);
        }
        Ok(Self {
            component_id: component.id.clone(),
            own,
            dependencies,
        })
    }

    /// キーアクセスを解決する。`:#name` 形式は自プロパティとして解決する。
    fn resolve(&self, name: &str) -> Result<Dynamic, Box<EvalAltResult>> {
        if let Some(own_name) = name.strip_prefix(":#") {
            return match self.own.get(own_name) {
                Some(value) => Ok(value.as_ref().map_or(Dynamic::UNIT, PropsValue::to_dynamic)),
                None => Err(self.not_own_property_error(name)),
            };
        }
        if let Some(value) = self.own.get(name) {
            return Ok(value.as_ref().map_or(Dynamic::UNIT, PropsValue::to_dynamic));
        }
        match self.dependencies.get(name) {
            Some(Dependency::Value(value)) => Ok(value.to_dynamic()),
            Some(Dependency::DeclaredOnly | Dependency::Unsatisfied) => Ok(Dynamic::UNIT),
            None => Err(self.undeclared_error(name)),
        }
    }

    /// 宣言した名前が満たされているかを返す。任意依存の有無での分岐用。
    fn has(&self, name: &str) -> Result<bool, Box<EvalAltResult>> {
        if let Some(own_name) = name.strip_prefix(":#") {
            return match self.own.get(own_name) {
                Some(value) => Ok(value.is_some()),
                None => Err(self.not_own_property_error(name)),
            };
        }
        if let Some(value) = self.own.get(name) {
            return Ok(value.is_some());
        }
        match self.dependencies.get(name) {
            Some(Dependency::Value(_) | Dependency::DeclaredOnly) => Ok(true),
            Some(Dependency::Unsatisfied) => Ok(false),
            None => Err(self.undeclared_error(name)),
        }
    }

    fn undeclared_error(&self, name: &str) -> Box<EvalAltResult> {
        format!(
            "props of component `{}` has no property or [[requires]] dependency named `{name}`",
            self.component_id.full()
        )
        .into()
    }

    fn not_own_property_error(&self, name: &str) -> Box<EvalAltResult> {
        format!(
            "`{name}` is not a property of component `{}`",
            self.component_id.full()
        )
        .into()
    }
}

/// props ハンドルの型と、キーアクセス・存在確認の関数をエンジンへ登録する。
pub fn register_props_api(engine: &mut Engine) {
    engine.register_type_with_name::<ComponentProps>("ComponentProps");
    engine.register_indexer_get(|props: &mut ComponentProps, name: ImmutableString| {
        props.resolve(&name)
    });
    engine.register_fn(
        "has",
        |props: &mut ComponentProps, name: ImmutableString| props.has(&name),
    );
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use rhai::{Engine, Scope};

    use super::*;
    use crate::plugin::clip::component::ComponentRequire;
    use crate::plugin::property::value::FieldTypeKind;
    use crate::plugin::property::{ComponentCapability, PropertyKey, PropertySchema};

    fn int_schema(
        owner: &NamespacedID,
        name: &str,
        capability: Option<ComponentCapability>,
    ) -> PropertySchema {
        PropertySchema {
            key: PropertyKey::new(owner.clone(), name).unwrap(),
            category: vec![],
            label: name.to_string(),
            kind: FieldTypeKind::Int {
                min: None,
                max: None,
            },
            default: FieldValue::Int(0),
            capability,
            member_capabilities: BTreeMap::new(),
        }
    }

    fn component(
        id: &NamespacedID,
        capabilities: &[ComponentCapability],
        properties: Vec<PropertySchema>,
        requires: Vec<ComponentRequire>,
    ) -> ComponentDefinition {
        ComponentDefinition {
            id: id.clone(),
            name: id.local_id().to_string(),
            kind: id.clone(),
            capabilities: capabilities.iter().cloned().collect::<BTreeSet<_>>(),
            editor: None,
            script: None,
            properties,
            requires,
        }
    }

    /// owner（自プロパティ `own` = 7）と、Transformable なプロパティ
    /// `provided` = 42 を持つ provider からなる定義とインスタンス。
    fn fixture(
        requires: Vec<ComponentRequire>,
        extra_components: Vec<ComponentDefinition>,
    ) -> (RuntimeDefinition, ClipInstance, ComponentDefinition) {
        let owner_id = NamespacedID::new("test", "owner").unwrap();
        let provider_id = NamespacedID::new("test", "provider").unwrap();
        let owner = component(
            &owner_id,
            &[],
            vec![int_schema(&owner_id, "own", None)],
            requires,
        );
        let provider = component(
            &provider_id,
            &[],
            vec![int_schema(
                &provider_id,
                "provided",
                Some(ComponentCapability::Transformable),
            )],
            vec![],
        );
        let mut components = vec![owner.clone(), provider];
        components.extend(extra_components);
        let definition = RuntimeDefinition {
            kind_id: NamespacedID::new("test", "clip").unwrap(),
            components,
        };
        let instance = ClipInstance {
            kind_id: definition.kind_id.clone(),
            properties: HashMap::from([
                (
                    PropertyKey::new(owner_id, "own").unwrap(),
                    ClipBindingValue::Static(FieldValue::Int(7)),
                ),
                (
                    PropertyKey::new(provider_id, "provided").unwrap(),
                    ClipBindingValue::Static(FieldValue::Int(42)),
                ),
            ]),
            state: HashMap::new(),
        };
        (definition, instance, owner)
    }

    fn eval(props: ComponentProps, code: &str) -> Result<Dynamic, Box<EvalAltResult>> {
        let mut engine = Engine::new();
        register_props_api(&mut engine);
        let mut scope = Scope::new();
        scope.push("props", props);
        engine.eval_with_scope(&mut scope, code)
    }

    #[test]
    fn resolves_own_self_relative_and_dependency_values() {
        let (definition, instance, owner) = fixture(
            vec![ComponentRequire {
                name: "dep".to_string(),
                capability: ComponentCapability::Transformable,
                required: false,
            }],
            vec![],
        );
        let props = ComponentProps::build(&definition, &instance, &owner).unwrap();
        assert_eq!(
            eval(props.clone(), "props[\"own\"]").unwrap().cast::<i64>(),
            7
        );
        assert_eq!(
            eval(props.clone(), "props[\":#own\"]")
                .unwrap()
                .cast::<i64>(),
            7
        );
        assert_eq!(
            eval(props.clone(), "props[\"dep\"]").unwrap().cast::<i64>(),
            42
        );
        assert!(eval(props.clone(), "props.has(\"dep\")")
            .unwrap()
            .cast::<bool>());
        assert!(eval(props, "props.has(\":#own\")").unwrap().cast::<bool>());
    }

    #[test]
    fn unsatisfied_optional_dependency_is_unit_and_has_is_false() {
        let (definition, instance, owner) = fixture(
            vec![ComponentRequire {
                name: "maybe".to_string(),
                capability: ComponentCapability::Keyframeable,
                required: false,
            }],
            vec![],
        );
        let props = ComponentProps::build(&definition, &instance, &owner).unwrap();
        assert!(eval(props.clone(), "props[\"maybe\"]").unwrap().is_unit());
        assert!(!eval(props, "props.has(\"maybe\")").unwrap().cast::<bool>());
    }

    #[test]
    fn unsatisfied_required_dependency_fails_build() {
        let (definition, instance, owner) = fixture(
            vec![ComponentRequire {
                name: "need".to_string(),
                capability: ComponentCapability::Keyframeable,
                required: true,
            }],
            vec![],
        );
        assert!(ComponentProps::build(&definition, &instance, &owner).is_err());
    }

    #[test]
    fn declared_only_dependency_has_no_value_but_counts_as_satisfied() {
        let marker_id = NamespacedID::new("test", "marker").unwrap();
        let marker = component(
            &marker_id,
            &[ComponentCapability::Prefetchable],
            vec![],
            vec![],
        );
        let (definition, instance, owner) = fixture(
            vec![ComponentRequire {
                name: "marker".to_string(),
                capability: ComponentCapability::Prefetchable,
                required: false,
            }],
            vec![marker],
        );
        let props = ComponentProps::build(&definition, &instance, &owner).unwrap();
        assert!(eval(props.clone(), "props.has(\"marker\")")
            .unwrap()
            .cast::<bool>());
        assert!(eval(props, "props[\"marker\"]").unwrap().is_unit());
    }

    #[test]
    fn undeclared_name_access_is_an_error_naming_the_key() {
        let (definition, instance, owner) = fixture(vec![], vec![]);
        let props = ComponentProps::build(&definition, &instance, &owner).unwrap();
        let err = eval(props.clone(), "props[\"nope\"]").unwrap_err();
        assert!(err.to_string().contains("nope"), "{err}");
        assert!(eval(props.clone(), "props.has(\"nope\")").is_err());
        assert!(eval(props, "props[\":#nope\"]").is_err());
    }
}
