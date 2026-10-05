//! The clip preview: video stage, playback row, volume, information panel and the
//! fullscreen header and controls.

use crate::render::*;

impl Renderer {
    pub(crate) fn render_player(
        &mut self,
        model: &UiModel,
        left: f32,
        right: f32,
        height: f32,
    ) -> Result<(), String> {
        let title = model
            .active_clip()
            .map_or(self.strings.preview_title, |clip| clip.title.as_str());
        let detail_width = if right - left >= 960.0 { 276.0 } else { 0.0 };
        // edit and delete sit together: under the details, or in the title row when
        // the window is too narrow for the details column
        let actions_left = if detail_width > 0.0 {
            None
        } else {
            Some(right - self.clip_actions_width())
        };
        self.page_toolbar(
            title,
            left,
            actions_left.map_or(right, |x| x - 16.0),
            Action::Back,
        )?;
        if let (Some(x), Some(index)) = (actions_left, model.active_clip) {
            self.clip_actions(x, content_top() + 2.0, index)?;
        }
        let Some(clip) = model.active_clip() else {
            self.empty_state(self.strings.clip_unavailable, left, right, PLAYER_TOP)?;
            return Ok(());
        };
        let stage = video_stage(left, right, height, model);
        // square like the child window on top of it; a window region cannot be antialiased
        self.fill(stage, 0x000000, 0.0)?;
        self.hits.push(HitRegion {
            rect: stage,
            action: Action::PlayPause,
        });
        self.render_media_controls(model, stage, false)?;
        let center = (stage.top + stage.bottom) / 2.0;
        for (offset, glyph, action, area) in [
            (
                -1,
                Glyph::ChevronLeft,
                Action::PreviousClip,
                rect(
                    stage.left - 42.0,
                    center - 17.0,
                    stage.left - 8.0,
                    center + 17.0,
                ),
            ),
            (
                1,
                Glyph::ChevronRight,
                Action::NextClip,
                rect(
                    stage.right + 8.0,
                    center - 17.0,
                    stage.right + 42.0,
                    center + 17.0,
                ),
            ),
        ] {
            if model.adjacent_clip(offset).is_some() {
                self.door(area, glyph, false, action)?;
            }
        }

        if detail_width > 0.0 {
            let detail_left = right - detail_width;
            let bottom = self.clip_information_panel(model, clip, detail_left, right, stage.top)?;
            if let Some(index) = model.active_clip {
                self.clip_actions(detail_left, bottom + 14.0, index)?;
            }
        }
        Ok(())
    }

    pub(crate) fn clip_actions_width(&self) -> f32 {
        [self.strings.edit_clip, self.strings.delete_clip]
            .iter()
            .map(|label| self.measure(label, &self.small_center) + 28.0)
            .sum::<f32>()
            + 8.0
    }

    /// The preview's two clip actions as quick pills, edit first.
    pub(crate) fn clip_actions(&mut self, left: f32, top: f32, index: usize) -> Result<(), String> {
        let mut x = left;
        for (label, action, destructive) in [
            (self.strings.edit_clip, Action::EditActiveClip, false),
            (self.strings.delete_clip, Action::DeleteClip(index), true),
        ] {
            let width = self.measure(label, &self.small_center) + 28.0;
            self.quick_button(
                rect(x, top, x + width, top + 28.0),
                label,
                action,
                destructive,
            )?;
            x += width + 8.0;
        }
        Ok(())
    }

    /// Back chevron and page title in one row, the macOS navigation bar.
    pub(crate) fn page_toolbar(
        &mut self,
        title: &str,
        left: f32,
        right: f32,
        back: Action,
    ) -> Result<(), String> {
        let top = content_top();
        self.door(
            rect(left - 4.0, top + 3.0, left + 22.0, top + 29.0),
            Glyph::ChevronLeft,
            false,
            back,
        )?;
        self.text(
            &self.shorten(title, &self.page_title, right - left - 36.0),
            rect(left + 34.0, top, right, top + 32.0),
            &self.page_title.clone(),
            self.palette.primary,
        )
    }

    pub(crate) fn render_media_controls(
        &mut self,
        model: &UiModel,
        stage: LogicalRect,
        editor: bool,
    ) -> Result<(), String> {
        let center = stage.bottom + PLAYBACK_ROW_CENTER;
        if !editor {
            return self.playback_row(model, stage.left, stage.right, center, false);
        }
        // the storyboard below carries the editor's playhead
        self.floating_glyph(
            rect(
                stage.left - 6.0,
                center - 18.0,
                stage.left + 26.0,
                center + 18.0,
            ),
            if model.player_playing {
                Glyph::Pause
            } else {
                Glyph::Play
            },
            self.palette.primary,
            Some(Action::PlayPause),
        )?;
        self.text(
            &format!(
                "{} / {}",
                format_player_time(model.player_position_seconds),
                format_player_time(model.player_duration_seconds)
            ),
            rect(
                stage.left + 34.0,
                center - 18.0,
                stage.left + 200.0,
                center + 18.0,
            ),
            &self.small.clone(),
            self.palette.muted,
        )
    }

