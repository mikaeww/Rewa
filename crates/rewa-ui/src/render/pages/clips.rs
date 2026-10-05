//! Clip cards in day sections, with their hover buttons and the filter dropdowns,
//! and the grid arithmetic that places them.

use crate::render::*;

impl Renderer {
    pub(crate) fn render_clip_sections(
        &mut self,
        model: &UiModel,
        groups: &[ClipGroup],
        layout: &LibraryLayout,
        area: LogicalRect,
        scroll: f32,
        today: crate::clock::Civil,
    ) -> Result<(), String> {
        for (section, group) in groups.iter().enumerate() {
            let header_top = area.top + layout.sections[section] - scroll;
            let rows_top = header_top + layout.header;
            if rows_top > area.bottom {
                break;
            }
            if layout.header > 0.0 && header_top + layout.header > area.top {
                self.text(
                    &group.label,
                    rect(area.left, header_top, area.left + 300.0, header_top + 26.0),
                    &self.section.clone(),
                    self.palette.primary,
                )?;
            }
            for (position, index) in group.indices.iter().copied().enumerate() {
                let row = position / layout.columns;
                let column = position % layout.columns;
                let card_top = rows_top + row as f32 * layout.row_pitch;
                if card_top > area.bottom {
                    break;
                }
                if card_top + layout.card_height < area.top {
                    continue;
                }
                let card_left = area.left + column as f32 * (layout.card_width + layout.gap);
                let card = rect(
                    card_left,
                    card_top,
                    card_left + layout.card_width,
                    card_top + layout.card_height,
                );
                self.clip_card(model, index, card, area, today)?;
            }
        }
        Ok(())
    }

