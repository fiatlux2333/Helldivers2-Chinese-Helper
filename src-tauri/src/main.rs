#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(all(windows, feature = "tauri-shell"))]
fn main() {
    helldivers2_cn_helper::run();
}

#[cfg(not(all(windows, feature = "tauri-shell")))]
fn main() {
    eprintln!("UnsupportedPlatform: Helldivers 2 中文助手后端仅支持 Windows Tauri 构建");
}
