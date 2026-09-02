use std::path::PathBuf;

pub fn install_dir() -> Option<std::path::PathBuf> {
    let exe_dir = std::env::current_exe()
        .expect("failed to get exe path")
        .parent()
        .unwrap()
        .to_path_buf();

    #[cfg(target_os = "macos")]
    {
        // Contents/MacOS/exe -> Contents/Resources/
        Some(exe_dir.parent().unwrap().join("Resources"))
    }

    #[cfg(target_os = "windows")]
    {
        use winreg::RegKey;
        use winreg::enums::HKEY_LOCAL_MACHINE;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.open_subkey("Software\\Esotereel").ok()?;
        let path: String = key.get_value("InstallLocation").ok()?;
        Some(std::path::PathBuf::from(path))
    }

    #[cfg(target_os = "linux")]
    {
        Some(
            std::env::var("APPDIR")
                .map(PathBuf::from)
                .map(|p| p.join("usr/share/esotereel"))
                .unwrap_or(exe_dir), // AppImage外での実行(開発時等)のフォールバック
        )
    }
}
