#[cfg(target_os = "windows")]
pub mod app;
#[cfg(target_os = "windows")]
pub mod autostart;
#[cfg(any(target_os = "windows", test))]
pub use rewa_shell::clock;
#[cfg(target_os = "windows")]
pub mod icon;
#[cfg(any(target_os = "windows", test))]
pub use rewa_shell::model;
#[cfg(any(target_os = "windows", test))]
pub use rewa_shell::motion;
#[cfg(target_os = "windows")]
pub mod player;
#[cfg(any(target_os = "windows", test))]
pub mod recovery;
#[cfg(target_os = "windows")]
pub mod renderer;
#[cfg(any(target_os = "windows", test))]
pub use rewa_shell::text;
#[cfg(target_os = "windows")]
pub mod toast;
#[cfg(target_os = "windows")]
pub mod tray;
