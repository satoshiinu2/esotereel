use std::collections::BTreeMap;

use rkyv::{Archive, CheckBytes, bytecheck};

use crate::util::color::RgbaColor;

#[derive(
    Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub enum PropertyValue {
    Static(FieldValue),
    // 将来的に Keyframes(Vec<(TimelineTick, FieldValue)>) 等を追加予定
}

#[derive(
    Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[archive_attr(
    derive(CheckBytes),
    check_bytes(
        bound = "__C: rkyv::validation::ArchiveContext, <__C as rkyv::Fallible>::Error: std::error::Error"
    )
)]
#[archive(bound(
    serialize = "__S: rkyv::ser::Serializer + rkyv::ser::ScratchSpace",
    deserialize = "__D: rkyv::Fallible"
))]
pub enum FieldValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Enum(String),
    String(String),
    Color(RgbaColor),
    Array(
        #[omit_bounds]
        #[archive_attr(omit_bounds)]
        Vec<FieldValue>,
    ),

    Map(
        #[omit_bounds]
        #[archive_attr(omit_bounds)]
        BTreeMap<String, FieldValue>,
    ),
}
