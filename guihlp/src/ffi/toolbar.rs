use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::plugin::toolbar::ToolbarButtonSpec;

use crate::{
    IntoWrapperError, WrapperErrorCode,
    ffi::{
        log_if_panicked,
        state::ClientStateHandle,
        stringview::{OwnedString, StringView},
    },
};

#[repr(C)]
pub struct FfiToolbarButton {
    pub id: OwnedString,
    pub label: OwnedString,
    pub tooltip: OwnedString,
    /// アイコン未指定なら空文字列。
    pub icon: OwnedString,
    pub action: OwnedString,
}

impl FfiToolbarButton {
    fn from_spec(spec: &ToolbarButtonSpec) -> Self {
        Self {
            id: OwnedString::from_string(spec.id.clone()),
            label: OwnedString::from_string(spec.label.clone()),
            tooltip: OwnedString::from_string(spec.tooltip.clone()),
            icon: OwnedString::from_string(spec.icon.clone().unwrap_or_default()),
            action: OwnedString::from_string(spec.action.func_name.clone()),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn toolbar_get_buttons_count(
    ptr_state: *const ClientStateHandle,
    target: StringView,
) -> i32 {
    if ptr_state.is_null() {
        return -1;
    }

    // Arc::into_raw 由来のポインタから、参照カウントを増やして
    // 独立した Arc クローンを作る（元のポインタは消費しない）
    let state = ClientStateHandle::from_ptr(ptr_state);

    let target_str = match target.as_str() {
        Some(s) => s,
        None => return -1,
    };
    let state = state.lock().expect("mutex poisoned");

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<i32, IntoWrapperError> {
        Ok(state.toolbar.get_layout(target_str).len() as i32)
    }));

    match result {
        Ok(Ok(count)) => count,
        Ok(Err(e)) => {
            e.set_last_err_msg();
            -1
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<i32, _>(panic), "toolbar_get_buttons_count");
            if let Some(msg) = msg {
                WrapperErrorCode::set_last_err_msg(Some(&msg));
            }
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn toolbar_get_buttons(
    ptr_state: *const ClientStateHandle,
    target: StringView,
    output: *mut FfiToolbarButton,
    output_len: usize,
) -> WrapperErrorCode {
    if ptr_state.is_null() || output.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    // Arc::into_raw 由来のポインタから、参照カウントを増やして
    // 独立した Arc クローンを作る（元のポインタは消費しない）
    let state = ClientStateHandle::from_ptr(ptr_state);

    let target_str = match target.as_str() {
        Some(s) => s,
        None => return WrapperErrorCode::invalid_string_error(),
    };
    let state = state.lock().expect("mutex poisoned");

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let ids = state.toolbar.get_layout(target_str);

        let specs: Vec<&ToolbarButtonSpec> = ids
            .iter()
            .filter_map(|id| state.toolbar.registry.buttons().find(|b| &b.id == id))
            .collect();

        let output_slice = unsafe { std::slice::from_raw_parts_mut(output, output_len) };
        for (i, spec) in specs.iter().enumerate() {
            if i >= output_len {
                break;
            }
            output_slice[i] = FfiToolbarButton::from_spec(spec);
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
            let msg = log_if_panicked(Err::<(), _>(panic), "toolbar_get_buttons");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn toolbar_set_layout(
    ptr_state: *const ClientStateHandle,
    target: StringView,
    ids_toml_array: StringView,
) -> WrapperErrorCode {
    if ptr_state.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    let state = ClientStateHandle::from_ptr(ptr_state);
    let target_str = match target.as_str() {
        Some(s) => s,
        None => return WrapperErrorCode::invalid_string_error(),
    };
    let ids_str = match ids_toml_array.as_str() {
        Some(s) => s,
        None => return WrapperErrorCode::invalid_string_error(),
    };

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let parsed_value: toml::Value = toml::from_str(ids_str).map_err(|e| {
            IntoWrapperError::Error(Some(format!("Failed to parse ids: {}", e).into()))
        })?;
        let ids: Vec<String> = parsed_value.try_into().map_err(|e| {
            IntoWrapperError::Error(Some(
                format!("ids must be an array of strings: {}", e).into(),
            ))
        })?;

        let mut state = state.lock().expect("mutex poisoned");

        state.toolbar.set_layout(target_str.to_string(), ids);

        Ok(())
    }));

    match result {
        Ok(Ok(())) => WrapperErrorCode::ok(),
        Ok(Err(e)) => {
            e.set_last_err_msg();
            e.into()
        }
        Err(panic) => {
            let msg = log_if_panicked(Err::<(), _>(panic), "toolbar_set_layout");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn toolbar_handle_action(
    ptr_state: *const ClientStateHandle,
    button_id: StringView,
) -> WrapperErrorCode {
    if ptr_state.is_null() {
        return WrapperErrorCode::null_ptr();
    }

    // Arc::into_raw 由来のポインタから、参照カウントを増やして
    // 独立した Arc クローンを作る（元のポインタは消費しない）
    let state = ClientStateHandle::from_ptr(ptr_state);

    let button_id = match button_id.as_str() {
        Some(s) => s,
        None => return WrapperErrorCode::invalid_string_error(),
    };

    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), IntoWrapperError> {
        let state = state.lock().expect("mutex poisoned");

        let (plugin_id, button) =
            state
                .toolbar
                .registry
                .get_button_by(button_id)
                .ok_or(IntoWrapperError::Error(Some(
                    "ToolbarButton not found".into(),
                )))?;

        state
            .scripts
            .call::<()>(plugin_id, &button.action.func_name, ())
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
            let msg = log_if_panicked(Err::<(), _>(panic), "toolbar_handle_action");
            WrapperErrorCode::error_from_option(msg.as_deref())
        }
    }
}
