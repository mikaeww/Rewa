//! The clips page: title, tabs, view switch, filter door, search and the grid.

use crate::render::*;

impl Renderer {
    pub(crate) fn render_library(
        &mut self,
        model: &UiModel,
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
    ) -> Result<(), String> {
        let today = crate::clock::now();
        let title_width = self.measure(self.strings.clips, &self.page_title) + 4.0;
        self.text(
            self.strings.clips,
            rect(left, top, left + title_width, top + 32.0),
            &self.page_title.clone(),
            self.palette.primary,
        )?;

        let tabs = [ClipTab::All, ClipTab::Favorites];
        let tab_width = tabs
            .iter()
            .map(|tab| self.measure(tab.label(self.strings), &self.button))
            .fold(0.0_f32, f32::max)
            + 28.0;
        let segments = tabs.map(|tab| {
            (
                Segment::Label(tab.label(self.strings)),
                Action::SetClipTab(tab),
            )
        });
        let tabs_area = rect(
            left + title_width + 18.0,
            top + 3.0,
            left + title_width + 18.0 + tab_width * 2.0 + 4.0,
            top + 29.0,
        );
        self.segmented(
            tabs_area,
            &segments,
            usize::from(model.clip_tab == ClipTab::Favorites),
            "clip_tab",
        )?;
        let tab_left = tabs_area.right;

        let views = [
            (LibraryView::Grid, Glyph::Grid),
            (LibraryView::Compact, Glyph::GridCompact),
        ];
        let current = model.config.appearance.library_view;
        let view = rect(right - 64.0, top + 3.0, right, top + 29.0);
        self.segmented(
            view,
            &views.map(|(view, glyph)| (Segment::Icon(glyph), Action::SetLibraryView(view))),
            views
                .iter()
                .position(|(view, _)| *view == current)
                .unwrap_or(0),
            "library_view",
        )?;
        let filter = rect(view.left - 36.0, top + 3.0, view.left - 10.0, top + 29.0);
        self.door(
            filter,
            Glyph::Filter,
            model.filter_panel_open || model.filters_are_active(),
            Action::ToggleFilterPanel,
        )?;
        let search = rect(
            (filter.left - 8.0 - 220.0).max(tab_left + 16.0),
            top + 2.0,
            filter.left - 8.0,
            top + 30.0,
        );
        if search.right - search.left >= 120.0 {
            self.search_field(model, search, self.strings.search_clips)?;
        }

        let body_top = top + LIBRARY_BODY_OFFSET;
        let selecting = model.selection_mode && !model.selected_clips.is_empty();
        let area = rect(
            left,
            body_top,
            right,
            if selecting { bottom - 60.0 } else { bottom },
        );

        let indices = model.visible_clip_indices_at(usize::MAX, today);
        if indices.is_empty() {
            self.empty_state(
                if !model.search.value.is_empty() {
                    self.strings.empty_no_match
                } else if model.clip_tab == ClipTab::Favorites {
                    self.strings.empty_no_favorites
                } else if model.filters_are_active() {
                    self.strings.empty_no_filter_match
                } else {
                    self.strings.empty_no_clips
                },
                area.left,
                area.right,
                area.top + 8.0,
            )?;
            if model.clips.is_empty()
                && model.search.value.is_empty()
                && !model.filters_are_active()
                && model.clip_tab == ClipTab::All
            {
                self.text(
                    self.strings.first_clip_hint,
                    rect(area.left, area.top + 50.0, area.right, area.top + 80.0),
                    &self.body.clone(),
                    self.palette.secondary,
                )?;
                let action = rect(
                    area.left,
                    area.top + 100.0,
                    area.left + 180.0,
                    area.top + 144.0,
                );
                if model.replay_pending {
                    self.text(
                        self.strings.saving,
                        action,
                        &self.body.clone(),
                        self.palette.secondary,
                    )?;
                } else {
                    self.action_button(action, self.strings.save_clip, Action::SaveReplay)?;
                }
            }
        } else {
            let groups = model.clip_groups(&indices, today);
            let counts = groups
                .iter()
                .map(|group| group.indices.len())
                .collect::<Vec<_>>();
            let layout = library_layout(
                &counts,
                area.right - area.left - CLIP_SCROLL_RESERVE,
                model.config.appearance.library_view,
            );
            let viewport_height = (area.bottom - area.top).max(0.0);
            let overflow = (layout.height - viewport_height).max(0.0);
            let scroll = model.library_scroll.clamp(0.0, overflow);

            self.push_clip(area)?;
            let painted = self.render_clip_sections(model, &groups, &layout, area, scroll, today);
            self.pop_clip();
            painted?;

            if overflow > 0.0 {
                let track = rect(area.right - 5.0, area.top, area.right - 2.0, area.bottom);
                let visible = (viewport_height / layout.height).clamp(0.1, 1.0);
                let thumb_height = viewport_height * visible;
                let thumb_top = area.top + (viewport_height - thumb_height) * (scroll / overflow);
                self.fill(
                    rect(track.left, thumb_top, track.right, thumb_top + thumb_height),
                    self.palette.border,
                    1.5,
                )?;
            }
        }

        if selecting {
            self.selection_toolbar(model, area.left, area.right, bottom - 44.0)?;
        }
        if model.filter_panel_open {
            self.render_filter_panel(model, filter, bottom)?;
        }
        if model.collection_picker_open {
            self.render_collection_picker(model, right, top + 44.0)?;
        }
        Ok(())
    }

