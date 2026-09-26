use esotereel_lib::{plugin::property::value::FieldValue, util::color::RgbaColor};

use crate::ffi::stringview::{FfiOwnedString, FfiOwnedStringArray};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CFieldValueTag {
    Bool,
    Int,
    Float,
    Enum,
    String,
    Path,
    Color,
    Array,
    Map,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CFieldValue {
    pub tag: CFieldValueTag,
    pub data: CFieldValueData,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union CFieldValueData {
    pub bool_value: u8,
    pub int_value: i64,
    pub float_value: f64,

    pub enum_value: FfiOwnedString,
    pub string_value: FfiOwnedString,

    pub path_value: FfiOwnedStringArray,

    pub color_value: RgbaColor,

    pub array_value: CFieldValueArray,
    pub map_value: CFieldValueMap,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CFieldValueArray {
    pub ptr: *mut CFieldValue,
    pub len: usize,
    pub capacity: usize,
}

impl CFieldValueArray {
    unsafe fn free(self) {
        if self.ptr.is_null() {
            return;
        }

        let values = unsafe { Vec::from_raw_parts(self.ptr, self.len, self.capacity) };

        for value in values {
            unsafe {
                value.free();
            }
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CFieldValueMapEntry {
    pub key: FfiOwnedString,
    pub value: CFieldValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CFieldValueMap {
    pub ptr: *mut CFieldValueMapEntry,
    pub len: usize,
    pub capacity: usize,
}

impl CFieldValueMap {
    unsafe fn free(self) {
        if self.ptr.is_null() {
            return;
        }

        let entries = unsafe { Vec::from_raw_parts(self.ptr, self.len, self.capacity) };

        for entry in entries {
            unsafe {
                entry.key.free();
                entry.value.free();
            }
        }
    }
}

impl CFieldValue {
    pub fn wrap_ffi(value: &FieldValue) -> CFieldValue {
        match value {
            FieldValue::Bool(v) => CFieldValue {
                tag: CFieldValueTag::Bool,
                data: CFieldValueData {
                    bool_value: *v as u8,
                },
            },

            FieldValue::Int(v) => CFieldValue {
                tag: CFieldValueTag::Int,
                data: CFieldValueData { int_value: *v },
            },

            FieldValue::Float(v) => CFieldValue {
                tag: CFieldValueTag::Float,
                data: CFieldValueData { float_value: *v },
            },

            FieldValue::Enum(v) => CFieldValue {
                tag: CFieldValueTag::Enum,
                data: CFieldValueData {
                    enum_value: FfiOwnedString::from_string(v.clone()),
                },
            },

            FieldValue::String(v) => CFieldValue {
                tag: CFieldValueTag::String,
                data: CFieldValueData {
                    string_value: FfiOwnedString::from_string(v.clone()),
                },
            },

            FieldValue::Path(paths) => {
                let values = paths
                    .iter()
                    .map(|path| FfiOwnedString::from_string(path.to_string_lossy().into_owned()))
                    .collect();

                CFieldValue {
                    tag: CFieldValueTag::Path,
                    data: CFieldValueData {
                        path_value: FfiOwnedStringArray::from_vec(values),
                    },
                }
            }

            FieldValue::Color(v) => CFieldValue {
                tag: CFieldValueTag::Color,
                data: CFieldValueData {
                    color_value: RgbaColor {
                        r: v.r,
                        g: v.g,
                        b: v.b,
                        a: v.a,
                    },
                },
            },

            FieldValue::Array(values) => {
                let mut values: Vec<CFieldValue> =
                    values.iter().map(CFieldValue::wrap_ffi).collect();

                let ptr = values.as_mut_ptr();
                let len = values.len();
                let cap = values.capacity();

                std::mem::forget(values);

                CFieldValue {
                    tag: CFieldValueTag::Array,
                    data: CFieldValueData {
                        array_value: CFieldValueArray {
                            ptr,
                            len,
                            capacity: cap,
                        },
                    },
                }
            }

            FieldValue::Map(values) => {
                let mut entries: Vec<CFieldValueMapEntry> = values
                    .iter()
                    .map(|(key, value)| CFieldValueMapEntry {
                        key: FfiOwnedString::from_string(key.clone()),
                        value: CFieldValue::wrap_ffi(value),
                    })
                    .collect();

                let ptr = entries.as_mut_ptr();
                let len = entries.len();
                let cap = entries.capacity();

                std::mem::forget(entries);

                CFieldValue {
                    tag: CFieldValueTag::Map,
                    data: CFieldValueData {
                        map_value: CFieldValueMap {
                            ptr,
                            len,
                            capacity: cap,
                        },
                    },
                }
            }
        }
    }

    pub fn unwrap_ffi(&self) -> anyhow::Result<FieldValue> {
        unsafe {
            match self.tag {
                CFieldValueTag::Bool => Ok(FieldValue::Bool(self.data.bool_value != 0)),

                CFieldValueTag::Int => Ok(FieldValue::Int(self.data.int_value)),

                CFieldValueTag::Float => Ok(FieldValue::Float(self.data.float_value)),

                CFieldValueTag::Enum => {
                    let str = self.data.enum_value.as_str()?;
                    Ok(FieldValue::Enum(str.to_owned()))
                }

                CFieldValueTag::String => {
                    let str = self.data.string_value.as_str()?;
                    Ok(FieldValue::String(str.to_owned()))
                }

                CFieldValueTag::Path => {
                    let array = self.data.path_value;

                    if array.ptr.is_null() {
                        if array.len != 0 {
                            anyhow::bail!("Path array has null pointer with non-zero length")
                        }

                        return Ok(FieldValue::Path(Vec::new()));
                    }

                    let values = std::slice::from_raw_parts(array.ptr, array.len);

                    let mut paths = Vec::with_capacity(values.len());

                    for value in values {
                        let path = value.as_str()?;
                        paths.push(std::path::PathBuf::from(path));
                    }

                    Ok(FieldValue::Path(paths))
                }

                CFieldValueTag::Color => Ok(FieldValue::Color(self.data.color_value)),

                CFieldValueTag::Array => {
                    let array = self.data.array_value;

                    if array.ptr.is_null() {
                        if array.len != 0 {
                            anyhow::bail!("CFieldValue array has null pointer with non-zero length")
                        }

                        return Ok(FieldValue::Array(Vec::new()));
                    }

                    let values = std::slice::from_raw_parts(array.ptr, array.len);

                    let mut result = Vec::with_capacity(values.len());

                    for value in values {
                        result.push(value.unwrap_ffi()?);
                    }

                    Ok(FieldValue::Array(result))
                }

                CFieldValueTag::Map => {
                    let map = self.data.map_value;

                    if map.ptr.is_null() {
                        if map.len != 0 {
                            anyhow::bail!("CFieldValue map has null pointer with non-zero length")
                        }

                        return Ok(FieldValue::Map(Default::default()));
                    }

                    let entries = std::slice::from_raw_parts(map.ptr, map.len);

                    let mut result: std::collections::BTreeMap<String, FieldValue> =
                        Default::default();

                    for entry in entries {
                        let key = entry.key.as_string_lossy().to_string();
                        let value = entry.value.unwrap_ffi()?;

                        result.insert(key, value);
                    }

                    Ok(FieldValue::Map(result))
                }
            }
        }
    }

    unsafe fn free(self) {
        unsafe {
            match self.tag {
                CFieldValueTag::Bool
                | CFieldValueTag::Int
                | CFieldValueTag::Float
                | CFieldValueTag::Color => {}

                CFieldValueTag::Enum => {
                    self.data.enum_value.free();
                }

                CFieldValueTag::String => {
                    self.data.string_value.free();
                }

                CFieldValueTag::Path => {
                    self.data.path_value.free();
                }

                CFieldValueTag::Array => {
                    self.data.array_value.free();
                }

                CFieldValueTag::Map => {
                    self.data.map_value.free();
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn esotereel_field_value_export(value: *const FieldValue) -> *mut CFieldValue {
    if value.is_null() {
        return std::ptr::null_mut();
    }

    let value = unsafe { &*value };

    Box::into_raw(Box::new(CFieldValue::wrap_ffi(value)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn esotereel_field_value_free(value: *mut CFieldValue) {
    if value.is_null() {
        return;
    }

    unsafe {
        let value = Box::from_raw(value);
        (*value).free();
    }
}
