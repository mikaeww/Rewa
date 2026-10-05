//! Talking to the recorder: status once a second, saving the replay, applying
//! saved settings with a reload, and the replay shortcut. Every call runs on a
//! worker thread and reports back through a channel the poll drains.

use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use rewa_core::config::{Config, HotkeyConfig};
use rewa_core::ipc::{self, Request, Response};
use rewa_core::paths::AppPaths;
use rewa_core::shortcuts::{self, ShortcutInstall};

use super::{App, system};
use crate::model::{DaemonSnapshot, UiModel};

const STATUS_INTERVAL: Duration = Duration::from_secs(1);
const DAEMON_STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const DAEMON_RETRY_INTERVAL: Duration = Duration::from_millis(100);

pub(crate) struct HotkeyUpdate {
    hotkey: HotkeyConfig,
    result: Result<ShortcutInstall, String>,
}

fn send(request: &Request) -> Result<Response, String> {
    let paths = AppPaths::discover();
    let mut stream = UnixStream::connect(paths.socket_file())
        .map_err(|error| format!("cannot reach the recorder: {error}"))?;
    ipc::write_request(&mut stream, request).map_err(|error| error.to_string())?;
    ipc::read_response(&mut BufReader::new(stream)).map_err(|error| error.to_string())
}

/// Starts the recorder service when it is not running, then waits for it.
fn send_starting(request: &Request) -> Result<Response, String> {
    let mut last_error = match send(request) {
        Ok(response) => return Ok(response),
        Err(error) => error,
    };
    system::start_daemon()
        .map_err(|error| format!("background service could not be started: {error}"))?;
    let attempts = DAEMON_STARTUP_TIMEOUT.as_millis() / DAEMON_RETRY_INTERVAL.as_millis();
    for _ in 0..attempts {
        std::thread::sleep(DAEMON_RETRY_INTERVAL);
        match send(request) {
            Ok(response) => return Ok(response),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "background service did not become ready within {} seconds (last error: {last_error})",
        DAEMON_STARTUP_TIMEOUT.as_secs()
    ))
}

pub(super) fn set_result(model: &mut UiModel, result: Result<(), String>, success: &str) {
    model.notice = Some(result.map_or_else(|error| error, |_| success.into()));
}

pub(super) fn poll_recorder_status(app: &mut App) -> bool {
    let mut changed = false;
    while let Ok(snapshot) = app.status_updates.try_recv() {
        app.status_pending = false;
        app.status_due = Instant::now() + STATUS_INTERVAL;
        if app.model.daemon != snapshot {
            app.model.daemon = snapshot;
            changed = true;
        }
    }
    let now = Instant::now();
    if !app.status_pending && now >= app.status_due {
        app.status_pending = true;
        let sender = app.status_sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send(read_daemon_status());
        });
    }
    if now >= app.microphone_due {
        changed |= system::poll_microphone_level(app, now);
    }
    changed
}

fn read_daemon_status() -> DaemonSnapshot {
    match send(&Request::Status) {
        Ok(Response::Status {
            state,
            buffered_seconds,
            error,
            ..
        }) => DaemonSnapshot {
            state: Some(state),
            buffered_seconds,
            error,
        },
        _ => DaemonSnapshot::default(),
    }
}

pub(super) fn start_replay_save(app: &mut App) {
    if app.model.replay_pending {
        return;
    }
    let (sender, receiver) = mpsc::channel();
    match std::thread::Builder::new()
        .name("rewa-save-replay".into())
        .spawn(move || {
            let _ = sender.send(send_starting(&Request::Save));
        }) {
        Ok(_) => {
            app.model.replay_pending = true;
            app.replay_updates = Some(receiver);
        }
        Err(error) => app.model.notice = Some(error.to_string()),
    }
}

pub(super) fn poll_replay_save(app: &mut App) -> bool {
    let Some(receiver) = &app.replay_updates else {
        return false;
    };
    let result = match receiver.try_recv() {
        Ok(result) => result,
        Err(mpsc::TryRecvError::Empty) => return false,
        Err(mpsc::TryRecvError::Disconnected) => {
            Err(app.model.strings().save_interrupted.to_owned())
        }
    };
    app.replay_updates = None;
    app.model.replay_pending = false;
    match result {
        Ok(Response::Saved { .. }) => {
            let refreshed = app.model.refresh();
            app.renderer.retry_unavailable_thumbnails();
            app.model.notice = Some(match refreshed {
                Ok(()) => app.model.strings().clip_saved.to_owned(),
                Err(error) => format!("{} · {error}", app.model.strings().clip_saved),
            });
        }
        Ok(Response::Error { message }) | Err(message) => app.model.notice = Some(message),
        Ok(_) => app.model.notice = Some(app.model.strings().save_interrupted.to_owned()),
    }
    true
}

