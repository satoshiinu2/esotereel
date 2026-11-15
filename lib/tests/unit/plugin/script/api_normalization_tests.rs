use super::*;
use std::collections::BTreeMap;

#[test]
fn dynamic_to_field_value_int_stays_int() {
    assert_eq!(
        dynamic_to_field_value(42i64.into()).unwrap(),
        FieldValue::Int(42)
    );
}

#[test]
fn dynamic_to_field_value_float_stays_float() {
    assert_eq!(
        dynamic_to_field_value(3.14f64.into()).unwrap(),
        FieldValue::Float(3.14)
    );
}

#[test]
fn dynamic_to_field_value_bool_stays_bool() {
    assert_eq!(
        dynamic_to_field_value(true.into()).unwrap(),
        FieldValue::Bool(true)
    );
}

#[test]
fn dynamic_to_field_value_string_stays_string() {
    assert_eq!(
        dynamic_to_field_value("hello".into()).unwrap(),
        FieldValue::String("hello".to_string())
    );
}

#[test]
fn dynamic_to_field_value_numeric_string_parses() {
    assert_eq!(
        dynamic_to_field_value("42".into()).unwrap(),
        FieldValue::Int(42)
    );
    assert_eq!(
        dynamic_to_field_value("3.14".into()).unwrap(),
        FieldValue::Float(3.14)
    );
    assert_eq!(
        dynamic_to_field_value("true".into()).unwrap(),
        FieldValue::Bool(true)
    );
}

#[test]
fn dynamic_to_field_value_array() {
    let arr = rhai::Array::from(vec![
        rhai::Dynamic::from(1i64),
        rhai::Dynamic::from(2.5f64),
        rhai::Dynamic::from("three".to_string()),
    ]);
    assert_eq!(
        dynamic_to_field_value(arr.into()).unwrap(),
        FieldValue::Array(vec![
            FieldValue::Int(1),
            FieldValue::Float(2.5),
            FieldValue::String("three".to_string()),
        ])
    );
}

#[test]
fn dynamic_to_field_value_map() {
    let mut map = rhai::Map::new();
    map.insert("count".into(), rhai::Dynamic::from(10i64));
    map.insert("name".into(), rhai::Dynamic::from("test".to_string()));
    assert_eq!(
        dynamic_to_field_value(map.into()).unwrap(),
        FieldValue::Map(BTreeMap::from([
            ("count".to_string(), FieldValue::Int(10)),
            ("name".to_string(), FieldValue::String("test".to_string())),
        ]))
    );
}
