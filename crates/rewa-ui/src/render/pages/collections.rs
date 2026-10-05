//! The collections page: the folder column with its divider and the clips of the
//! chosen folder or game.

use crate::render::*;

impl Renderer {
    pub(crate) fn render_collections(
        &mut self,
        model: &UiModel,
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
    ) -> Result<(), String> {
        let today = crate::clock::now();
        let title_width = self.measure(self.strings.collections, &self.page_title) + 4.0;
        self.text(
            self.strings.collections,
            rect(left, top, left + title_width, top + 32.0),
            &self.page_title.clone(),
            self.palette.primary,
        )?;
        let column_width = folder_column_width(model);
        self.door(
            rect(
                left + title_width + 12.0,
                top + 3.0,
                left + title_width + 38.0,
                top + 29.0,
            ),
            Glyph::Sidebar,
            column_width == 0.0,
            Action::ToggleFolderColumn,
        )?;

        let create_width = self.measure(self.strings.new_collection_button, &self.button) + 28.0;
        let create = rect(right - create_width, top + 2.0, right, top + 30.0);
        self.action_button(
            create,
            self.strings.new_collection_button,
            Action::CreateCollection,
        )?;

        let body_top = top + LIBRARY_BODY_OFFSET;
        let area_left = folder_area_left(model, left);
        if column_width > 0.0 {
            // the search lives in the column it filters: games and folders by name
            let column = rect(left, body_top, left + column_width, bottom);
            self.search_field(
                model,
                rect(
                    column.left,
                    column.top - 6.0,
                    column.right,
                    column.top + 22.0,
                ),
                self.strings.search_collections,
            )?;
            self.render_folder_column(
                model,
                rect(
                    column.left,
                    column.top + FOLDER_SEARCH_HEIGHT,
                    column.right,
                    column.bottom,
                ),
            )?;
            let divider = Action::DragFolderDivider;
            let lit = self.is_hovered(&divider);
            self.fill(
                rect(
                    column.right + 12.0,
                    body_top - 4.0,
                    column.right + if lit { 14.0 } else { 13.0 },
                    bottom,
                ),
                if lit {
                    self.palette.secondary
                } else {
                    self.palette.border
                },
                0.0,
            )?;
            self.hits.push(HitRegion {
                rect: rect(
                    column.right + 7.0,
                    body_top - 4.0,
                    column.right + 19.0,
                    bottom,
                ),
                action: divider,
            });
        } else {
            let search = rect(
                (create.left - 10.0 - 240.0).max(left + title_width + 50.0),
                top + 2.0,
                create.left - 10.0,
                top + 30.0,
            );
            if search.right - search.left >= 120.0 {
                self.search_field(model, search, self.strings.search_collections)?;
            }
        }

        let active = model.active_collection.as_ref().and_then(|path| {
            model
                .collections
                .iter()
                .find(|collection| &collection.path == path)
        });
        let title = model.active_game.as_deref().unwrap_or_else(|| {
            active.map_or(self.strings.all_clips, |collection| {
                collection.name.as_str()
            })
        });
        self.text(
            &self.shorten(title, &self.heading, right - area_left - 220.0),
            rect(area_left, body_top - 6.0, right - 220.0, body_top + 22.0),
            &self.heading.clone(),
            self.palette.primary,
        )?;
        if active.is_some() {
            let mut quick_right = right;
            for (label, action, destructive) in [
                (self.strings.delete, Action::DeleteActiveCollection, true),
                (self.strings.rename, Action::RenameActiveCollection, false),
            ] {
                let width = self.measure(label, &self.small) + 18.0;
                let area = rect(
                    quick_right - width,
                    body_top - 3.0,
                    quick_right,
                    body_top + 19.0,
                );
                self.quick_button(area, label, action, destructive)?;
                quick_right = area.left - 6.0;
            }
        }
        let clips_top = body_top + 34.0;
        let selecting = model.selection_mode && !model.selected_clips.is_empty();
        let area = rect(
            area_left,
            clips_top,
            right,
            if selecting { bottom - 60.0 } else { bottom },
        );
        let indices = model.visible_clip_indices_at(usize::MAX, today);
        if indices.is_empty() {
            self.empty_state(
                if active.is_some() || model.active_game.is_some() {
                    self.strings.empty_collection
                } else {
                    self.strings.empty_no_clips
                },
                area.left,
                area.right,
                area.top + 8.0,
            )?;
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
                let visible = (viewport_height / layout.height).clamp(0.1, 1.0);
                let thumb_height = viewport_height * visible;
                let thumb_top = area.top + (viewport_height - thumb_height) * (scroll / overflow);
                self.fill(
                    rect(
                        area.right - 5.0,
                        thumb_top,
                        area.right - 2.0,
                        thumb_top + thumb_height,
                    ),
                    self.palette.border,
                    1.5,
                )?;
            }
        }
        if selecting {
            self.selection_toolbar(model, area.left, area.right, bottom - 44.0)?;
        }
        if model.collection_picker_open {
            self.render_collection_picker(model, right, top + 44.0)?;
        }
        Ok(())
    }

    pub(crate) fn render_folder_column(
        &mut self,
        model: &UiModel,
        area: LogicalRect,
    ) -> Result<(), String> {
        let entries = folder_entries(model);
        let overflow = folder_column_overflow_in(area, &entries);
        let scroll = model.folder_scroll.clamp(0.0, overflow);
        self.push_clip(area)?;
        let painted = self.render_folder_rows(model, area, scroll, &entries);
        self.pop_clip();
        painted?;
        if overflow > 0.0 {
            let height = area.bottom - area.top;
            let visible = (height / (height + overflow)).clamp(0.1, 1.0);
            let thumb = height * visible;
            let top = area.top + (height - thumb) * (scroll / overflow);
            self.fill(
                rect(area.right - 3.0, top, area.right - 1.0, top + thumb),
                self.palette.border,
                1.5,
            )?;
        }
        Ok(())
    }

    pub(crate) fn render_folder_rows(
        &mut self,
        model: &UiModel,
        area: LogicalRect,
        scroll: f32,
        entries: &[FolderEntry],
    ) -> Result<(), String> {
        let dragging = model.clip_drag_preview.as_ref();
        // the active row's pill glides between folders like the sidebar's
        let mut offset = 0.0;
        let mut active_offset = None;
        for entry in entries {
            if matches!(entry, FolderEntry::Row { active: true, .. }) {
                active_offset = Some(offset);
            }
            offset += entry.pitch();
        }
        if let Some(target) = active_offset {
            let now = Instant::now();
            let reduced = self.reduced_motion;
            let motion = self
                .toggle_motions
                .entry("collection_row")
                .or_insert_with(|| {
                    crate::motion::Motion::with_curve(target, crate::motion::Curve::Glide)
                });
            motion.retarget(target, now, reduced);
            let top = area.top - scroll + motion.value(now);
            self.navigation_was_moving |= motion.active(now);
            self.fill(
                rect(area.left, top, area.right, top + FOLDER_ROW_HEIGHT),
                self.palette.surface_raised,
                RADIUS_SMALL,
            )?;
        }
        let mut top = area.top - scroll;
        for entry in entries {
            let pitch = entry.pitch();
            if top > area.bottom {
                break;
            }
            if top + pitch >= area.top {
                match entry {
                    FolderEntry::Caption(label) => self.text(
                        label,
                        rect(area.left + 10.0, top + 10.0, area.right, top + pitch),
                        &self.caption.clone(),
                        self.palette.muted,
                    )?,
                    FolderEntry::Note(label) => self.text(
                        label,
                        rect(area.left + 10.0, top, area.right, top + FOLDER_ROW_HEIGHT),
                        &self.small.clone(),
                        self.palette.muted,
                    )?,
                    FolderEntry::Row {
                        glyph,
                        label,
                        action,
                        active,
                    } => {
                        let drop_target = dragging.is_some_and(|drag| {
                            matches!(action, Action::SelectCollection(Some(index))
                                if drag.target_collection == Some(*index))
                        });
                        self.folder_row(
                            rect(area.left, top, area.right, top + FOLDER_ROW_HEIGHT),
                            area,
                            *glyph,
                            label,
                            *active,
                            drop_target,
                            action.clone(),
                        )?;
                    }
                }
            }
            top += pitch;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn folder_row(
        &mut self,
        area: LogicalRect,
        viewport: LogicalRect,
        glyph: Glyph,
        name: &str,
        active: bool,
        drop_target: bool,
        action: Action,
    ) -> Result<(), String> {
        let hovered = !active && self.is_hovered(&action);
        if drop_target {
            self.tint(area, 0.08, RADIUS_SMALL)?;
            self.stroke(area, self.palette.primary, RADIUS_SMALL, 1.0)?;
        } else if hovered {
            self.tint(area, 0.05 * self.hover_amount(1.0), RADIUS_SMALL)?;
        }
        let tone = if active {
            self.palette.primary
        } else if hovered {
            self.palette.secondary
        } else {
            self.palette.muted
        };
        let center = (area.top + area.bottom) / 2.0;
        self.glyph(
            glyph,
            rect(
                area.left + 10.0,
                center - 7.5,
                area.left + 25.0,
                center + 7.5,
            ),
            tone,
        )?;
        self.text(
            &self.shorten(name, &self.body, area.right - area.left - 44.0),
            rect(area.left + 34.0, area.top, area.right - 10.0, area.bottom),
            &self.body.clone(),
            if active {
                self.palette.primary
            } else {
                mix(self.palette.muted, self.palette.primary, 0.35)
            },
        )?;
        self.push_clipped_hit(area, viewport, action);
        Ok(())
    }

    /// Leech's quick button: a small wash pill for secondary row actions.
    pub(crate) fn quick_button(
        &mut self,
        area: LogicalRect,
        label: &str,
        action: Action,
        destructive: bool,
    ) -> Result<(), String> {
        let hovered = self.is_hovered(&action);
        self.tint(
            area,
            if hovered { 0.12 } else { 0.07 },
            (area.bottom - area.top) / 2.0,
        )?;
        self.text(
            label,
            area,
            &self.small_center.clone(),
            if destructive {
                self.palette.destructive
            } else {
                self.palette.primary
            },
        )?;
        self.hits.push(HitRegion { rect: area, action });
        Ok(())
    }
}
