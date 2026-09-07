use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::setting::{FieldSchema, FieldTypeKind, SettingsStore};

use crate::{
    IntoWrapperError, WrapperErrorCode,
    ffi::{
        log_if_panicked,
        stringview::{OwnedString, StringView},
    },
    network::ClientNetworkHandler,
};

#[repr(C)]
pub enum SettingsFieldType {
    Bool = 0,
    Int = 1,
    Float = 2,
    Enum = 3,
    String = 4,
    Color = 5,
    Array = 6,
    Map = 7,
}

#[repr(C)]
pub struct SettingsField {
    pub key: OwnedString,
    pub category: OwnedString,
    pub label: OwnedString,
    pub kind_type: SettingsFieldType,
    pub default_value: OwnedString,
}

impl SettingsField {
    fn from_field(field: &FieldSchema) -> Self {
        let kind_type = match &field.kind {
            FieldTypeKind::Bool => SettingsFieldType::Bool,
            FieldTypeKind::Int { .. } => SettingsFieldType::Int,
            FieldTypeKind::Float { .. } => SettingsFieldType::Float,
            FieldTypeKind::Enum { .. } => SettingsFieldType::Enum,
            FieldTypeKind::String => SettingsFieldType::String,
            FieldTypeKind::Color => SettingsFieldType::Color,
            FieldTypeKind::Array { .. } => SettingsFieldType::Array,
            FieldTypeKind::Map { .. } => SettingsFieldType::Map,
        };

        let category_str = field.category.join(" > ");
        let default_str = field.default.to_string();

        Self {
            key: OwnedString::from_string(field.key.clone()),
            category: OwnedString::from_string(category_str),
            label: OwnedString::from_string(field.label.clone()),
            kind_type,
            default_value: OwnedString::from_string(default_str),
        }
    }
}

// slop
#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_initialize(
    ptr_network: *const ClientNetworkHandler,
    schema: StringView,
) -> WrapperErrorCode {
    if ptr_network.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = unsafe { &*ptr_network };
    let schema_str = match schema.as_str() {
        Some(s) => s,
        None => return WrapperErrorCode::invalid_string_error(),
    };

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let mut app_state = network.app_state.lock().expect("mutex poisoned");

        // Parse schema and register fields
        let fields = FieldSchema::parse_toml(schema_str, "builtin")
            .map_err(|e| IntoWrapperError::Error(Some(e.to_string().into())))?;

        for field in fields {
            app_state.settings.schema.register(field);
        }

        // Add missing defaults
        app_state.settings.add_missing_from_schema();

        Ok(())
    }));

    match result {
        Ok(Ok(())) => WrapperErrorCode::ok(),
        Ok(Err(e)) => {
            e.set_last_err_msg();
            e.into()
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "settings_initialize");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_all_fields_count(
    ptr_network: *const ClientNetworkHandler,
) -> i32 {
    if ptr_network.is_null() {
        return -1;
    }

    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<i32, IntoWrapperError> {
        Ok(app_state.settings.schema.fields().len() as i32)
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
    ptr_network: *const ClientNetworkHandler,
    output: *mut SettingsField,
    output_len: usize,
) -> WrapperErrorCode {
    if ptr_network.is_null() || output.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let fields = app_state.settings.schema.fields().to_vec();

        let output_slice = std::slice::from_raw_parts_mut(output, output_len);

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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_value(
    ptr_network: *const ClientNetworkHandler,
    key: StringView,
    output: *mut OwnedString,
) -> WrapperErrorCode {
    if ptr_network.is_null() || output.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = unsafe { &*ptr_network };
    let key_str = match key.as_str() {
        Some(s) => s,
        None => return WrapperErrorCode::invalid_string_error(),
    };

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let app_state = network.app_state.lock().expect("mutex poisoned");

        let value = app_state.settings.get_value(key_str);

        match value {
            Some(v) => {
                let value_str = v.to_string();
                *output = OwnedString::from_string(value_str);
            }
            None => {
                *output = OwnedString::zero();
            }
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
            let msg = log_if_panicked(Err::<(), _>(panic), "settings_get_value");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_set_value(
    ptr_network: *const ClientNetworkHandler,
    key: StringView,
    value: StringView,
) -> WrapperErrorCode {
    if ptr_network.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = unsafe { &*ptr_network };
    let key_str = match key.as_str() {
        Some(s) => s,
        None => return WrapperErrorCode::invalid_string_error(),
    };

    let value_str = match value.as_str() {
        Some(s) => s,
        None => return WrapperErrorCode::invalid_string_error(),
    };

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let parsed_value: toml::Value = toml::from_str(value_str).map_err(|e| {
            IntoWrapperError::Error(Some(format!("Failed to parse value: {}", e).into()))
        })?;

        let mut app_state = network.app_state.lock().expect("mutex poisoned");

        app_state
            .settings
            .set_value(key_str.to_string(), parsed_value)
            .map_err(|e| IntoWrapperError::Error(Some(e.to_string().into())))?;

        Ok(())
    }));

    match result {
        Ok(Ok(())) => WrapperErrorCode::ok(),
        Ok(Err(e)) => {
            e.set_last_err_msg();
            e.into()
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "settings_set_value");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_categories_count(
    ptr_network: *const ClientNetworkHandler,
) -> i32 {
    if ptr_network.is_null() {
        return -1;
    }

    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<i32, IntoWrapperError> {
        let mut categories = std::collections::HashSet::new();
        for field in app_state.settings.schema.fields() {
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
    ptr_network: *const ClientNetworkHandler,
    output: *mut OwnedString,
    output_len: usize,
) -> WrapperErrorCode {
    if ptr_network.is_null() || output.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let network = unsafe { &*ptr_network };
    let app_state = network.app_state.lock().expect("mutex poisoned");

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let mut categories = std::collections::HashSet::new();
        for field in app_state.settings.schema.fields() {
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