pub(super) fn save_settings(model: &mut UiModel, success: &str) {
    match model.config.save(&model.paths) {
        Ok(()) => {
            model.settings_success = success.into();
            if model.settings_reload.is_some() {
                // persist every choice in order, coalesce reloads while the recorder
                // restarts, and apply the latest configuration last
                model.settings_reload_again = true;
            } else {
                start_settings_reload(model);
            }
        }
        Err(error) => {
            // a reload still running must not turn this failure into a success message
            model.settings_success.clear();
            model.notice = Some(format!(
                "{}: {error}",
                model.strings().notice_cannot_save_settings
            ));
        }
    }
}

fn start_settings_reload(model: &mut UiModel) {
    let (sender, receiver) = mpsc::channel();
    match std::thread::Builder::new()
        .name("rewa-reload-settings".into())
        .spawn(move || {
            let _ = sender.send(send_starting(&Request::Reload));
        }) {
        Ok(_) => model.settings_reload = Some(receiver),
        Err(error) => {
            model.notice = Some(format!(
                "{}: {error}",
                model.strings().notice_saved_reload_failed
            ));
        }
    }
}

pub(super) fn poll_settings_reload(model: &mut UiModel) -> bool {
    let Some(receiver) = &model.settings_reload else {
        return false;
    };
    let result = match receiver.try_recv() {
        Ok(result) => result,
        Err(mpsc::TryRecvError::Empty) => return false,
        Err(mpsc::TryRecvError::Disconnected) => {
            Err(model.strings().notice_saved_reload_failed.to_owned())
        }
    };
    model.settings_reload = None;
    if std::mem::take(&mut model.settings_reload_again) {
        start_settings_reload(model);
        return true;
    }
    if model.settings_success.is_empty() {
        return true;
    }
    model.notice = Some(match result {
        Ok(Response::Ok) => model.settings_success.clone(),
        Ok(Response::Error { message }) | Err(message) => {
            format!("{}: {message}", model.strings().notice_saved_reload_failed)
        }
        Ok(_) => model.strings().notice_saved_reload_failed.to_owned(),
    });
    true
}

pub(super) fn cancel_hotkey_capture(app: &mut App) {
    app.model.hotkey_capture = false;
    app.model.hotkey_modifiers.clear();
}

/// The shortcut is stored in the config and handed to the desktop: Hyprland gets
/// a bind, other desktops are told the command to bind themselves.
pub(super) fn begin_hotkey_update(app: &mut App, hotkey: HotkeyConfig) {
    cancel_hotkey_capture(app);
    app.model.hotkey_pending = true;
    app.model.hotkey_deferred = false;
    app.model.hotkey_error = None;
    app.model.notice = None;
    let paths = app.model.paths.clone();
    let updates = app.hotkey_sender.clone();
    let worker_hotkey = hotkey.clone();
    let spawned = std::thread::Builder::new()
        .name("rewa-hotkey-update".into())
        .spawn(move || {
            let result = apply_hotkey(&paths, &worker_hotkey);
            let _ = updates.send(HotkeyUpdate {
                hotkey: worker_hotkey,
                result,
            });
        });
    if let Err(error) = spawned {
        app.model.hotkey_pending = false;
        app.model.hotkey_error = Some(format!(
            "{}: {error}",
            app.model.strings().notice_shortcut_failed
        ));
    }
}

fn apply_hotkey(paths: &AppPaths, hotkey: &HotkeyConfig) -> Result<ShortcutInstall, String> {
    let mut config = Config::load(paths).map_err(|error| error.to_string())?;
    let previous = std::mem::replace(&mut config.hotkey, hotkey.clone());
    let installed = shortcuts::replace(Some(&previous), hotkey, &system::control_executable())
        .map_err(|error| error.to_string())?;
    config.save(paths).map_err(|error| error.to_string())?;
    Ok(installed)
}

pub(super) fn poll_hotkey_updates(app: &mut App) -> bool {
    let mut changed = false;
    while let Ok(update) = app.hotkey_updates.try_recv() {
        changed = true;
        app.model.hotkey_pending = false;
        app.model.hotkey_deferred = false;
        app.model.notice = None;
        match update.result {
            Ok(installed) => {
                app.model.config.hotkey = update.hotkey;
                app.model.hotkey_error = None;
                if let ShortcutInstall::Manual { command, .. } = installed
                    && app.model.config.hotkey.is_bound()
                {
                    app.model.notice = Some(format!(
                        "{}: {command}",
                        app.model.strings().notice_shortcut_manual
                    ));
                }
            }
            Err(error) => {
                app.model.hotkey_error = Some(format!(
                    "{}: {error}",
                    app.model.strings().notice_shortcut_failed
                ));
            }
        }
    }
    changed
}
