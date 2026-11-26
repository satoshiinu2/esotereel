//! アプリ設定（lib の階層設定 TOML）の FFI です。
//!
//! プラグイン設定（`ffi::settings` の `SettingsStore`）とは別の仕組みで、
//! こちらは `settings.toml` ファイルそのものを層ごとに読み書きします。
//! 層は 0=ユーザー、1=プロジェクトです。プロジェクト層のファイルは、
//! サーバーが現在開いているプロジェクトフォルダ内の `settings.toml` です。

use std::path::PathBuf;
use std::sync::Arc;

use esotereel_lib::{
    settings::{
        declared_keys, read_layer_values, write_layer_value, AppSettings, Layer, SettingScope,
        SettingValue, SettingValueKind,
    },
    util::result::{EsotereelError, LockExt},
};

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::ffi::{
    array::FfiArray,
    result::{FfiResult, FfiResultVoid},
    state::ClientStateHandle,
    stringview::{FfiOwnedString, FfiStringView},
};
use crate::state::ClientState;

/// 宣言済みキー1件の、ある層から見た状態です。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CAppSetting {
    pub key: FfiOwnedString,
    /// 0=UserOnly、1=ProjectOrServerOnly、2=Overridable。
    pub scope: u8,
    /// 0=Int、1=UInt、2=Bool。
    pub kind: u8,
    /// この層で編集できるかどうか（スコープ外なら false）。
    pub editable_here: bool,
    /// この層に値が設定されているかどうか。
    pub is_set: bool,
    /// この層の値（未設定なら空文字列）。
    pub layer_value: FfiOwnedString,
    /// 階層解決した実効値（未設定が実効値なら空文字列）。
    pub effective_value: FfiOwnedString,
    /// 既定値（「未設定」が既定のキーでは空文字列）。
    pub default_value: FfiOwnedString,
}

fn check_ptr(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
    if ptr_state.is_null() {
        return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
    }
    Ok(())
}

fn state_of(ptr_state: *const ClientStateHandle) -> Arc<ClientState> {
    ClientStateHandle::from_ptr(ptr_state)
}

fn layer_of(layer: u8) -> anyhow::Result<Layer> {
    match layer {
        0 => Ok(Layer::User),
        1 => Ok(Layer::Project),
        _ => anyhow::bail!("invalid settings layer {layer}"),
    }
}

/// 層の設定ファイルのパスを返します。
fn layer_file(state: &ClientState, layer: Layer) -> anyhow::Result<PathBuf> {
    match layer {
        Layer::User => Ok(state.dir.client_settings_path()?),
        Layer::Project => {
            let folder = state
                .current_project_folder
                .read_or_err()?
                .clone()
                .ok_or_else(|| {
                    anyhow::anyhow!("no project folder is open; open or save a project first")
                })?;
            Ok(PathBuf::from(folder).join("settings.toml"))
        }
        Layer::Server => anyhow::bail!("server settings are not editable from the client"),
    }
}

fn scope_code(scope: SettingScope) -> u8 {
    match scope {
        SettingScope::UserOnly => 0,
        SettingScope::ProjectOrServerOnly => 1,
        SettingScope::Overridable => 2,
    }
}

fn kind_code(kind: SettingValueKind) -> u8 {
    match kind {
        SettingValueKind::Int => 0,
        SettingValueKind::UInt => 1,
        SettingValueKind::Bool => 2,
    }
}

fn display(value: Option<SettingValue>) -> FfiOwnedString {
    FfiOwnedString::from_string(value.map(|v| v.to_display_string()).unwrap_or_default())
}

pub type CAppSettingArrayResult = FfiResult<FfiArray<CAppSetting>>;

#[unsafe(no_mangle)]
pub extern "C" fn app_settings_list(
    ptr_state: *const ClientStateHandle,
    layer: u8,
) -> CAppSettingArrayResult {
    fn inner(
        ptr_state: *const ClientStateHandle,
        layer: u8,
    ) -> anyhow::Result<FfiArray<CAppSetting>> {
        check_ptr(ptr_state)?;
        let state = state_of(ptr_state);
        let layer = layer_of(layer)?;
        let file = layer_file(&state, layer)?;
        let layer_values = read_layer_values(&file)?;
        let user_path = state.dir.client_settings_path().ok();
        let project_dir = state
            .current_project_folder
            .read_or_err()?
            .clone()
            .map(PathBuf::from);
        let effective = AppSettings::load(user_path.as_deref(), project_dir.as_deref(), None)?;

        let entries: Vec<CAppSetting> = declared_keys()
            .iter()
            .map(|info| {
                let layer_value = layer_values
                    .iter()
                    .find(|(key, _)| key == info.key)
                    .map(|(_, value)| *value);
                CAppSetting {
                    key: FfiOwnedString::from_string(info.key.to_string()),
                    scope: scope_code(info.scope),
                    kind: kind_code(info.kind),
                    editable_here: info.scope.allows(layer),
                    is_set: layer_value.is_some(),
                    layer_value: display(layer_value),
                    effective_value: display(effective.value_of(info.key)),
                    default_value: display(info.default_value),
                }
            })
            .collect();
        Ok(FfiArray::from_vec(entries))
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, layer))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn app_settings_set(
    ptr_state: *const ClientStateHandle,
    layer: u8,
    key: FfiStringView,
    value: FfiStringView,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        layer: u8,
        key: FfiStringView,
        value: FfiStringView,
    ) -> anyhow::Result<()> {
        check_ptr(ptr_state)?;
        let state = state_of(ptr_state);
        let layer = layer_of(layer)?;
        let key = key.as_str()?;
        let info = declared_keys()
            .into_iter()
            .find(|info| info.key == key)
            .ok_or_else(|| anyhow::anyhow!("unknown setting key `{key}`"))?;
        if !info.scope.allows(layer) {
            anyhow::bail!("setting `{key}` cannot be set in this layer (scope)");
        }
        let value = SettingValue::parse_as(info.kind, value.as_str()?)
            .ok_or_else(|| anyhow::anyhow!("invalid value for setting `{key}`"))?;
        let file = layer_file(&state, layer)?;
        write_layer_value(&file, key, Some(value))?;
        Ok(())
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, layer, key, value))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn app_settings_reset(
    ptr_state: *const ClientStateHandle,
    layer: u8,
    key: FfiStringView,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        layer: u8,
        key: FfiStringView,
    ) -> anyhow::Result<()> {
        check_ptr(ptr_state)?;
        let state = state_of(ptr_state);
        let layer = layer_of(layer)?;
        let key = key.as_str()?;
        let info = declared_keys()
            .into_iter()
            .find(|info| info.key == key)
            .ok_or_else(|| anyhow::anyhow!("unknown setting key `{key}`"))?;
        if !info.scope.allows(layer) {
            anyhow::bail!("setting `{key}` cannot be set in this layer (scope)");
        }
        let file = layer_file(&state, layer)?;
        write_layer_value(&file, key, None)?;
        Ok(())
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, layer, key))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}
