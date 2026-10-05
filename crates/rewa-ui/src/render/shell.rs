//! The frame: sidebar with its folding rail, the replay block at its foot, and the
//! stage card that holds the current page.

use gtk::prelude::*;

use super::*;

impl Renderer {
    pub(crate) fn render_shell(
        &mut self,
        model: &UiModel,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let target = sidebar_width(model.sidebar_collapsed);
        let now = Instant::now();
        let reduced = self.reduced_motion;
        let motion = self.sidebar_motion.get_or_insert_with(|| {
            rewa_shell::motion::Motion::with_curve(target, rewa_shell::motion::Curve::Glide)
        });
        motion.retarget(target, now, reduced);
        self.rail = motion.value(now);
        self.navigation_was_moving |= motion.active(now);
        self.render_sidebar(model, height)?;
        let stage = rect(
            self.rail,
            STAGE_INSET,
            width - STAGE_INSET,
            height - STAGE_INSET,
        );
        self.fill(stage, self.palette.surface, RADIUS)?;
        self.stroke(
            stage,
            mix(self.palette.canvas, self.palette.primary, 0.045),
            RADIUS,
            1.0,
        )?;
        // Pages take their final layout at once and slide with the rail as one piece,
        // so the grid never reflows mid-fold; preview and editor stay put because
        // their video is a child window that already sits at the final spot.
        let left = target + CONTENT_PADDING;
        let right = width - CONTENT_PADDING;
        let chrome = page_has_chrome(model.page);
        let top = if chrome { content_top() } else { 0.0 };
        let bottom = content_bottom(height, chrome);
        let shift = if chrome { self.rail - target } else { 0.0 };
        self.push_clip(stage)?;
        self.snapshot().save();
        self.snapshot()
            .translate(&gtk::graphene::Point::new(shift, 0.0));
        let painted = match model.page {
            Page::Library => self.render_library(model, left, right, top, bottom),
            Page::Collections => self.render_collections(model, left, right, top, bottom),
            Page::Settings => self.render_settings(model, left, right, top, bottom),
            Page::Player => self.render_player(model, left, right, height),
            Page::Editor => self.render_editor(model, left, right, height),
        };
        self.snapshot().restore();
        self.pop_clip();
        painted?;
        self.render_video(model, width, height);
        if model.capture_panel_open {
            self.render_capture_panel(model, height)?;
        }
        Ok(())
    }

    pub(crate) fn render_sidebar(&mut self, model: &UiModel, height: f32) -> Result<(), String> {
        let rail = self.rail;
        // 0 while collapsed, 1 while expanded; labels fade with it while the edge clips them
        let reveal = self.sidebar_reveal();
        self.fill(rect(0.0, 0.0, rail, height), self.palette.rail, 0.0)?;
        self.push_clip(rect(0.0, 0.0, rail, height))?;
        let painted = self.render_sidebar_content(model, height, rail, reveal);
        self.pop_clip();
        painted
    }

    pub(crate) fn sidebar_reveal(&self) -> f32 {
        ((self.rail - SIDEBAR_COLLAPSED_WIDTH) / (SIDEBAR_WIDTH - SIDEBAR_COLLAPSED_WIDTH))
            .clamp(0.0, 1.0)
    }

