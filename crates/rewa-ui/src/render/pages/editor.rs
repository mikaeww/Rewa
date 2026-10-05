//! The trim editor: preview, timeline with keyframe-snapped handles and the cut actions.

use crate::render::*;

impl Renderer {
    pub(crate) fn render_editor(
        &mut self,
        model: &UiModel,
        left: f32,
        right: f32,
        height: f32,
    ) -> Result<(), String> {
        let Some(clip) = model.active_clip() else {
            self.page_toolbar(self.strings.edit_clip, left, right, Action::Back)?;
            self.empty_state(self.strings.clip_unavailable, left, right, PLAYER_TOP)?;
            return Ok(());
        };
        let enabled = model.editor_timing.is_some() && !model.editor_working;
        let undo_enabled = enabled && model.can_undo_editor_trim();
        let redo_enabled = enabled && model.can_redo_editor_trim();
        let top = content_top();
        let save_label = if model.editor_working {
            self.strings.saving
        } else {
            self.strings.save
        };
        let save_width = self.measure(save_label, &self.button) + 28.0;
        let save = rect(right - save_width, top + 2.0, right, top + 30.0);
        self.pill(
            save,
            if enabled {
                self.palette.accent
            } else {
                self.palette.surface
            },
            save_label,
            if enabled {
                self.palette.accent_text
            } else {
                self.palette.muted
            },
            enabled.then_some(if model.trim_replace_original {
                Action::ReplaceCut
            } else {
                Action::SaveCut
            }),
        )?;
        let discard_width = self.measure(self.strings.discard, &self.button) + 28.0;
        let discard = rect(
            save.left - 8.0 - discard_width,
            save.top,
            save.left - 8.0,
            save.bottom,
        );
        self.pill(
            discard,
            self.palette.surface,
            self.strings.discard,
            self.palette.primary,
            Some(Action::Back),
        )?;
        let redo = rect(
            discard.left - 16.0 - 26.0,
            top + 3.0,
            discard.left - 16.0,
            top + 29.0,
        );
        let undo = rect(redo.left - 30.0, redo.top, redo.left - 4.0, redo.bottom);
        for (area, glyph, active, action) in [
            (undo, Glyph::Undo, undo_enabled, Action::UndoEditorTrim),
            (redo, Glyph::Redo, redo_enabled, Action::RedoEditorTrim),
        ] {
            if active {
                self.door(area, glyph, false, action)?;
            } else {
                let center_x = (area.left + area.right) / 2.0;
                let center_y = (area.top + area.bottom) / 2.0;
                self.glyph(
                    glyph,
                    rect(
                        center_x - 8.0,
                        center_y - 8.0,
                        center_x + 8.0,
                        center_y + 8.0,
                    ),
                    mix(self.palette.surface, self.palette.muted, 0.45),
                )?;
            }
        }
        self.page_toolbar(self.strings.edit_clip, left, undo.left - 16.0, Action::Back)?;

        let detail_width = if right - left >= 960.0 { 276.0 } else { 0.0 };
        let stage = video_stage(left, right, height, model);
        // square corners: the editor shows the whole frame, a rounded region would crop it
        self.fill(stage, 0x000000, 0.0)?;
        self.hits.push(HitRegion {
            rect: stage,
            action: Action::PlayPause,
        });
        self.render_media_controls(model, stage, true)?;

        if detail_width > 0.0 {
            let detail_left = right - detail_width;
            let rows = [
                (
                    self.strings.field_start,
                    format_editor_time(model.editor_start),
                ),
                (self.strings.field_end, format_editor_time(model.editor_end)),
                (
                    self.strings.field_length,
                    format_editor_time(model.editor_selected_duration()),
                ),
            ];
            let bottom = self.detail_card(
                detail_left,
                right,
                stage.top,
                self.strings.selection_heading,
                &rows,
                None,
            )?;
            let mode = rect(detail_left, bottom + 24.0, right, bottom + 44.0);
            self.text(
                self.strings.save_as_new,
                mode,
                &self.caption.clone(),
                self.palette.muted,
            )?;
            self.segmented(
                rect(detail_left, mode.bottom + 6.0, right, mode.bottom + 34.0),
                &[
                    (
                        Segment::Label(self.strings.new_clip),
                        Action::SetTrimReplace(false),
                    ),
                    (
                        Segment::Label(self.strings.replace_original),
                        Action::SetTrimReplace(true),
                    ),
                ],
                usize::from(model.trim_replace_original),
                "trim_mode",
            )?;
        }

        let timeline_top = stage.bottom + 92.0;
        let timeline = rect(
            left,
            timeline_top,
            right,
            (timeline_top + EDITOR_TIMELINE_HEIGHT).min(height - 16.0),
        );
        self.timeline_labels(
            model,
            rect(
                timeline.left + 24.0,
                timeline.top + 6.0,
                timeline.right - 24.0,
                timeline.top + 32.0,
            ),
        )?;
        let storyboard = rect(
            timeline.left + 24.0,
            timeline.top + 34.0,
            timeline.right - 24.0,
            timeline.top + 94.0,
        );
        self.trim_storyboard(model, clip, storyboard)?;
        Ok(())
    }

