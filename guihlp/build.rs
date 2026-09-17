use std::process::Command;

fn main() {
    let status = Command::new("cbindgen")
        .args(&[
            "--config",
            "cbindgen.toml",
            "--crate",
            "esotereel_gui_helper",
            "--output",
            "include/esotereel_gui_helper.h",
        ])
        .status()
        .expect("Failed to run cbindgen");

    if !status.success() {
        panic!("cbindgen failed");
    }

    cxx_qt_build::CxxQtBuilder::new()
        .qt_module("Gui")
        .file("src/ffi/cxxqt_field_value.rs")
        .build();
}
