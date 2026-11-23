use super::*;
use crate::plugin::shader::parse_shader_definition;

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

[[properties]]
key = "frequency"
label = "Frequency"
kind = { type = "float", min = 0.0, max = 1.0, step = 0.01 }
default = 0.05

[[properties]]
key = "speed"
label = "Speed"
kind = { type = "float", min = 0.0, max = 10.0, step = 0.1 }
default = 2.0
"#;

const MIXED: &str = r##"
id = "mixed"
name = "Mixed"
source = "mixed.wgsl"
entry_vertex = "vs_main"
entry_fragment = "fs_main"

[[properties]]
key = "strength"
label = "Strength"
kind = { type = "float", step = 0.1 }
default = 0.5

[[properties]]
key = "tint"
label = "Tint"
kind = { type = "color" }
default = "#FF0000"

[[properties]]
key = "count"
label = "Count"
kind = { type = "int", min = 0, max = 10 }
default = 3

[[properties]]
key = "enabled"
label = "Enabled"
kind = { type = "bool" }
default = true
"##;

fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn pack_uses_defaults_for_missing_own_values() {
    let def = parse_shader_definition(COMPUTE, "std").unwrap();
    let core = HashMap::from([("core:time".to_string(), CoreParamValue::Float32(1.5))]);
    let own = HashMap::from([("amplitude".to_string(), FieldValue::Float(2.0))]);

    let bytes = pack_params(&def, &core, &own).unwrap();

    // time, amplitude（指定値）, frequency, speed（default）の順に f32 が並ぶ。
    assert_eq!(floats(&bytes), vec![1.5, 2.0, 0.05, 2.0]);
}

#[test]
fn pack_respects_alignment() {
    let def = parse_shader_definition(MIXED, "std").unwrap();
    let bytes = pack_params(&def, &HashMap::new(), &HashMap::new()).unwrap();

    // strength(f32) @0, tint(vec4) @16, count(i32) @32, enabled(u32) @36。
    // 全体は最大アライメント 16 に切り上げて 48 バイト。
    assert_eq!(bytes.len(), 48);
    let values = floats(&bytes);
    assert_eq!(values[0], 0.5);
    assert_eq!(values[4..8], [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(i32::from_le_bytes(bytes[32..36].try_into().unwrap()), 3);
    assert_eq!(u32::from_le_bytes(bytes[36..40].try_into().unwrap()), 1);
}

#[test]
fn pack_mat4_core_parameter() {
    let text = r#"
id = "render"
name = "Render"
source = "render.wgsl"
entry_vertex = "vs_main"
entry_fragment = "fs_main"

[[properties]]
key = "core:view_projection"
"#;
    let def = parse_shader_definition(text, "std").unwrap();
    let matrix: [f32; 16] = std::array::from_fn(|i| i as f32);
    let core = HashMap::from([(
        "core:view_projection".to_string(),
        CoreParamValue::Mat4x4F32(matrix),
    )]);

    let bytes = pack_params(&def, &core, &HashMap::new()).unwrap();

    assert_eq!(bytes.len(), 64);
    assert_eq!(floats(&bytes), matrix.to_vec());
}

#[test]
fn pack_rejects_unknown_keys() {
    let def = parse_shader_definition(COMPUTE, "std").unwrap();
    let core = HashMap::from([("core:time".to_string(), CoreParamValue::Float32(0.0))]);

    let own = HashMap::from([("bogus".to_string(), FieldValue::Float(1.0))]);
    assert!(pack_params(&def, &core, &own).is_err());

    let core = HashMap::from([("core:bogus".to_string(), CoreParamValue::Float32(0.0))]);
    assert!(pack_params(&def, &core, &HashMap::new()).is_err());
}

#[test]
fn pack_rejects_missing_and_mismatched_values() {
    let def = parse_shader_definition(COMPUTE, "std").unwrap();

    // core:time の値が無い。
    assert!(pack_params(&def, &HashMap::new(), &HashMap::new()).is_err());

    // core:time に vec2 を渡す。
    let core = HashMap::from([("core:time".to_string(), CoreParamValue::Vec2F32([0.0, 0.0]))]);
    assert!(pack_params(&def, &core, &HashMap::new()).is_err());

    // float プロパティに文字列を渡す。
    let core = HashMap::from([("core:time".to_string(), CoreParamValue::Float32(0.0))]);
    let own = HashMap::from([(
        "amplitude".to_string(),
        FieldValue::String("loud".to_string()),
    )]);
    assert!(pack_params(&def, &core, &own).is_err());
}
