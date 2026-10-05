//! What the Linux desktop provides where Windows has its own APIs: displays and
//! audio devices, autostart through the systemd user unit, opening files and
//! folders through the desktop portal, the folder picker and the microphone test.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

use gtk::gio;
use gtk::prelude::*;

use super::App;
use crate::model::{DisplayOption, UiModel};

pub(super) fn load_displays(model: &mut UiModel) -> Result<(), String> {
    let mut monitors = rewa_core::display::monitors().map_err(|error| error.to_string())?;
    // the focused screen first, so the fallback the page shows is the one the recorder takes
    monitors.sort_by_key(|monitor| !monitor.focused);
    if monitors.is_empty() {
        return Err("No displays were found".into());
    }
    model.displays = monitors
        .into_iter()
        .map(|monitor| {
            let label = if monitor.uses_portal() {
                monitor.description.clone()
            } else {
                format!(
                    "{} · {}×{} · {:.0} Hz",
                    monitor.name, monitor.width, monitor.height, monitor.refresh_rate
                )
            };
            DisplayOption {
                label,
                short_label: monitor.name.clone(),
                // the recorder resolves a description or a connector; Rewa stores descriptions
                name: if monitor.description.is_empty() {
                    monitor.name
                } else {
                    monitor.description
                },
                refresh_rate: monitor.refresh_rate,
                width: monitor.width,
                height: monitor.height,
            }
        })
        .collect();
    Ok(())
}

pub(super) fn refresh_displays(model: &mut UiModel) {
    // ponytail: a missing gpu-screen-recorder leaves the list empty and the page on "automatic"
    let _ = load_displays(model);
}

pub(super) fn refresh_microphones(model: &mut UiModel) {
    if let Ok(devices) = rewa_core::audio::microphones() {
        model.microphone_names = devices
            .into_iter()
            .map(|device| (device.name, device.label))
            .collect();
    }
}

pub(super) fn refresh_outputs(model: &mut UiModel) {
    if let Ok(devices) = rewa_core::audio::desktop_outputs() {
        model.output_names = devices
            .into_iter()
            .map(|device| (device.name, device.label))
            .collect();
    }
}

fn systemctl(arguments: &[&str]) -> Result<std::process::Output, String> {
    Command::new("systemctl")
        .arg("--user")
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run systemctl: {error}"))
}

pub(super) fn autostart_enabled() -> bool {
    systemctl(&["is-enabled", "--quiet", "rewad.service"])
        .is_ok_and(|output| output.status.success())
}

pub(super) fn set_autostart(enabled: bool) -> Result<(), String> {
    let output = systemctl(&[if enabled { "enable" } else { "disable" }, "rewad.service"])?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if detail.is_empty() {
        format!("systemctl exited with {}", output.status)
    } else {
        detail
    })
}

