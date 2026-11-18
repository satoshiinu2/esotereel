use super::*;
use crate::plugin::shader::ShaderKind;

#[test]
fn shader_definitions_are_loaded_and_compiled_via_plugin() {
    use std::fs;

    let dir = std::env::temp_dir().join("esotereel_shader_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("shaders")).unwrap();

    fs::write(
        dir.join("manifest.toml"),
        "id = \"shader-test\"\nname = \"Shader Test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        dir.join("shaders").join("wave_compute.toml"),
        r#"
id = "wave_compute"
name = "Wave (Compute)"
source = "wave_compute.wgsl"
kind = "compute"
entry_compute = "deform_main"

[[properties]]
key = "core:time"
"#,
    )
    .unwrap();
    fs::write(
        dir.join("shaders").join("wave_compute.wgsl"),
        r#"
import core::common::CLIP_VERTEX_FLOATS;

@group(0) @binding(0)
var<storage, read_write> vertices: array<f32>;

@compute @workgroup_size(64)
fn deform_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x < arrayLength(&vertices) / CLIP_VERTEX_FLOATS) {
        vertices[gid.x * CLIP_VERTEX_FLOATS + 1u] += 1.0;
    }
}
"#,
    )
    .unwrap();

    let mut loader = PluginLoader::new();
    loader.load_additional_plugin(&dir).unwrap();

    let id = NamespacedID::new("shader-test", "wave_compute").unwrap();
    let def = loader
        .get_shader(&id)
        .expect("shader definition must be registered");
    assert_eq!(
        def.kind,
        ShaderKind::Compute {
            entry_compute: "deform_main".to_string()
        }
    );
    // wesl が core::common の import を解決し、検証済み WGSL が保持される。
    assert!(def.compiled_wgsl.contains("deform_main"));
    assert!(def.compiled_wgsl.contains("9u"));
}

#[test]
fn std_plugin_shader_is_loaded_and_compiled() {
    let mut loader = PluginLoader::new();
    loader
        .load_additional_plugin(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plugins/std"
        )))
        .unwrap();

    let id = NamespacedID::new("std", "wave_compute").unwrap();
    let def = loader
        .get_shader(&id)
        .expect("std:wave_compute must be registered");
    assert_eq!(
        def.kind,
        ShaderKind::Compute {
            entry_compute: "deform_main".to_string()
        }
    );
    assert_eq!(def.properties.len(), 4);
    assert!(def.compiled_wgsl.contains("deform_main"));
}