    pub(crate) fn clip_card(
        &mut self,
        model: &UiModel,
        index: usize,
        card: LogicalRect,
        viewport: LogicalRect,
        today: crate::clock::Civil,
    ) -> Result<(), String> {
        let Some(clip) = model.clips.get(index) else {
            return Ok(());
        };
        let open = if model.selection_mode {
            Action::ToggleClipSelection(index)
        } else {
            Action::OpenClip(index)
        };
        let favorite = Action::ToggleFavorite(index);
        let external = Action::OpenClipExternally(index);
        let menu = Action::OpenClipMenu(index);
        let selected = model.clip_is_selected(index);
        let starred = model.is_favorite(index);
        let hovered = [&open, &favorite, &external, &menu]
            .into_iter()
            .any(|action| self.is_hovered(action));

        // the pointer target for the card is registered first so the overlay
        // buttons drawn on top of it keep their own targets
        self.push_clipped_hit(card, viewport, open);

        let compact = model.config.appearance.library_view == LibraryView::Compact;
        let resting = rect(
            card.left,
            card.top,
            card.right,
            card.bottom
                - if compact {
                    COMPACT_META_HEIGHT
                } else {
                    CLIP_META_HEIGHT
                },
        );
        let preview = resting;
        if selected {
            self.stroke(
                rect(
                    preview.left - 3.0,
                    preview.top - 3.0,
                    preview.right + 3.0,
                    preview.bottom + 3.0,
                ),
                self.palette.primary,
                RADIUS + 3.0,
                2.0,
            )?;
        }
        self.fill(preview, self.palette.stage, RADIUS)?;
        if !self.draw_thumbnail(&clip.path, preview, RADIUS)? {
            self.glyph(
                Glyph::Play,
                rect(
                    (preview.left + preview.right) / 2.0 - 10.0,
                    (preview.top + preview.bottom) / 2.0 - 10.0,
                    (preview.left + preview.right) / 2.0 + 10.0,
                    (preview.top + preview.bottom) / 2.0 + 10.0,
                ),
                self.palette.muted,
            )?;
        }
        self.stroke(
            preview,
            mix(self.palette.stage, self.palette.primary, 0.08),
            RADIUS,
            1.0,
        )?;

        if let Some(duration) = self.clip_duration(&clip.path) {
            let label = format_clip_badge_duration(duration);
            let badge_width = self.measure(&label, &self.small_center) + 12.0;
            let badge = rect(
                preview.right - 8.0 - badge_width,
                preview.bottom - 26.0,
                preview.right - 8.0,
                preview.bottom - 8.0,
            );
            self.fill_alpha(badge, 0x000000, 0.55, 5.0)?;
            self.text(&label, badge, &self.small_center.clone(), 0xffffff)?;
        }

        if model.selection_mode {
            let check = rect(
                preview.right - 30.0,
                preview.top + 8.0,
                preview.right - 8.0,
                preview.top + 30.0,
            );
            if selected {
                self.fill(check, self.palette.accent, 11.0)?;
                self.glyph(
                    Glyph::Check,
                    rect(
                        check.left + 4.0,
                        check.top + 4.0,
                        check.right - 4.0,
                        check.bottom - 4.0,
                    ),
                    self.palette.accent_text,
                )?;
            } else {
                self.fill_alpha(check, 0x000000, 0.35, 11.0)?;
                self.stroke(check, 0xffffff, 11.0, 1.5)?;
            }
        } else if hovered {
            let star = rect(
                preview.right - 34.0,
                preview.top + 8.0,
                preview.right - 8.0,
                preview.top + 34.0,
            );
            self.overlay_button(
                star,
                if starred {
                    Glyph::StarFilled
                } else {
                    Glyph::Star
                },
                favorite,
                viewport,
            )?;
            self.overlay_button(
                rect(star.left - 32.0, star.top, star.left - 6.0, star.bottom),
                Glyph::External,
                external,
                viewport,
            )?;
        } else if starred {
            let star = rect(
                preview.right - 30.0,
                preview.top + 8.0,
                preview.right - 8.0,
                preview.top + 30.0,
            );
            self.fill_alpha(star, 0x000000, 0.45, 11.0)?;
            self.glyph(
                Glyph::StarFilled,
                rect(
                    star.left + 5.0,
                    star.top + 5.0,
                    star.right - 5.0,
                    star.bottom - 5.0,
                ),
                0xffffff,
            )?;
        }

        let text_top = resting.bottom + if compact { 6.0 } else { 9.0 };
        let title_format = if compact {
            self.caption.clone()
        } else {
            self.strong.clone()
        };
        let more = rect(
            card.right - 24.0,
            text_top - 2.0,
            card.right,
            text_top + if compact { 18.0 } else { 22.0 },
        );
        let show_more = hovered && !model.selection_mode;
        let title_right = if show_more {
            more.left - 6.0
        } else {
            card.right
        };
        self.text(
            &self.shorten(&clip.title, &title_format, title_right - card.left),
            rect(
                card.left + 1.0,
                text_top,
                title_right,
                text_top + if compact { 16.0 } else { 19.0 },
            ),
            &title_format,
            self.palette.primary,
        )?;
        if !compact {
            self.text(
                &format!(
                    "{}  ·  {}",
                    crate::clock::stamp_label(
                        crate::clock::local(clip.modified),
                        today,
                        self.strings
                    ),
                    format_bytes(clip.size_bytes)
                ),
                rect(
                    card.left + 1.0,
                    text_top + 19.0,
                    card.right,
                    text_top + 37.0,
                ),
                &self.small.clone(),
                self.palette.muted,
            )?;
        }
        if show_more {
            if self.is_hovered(&menu) {
                self.tint(more, 0.08, RADIUS_SMALL)?;
            }
            self.glyph(
                Glyph::More,
                rect(
                    more.left + 5.0,
                    more.top + 5.0,
                    more.right - 5.0,
                    more.bottom - 5.0,
                ),
                if self.is_hovered(&menu) {
                    self.palette.primary
                } else {
                    self.palette.muted
                },
            )?;
            self.push_clipped_hit(more, viewport, menu);
        }
        Ok(())
    }

    pub(crate) fn overlay_button(
        &mut self,
        area: LogicalRect,
        glyph: Glyph,
        action: Action,
        viewport: LogicalRect,
    ) -> Result<(), String> {
        let hovered = self.is_hovered(&action);
        let radius = (area.bottom - area.top) / 2.0;
        self.fill_alpha(area, 0x000000, if hovered { 0.7 } else { 0.45 }, radius)?;
        self.glyph(
            glyph,
            rect(
                area.left + 6.0,
                area.top + 6.0,
                area.right - 6.0,
                area.bottom - 6.0,
            ),
            0xffffff,
        )?;
        self.push_clipped_hit(area, viewport, action);
        Ok(())
    }

