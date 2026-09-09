use log::info;

pub(super) fn register_fn_for(engine: &mut rhai::Engine) {
    engine.register_fn("core_undo", core_undo);
    engine.register_fn("core_redo", core_redo);
}

fn core_undo() {
    info!("undo placeholder called");
}
fn core_redo() {
    info!("redo placeholder called");
}
