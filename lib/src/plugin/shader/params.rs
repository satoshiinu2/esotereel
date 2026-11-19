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
            .any(|p| matches!(p, ShaderProperty::Own(schema) if schema.key.local_name() == key));
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
                    .get(schema.key.local_name())
                    .unwrap_or(&schema.default);
                write_own(&mut packer, &schema.kind, value).with_context(|| {
                    format!(
                        "invalid value for property `{}` of shader `{}`",
                        schema.key.local_name(),
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
#[path = "../../../tests/unit/plugin/shader/params_tests.rs"]
mod tests;
