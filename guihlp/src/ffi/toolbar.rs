use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use esotereel_lib::{
    plugin::{
        NamespacedID,
        script::api::PluginActionContext,
        toolbar::{RunOn, ToolbarButtonSpec},
    },
    project::ids::TimelineId,
    requests::Request,
    util::result::EsotereelError,
};

use crate::ffi::{
    array::FfiArray,
    result::{FfiResult, FfiResultVoid},
    state::ClientStateHandle,
    stringview::{FfiOwnedString, FfiStringView},
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiToolbarButton {
    pub id: FfiOwnedString,
    pub label: FfiOwnedString,
    pub tooltip: FfiOwnedString,
    /// アイコン未指定なら空文字列。
    pub icon: FfiOwnedString,
    pub action: FfiOwnedString,
    pub run_on: FfiOwnedString,
}

impl FfiToolbarButton {
    fn from_spec(spec: &ToolbarButtonSpec) -> Self {
        let run_on_str = match spec.action.run_on {
            RunOn::Client => "client",
            RunOn::Server => "server",
        };
        Self {
            id: FfiOwnedString::from_string(spec.id.to_string()),
            label: FfiOwnedString::from_string(spec.label.clone()),
            tooltip: FfiOwnedString::from_string(spec.tooltip.clone()),
            icon: FfiOwnedString::from_string(spec.icon.clone().unwrap_or_default()),
            action: FfiOwnedString::from_string(spec.action.func_name.clone()),
            run_on: FfiOwnedString::from_string(run_on_str.to_string()),
        }
    }
}

/// 個数取得(toolbar_get_buttons_count)+バッファ書き込み(toolbar_get_buttons)の2関数ペアを、
/// FfiArrayを直接返す1関数に統合。呼び出し側(C++)はFfiArray::free_fnで解放する。
pub type FfiToolbarButtonArrayResult = FfiResult<FfiArray<FfiToolbarButton>>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn toolbar_get_buttons(
    ptr_state: *const ClientStateHandle,
    target: FfiStringView,
) -> FfiToolbarButtonArrayResult {
    fn inner(
        ptr_state: *const ClientStateHandle,
        target: FfiStringView,
    ) -> anyhow::Result<FfiArray<FfiToolbarButton>> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        // Arc::into_raw 由来のポインタから、参照カウントを増やして
        // 独立した Arc クローンを作る（元のポインタは消費しない）
        let state = ClientStateHandle::from_ptr(ptr_state);
        let target_str = target.as_str()?;
        let state = state.lock().expect("mutex poisoned");

        let ids = state.toolbar.get_layout(target_str);
        let buttons: Vec<FfiToolbarButton> = ids
            .iter()
            .filter_map(|id| state.toolbar.registry.buttons().find(|b| &b.id == id))
            .map(FfiToolbarButton::from_spec)
            .collect();

        Ok(FfiArray::from_vec(buttons))
    }

    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, target))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn toolbar_set_layout(
    ptr_state: *const ClientStateHandle,
    target: FfiStringView,
    ids_toml_array: FfiStringView,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        target: FfiStringView,
        ids_toml_array: FfiStringView,
    ) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        let state = ClientStateHandle::from_ptr(ptr_state);
        let target_str = target.as_str()?;
        let ids_str = ids_toml_array.as_str()?;

        let parsed_value: toml::Value =
            toml::from_str(ids_str).map_err(|e| anyhow::anyhow!("Failed to parse ids: {}", e))?;
        let ids: Vec<String> = parsed_value
            .try_into()
            .map_err(|e| anyhow::anyhow!("ids must be an array of strings: {}", e))?;

        let ids = ids
            .into_iter()
            .map(|s| NamespacedID::parse(&s))
            .collect::<Result<Vec<NamespacedID>, _>>()
            .map_err(|e| anyhow::anyhow!("Failed to parse NamespacedID: {}", e))?;

        let mut state = state.lock().expect("mutex poisoned");

        state.toolbar.set_layout(target_str.to_string(), ids);

        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| {
        inner(ptr_state, target, ids_toml_array)
    })) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn toolbar_handle_action(
    ptr_state: *const ClientStateHandle,
    button_id: FfiStringView,
    timeline_id: TimelineId,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        button_id: FfiStringView,
        timeline_id: TimelineId,
    ) -> anyhow::Result<()> {
        if ptr_state.is_null() {
            return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
        }

        // Arc::into_raw 由来のポインタから、参照カウントを増やして
        // 独立した Arc クローンを作る（元のポインタは消費しない）
        let state = ClientStateHandle::from_ptr(ptr_state);
        let button_id = NamespacedID::parse(button_id.as_str()?)?;
        let state = state.lock().expect("mutex poisoned");

        let button = state
            .toolbar
            .registry
            .get_button_by(&button_id)
            .ok_or_else(|| anyhow::anyhow!("ToolbarButton not found"))?;

        // run_onをチェック
        if matches!(button.action.run_on, RunOn::Server) {
            // サーバーで実行するリクエストを送信
            let request = Request::ToolbarAction {
                button_id: button_id.to_string(),
                func_name: button.action.func_name.clone(),
                run_on: Some(timeline_id),
                timeline_id,
            };
            state.network.send(&request);
            return Ok(());
        }

        // clientまたは指定なしの場合はローカルで実行
        // PluginActionContextを作成
        let project = state.project.clone();
        let context = PluginActionContext::new(Arc::clone(&state.network), project, timeline_id);

        // スクリプト実行
        state
            .common
            .plugin_loader
            .read()
            .expect("lock poisoned")
            .call_script::<()>(
                &state.common.script_engine,
                button_id.plugin_id(),
                &button.action.func_name,
                (context.clone(),),
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        for req in context.take_packets() {
            state.network.send(&req);
        }

        Ok(())
    }

    match catch_unwind(AssertUnwindSafe(|| {
        inner(ptr_state, button_id, timeline_id)
    })) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}
