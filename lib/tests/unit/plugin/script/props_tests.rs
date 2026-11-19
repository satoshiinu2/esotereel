use std::collections::{BTreeSet, HashMap};

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
fn resolves_self_relative_and_dependency_values() {
    let (definition, instance, owner) = fixture(
        vec![ComponentRequire {
            name: "dep".to_string(),
            capability: ComponentCapability::Transformable,
            required: false,
        }],
        vec![],
    );
    let props = ComponentProps::build(&definition, &instance, &owner).unwrap();
    // 自プロパティは `:#` 形式でのみ引ける。
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
    assert!(
        eval(props.clone(), "props.has(\"dep\")")
            .unwrap()
            .cast::<bool>()
    );
    assert!(
        eval(props.clone(), "props.has(\":#own\")")
            .unwrap()
            .cast::<bool>()
    );
    // 裸の名前で自プロパティを引くと、`:#` 形式へ誘導するエラーになる。
    let err = eval(props.clone(), "props[\"own\"]").unwrap_err();
    assert!(err.to_string().contains(":#own"), "{err}");
    assert!(eval(props, "props.has(\"own\")").is_err());
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
    assert!(
        eval(props.clone(), "props.has(\"marker\")")
            .unwrap()
            .cast::<bool>()
    );
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
