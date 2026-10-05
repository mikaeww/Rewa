//! The choice menus of settings, filters and the capture popover, and what picking
//! an entry changes. Taken over from the Windows `app.rs` unchanged apart from
//! where displays and devices come from.

use rewa_core::config::{Codec, HoverStrength, HoverStyle, Language, Theme};

use super::{daemon, system};
use crate::model::{
    Page, SettingsMenu, SettingsMenuItem, SettingsMenuKind, SizeFilter, TimeFilter, TypeFilter,
    UiModel, hover_strength_label, hover_style_label, language_label, theme_label,
};

pub(super) fn choose_duration(model: &mut UiModel) {
    let values = [15, 30, 45, 60, 90, 120];
    let text = model.strings();
    let labels = values
        .iter()
        .map(|seconds| text.seconds(*seconds))
        .collect::<Vec<_>>();
    let current = values
        .iter()
        .position(|value| *value == model.config.capture.duration_seconds);
    open_choice_menu(model, SettingsMenuKind::Duration, labels, current);
}

pub(super) fn choose_frame_rate(model: &mut UiModel) {
    let values = model.frame_rate_options();
    let labels = values
        .iter()
        .map(|rate| format!("{rate} fps"))
        .collect::<Vec<_>>();
    let current = values
        .iter()
        .position(|value| *value == model.config.capture.frames_per_second);
    open_choice_menu(model, SettingsMenuKind::FrameRate, labels, current);
}

