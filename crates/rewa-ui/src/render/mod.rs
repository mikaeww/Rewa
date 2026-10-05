//! The Linux renderer: the Windows renderer's layout and drawing, painted into a
//! GTK snapshot instead of a Direct2D target. Pages, controls and geometry keep the
//! Windows names and numbers so a change on one side ports line by line.

mod capture;
mod controls;
mod format;
mod glyph;
mod images;
mod layout;
mod overlays;
mod pages;
mod painter;
mod shell;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gtk::{gdk, pango};
use rewa_core::config::{HoverStyle, Language, LibraryView, Theme};

use crate::model::{
    Action, ClipGroup, ClipTab, DeleteTarget, Page, SettingsMenuKind, SettingsSection, TextInput,
    UiModel, hover_strength_label, hover_style_label, language_label, quality_label, theme_label,
};
use crate::text::Strings;

use controls::{PlateButton, Segment};
use format::*;
use glyph::Glyph;
use images::Images;
use layout::*;
use overlays::TextInputTarget;
use pages::{LineControl, library_layout};
use painter::{Alignment, Font, Palette, mix};

pub(crate) use layout::{
    FULLSCREEN_HEADER_HEIGHT, LogicalRect, editor_player_bounds, editor_timeline_fraction,
    editor_timeline_rail, folder_column_contains, folder_column_overflow, folder_width_at,
    fullscreen_picture, fullscreen_timeline_rail, fullscreen_volume_rail, player_bounds,
    player_timeline_rail, player_volume_rail, settings_audio_gain_rail, settings_gain_percent,
};
pub(crate) use pages::clips_overflow;
pub(crate) use painter::palette_for;

#[derive(Clone)]
struct HitRegion {
    rect: LogicalRect,
    action: Action,
}

pub struct Renderer {
    snapshot: Option<gtk::Snapshot>,
    pango: pango::Context,
    palette: Palette,
    strings: &'static Strings,
    hover_style: HoverStyle,
    hover_strength: f32,
    page_title: Font,
    section: Font,
    brand: Font,
    heading: Font,
    heading_center: Font,
    caption: Font,
    strong: Font,
    body: Font,
    small: Font,
    small_center: Font,
    small_right: Font,
    body_trailing: Font,
    body_center: Font,
    body_wrap: Font,
    button: Font,
    button_leading: Font,
    hits: Vec<HitRegion>,
    images: Images,
    /// The clip video, drawn where the Windows player child window sits.
    video: Option<gdk::Paintable>,
    measured: std::cell::RefCell<HashMap<(String, usize), f32>>,
    hovered: Option<Action>,
    hover_progress: f32,
    reduced_motion: bool,
    navigation_motion: Option<rewa_shell::motion::Motion>,
    sidebar_motion: Option<rewa_shell::motion::Motion>,
    rail: f32,
    navigation_was_moving: bool,
    hover_started: Instant,
    toggle_motions: HashMap<&'static str, rewa_shell::motion::Motion>,
}

impl Renderer {
    /// `family` is the desktop interface font; Windows uses Segoe UI Variable.
    pub fn new(
        pango: pango::Context,
        family: &str,
        thumbnail_directory: PathBuf,
        reduced_motion: bool,
    ) -> Self {
        let mut id = 0;
        let mut font = |size: f32, semibold: bool, alignment: Alignment| {
            id += 1;
            Font::new(family, size, semibold, alignment, id)
        };
        Self {
            snapshot: None,
            pango,
            palette: palette_for(Theme::default()),
            strings: crate::text::strings(Language::default()),
            hover_style: HoverStyle::default(),
            hover_strength: 1.0,
            page_title: font(22.0, true, Alignment::Leading),
            section: font(14.0, true, Alignment::Leading),
            brand: font(14.5, true, Alignment::Leading),
            heading: font(17.0, true, Alignment::Leading),
            heading_center: font(17.0, true, Alignment::Center),
            caption: font(11.5, true, Alignment::Leading),
            strong: font(13.0, true, Alignment::Leading),
            body: font(13.0, false, Alignment::Leading),
            small: font(11.5, false, Alignment::Leading),
            small_center: font(11.0, true, Alignment::Center),
            small_right: font(11.5, false, Alignment::Trailing),
            body_trailing: font(13.0, false, Alignment::Trailing),
            body_center: font(13.0, false, Alignment::Center),
            body_wrap: font(13.0, false, Alignment::Leading).wrapping(),
            button: font(12.5, true, Alignment::Center),
            button_leading: font(12.5, true, Alignment::Leading),
            hits: Vec::new(),
            images: Images::new(thumbnail_directory),
            video: None,
            measured: std::cell::RefCell::new(HashMap::new()),
            hovered: None,
            hover_progress: 0.0,
            reduced_motion,
            navigation_motion: None,
            sidebar_motion: None,
            rail: SIDEBAR_WIDTH,
            navigation_was_moving: false,
            hover_started: Instant::now(),
            toggle_motions: HashMap::new(),
        }
    }