    pub(crate) fn draw_progress_rail(
        &self,
        model: &UiModel,
        rail: LogicalRect,
        hovered: bool,
    ) -> Result<(), String> {
        let progress = if model.player_duration_seconds > 0.0 {
            (model.player_position_seconds / model.player_duration_seconds).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        self.slider(rail, progress, hovered)
    }

    /// One slider for seeking and levels: a 4 px track, the filled part in ink and
    /// a white knob that grows while the pointer is on it.
    pub(crate) fn slider(
        &self,
        track: LogicalRect,
        fraction: f32,
        hovered: bool,
    ) -> Result<(), String> {
        let center = (track.top + track.bottom) / 2.0;
        let track = rect(track.left, center - 2.0, track.right, center + 2.0);
        self.tint(track, 0.12, 2.0)?;
        let x = track.left + (track.right - track.left) * fraction.clamp(0.0, 1.0);
        if x > track.left + 1.0 {
            self.fill(
                rect(track.left, track.top, x, track.bottom),
                mix(self.palette.surface, self.palette.primary, 0.82),
                2.0,
            )?;
        }
        let radius = if hovered { 8.0 } else { 6.5 };
        let knob = rect(x - radius, center - radius, x + radius, center + radius);
        for (spread, alpha) in [(2.0, 0.08), (1.0, 0.16)] {
            self.fill_alpha(
                rect(
                    knob.left - spread,
                    knob.top - spread + 1.0,
                    knob.right + spread,
                    knob.bottom + spread + 1.0,
                ),
                0x000000,
                alpha,
                radius + spread,
            )?;
        }
        self.fill(knob, 0xffffff, radius)?;
        self.stroke(knob, 0xd9d9d9, radius, 0.5)
    }

    /// The clip's facts as macOS shows them in Get Info; returns the card's foot.
    pub(crate) fn clip_information_panel(
        &mut self,
        model: &UiModel,
        clip: &rewa_core::clips::Clip,
        left: f32,
        right: f32,
        top: f32,
    ) -> Result<f32, String> {
        let resolution = if model.player_video_width > 0 && model.player_video_height > 0 {
            format!("{}×{}", model.player_video_width, model.player_video_height)
        } else {
            self.strings.loading.to_owned()
        };
        let rows = [
            (self.strings.field_title, clip.title.clone()),
            (
                self.strings.field_created,
                crate::clock::stamp_label(
                    crate::clock::local(clip.modified),
                    crate::clock::now(),
                    self.strings,
                ),
            ),
            (
                self.strings.field_duration,
                format_player_time(model.player_duration_seconds),
            ),
            (self.strings.field_size, format_bytes(clip.size_bytes)),
            (self.strings.field_resolution, resolution),
        ];
        self.detail_card(
            left,
            right,
            top,
            self.strings.clip_information,
            &rows,
            Some(Action::RenameActiveClip),
        )
    }

    /// A captioned hairline card of label/value lines; the first line may carry a
    /// pencil. Returns the card's foot.
    pub(crate) fn detail_card(
        &mut self,
        left: f32,
        right: f32,
        top: f32,
        heading: &str,
        rows: &[(&str, String)],
        first_action: Option<Action>,
    ) -> Result<f32, String> {
        const LINE: f32 = 38.0;
        self.text(
            heading,
            rect(left + 2.0, top - 2.0, right, top + 18.0),
            &self.caption.clone(),
            self.palette.muted,
        )?;
        let card = rect(
            left,
            top + 26.0,
            right,
            top + 26.0 + rows.len() as f32 * LINE,
        );
        self.stroke(card, self.palette.border, 11.0, 1.0)?;
        let label_width = rows
            .iter()
            .map(|(label, _)| self.measure(label, &self.body))
            .fold(0.0_f32, f32::max);
        for (index, (label, value)) in rows.iter().enumerate() {
            let line = rect(
                card.left,
                card.top + index as f32 * LINE,
                card.right,
                card.top + (index + 1) as f32 * LINE,
            );
            if index > 0 {
                self.fill(
                    rect(line.left + 14.0, line.top, line.right, line.top + 1.0),
                    self.palette.hairline,
                    0.0,
                )?;
            }
            self.text(
                label,
                rect(line.left + 14.0, line.top, line.right - 14.0, line.bottom),
                &self.body.clone(),
                self.palette.muted,
            )?;
            let action = first_action.clone().filter(|_| index == 0);
            let value_right = if action.is_some() {
                line.right - 40.0
            } else {
                line.right - 14.0
            };
            let value_left = line.left + 14.0 + label_width + 16.0;
            self.text(
                &self.shorten(value, &self.body, value_right - value_left),
                rect(value_left, line.top, value_right, line.bottom),
                &self.body_trailing.clone(),
                self.palette.primary,
            )?;
            if let Some(action) = action {
                let center = (line.top + line.bottom) / 2.0;
                self.door(
                    rect(
                        line.right - 36.0,
                        center - 13.0,
                        line.right - 10.0,
                        center + 13.0,
                    ),
                    Glyph::Pencil,
                    false,
                    action,
                )?;
            }
        }
        Ok(card.bottom)
    }

    pub(crate) fn render_fullscreen_header(&mut self, width: f32) -> Result<(), String> {
        self.fill(
            rect(0.0, 0.0, width, FULLSCREEN_HEADER_HEIGHT),
            0x000000,
            0.0,
        )?;
        let back_width = self.measure(self.strings.back_to_preview, &self.button) + 48.0;
        let back = rect(18.0, 22.0, 18.0 + back_width, 52.0);
        self.pill(
            back,
            self.palette.surface,
            "",
            self.palette.primary,
            Some(Action::ToggleFullscreen),
        )?;
        self.glyph(
            Glyph::ChevronLeft,
            rect(
                back.left + 10.0,
                back.top + 8.0,
                back.left + 24.0,
                back.bottom - 8.0,
            ),
            self.palette.primary,
        )?;
        self.text(
            self.strings.back_to_preview,
            rect(back.left + 30.0, back.top, back.right - 12.0, back.bottom),
            &self.button_leading.clone(),
            self.palette.primary,
        )?;
        let hint_width = self.measure(self.strings.original_size_hint, &self.button) + 28.0;
        self.pill(
            rect(width - 18.0 - hint_width, 22.0, width - 18.0, 52.0),
            self.palette.surface,
            self.strings.original_size_hint,
            self.palette.primary,
            Some(Action::ToggleFullscreen),
        )
    }

    pub(crate) fn render_fullscreen_controls(
        &mut self,
        model: &UiModel,
        width: u32,
        height: u32,
        picture: LogicalRect,
    ) -> Result<(), String> {
        let width = width as f32;
        let height = height as f32;
        self.fill(rect(0.0, 0.0, width, height), 0x000000, 0.0)?;
        self.playback_row(model, picture.left, picture.right, height / 2.0, true)
    }

    /// Play at the picture's left edge, the fullscreen door at its right edge, time
    /// and seeking between; the same row under the preview and in fullscreen.
    pub(crate) fn playback_row(
        &mut self,
        model: &UiModel,
        left: f32,
        right: f32,
        center: f32,
        fullscreen: bool,
    ) -> Result<(), String> {
        // floating glyphs are 20 px wide and centred in their area, so an area
        // centred 10 px in from an edge puts the glyph flush with it
        self.floating_glyph(
            rect(left - 6.0, center - 18.0, left + 26.0, center + 18.0),
            if model.player_playing {
                Glyph::Pause
            } else {
                Glyph::Play
            },
            self.palette.primary,
            Some(Action::PlayPause),
        )?;
        self.text(
            &format!(
                "{} / {}",
                format_player_time(model.player_position_seconds),
                format_player_time(model.player_duration_seconds)
            ),
            rect(
                left + 34.0,
                center - 18.0,
                left + PLAYER_RAIL_LEFT - 8.0,
                center + 18.0,
            ),
            &self.small.clone(),
            self.palette.muted,
        )?;
        let rail = playback_rail(left, right, center, fullscreen);
        let seek = Action::DragPlayerSeek;
        let hovered = self.is_hovered(&seek);
        self.draw_progress_rail(model, rail, hovered)?;
        self.hits.push(HitRegion {
            rect: rect(
                rail.left - 6.0,
                center - 16.0,
                rail.right + 6.0,
                center + 16.0,
            ),
            action: seek,
        });
        if fullscreen {
            self.floating_glyph(
                rect(right - 62.0, center - 18.0, right - 30.0, center + 18.0),
                if model.player_volume_percent == 0 {
                    Glyph::Muted
                } else {
                    Glyph::Audio
                },
                self.palette.primary,
                Some(Action::ToggleMute),
            )?;
        }
        self.floating_glyph(
            rect(right - 26.0, center - 18.0, right + 6.0, center + 18.0),
            Glyph::Fullscreen,
            if fullscreen {
                self.palette.primary
            } else {
                self.palette.muted
            },
            Some(Action::ToggleFullscreen),
        )
    }
}
