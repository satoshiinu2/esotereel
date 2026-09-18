pub mod array;
pub mod option;
pub mod result;
pub mod result_option;

// 必要な型を再エクスポート
pub use crate::ffi::result::FfiResult;
pub use crate::ffi::option::FfiOption;
pub use crate::ffi::array::FfiArray;
pub use crate::ffi::field_value::CFieldValue;
pub use esotereel_lib::project::ids::{TimelineId, ClipId};
