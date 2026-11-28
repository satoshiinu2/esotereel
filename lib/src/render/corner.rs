//! 四隅オフセットによるクワッド変形（コーナーピン）の入力解決です。
//!
//! テキストのようにメッシュ化が重いクリップ向けに、矩形の四隅だけを
//! ずらす変形を提供します。変形そのものは CPU では行わず、std プラグイン
//! の compute シェーダ `std:corner_deform` をシェーダスタックの先頭に
//! 合成して `ComputeStackRunner` で実行します（`render::deform` 参照）。
//! このモジュールはクリッププロパティからオフセットを解決し、シェーダの
//! Params へ渡すスタックエントリへ変換する役割を持ちます。
//!
//! オフセットはコンポーネントが `corner-deformable` 能力で宣言する
//! グループ型プロパティに保持され、各隅はメンバー自身が宣言する
//! 能力（`corner-top-left` 等）から解決します。プロパティ ID や
//! メンバー名のハードコードはしません（AGENTS.md 第9条）。

use std::collections::HashMap;

use crate::{
    plugin::{
        NamespacedID,
        property::{
            ComponentCapability, PropertySchema,
            value::{FieldTypeKind, FieldValue},
        },
    },
    project::clip::ShaderStackEntry,
};

/// 四隅変形 compute シェーダの ID です。
///
/// std プラグインの `shaders/corner_deform.toml` で定義され、他の
/// シェーダと同じくプラグインローダー経由で解決します。
pub const CORNER_DEFORM_SHADER_ID: (&str, &str) = ("std", "corner_deform");

pub fn corner_deform_shader_id() -> Option<NamespacedID> {
    NamespacedID::new(CORNER_DEFORM_SHADER_ID.0, CORNER_DEFORM_SHADER_ID.1).ok()
}

/// 四隅のオフセット（クリップのローカル座標系、x/y の 2 成分）。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CornerOffsets {
    pub top_left: [f32; 2],
    pub top_right: [f32; 2],
    pub bottom_right: [f32; 2],
    pub bottom_left: [f32; 2],
}

impl CornerOffsets {
    /// `corner-deformable` 能力を持つプロパティスキーマとその値から解決します。
    ///
    /// 各隅はメンバーが宣言する能力から逆引きします。能力を宣言する
    /// メンバーが無い場合は宣言不備としてエラーにします。値が無い隅は
    /// スキーマのデフォルトを使います。値の形式が不正（2 成分の数値配列
    /// でない、非有限）な場合もエラーにし、呼び出し側が見える失敗として
    /// 扱えるようにします。
    pub fn from_property(schema: &PropertySchema, value: &FieldValue) -> Result<Self, String> {
        let FieldValue::Map(values) = value else {
            return Err("corner deform property value is not a group".into());
        };
        let corner = |capability: ComponentCapability| -> Result<[f32; 2], String> {
            let member = schema
                .member_key_by_capability(&capability)
                .ok_or_else(|| {
                    format!(
                        "corner deform property `{}` does not declare a `{}` member",
                        schema.key,
                        capability.as_str()
                    )
                })?;
            let FieldTypeKind::Group { fields } = &schema.kind else {
                return Err(format!(
                    "corner deform property `{}` is not a group",
                    schema.key
                ));
            };
            let field = fields
                .iter()
                .find(|f| f.key.local_name() == member)
                .ok_or_else(|| format!("corner member `{member}` missing from schema"))?;
            let raw = values.get(member).unwrap_or(&field.default);
            parse_offset(raw, member)
        };
        Ok(Self {
            top_left: corner(ComponentCapability::CornerTopLeft)?,
            top_right: corner(ComponentCapability::CornerTopRight)?,
            bottom_right: corner(ComponentCapability::CornerBottomRight)?,
            bottom_left: corner(ComponentCapability::CornerBottomLeft)?,
        })
    }

    pub fn is_identity(&self) -> bool {
        *self == Self::default()
    }

    /// シェーダスタックへ合成するエントリへ変換します。
    ///
    /// キーは `std:corner_deform` シェーダが宣言する固有プロパティの
    /// ローカル名で、`pack_params` が宣言順に Params へ詰めます。
    /// Params はスカラーしか詰められないため、各隅は x/y の
    /// 2 プロパティに分けて渡します。
    pub fn stack_entry(&self, shader_id: NamespacedID) -> ShaderStackEntry {
        let mut properties = HashMap::new();
        let mut insert = |key: &str, offset: [f32; 2]| {
            properties.insert(format!("{key}_x"), FieldValue::Float(offset[0] as f64));
            properties.insert(format!("{key}_y"), FieldValue::Float(offset[1] as f64));
        };
        insert("top_left", self.top_left);
        insert("top_right", self.top_right);
        insert("bottom_right", self.bottom_right);
        insert("bottom_left", self.bottom_left);
        ShaderStackEntry {
            shader_id,
            properties,
        }
    }
}

fn parse_offset(value: &FieldValue, member: &str) -> Result<[f32; 2], String> {
    let FieldValue::Array(items) = value else {
        return Err(format!("corner member `{member}` is not an array"));
    };
    if items.len() != 2 {
        return Err(format!(
            "corner member `{member}` must have exactly 2 components, got {}",
            items.len()
        ));
    }
    let mut offset = [0.0f32; 2];
    for (out, item) in offset.iter_mut().zip(items) {
        let number = match item {
            FieldValue::Float(f) => *f,
            FieldValue::Int(i) => *i as f64,
            _ => return Err(format!("corner member `{member}` must contain numbers")),
        };
        if !number.is_finite() {
            return Err(format!("corner member `{member}` must be finite"));
        }
        *out = number as f32;
    }
    Ok(offset)
}

#[cfg(test)]
#[path = "../../tests/unit/render/corner_tests.rs"]
mod tests;