pub(super) fn start_daemon() -> Result<(), String> {
    let output = systemctl(&["start", "rewad.service"])?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

/// Opens a clip or folder in the user's default application.
pub(super) fn open_path(window: &gtk::ApplicationWindow, path: &Path) {
    gtk::FileLauncher::new(Some(&gio::File::for_path(path))).launch(
        Some(window),
        None::<&gio::Cancellable>,
        |result| {
            if let Err(error) = result {
                rewa_core::diagnostic!("Rewa: cannot open the file: {error}");
            }
        },
    );
}

pub(super) fn show_in_file_manager(window: &gtk::ApplicationWindow, path: &Path) {
    gtk::FileLauncher::new(Some(&gio::File::for_path(path))).open_containing_folder(
        Some(window),
        None::<&gio::Cancellable>,
        |result| {
            if let Err(error) = result {
                rewa_core::diagnostic!("Rewa: cannot show the file: {error}");
            }
        },
    );
}

pub(super) fn choose_storage(app: &mut App) {
    let dialog = gtk::FileDialog::new();
    dialog.set_initial_folder(Some(&gio::File::for_path(
        &app.model.config.storage.directory,
    )));
    let weak = app.this.clone();
    dialog.select_folder(
        Some(&app.window),
        None::<&gio::Cancellable>,
        move |result| {
            super::input::with(&weak, |app| {
                match result {
                    Ok(folder) => {
                        if let Some(path) = folder.path() {
                            app.model.config.storage.directory = path;
                        }
                    }
                    Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                    Err(error) => {
                        app.model.notice = Some(format!(
                            "{}: {error}",
                            app.model.strings().notice_folder_picker_failed
                        ));
                    }
                }
                app.redraw();
            });
        },
    );
}

/// The microphone test: a short-latency `parec` stream whose loudest sample per
/// read becomes the meter level. It only runs while the test is on, so the desktop
/// never shows Rewa holding the microphone otherwise.
pub(crate) struct MicrophoneProbe {
    child: Child,
    device: Option<String>,
    peak: Arc<AtomicU8>,
}

impl MicrophoneProbe {
    fn open(device: Option<&str>) -> Result<Self, String> {
        let mut command = Command::new("parec");
        command.args([
            "--raw",
            "--format=s16le",
            "--rate=16000",
            "--channels=1",
            "--latency-msec=30",
        ]);
        if let Some(device) = device {
            command.arg(format!("--device={device}"));
        }
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("cannot run parec: {error}"))?;
        let mut stream = child
            .stdout
            .take()
            .ok_or_else(|| "parec gave no audio stream".to_owned())?;
        let peak = Arc::new(AtomicU8::new(0));
        let level = peak.clone();
        std::thread::Builder::new()
            .name("rewa-microphone".into())
            .spawn(move || {
                let mut buffer = [0_u8; 960];
                while let Ok(read) = stream.read(&mut buffer) {
                    if read == 0 {
                        break;
                    }
                    level.fetch_max(peak_percent(&buffer[..read]), Ordering::Relaxed);
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            child,
            device: device.map(str::to_owned),
            peak,
        })
    }

    fn matches(&self, device: Option<&str>) -> bool {
        self.device.as_deref() == device
    }

    /// The loudest level since the last call, or nothing once parec has stopped.
    fn take_peak(&mut self) -> Option<u8> {
        if self.child.try_wait().ok().flatten().is_some() {
            return None;
        }
        Some(self.peak.swap(0, Ordering::Relaxed))
    }
}

impl Drop for MicrophoneProbe {
    fn drop(&mut self) {
        // the stream belongs to this probe alone; a failed kill means it already ended
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn peak_percent(samples: &[u8]) -> u8 {
    let peak = samples
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]).unsigned_abs())
        .max()
        .unwrap_or(0);
    (u32::from(peak) * 100 / u32::from(i16::MAX.unsigned_abs())).min(100) as u8
}

/// Opens the configured input, falling back to the system default so the test
/// still answers the question when the saved device disappeared.
pub(super) fn start_microphone_test(app: &mut App) -> bool {
    let device = app.model.config.audio.microphone_device.clone();
    let opened = MicrophoneProbe::open(device.as_deref()).map(|probe| (probe, false));
    let opened = match opened {
        Ok(opened) => Ok(opened),
        Err(_) if device.is_some() => MicrophoneProbe::open(None).map(|probe| (probe, true)),
        Err(error) => Err(error),
    };
    match opened {
        Ok((probe, fell_back)) => {
            app.microphone = Some(probe);
            if fell_back {
                app.model.notice = Some(app.model.strings().notice_microphone_fallback.to_owned());
            }
            true
        }
        Err(error) => {
            app.model.microphone_test = false;
            app.microphone = None;
            app.model.notice = Some(format!(
                "{}: {error}",
                app.model.strings().notice_microphone_test
            ));
            false
        }
    }
}

pub(super) fn poll_microphone_level(app: &mut App, now: Instant) -> bool {
    app.microphone_due = now + std::time::Duration::from_millis(66);
    if !app.model.microphone_test {
        app.microphone = None;
        return std::mem::take(&mut app.model.microphone_level) != 0;
    }
    let device = app.model.config.audio.microphone_device.clone();
    let stale = app
        .microphone
        .as_ref()
        .is_none_or(|probe| !probe.matches(device.as_deref()));
    if stale && !start_microphone_test(app) {
        return true;
    }
    let peak = app
        .microphone
        .as_mut()
        .and_then(MicrophoneProbe::take_peak)
        .unwrap_or(0);
    app.model.apply_microphone_peak(peak)
}

pub(super) fn control_executable() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .map(|directory| directory.join("rewactl"))
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from("rewactl"))
}

#[cfg(test)]
mod tests {
    use super::peak_percent;

    #[test]
    fn the_meter_reads_the_loudest_sample_of_a_read() {
        let quiet = [0_i16, 120, -80]
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        let loud = [0_i16, -32_767, 5]
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        assert_eq!(peak_percent(&quiet), 0);
        assert_eq!(peak_percent(&loud), 100);
        assert_eq!(peak_percent(&[]), 0);
    }
}
