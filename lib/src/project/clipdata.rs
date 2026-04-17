use rkyv::{Archive, CheckBytes, Deserialize, Serialize, bytecheck};

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[archive_attr(derive(CheckBytes, Ord, PartialOrd, Eq, PartialEq))]
#[repr(u8)]
pub enum ClipData {
    Dummy,
}
