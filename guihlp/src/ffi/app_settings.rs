//! アプリ設定（lib の階層設定 TOML）の FFI です。
//!
//! プラグイン設定（`ffi::settings` の `SettingsStore`）とは別の仕組みで、
//! こちらは `settings.toml` ファイルそのものを層ごとに読み書きします。
//! 層は 0=ユーザー、1=プロジェクトです。プロジェクト層のファイルは、
//! サーバーが現在開いているプロジェクトフォルダ内の `settings.toml` です。
//!
//! 値はクリッププロパティと同じ `CFieldValue` で受け渡しします。
//! 設定の `UInt` は TOML の整数（i64）の範囲に限られるため、
//! 境界で `Int` へ checked 変換します（範囲外はエラー）。

use std::path::PathBuf;
use std::sync::Arc;

use esotereel_lib::{
    plugin::property::value::FieldValue,
    settings::{
        declared_keys, read_layer_values, write_layer_value, AppSettings, Layer, SettingScope,
        SettingValue, SettingValueKind,
    },
    util::result::{EsotereelError, LockExt},
};

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::ffi::{
    array::FfiArray,
    field_value::CFieldValue,
    option::FfiOption,
    result::{FfiResult, FfiResultVoid},
    state::ClientStateHandle,
    stringview::{FfiOwnedString, FfiStringView},
};
use crate::state::ClientState;

/// `SettingScope` の C 表現です。
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CSettingScope {
    UserOnly = 0,
    ProjectOrServerOnly = 1,
    Overridable = 2,
}

impl From<SettingScope> for CSettingScope {
    fn from(scope: SettingScope) -> Self {
        match scope {
            SettingScope::UserOnly => CSettingScope::UserOnly,
            SettingScope::ProjectOrServerOnly => CSettingScope::ProjectOrServerOnly,
            SettingScope::Overridable => CSettingScope::Overridable,
        }
    }
}

/// 宣言済みキー1件の、ある層から見た状態です。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CAppSetting {
    pub key: FfiOwnedString,
    pub scope: CSettingScope,
    /// この層で編集できるかどうか（スコープ外なら false）。
    pub editable_here: bool,
    /// この層に値が設定されているかどうか。
    pub is_set: bool,
    /// この層の値（未設定なら None）。
    pub layer_value: FfiOption<CFieldValue>,
    /// 階層解決した実効値（未設定が実効値なら None）。
    pub effective_value: FfiOption<CFieldValue>,
    /// 既定値（「未設定」が既定のキーでは None）。
    pub default_value: FfiOption<CFieldValue>,
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

/// 設定値をプロパティ値へ変換します。
///
/// `UInt` は TOML の整数が i64 のため、設定ファイルに現れる値は
/// i64 の範囲内に限られます。範囲外は黙って丸めずエラーにします。
fn field_value_of(value: SettingValue) -> anyhow::Result<FieldValue> {
    match value {
        SettingValue::Int(value) => Ok(FieldValue::Int(value)),
        SettingValue::UInt(value) => i64::try_from(value)
            .map(FieldValue::Int)
            .map_err(|_| anyhow::anyhow!("setting value {value} does not fit in i64")),
        SettingValue::Bool(value) => Ok(FieldValue::Bool(value)),
    }
}

fn c_field_value_of(value: Option<SettingValue>) -> anyhow::Result<FfiOption<CFieldValue>> {
    match value {
        Some(value) => Ok(FfiOption::some(CFieldValue::wrap_ffi(&field_value_of(
            value,
        )?))),
        None => Ok(FfiOption::none()),
    }
}

/// プロパティ値を、宣言された型に合わせて設定値へ変換します。
/// 型が合わない場合と `UInt` への負値・範囲外はエラーです。
fn setting_value_of(kind: SettingValueKind, value: FieldValue) -> anyhow::Result<SettingValue> {
    match (kind, value) {
        (SettingValueKind::Int, FieldValue::Int(value)) => Ok(SettingValue::Int(value)),
        (SettingValueKind::UInt, FieldValue::Int(value)) => u64::try_from(value)
            .map(SettingValue::UInt)
            .map_err(|_| anyhow::anyhow!("value {value} does not fit in UInt")),
        (SettingValueKind::Bool, FieldValue::Bool(value)) => Ok(SettingValue::Bool(value)),
        _ => anyhow::bail!("value type does not match the declared setting kind"),
    }
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
                Ok(CAppSetting {
                    key: FfiOwnedString::from_string(info.key.to_string()),
                    scope: CSettingScope::from(info.scope),
                    editable_here: info.scope.allows(layer),
                    is_set: layer_value.is_some(),
                    layer_value: c_field_value_of(layer_value)?,
                    effective_value: c_field_value_of(effective.value_of(info.key))?,
                    default_value: c_field_value_of(info.default_value)?,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
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
    value: CFieldValue,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        layer: u8,
        key: FfiStringView,
        value: CFieldValue,
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
        let value = setting_value_of(info.kind, value.unwrap_ffi()?)?;
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