    pub fn retry_unavailable_thumbnails(&mut self) {
        self.images.retry_unavailable();
    }

    /// A clip replaced in place keeps its path, so its cached picture and length go.
    pub fn forget_clip(&mut self, path: &Path) {
        self.images.forget(path);
    }

    /// Takes in thumbnails the worker finished; true when the grid should repaint.
    pub fn collect_images(&mut self) -> bool {
        self.images.collect()
    }

    pub fn set_video(&mut self, video: Option<gdk::Paintable>) {
        self.video = video;
    }

    pub fn paint(
        &mut self,
        snapshot: &gtk::Snapshot,
        model: &UiModel,
        width: f32,
        height: f32,
        fullscreen: bool,
    ) -> Result<(), String> {
        self.begin(snapshot, model, width, height);
        let painted = self.render_frame(model, width as u32, height as u32, fullscreen);
        self.snapshot = None;
        painted
    }

    pub fn paint_fullscreen_controls(
        &mut self,
        snapshot: &gtk::Snapshot,
        model: &UiModel,
        width: f32,
        height: f32,
        picture: LogicalRect,
    ) -> Result<(), String> {
        self.begin(snapshot, model, width, height);
        let painted = self.render_fullscreen_controls(model, width as u32, height as u32, picture);
        self.snapshot = None;
        painted
    }

    fn begin(&mut self, snapshot: &gtk::Snapshot, model: &UiModel, width: f32, height: f32) {
        self.apply_appearance(model);
        self.hits.clear();
        self.images.begin_frame();
        self.snapshot = Some(snapshot.clone());
        let _ = self.fill(rect(0.0, 0.0, width, height), self.palette.canvas, 0.0);
    }

