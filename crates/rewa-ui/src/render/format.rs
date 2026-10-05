//! Text for numbers and keys: durations, sizes, times and shortcut labels.

use super::*;

pub(crate) fn format_clip_badge_duration(total_seconds: u64) -> String {
    let hours = total_seconds / 3_600;
    let minutes = total_seconds % 3_600 / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

pub(crate) fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    }
}

pub(crate) fn format_storage_limit(megabytes: u32) -> String {
    if megabytes >= 1_048_576 && megabytes % 1_048_576 == 0 {
        format!("{} TB", megabytes / 1_048_576)
    } else if megabytes >= 1_024 && megabytes % 1_024 == 0 {
        format!("{} GB", megabytes / 1_024)
    } else {
        format!("{megabytes} MB")
    }
}

pub(crate) fn format_player_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

pub(crate) fn format_editor_time(value: Duration) -> String {
    let total_millis = value.as_millis();
    let minutes = total_millis / 60_000;
    let seconds = total_millis % 60_000 / 1_000;
    let millis = total_millis % 1_000;
    format!("{minutes:02}:{seconds:02}.{millis:03}")
}

/// Keys read the way the desktop names them, modifiers first.
pub(crate) fn hotkey_label(model: &UiModel, text: &Strings) -> String {
    let hotkey = &model.config.hotkey;
    if !hotkey.is_bound() {
        return text.hotkey_unbound.to_owned();
    }
    hotkey
        .modifiers
        .iter()
        .map(|modifier| modifier_label(modifier).to_owned())
        .chain(std::iter::once(key_label(&hotkey.key)))
        .collect::<Vec<_>>()
        .join(" + ")
}

fn modifier_label(modifier: &str) -> &str {
    match modifier {
        "SUPER" => "Super",
        "CTRL" => "Ctrl",
        "ALT" => "Alt",
        "SHIFT" => "Shift",
        value => value,
    }
}

/// XKB names are upper-cased in the config; single letters stay that way, named
/// keys such as PRINT or F12 read as Print and F12.
fn key_label(key: &str) -> String {
    let mut characters = key.chars();
    match characters.next() {
        Some(first) if key.len() > 3 && key.chars().all(|c| c.is_ascii_alphabetic()) => {
            first.to_string() + &characters.as_str().to_ascii_lowercase()
        }
        _ => key.to_owned(),
    }
}

pub(crate) fn hotkey_capture_label(modifiers: &[String], text: &Strings) -> String {
    let modifiers = modifiers
        .iter()
        .map(|modifier| modifier_label(modifier))
        .collect::<Vec<_>>();
    if modifiers.is_empty() {
        text.hotkey_prompt.to_owned()
    } else {
        format!("{} + …", modifiers.join(" + "))
    }
}
