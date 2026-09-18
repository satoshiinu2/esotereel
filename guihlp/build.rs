use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

include!("ffi_types.rs");

fn main() {
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/generated");
    std::fs::create_dir_all(&out_dir).expect("failed to create generated dir");

    generate_result_types(&out_dir);
    generate_result_option_types(&out_dir);
    generate_option_types(&out_dir);
    generate_array_types(&out_dir);

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

    // 🫩
    fix_header_file();
}

/// C++ヘッダーファイルの後処理：前方宣言を追加
fn fix_header_file() {
    let path = "include/esotereel_gui_helper.h";

    let mut header = std::fs::read_to_string(path).expect("failed to read generated header");

    // CFieldValueMapEntry の定義を取り出す
    let entry_start = header
        .find("struct CFieldValueMapEntry {")
        .expect("CFieldValueMapEntry definition not found");

    let brace_start = header[entry_start..]
        .find('{')
        .map(|i| entry_start + i)
        .unwrap();

    let mut depth = 0;
    let mut entry_end = None;

    for (i, c) in header[brace_start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;

                if depth == 0 {
                    let pos = brace_start + i;

                    // `};` まで含める
                    let end = header[pos..].find(';').map(|i| pos + i + 1).unwrap();

                    entry_end = Some(end);
                    break;
                }
            }
            _ => {}
        }
    }

    let entry_end = entry_end.expect("CFieldValueMapEntry end not found");

    let entry = header[entry_start..entry_end].to_string();

    // 元の位置から削除
    header.replace_range(entry_start..entry_end, "");

    // CFieldValue の定義の直後を探す
    let value_start = header
        .find("struct CFieldValue {")
        .expect("CFieldValue definition not found");

    let value_brace_start = header[value_start..]
        .find('{')
        .map(|i| value_start + i)
        .unwrap();

    let mut depth = 0;
    let mut value_end = None;

    for (i, c) in header[value_brace_start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;

                if depth == 0 {
                    let pos = value_brace_start + i;

                    let end = header[pos..].find(';').map(|i| pos + i + 1).unwrap();

                    value_end = Some(end);
                    break;
                }
            }
            _ => {}
        }
    }

    let value_end = value_end.expect("CFieldValue end not found");

    // CFieldValue の直後に挿入
    header.insert_str(value_end, &format!("\n\n{}", entry));

    // 前方宣言
    let marker = "struct CFieldValueArray {";

    let pos = header.find(marker).expect("CFieldValueArray not found");

    header.insert_str(pos, "struct CFieldValue;\nstruct CFieldValueMapEntry;\n\n");

    std::fs::write(path, header).expect("failed to write generated header");
}
fn generate_result_types(out_dir: &Path) {
    let mut out = String::new();
    writeln!(
        out,
        "// 自動生成ファイル。手動編集禁止。build.rs / ffi_types.rs を編集してください。"
    )
    .unwrap();
    writeln!(out, "use super::*;\n").unwrap();

    for (name, ty) in RESULT_TYPES {
        writeln!(
            out,
            "#[warn(non_camel_case_types)]\n\
             pub type FfiResult_{name} = FfiResult<{ty}>;\n"
        )
        .unwrap();
        writeln!(
            out,
            "#[unsafe(no_mangle)]\n\
             pub extern \"C\" fn ffi_result_{name}_free_err(result: *mut FfiResult<{ty}>) {{\n\
                 unsafe {{\n\
                     if !(*result).is_ok {{\n\
                         std::ptr::drop_in_place(&mut (*result).value.err);\n\
                     }}\n\
                 }}\n\
             }}\n"
        )
        .unwrap();
    }

    std::fs::write(out_dir.join("result.rs"), out).expect("failed to write result.rs");
}

fn generate_result_option_types(out_dir: &Path) {
    let mut out = String::new();
    writeln!(out, "// 自動生成ファイル。").unwrap();
    writeln!(out, "use super::*;\n").unwrap();

    for (name, ty) in RESULT_OPTION_TYPES {
        writeln!(
            out,
            "#[warn(non_camel_case_types)]\n\
            pub type FfiResult_Option_{name} = FfiResult<FfiOption<{ty}>>;\n"
        )
        .unwrap();

        writeln!(
            out,
            "#[unsafe(no_mangle)]\n\
             pub extern \"C\" fn ffi_result_option_{name}_free_err(result: *mut FfiResult<FfiOption<{ty}>>) {{\n\
                 unsafe {{\n\
                     if !(*result).is_ok {{\n\
                         std::ptr::drop_in_place(&mut (*result).value.err);\n\
                     }}\n\
                 }}\n\
             }}\n\n"
        ).unwrap();
    }

    std::fs::write(out_dir.join("result_option.rs"), out).unwrap();
}

fn generate_option_types(out_dir: &Path) {
    let mut out = String::new();
    writeln!(out, "// 自動生成ファイル。手動編集禁止。").unwrap();
    writeln!(out, "use super::*;\n").unwrap();

    for (name, ty) in OPTION_TYPES {
        writeln!(
            out,
            "#[warn(non_camel_case_types)]\n\
           pub type FfiOption_{name} = FfiOption<{ty}>;\n"
        )
        .unwrap();
        writeln!(
            out,
            "#[unsafe(no_mangle)]\n\
             pub extern \"C\" fn ffi_option_{name}_free(opt: *mut FfiOption<{ty}>) {{\n\
                 unsafe {{\n\
                     if (*opt).has_value {{\n\
                         std::ptr::drop_in_place(&mut (*opt).value.some);\n\
                     }}\n\
                 }}\n\
             }}\n"
        )
        .unwrap();
    }

    std::fs::write(out_dir.join("option.rs"), out).expect("failed to write option.rs");
}

fn generate_array_types(out_dir: &Path) {
    let mut out = String::new();
    writeln!(out, "// 自動生成ファイル。手動編集禁止。").unwrap();
    writeln!(out, "use super::*;\n").unwrap();

    for (name, ty) in ARRAY_TYPES {
        writeln!(
            out,
            "#[warn(non_camel_case_types)]\n\
           pub type FfiArray_{name} = FfiArray<{ty}>;\n"
        )
        .unwrap();
        writeln!(
            out,
            "#[unsafe(no_mangle)]\n\
             pub extern \"C\" fn ffi_array_{name}_free(arr: *mut FfiArray<{ty}>) {{\n\
                 unsafe {{\n\
                     let a = &mut *arr;\n\
                     if !a.ptr.is_null() {{\n\
                         drop(Vec::from_raw_parts(a.ptr, a.len, a.cap));\n\
                         a.ptr = std::ptr::null_mut();\n\
                         a.len = 0;\n\
                         a.cap = 0;\n\
                     }}\n\
                 }}\n\
             }}\n"
        )
        .unwrap();
    }

    std::fs::write(out_dir.join("array.rs"), out).expect("failed to write array.rs");
}
