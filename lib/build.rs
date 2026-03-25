use std::process::Command;

fn main() {
    let status = Command::new("cbindgen")
        .args(&[
            "--config",
            "cbindgen.toml",
            "--crate",
            "muscedit_lib",
            "--output",
            "include/muscedit_lib.h",
        ])
        .status()
        .expect("Failed to run cbindgen");

    if !status.success() {
        panic!("cbindgen failed");
    }
}