    pub(crate) fn measure(&self, value: &str, format: &Font) -> f32 {
        if value.is_empty() {
            return 0.0;
        }
        // titles are measured every frame, several times while they are shortened
        let key = (value.to_owned(), format.id);
        if let Some(width) = self.measured.borrow().get(&key) {
            return *width;
        }
        let width = self.measure_uncached(value, format);
        let mut measured = self.measured.borrow_mut();
        // ponytail: dropped wholesale at the cap; an LRU only pays off with far more text on screen
        if measured.len() >= 4_096 {
            measured.clear();
        }
        measured.insert(key, width);
        width
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<Action> {
        self.hits
            .iter()
            .rev()
            .find(|hit| hit.rect.contains(x, y))
            .map(|hit| hit.action.clone())
    }

    pub fn update_hover(&mut self, x: f32, y: f32) -> bool {
        let next = self.hit_test(x, y);
        if next == self.hovered {
            return false;
        }
        self.hovered = next;
        self.hover_started = Instant::now();
        self.hover_progress = if self.reduced_motion { 1.0 } else { 0.0 };
        true
    }

    pub fn clear_hover(&mut self) -> bool {
        if self.hovered.take().is_none() {
            return false;
        }
        self.hover_progress = 0.0;
        true
    }

    pub fn hovered(&self) -> Option<&Action> {
        self.hovered.as_ref()
    }

    pub fn is_animating(&self) -> bool {
        self.navigation_was_moving || (self.hovered.is_some() && self.hover_progress < 1.0)
    }

    pub fn advance_motion(&mut self) -> bool {
        let now = Instant::now();
        let moving = self
            .navigation_motion
            .as_ref()
            .is_some_and(|motion| motion.active(now))
            || self
                .toggle_motions
                .values()
                .any(|motion| motion.active(now));
        let mut changed = moving || self.navigation_was_moving;
        self.navigation_was_moving = moving;
        if self.hovered.is_some() && self.hover_progress < 1.0 {
            self.hover_progress = rewa_shell::motion::Curve::Quick.ease(
                now.saturating_duration_since(self.hover_started)
                    .as_secs_f32()
                    / 0.14,
            );
            changed = true;
        }
        changed
    }

    pub(crate) fn apply_appearance(&mut self, model: &UiModel) {
        let config = &model.config;
        self.palette = palette_for(config.appearance.theme);
        self.strings = model.strings();
        self.hover_style = config.appearance.hover;
        self.hover_strength = config.appearance.hover_strength.factor();
    }

    pub(crate) fn hover_amount(&self, weight: f32) -> f32 {
        hover_blend_amount(self.hover_progress, self.hover_strength, weight)
    }

    pub(crate) fn is_hovered(&self, action: &Action) -> bool {
        self.hovered.as_ref() == Some(action)
    }

    pub(crate) fn shorten(&self, value: &str, format: &Font, max_width: f32) -> String {
        if max_width <= 0.0 || self.measure(value, format) <= max_width {
            return value.to_owned();
        }
        let characters = value.chars().count();
        let mut fits = 0;
        let mut too_long = characters;
        while too_long - fits > 1 {
            let middle = (fits + too_long) / 2;
            let candidate = value
                .chars()
                .take(middle)
                .chain(std::iter::once('…'))
                .collect::<String>();
            if self.measure(&candidate, format) <= max_width {
                fits = middle;
            } else {
                too_long = middle;
            }
        }
        value
            .chars()
            .take(fits)
            .chain(std::iter::once('…'))
            .collect()
    }

    pub(crate) fn render_frame(
        &mut self,
        model: &UiModel,
        width: u32,
        height: u32,
        fullscreen: bool,
    ) -> Result<(), String> {
        if fullscreen && model.page == Page::Player {
            // the video is a disabled child, so its clicks land here and pause or play
            self.hits.push(HitRegion {
                rect: rect(0.0, FULLSCREEN_HEADER_HEIGHT, width as f32, height as f32),
                action: Action::PlayPause,
            });
            if let Some(video) = self.video.clone() {
                self.video_into(
                    &video,
                    rect(0.0, FULLSCREEN_HEADER_HEIGHT, width as f32, height as f32),
                );
            }
            self.render_fullscreen_header(width as f32)?;
            return Ok(());
        }
        self.render_shell(model, width as f32, height as f32)?;
        if model.settings_menu.is_some() {
            self.render_settings_menu(model, width as f32, height as f32)?;
        }
        if model.context_menu.is_some() {
            self.render_context_menu(model, width as f32, height as f32)?;
        }
        if model.pending_delete.is_some() {
            self.render_delete_modal(model, width as f32, height as f32)?;
        }
        if model.prompt.is_some() {
            self.render_prompt_modal(model, width as f32, height as f32)?;
        }
        if let Some(notice) = &model.notice {
            // a toast at the foot of the stage, the way Leech confirms things
            let stage = rect(
                self.rail,
                STAGE_INSET,
                width as f32 - STAGE_INSET,
                height as f32 - STAGE_INSET,
            );
            let text_width = self.measure(notice, &self.body);
            let toast_width = (text_width + 64.0).min(stage.right - stage.left - 48.0);
            let center = (stage.left + stage.right) / 2.0;
            let notice_area = rect(
                center - toast_width / 2.0,
                stage.bottom - 24.0 - 38.0,
                center + toast_width / 2.0,
                stage.bottom - 24.0,
            );
            self.popover_surface(notice_area)?;
            self.text(
                &self.shorten(notice, &self.body, toast_width - 58.0),
                rect(
                    notice_area.left + 18.0,
                    notice_area.top,
                    notice_area.right - 36.0,
                    notice_area.bottom,
                ),
                &self.body.clone(),
                if model.hotkey_capture {
                    self.palette.primary
                } else {
                    self.palette.secondary
                },
            )?;
            let close = rect(
                notice_area.right - 32.0,
                notice_area.top + 7.0,
                notice_area.right - 8.0,
                notice_area.bottom - 7.0,
            );
            if self.is_hovered(&Action::DismissNotice) {
                self.tint(close, 0.08, RADIUS_SMALL)?;
            }
            self.glyph(
                Glyph::Close,
                rect(
                    close.left + 6.0,
                    close.top + 6.0,
                    close.right - 6.0,
                    close.bottom - 6.0,
                ),
                self.palette.muted,
            )?;
            self.hits.push(HitRegion {
                rect: close,
                action: Action::DismissNotice,
            });
        }
        if let Some(drag) = &model.clip_drag_preview {
            let label = if model.page == Page::Collections {
                self.strings.move_drag(drag.count)
            } else {
                self.strings.drag_out.to_owned()
            };
            let chip_width = self.measure(&label, &self.body) + 32.0;
            let chip_height = 34.0;
            let left = (drag.x + 14.0).clamp(12.0, width as f32 - chip_width - 12.0);
            let top = (drag.y + 14.0).clamp(12.0, height as f32 - chip_height - 12.0);
            let chip = rect(left, top, left + chip_width, top + chip_height);
            self.popover_surface(chip)?;
            self.text(
                &label,
                chip,
                &self.body_center.clone(),
                self.palette.primary,
            )?;
        }
        Ok(())
    }
}
