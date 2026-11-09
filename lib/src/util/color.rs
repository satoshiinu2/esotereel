use rkyv::{Archive, CheckBytes, bytecheck};

/// RGBA カラー値です。
///
/// 各チャネルは `0.0..=1.0` の `f32` で表現され、`rkyv` / `serde` による
/// シリアライズおよび `bytemuck` による GPU バッファ転送をサポートします。
#[derive(
    Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
)]
#[archive_attr(derive(CheckBytes))]
#[repr(C)]
pub struct RgbaColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
