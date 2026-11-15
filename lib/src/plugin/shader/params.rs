//! シェーダ定義の `properties` 宣言から Params uniform のバイト列を生成します。
//!
//! 宣言順に値を詰め、各値の配置は WGSL の uniform レイアウト規則
//! （f32 / i32 / u32 は 4 バイト、vec2 は 8 バイト、vec4 / mat4 は 16 バイトアライン）
//! に従います。全体サイズは最大アライメントに切り上げます。

use std::collections::HashMap;

use anyhow::{bail, Context};

use crate::plugin::{
    property::value::{FieldTypeKind, FieldValue},
    shader::{CoreParamType, ShaderDefinition, ShaderProperty},
};

/// lib が供給する `core:` パラメータの値です。バリアントは `CoreParamType` と対応します。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoreParamValue {
    Float32(f32),
    Vec2F32([f32; 2]),
    Mat4x4F32([f32; 16]),
}

fn align_up(offset: usize, align: usize) -> usize {
    (offset + align - 1) / align * align
}

/// バイト列へアライメントを揃えて値を書き込むための内部バッファです。
#[derive(Default)]
struct Packer {
    bytes: Vec<u8>,
    max_align: usize,
}

impl Packer {
    fn write(&mut self, align: usize, data: &[u8]) {
        let offset = align_up(self.bytes.len(), align);
        self.bytes.resize(offset, 0);
        self.bytes.extend_from_slice(data);
        self.max_align = self.max_align.max(align);
    }

    fn write_f32(&mut self, value: f32) {
        self.write(4, &value.to_le_bytes());
    }

    fn write_f32_array(&mut self, align: usize, values: &[f32]) {
        let mut data = vec![0u8; values.len() * 4];
        for (i, value) in values.iter().enumerate() {
            data[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        self.write(align, &data);
    }

    fn finish(mut self) -> Vec<u8> {
        let align = self.max_align.max(1);
        self.bytes.resize(align_up(self.bytes.len(), align), 0);
        self.bytes
    }
}

/// シェーダ定義の宣言順にパラメータ値を詰めた Params uniform のバイト列を生成します。
///
/// `core_values` は `core:` パラメータの値で、宣言された全項目分が必要です。
/// `own_values` は固有プロパティの値で、キーは TOML で宣言したローカルキーです。
/// 値が無い固有プロパティは TOML の `default` を使います。
/// 未知キー・型不一致・`core:` 値の不足はエラーになります。
pub fn pack_params(
    definition: &ShaderDefinition,
    core_values: &HashMap<String, CoreParamValue>,
    own_values: &HashMap<String, FieldValue>,
) -> anyhow::Result<Vec<u8>> {
    for key in core_values.keys() {
        let declared = definition
            .properties
            .iter()
            .any(|p| matches!(p, ShaderProperty::Core { key: k, .. } if k == key));
        if !declared {
            bail!(
                "unknown core parameter `{key}` for shader `{}`",
                definition.id.full()
            );
        }
    }
    for key in own_values.keys() {
        let declared = definition
            .properties
            .iter()
            .any(|p| matches!(p, ShaderProperty::Own(schema) if schema.key.local_id() == key));
        if !declared {
            bail!(
                "unknown property `{key}` for shader `{}`",
                definition.id.full()
            );
        }
    }

    let mut packer = Packer::default();
    for property in &definition.properties {
        match property {
            ShaderProperty::Core { key, value_type } => {
                let value = core_values.get(key).with_context(|| {
                    format!(
                        "missing value for core parameter `{key}` of shader `{}`",
                        definition.id.full()
                    )
                })?;
                write_core(&mut packer, *value_type, *value).with_context(|| {
                    format!(
                        "invalid value for core parameter `{key}` of shader `{}`",
                        definition.id.full()
                    )
                })?;
            }
            ShaderProperty::Own(schema) => {
                let value = own_values
                    .get(schema.key.local_id())
                    .unwrap_or(&schema.default);
                write_own(&mut packer, &schema.kind, value).with_context(|| {
                    format!(
                        "invalid value for property `{}` of shader `{}`",
                        schema.key.local_id(),
                        definition.id.full()
                    )
                })?;
            }
        }
    }
    Ok(packer.finish())
}

fn write_core(
    packer: &mut Packer,
    value_type: CoreParamType,
    value: CoreParamValue,
) -> anyhow::Result<()> {
    match (value_type, value) {
        (CoreParamType::Float32, CoreParamValue::Float32(v)) => packer.write_f32(v),
        (CoreParamType::Vec2F32, CoreParamValue::Vec2F32(v)) => packer.write_f32_array(8, &v),
        (CoreParamType::Mat4x4F32, CoreParamValue::Mat4x4F32(v)) => packer.write_f32_array(16, &v),
        _ => bail!("core parameter value type does not match the declared type"),
    }
    Ok(())
}

fn write_own(packer: &mut Packer, kind: &FieldTypeKind, value: &FieldValue) -> anyhow::Result<()> {
    match (kind, value) {
        (FieldTypeKind::Float { .. }, FieldValue::Float(v)) => packer.write_f32(*v as f32),
        // TOML の整数リテラル救済（parse_toml_to）と同様に、Float には Int も許容する。
        (FieldTypeKind::Float { .. }, FieldValue::Int(v)) => packer.write_f32(*v as f32),
        (FieldTypeKind::Int { .. }, FieldValue::Int(v)) => {
            let v = i32::try_from(*v)
                .map_err(|_| anyhow::anyhow!("integer value {v} is out of i32 range"))?;
            packer.write(4, &v.to_le_bytes());
        }
        (FieldTypeKind::Bool, FieldValue::Bool(v)) => packer.write(4, &(*v as u32).to_le_bytes()),
        (FieldTypeKind::Color, FieldValue::Color(c)) => {
            packer.write_f32_array(16, &[c.r, c.g, c.b, c.a])
        }
        (
            FieldTypeKind::Float { .. }
            | FieldTypeKind::Int { .. }
            | FieldTypeKind::Bool
            | FieldTypeKind::Color,
            _,
        ) => {
            bail!("property value type does not match the declared kind");
        }
        _ => bail!("property kind cannot be packed into shader params"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
