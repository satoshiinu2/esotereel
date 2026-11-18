use rhai::{Dynamic, Map};

use crate::{plugin::property::value::FieldValue, project::clip::ClipBindingValue};

/// `ClipBindingValue` を `rhai::Dynamic` に変換し、スクリプトエンジンに渡せる形式にします。
pub fn binding_value_to_dynamic(v: &ClipBindingValue) -> Dynamic {
    match v {
        ClipBindingValue::Static(fv) => field_value_to_dynamic(fv),
        // 将来Keyframes等が増えたら、ここでmedia_time時点の値を評価して渡す
    }
}

pub fn field_value_to_dynamic(v: &FieldValue) -> Dynamic {
    match v {
        FieldValue::Bool(b) => Dynamic::from(*b),
        FieldValue::Int(i) => Dynamic::from(*i),
        FieldValue::Float(f) => Dynamic::from(*f),
        FieldValue::Enum(s) | FieldValue::String(s) => Dynamic::from(s.clone()),
        FieldValue::Path(paths) => Dynamic::from_array(
            paths
                .iter()
                .map(|path| Dynamic::from(path.to_string_lossy().into_owned()))
                .collect(),
        ),
        FieldValue::Color(c) => {
            let mut m = Map::new();
            m.insert("r".into(), Dynamic::from(c.r));
            m.insert("g".into(), Dynamic::from(c.g));
            m.insert("b".into(), Dynamic::from(c.b));
            m.insert("a".into(), Dynamic::from(c.a));
            Dynamic::from_map(m)
        }
        FieldValue::Array(arr) => {
            Dynamic::from_array(arr.iter().map(field_value_to_dynamic).collect())
        }
        FieldValue::Map(map) => {
            let m: Map = map
                .iter()
                .map(|(k, v)| (k.into(), field_value_to_dynamic(v)))
                .collect();
            Dynamic::from_map(m)
        }
    }
}
