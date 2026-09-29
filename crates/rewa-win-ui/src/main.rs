#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
fn main() -> std::process::ExitCode {
    match rewa_win_ui::app::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            rewa_win_ui::app::show_error(&error);
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("rewa-win-ui is available only on Windows");
}