    pub(crate) fn render_sidebar_content(
        &mut self,
        model: &UiModel,
        height: f32,
        rail: f32,
        reveal: f32,
    ) -> Result<(), String> {
        // the door keeps one spot in both states: centred in the collapsed rail
        self.door(
            rect(
                SIDEBAR_COLLAPSED_WIDTH / 2.0 - 13.0,
                14.0,
                SIDEBAR_COLLAPSED_WIDTH / 2.0 + 13.0,
                40.0,
            ),
            Glyph::Sidebar,
            false,
            Action::ToggleSidebar,
        )?;
        if reveal > 0.0 {
            // lowercase without ascenders sits low in its line box, so the word rises 2 px
            // to share the door's centre; it starts where the row labels start
            // the ghost takes the label column, the word follows at the row gap, both
            // centred on the door (lowercase sits low, so the word rises 2 px)
            let ghost = rect(
                SIDEBAR_ICON_LEFT + 26.0,
                17.0,
                SIDEBAR_ICON_LEFT + 46.0,
                37.0,
            );
            self.draw_ghost(ghost, reveal);
            let brand = rect(ghost.right + 7.0, 12.0, rail - 12.0, 38.0);
            self.text(
                "rewa",
                brand,
                &self.brand.clone(),
                mix(self.palette.rail, self.palette.primary, reveal),
            )?;
            let word = self.measure("rewa", &self.brand);
            self.hits.push(HitRegion {
                rect: rect(ghost.left - 4.0, 14.0, brand.left + word + 4.0, 40.0),
                action: Action::Home,
            });
        }

        let navigation = [
            (Some(Page::Library), Glyph::Library, self.strings.clips),
            (
                Some(Page::Collections),
                Glyph::Collections,
                self.strings.collections,
            ),
            (None, Glyph::Folder, self.strings.open_folder),
        ];
        let settings_top = height - 14.0 - NAVIGATION_HEIGHT;
        // The selection pill travels between destinations on the glide spring;
        // labels and hit regions stay put, also while a move is interrupted.
        let selected_page = if matches!(model.page, Page::Player | Page::Editor) {
            model.previous_page
        } else {
            model.page
        };
        let selected_top = match selected_page {
            Page::Settings => settings_top,
            Page::Collections => NAVIGATION_TOP + NAVIGATION_PITCH,
            _ => NAVIGATION_TOP,
        };
        let now = Instant::now();
        let motion = self.navigation_motion.get_or_insert_with(|| {
            rewa_shell::motion::Motion::with_curve(selected_top, rewa_shell::motion::Curve::Glide)
        });
        motion.retarget(selected_top, now, self.reduced_motion);
        let selection_top = motion.value(now);
        self.navigation_was_moving |= motion.active(now);
        self.fill(
            rect(
                SIDEBAR_ROW_INSET,
                selection_top,
                rail - SIDEBAR_ROW_INSET,
                selection_top + NAVIGATION_HEIGHT,
            ),
            self.palette.surface_raised,
            RADIUS_SMALL + 1.0,
        )?;
        for (offset, (page, glyph, label)) in navigation.iter().enumerate() {
            let active = page.is_some_and(|page| selected_page == page);
            let action = page.map_or(Action::OpenClipsFolder, Action::Navigate);
            self.sidebar_item(
                NAVIGATION_TOP + offset as f32 * NAVIGATION_PITCH,
                *glyph,
                label,
                active,
                action,
            )?;
        }

        self.render_replay_block(model, settings_top - 12.0)?;
        self.sidebar_item(
            settings_top,
            Glyph::Settings,
            self.strings.settings,
            model.page == Page::Settings,
            Action::Navigate(Page::Settings),
        )
    }

    pub(crate) fn sidebar_item(
        &mut self,
        top: f32,
        glyph: Glyph,
        label: &str,
        active: bool,
        action: Action,
    ) -> Result<(), String> {
        let reveal = self.sidebar_reveal();
        let area = rect(
            SIDEBAR_ROW_INSET,
            top,
            self.rail - SIDEBAR_ROW_INSET,
            top + NAVIGATION_HEIGHT,
        );
        let hovered = !active && self.is_hovered(&action);
        if hovered {
            self.tint(area, 0.06 * self.hover_amount(1.0), RADIUS_SMALL + 1.0)?;
        }
        let tone = if active {
            self.palette.primary
        } else {
            mix(
                self.palette.muted,
                self.palette.primary,
                if hovered {
                    0.45 * self.hover_amount(1.0)
                } else {
                    0.0
                },
            )
        };
        self.glyph(
            glyph,
            rect(
                SIDEBAR_ICON_LEFT,
                area.top + 7.0,
                SIDEBAR_ICON_LEFT + 16.0,
                area.bottom - 7.0,
            ),
            tone,
        )?;
        if reveal > 0.0 {
            self.text(
                label,
                rect(
                    SIDEBAR_ICON_LEFT + 26.0,
                    area.top,
                    area.right - 8.0,
                    area.bottom,
                ),
                &self.body.clone(),
                mix(self.palette.rail, tone, reveal),
            )?;
        }
        self.hits.push(HitRegion { rect: area, action });
        Ok(())
    }

