use super::*;
use crate::plugin::{
    property::value::{FieldTypeKind, FieldValue},
    shader::CoreParamType,
};

const COMPUTE: &str = r#"
id = "wave_compute"
name = "Wave (Compute)"
source = "wave_compute.wgsl"
kind = "compute"
entry_compute = "deform_main"

[[properties]]
key = "core:time"

[[properties]]
key = "amplitude"
label = "Amplitude"
kind = { type = "float", min = 0.0, max = 100.0, step = 0.1 }
default = 10.0
"#;

const RENDER: &str = r##"
id = "wave"
name = "Wave"
source = "wave.wgsl"
entry_vertex = "vs_main"
entry_fragment = "fs_main"

[[properties]]
key = "core:view_projection"

[[properties]]
key = "tint"
label = "Tint"
kind = { type = "color" }
default = "#FFFFFF"
"##;

#[test]
fn parse_compute_shader_definition() {
    let def = parse_shader_definition(COMPUTE, "std").unwrap();
    assert_eq!(def.id.full(), "std:wave_compute");
    assert_eq!(
        def.kind,
        ShaderKind::Compute {
            entry_compute: "deform_main".to_string()
        }
    );
    assert_eq!(def.properties.len(), 2);
    assert_eq!(
        def.properties[0],
        ShaderProperty::Core {
            key: "core:time".to_string(),
            value_type: CoreParamType::Float32,
        }
    );
    let ShaderProperty::Own(schema) = &def.properties[1] else {
        panic!("expected own property");
    };
    assert_eq!(
        schema.kind,
        FieldTypeKind::Float {
            min: Some(0.0),
            max: Some(100.0),
            step: 0.1
        }
    );
    assert_eq!(schema.default, FieldValue::Float(10.0));
}

#[test]
fn parse_render_shader_definition() {
    let def = parse_shader_definition(RENDER, "std").unwrap();
    assert_eq!(
        def.kind,
        ShaderKind::Render {
            entry_vertex: "vs_main".to_string(),
            entry_fragment: "fs_main".to_string(),
        }
    );
    assert_eq!(
        def.properties[0],
        ShaderProperty::Core {
            key: "core:view_projection".to_string(),
            value_type: CoreParamType::Mat4x4F32,
        }
    );
}

#[test]
fn parse_rejects_unknown_core_parameter() {
    let text = COMPUTE.replace("core:time", "core:bogus");
    assert!(parse_shader_definition(&text, "std").is_err());
}

#[test]
fn parse_rejects_kind_on_core_parameter() {
    let text = r#"
id = "bad"
name = "Bad"
source = "bad.wgsl"
kind = "compute"
entry_compute = "main"

[[properties]]
key = "core:time"
kind = { type = "float", step = 0.1 }
"#;
    assert!(parse_shader_definition(text, "std").is_err());
}

#[test]
fn parse_rejects_compute_without_entry() {
    let text = r#"
id = "bad"
name = "Bad"
source = "bad.wgsl"
kind = "compute"
"#;
    assert!(parse_shader_definition(text, "std").is_err());
}
