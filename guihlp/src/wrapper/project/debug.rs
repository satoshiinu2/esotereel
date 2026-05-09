use esotereel_lib::project::Project;

#[unsafe(no_mangle)]
pub extern "C" fn project_debug_log(ptr: *const Project) {
    if ptr.is_null() {
        return;
    }

    unsafe{log::debug!("{:?}", (&*ptr))};
}
