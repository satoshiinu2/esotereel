//! プロジェクトフォルダの開閉・保存、オートセーブ復旧、セッション
//! 状態に関する FFI です。
//!
//! 開く流れは2段階です: `req_inspect_project` でバージョン判定を受け、
//! C++ 側が段階表示（警告・バックアップ選択）をしてから
//! `req_open_project` を送ります。結果は `client_take_*` で取り出します。

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use esotereel_lib::{
    requests::Request,
    responces::WireVersionStatus,
    util::result::EsotereelError,
};

use crate::ffi::{
    array::FfiArray,
    option::FfiOption,
    result::{FfiResult, FfiResultVoid},
    state::ClientStateHandle,
    stringview::{FfiOwnedString, FfiStringView},
};
use crate::network::ClientNetworkHandler;
use crate::state::ClientState;

fn network_of(ptr_state: *const ClientStateHandle) -> Arc<ClientNetworkHandler> {
    ClientStateHandle::from_ptr(ptr_state).network_arc()
}

fn state_of(ptr_state: *const ClientStateHandle) -> Arc<ClientState> {
    ClientStateHandle::from_ptr(ptr_state)
}

fn check_ptr(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
    if ptr_state.is_null() {
        return Err(EsotereelError::NullPointer("ptr_state".to_string()).into());
    }
    Ok(())
}

/// `WireVersionStatus` の C 表現です。
/// 0=Compatible、1=NewerRevision、2=NewerMinor、3=NewerMajor。
fn status_code(status: WireVersionStatus) -> u8 {
    match status {
        WireVersionStatus::Compatible => 0,
        WireVersionStatus::NewerRevision => 1,
        WireVersionStatus::NewerMinor => 2,
        WireVersionStatus::NewerMajor => 3,
    }
}

/// プロジェクトフォルダの事前確認結果です。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CProjectInspection {
    pub path: FfiOwnedString,
    pub project_id: FfiOwnedString,
    pub version_major: u32,
    pub version_minor: u32,
    pub version_revision: u32,
    pub status: u8,
}

/// プロジェクトを開いた結果です。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CProjectOpened {
    pub path: FfiOwnedString,
    pub version_major: u32,
    pub version_minor: u32,
    pub version_revision: u32,
    pub status: u8,
}

/// オートセーブ世代1件です。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CAutosaveGeneration {
    pub id: FfiOwnedString,
    pub project_id: FfiOwnedString,
    pub created_at_ms: u64,
    pub size_bytes: u64,
    pub file_count: u64,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_inspect_project(
    ptr_state: *const ClientStateHandle,
    path: FfiStringView,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        path: FfiStringView,
    ) -> anyhow::Result<()> {
        check_ptr(ptr_state)?;
        let path = path.as_str()?.to_string();
        network_of(ptr_state).send(&Request::InspectProject { path });
        Ok(())
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, path))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_open_project(
    ptr_state: *const ClientStateHandle,
    path: FfiStringView,
    backup: bool,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        path: FfiStringView,
        backup: bool,
    ) -> anyhow::Result<()> {
        check_ptr(ptr_state)?;
        let path = path.as_str()?.to_string();
        network_of(ptr_state).send(&Request::OpenProject { path, backup });
        Ok(())
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, path, backup))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn req_save_project(
    ptr_state: *const ClientStateHandle,
    overwrite: bool,
) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle, overwrite: bool) -> anyhow::Result<()> {
        check_ptr(ptr_state)?;
        network_of(ptr_state).send(&Request::SaveProject { overwrite });
        Ok(())
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, overwrite))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_save_project_as(
    ptr_state: *const ClientStateHandle,
    path: FfiStringView,
    overwrite: bool,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        path: FfiStringView,
        overwrite: bool,
    ) -> anyhow::Result<()> {
        check_ptr(ptr_state)?;
        let path = path.as_str()?.to_string();
        network_of(ptr_state).send(&Request::SaveProjectAs { path, overwrite });
        Ok(())
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, path, overwrite))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn req_list_autosave_generations(
    ptr_state: *const ClientStateHandle,
) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        check_ptr(ptr_state)?;
        network_of(ptr_state).send(&Request::ListAutosaveGenerations);
        Ok(())
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn req_restore_autosave_generation(
    ptr_state: *const ClientStateHandle,
    generation_id: FfiStringView,
) -> FfiResultVoid {
    fn inner(
        ptr_state: *const ClientStateHandle,
        generation_id: FfiStringView,
    ) -> anyhow::Result<()> {
        check_ptr(ptr_state)?;
        let generation_id = generation_id.as_str()?.to_string();
        network_of(ptr_state).send(&Request::RestoreAutosaveGeneration { generation_id });
        Ok(())
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state, generation_id))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}

