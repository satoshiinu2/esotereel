use std::panic::{catch_unwind, AssertUnwindSafe};

use esotereel_lib::util::result::{EsotereelError, LockExt};

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
        let settings = state.settings_arc();
        let settings = settings.read_or_err()?;

        let fields = crate::settings::get_all_fields(&settings);

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
        let settings = state.settings_arc();
        let settings = settings.read_or_err()?;
        let value = crate::settings::get_value(&settings, key_str)?;
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
        let settings = state.settings_arc();
        {
            let mut settings = settings.write_or_err()?;
            crate::settings::set_value(&mut settings, key_str, value)?;
        } // write ガードはここで解放（apply_logging_settings が read を取るため）
        state.apply_logging_settings();

        if is_core_setting {
            state.save_prebootstrap_settings()
        } else {
            let settings_path = state.dir.client_settings_path()?;
            let settings = settings.read_or_err()?;
            settings.save_to_path(&settings_path)
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
        let settings = state.settings_arc();
        let settings = settings.read_or_err()?;

        let cats = crate::settings::get_categories(&settings);

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
