use std::{
    collections::HashSet,
    panic::{AssertUnwindSafe, catch_unwind},
};

use esotereel_lib::{
    plugin::{NamespacedID, property::PropertySchema},
    project::{
        Clip,
        clip::ClipBindingValue,
        command::CommandRequest,
        ids::{ClipId, TimelineId},
    },
    util::result::EsotereelError,
};

use crate::ffi::{
    array::FfiArray,
    commands::CommandQueue,
    field_value::CFieldValue,
    option::FfiOption,
    result::{FfiResult, FfiResultVoid},
    state::ClientStateHandle,
    stringview::{OwnedString, StringView},
};

pub use crate::ffi::settings::{FfiPropertySchema, SettingsFieldType as PropertyFieldType};

// ---- ClipBindingValueのC表現 ----
// 今はStaticのみ。将来Keyframesが増えたらタグを1つ足すだけで拡張できる形。

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CClipBindingValueTag {
    Static,
    // 将来: Keyframes,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union CClipBindingValueData {
    pub static_value: CFieldValue,
    // 将来: pub keyframes: CClipKeyframeArray,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CClipBindingValue {
    pub tag: CClipBindingValueTag,
    pub data: CClipBindingValueData,
}

impl CClipBindingValue {
    pub fn wrap_ffi(value: &ClipBindingValue) -> Self {
        match value {
            ClipBindingValue::Static(fv) => CClipBindingValue {
                tag: CClipBindingValueTag::Static,
                data: CClipBindingValueData {
                    static_value: CFieldValue::wrap_ffi(fv),
                },
            },
        }
    }

    pub fn unwrap_ffi(&self) -> anyhow::Result<ClipBindingValue> {
        unsafe {
            match self.tag {
                CClipBindingValueTag::Static => Ok(ClipBindingValue::Static(
                    self.data.static_value.unwrap_ffi()?,
                )),
            }
        }
    }
}

pub type CClipBindingValueResult = FfiResult<FfiOption<CClipBindingValue>>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clip_get_property_value(
    ptr: *const Clip,
    key: StringView,
) -> CClipBindingValueResult {
    fn inner(ptr: *const Clip, key: StringView) -> anyhow::Result<Option<CClipBindingValue>> {
        if ptr.is_null() {
            return Err(EsotereelError::NullPointer("ptr".to_string()).into());
        }

        let key_str = key.as_str()?;
        let key = NamespacedID::parse(key_str)?;

        let clip = unsafe { &*ptr };
        Ok(clip.properties.get(&key).map(CClipBindingValue::wrap_ffi))
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr, key))) {
        Ok(r) => FfiResult::from_result(r.map(FfiOption::from)),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

pub type FfiPropertySchemaArrayResult = FfiResult<FfiArray<FfiPropertySchema>>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clip_get_all_fields(
    ptr_state: *const ClientStateHandle,
    ptr_clip: *const Clip,
) -> FfiPropertySchemaArrayResult {
    fn inner(
        ptr_state: *const ClientStateHandle,
        ptr_clip: *const Clip,
    ) -> anyhow::Result<FfiArray<FfiPropertySchema>> {
        if ptr_state.is_null() || ptr_clip.is_null() {
            return Err(EsotereelError::NullPointer("ptr_clip".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state = state.lock().expect("mutex poisoned");
        let clip = unsafe { &*ptr_clip };

        let plugin_loader = state.plugin_loader.read().expect("mutex poisoned");
        let schema = plugin_loader
            .get_clip_properties(&clip.kind_id)
            .ok_or(EsotereelError::ClipKindNotFound(clip.kind_id.clone()))?;

        let fields: Vec<FfiPropertySchema> =
            schema.iter().map(FfiPropertySchema::from_schema).collect();

        Ok(FfiArray::from_vec(fields))
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, ptr_clip))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

pub type FfiStringArrayResult = FfiResult<FfiArray<OwnedString>>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clip_get_categories(
    ptr_state: *const ClientStateHandle,
    ptr: *const Clip,
) -> FfiStringArrayResult {
    fn inner(
        ptr_state: *const ClientStateHandle,
        ptr: *const Clip,
    ) -> anyhow::Result<FfiArray<OwnedString>> {
        if ptr_state.is_null() || ptr.is_null() {
            return Err(EsotereelError::NullPointer("ptr".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let state = state.lock().expect("mutex poisoned");
        let clip = unsafe { &*ptr };

        let plugin_loader = state.plugin_loader.read().expect("mutex poisoned");
        let schema = plugin_loader
            .get_clip_properties(&clip.kind_id)
            .ok_or(EsotereelError::ClipKindNotFound(clip.kind_id.clone()))?;

        let mut categories = HashSet::new();
        for field in schema {
            for cat in &field.category {
                categories.insert(cat.clone());
            }
        }
        let mut cats: Vec<_> = categories.into_iter().collect();
        cats.sort();

        let owned: Vec<OwnedString> = cats.into_iter().map(OwnedString::from_string).collect();
        Ok(FfiArray::from_vec(owned))
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, ptr))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clip_set_property_value(
    ptr_queue: *mut CommandQueue,
    timeline_id: TimelineId,
    clip_id: ClipId,
    key: StringView,
    value: CClipBindingValue,
) -> FfiResultVoid {
    fn inner(
        ptr_queue: *mut CommandQueue,
        timeline_id: TimelineId,
        clip_id: ClipId,
        key: StringView,
        value: CClipBindingValue,
    ) -> anyhow::Result<()> {
        if ptr_queue.is_null() {
            return Err(EsotereelError::NullPointer("ptr_queue".to_string()).into());
        }

        let key_str = key.as_str()?;
        let key = NamespacedID::parse(key_str)?;
        let field_value = value.unwrap_ffi()?;

        let queue = unsafe { &mut *ptr_queue };
        let command = CommandRequest::SetClipPropertyValue {
            clip_id,
            key,
            value: field_value.clone(),
        };

        queue.enqueue(timeline_id, command);

        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| {
        inner(ptr_queue, timeline_id, clip_id, key, value)
    })) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}
