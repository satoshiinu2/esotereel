use super::*;

#[test]
fn parse_toml_to_bool() {
    let v = FieldTypeKind::Bool
        .parse_toml_to(&toml::Value::Boolean(true))
        .unwrap();
    assert_eq!(v, FieldValue::Bool(true));
}

#[test]
fn parse_toml_to_int_out_of_range() {
    let err = FieldTypeKind::Int {
        min: Some(0),
        max: Some(10),
    }
    .parse_toml_to(&toml::Value::Integer(100))
    .unwrap_err();
    assert!(matches!(err, ConvertError::OutOfRange(_)));
}

#[test]
fn parse_toml_to_float_accepts_integer() {
    let v = FieldTypeKind::Float {
        min: None,
        max: None,
        step: 0.1,
    }
    .parse_toml_to(&toml::Value::Integer(1))
    .unwrap();
    assert_eq!(v, FieldValue::Float(1.0));
}

#[test]
fn parse_toml_to_enum_invalid_option() {
    let err = FieldTypeKind::Enum {
        options: vec!["a".to_string(), "b".to_string()],
    }
    .parse_toml_to(&toml::Value::String("c".to_string()))
    .unwrap_err();
    assert!(matches!(err, ConvertError::InvalidOption(_)));
}

#[test]
fn parse_toml_to_type_mismatch() {
    let err = FieldTypeKind::String
        .parse_toml_to(&toml::Value::Integer(5))
        .unwrap_err();
    assert!(matches!(err, ConvertError::TypeMismatch { .. }));
}

#[test]
fn parse_toml_to_invalid_color() {
    let err = FieldTypeKind::Color
        .parse_toml_to(&toml::Value::String("not-a-color".to_string()))
        .unwrap_err();
    assert!(matches!(err, ConvertError::InvalidColor(_)));
}

#[test]
fn field_value_to_string() {
    assert_eq!(FieldValue::Int(42).to_string(), "42");
    assert_eq!(FieldValue::Float(1.5).to_string(), "1.5");
    assert_eq!(FieldValue::Bool(true).to_string(), "true");
    assert_eq!(FieldValue::String("hello".to_string()).to_string(), "hello");
    assert_eq!(
        FieldValue::Color(crate::util::color::RgbaColor {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        })
        .to_string(),
        "#FF0000FF"
    );
}
