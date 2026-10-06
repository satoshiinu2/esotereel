use std::panic::{AssertUnwindSafe, catch_unwind};

use esotereel_lib::util::result::EsotereelError;

use crate::{
    ffi::{
        array::FfiArray,
        field_value::CFieldValue,
        option::FfiOption,
        result::{FfiResult, FfiResultVoid},
        state::ClientStateHandle,
        stringview::{FfiOwnedString, FfiStringView},
    },
    settings::FfiPropertySchema,
};

pub type FfiPropertySchemaArrayResult = FfiResult<FfiArray<FfiPropertySchema>>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_all_fields(
    ptr_state: *const ClientStateHandle,
) -> FfiPropertySchemaArrayResult {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<FfiArray<FfiPropertySchema>> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state = state.lock().expect("mutex poisoned");

        let fields = crate::settings::get_all_fields(&state.settings);

        Ok(FfiArray::from_vec(fields))
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

pub type CFieldValueResult = FfiResult<FfiOption<CFieldValue>>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_value(
    ptr_state: *const ClientStateHandle,
    key: FfiStringView,
) -> CFieldValueResult {
    fn inner(
        ptr_state: *const ClientStateHandle,
        key: FfiStringView,
    ) -> anyhow::Result<Option<CFieldValue>> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }
        let state = ClientStateHandle::from_ptr(ptr_state);
        let key_str = key.as_str()?;
        let state = state.lock().expect("mutex poisoned");
        let value = crate::settings::get_value(&state.settings, key_str)?;
        Ok(value)
    }

    FfiResult::from_panic_result_result_option(
        catch_unwind(AssertUnwindSafe(|| inner(ptr_state, key))),
        FfiOption::from,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_set_value(
    ptr_state: *const ClientStateHandle,
    key: FfiStringView,
    value: CFieldValue,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        key: FfiStringView,
        value: CFieldValue,
    ) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }
        let state = ClientStateHandle::from_ptr(ptr_state);
        let key_str = key.as_str()?;
        let is_core_setting = key_str.starts_with("core:");
        let mut state = state.lock().expect("mutex poisoned");
        crate::settings::set_value(&mut state.settings, key_str, value)?;
        state.apply_logging_settings();

        if is_core_setting {
            state.save_prebootstrap_settings()
        } else {
            let settings_path = state.dir.client_settings_path()?;
            state.settings.save_to_path(&settings_path)
        }
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, key, value))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

pub type FfiStringArrayResult = FfiResult<FfiArray<FfiOwnedString>>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_categories(
    ptr_state: *const ClientStateHandle,
) -> FfiStringArrayResult {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<FfiArray<FfiOwnedString>> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state = state.lock().expect("mutex poisoned");

        let cats = crate::settings::get_categories(&state.settings);

        let owned: Vec<FfiOwnedString> =
            cats.into_iter().map(FfiOwnedString::from_string).collect();
        Ok(FfiArray::from_vec(owned))
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}
