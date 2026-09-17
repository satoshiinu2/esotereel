use rkyv::{Archive, CheckBytes, bytecheck};

use crate::plugin::property::value::FieldValue;

#[derive(
    Archive, rkyv::Deserialize, rkyv::Serialize, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[archive_attr(derive(CheckBytes))]
pub enum PropertyValue {
    Static(FieldValue),
    // 将来的に Keyframes(Vec<(TimelineTick, FieldValue)>) 等を追加予定
}