pub(super) fn choose_codec(model: &mut UiModel) {
    let values = [Codec::Auto, Codec::H264, Codec::Hevc, Codec::Av1];
    let labels = [model.strings().codec_auto, "H.264", "HEVC", "AV1"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let current = values
        .iter()
        .position(|value| *value == model.config.capture.codec);
    open_choice_menu(model, SettingsMenuKind::Codec, labels, current);
}

pub(super) fn choose_quality(model: &mut UiModel) {
    let options = model.quality_options();
    let items = options
        .iter()
        .map(|option| SettingsMenuItem {
            label: option.label.clone(),
            detail: Some(format!(
                "≈ {} MB total · {} s",
                option.megabytes, option.seconds
            )),
        })
        .collect::<Vec<_>>();
    let current = options
        .iter()
        .position(|option| option.value == model.config.capture.quality);
    model.settings_menu = Some(SettingsMenu::new(SettingsMenuKind::Quality, items, current));
}

pub(super) fn choose_audio_mode(model: &mut UiModel) {
    let text = model.strings();
    let labels = [
        text.audio_system,
        text.audio_microphone,
        text.audio_system_and_microphone,
        text.audio_none,
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    let current = match (model.config.audio.desktop, model.config.audio.microphone) {
        (true, false) => Some(0),
        (false, true) => Some(1),
        (true, true) => Some(2),
        (false, false) => Some(3),
    };
    open_choice_menu(model, SettingsMenuKind::AudioMode, labels, current);
}

pub(super) fn choose_display(model: &mut UiModel) {
    if let Err(error) = system::load_displays(model) {
        model.notice = Some(error);
        return;
    }
    let labels = model
        .displays
        .iter()
        .map(|display| display.label.clone())
        .collect::<Vec<_>>();
    let current = model.config.capture.monitor.as_deref().and_then(|name| {
        model
            .displays
            .iter()
            .position(|display| display.name.eq_ignore_ascii_case(name))
    });
    open_choice_menu(model, SettingsMenuKind::Display, labels, current);
}

pub(super) fn choose_microphone(model: &mut UiModel) {
    system::refresh_microphones(model);
    let mut labels = vec![model.strings().windows_default.to_string()];
    labels.extend(model.microphone_names.iter().map(|(_, name)| name.clone()));
    let current = model
        .config
        .audio
        .microphone_device
        .as_deref()
        .and_then(|id| {
            model
                .microphone_names
                .iter()
                .position(|(device_id, _)| device_id == id)
        })
        .map_or(Some(0), |index| Some(index + 1));
    open_choice_menu(model, SettingsMenuKind::Microphone, labels, current);
}

pub(super) fn choose_microphone_gain(model: &mut UiModel) {
    let values = [25, 50, 75, 100];
    let labels = values
        .iter()
        .map(|gain| format!("{gain}%"))
        .collect::<Vec<_>>();
    let current = values
        .iter()
        .position(|value| *value == model.config.audio.microphone_gain_percent);
    open_choice_menu(model, SettingsMenuKind::MicrophoneGain, labels, current);
}

pub(super) fn choose_desktop_device(model: &mut UiModel) {
    system::refresh_outputs(model);
    let mut labels = vec![model.strings().windows_default.to_string()];
    labels.extend(model.output_names.iter().map(|(_, name)| name.clone()));
    let current = model
        .config
        .audio
        .desktop_device
        .as_deref()
        .and_then(|id| {
            model
                .output_names
                .iter()
                .position(|(device_id, _)| device_id == id)
        })
        .map_or(Some(0), |index| Some(index + 1));
    open_choice_menu(model, SettingsMenuKind::DesktopDevice, labels, current);
}

pub(super) fn choose_desktop_gain(model: &mut UiModel) {
    let values = [0, 25, 50, 75, 100, 125, 150, 175, 200];
    let labels = values
        .iter()
        .map(|gain| format!("{gain}%"))
        .collect::<Vec<_>>();
    let current = values
        .iter()
        .position(|value| *value == model.config.audio.desktop_gain_percent);
    open_choice_menu(model, SettingsMenuKind::DesktopGain, labels, current);
}

pub(super) fn choose_theme(model: &mut UiModel) {
    let labels = Theme::OPTIONS
        .iter()
        .map(|theme| theme_label(*theme, model.strings()).to_owned())
        .collect::<Vec<_>>();
    let current = Theme::OPTIONS
        .iter()
        .position(|theme| *theme == model.config.appearance.theme);
    open_choice_menu(model, SettingsMenuKind::Theme, labels, current);
}

pub(super) fn choose_hover_style(model: &mut UiModel) {
    let labels = HoverStyle::OPTIONS
        .iter()
        .map(|style| hover_style_label(*style, model.strings()).to_owned())
        .collect::<Vec<_>>();
    let current = HoverStyle::OPTIONS
        .iter()
        .position(|style| *style == model.config.appearance.hover);
    open_choice_menu(model, SettingsMenuKind::HoverStyle, labels, current);
}

pub(super) fn choose_hover_strength(model: &mut UiModel) {
    let labels = HoverStrength::OPTIONS
        .iter()
        .map(|strength| hover_strength_label(*strength, model.strings()).to_owned())
        .collect::<Vec<_>>();
    let current = HoverStrength::OPTIONS
        .iter()
        .position(|strength| *strength == model.config.appearance.hover_strength);
    open_choice_menu(model, SettingsMenuKind::HoverStrength, labels, current);
}

pub(super) fn choose_language(model: &mut UiModel) {
    let labels = Language::OPTIONS
        .iter()
        .map(|language| language_label(*language, model.strings()).to_owned())
        .collect::<Vec<_>>();
    let current = Language::OPTIONS
        .iter()
        .position(|language| *language == model.config.appearance.language);
    open_choice_menu(model, SettingsMenuKind::Language, labels, current);
}

pub(super) fn choose_time_filter(model: &mut UiModel) {
    let labels = TimeFilter::OPTIONS
        .iter()
        .map(|option| option.label(model.strings()).to_owned())
        .collect::<Vec<_>>();
    let current = TimeFilter::OPTIONS
        .iter()
        .position(|option| *option == model.filter_time);
    open_choice_menu(model, SettingsMenuKind::TimeFilter, labels, current);
}

pub(super) fn choose_collection_filter(model: &mut UiModel) {
    let mut labels = vec![model.strings().all.to_owned()];
    labels.extend(
        model
            .collections
            .iter()
            .map(|collection| collection.name.clone()),
    );
    let current = model
        .filter_collection
        .as_ref()
        .and_then(|path| {
            model
                .collections
                .iter()
                .position(|collection| &collection.path == path)
        })
        .map_or(0, |index| index + 1);
    open_choice_menu(
        model,
        SettingsMenuKind::CollectionFilter,
        labels,
        Some(current),
    );
}

pub(super) fn choose_type_filter(model: &mut UiModel) {
    let labels = TypeFilter::OPTIONS
        .iter()
        .map(|option| option.label(model.strings()).to_owned())
        .collect::<Vec<_>>();
    let current = TypeFilter::OPTIONS
        .iter()
        .position(|option| *option == model.filter_type);
    open_choice_menu(model, SettingsMenuKind::TypeFilter, labels, current);
}

pub(super) fn choose_size_filter(model: &mut UiModel) {
    let labels = SizeFilter::OPTIONS
        .iter()
        .map(|option| option.label(model.strings()).to_owned())
        .collect::<Vec<_>>();
    let current = SizeFilter::OPTIONS
        .iter()
        .position(|option| *option == model.filter_size);
    open_choice_menu(model, SettingsMenuKind::SizeFilter, labels, current);
}

pub(super) fn choose_clip_sort(model: &mut UiModel) {
    let text = model.strings();
    let labels = [text.sort_newest, text.sort_oldest]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let current = usize::from(model.clips_oldest_first);
    open_choice_menu(model, SettingsMenuKind::ClipSort, labels, Some(current));
}

pub(super) fn choose_storage_limit(model: &mut UiModel) {
    let values = [1_024, 5_120, 10_240, 25_600, 51_200, 102_400, 1_048_576];
    let labels = ["1 GB", "5 GB", "10 GB", "25 GB", "50 GB", "100 GB", "1 TB"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let current = values
        .iter()
        .position(|value| *value == model.config.storage.max_megabytes);
    open_choice_menu(model, SettingsMenuKind::StorageLimit, labels, current);
}

fn open_choice_menu(
    model: &mut UiModel,
    kind: SettingsMenuKind,
    labels: Vec<String>,
    current: Option<usize>,
) {
    let items = labels
        .into_iter()
        .map(|label| SettingsMenuItem {
            label,
            detail: None,
        })
        .collect();
    model.settings_menu = Some(SettingsMenu::new(kind, items, current));
}

pub(super) fn select_settings_option(model: &mut UiModel, index: usize) {
    let Some(menu) = model.settings_menu.as_ref() else {
        return;
    };
    if index >= menu.items.len() {
        return;
    }
    let kind = menu.kind;
    let appearance = matches!(
        menu.kind,
        SettingsMenuKind::Theme
            | SettingsMenuKind::HoverStyle
            | SettingsMenuKind::HoverStrength
            | SettingsMenuKind::Language
    );
    let library_filter = matches!(
        menu.kind,
        SettingsMenuKind::TimeFilter
            | SettingsMenuKind::CollectionFilter
            | SettingsMenuKind::TypeFilter
            | SettingsMenuKind::SizeFilter
            | SettingsMenuKind::ClipSort
    );
    let apply_immediately = !library_filter && !appearance && model.page != Page::Settings;
    match kind {
        SettingsMenuKind::Theme => {
            if let Some(theme) = Theme::OPTIONS.get(index) {
                model.config.appearance.theme = *theme;
            }
        }
        SettingsMenuKind::Language => {
            if let Some(language) = Language::OPTIONS.get(index) {
                model.config.appearance.language = *language;
                model.refresh_language();
            }
        }
        SettingsMenuKind::HoverStyle => {
            if let Some(style) = HoverStyle::OPTIONS.get(index) {
                model.config.appearance.hover = *style;
            }
        }
        SettingsMenuKind::HoverStrength => {
            if let Some(strength) = HoverStrength::OPTIONS.get(index) {
                model.config.appearance.hover_strength = *strength;
            }
        }
        SettingsMenuKind::TimeFilter => {
            if let Some(value) = TimeFilter::OPTIONS.get(index) {
                model.filter_time = *value;
            }
        }
        SettingsMenuKind::CollectionFilter => {
            model.filter_collection = index
                .checked_sub(1)
                .and_then(|index| model.collections.get(index))
                .map(|collection| collection.path.clone());
        }
        SettingsMenuKind::TypeFilter => {
            if let Some(value) = TypeFilter::OPTIONS.get(index) {
                model.filter_type = *value;
            }
        }
        SettingsMenuKind::SizeFilter => {
            if let Some(value) = SizeFilter::OPTIONS.get(index) {
                model.filter_size = *value;
            }
        }
        SettingsMenuKind::ClipSort => model.clips_oldest_first = index == 1,
        SettingsMenuKind::Duration => {
            if let Some(value) = [15, 30, 45, 60, 90, 120].get(index) {
                model.config.capture.duration_seconds = *value;
            }
        }
        SettingsMenuKind::FrameRate => {
            if let Some(value) = model.frame_rate_options().get(index) {
                model.config.capture.frames_per_second = *value;
            }
        }
        SettingsMenuKind::Codec => {
            if let Some(value) = [Codec::Auto, Codec::H264, Codec::Hevc, Codec::Av1].get(index) {
                model.config.capture.codec = *value;
            }
        }
        SettingsMenuKind::Quality => {
            if let Some(option) = model.quality_options().get(index) {
                model.config.capture.quality = option.value;
            }
        }
        SettingsMenuKind::AudioMode => match index {
            0 => {
                model.config.audio.desktop = true;
                model.config.audio.microphone = false;
            }
            1 => {
                model.config.audio.desktop = false;
                model.config.audio.microphone = true;
            }
            2 => {
                model.config.audio.desktop = true;
                model.config.audio.microphone = true;
            }
            3 => {
                model.config.audio.desktop = false;
                model.config.audio.microphone = false;
            }
            _ => {}
        },
        SettingsMenuKind::Display => {
            if let Some(display) = model.displays.get(index) {
                model.config.capture.monitor = Some(display.name.clone());
                let native_rate = (display.refresh_rate.round() as u16)
                    .clamp(15, rewa_core::config::MAX_FRAMES_PER_SECOND);
                model.config.capture.frames_per_second =
                    model.config.capture.frames_per_second.min(native_rate);
            }
        }
        SettingsMenuKind::Microphone => {
            model.config.audio.microphone_device = index
                .checked_sub(1)
                .and_then(|index| model.microphone_names.get(index))
                .map(|(id, _)| id.clone());
        }
        SettingsMenuKind::MicrophoneGain => {
            if let Some(value) = [25, 50, 75, 100].get(index) {
                model.config.audio.microphone_gain_percent = *value;
            }
        }
        SettingsMenuKind::DesktopDevice => {
            model.config.audio.desktop_device = index
                .checked_sub(1)
                .and_then(|index| model.output_names.get(index))
                .map(|(id, _)| id.clone());
        }
        SettingsMenuKind::DesktopGain => {
            if let Some(value) = [0, 25, 50, 75, 100, 125, 150, 175, 200].get(index) {
                model.config.audio.desktop_gain_percent = *value;
            }
        }
        SettingsMenuKind::StorageLimit => {
            if let Some(value) =
                [1_024, 5_120, 10_240, 25_600, 51_200, 102_400, 1_048_576].get(index)
            {
                model.config.storage.max_megabytes = *value;
            }
        }
    }
    model.settings_menu = None;
    if library_filter {
        model.library_scroll = 0.0;
    }
    if appearance {
        persist_appearance(model);
    }
    if apply_immediately {
        let message = model.strings().notice_setting_applied;
        daemon::save_settings(model, message);
    }
}

/// The look applies at once, but the settings page may hold unconfirmed capture
/// edits, so only the appearance block reaches the file.
pub(super) fn persist_appearance(model: &mut UiModel) {
    let paths = model.paths.clone();
    let mut stored =
        rewa_core::config::Config::load(&paths).unwrap_or_else(|_| model.config.clone());
    stored.appearance = model.config.appearance;
    if let Err(error) = stored.save(&paths) {
        model.notice = Some(format!(
            "{}: {error}",
            model.strings().notice_appearance_failed
        ));
    }
}
