//! The capture popover the replay row opens: state, the four capture choices,
//! the microphone meter, storage use and the shortcut.

use super::*;

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    /// Replay state and the save action live at the foot of the sidebar, so every
    /// page can save without a toolbar; the state row opens the capture popover.
    pub(crate) fn render_capture_panel(
        &mut self,
        model: &UiModel,
        height: f32,
    ) -> Result<(), String> {
        let rail = self.rail;
        let rows = 4.0;
        let panel_height =
            16.0 + 40.0 + rows * CAPTURE_ROW_HEIGHT + 17.0 + 3.0 * CAPTURE_ROW_HEIGHT + 10.0;
        let (left, bottom) = if model.sidebar_collapsed {
            (rail + 8.0, height - 14.0 - NAVIGATION_HEIGHT - 12.0)
        } else {
            (10.0, height - 14.0 - NAVIGATION_HEIGHT - 12.0 - 30.0 - 52.0)
        };
        let panel = rect(
            left,
            (bottom - panel_height).max(12.0),
            left + POPOVER_WIDTH,
            bottom,
        );
        // the panel floats over the page and swallows clicks before the page sees them
        self.hits.push(HitRegion {
            rect: panel,
            action: Action::Ignore,
        });
        self.popover_surface(panel)?;
        self.capture_panel_content(model, panel)
    }

    pub(crate) fn capture_panel_content(
        &mut self,
        model: &UiModel,
        panel: LogicalRect,
    ) -> Result<(), String> {
        let live = model.daemon.is_recording();
        let inner = rect(
            panel.left + 8.0,
            panel.top + 8.0,
            panel.right - 8.0,
            panel.bottom - 8.0,
        );
        self.status_dot(inner.left + 14.0, inner.top + 20.0, live)?;
        self.text(
            model.daemon.toolbar_headline(self.strings),
            rect(
                inner.left + 28.0,
                inner.top + 8.0,
                inner.right - 10.0,
                inner.top + 32.0,
            ),
            &self.strong.clone(),
            self.palette.primary,
        )?;
        let seconds = model
            .daemon
            .buffered_seconds
            .min(model.config.capture.duration_seconds);
        self.text(
            &self.strings.buffered_seconds(seconds),
            rect(
                inner.left + 28.0,
                inner.top + 8.0,
                inner.right - 10.0,
                inner.top + 32.0,
            ),
            &self.small_right.clone(),
            self.palette.muted,
        )?;

        let display = model.selected_display().map_or_else(
            || self.strings.automatic.to_owned(),
            |display| display.short_label.clone(),
        );
        let quality = self.strings.resolution_line(
            model
                .selected_display()
                .map_or(1_080, |display| display.height),
            model.config.capture.frames_per_second,
        );
        let audio = match (model.config.audio.desktop, model.config.audio.microphone) {
            (true, true) => self.strings.audio_system_and_microphone,
            (true, false) => self.strings.audio_system,
            (false, true) => self.strings.audio_microphone,
            (false, false) => self.strings.audio_none,
        };
        let settings = [
            (
                Glyph::Clock,
                self.strings.clip_length_label,
                self.strings.seconds(model.config.capture.duration_seconds),
                Action::ChooseDuration,
            ),
            (
                Glyph::Monitor,
                self.strings.display_label,
                display,
                Action::ChooseDisplay,
            ),
            (
                Glyph::Quality,
                self.strings.quality_label,
                quality,
                Action::ChooseQuality,
            ),
            (
                Glyph::Audio,
                self.strings.audio_label,
                audio.to_owned(),
                Action::ChooseAudioMode,
            ),
        ];
        let mut top = inner.top + 40.0;
        for (glyph, label, value, action) in settings {
            let row = rect(inner.left, top, inner.right, top + CAPTURE_ROW_HEIGHT);
            self.capture_row(row, glyph, label, &value, Some(action), true)?;
            top += CAPTURE_ROW_HEIGHT;
        }
        top += 8.0;
        self.fill(
            rect(inner.left + 10.0, top, inner.right - 10.0, top + 1.0),
            self.palette.border,
            0.0,
        )?;
        top += 9.0;

        let microphone = rect(inner.left, top, inner.right, top + CAPTURE_ROW_HEIGHT);
        self.capture_row(
            microphone,
            Glyph::Microphone,
            self.strings.microphone_label,
            "",
            Some(Action::ToggleMicrophoneTest),
            false,
        )?;
        let meter_right = microphone.right - 12.0;
        self.level_meter(
            rect(
                meter_right - METER_WIDTH,
                microphone.top + 13.0,
                meter_right,
                microphone.bottom - 13.0,
            ),
            model.config.audio.microphone || model.microphone_test,
            model.microphone_level,
            model.microphone_peak_hold,
        )?;
        self.text(
            &model.microphone_readout(),
            rect(
                microphone.left + 120.0,
                microphone.top,
                meter_right - METER_WIDTH - 10.0,
                microphone.bottom,
            ),
            &self.small_right.clone(),
            if model.microphone_test {
                self.palette.primary
            } else {
                self.palette.muted
            },
        )?;
        top += CAPTURE_ROW_HEIGHT;

        let used = model.total_size_bytes();
        let limit = u64::from(model.config.storage.max_megabytes).saturating_mul(1_048_576);
        let fraction = if limit == 0 {
            0.0
        } else {
            (used as f32 / limit as f32).clamp(0.0, 1.0)
        };
        let storage = format!(
            "{} / {}",
            format_bytes(used),
            format_storage_limit(model.config.storage.max_megabytes)
        );
        let storage_row = rect(inner.left, top, inner.right, top + CAPTURE_ROW_HEIGHT);
        self.capture_row(
            storage_row,
            Glyph::Folder,
            self.strings.storage_label,
            &storage,
            None,
            false,
        )?;
        let track = rect(
            storage_row.left + 36.0,
            storage_row.bottom - 5.0,
            storage_row.right - 12.0,
            storage_row.bottom - 3.0,
        );
        self.tint(track, 0.08, 1.0)?;
        self.fill(
            rect(
                track.left,
                track.top,
                track.left + (track.right - track.left) * fraction,
                track.bottom,
            ),
            self.palette.secondary,
            1.0,
        )?;
        top += CAPTURE_ROW_HEIGHT;

        let hotkey_row = rect(inner.left, top, inner.right, top + CAPTURE_ROW_HEIGHT);
        self.capture_row(
            hotkey_row,
            Glyph::Record,
            self.strings.hotkey_label,
            "",
            None,
            false,
        )?;
        let hotkey = hotkey_label(model, self.strings);
        let key_width = self.measure(&hotkey, &self.small) + 14.0;
        let key = rect(
            hotkey_row.right - 12.0 - key_width,
            hotkey_row.top + 8.0,
            hotkey_row.right - 12.0,
            hotkey_row.bottom - 8.0,
        );
        self.tint(key, 0.07, 5.0)?;
        self.text(
            &hotkey,
            key,
            &self.small_center.clone(),
            self.palette.secondary,
        )
    }

    pub(crate) fn capture_row(
        &mut self,
        row: LogicalRect,
        glyph: Glyph,
        label: &str,
        value: &str,
        action: Option<Action>,
        chevron: bool,
    ) -> Result<(), String> {
        let hovered = action
            .as_ref()
            .is_some_and(|action| self.is_hovered(action));
        if hovered {
            self.tint(row, 0.06 * self.hover_amount(1.0), RADIUS_SMALL)?;
        }
        self.glyph(
            glyph,
            rect(
                row.left + 10.0,
                row.top + 10.0,
                row.left + 26.0,
                row.bottom - 10.0,
            ),
            self.palette.muted,
        )?;
        self.text(
            label,
            rect(row.left + 36.0, row.top, row.left + 150.0, row.bottom),
            &self.body.clone(),
            self.palette.primary,
        )?;
        if !value.is_empty() {
            let value_right = if chevron {
                row.right - 28.0
            } else {
                row.right - 12.0
            };
            let value_area = rect(row.left + 136.0, row.top, value_right, row.bottom);
            self.text(
                &self.shorten(value, &self.body, value_area.right - value_area.left),
                value_area,
                &self.body_trailing.clone(),
                self.palette.muted,
            )?;
        }
        if chevron {
            self.glyph(
                Glyph::ChevronRight,
                rect(
                    row.right - 22.0,
                    row.top + 13.0,
                    row.right - 12.0,
                    row.bottom - 13.0,
                ),
                if hovered {
                    self.palette.secondary
                } else {
                    self.palette.muted
                },
            )?;
        }
        if let Some(action) = action {
            self.hits.push(HitRegion { rect: row, action });
        }
        Ok(())
    }
}
