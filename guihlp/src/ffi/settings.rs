use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use esotereel_lib::{
    plugin::{
        NamespacedID,
        property::{PropertySchema, value::FieldTypeKind},
    },
    util::result::EsotereelError,
};

use crate::{
    IntoWrapperError, WrapperErrorCode,
    ffi::{
        field_value::CFieldValue,
        log_if_panicked,
        option::FfiOption,
        result::{FfiResult, FfiResultVoid},
        state::ClientStateHandle,
        stringview::{OwnedString, StringView},
    },
    ffi_fn,
};

#[repr(C)]
pub enum SettingsFieldType {
    Bool,
    Int,
    Float,
    Enum,
    String,
    FilePath,
    Color,
    Array,
    Map,
}

#[repr(C)]
pub struct SettingsField {
    pub key: OwnedString,
    pub category: OwnedString,
    pub label: OwnedString,
    pub kind_type: SettingsFieldType,
    pub default_value: CFieldValue,
}

impl SettingsField {
    fn from_field(field: &PropertySchema) -> Self {
        let kind_type = match &field.kind {
            FieldTypeKind::Bool => SettingsFieldType::Bool,
            FieldTypeKind::Int { .. } => SettingsFieldType::Int,
            FieldTypeKind::Float { .. } => SettingsFieldType::Float,
            FieldTypeKind::Enum { .. } => SettingsFieldType::Enum,
            FieldTypeKind::String => SettingsFieldType::String,
            FieldTypeKind::FilePath { .. } => SettingsFieldType::FilePath,
            FieldTypeKind::Color => SettingsFieldType::Color,
            FieldTypeKind::Array { .. } => SettingsFieldType::Array,
            FieldTypeKind::Map { .. } => SettingsFieldType::Map,
        };

        let category_str = field.category.join(" > ");
        let default_value = CFieldValue::wrap_ffi(&field.default);

        Self {
            key: OwnedString::from_string(field.key.full().to_owned()),
            category: OwnedString::from_string(category_str),
            label: OwnedString::from_string(field.label.clone()),
            kind_type,
            default_value,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_all_fields_count(ptr_state: *const ClientStateHandle) -> i32 {
    if ptr_state.is_null() {
        return -1;
    }

    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<i32, IntoWrapperError> {
        Ok(state.settings.schema.fields().len() as i32)
    }));

    match result {
        Ok(Ok(count)) => count,
        Ok(Err(e)) => {
            e.set_last_err_msg();
            -1
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<i32, _>(panic), "settings_get_all_fields_count");
            if let Some(msg) = msg {
                WrapperErrorCode::set_last_err_msg(Some(&msg));
            }
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_all_fields(
    ptr_state: *const ClientStateHandle,
    output: *mut SettingsField,
    output_len: usize,
) -> WrapperErrorCode {
    if ptr_state.is_null() || output.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let fields = state.settings.schema.fields().to_vec();

        let output_slice = unsafe { std::slice::from_raw_parts_mut(output, output_len) };

        for (i, field) in fields.iter().enumerate() {
            if i >= output_len {
                break;
            }
            output_slice[i] = SettingsField::from_field(field);
        }

        Ok(())
    }));

    match result {
        Ok(Ok(())) => WrapperErrorCode::ok(),
        Ok(Err(e)) => {
            e.set_last_err_msg();
            e.into()
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "settings_get_all_fields");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}

pub type CFieldValueResult = FfiResult<FfiOption<CFieldValue>>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_value(
    ptr_state: *const ClientStateHandle,
    key: StringView,
) -> CFieldValueResult {
    fn inner(
        ptr_state: *const ClientStateHandle,
        key: StringView,
    ) -> anyhow::Result<Option<CFieldValue>> {
        {
            if ptr_state.is_null() {
                return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
            }
            let state = ClientStateHandle::from_ptr(ptr_state);
            let key_str = key.as_str()?;
            let key = NamespacedID::parse(key_str)?;
            let state = state.lock().expect("mutex poisoned");
            let value = state
                .settings
                .get_value(&key)
                .map(|value| CFieldValue::wrap_ffi(value));
            Ok(value)
        }
    }
    FfiResult::from_result(inner(ptr_state, key).map(FfiOption::from))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_set_value(
    ptr_state: *const ClientStateHandle,
    key: StringView,
    value: CFieldValue,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        key: StringView,
        value: CFieldValue,
    ) -> anyhow::Result<()> {
        {
            if ptr_state.is_null() {
                return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
            }
            let state = ClientStateHandle::from_ptr(ptr_state);
            let key_str = key.as_str()?;
            let key = NamespacedID::parse(key_str)?;
            let mut state = state.lock().expect("mutex poisoned");
            let field_value = value.unwrap_ffi()?;
            state.settings.set_value(key, field_value)
        }
    }
    FfiResultVoid::from_result(inner(ptr_state, key, value))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_categories_count(ptr_state: *const ClientStateHandle) -> i32 {
    if ptr_state.is_null() {
        return -1;
    }

    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<i32, IntoWrapperError> {
        let mut categories = std::collections::HashSet::new();
        for field in state.settings.schema.fields() {
            for cat in &field.category {
                categories.insert(cat.clone());
            }
        }
        let mut cats: Vec<_> = categories.into_iter().collect();
        cats.sort();
        Ok(cats.len() as i32)
    }));

    match result {
        Ok(Ok(count)) => count,
        Ok(Err(e)) => {
            e.set_last_err_msg();
            -1
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<i32, _>(panic), "settings_get_categories_count");
            if let Some(msg) = msg {
                WrapperErrorCode::set_last_err_msg(Some(&msg));
            }
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_categories(
    ptr_state: *const ClientStateHandle,
    output: *mut OwnedString,
    output_len: usize,
) -> WrapperErrorCode {
    if ptr_state.is_null() || output.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    // Arc::into_raw 由来のポインタから、参照カウントを増やして
    // 独立した Arc クローンを作る（元のポインタは消費しない）
    let state = ClientStateHandle::from_ptr(ptr_state);
    let state = state.lock().expect("mutex poisoned");

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let mut categories = std::collections::HashSet::new();
        for field in state.settings.schema.fields() {
            for cat in &field.category {
                categories.insert(cat.clone());
            }
        }
        let mut cats: Vec<_> = categories.into_iter().collect();
        cats.sort();

        let output_slice = unsafe { std::slice::from_raw_parts_mut(output, output_len) };

        for (i, cat) in cats.iter().enumerate() {
            if i >= output_len {
                break;
            }
            output_slice[i] = OwnedString::from_string(cat.clone());
        }

        Ok(())
    }));

    match result {
        Ok(Ok(())) => WrapperErrorCode::ok(),
        Ok(Err(e)) => {
            e.set_last_err_msg();
            e.into()
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "settings_get_categories");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}