    /// Replay state and the save action live at the foot of the sidebar, so every
    /// page can save without a toolbar; the state row opens the capture popover.
    pub(crate) fn render_replay_block(
        &mut self,
        model: &UiModel,
        bottom: f32,
    ) -> Result<(), String> {
        let rail = self.rail;
        let reveal = self.sidebar_reveal();
        let live = model.daemon.is_recording();
        let save = rect(
            SIDEBAR_ROW_INSET,
            bottom - 30.0,
            rail - SIDEBAR_ROW_INSET,
            bottom,
        );
        let status = rect(
            SIDEBAR_ROW_INSET,
            save.top - 44.0,
            rail - SIDEBAR_ROW_INSET,
            save.top - 8.0,
        );
        let status_action = Action::ToggleCapturePanel;
        let status_hovered = self.is_hovered(&status_action);
        if model.capture_panel_open {
            self.tint(status, 0.07, RADIUS_SMALL + 1.0)?;
        } else if status_hovered {
            self.tint(status, 0.06 * self.hover_amount(1.0), RADIUS_SMALL + 1.0)?;
        }
        self.status_dot(SIDEBAR_COLLAPSED_WIDTH / 2.0, status.top + 18.0, live)?;
        if reveal > 0.0 {
            let fade = |renderer: &Self, tone: u32| mix(renderer.palette.rail, tone, reveal);
            self.text(
                model.daemon.toolbar_headline(self.strings),
                rect(
                    SIDEBAR_ICON_LEFT + 26.0,
                    status.top + 3.0,
                    status.right - 28.0,
                    status.top + 22.0,
                ),
                &self.body.clone(),
                fade(self, self.palette.primary),
            )?;
            let seconds = model
                .daemon
                .buffered_seconds
                .min(model.config.capture.duration_seconds);
            self.text(
                &self.strings.buffered_seconds(seconds),
                rect(
                    SIDEBAR_ICON_LEFT + 26.0,
                    status.top + 20.0,
                    status.right - 28.0,
                    status.bottom - 2.0,
                ),
                &self.small.clone(),
                fade(self, self.palette.muted),
            )?;
            self.glyph(
                if model.capture_panel_open {
                    Glyph::ChevronDown
                } else {
                    Glyph::ChevronRight
                },
                rect(
                    status.right - 22.0,
                    status.top + 12.0,
                    status.right - 10.0,
                    status.bottom - 12.0,
                ),
                fade(
                    self,
                    if status_hovered || model.capture_panel_open {
                        self.palette.secondary
                    } else {
                        self.palette.muted
                    },
                ),
            )?;
        }
        self.hits.push(HitRegion {
            rect: status,
            action: status_action,
        });

        let action = Action::SaveReplay;
        if model.replay_pending {
            self.tint(save, 0.08, RADIUS_SMALL)?;
        } else {
            let fill = if self.is_hovered(&action) {
                mix(
                    self.palette.accent,
                    self.palette.accent_hover,
                    self.hover_amount(1.0),
                )
            } else {
                self.palette.accent
            };
            self.fill(save, fill, RADIUS_SMALL)?;
        }
        let ink = if model.replay_pending {
            self.palette.secondary
        } else {
            self.palette.accent_text
        };
        let ground = if model.replay_pending {
            self.palette.rail
        } else {
            self.palette.accent
        };
        // the record mark holds the collapsed button, the words take over as it widens
        if reveal < 1.0 {
            self.glyph(
                Glyph::Record,
                rect(
                    SIDEBAR_ICON_LEFT,
                    save.top + 7.0,
                    SIDEBAR_ICON_LEFT + 16.0,
                    save.bottom - 7.0,
                ),
                mix(ground, ink, 1.0 - reveal),
            )?;
        }
        if reveal > 0.0 {
            self.text(
                if model.replay_pending {
                    self.strings.saving
                } else {
                    self.strings.save_clip
                },
                save,
                &self.button.clone(),
                mix(ground, ink, reveal),
            )?;
        }
        if !model.replay_pending {
            self.hits.push(HitRegion { rect: save, action });
        }
        Ok(())
    }

    /// The video sits where Windows places its player child window, and like that
    /// window it steps aside while a dialog plate is open.
    pub(crate) fn render_video(&mut self, model: &UiModel, width: f32, height: f32) {
        let Some(video) = self.video.clone() else {
            return;
        };
        if !matches!(model.page, Page::Player | Page::Editor)
            || model.prompt.is_some()
            || model.pending_delete.is_some()
        {
            return;
        }
        let bounds = if model.page == Page::Editor {
            editor_player_bounds(model, width as u32, height as u32)
        } else {
            player_bounds(model, width as u32, height as u32)
        };
        self.video_into(&video, bounds);
    }

    pub(crate) fn video_into(&self, video: &gdk::Paintable, bounds: LogicalRect) {
        let picture = fit_aspect(bounds, model_aspect(video));
        let snapshot = self.snapshot();
        let _ = self.fill(bounds, 0x000000, 0.0);
        snapshot.save();
        snapshot.translate(&gtk::graphene::Point::new(picture.left, picture.top));
        video.snapshot(
            snapshot,
            f64::from(picture.right - picture.left),
            f64::from(picture.bottom - picture.top),
        );
        snapshot.restore();
    }
}

fn model_aspect(video: &gdk::Paintable) -> f32 {
    let ratio = video.intrinsic_aspect_ratio();
    if ratio > 0.0 {
        ratio as f32
    } else {
        16.0 / 9.0
    }
}