    pub(crate) fn render_filter_panel(
        &mut self,
        model: &UiModel,
        anchor: LogicalRect,
        bottom: f32,
    ) -> Result<(), String> {
        let entries: [(&str, String, Action); 5] = [
            (
                self.strings.filter_time,
                model.filter_time.label(self.strings).to_owned(),
                Action::ChooseTimeFilter,
            ),
            (
                self.strings.filter_collection,
                model.filter_collection_label().to_owned(),
                Action::ChooseCollectionFilter,
            ),
            (
                self.strings.filter_type,
                model.filter_type.label(self.strings).to_owned(),
                Action::ChooseTypeFilter,
            ),
            (
                self.strings.filter_size,
                model.filter_size.label(self.strings).to_owned(),
                Action::ChooseSizeFilter,
            ),
            (
                self.strings.filter_sort,
                model.sort_label().to_owned(),
                Action::ChooseClipSort,
            ),
        ];
        let width = FILTER_PANEL_WIDTH;
        let panel_height = (44.0 + entries.len() as f32 * FILTER_ROW_PITCH + 40.0)
            .min((bottom - anchor.bottom - 16.0).max(200.0));
        let panel = rect(
            anchor.right - width,
            anchor.bottom + 8.0,
            anchor.right,
            anchor.bottom + 8.0 + panel_height,
        );
        // the panel floats over the clip grid, so it has to swallow every click
        // inside it before the cards register their own
        self.hits.push(HitRegion {
            rect: panel,
            action: Action::Ignore,
        });
        self.popover_surface(panel)?;
        self.text(
            self.strings.filter_label,
            rect(
                panel.left + 18.0,
                panel.top + 12.0,
                panel.right - 60.0,
                panel.top + 34.0,
            ),
            &self.caption.clone(),
            self.palette.muted,
        )?;
        let active = model.filters_are_active();
        let reset = rect(
            panel.right - 130.0,
            panel.top + 10.0,
            panel.right - 16.0,
            panel.top + 34.0,
        );
        self.text(
            self.strings.reset,
            reset,
            &self.small_right.clone(),
            if !active {
                self.palette.muted
            } else if self.is_hovered(&Action::ResetFilters) {
                self.palette.primary
            } else {
                self.palette.secondary
            },
        )?;
        if active {
            self.hits.push(HitRegion {
                rect: reset,
                action: Action::ResetFilters,
            });
        }

        for (index, (label, value, action)) in entries.into_iter().enumerate() {
            let top = panel.top + 44.0 + index as f32 * FILTER_ROW_PITCH;
            if top + FILTER_ROW_PITCH > panel.bottom {
                break;
            }
            self.text(
                label,
                rect(panel.left + 18.0, top, panel.right - 18.0, top + 18.0),
                &self.small.clone(),
                self.palette.secondary,
            )?;
            self.dropdown(
                rect(
                    panel.left + 18.0,
                    top + 20.0,
                    panel.right - 18.0,
                    top + 48.0,
                ),
                &value,
                action,
            )?;
        }
        Ok(())
    }
}
