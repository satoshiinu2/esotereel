use std::{
    collections::HashSet,
    panic::{AssertUnwindSafe, catch_unwind},
};

use esotereel_lib::{
    plugin::{
        NamespacedID,
        property::{PropertySchema, value::FieldTypeKind},
    },
    util::result::EsotereelError,
};

use crate::ffi::{
    array::FfiArray,
    field_value::CFieldValue,
    option::FfiOption,
    result::{FfiResult, FfiResultVoid},
    state::ClientStateHandle,
    stringview::{OwnedString, StringView},
};

#[repr(C)]
#[derive(Clone, Copy)]
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
#[derive(Clone, Copy)]
pub struct FfiPropertySchema {
    pub key: OwnedString,
    pub category: OwnedString,
    pub label: OwnedString,
    pub kind_type: SettingsFieldType,
    pub default_value: CFieldValue,
}

impl FfiPropertySchema {
    pub fn from_schema(schema: &PropertySchema) -> Self {
        let kind_type = match &schema.kind {
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

        let category_str = schema.category.join(" > ");
        let default_value = CFieldValue::wrap_ffi(&schema.default);

        Self {
            key: OwnedString::from_string(schema.key.full().to_owned()),
            category: OwnedString::from_string(category_str),
            label: OwnedString::from_string(schema.label.clone()),
            kind_type,
            default_value,
        }
    }
}

/// 個数取得+バッファ書き込みの2関数ペアを、FfiArrayを直接返す1関数に統合。
/// 呼び出し側(C++)はFfiArray::free_fnで解放する。
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

        let fields: Vec<FfiPropertySchema> = state
            .settings
            .schema
            .fields()
            .iter()
            .map(FfiPropertySchema::from_schema)
            .collect();

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
    key: StringView,
) -> CFieldValueResult {
    fn inner(
        ptr_state: *const ClientStateHandle,
        key: StringView,
    ) -> anyhow::Result<Option<CFieldValue>> {
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

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, key))) {
        Ok(r) => FfiResult::from_result(r.map(FfiOption::from)),
        Err(panic) => FfiResult::err_panic(panic),
    }
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

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, key, value))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

pub type FfiStringArrayResult = FfiResult<FfiArray<OwnedString>>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn settings_get_categories(
    ptr_state: *const ClientStateHandle,
) -> FfiStringArrayResult {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<FfiArray<OwnedString>> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state = state.lock().expect("mutex poisoned");

        let mut categories = HashSet::new();
        for field in state.settings.schema.fields() {
            for cat in &field.category {
                categories.insert(cat.clone());
            }
        }
        let mut cats: Vec<_> = categories.into_iter().collect();
        cats.sort();

        let owned: Vec<OwnedString> = cats.into_iter().map(OwnedString::from_string).collect();
        Ok(FfiArray::from_vec(owned))
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}