    pub(crate) fn timeline_labels(&self, model: &UiModel, area: LogicalRect) -> Result<(), String> {
        let duration = model
            .editor_timing
            .as_ref()
            .map_or(0.0, |timing| timing.duration.as_secs_f64());
        for step in 0..=6 {
            let x = area.left + (area.right - area.left) * step as f32 / 6.0;
            let label_area = match step {
                0 => rect(x + 10.0, area.top, x + 90.0, area.bottom),
                6 => rect(x - 90.0, area.top, x - 10.0, area.bottom),
                _ => rect(x - 30.0, area.top, x + 50.0, area.bottom),
            };
            self.text(
                &format_player_time(duration * step as f64 / 6.0),
                label_area,
                &self.small.clone(),
                self.palette.secondary,
            )?;
        }
        Ok(())
    }

    /// The trim strip after Photos: the clip as a film strip, the kept range in a
    /// bracket whose sides are the handles, the rest dimmed, the playhead on top.
    pub(crate) fn trim_storyboard(
        &mut self,
        model: &UiModel,
        clip: &rewa_core::clips::Clip,
        area: LogicalRect,
    ) -> Result<(), String> {
        self.push_clip(area)?;
        self.fill(area, self.palette.stage, 0.0)?;
        let tiles = ((area.right - area.left) / 96.0).ceil().max(1.0) as usize;
        let tile_width = (area.right - area.left) / tiles as f32;
        let mut drawn = Ok(());
        for tile in 0..tiles {
            let preview = rect(
                area.left + tile as f32 * tile_width,
                area.top,
                area.left + (tile + 1) as f32 * tile_width - 1.0,
                area.bottom,
            );
            if let Err(error) = self.draw_thumbnail(&clip.path, preview, 0.0) {
                drawn = Err(error);
                break;
            }
        }
        self.pop_clip();
        drawn?;
        let duration = model
            .editor_timing
            .as_ref()
            .map_or(0.0, |timing| timing.duration.as_secs_f64());
        if duration <= 0.0 {
            return Ok(());
        }
        let x_at = |seconds: f64| {
            area.left + (area.right - area.left) * (seconds / duration).clamp(0.0, 1.0) as f32
        };
        let start_x = x_at(model.editor_start.as_secs_f64());
        let end_x = x_at(model.editor_end.as_secs_f64());
        let playhead_x = x_at(model.player_position_seconds);
        self.fill_alpha(
            rect(area.left, area.top, start_x, area.bottom),
            0x000000,
            0.6,
            0.0,
        )?;
        self.fill_alpha(
            rect(end_x, area.top, area.right, area.bottom),
            0x000000,
            0.6,
            0.0,
        )?;

        const HANDLE: f32 = 12.0;
        const BAR: f32 = 3.0;
        let bracket = rect(
            start_x - HANDLE,
            area.top - BAR,
            end_x + HANDLE,
            area.bottom + BAR,
        );
        let ink = self.palette.primary;
        self.fill(rect(start_x, bracket.top, end_x, area.top), ink, 0.0)?;
        self.fill(rect(start_x, area.bottom, end_x, bracket.bottom), ink, 0.0)?;
        let center = (area.top + area.bottom) / 2.0;
        for (handle, action) in [
            (
                rect(bracket.left, bracket.top, start_x, bracket.bottom),
                Action::DragEditorStart,
            ),
            (
                rect(end_x, bracket.top, bracket.right, bracket.bottom),
                Action::DragEditorEnd,
            ),
        ] {
            let hovered = self.is_hovered(&action);
            self.fill(
                handle,
                if hovered {
                    mix(ink, self.palette.secondary, 0.35)
                } else {
                    ink
                },
                4.0,
            )?;
            let grip = (handle.left + handle.right) / 2.0;
            self.fill(
                rect(grip - 1.0, center - 9.0, grip + 1.0, center + 9.0),
                self.palette.surface,
                1.0,
            )?;
        }

        self.fill(
            rect(
                playhead_x - 1.0,
                area.top - 6.0,
                playhead_x + 1.0,
                area.bottom + 6.0,
            ),
            0xffffff,
            1.0,
        )?;
        self.fill(
            rect(
                playhead_x - 5.0,
                area.top - 12.0,
                playhead_x + 5.0,
                area.top - 2.0,
            ),
            0xffffff,
            5.0,
        )?;
        self.hits.push(HitRegion {
            rect: rect(area.left, area.top - 20.0, area.right, area.bottom + 20.0),
            action: Action::DragEditorPlayhead,
        });
        for (x, action) in [
            (start_x - HANDLE / 2.0, Action::DragEditorStart),
            (end_x + HANDLE / 2.0, Action::DragEditorEnd),
        ] {
            self.hits.push(HitRegion {
                rect: rect(x - 12.0, bracket.top - 6.0, x + 12.0, bracket.bottom + 6.0),
                action,
            });
        }
        Ok(())
    }
}