pub type CProjectInspectionResult = FfiResult<FfiOption<CProjectInspection>>;
pub type CProjectOpenedResult = FfiResult<FfiOption<CProjectOpened>>;
pub type COptionStringResult = FfiResult<FfiOption<FfiOwnedString>>;
pub type CAutosaveGenerationArrayResult = FfiResult<FfiArray<CAutosaveGeneration>>;

#[unsafe(no_mangle)]
pub extern "C" fn client_take_project_inspection(
    ptr_state: *const ClientStateHandle,
) -> CProjectInspectionResult {
    fn inner(
        ptr_state: *const ClientStateHandle,
    ) -> anyhow::Result<Option<CProjectInspection>> {
        check_ptr(ptr_state)?;
        let state = state_of(ptr_state);
        let mut slot = state
            .project_inspection
            .lock()
            .map_err(|_| anyhow::anyhow!("project inspection lock poisoned"))?;
        Ok(slot.take().map(|inspection| CProjectInspection {
            path: FfiOwnedString::from_string(inspection.path),
            project_id: FfiOwnedString::from_string(inspection.project_id),
            version_major: inspection.version_major,
            version_minor: inspection.version_minor,
            version_revision: inspection.version_revision,
            status: status_code(inspection.status),
        }))
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(v)) => FfiResult::ok(FfiOption::from(v)),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn client_take_project_opened(
    ptr_state: *const ClientStateHandle,
) -> CProjectOpenedResult {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<Option<CProjectOpened>> {
        check_ptr(ptr_state)?;
        let state = state_of(ptr_state);
        let mut slot = state
            .project_opened
            .lock()
            .map_err(|_| anyhow::anyhow!("project opened lock poisoned"))?;
        Ok(slot.take().map(|opened| CProjectOpened {
            path: FfiOwnedString::from_string(opened.path),
            version_major: opened.version_major,
            version_minor: opened.version_minor,
            version_revision: opened.version_revision,
            status: status_code(opened.status),
        }))
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(v)) => FfiResult::ok(FfiOption::from(v)),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn client_take_project_saved(
    ptr_state: *const ClientStateHandle,
) -> COptionStringResult {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<Option<FfiOwnedString>> {
        check_ptr(ptr_state)?;
        let state = state_of(ptr_state);
        let mut slot = state
            .project_saved
            .lock()
            .map_err(|_| anyhow::anyhow!("project saved lock poisoned"))?;
        Ok(slot.take().map(FfiOwnedString::from_string))
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(v)) => FfiResult::ok(FfiOption::from(v)),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn client_take_autosave_generations(
    ptr_state: *const ClientStateHandle,
) -> CAutosaveGenerationArrayResult {
    fn inner(
        ptr_state: *const ClientStateHandle,
    ) -> anyhow::Result<FfiArray<CAutosaveGeneration>> {
        check_ptr(ptr_state)?;
        let state = state_of(ptr_state);
        let mut slot = state
            .autosave_generations
            .lock()
            .map_err(|_| anyhow::anyhow!("autosave generations lock poisoned"))?;
        let generations: Vec<CAutosaveGeneration> = slot
            .drain(..)
            .map(|generation| CAutosaveGeneration {
                id: FfiOwnedString::from_string(generation.id),
                project_id: FfiOwnedString::from_string(generation.project_id),
                created_at_ms: generation.created_at_ms,
                size_bytes: generation.size_bytes,
                file_count: generation.file_count,
            })
            .collect();
        Ok(FfiArray::from_vec(generations))
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn client_previous_session_abnormal(
    ptr_state: *const ClientStateHandle,
) -> FfiResult<bool> {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<bool> {
        check_ptr(ptr_state)?;
        Ok(state_of(ptr_state).previous_session_abnormal)
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(v)) => FfiResult::ok(v),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn client_current_project_folder(
    ptr_state: *const ClientStateHandle,
) -> COptionStringResult {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<Option<FfiOwnedString>> {
        check_ptr(ptr_state)?;
        let state = state_of(ptr_state);
        let folder = state
            .current_project_folder
            .read()
            .map_err(|_| anyhow::anyhow!("current project folder lock poisoned"))?;
        Ok(folder.clone().map(FfiOwnedString::from_string))
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(Ok(v)) => FfiResult::ok(FfiOption::from(v)),
        Ok(Err(e)) => FfiResult::err(e),
        Err(panic) => FfiResult::err_panic(panic),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn client_mark_clean_exit(ptr_state: *const ClientStateHandle) -> FfiResultVoid {
    fn inner(ptr_state: *const ClientStateHandle) -> anyhow::Result<()> {
        check_ptr(ptr_state)?;
        state_of(ptr_state).mark_clean_exit();
        Ok(())
    }
    match catch_unwind(AssertUnwindSafe(|| inner(ptr_state))) {
        Ok(r) => FfiResultVoid::from_result(r),
        Err(panic) => FfiResultVoid::err_panic(panic),
    }
}
