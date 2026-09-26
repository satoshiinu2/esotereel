use std::any::Any;

use esotereel_lib::{project::Project, util::result::format_any_error};

pub mod array;
pub mod arrayview;
pub mod commands;
pub mod debug_streams;
pub mod field_value;
pub mod internalserver;
pub mod logger;
pub mod option;
pub mod project;
pub mod render;
pub mod requests;
pub mod result;
pub mod settings;
pub mod state;
pub mod stringview;
pub mod toolbar;
pub mod wgpuutil;

pub type OnServerReadyFn = extern "C" fn(bool);

pub type ProjectGuard<'a> = std::sync::RwLockReadGuard<'a, Option<Project>>;

pub(crate) fn log_if_panicked<T>(
    result: Result<T, Box<dyn Any + Send>>,
    context: &str,
) -> Option<String> {
    if let Err(panic_info) = result {
        let msg = format_any_error(panic_info);

        log::error!("FFI: Panic occurred in {}! Message: {}", context, msg);
        Some(msg)
    } else {
        None
    }
}

#[macro_export]
macro_rules! ffi_fn {
    // Option版のアームを先に書く(マッチ優先順位のため)
    (
        $(#[$meta:meta])*
        fn $name:ident($($arg:ident: $arg_ty:ty),* $(,)?) -> anyhow::Result<Option<$ret:ty>> $body:block
    ) => {
        $(#[$meta])*
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($arg: $arg_ty),*) -> FfiResult<FfiOption<$ret>> {
            fn inner($($arg: $arg_ty),*) -> anyhow::Result<Option<$ret>> {
                $body
            }
            FfiResult::from_result(unsafe { inner($($arg),*) }.map(FfiOption::from))
        }
    };

    // vec
    (
        $(#[$meta:meta])*
        fn $name:ident($($arg:ident: $arg_ty:ty),* $(,)?) -> anyhow::Result<Vec<$ret:ty>> $body:block
    ) => {
        $(#[$meta])*
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($arg: $arg_ty),*) -> FfiResult<FfiArray<$ret>> {
            fn inner($($arg: $arg_ty),*) -> anyhow::Result<Vec<$ret>> {
                $body
            }
            FfiResult::from_result(unsafe { inner($($arg),*) }.map(FfiArray::from))
        }
    };

    // 通常版
    (
        $(#[$meta:meta])*
        fn $name:ident($($arg:ident: $arg_ty:ty),* $(,)?) -> anyhow::Result<$ret:ty> $body:block
    ) => {
        $(#[$meta])*
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($arg: $arg_ty),*) -> FfiResult<$ret> {
            fn inner($($arg: $arg_ty),*) -> anyhow::Result<$ret> {
                $body
            }
            FfiResult::from_result(unsafe { inner($($arg),*) })
        }
    };
}
