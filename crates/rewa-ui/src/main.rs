//! Rewa's Linux window. It shows the same interface as the Windows application:
//! the shared model and strings from `rewa-shell`, painted by a port of the
//! Windows renderer into one GTK surface.

mod app;
mod player;
mod preview;
mod render;
mod surface;

use rewa_shell::{clock, model, motion, text};

fn main() -> std::process::ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.first().map(String::as_str) == Some("--render") {
        return match preview::render(&arguments[1..]) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("rewa-ui: {error}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    app::run()
}