    pub(crate) fn dropdown(
        &mut self,
        area: LogicalRect,
        value: &str,
        action: Action,
    ) -> Result<(), String> {
        let hovered = self.is_hovered(&action);
        self.tint(
            area,
            if hovered {
                0.06 + 0.04 * self.hover_amount(1.0)
            } else {
                0.06
            },
            RADIUS_SMALL - 1.0,
        )?;
        let center = (area.top + area.bottom) / 2.0;
        let chevron = rect(
            area.right - 22.0,
            center - 6.0,
            area.right - 10.0,
            center + 6.0,
        );
        let value_area = rect(area.left + 10.0, area.top, chevron.left - 6.0, area.bottom);
        self.text(
            &self.shorten(value, &self.body, value_area.right - value_area.left),
            value_area,
            &self.body.clone(),
            self.palette.primary,
        )?;
        self.glyph(Glyph::UpDown, chevron, self.palette.muted)?;
        self.hits.push(HitRegion { rect: area, action });
        Ok(())
    }

    pub(crate) fn push_clipped_hit(
        &mut self,
        area: LogicalRect,
        viewport: LogicalRect,
        action: Action,
    ) {
        let clipped = rect(
            area.left.max(viewport.left),
            area.top.max(viewport.top),
            area.right.min(viewport.right),
            area.bottom.min(viewport.bottom),
        );
        if clipped.right > clipped.left && clipped.bottom > clipped.top {
            self.hits.push(HitRegion {
                rect: clipped,
                action,
            });
        }
    }
}

pub(crate) struct LibraryLayout {
    pub(crate) columns: usize,
    pub(crate) gap: f32,
    /// Room for a day heading above each section; none in the compact grid.
    pub(crate) header: f32,
    pub(crate) card_width: f32,
    pub(crate) card_height: f32,
    pub(crate) row_pitch: f32,
    pub(crate) sections: Vec<f32>,
    pub(crate) height: f32,
}

pub(crate) fn clip_columns(width: f32) -> usize {
    if width >= 1_460.0 {
        5
    } else if width >= 1_080.0 {
        4
    } else if width >= 700.0 {
        3
    } else if width >= 480.0 {
        2
    } else {
        1
    }
}

/// Compact cards aim for this width, so a wide window shows more clips at once.
pub(crate) fn compact_columns(width: f32) -> usize {
    ((width + COMPACT_GAP) / (COMPACT_CARD_WIDTH + COMPACT_GAP)).clamp(2.0, 10.0) as usize
}

pub(crate) fn library_layout(counts: &[usize], width: f32, view: LibraryView) -> LibraryLayout {
    let (columns, gap, meta, header) = match view {
        LibraryView::Grid => (
            clip_columns(width),
            CLIP_GAP,
            CLIP_META_HEIGHT,
            CLIP_SECTION_HEADER,
        ),
        LibraryView::Compact => (
            compact_columns(width),
            COMPACT_GAP,
            COMPACT_META_HEIGHT,
            0.0,
        ),
    };
    let card_width = ((width - gap * (columns - 1) as f32) / columns as f32).max(120.0);
    let card_height = (card_width * 9.0 / 16.0).round() + meta;
    let row_pitch = card_height + gap;
    let mut sections = Vec::with_capacity(counts.len());
    let mut offset = 0.0;
    for count in counts {
        sections.push(offset);
        let rows = count.div_ceil(columns).max(1);
        offset += header + (rows - 1) as f32 * row_pitch + card_height + CLIP_GROUP_GAP;
    }
    LibraryLayout {
        columns,
        gap,
        header,
        card_width,
        card_height,
        row_pitch,
        sections,
        height: (offset - CLIP_GROUP_GAP).max(0.0),
    }
}

pub(crate) fn clips_overflow(model: &UiModel, width: f32, height: f32) -> f32 {
    let collections = match model.page {
        Page::Library => false,
        Page::Collections => true,
        _ => return 0.0,
    };
    let today = crate::clock::now();
    let indices = model.visible_clip_indices_at(usize::MAX, today);
    if indices.is_empty() {
        return 0.0;
    }
    let counts = model
        .clip_groups(&indices, today)
        .iter()
        .map(|group| group.indices.len())
        .collect::<Vec<_>>();
    let mut left = sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING;
    let mut top = content_top() + LIBRARY_BODY_OFFSET;
    if collections {
        left = folder_area_left(model, left);
        top = content_top() + LIBRARY_BODY_OFFSET + 34.0;
    }
    let area_width = width - CONTENT_PADDING - left - CLIP_SCROLL_RESERVE;
    let layout = library_layout(&counts, area_width, model.config.appearance.library_view);
    let selecting = model.selection_mode && !model.selected_clips.is_empty();
    let mut bottom = content_bottom(height, true);
    if selecting {
        bottom -= 60.0;
    }
    (layout.height - (bottom - top).max(0.0)).max(0.0)
}
