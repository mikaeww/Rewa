//! The settings page: the section rail and the lines of the chosen section.

use crate::render::*;

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_settings(
        &mut self,
        model: &UiModel,
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
    ) -> Result<(), String> {
        let rail_right = left + SETTINGS_RAIL_WIDTH;
        self.text(
            self.strings.settings,
            rect(left, top, rail_right - 12.0, top + 30.0),
            &self.heading_center.clone(),
            self.palette.primary,
        )?;
        let sections = [
            (
                SettingsSection::General,
                Glyph::Settings,
                self.strings.panel_general,
            ),
            (
                SettingsSection::Capture,
                Glyph::Monitor,
                self.strings.panel_capture,
            ),
            (
                SettingsSection::Audio,
                Glyph::Audio,
                self.strings.panel_audio,
            ),
            (
                SettingsSection::Storage,
                Glyph::Folder,
                self.strings.panel_storage,
            ),
            (
                SettingsSection::About,
                Glyph::Info,
                self.strings.about_label,
            ),
        ];
        let rows_top = top + 44.0;
        let selected = sections
            .iter()
            .position(|(section, _, _)| *section == model.settings_section)
            .unwrap_or(0);
        let now = Instant::now();
        let reduced = self.reduced_motion;
        let motion = self
            .toggle_motions
            .entry("settings_section")
            .or_insert_with(|| {
                crate::motion::Motion::with_curve(selected as f32, crate::motion::Curve::Glide)
            });
        motion.retarget(selected as f32, now, reduced);
        let position = motion.value(now);
        self.navigation_was_moving |= motion.active(now);
        let pill = rect(
            left,
            rows_top + position * NAVIGATION_PITCH,
            rail_right - 12.0,
            rows_top + position * NAVIGATION_PITCH + NAVIGATION_HEIGHT,
        );
        self.fill_alpha(
            rect(pill.left, pill.top + 1.0, pill.right, pill.bottom + 1.0),
            0x000000,
            0.16,
            RADIUS_SMALL,
        )?;
        self.fill(pill, self.palette.surface_raised, RADIUS_SMALL)?;
        for (index, (section, glyph, label)) in sections.into_iter().enumerate() {
            let row = rect(
                left,
                rows_top + index as f32 * NAVIGATION_PITCH,
                rail_right - 12.0,
                rows_top + index as f32 * NAVIGATION_PITCH + NAVIGATION_HEIGHT,
            );
            let action = Action::SettingsSection(section);
            let active = index == selected;
            let hovered = !active && self.is_hovered(&action);
            if hovered {
                self.tint(row, 0.05 * self.hover_amount(1.0), RADIUS_SMALL)?;
            }
            let tone = if active {
                self.palette.primary
            } else if hovered {
                self.palette.secondary
            } else {
                self.palette.muted
            };
            self.glyph(
                glyph,
                rect(
                    row.left + 10.0,
                    row.top + 7.5,
                    row.left + 25.0,
                    row.bottom - 7.5,
                ),
                tone,
            )?;
            self.text(
                label,
                rect(row.left + 34.0, row.top, row.right - 8.0, row.bottom),
                &self.body.clone(),
                tone,
            )?;
            self.hits.push(HitRegion { rect: row, action });
        }
        self.fill(
            rect(rail_right, top - 6.0, rail_right + 1.0, bottom),
            self.palette.border,
            0.0,
        )?;

        let (content_left, content_right) = settings_content(left, right);
        self.text(
            sections[selected].2,
            rect(content_left, top, content_right - 120.0, top + 30.0),
            &self.heading.clone(),
            self.palette.primary,
        )?;
        if model.settings_section != SettingsSection::About {
            let saving = model.settings_reload.is_some();
            let label = if saving {
                self.strings.saving
            } else {
                self.strings.save
            };
            let width = self.measure(label, &self.button) + 28.0;
            let save = rect(content_right - width, top + 2.0, content_right, top + 28.0);
            let action = Action::SaveSettings;
            if saving {
                self.tint(save, 0.08, 13.0)?;
            } else {
                self.fill(
                    save,
                    if self.is_hovered(&action) {
                        mix(
                            self.palette.accent,
                            self.palette.accent_hover,
                            self.hover_amount(1.0),
                        )
                    } else {
                        self.palette.accent
                    },
                    13.0,
                )?;
                self.hits.push(HitRegion { rect: save, action });
            }
            self.text(
                label,
                save,
                &self.button.clone(),
                if saving {
                    self.palette.secondary
                } else {
                    self.palette.accent_text
                },
            )?;
        }

        let line = |index: usize| settings_line_rect(left, right, top, index);
        match model.settings_section {
            SettingsSection::General => {
                self.settings_card(line(0), 6)?;
                self.settings_line(
                    line(0),
                    0,
                    self.strings.autostart,
                    self.strings.autostart_hint,
                    LineControl::Switch(model.autostart_enabled, Action::ToggleAutostart),
                )?;
                let shortcut = if model.hotkey_pending {
                    self.strings.hotkey_activating.to_owned()
                } else if model.hotkey_capture {
                    hotkey_capture_label(&model.hotkey_modifiers, self.strings)
                } else {
                    hotkey_label(model, self.strings)
                };
                // the line tells why a key was refused; a silent refusal reads as a dead control
                let (detail, detail_tone) = if let Some(error) = &model.hotkey_error {
                    (error.as_str(), self.palette.destructive)
                } else if model.hotkey_capture {
                    (self.strings.hotkey_rule, self.palette.secondary)
                } else if model.hotkey_deferred {
                    (self.strings.hotkey_next_start, self.palette.muted)
                } else {
                    (self.strings.replay_hotkey_hint, self.palette.muted)
                };
                let key = self.settings_line_toned(
                    line(1),
                    1,
                    self.strings.replay_hotkey,
                    detail,
                    detail_tone,
                    LineControl::Pill(&shortcut, Action::CaptureHotkey, model.hotkey_capture),
                )?;
                let center = (key.top + key.bottom) / 2.0;
                self.door(
                    rect(
                        key.left - 30.0,
                        center - 12.0,
                        key.left - 6.0,
                        center + 12.0,
                    ),
                    Glyph::Close,
                    false,
                    Action::ClearHotkey,
                )?;
                self.settings_line(
                    line(2),
                    2,
                    self.strings.theme_row,
                    self.strings.theme_hint,
                    LineControl::Popup(
                        theme_label(model.config.appearance.theme, self.strings),
                        Action::ChooseTheme,
                    ),
                )?;
                self.settings_line(
                    line(3),
                    3,
                    self.strings.hover_row,
                    self.strings.hover_hint,
                    LineControl::Popup(
                        hover_style_label(model.config.appearance.hover, self.strings),
                        Action::ChooseHoverStyle,
                    ),
                )?;
                self.settings_line(
                    line(4),
                    4,
                    self.strings.hover_strength_row,
                    self.strings.hover_strength_hint,
                    LineControl::Popup(
                        hover_strength_label(model.config.appearance.hover_strength, self.strings),
                        Action::ChooseHoverStrength,
                    ),
                )?;
                self.settings_line(
                    line(5),
                    5,
                    self.strings.language_row,
                    self.strings.language_hint,
                    LineControl::Popup(
                        language_label(model.config.appearance.language, self.strings),
                        Action::ChooseLanguage,
                    ),
                )?;
            }
            SettingsSection::Capture => {
                self.settings_card(line(0), 6)?;
                let display = model
                    .selected_display()
                    .map_or(self.strings.primary_display, |display| {
                        display.label.as_str()
                    });
                self.settings_line(
                    line(0),
                    0,
                    self.strings.display_row,
                    self.strings.display_hint,
                    LineControl::Popup(display, Action::ChooseDisplay),
                )?;
                self.settings_line(
                    line(1),
                    1,
                    self.strings.clip_duration,
                    self.strings.clip_duration_hint,
                    LineControl::Popup(
                        &self.strings.seconds(model.config.capture.duration_seconds),
                        Action::ChooseDuration,
                    ),
                )?;
                self.settings_line(
                    line(2),
                    2,
                    self.strings.frame_rate,
                    self.strings.frame_rate_hint,
                    LineControl::Popup(
                        &self
                            .strings
                            .frames_per_second(model.config.capture.frames_per_second),
                        Action::ChooseFrameRate,
                    ),
                )?;
                self.settings_line(
                    line(3),
                    3,
                    self.strings.video_quality,
                    self.strings.video_quality_hint,
                    LineControl::Popup(
                        &quality_label(model.config.capture.quality),
                        Action::ChooseQuality,
                    ),
                )?;
                self.settings_line(
                    line(4),
                    4,
                    self.strings.codec,
                    self.strings.codec_hint,
                    LineControl::Popup(
                        &format!("{:?}", model.config.capture.codec),
                        Action::ChooseCodec,
                    ),
                )?;
                self.settings_line(
                    line(5),
                    5,
                    self.strings.capture_cursor,
                    self.strings.capture_cursor_hint,
                    LineControl::Switch(model.config.capture.cursor, Action::ToggleCursor),
                )?;
            }
            SettingsSection::Audio => {
                self.settings_card(line(0), 6)?;
                self.settings_line(
                    line(0),
                    0,
                    self.strings.system_audio,
                    self.strings.system_audio_hint,
                    LineControl::Switch(model.config.audio.desktop, Action::ToggleDesktopAudio),
                )?;
                self.settings_line(
                    line(1),
                    1,
                    self.strings.output_device,
                    self.strings.output_device_hint,
                    LineControl::Popup(
                        device_name(
                            &model.output_names,
                            model.config.audio.desktop_device.as_ref(),
                            self.strings.windows_default,
                        ),
                        Action::ChooseDesktopDevice,
                    ),
                )?;
                self.settings_line(
                    line(2),
                    2,
                    self.strings.system_level,
                    self.strings.system_level_hint,
                    LineControl::Slider(
                        model.config.audio.desktop_gain_percent,
                        Action::DragDesktopGain,
                    ),
                )?;
                self.settings_line(
                    line(3),
                    3,
                    self.strings.microphone,
                    self.strings.microphone_hint,
                    LineControl::Switch(model.config.audio.microphone, Action::ToggleMicrophone),
                )?;
                self.settings_line(
                    line(4),
                    4,
                    self.strings.microphone_device,
                    self.strings.microphone_device_hint,
                    LineControl::Popup(
                        device_name(
                            &model.microphone_names,
                            model.config.audio.microphone_device.as_ref(),
                            self.strings.windows_default,
                        ),
                        Action::ChooseMicrophone,
                    ),
                )?;
                self.settings_line(
                    line(5),
                    5,
                    self.strings.microphone_level,
                    self.strings.microphone_level_hint,
                    LineControl::Slider(
                        model.config.audio.microphone_gain_percent,
                        Action::DragMicrophoneGain,
                    ),
                )?;
            }
            SettingsSection::Storage => {
                self.settings_card(line(0), 2)?;
                self.settings_line(
                    line(0),
                    0,
                    self.strings.storage_location,
                    self.strings.storage_location_hint,
                    LineControl::Pill(
                        &model.config.storage.directory.display().to_string(),
                        Action::ChooseStorage,
                        false,
                    ),
                )?;
                self.settings_line(
                    line(1),
                    1,
                    self.strings.storage_limit,
                    self.strings.storage_limit_hint,
                    LineControl::Popup(
                        &format_storage_limit(model.config.storage.max_megabytes),
                        Action::ChooseStorageLimit,
                    ),
                )?;
            }
            SettingsSection::About => {
                let head = line(0);
                self.draw_app_icon(rect(
                    head.left,
                    head.top + 4.0,
                    head.left + 48.0,
                    head.top + 52.0,
                ))?;
                self.text(
                    "rewa",
                    rect(
                        head.left + 56.0,
                        head.top + 8.0,
                        head.right,
                        head.top + 30.0,
                    ),
                    &self.heading.clone(),
                    self.palette.primary,
                )?;
                self.text(
                    &self.strings.version_line(env!("CARGO_PKG_VERSION")),
                    rect(
                        head.left + 56.0,
                        head.top + 30.0,
                        head.right,
                        head.top + 48.0,
                    ),
                    &self.small.clone(),
                    self.palette.muted,
                )?;
            }
        }
        Ok(())
    }
}
