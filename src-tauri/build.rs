fn main() {
    let is_windows_target = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let has_tauri_shell = std::env::var_os("CARGO_FEATURE_TAURI_SHELL").is_some();

    if is_windows_target && has_tauri_shell {
        tauri_build::build();
    }
}
