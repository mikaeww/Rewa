//! Bars over the clip pages: the search field, the selection toolbar and the
//! collection picker.

use crate::render::*;

impl Renderer {
    pub(crate) fn search_field(
        &mut self,
        model: &UiModel,
        area: LogicalRect,
        placeholder: &str,
    ) -> Result<(), String> {
        let action = Action::Search;
        let hovered = self.is_hovered(&action);
        self.tint(
            area,
            if model.search_focused {
                0.09
            } else if hovered {
                0.06 + 0.02 * self.hover_amount(1.0)
            } else {
                0.06
            },
            RADIUS_SMALL,
        )?;
        if model.search_focused {
            self.stroke(
                area,
                mix(self.palette.surface, self.palette.primary, 0.35),
                RADIUS_SMALL,
                1.0,
            )?;
        }
        let center_y = (area.top + area.bottom) / 2.0;
        self.glyph(
            Glyph::Search,
            rect(
                area.left + 10.0,
                center_y - 7.0,
                area.left + 24.0,
                center_y + 7.0,
            ),
            self.palette.muted,
        )?;
        self.render_text_input(
            &model.search,
            rect(area.left + 32.0, area.top, area.right - 10.0, area.bottom),
            placeholder,
            model.search_focused,
            TextInputTarget::Search,
        )?;
        self.hits.push(HitRegion { rect: area, action });
        Ok(())
    }

    pub(crate) fn selection_toolbar(
        &mut self,
        model: &UiModel,
        left: f32,
        right: f32,
        top: f32,
    ) -> Result<(), String> {
        let selected = model.selected_clips.len();
        let movable = selected > 0 && !model.collections.is_empty();
        let move_label = self.strings.move_button(selected);
        let count = self.strings.selected_count(selected);
        let widths = [
            self.measure(self.strings.cancel, &self.button) + 26.0,
            self.measure(self.strings.select_all, &self.button) + 26.0,
            self.measure(&move_label, &self.button) + 26.0,
        ];
        let bar_width =
            (self.measure(&count, &self.body) + 40.0 + widths.iter().sum::<f32>() + 16.0)
                .min(right - left);
        let center = (left + right) / 2.0;
        let bar = rect(
            center - bar_width / 2.0,
            top,
            center + bar_width / 2.0,
            top + 44.0,
        );
        self.popover_surface(bar)?;
        self.text(
            &count,
            rect(bar.left + 18.0, bar.top, bar.right, bar.bottom),
            &self.body.clone(),
            self.palette.secondary,
        )?;
        let mut x = bar.right - 7.0;
        let buttons = [
            (
                widths[2],
                &move_label,
                if movable {
                    self.palette.accent
                } else {
                    self.palette.surface
                },
                if movable {
                    self.palette.accent_text
                } else {
                    self.palette.muted
                },
                movable.then_some(Action::ToggleCollectionPicker),
            ),
            (
                widths[1],
                &self.strings.select_all.to_owned(),
                self.palette.surface,
                self.palette.primary,
                Some(Action::SelectAllVisibleClips),
            ),
            (
                widths[0],
                &self.strings.cancel.to_owned(),
                self.palette.surface,
                self.palette.secondary,
                Some(Action::ToggleSelectionMode),
            ),
        ];
        for (width, label, background, foreground, action) in buttons {
            let area = rect(x - width, bar.top + 7.0, x, bar.bottom - 7.0);
            self.pill(area, background, label, foreground, action)?;
            x = area.left - 6.0;
        }
        Ok(())
    }

    pub(crate) fn render_collection_picker(
        &mut self,
        model: &UiModel,
        right: f32,
        top: f32,
    ) -> Result<(), String> {
        let visible = model.collections.len().min(8);
        let width = 248.0;
        let row_height = 30.0;
        let area = rect(
            right - width,
            top,
            right,
            top + 40.0 + visible as f32 * row_height + 8.0,
        );
        self.popover_surface(area)?;
        self.text(
            self.strings.move_selected_to,
            rect(
                area.left + 14.0,
                area.top + 4.0,
                area.right - 14.0,
                area.top + 38.0,
            ),
            &self.caption.clone(),
            self.palette.muted,
        )?;
        for (index, collection) in model.collections.iter().take(visible).enumerate() {
            let action = Action::MoveSelectedToCollection(index);
            let row = rect(
                area.left + 8.0,
                area.top + 38.0 + index as f32 * row_height,
                area.right - 8.0,
                area.top + 38.0 + (index + 1) as f32 * row_height,
            );
            if self.is_hovered(&action) {
                self.tint(row, 0.08, RADIUS_SMALL - 2.0)?;
            }
            self.text(
                &collection.name,
                rect(row.left + 10.0, row.top, row.right - 42.0, row.bottom),
                &self.body.clone(),
                self.palette.primary,
            )?;
            self.text(
                &collection.clip_count.to_string(),
                rect(row.right - 32.0, row.top, row.right - 8.0, row.bottom),
                &self.small.clone(),
                self.palette.secondary,
            )?;
            self.hits.push(HitRegion { rect: row, action });
        }
        Ok(())
    }
}
