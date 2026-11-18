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
/// 見える名前は、`:#` 形式で引く自コンポーネントのプロパティと、裸の名前で
/// 引く `[[requires]]` で宣言した依存だけ（サンドボックス仕様）。
/// 宣言されていない名前へのアクセスは、黙って空を返さずキー名を含む
/// エラーにする。
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
    /// 自コンポーネントのプロパティを `:#` 形式のローカル名で、`[[requires]]`
    /// で宣言した依存先は capability に紐づく値だけを依存名で解決できる
    /// ようにする（依存先の全プロパティは渡らない）。
    /// 必須依存が満たせなければエラー。
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

    /// キーアクセスを解決する。
    ///
    /// `:#name` 形式は自プロパティ専用で、裸の名前は `[[requires]]` の
    /// 依存名としてだけ解決する。自プロパティを裸の名前で引いた場合は、
    /// 黙って解決せず `:#name` 形式へ誘導するエラーにする。
    fn resolve(&self, name: &str) -> Result<Dynamic, Box<EvalAltResult>> {
        if let Some(own_name) = name.strip_prefix(":#") {
            return match self.own.get(own_name) {
                Some(value) => Ok(value.as_ref().map_or(Dynamic::UNIT, PropsValue::to_dynamic)),
                None => Err(self.not_own_property_error(name)),
            };
        }
        if self.own.contains_key(name) {
            return Err(self.self_needs_prefix_error(name));
        }
        match self.dependencies.get(name) {
            Some(Dependency::Value(value)) => Ok(value.to_dynamic()),
            Some(Dependency::DeclaredOnly | Dependency::Unsatisfied) => Ok(Dynamic::UNIT),
            None => Err(self.undeclared_error(name)),
        }
    }

    /// 宣言した名前が満たされているかを返す。任意依存の有無での分岐用。
    /// 名前の解釈はキーアクセスと同じ（自プロパティは `:#name` 形式のみ）。
    fn has(&self, name: &str) -> Result<bool, Box<EvalAltResult>> {
        if let Some(own_name) = name.strip_prefix(":#") {
            return match self.own.get(own_name) {
                Some(value) => Ok(value.is_some()),
                None => Err(self.not_own_property_error(name)),
            };
        }
        if self.own.contains_key(name) {
            return Err(self.self_needs_prefix_error(name));
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

    fn self_needs_prefix_error(&self, name: &str) -> Box<EvalAltResult> {
        format!(
            "`{name}` is a property of component `{}`; access it as `:#{name}`",
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
#[path = "../../../tests/unit/plugin/script/props_tests.rs"]
mod tests;
