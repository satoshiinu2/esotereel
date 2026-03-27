use std::process::Command;

fn main() {
    let status = Command::new("cbindgen")
        .args(&[
            "--config",
            "cbindgen.toml",
            "--crate",
            "nomyoedit_gui_helper",
            "--output",
            "include/nomyoedit_gui_helper.h",
        ])
        .status()
        .expect("Failed to run cbindgen");

    if !status.success() {
        panic!("cbindgen failed");
    }
}
