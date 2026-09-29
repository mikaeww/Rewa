use std::collections::{HashMap, HashSet, VecDeque};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use windows::Win32::Foundation::{HWND, PROPERTYKEY};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_F, D2D_SIZE_U, D2D1_COLOR_F, D2D1_FIGURE_BEGIN, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_ARC_SEGMENT,
    D2D1_ARC_SIZE_SMALL, D2D1_BITMAP_BRUSH_PROPERTIES, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
    D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, D2D1_CAP_STYLE_ROUND, D2D1_DASH_STYLE_SOLID,
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_EXTEND_MODE_CLAMP,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_LAYER_OPTIONS_NONE,
    D2D1_LAYER_PARAMETERS, D2D1_LINE_JOIN_ROUND, D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT,
    D2D1_STROKE_STYLE_PROPERTIES, D2D1_SWEEP_DIRECTION_CLOCKWISE, D2D1CreateFactory, ID2D1Bitmap,
    ID2D1Factory, ID2D1HwndRenderTarget, ID2D1PathGeometry, ID2D1SolidColorBrush, ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
};
use windows::Win32::Graphics::Gdi::{DeleteObject, HPALETTE};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, IWICImagingFactory, WICBitmapIgnoreAlpha,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IBindCtx};
use windows::Win32::UI::Shell::PropertiesSystem::{
    GPS_DEFAULT, IPropertyStore, SHGetPropertyStoreFromParsingName,
};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_RESIZETOFIT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::{GUID, PCWSTR, w};
use windows_numerics::{Matrix3x2, Vector2};

use rewa_core::config::{HoverStyle, Language, Theme};

use crate::model::{
    Action, ClipGroup, ClipTab, DeleteTarget, Page, SettingsMenuKind, SettingsSection, TextInput,
    UiModel, hover_strength_label, hover_style_label, language_label, quality_label, theme_label,
};
use crate::text::Strings;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub canvas: u32,
    pub rail: u32,
    pub stage: u32,
    pub surface: u32,
    pub surface_raised: u32,
    pub surface_hover: u32,
    pub border: u32,
    pub hairline: u32,
    pub card: u32,
    pub primary: u32,
    pub secondary: u32,
    pub muted: u32,
    pub accent: u32,
    pub accent_hover: u32,
    pub accent_text: u32,
    pub selection: u32,
    pub destructive: u32,
    /// Colour of live indicators: the replay dot and the microphone meter. The
    /// café palette spends its single accent here, the others stay neutral.
    pub live: u32,
}

// Leech's grey pair (after Search's Design.swift): the frame holds the chrome, the stage card holds the page.
const DARK_PALETTE: Palette = Palette {
    canvas: 0x141414,
    rail: 0x141414,
    stage: 0x111111,
    surface: 0x1c1c1c,
    surface_raised: 0x2b2b2b,
    surface_hover: 0x262626,
    border: 0x333333,
    hairline: 0x2a2a2a,
    card: 0x1c1c1c,
    primary: 0xededed,
    secondary: 0xb3b3b3,
    muted: 0x949494,
    accent: 0xededed,
    accent_hover: 0xffffff,
    accent_text: 0x141414,
    selection: 0x2d2d2d,
    destructive: 0xff6b61,
    live: 0xededed,
};

const LIGHT_PALETTE: Palette = Palette {
    canvas: 0xf2f2f2,
    rail: 0xf2f2f2,
    stage: 0xe9e9e9,
    surface: 0xffffff,
    surface_raised: 0xffffff,
    surface_hover: 0xf6f6f6,
    border: 0xe0e0e0,
    hairline: 0xe8e8e8,
    card: 0xffffff,
    primary: 0x171717,
    secondary: 0x4d4d4d,
    muted: 0x6e6e6e,
    accent: 0x171717,
    accent_hover: 0x333333,
    accent_text: 0xffffff,
    selection: 0xefefef,
    destructive: 0xd70015,
    live: 0x171717,
};

const CAFE_PALETTE: Palette = Palette {
    canvas: 0x0d0c0b,
    rail: 0x100f0d,
    stage: 0x100f0e,
    surface: 0x151412,
    surface_raised: 0x191715,
    surface_hover: 0x1e1b18,
    border: 0x2a2621,
    hairline: 0x201d1a,
    card: 0x121110,
    primary: 0xf0ece4,
    secondary: 0xa9a29a,
    muted: 0x7c766d,
    accent: 0xe9e2d4,
    accent_hover: 0xf7f2e8,
    accent_text: 0x0d0c0b,
    selection: 0x3a352e,
    destructive: 0xd8d2c8,
    live: 0x7f9b6f,
};

const PINK_PALETTE: Palette = Palette {
    canvas: 0x120b0f,
    rail: 0x160d12,
    stage: 0x150d11,
    surface: 0x1d1218,
    surface_raised: 0x23161d,
    surface_hover: 0x2b1b24,
    border: 0x402834,
    hairline: 0x2e1d27,
    card: 0x180f14,
    primary: 0xf9edf3,
    secondary: 0xc4a6b6,
    muted: 0x947886,
    accent: 0xf25a9d,
    accent_hover: 0xff7ab4,
    accent_text: 0x180a11,
    selection: 0x533242,
    destructive: 0xe6ccd8,
    live: 0xf25a9d,
};

const CANDY_PALETTE: Palette = Palette {
    canvas: 0xfff0f6,
    rail: 0xffe3ef,
    stage: 0xffdfec,
    surface: 0xffffff,
    surface_raised: 0xffffff,
    surface_hover: 0xffdcea,
    border: 0xf7b9d4,
    hairline: 0xffd3e5,
    card: 0xffffff,
    primary: 0x40122a,
    secondary: 0x8a3f63,
    muted: 0xa86a88,
    accent: 0xb3105e,
    accent_hover: 0xcc1f70,
    accent_text: 0xffffff,
    selection: 0xffc6de,
    destructive: 0x7a2c52,
    live: 0xff4f9f,
};

pub const fn palette_for(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => DARK_PALETTE,
        Theme::Light => LIGHT_PALETTE,
        Theme::Cafe => CAFE_PALETTE,
        Theme::Pink => PINK_PALETTE,
        Theme::Candy => CANDY_PALETTE,
    }
}

const RADIUS: f32 = 10.0;
const RADIUS_SMALL: f32 = 8.0;
const RADIUS_LARGE: f32 = 12.0;
const SIDEBAR_WIDTH: f32 = 212.0;
const SIDEBAR_COLLAPSED_WIDTH: f32 = 64.0;
const CONTENT_PADDING: f32 = 28.0;
/// Frame visible around the stage card on its three free sides.
const STAGE_INSET: f32 = 8.0;
const SIDEBAR_ROW_INSET: f32 = 12.0;
/// Row icons share one column in both sidebar states, so nothing jumps while it folds.
const SIDEBAR_ICON_LEFT: f32 = 24.0;
const LIBRARY_BODY_OFFSET: f32 = 58.0;
const POPOVER_WIDTH: f32 = 304.0;
/// Top of the video stage in preview and editor; the player child window uses it too.
const PLAYER_TOP: f32 = STAGE_INSET + 22.0 + 54.0;
const CAPTURE_ROW_HEIGHT: f32 = 36.0;
const FILTER_PANEL_WIDTH: f32 = 272.0;
const CLIP_COLUMN_GAP: f32 = 20.0;
const CLIP_ROW_GAP: f32 = 20.0;
const CLIP_META_HEIGHT: f32 = 52.0;
const CLIP_SECTION_HEADER: f32 = 34.0;
const CLIP_LIST_ROW_HEIGHT: f32 = 62.0;
const CLIP_GROUP_GAP: f32 = 22.0;
const CLIP_SCROLL_RESERVE: f32 = 14.0;
const FILTER_ROW_PITCH: f32 = 56.0;
const FOLDER_COLUMN_WIDTH: f32 = 236.0;
const FOLDER_COLUMN_GAP: f32 = 24.0;
const FOLDER_ROW_HEIGHT: f32 = 30.0;
const SETTINGS_RAIL_WIDTH: f32 = 184.0;
const SETTINGS_LINE_HEIGHT: f32 = 58.0;
const SETTINGS_CONTENT_WIDTH: f32 = 640.0;
const METER_WIDTH: f32 = 84.0;
const NAVIGATION_TOP: f32 = 62.0;
const NAVIGATION_HEIGHT: f32 = 30.0;
const NAVIGATION_PITCH: f32 = 32.0;
const EDITOR_BOTTOM_RESERVE: f32 = 226.0;
const EDITOR_TIMELINE_HEIGHT: f32 = 118.0;

#[derive(Debug, Clone, Copy)]
enum Glyph {
    Library,
    Collections,
    Settings,
    Folder,
    Search,
    Grid,
    List,
    More,
    Clock,
    Monitor,
    Audio,
    Quality,
    Filter,
    Microphone,
    Star,
    StarFilled,
    External,
    Pencil,
    Play,
    Pause,
    ChevronLeft,
    ChevronRight,
    Fullscreen,
    ChevronDown,
    Close,
    Sidebar,
    Check,
    Record,
    Info,
    UpDown,
    Undo,
    Redo,
    Muted,
}

enum Segment<'a> {
    Label(&'a str),
    Icon(Glyph),
}

enum LineControl<'a> {
    Switch(bool, Action),
    Popup(&'a str, Action),
    /// A value to press; the flag marks a capture in progress.
    Pill(&'a str, Action, bool),
    Slider(u16, Action),
}

#[derive(Clone, Copy)]
enum TextInputTarget {
    Search,
    Prompt,
}

#[derive(Debug, Clone, Copy)]
pub struct LogicalRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl LogicalRect {
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }

    fn d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.left,
            top: self.top,
            right: self.right,
            bottom: self.bottom,
        }
    }
}

pub fn player_bounds(model: &UiModel, width: u32, height: u32) -> LogicalRect {
    let aspect_ratio = model.player_aspect_ratio;
    let width = width as f32;
    let left = sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING;
    let right = width - CONTENT_PADDING;
    let detail = if right - left >= 960.0 { 300.0 } else { 0.0 };
    fit_aspect(
        rect(
            left,
            PLAYER_TOP,
            right - detail,
            (height as f32 - 178.0).max(390.0),
        ),
        aspect_ratio,
    )
}

pub fn editor_player_bounds(model: &UiModel, width: u32, height: u32) -> LogicalRect {
    let aspect_ratio = model.player_aspect_ratio;
    let width = width as f32;
    let left = sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING;
    let right = width - CONTENT_PADDING;
    let detail = if right - left >= 960.0 { 300.0 } else { 0.0 };
    fit_aspect(
        rect(
            left,
            PLAYER_TOP,
            right - detail,
            (height as f32 - EDITOR_BOTTOM_RESERVE).max(360.0),
        ),
        aspect_ratio,
    )
}

pub fn editor_timeline_rail(model: &UiModel, width: u32, height: u32) -> LogicalRect {
    let left = sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING;
    let right = width as f32 - CONTENT_PADDING;
    let stage = editor_player_bounds(model, width, height);
    let timeline_top = stage.bottom + 92.0;
    rect(
        left + 24.0,
        timeline_top + 34.0,
        right - 24.0,
        timeline_top + 94.0,
    )
}

pub fn editor_timeline_fraction(rail: LogicalRect, x: f32) -> u16 {
    (((x - rail.left) / (rail.right - rail.left).max(1.0)).clamp(0.0, 1.0) * 1000.0).round() as u16
}

pub fn player_timeline_rail(model: &UiModel, width: u32, height: u32) -> LogicalRect {
    let stage = player_bounds(model, width, height);
    rect(
        stage.left + 260.0,
        stage.bottom + 35.0,
        stage.right - 64.0,
        stage.bottom + 41.0,
    )
}

pub fn player_volume_rail(model: &UiModel, width: u32, height: u32) -> LogicalRect {
    let stage = player_bounds(model, width, height);
    let switch_top = (stage.top + stage.bottom) / 2.0 - 22.0;
    let volume_x = stage.right + 78.0;
    let volume_top = switch_top - 76.0;
    rect(
        volume_x - 2.5,
        volume_top + 38.0,
        volume_x + 2.5,
        switch_top + 120.0,
    )
}

pub fn fullscreen_timeline_rail(width: u32, height: u32) -> LogicalRect {
    let width = width as f32;
    let _ = height;
    rect(42.0, 16.0, (width - 42.0).max(43.0), 22.0)
}

pub fn fullscreen_volume_rail(width: u32, height: u32) -> LogicalRect {
    let width = width as f32;
    let height = height as f32;
    rect(204.0, height - 34.0, width.min(334.0), height - 28.0)
}

pub fn settings_audio_gain_rail(
    model: &UiModel,
    width: u32,
    _height: u32,
    row: usize,
) -> LogicalRect {
    let left = sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING;
    let right = width as f32 - CONTENT_PADDING;
    settings_slider_track(settings_line_rect(left, right, content_top(), row))
}

fn device_name<'a>(
    devices: &'a [(String, String)],
    chosen: Option<&String>,
    fallback: &'a str,
) -> &'a str {
    chosen
        .and_then(|id| devices.iter().find(|(device_id, _)| device_id == id))
        .map_or(fallback, |(_, name)| name.as_str())
}

fn settings_content(left: f32, right: f32) -> (f32, f32) {
    let content_left = left + SETTINGS_RAIL_WIDTH + 33.0;
    (
        content_left,
        right.min(content_left + SETTINGS_CONTENT_WIDTH),
    )
}

fn settings_line_rect(left: f32, right: f32, top: f32, index: usize) -> LogicalRect {
    let (content_left, content_right) = settings_content(left, right);
    let card_top = top + 52.0;
    rect(
        content_left,
        card_top + index as f32 * SETTINGS_LINE_HEIGHT,
        content_right,
        card_top + (index + 1) as f32 * SETTINGS_LINE_HEIGHT,
    )
}

fn settings_slider_track(line: LogicalRect) -> LogicalRect {
    let center = (line.top + line.bottom) / 2.0;
    rect(
        line.right - 14.0 - 150.0,
        center - 2.0,
        line.right - 14.0,
        center + 2.0,
    )
}

pub fn settings_gain_percent(rail: LogicalRect, x: f32) -> u16 {
    (((x - rail.left) / (rail.right - rail.left).max(1.0)).clamp(0.0, 1.0) * 200.0).round() as u16
}

fn fit_aspect(area: LogicalRect, aspect_ratio: f32) -> LogicalRect {
    let aspect_ratio = if aspect_ratio.is_finite() && aspect_ratio > 0.1 {
        aspect_ratio
    } else {
        16.0 / 9.0
    };
    let available_width = (area.right - area.left).max(1.0);
    let available_height = (area.bottom - area.top).max(1.0);
    let (width, height) = if available_width / available_height > aspect_ratio {
        (available_height * aspect_ratio, available_height)
    } else {
        (available_width, available_width / aspect_ratio)
    };
    let left = area.left + (available_width - width) / 2.0;
    let top = area.top + (available_height - height) / 2.0;
    rect(left, top, left + width, top + height)
}

#[derive(Clone)]
struct HitRegion {
    rect: LogicalRect,
    action: Action,
}

pub struct Renderer {
    d2d_factory: ID2D1Factory,
    round_stroke: ID2D1StrokeStyle,
    palette: Palette,
    strings: &'static Strings,
    hover_style: HoverStyle,
    hover_strength: f32,
    write_factory: IDWriteFactory,
    target: Option<ID2D1HwndRenderTarget>,
    page_title: IDWriteTextFormat,
    section: IDWriteTextFormat,
    brand: IDWriteTextFormat,
    heading: IDWriteTextFormat,
    caption: IDWriteTextFormat,
    strong: IDWriteTextFormat,
    body: IDWriteTextFormat,
    small: IDWriteTextFormat,
    small_center: IDWriteTextFormat,
    small_right: IDWriteTextFormat,
    body_trailing: IDWriteTextFormat,
    body_center: IDWriteTextFormat,
    strong_center: IDWriteTextFormat,
    button: IDWriteTextFormat,
    button_leading: IDWriteTextFormat,
    hits: Vec<HitRegion>,
    wic_factory: IWICImagingFactory,
    thumbnails: HashMap<PathBuf, ID2D1Bitmap>,
    app_icon: Option<ID2D1Bitmap>,
    clip_durations: HashMap<PathBuf, Option<u64>>,
    thumbnail_order: VecDeque<PathBuf>,
    unavailable_thumbnails: HashSet<PathBuf>,
    consecutive_failures: u32,
    hovered: Option<Action>,
    hover_progress: f32,
    reduced_motion: bool,
    navigation_motion: Option<crate::motion::Motion>,
    sidebar_motion: Option<crate::motion::Motion>,
    rail: f32,
    navigation_was_moving: bool,
    hover_started: Instant,
    toggle_motions: HashMap<&'static str, crate::motion::Motion>,
}

impl Renderer {
    const MAX_THUMBNAILS: usize = 32;

    pub fn new() -> Result<Self, String> {
        let d2d_factory =
            unsafe { D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }
                .map_err(|error| error.to_string())?;
        let write_factory =
            unsafe { DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED) }
                .map_err(|error| error.to_string())?;
        let page_title = text_format(
            &write_factory,
            w!("Segoe UI Variable Display"),
            22.0,
            true,
            false,
        )?;
        let section = text_format(
            &write_factory,
            w!("Segoe UI Variable Display"),
            14.0,
            true,
            false,
        )?;
        let brand = text_format(
            &write_factory,
            w!("Segoe UI Variable Display"),
            14.5,
            true,
            false,
        )?;
        let heading = text_format(
            &write_factory,
            w!("Segoe UI Variable Display"),
            17.0,
            true,
            false,
        )?;
        let caption = text_format(
            &write_factory,
            w!("Segoe UI Variable Text"),
            11.5,
            true,
            false,
        )?;
        let strong = text_format(
            &write_factory,
            w!("Segoe UI Variable Text"),
            13.0,
            true,
            false,
        )?;
        let body = text_format(
            &write_factory,
            w!("Segoe UI Variable Text"),
            13.0,
            false,
            false,
        )?;
        let small = text_format(
            &write_factory,
            w!("Segoe UI Variable Text"),
            11.5,
            false,
            false,
        )?;
        let small_center = text_format(
            &write_factory,
            w!("Segoe UI Variable Text"),
            11.0,
            true,
            true,
        )?;
        let small_right = text_format_trailing(&write_factory, w!("Segoe UI Variable Text"), 11.5)?;
        let body_trailing =
            text_format_trailing(&write_factory, w!("Segoe UI Variable Text"), 13.0)?;
        let body_center = text_format(
            &write_factory,
            w!("Segoe UI Variable Text"),
            13.0,
            false,
            true,
        )?;
        let strong_center = text_format(
            &write_factory,
            w!("Segoe UI Variable Text"),
            12.0,
            true,
            true,
        )?;
        let button = text_format(
            &write_factory,
            w!("Segoe UI Variable Text"),
            12.5,
            true,
            true,
        )?;
        let button_leading = text_format(
            &write_factory,
            w!("Segoe UI Variable Text"),
            12.5,
            true,
            false,
        )?;
        let round_stroke = unsafe {
            d2d_factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 10.0,
                    dashStyle: D2D1_DASH_STYLE_SOLID,
                    dashOffset: 0.0,
                },
                None,
            )
        }
        .map_err(|error| error.to_string())?;
        let wic_factory =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
                .map_err(|error| error.to_string())?;
        let mut animations_enabled = 1_i32;
        let motion_setting_available = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some((&mut animations_enabled as *mut i32).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .is_ok();
        Ok(Self {
            d2d_factory,
            round_stroke,
            palette: palette_for(Theme::default()),
            strings: crate::text::strings(Language::default()),
            hover_style: HoverStyle::default(),
            hover_strength: 1.0,
            write_factory,
            target: None,
            page_title,
            section,
            brand,
            heading,
            caption,
            strong,
            body,
            small,
            small_center,
            small_right,
            body_trailing,
            body_center,
            strong_center,
            button,
            button_leading,
            hits: Vec::new(),
            wic_factory,
            thumbnails: HashMap::new(),
            app_icon: None,
            clip_durations: HashMap::new(),
            thumbnail_order: VecDeque::new(),
            unavailable_thumbnails: HashSet::new(),
            consecutive_failures: 0,
            hovered: None,
            hover_progress: 0.0,
            reduced_motion: motion_setting_available && animations_enabled == 0,
            navigation_motion: None,
            sidebar_motion: None,
            rail: SIDEBAR_WIDTH,
            navigation_was_moving: false,
            hover_started: Instant::now(),
            toggle_motions: HashMap::new(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let resize_failed = self
            .target
            .as_ref()
            .is_some_and(|target| unsafe { target.Resize(&D2D_SIZE_U { width, height }) }.is_err());
        if resize_failed {
            self.target = None;
            self.release_cached_images();
        }
    }

    pub fn retry_unavailable_thumbnails(&mut self) {
        self.unavailable_thumbnails.clear();
    }

    pub fn release_cached_images(&mut self) {
        self.app_icon = None;
        self.thumbnails.clear();
        self.thumbnail_order.clear();
    }

    fn touch_thumbnail(&mut self, path: &Path) {
        if let Some(index) = self
            .thumbnail_order
            .iter()
            .position(|cached| cached.as_path() == path)
        {
            let cached = self.thumbnail_order.remove(index).expect("index found");
            self.thumbnail_order.push_back(cached);
        }
    }

    fn evict_cold_thumbnails(&mut self) {
        while self.thumbnail_order.len() > Self::MAX_THUMBNAILS {
            if let Some(cold) = self.thumbnail_order.pop_front() {
                self.thumbnails.remove(&cold);
            }
        }
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
            self.hover_progress = crate::motion::Curve::Quick.ease(
                now.saturating_duration_since(self.hover_started)
                    .as_secs_f32()
                    / 0.14,
            );
            changed = true;
        }
        changed
    }

    fn apply_appearance(&mut self, model: &UiModel) {
        let config = &model.config;
        self.palette = palette_for(config.appearance.theme);
        self.strings = model.strings();
        self.hover_style = config.appearance.hover;
        self.hover_strength = config.appearance.hover_strength.factor();
    }

    /// Hover fill blend, following the personalised style and strength.
    fn hover_fill(&self, base: u32, target: u32) -> u32 {
        if !self.hover_style.fills() {
            return base;
        }
        mix(base, target, self.hover_amount(1.0))
    }

    fn hover_amount(&self, weight: f32) -> f32 {
        hover_blend_amount(self.hover_progress, self.hover_strength, weight)
    }

    fn is_hovered(&self, action: &Action) -> bool {
        self.hovered.as_ref() == Some(action)
    }

    pub fn paint(
        &mut self,
        window: HWND,
        model: &UiModel,
        width: u32,
        height: u32,
        fullscreen: bool,
    ) -> Result<(), String> {
        self.apply_appearance(model);
        self.ensure_target(window, width, height)?;
        self.hits.clear();
        let target = self.target.as_ref().expect("render target exists").clone();
        unsafe {
            target.BeginDraw();
            target.Clear(Some(&color(self.palette.canvas)));
        }
        let drawn = self.render_frame(model, width, height, fullscreen);
        let ended = unsafe { target.EndDraw(None, None) }.map_err(|error| error.to_string());
        let outcome = drawn.and(ended);
        if outcome.is_err() {
            self.discard_target();
            self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        } else {
            self.consecutive_failures = 0;
        }
        outcome
    }

    pub fn paint_fullscreen_controls(
        &mut self,
        window: HWND,
        model: &UiModel,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.apply_appearance(model);
        self.ensure_target(window, width, height)?;
        self.hits.clear();
        let target = self.target.as_ref().expect("render target exists").clone();
        unsafe {
            target.BeginDraw();
            target.Clear(Some(&color(self.palette.canvas)));
        }
        let drawn = self.render_fullscreen_controls(model, width, height);
        let ended = unsafe { target.EndDraw(None, None) }.map_err(|error| error.to_string());
        let outcome = drawn.and(ended);
        if outcome.is_err() {
            self.discard_target();
            self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        } else {
            self.consecutive_failures = 0;
        }
        outcome
    }

    pub fn wants_recovery_repaint(&self) -> bool {
        self.consecutive_failures == 1
    }

    pub fn is_failing(&self) -> bool {
        self.consecutive_failures > 1
    }

    fn measure(&self, value: &str, format: &IDWriteTextFormat) -> f32 {
        if value.is_empty() {
            return 0.0;
        }
        let wide = value.encode_utf16().collect::<Vec<_>>();
        let Ok(layout) = (unsafe {
            self.write_factory
                .CreateTextLayout(&wide, format, 4096.0, 4096.0)
        }) else {
            return 0.0;
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        if unsafe { layout.GetMetrics(&mut metrics) }.is_err() {
            return 0.0;
        }
        metrics.widthIncludingTrailingWhitespace
    }

    fn shorten(&self, value: &str, format: &IDWriteTextFormat, max_width: f32) -> String {
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

    fn discard_target(&mut self) {
        self.target = None;
        self.release_cached_images();
        self.unavailable_thumbnails.clear();
    }

    fn render_frame(
        &mut self,
        model: &UiModel,
        width: u32,
        height: u32,
        fullscreen: bool,
    ) -> Result<(), String> {
        if fullscreen && model.page == Page::Player {
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
            let label = self.strings.move_drag(drag.count);
            let chip_width = 138.0;
            let chip_height = 36.0;
            let left = (drag.x + 14.0).clamp(12.0, width as f32 - chip_width - 12.0);
            let top = (drag.y + 14.0).clamp(12.0, height as f32 - chip_height - 12.0);
            let chip = rect(left, top, left + chip_width, top + chip_height);
            self.fill(chip, self.palette.surface_raised, RADIUS)?;
            self.stroke(chip, self.palette.accent, RADIUS, 1.0)?;
            self.text(
                &label,
                chip,
                &self.body_center.clone(),
                self.palette.primary,
            )?;
        }
        Ok(())
    }

    fn render_fullscreen_header(&mut self, width: f32) -> Result<(), String> {
        self.fill(rect(0.0, 0.0, width, 78.0), 0x000000, 0.0)?;
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

    fn render_fullscreen_controls(
        &mut self,
        model: &UiModel,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let width = width as f32;
        let height = height as f32;
        self.fill(rect(0.0, 0.0, width, height), 0x000000, 0.0)?;
        let timeline = rect(42.0, 17.0, width - 42.0, 21.0);
        self.draw_progress_rail(model, timeline)?;
        self.hits.push(HitRegion {
            rect: rect(timeline.left, 2.0, timeline.right, 36.0),
            action: Action::DragPlayerSeek,
        });
        let row_top = 34.0;
        let row_bottom = 78.0;
        self.floating_glyph(
            rect(24.0, row_top, 66.0, row_bottom),
            if model.player_playing {
                Glyph::Pause
            } else {
                Glyph::Play
            },
            self.palette.primary,
            Some(Action::PlayPause),
        )?;
        self.floating_glyph(
            rect(72.0, row_top, 112.0, row_bottom),
            Glyph::ChevronLeft,
            self.palette.muted,
            model.adjacent_clip(-1).map(|_| Action::PreviousClip),
        )?;
        self.floating_glyph(
            rect(116.0, row_top, 156.0, row_bottom),
            Glyph::ChevronRight,
            self.palette.muted,
            model.adjacent_clip(1).map(|_| Action::NextClip),
        )?;
        self.text(
            &format!(
                "{} / {}",
                format_player_time(model.player_position_seconds),
                format_player_time(model.player_duration_seconds)
            ),
            rect(174.0, row_top, 302.0, row_bottom),
            &self.small.clone(),
            self.palette.muted,
        )?;
        self.floating_glyph(
            rect(width - 108.0, row_top, width - 66.0, row_bottom),
            if model.player_volume_percent == 0 {
                Glyph::Muted
            } else {
                Glyph::Audio
            },
            self.palette.primary,
            Some(Action::ToggleMute),
        )?;
        self.floating_glyph(
            rect(width - 60.0, row_top, width - 18.0, row_bottom),
            Glyph::Fullscreen,
            self.palette.primary,
            Some(Action::ToggleFullscreen),
        )?;
        let info = rect(18.0, 92.0, width - 18.0, height - 10.0);
        self.fill(info, 0x161616, RADIUS_LARGE)?;
        if let Some(clip) = model.active_clip() {
            let preview = rect(
                info.left + 14.0,
                info.top + 12.0,
                info.left + 104.0,
                info.bottom - 12.0,
            );
            self.fill(preview, self.palette.stage, RADIUS_SMALL)?;
            let _ = self.draw_thumbnail(&clip.path, preview, RADIUS_SMALL)?;
            self.text(
                &clip.title,
                rect(
                    info.left + 120.0,
                    info.top + 12.0,
                    info.left + 440.0,
                    info.top + 40.0,
                ),
                &self.strong.clone(),
                self.palette.primary,
            )?;
            self.text(
                &format!(
                    "{}  ·  {}  ·  {}×{}",
                    age(clip.modified),
                    format_bytes(clip.size_bytes),
                    model.player_video_width,
                    model.player_video_height
                ),
                rect(
                    info.left + 120.0,
                    info.top + 40.0,
                    info.left + 480.0,
                    info.bottom - 12.0,
                ),
                &self.small.clone(),
                self.palette.muted,
            )?;
            let mut x = info.right - 14.0;
            let mut actions = vec![
                (self.strings.open_folder, Action::OpenClipsFolder, false),
                (self.strings.edit_clip, Action::EditActiveClip, false),
            ];
            if let Some(index) = model.active_clip {
                actions.insert(
                    0,
                    (self.strings.delete_clip, Action::DeleteClip(index), true),
                );
            }
            let center = (info.top + info.bottom) / 2.0;
            for (label, action, destructive) in actions {
                let width = self.measure(label, &self.small) + 22.0;
                let area = rect(x - width, center - 13.0, x, center + 13.0);
                self.quick_button(area, label, action, destructive)?;
                x = area.left - 8.0;
            }
        }
        Ok(())
    }

    fn ensure_target(&mut self, window: HWND, width: u32, height: u32) -> Result<(), String> {
        if self.target.is_some() {
            return Ok(());
        }
        let properties = D2D1_RENDER_TARGET_PROPERTIES::default();
        let window_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd: window,
            pixelSize: D2D_SIZE_U { width, height },
            ..Default::default()
        };
        self.target = Some(
            unsafe {
                self.d2d_factory
                    .CreateHwndRenderTarget(&properties, &window_properties)
            }
            .map_err(|error| error.to_string())?,
        );
        Ok(())
    }

    fn render_shell(&mut self, model: &UiModel, width: f32, height: f32) -> Result<(), String> {
        let target = sidebar_width(model.sidebar_collapsed);
        let now = Instant::now();
        let reduced = self.reduced_motion;
        let motion = self.sidebar_motion.get_or_insert_with(|| {
            crate::motion::Motion::with_curve(target, crate::motion::Curve::Glide)
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
        let left = self.rail + CONTENT_PADDING;
        let right = width - CONTENT_PADDING;
        let chrome = page_has_chrome(model.page);
        let top = if chrome { content_top() } else { 0.0 };
        let bottom = content_bottom(height, chrome);
        match model.page {
            Page::Library => self.render_library(model, left, right, top, bottom)?,
            Page::Collections => self.render_collections(model, left, right, top, bottom)?,
            Page::Settings => self.render_settings(model, left, right, top, bottom)?,
            Page::Player => self.render_player(model, left, right, height)?,
            Page::Editor => self.render_editor(model, left, right, height)?,
        }
        if model.capture_panel_open {
            self.render_capture_panel(model, height)?;
        } else {
            self.toggle_motions.remove("capture_panel");
        }
        Ok(())
    }

    fn render_sidebar(&mut self, model: &UiModel, height: f32) -> Result<(), String> {
        let rail = self.rail;
        // 0 while collapsed, 1 while expanded; labels fade with it while the edge clips them
        let reveal = self.sidebar_reveal();
        self.fill(rect(0.0, 0.0, rail, height), self.palette.rail, 0.0)?;
        self.push_clip(rect(0.0, 0.0, rail, height))?;
        let painted = self.render_sidebar_content(model, height, rail, reveal);
        self.pop_clip();
        painted
    }

    fn sidebar_reveal(&self) -> f32 {
        ((self.rail - SIDEBAR_COLLAPSED_WIDTH) / (SIDEBAR_WIDTH - SIDEBAR_COLLAPSED_WIDTH))
            .clamp(0.0, 1.0)
    }

    fn render_sidebar_content(
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
            self.text(
                "rewa",
                rect(52.0, 14.0, rail - 12.0, 40.0),
                &self.brand.clone(),
                mix(self.palette.rail, self.palette.primary, reveal),
            )?;
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
            crate::motion::Motion::with_curve(selected_top, crate::motion::Curve::Glide)
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

    fn sidebar_item(
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
    fn render_replay_block(&mut self, model: &UiModel, bottom: f32) -> Result<(), String> {
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

    #[allow(clippy::too_many_arguments)]
    /// Replay state and the save action live at the foot of the sidebar, so every
    /// page can save without a toolbar; the state row opens the capture popover.
    fn render_capture_panel(&mut self, model: &UiModel, height: f32) -> Result<(), String> {
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
        let now = Instant::now();
        let reduced = self.reduced_motion;
        let motion = self
            .toggle_motions
            .entry("capture_panel")
            .or_insert_with(|| crate::motion::Motion::new(0.0));
        motion.retarget(1.0, now, reduced);
        let progress = motion.value(now);
        self.navigation_was_moving |= motion.active(now);

        // the panel floats over the page and swallows clicks before the page sees them
        self.hits.push(HitRegion {
            rect: panel,
            action: Action::Ignore,
        });
        self.with_arrival(
            Vector2 {
                X: panel.left,
                Y: panel.bottom,
            },
            progress,
            |renderer| {
                renderer.popover_surface(panel)?;
                renderer.capture_panel_content(model, panel)
            },
        )
    }

    fn capture_panel_content(&mut self, model: &UiModel, panel: LogicalRect) -> Result<(), String> {
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
        let hotkey = rewa_windows::hotkey::localized_hotkey_label(&model.config.hotkey);
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

    fn capture_row(
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

    /// Menus and popovers: one raised surface with a hairline edge and a soft,
    /// offset shadow built from stacked translucent layers.
    fn popover_surface(&self, area: LogicalRect) -> Result<(), String> {
        for (spread, alpha) in [(14.0, 0.05), (9.0, 0.07), (5.0, 0.09), (2.0, 0.12)] {
            self.fill_alpha(
                rect(
                    area.left - spread,
                    area.top - spread + 8.0,
                    area.right + spread,
                    area.bottom + spread + 8.0,
                ),
                0x000000,
                alpha,
                RADIUS_LARGE + spread,
            )?;
        }
        self.fill(area, self.palette.surface_raised, RADIUS_LARGE)?;
        self.stroke(
            area,
            mix(self.palette.surface_raised, self.palette.primary, 0.1),
            RADIUS_LARGE,
            1.0,
        )
    }

    /// Leech's plate: the dialog surface, a larger radius and a deeper shadow than a menu.
    fn plate_surface(&self, area: LogicalRect) -> Result<(), String> {
        for (spread, alpha) in [(26.0, 0.04), (16.0, 0.06), (8.0, 0.08), (3.0, 0.1)] {
            self.fill_alpha(
                rect(
                    area.left - spread,
                    area.top - spread + 12.0,
                    area.right + spread,
                    area.bottom + spread + 12.0,
                ),
                0x000000,
                alpha,
                16.0 + spread,
            )?;
        }
        self.fill(area, self.palette.surface, 16.0)?;
        self.stroke(area, self.palette.border, 16.0, 1.0)
    }

    /// Scales and fades a popover in from its anchor corner on the settle spring.
    fn with_arrival(
        &mut self,
        origin: Vector2,
        progress: f32,
        body: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        let Some(target) = self.target.clone() else {
            return body(self);
        };
        if progress >= 1.0 {
            return body(self);
        }
        let scale = 0.94 + 0.06 * progress;
        let layer = unsafe { target.CreateLayer(None) }.map_err(|error| error.to_string())?;
        let parameters = D2D1_LAYER_PARAMETERS {
            contentBounds: D2D_RECT_F {
                left: f32::MIN,
                top: f32::MIN,
                right: f32::MAX,
                bottom: f32::MAX,
            },
            geometricMask: std::mem::ManuallyDrop::new(None),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: progress.clamp(0.0, 1.0),
            opacityBrush: std::mem::ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS_NONE,
        };
        unsafe {
            target.SetTransform(&Matrix3x2 {
                M11: scale,
                M12: 0.0,
                M21: 0.0,
                M22: scale,
                M31: origin.X * (1.0 - scale),
                M32: origin.Y * (1.0 - scale),
            });
            target.PushLayer(&parameters, &layer);
        }
        let painted = body(self);
        unsafe {
            target.PopLayer();
            target.SetTransform(&Matrix3x2::identity());
        }
        painted
    }

    /// Translucent ink over whatever is below: hover and pressed washes.
    fn tint(&self, area: LogicalRect, amount: f32, radius: f32) -> Result<(), String> {
        self.fill_alpha(area, self.palette.primary, amount, radius)
    }

    /// Leech's door: a 26 px icon button that only shows a wash on hover.
    fn door(
        &mut self,
        area: LogicalRect,
        glyph: Glyph,
        active: bool,
        action: Action,
    ) -> Result<(), String> {
        let hovered = self.is_hovered(&action);
        if active {
            self.tint(area, 0.1, RADIUS_SMALL)?;
        } else if hovered {
            self.tint(area, 0.07 * self.hover_amount(1.0), RADIUS_SMALL)?;
        }
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
            if active {
                self.palette.primary
            } else {
                mix(
                    self.palette.muted,
                    self.palette.primary,
                    if hovered {
                        0.5 * self.hover_amount(1.0)
                    } else {
                        0.0
                    },
                )
            },
        )?;
        self.hits.push(HitRegion { rect: area, action });
        Ok(())
    }

    /// A macOS segmented control; the thumb glides between segments.
    fn segmented(
        &mut self,
        area: LogicalRect,
        segments: &[(Segment<'_>, Action)],
        selected: usize,
        key: &'static str,
    ) -> Result<(), String> {
        self.tint(area, 0.06, RADIUS_SMALL)?;
        let width = (area.right - area.left - 4.0) / segments.len() as f32;
        let now = Instant::now();
        let reduced = self.reduced_motion;
        let motion = self.toggle_motions.entry(key).or_insert_with(|| {
            crate::motion::Motion::with_curve(selected as f32, crate::motion::Curve::Glide)
        });
        motion.retarget(selected as f32, now, reduced);
        let position = motion.value(now);
        self.navigation_was_moving |= motion.active(now);
        let thumb = rect(
            area.left + 2.0 + position * width,
            area.top + 2.0,
            area.left + 2.0 + (position + 1.0) * width,
            area.bottom - 2.0,
        );
        self.fill_alpha(
            rect(thumb.left, thumb.top + 1.0, thumb.right, thumb.bottom + 1.0),
            0x000000,
            0.18,
            RADIUS_SMALL - 1.0,
        )?;
        self.fill(thumb, self.palette.surface_raised, RADIUS_SMALL - 1.0)?;
        for (index, (segment, action)) in segments.iter().enumerate() {
            let cell = rect(
                area.left + 2.0 + index as f32 * width,
                area.top,
                area.left + 2.0 + (index + 1) as f32 * width,
                area.bottom,
            );
            let active = index == selected;
            let hovered = !active && self.is_hovered(action);
            let tone = if active {
                self.palette.primary
            } else if hovered {
                self.palette.secondary
            } else {
                self.palette.muted
            };
            match segment {
                Segment::Label(label) => {
                    self.text(label, cell, &self.button.clone(), tone)?;
                }
                Segment::Icon(glyph) => {
                    let center_x = (cell.left + cell.right) / 2.0;
                    let center_y = (cell.top + cell.bottom) / 2.0;
                    self.glyph(
                        *glyph,
                        rect(
                            center_x - 7.5,
                            center_y - 7.5,
                            center_x + 7.5,
                            center_y + 7.5,
                        ),
                        tone,
                    )?;
                }
            }
            self.hits.push(HitRegion {
                rect: cell,
                action: action.clone(),
            });
        }
        Ok(())
    }

    fn status_dot(&self, x: f32, y: f32, live: bool) -> Result<(), String> {
        let target = self.target.as_ref().expect("render target exists");
        let brush = unsafe {
            target.CreateSolidColorBrush(
                &color(if live {
                    self.palette.live
                } else {
                    self.palette.muted
                }),
                None,
            )
        }
        .map_err(|error| error.to_string())?;
        unsafe {
            target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2 { X: x, Y: y },
                    radiusX: 4.0,
                    radiusY: 4.0,
                },
                &brush,
            );
        }
        Ok(())
    }

    fn action_button(
        &mut self,
        area: LogicalRect,
        label: &str,
        action: Action,
    ) -> Result<(), String> {
        let background = if self.is_hovered(&action) {
            mix(
                self.palette.accent,
                self.palette.accent_hover,
                self.hover_amount(1.0),
            )
        } else {
            self.palette.accent
        };
        self.fill(area, background, RADIUS_SMALL)?;
        self.text(label, area, &self.button.clone(), self.palette.accent_text)?;
        self.hits.push(HitRegion { rect: area, action });
        Ok(())
    }

    fn level_meter(
        &self,
        area: LogicalRect,
        enabled: bool,
        level: u8,
        hold: u8,
    ) -> Result<(), String> {
        const BARS: usize = 16;
        let pitch = (area.right - area.left) / BARS as f32;
        let bars = |value: u8| (f32::from(value) / 100.0 * BARS as f32).round() as usize;
        let lit = if enabled { bars(level) } else { 0 };
        let marker = if enabled { bars(hold) } else { 0 };
        for index in 0..BARS {
            let x = area.left + index as f32 * pitch;
            let scale = 0.45 + 0.55 * (index as f32 / (BARS - 1) as f32);
            let height = (area.bottom - area.top) * scale;
            let bar = rect(
                x,
                area.bottom - height,
                x + (pitch - 2.0).max(1.5),
                area.bottom,
            );
            self.fill(
                bar,
                if index < lit {
                    self.palette.live
                } else if marker > 0 && index + 1 == marker {
                    self.palette.secondary
                } else if enabled {
                    self.palette.border
                } else {
                    mix(self.palette.canvas, self.palette.border, 0.6)
                },
                1.0,
            )?;
        }
        Ok(())
    }

    fn render_library(
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

        let view = rect(right - 64.0, top + 3.0, right, top + 29.0);
        self.segmented(
            view,
            &[
                (Segment::Icon(Glyph::Grid), Action::SetLibraryGrid(true)),
                (Segment::Icon(Glyph::List), Action::SetLibraryGrid(false)),
            ],
            usize::from(!model.library_grid),
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
            let groups = model.clip_day_groups(&indices, today);
            let counts = groups
                .iter()
                .map(|group| group.indices.len())
                .collect::<Vec<_>>();
            let layout = library_layout(
                &counts,
                area.right - area.left - CLIP_SCROLL_RESERVE,
                model.library_grid,
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

    fn render_filter_panel(
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
                self.strings.filter_game,
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

    fn render_clip_sections(
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
            let rows_top = header_top + CLIP_SECTION_HEADER;
            if rows_top > area.bottom {
                break;
            }
            if header_top + CLIP_SECTION_HEADER > area.top {
                self.text(
                    &group.label,
                    rect(area.left, header_top, area.left + 300.0, header_top + 26.0),
                    &self.section.clone(),
                    self.palette.primary,
                )?;
                let count = group.indices.len();
                self.text(
                    &self.strings.clip_count(count),
                    rect(
                        area.right - 160.0 - CLIP_SCROLL_RESERVE,
                        header_top + 3.0,
                        area.right - CLIP_SCROLL_RESERVE,
                        header_top + 24.0,
                    ),
                    &self.small_right.clone(),
                    self.palette.muted,
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
                let card_left = area.left + column as f32 * (layout.card_width + CLIP_COLUMN_GAP);
                let card = rect(
                    card_left,
                    card_top,
                    card_left + layout.card_width,
                    card_top + layout.card_height,
                );
                if model.library_grid {
                    self.clip_card(model, index, card, area, today)?;
                } else {
                    self.clip_row(model, index, card, area, today)?;
                }
            }
        }
        Ok(())
    }

    fn clip_card(
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
        let lift = if hovered { self.hover_amount(1.0) } else { 0.0 };

        // the pointer target for the card is registered first so the overlay
        // buttons drawn on top of it keep their own targets
        self.push_clipped_hit(card, viewport, open);

        let resting = rect(
            card.left,
            card.top,
            card.right,
            card.bottom - CLIP_META_HEIGHT,
        );
        // hover lifts the picture by 2.5 %, the way tvOS and Photos answer a pointer
        let grow_x = (resting.right - resting.left) * 0.0125 * lift;
        let grow_y = (resting.bottom - resting.top) * 0.0125 * lift;
        let preview = rect(
            resting.left - grow_x,
            resting.top - grow_y,
            resting.right + grow_x,
            resting.bottom + grow_y,
        );
        if lift > 0.0 {
            self.fill_alpha(
                rect(
                    preview.left,
                    preview.top + 6.0,
                    preview.right,
                    preview.bottom + 8.0,
                ),
                0x000000,
                0.22 * lift,
                RADIUS + 4.0,
            )?;
        }
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

        let text_top = resting.bottom + 10.0;
        let more = rect(
            card.right - 26.0,
            text_top - 2.0,
            card.right,
            text_top + 22.0,
        );
        let show_more = hovered && !model.selection_mode;
        let title_right = if show_more {
            more.left - 6.0
        } else {
            card.right
        };
        self.text(
            &self.shorten(&clip.title, &self.strong, title_right - card.left),
            rect(card.left + 1.0, text_top, title_right, text_top + 19.0),
            &self.strong.clone(),
            self.palette.primary,
        )?;
        self.text(
            &format!(
                "{}  ·  {}",
                crate::clock::stamp_label(crate::clock::local(clip.modified), today, self.strings),
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

    fn clip_row(
        &mut self,
        model: &UiModel,
        index: usize,
        row: LogicalRect,
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
        let menu = Action::OpenClipMenu(index);
        let selected = model.clip_is_selected(index);
        if selected {
            self.fill(row, self.palette.surface_hover, RADIUS_SMALL)?;
        } else if self.is_hovered(&open) {
            self.fill(
                row,
                self.hover_fill(self.palette.canvas, self.palette.surface),
                RADIUS_SMALL,
            )?;
        }
        self.fill(
            rect(row.left, row.bottom - 1.0, row.right, row.bottom),
            self.palette.border,
            0.0,
        )?;
        let preview = rect(
            row.left + 8.0,
            row.top + 6.0,
            row.left + 96.0,
            row.bottom - 6.0,
        );
        self.fill(preview, self.palette.stage, RADIUS_SMALL)?;
        let _ = self.draw_thumbnail(&clip.path, preview, RADIUS_SMALL)?;
        if let Some(duration) = self.clip_duration(&clip.path) {
            let label = format_clip_badge_duration(duration);
            let badge = rect(
                preview.right - 14.0 - label.chars().count() as f32 * 6.5,
                preview.bottom - 20.0,
                preview.right - 4.0,
                preview.bottom - 4.0,
            );
            self.fill_alpha(badge, 0x000000, 0.72, RADIUS_SMALL)?;
            self.text(
                &label,
                badge,
                &self.strong_center.clone(),
                self.palette.primary,
            )?;
        }
        self.text(
            &clip.title,
            rect(preview.right + 16.0, row.top, row.right - 300.0, row.bottom),
            &self.strong.clone(),
            self.palette.primary,
        )?;
        self.text(
            &crate::clock::stamp_label(crate::clock::local(clip.modified), today, self.strings),
            rect(row.right - 296.0, row.top, row.right - 150.0, row.bottom),
            &self.small.clone(),
            self.palette.secondary,
        )?;
        self.text(
            &format_bytes(clip.size_bytes),
            rect(row.right - 146.0, row.top, row.right - 74.0, row.bottom),
            &self.small.clone(),
            self.palette.muted,
        )?;
        let star = rect(
            row.right - 68.0,
            row.top + 18.0,
            row.right - 42.0,
            row.bottom - 18.0,
        );
        self.glyph(
            Glyph::Star,
            rect(
                star.left + 3.0,
                star.top + 3.0,
                star.right - 3.0,
                star.bottom - 3.0,
            ),
            if model.is_favorite(index) {
                self.palette.primary
            } else if self.is_hovered(&favorite) {
                self.palette.secondary
            } else {
                self.palette.muted
            },
        )?;
        if model.is_favorite(index) {
            self.glyph(
                Glyph::StarFilled,
                rect(
                    star.left + 3.0,
                    star.top + 3.0,
                    star.right - 3.0,
                    star.bottom - 3.0,
                ),
                self.palette.primary,
            )?;
        }
        let more = rect(
            row.right - 34.0,
            row.top + 18.0,
            row.right - 8.0,
            row.bottom - 18.0,
        );
        self.glyph(
            Glyph::More,
            rect(
                more.left + 3.0,
                more.top + 7.0,
                more.right - 3.0,
                more.bottom - 7.0,
            ),
            if self.is_hovered(&menu) {
                self.palette.primary
            } else {
                self.palette.muted
            },
        )?;
        self.push_clipped_hit(row, viewport, open);
        if !model.selection_mode {
            self.push_clipped_hit(star, viewport, favorite);
            self.push_clipped_hit(more, viewport, menu);
        }
        Ok(())
    }

    fn overlay_button(
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

    fn dropdown(&mut self, area: LogicalRect, value: &str, action: Action) -> Result<(), String> {
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

    fn push_clip(&self, area: LogicalRect) -> Result<(), String> {
        let target = self.target.as_ref().expect("render target exists");
        unsafe { target.PushAxisAlignedClip(&area.d2d(), D2D1_ANTIALIAS_MODE_ALIASED) };
        Ok(())
    }

    fn pop_clip(&self) {
        if let Some(target) = self.target.as_ref() {
            unsafe { target.PopAxisAlignedClip() };
        }
    }

    fn push_clipped_hit(&mut self, area: LogicalRect, viewport: LogicalRect, action: Action) {
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

    fn render_collections(
        &mut self,
        model: &UiModel,
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
    ) -> Result<(), String> {
        let today = crate::clock::now();
        self.text(
            self.strings.collections,
            rect(left, top, left + 320.0, top + 32.0),
            &self.page_title.clone(),
            self.palette.primary,
        )?;

        let create_width = self.measure(self.strings.new_collection_button, &self.button) + 28.0;
        let create = rect(right - create_width, top + 2.0, right, top + 30.0);
        self.action_button(
            create,
            self.strings.new_collection_button,
            Action::CreateCollection,
        )?;
        let search = rect(
            (create.left - 10.0 - 220.0).max(left + 240.0),
            top + 2.0,
            create.left - 10.0,
            top + 30.0,
        );
        if search.right - search.left >= 120.0 {
            self.search_field(model, search, self.strings.search_collections)?;
        }

        let body_top = top + LIBRARY_BODY_OFFSET;
        let column = rect(left, body_top, left + FOLDER_COLUMN_WIDTH, bottom);
        self.render_folder_column(model, column)?;
        self.fill(
            rect(
                column.right + 12.0,
                body_top - 4.0,
                column.right + 13.0,
                bottom,
            ),
            self.palette.border,
            0.0,
        )?;

        let area_left = column.right + 12.0 + FOLDER_COLUMN_GAP;
        let active = model.active_collection.as_ref().and_then(|path| {
            model
                .collections
                .iter()
                .find(|collection| &collection.path == path)
        });
        let title = active.map_or(self.strings.all_clips, |collection| {
            collection.name.as_str()
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
                if active.is_some() {
                    self.strings.empty_collection
                } else {
                    self.strings.empty_no_clips
                },
                area.left,
                area.right,
                area.top + 8.0,
            )?;
        } else {
            let groups = model.clip_day_groups(&indices, today);
            let counts = groups
                .iter()
                .map(|group| group.indices.len())
                .collect::<Vec<_>>();
            let layout = library_layout(
                &counts,
                area.right - area.left - CLIP_SCROLL_RESERVE,
                model.library_grid,
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

    fn render_folder_column(&mut self, model: &UiModel, area: LogicalRect) -> Result<(), String> {
        self.text(
            self.strings.folders_label,
            rect(
                area.left + 4.0,
                area.top - 4.0,
                area.left + 140.0,
                area.top + 18.0,
            ),
            &self.caption.clone(),
            self.palette.muted,
        )?;
        let sort = Action::ToggleCollectionSort;
        let sort_area = rect(
            area.right - 70.0,
            area.top - 6.0,
            area.right,
            area.top + 18.0,
        );
        self.text(
            if model.collections_descending {
                self.strings.sort_descending
            } else {
                self.strings.sort_ascending
            },
            sort_area,
            &self.small_right.clone(),
            if self.is_hovered(&sort) {
                self.palette.primary
            } else {
                self.palette.muted
            },
        )?;
        self.hits.push(HitRegion {
            rect: sort_area,
            action: sort,
        });

        let dragging = model.clip_drag_preview.as_ref();
        let rows = rect(area.left, area.top + 26.0, area.right, area.bottom);
        let overflow = folder_column_overflow_in(rows, model.collections.len());
        let scroll = model.folder_scroll.clamp(0.0, overflow);
        self.push_clip(rows)?;
        let painted = self.render_folder_rows(model, rows, scroll, dragging);
        self.pop_clip();
        painted?;
        if overflow > 0.0 {
            let height = rows.bottom - rows.top;
            let visible = (height / (height + overflow)).clamp(0.1, 1.0);
            let thumb = height * visible;
            let top = rows.top + (height - thumb) * (scroll / overflow);
            self.fill(
                rect(rows.right - 3.0, top, rows.right - 1.0, top + thumb),
                self.palette.border,
                1.5,
            )?;
        }
        if model.collections.is_empty() {
            self.text(
                self.strings.no_collections,
                rect(
                    rows.left + 10.0,
                    rows.top + FOLDER_ROW_HEIGHT + 8.0,
                    rows.right,
                    rows.top + FOLDER_ROW_HEIGHT + 32.0,
                ),
                &self.small.clone(),
                self.palette.muted,
            )?;
        }
        Ok(())
    }

    fn render_folder_rows(
        &mut self,
        model: &UiModel,
        rows: LogicalRect,
        scroll: f32,
        dragging: Option<&crate::model::ClipDragPreview>,
    ) -> Result<(), String> {
        let area = rows;
        let visible = model.visible_collection_indices();
        // the active row's pill glides between folders like the sidebar's
        let active_slot = model.active_collection.as_ref().map_or(Some(0), |path| {
            visible
                .iter()
                .position(|index| &model.collections[*index].path == path)
                .map(|position| position + 1)
        });
        if let Some(slot) = active_slot {
            let target = slot as f32 * (FOLDER_ROW_HEIGHT + 2.0);
            let now = Instant::now();
            let reduced = self.reduced_motion;
            let motion = self
                .toggle_motions
                .entry("collection_row")
                .or_insert_with(|| {
                    crate::motion::Motion::with_curve(target, crate::motion::Curve::Glide)
                });
            motion.retarget(target, now, reduced);
            let offset = motion.value(now);
            self.navigation_was_moving |= motion.active(now);
            let top = area.top - scroll + offset;
            self.fill(
                rect(area.left, top, area.right, top + FOLDER_ROW_HEIGHT),
                self.palette.surface_raised,
                RADIUS_SMALL,
            )?;
        }
        let mut row_top = area.top - scroll;
        self.folder_row(
            rect(area.left, row_top, area.right, row_top + FOLDER_ROW_HEIGHT),
            rows,
            Glyph::Library,
            self.strings.all_clips,
            model.clips.len(),
            model.active_collection.is_none(),
            false,
            Action::SelectCollection(None),
        )?;
        row_top += FOLDER_ROW_HEIGHT + 2.0;

        for index in visible {
            if row_top > area.bottom {
                break;
            }
            if row_top + FOLDER_ROW_HEIGHT < area.top {
                row_top += FOLDER_ROW_HEIGHT + 2.0;
                continue;
            }
            let collection = &model.collections[index];
            self.folder_row(
                rect(area.left, row_top, area.right, row_top + FOLDER_ROW_HEIGHT),
                rows,
                Glyph::Folder,
                &collection.name,
                collection.clip_count,
                model.active_collection.as_ref() == Some(&collection.path),
                dragging.is_some_and(|drag| drag.target_collection == Some(index)),
                Action::SelectCollection(Some(index)),
            )?;
            row_top += FOLDER_ROW_HEIGHT + 2.0;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn folder_row(
        &mut self,
        area: LogicalRect,
        viewport: LogicalRect,
        glyph: Glyph,
        name: &str,
        count: usize,
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
        let count_area = rect(area.right - 48.0, area.top, area.right - 10.0, area.bottom);
        self.text(
            &self.shorten(name, &self.body, count_area.left - (area.left + 34.0) - 8.0),
            rect(
                area.left + 34.0,
                area.top,
                count_area.left - 8.0,
                area.bottom,
            ),
            &self.body.clone(),
            if active {
                self.palette.primary
            } else {
                mix(self.palette.muted, self.palette.primary, 0.35)
            },
        )?;
        self.text(
            &count.to_string(),
            count_area,
            &self.small_right.clone(),
            self.palette.muted,
        )?;
        self.push_clipped_hit(area, viewport, action);
        Ok(())
    }

    /// Leech's quick button: a small wash pill for secondary row actions.
    fn quick_button(
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

    #[allow(clippy::too_many_arguments)]
    fn render_settings(
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
            rect(left + 10.0, top, rail_right, top + 30.0),
            &self.heading.clone(),
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
                    rewa_windows::hotkey::localized_hotkey_label(&model.config.hotkey)
                };
                let key = self.settings_line(
                    line(1),
                    1,
                    self.strings.replay_hotkey,
                    self.strings.replay_hotkey_hint,
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

    fn settings_card(&self, first: LogicalRect, lines: usize) -> Result<(), String> {
        self.stroke(
            rect(
                first.left,
                first.top,
                first.right,
                first.top + lines as f32 * SETTINGS_LINE_HEIGHT,
            ),
            self.palette.border,
            11.0,
            1.0,
        )
    }

    /// One Leech settings line: a name, a quiet detail and the control on the
    /// right; returns where the control landed.
    fn settings_line(
        &mut self,
        line: LogicalRect,
        index: usize,
        title: &'static str,
        detail: &str,
        control: LineControl<'_>,
    ) -> Result<LogicalRect, String> {
        if index > 0 {
            self.fill(
                rect(line.left + 14.0, line.top, line.right, line.top + 1.0),
                self.palette.hairline,
                0.0,
            )?;
        }
        let center = (line.top + line.bottom) / 2.0;
        let control_area = match &control {
            LineControl::Switch(_, _) => rect(
                line.right - 44.0,
                center - 9.0,
                line.right - 14.0,
                center + 9.0,
            ),
            LineControl::Popup(value, _) => {
                let width = (self.measure(value, &self.body) + 36.0).clamp(80.0, 230.0);
                rect(
                    line.right - 14.0 - width,
                    center - 12.0,
                    line.right - 14.0,
                    center + 12.0,
                )
            }
            LineControl::Pill(value, _, _) => {
                let width = (self.measure(value, &self.small) + 26.0).clamp(64.0, 260.0);
                rect(
                    line.right - 14.0 - width,
                    center - 12.0,
                    line.right - 14.0,
                    center + 12.0,
                )
            }
            LineControl::Slider(_, _) => {
                let track = settings_slider_track(line);
                rect(track.left - 54.0, center - 12.0, track.right, center + 12.0)
            }
        };
        let words_right = control_area.left - 20.0;
        self.text(
            title,
            rect(line.left + 14.0, center - 19.0, words_right, center + 1.0),
            &self.body.clone(),
            self.palette.primary,
        )?;
        self.text(
            &self.shorten(detail, &self.small, words_right - line.left - 14.0),
            rect(line.left + 14.0, center + 1.0, words_right, center + 19.0),
            &self.small.clone(),
            self.palette.muted,
        )?;
        match control {
            LineControl::Switch(on, action) => {
                self.switch(control_area, on, title)?;
                self.hits.push(HitRegion {
                    rect: rect(
                        control_area.left - 6.0,
                        line.top + 8.0,
                        control_area.right + 6.0,
                        line.bottom - 8.0,
                    ),
                    action,
                });
            }
            LineControl::Popup(value, action) => {
                let hovered = self.is_hovered(&action);
                self.fill(
                    control_area,
                    if hovered {
                        mix(
                            self.palette.surface_raised,
                            self.palette.primary,
                            0.05 * self.hover_amount(1.0),
                        )
                    } else {
                        self.palette.surface_raised
                    },
                    6.0,
                )?;
                self.stroke(
                    control_area,
                    mix(self.palette.surface_raised, self.palette.primary, 0.08),
                    6.0,
                    1.0,
                )?;
                let text_right = control_area.right - 24.0;
                self.text(
                    &self.shorten(value, &self.body, text_right - control_area.left - 10.0),
                    rect(
                        control_area.left + 10.0,
                        control_area.top,
                        text_right,
                        control_area.bottom,
                    ),
                    &self.body.clone(),
                    self.palette.primary,
                )?;
                self.glyph(
                    Glyph::UpDown,
                    rect(
                        control_area.right - 20.0,
                        center - 6.0,
                        control_area.right - 8.0,
                        center + 6.0,
                    ),
                    self.palette.muted,
                )?;
                self.hits.push(HitRegion {
                    rect: control_area,
                    action,
                });
            }
            LineControl::Pill(value, action, capturing) => {
                let hovered = self.is_hovered(&action);
                if hovered || capturing {
                    self.tint(control_area, if capturing { 0.1 } else { 0.06 }, 12.0)?;
                }
                self.stroke(
                    control_area,
                    if capturing {
                        self.palette.primary
                    } else {
                        self.palette.border
                    },
                    12.0,
                    1.0,
                )?;
                self.text(
                    &self.shorten(
                        value,
                        &self.small,
                        control_area.right - control_area.left - 20.0,
                    ),
                    control_area,
                    &self.small_center.clone(),
                    self.palette.primary,
                )?;
                self.hits.push(HitRegion {
                    rect: control_area,
                    action,
                });
            }
            LineControl::Slider(value, action) => {
                let track = settings_slider_track(line);
                let fraction = f32::from(value.min(200)) / 200.0;
                let knob_x = track.left + (track.right - track.left) * fraction;
                self.text(
                    &format!("{} %", value.min(200)),
                    rect(
                        control_area.left,
                        control_area.top,
                        track.left - 12.0,
                        control_area.bottom,
                    ),
                    &self.small_right.clone(),
                    self.palette.muted,
                )?;
                self.tint(track, 0.12, 2.0)?;
                self.fill(
                    rect(
                        track.left,
                        track.top,
                        knob_x.max(track.left + 4.0),
                        track.bottom,
                    ),
                    self.palette.primary,
                    2.0,
                )?;
                let knob = if self.is_hovered(&action) { 16.0 } else { 14.0 };
                let knob_rect = rect(
                    knob_x - knob / 2.0,
                    center - knob / 2.0,
                    knob_x + knob / 2.0,
                    center + knob / 2.0,
                );
                self.fill_alpha(
                    rect(
                        knob_rect.left,
                        knob_rect.top + 1.0,
                        knob_rect.right,
                        knob_rect.bottom + 1.0,
                    ),
                    0x000000,
                    0.25,
                    knob / 2.0,
                )?;
                self.fill(knob_rect, 0xffffff, knob / 2.0)?;
                self.hits.push(HitRegion {
                    rect: rect(
                        track.left - 8.0,
                        line.top + 12.0,
                        track.right + 8.0,
                        line.bottom - 12.0,
                    ),
                    action,
                });
            }
        }
        Ok(control_area)
    }

    /// Leech's switch: 30 × 18, the knob settles across on the settle spring.
    fn switch(
        &mut self,
        area: LogicalRect,
        enabled: bool,
        key: &'static str,
    ) -> Result<(), String> {
        let now = Instant::now();
        let value = if enabled { 1.0 } else { 0.0 };
        let reduced = self.reduced_motion;
        let motion = self
            .toggle_motions
            .entry(key)
            .or_insert_with(|| crate::motion::Motion::new(value));
        motion.retarget(value, now, reduced);
        let travel = motion.value(now);
        self.navigation_was_moving |= motion.active(now);
        let off = mix(self.palette.surface, self.palette.primary, 0.22);
        self.fill(
            area,
            mix(off, self.palette.primary, travel.clamp(0.0, 1.0)),
            9.0,
        )?;
        let left = area.left + 2.0 + 12.0 * travel;
        let knob = rect(left, area.top + 2.0, left + 14.0, area.bottom - 2.0);
        self.fill_alpha(
            rect(knob.left, knob.top + 1.0, knob.right, knob.bottom + 1.0),
            0x000000,
            0.22,
            7.0,
        )?;
        self.fill(knob, self.palette.surface, 7.0)
    }

    fn render_player(
        &mut self,
        model: &UiModel,
        left: f32,
        right: f32,
        height: f32,
    ) -> Result<(), String> {
        let title = model
            .active_clip()
            .map_or(self.strings.preview_title, |clip| clip.title.as_str());
        let edit_width = self.measure(self.strings.edit_clip, &self.button) + 28.0;
        let edit = rect(
            right - edit_width,
            content_top() + 2.0,
            right,
            content_top() + 30.0,
        );
        self.page_toolbar(title, left, edit.left - 16.0, Action::Back)?;
        if model.active_clip().is_some() {
            self.action_button(edit, self.strings.edit_clip, Action::EditActiveClip)?;
        }
        let Some(clip) = model.active_clip() else {
            self.empty_state(self.strings.clip_unavailable, left, right, PLAYER_TOP)?;
            return Ok(());
        };
        let detail_width = if right - left >= 960.0 { 276.0 } else { 0.0 };
        let main_right = right
            - if detail_width > 0.0 {
                detail_width + 24.0
            } else {
                0.0
            };
        let stage = fit_aspect(
            rect(left, PLAYER_TOP, main_right, (height - 178.0).max(390.0)),
            model.player_aspect_ratio,
        );
        self.fill(stage, 0x000000, RADIUS_LARGE)?;
        self.hits.push(HitRegion {
            rect: stage,
            action: Action::PlayPause,
        });
        self.render_media_controls(model, stage, false)?;

        if detail_width > 0.0 {
            let detail_left = right - detail_width;
            let info = rect(detail_left, stage.top, right, (height - 24.0).min(620.0));
            self.clip_information_panel(model, clip, info, true, model.active_clip)?;
        }
        Ok(())
    }

    /// Back chevron and page title in one row, the macOS navigation bar.
    fn page_toolbar(
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

    fn render_media_controls(
        &mut self,
        model: &UiModel,
        stage: LogicalRect,
        editor: bool,
    ) -> Result<(), String> {
        let controls = rect(stage.left, stage.bottom, stage.right, stage.bottom + 76.0);
        self.floating_glyph(
            rect(
                controls.left + 8.0,
                controls.top + 18.0,
                controls.left + 48.0,
                controls.bottom - 18.0,
            ),
            if model.player_playing {
                Glyph::Pause
            } else {
                Glyph::Play
            },
            self.palette.primary,
            Some(Action::PlayPause),
        )?;
        if !editor {
            self.floating_glyph(
                rect(
                    controls.left + 58.0,
                    controls.top + 18.0,
                    controls.left + 98.0,
                    controls.bottom - 18.0,
                ),
                Glyph::ChevronLeft,
                self.palette.muted,
                Some(Action::PreviousClip),
            )?;
            self.floating_glyph(
                rect(
                    controls.left + 102.0,
                    controls.top + 18.0,
                    controls.left + 142.0,
                    controls.bottom - 18.0,
                ),
                Glyph::ChevronRight,
                self.palette.muted,
                Some(Action::NextClip),
            )?;
        }
        self.text(
            &format!(
                "{} / {}",
                format_player_time(model.player_position_seconds),
                format_player_time(model.player_duration_seconds)
            ),
            rect(
                controls.left + 154.0,
                controls.top,
                controls.left + 252.0,
                controls.bottom,
            ),
            &self.small.clone(),
            self.palette.muted,
        )?;
        let rail = rect(
            controls.left + 260.0,
            controls.top + 36.0,
            controls.right - 64.0,
            controls.top + 40.0,
        );
        self.draw_progress_rail(model, rail)?;
        self.hits.push(HitRegion {
            rect: rect(
                rail.left,
                controls.top + 12.0,
                rail.right,
                controls.bottom - 12.0,
            ),
            action: if editor {
                Action::DragEditorPlayhead
            } else {
                Action::DragPlayerSeek
            },
        });
        if editor {
            return Ok(());
        }
        self.floating_glyph(
            rect(
                controls.right - 48.0,
                controls.top + 18.0,
                controls.right - 8.0,
                controls.bottom - 18.0,
            ),
            Glyph::Fullscreen,
            self.palette.muted,
            Some(Action::ToggleFullscreen),
        )
    }

    fn draw_progress_rail(&self, model: &UiModel, rail: LogicalRect) -> Result<(), String> {
        let radius = (rail.bottom - rail.top) / 2.0;
        self.tint(rail, 0.14, radius)?;
        let progress = if model.player_duration_seconds > 0.0 {
            (model.player_position_seconds / model.player_duration_seconds).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let x = rail.left + (rail.right - rail.left) * progress;
        self.fill(
            rect(
                rail.left,
                rail.top,
                x.max(rail.left + radius * 2.0),
                rail.bottom,
            ),
            self.palette.primary,
            radius,
        )?;
        let center = (rail.top + rail.bottom) / 2.0;
        self.fill_alpha(
            rect(x - 6.0, center - 5.0, x + 6.0, center + 7.0),
            0x000000,
            0.25,
            6.0,
        )?;
        self.fill(
            rect(x - 6.0, center - 6.0, x + 6.0, center + 6.0),
            0xffffff,
            6.0,
        )
    }

    fn clip_information_panel(
        &mut self,
        model: &UiModel,
        clip: &rewa_core::clips::Clip,
        area: LogicalRect,
        allow_rename: bool,
        delete_for: Option<usize>,
    ) -> Result<(), String> {
        self.text(
            self.strings.clip_information,
            rect(area.left + 2.0, area.top - 2.0, area.right, area.top + 18.0),
            &self.caption.clone(),
            self.palette.muted,
        )?;
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
        let reserved_actions = if delete_for.is_some() { 52.0 } else { 0.0 };
        let row_height = ((area.bottom - area.top - 26.0 - reserved_actions) / rows.len() as f32)
            .clamp(40.0, 50.0);
        let card = rect(
            area.left,
            area.top + 26.0,
            area.right,
            area.top + 26.0 + rows.len() as f32 * row_height,
        );
        self.stroke(card, self.palette.border, 11.0, 1.0)?;
        for (index, (label, value)) in rows.into_iter().enumerate() {
            let top = card.top + index as f32 * row_height;
            if index > 0 {
                self.fill(
                    rect(card.left + 14.0, top, card.right, top + 1.0),
                    self.palette.hairline,
                    0.0,
                )?;
            }
            let has_title_action = index == 0 && allow_rename;
            let center = top + row_height / 2.0;
            self.text(
                label,
                rect(
                    card.left + 14.0,
                    center - 17.0,
                    card.right - 14.0,
                    center - 1.0,
                ),
                &self.small.clone(),
                self.palette.muted,
            )?;
            let value_area = rect(
                card.left + 14.0,
                center - 1.0,
                if has_title_action {
                    card.right - 44.0
                } else {
                    card.right - 14.0
                },
                center + 17.0,
            );
            self.text(
                &self.shorten(&value, &self.body, value_area.right - value_area.left),
                value_area,
                &self.body.clone(),
                self.palette.primary,
            )?;
            if has_title_action {
                self.door(
                    rect(
                        card.right - 38.0,
                        center - 13.0,
                        card.right - 12.0,
                        center + 13.0,
                    ),
                    Glyph::Pencil,
                    false,
                    Action::RenameActiveClip,
                )?;
            }
        }
        if let Some(index) = delete_for {
            let delete = rect(
                area.left,
                card.bottom + 14.0,
                area.right,
                card.bottom + 44.0,
            );
            self.quick_button(
                delete,
                self.strings.delete_clip,
                Action::DeleteClip(index),
                true,
            )?;
        }
        Ok(())
    }

    fn render_editor(
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
        let main_right = right
            - if detail_width > 0.0 {
                detail_width + 24.0
            } else {
                0.0
            };
        let stage = fit_aspect(
            rect(
                left,
                PLAYER_TOP,
                main_right,
                (height - EDITOR_BOTTOM_RESERVE).max(360.0),
            ),
            model.player_aspect_ratio,
        );
        self.fill(stage, 0x000000, RADIUS_LARGE)?;
        self.hits.push(HitRegion {
            rect: stage,
            action: Action::PlayPause,
        });
        self.render_media_controls(model, stage, true)?;

        if detail_width > 0.0 {
            let detail_left = right - detail_width;
            let info = rect(
                detail_left,
                stage.top,
                right,
                (stage.top + 276.0).min(height - 420.0),
            );
            self.clip_information_panel(model, clip, info, true, None)?;
            let duration = rect(detail_left, info.bottom + 24.0, right, info.bottom + 90.0);
            self.text(
                self.strings.trimmed_duration,
                rect(
                    duration.left + 2.0,
                    duration.top - 2.0,
                    duration.right,
                    duration.top + 18.0,
                ),
                &self.caption.clone(),
                self.palette.muted,
            )?;
            let range = rect(
                duration.left,
                duration.top + 24.0,
                duration.right,
                duration.bottom,
            );
            self.stroke(range, self.palette.border, 11.0, 1.0)?;
            self.text(
                &format!(
                    "{} – {}",
                    format_editor_time(model.editor_start),
                    format_editor_time(model.editor_end)
                ),
                rect(
                    range.left + 14.0,
                    range.top,
                    range.right - 90.0,
                    range.bottom,
                ),
                &self.small.clone(),
                self.palette.muted,
            )?;
            self.text(
                &format_editor_time(model.editor_selected_duration()),
                rect(
                    range.right - 100.0,
                    range.top,
                    range.right - 14.0,
                    range.bottom,
                ),
                &self.body_trailing.clone(),
                self.palette.primary,
            )?;
            let mode = rect(detail_left, range.bottom + 24.0, right, range.bottom + 44.0);
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
        self.stroke(timeline, self.palette.border, 11.0, 1.0)?;
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

    fn timeline_labels(&self, model: &UiModel, area: LogicalRect) -> Result<(), String> {
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

    fn trim_storyboard(
        &mut self,
        model: &UiModel,
        clip: &rewa_core::clips::Clip,
        area: LogicalRect,
    ) -> Result<(), String> {
        self.fill(area, self.palette.stage, RADIUS_SMALL)?;
        let segment_width = (area.right - area.left) / 8.0;
        for segment in 0..8 {
            let preview = rect(
                area.left + segment as f32 * segment_width,
                area.top,
                area.left + (segment + 1) as f32 * segment_width,
                area.bottom,
            );
            let _ = self.draw_thumbnail(&clip.path, preview, 2.0)?;
        }
        let duration = model
            .editor_timing
            .as_ref()
            .map_or(0.0, |timing| timing.duration.as_secs_f64());
        if duration <= 0.0 {
            return Ok(());
        }
        let start_x = area.left
            + (area.right - area.left) * (model.editor_start.as_secs_f64() / duration) as f32;
        let end_x = area.left
            + (area.right - area.left) * (model.editor_end.as_secs_f64() / duration) as f32;
        let playhead_x = area.left
            + (area.right - area.left)
                * (model.player_position_seconds / duration).clamp(0.0, 1.0) as f32;
        self.fill_alpha(
            rect(area.left, area.top, start_x, area.bottom),
            0x000000,
            0.55,
            0.0,
        )?;
        self.fill_alpha(
            rect(end_x, area.top, area.right, area.bottom),
            0x000000,
            0.55,
            0.0,
        )?;
        self.stroke(
            rect(start_x, area.top - 1.0, end_x, area.bottom + 1.0),
            self.palette.primary,
            3.0,
            2.0,
        )?;
        self.fill(
            rect(
                start_x - 8.0,
                area.top - 2.0,
                start_x + 8.0,
                area.bottom + 2.0,
            ),
            self.palette.primary,
            5.0,
        )?;
        self.fill(
            rect(end_x - 8.0, area.top - 2.0, end_x + 8.0, area.bottom + 2.0),
            self.palette.primary,
            5.0,
        )?;
        self.fill(
            rect(
                playhead_x - 1.0,
                area.top - 56.0,
                playhead_x + 1.0,
                area.bottom + 24.0,
            ),
            self.palette.primary,
            0.0,
        )?;
        self.fill(
            rect(
                playhead_x - 5.0,
                area.top - 60.0,
                playhead_x + 5.0,
                area.top - 50.0,
            ),
            self.palette.primary,
            5.0,
        )?;
        self.hits.push(HitRegion {
            rect: rect(area.left, area.top - 20.0, area.right, area.bottom + 20.0),
            action: Action::DragEditorPlayhead,
        });
        self.hits.push(HitRegion {
            rect: rect(
                start_x - 14.0,
                area.top - 8.0,
                start_x + 14.0,
                area.bottom + 8.0,
            ),
            action: Action::DragEditorStart,
        });
        self.hits.push(HitRegion {
            rect: rect(
                end_x - 14.0,
                area.top - 8.0,
                end_x + 14.0,
                area.bottom + 8.0,
            ),
            action: Action::DragEditorEnd,
        });
        Ok(())
    }

    /// The app icon from the executable's own resources, scaled on whole pixels
    /// because it is pixel art.
    fn draw_app_icon(&mut self, area: LogicalRect) -> Result<(), String> {
        if self.app_icon.is_none() {
            self.app_icon = Some(self.load_app_icon()?);
        }
        let (Some(target), Some(bitmap)) = (self.target.as_ref(), self.app_icon.as_ref()) else {
            return Ok(());
        };
        unsafe {
            target.DrawBitmap(
                bitmap,
                Some(&area.d2d()),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
            );
        }
        Ok(())
    }

    fn load_app_icon(&self) -> Result<ID2D1Bitmap, String> {
        use windows::Win32::Foundation::HINSTANCE;
        use windows::Win32::Graphics::Imaging::{
            GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, WICBitmapPaletteTypeMedianCut,
        };
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::{
            DestroyIcon, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW,
        };

        let module =
            unsafe { GetModuleHandleW(PCWSTR::null()) }.map_err(|error| error.to_string())?;
        let handle = unsafe {
            LoadImageW(
                Some(HINSTANCE(module.0)),
                PCWSTR(crate::icon::REWA_ICON_RESOURCE_ID as *const u16),
                IMAGE_ICON,
                256,
                256,
                LR_DEFAULTCOLOR,
            )
        }
        .map_err(|error| error.to_string())?;
        let icon = HICON(handle.0);
        let wic = unsafe { self.wic_factory.CreateBitmapFromHICON(icon) };
        let _ = unsafe { DestroyIcon(icon) };
        let wic = wic.map_err(|error| error.to_string())?;
        let converter = unsafe { self.wic_factory.CreateFormatConverter() }
            .map_err(|error| error.to_string())?;
        unsafe {
            converter.Initialize(
                &wic,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeMedianCut,
            )
        }
        .map_err(|error| error.to_string())?;
        let target = self.target.as_ref().expect("render target exists");
        unsafe { target.CreateBitmapFromWicBitmap(&converter, None) }
            .map_err(|error| error.to_string())
    }

    fn search_field(
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

    fn selection_toolbar(
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

    fn render_collection_picker(
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

    fn render_text_input(
        &mut self,
        input: &TextInput,
        field: LogicalRect,
        placeholder: &str,
        focused: bool,
        target: TextInputTarget,
    ) -> Result<(), String> {
        let body = self.body.clone();
        if input.value.is_empty() {
            self.text(placeholder, field, &body, self.palette.secondary)?;
        } else {
            if focused {
                let (start, end) = input.selection();
                if end > start {
                    let before: String = input.value.chars().take(start).collect();
                    let selected: String =
                        input.value.chars().skip(start).take(end - start).collect();
                    let offset = self.measure(&before, &body);
                    let width = self.measure(&selected, &body);
                    self.fill(
                        rect(
                            field.left + offset,
                            field.top + 8.0,
                            (field.left + offset + width).min(field.right),
                            field.bottom - 8.0,
                        ),
                        self.palette.selection,
                        3.0,
                    )?;
                }
            }
            self.text(&input.value, field, &body, self.palette.primary)?;
        }

        if focused {
            let prefixes = (0..=input.characters())
                .map(|count| {
                    let prefix: String = input.value.chars().take(count).collect();
                    self.measure(&prefix, &body)
                })
                .collect::<Vec<_>>();
            let caret_x = (field.left + prefixes[input.caret]).min(field.right);
            self.fill(
                rect(caret_x, field.top + 8.0, caret_x + 1.5, field.bottom - 8.0),
                self.palette.primary,
                0.0,
            )?;
            for index in 0..=input.characters() {
                let left = if index == 0 {
                    field.left
                } else {
                    field.left + (prefixes[index - 1] + prefixes[index]) / 2.0
                };
                let right = if index == input.characters() {
                    field.right
                } else {
                    field.left + (prefixes[index] + prefixes[index + 1]) / 2.0
                };
                self.hits.push(HitRegion {
                    rect: rect(
                        left.min(field.right),
                        field.top,
                        right.min(field.right),
                        field.bottom,
                    ),
                    action: match target {
                        TextInputTarget::Search => Action::PlaceSearchCaret(index),
                        TextInputTarget::Prompt => Action::PlacePromptCaret(index),
                    },
                });
            }
        }
        Ok(())
    }

    fn render_settings_menu(
        &mut self,
        model: &UiModel,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let Some(menu_state) = &model.settings_menu else {
            return Ok(());
        };
        if menu_state.items.is_empty() {
            return Ok(());
        }

        self.hits.push(HitRegion {
            rect: rect(0.0, 0.0, width, height),
            action: Action::DismissSettingsMenu,
        });

        let target_action = match menu_state.kind {
            SettingsMenuKind::Theme => Action::ChooseTheme,
            SettingsMenuKind::Language => Action::ChooseLanguage,
            SettingsMenuKind::HoverStyle => Action::ChooseHoverStyle,
            SettingsMenuKind::HoverStrength => Action::ChooseHoverStrength,
            SettingsMenuKind::TimeFilter => Action::ChooseTimeFilter,
            SettingsMenuKind::CollectionFilter => Action::ChooseCollectionFilter,
            SettingsMenuKind::TypeFilter => Action::ChooseTypeFilter,
            SettingsMenuKind::SizeFilter => Action::ChooseSizeFilter,
            SettingsMenuKind::ClipSort => Action::ChooseClipSort,
            SettingsMenuKind::Display => Action::ChooseDisplay,
            SettingsMenuKind::FrameRate => Action::ChooseFrameRate,
            SettingsMenuKind::Duration => Action::ChooseDuration,
            SettingsMenuKind::Codec => Action::ChooseCodec,
            SettingsMenuKind::Quality => Action::ChooseQuality,
            SettingsMenuKind::AudioMode => Action::ChooseAudioMode,
            SettingsMenuKind::DesktopDevice => Action::ChooseDesktopDevice,
            SettingsMenuKind::DesktopGain => Action::ChooseDesktopGain,
            SettingsMenuKind::Microphone => Action::ChooseMicrophone,
            SettingsMenuKind::MicrophoneGain => Action::ChooseMicrophoneGain,
            SettingsMenuKind::StorageLimit => Action::ChooseStorageLimit,
        };
        let anchor = self
            .hits
            .iter()
            .rev()
            .find(|hit| hit.action == target_action)
            .map_or(rect(width - 380.0, 150.0, width - 40.0, 190.0), |hit| {
                hit.rect
            });
        let control_width = (anchor.right - anchor.left).max(190.0);
        let columns = if menu_state.kind == SettingsMenuKind::DesktopGain {
            3
        } else {
            1
        };
        let has_details = menu_state.items.iter().any(|item| item.detail.is_some());
        let item_height = if has_details { 48.0 } else { 30.0 };
        let rows = menu_state.items.len().div_ceil(columns);
        let menu_height = 12.0 + rows as f32 * item_height;
        let menu_width = control_width.max(if has_details { 310.0 } else { 190.0 });
        let menu_left = (anchor.right - menu_width).max(18.0);
        let below = anchor.bottom + 8.0;
        let above = anchor.top - menu_height - 8.0;
        let menu_top = if below + menu_height <= height - 18.0 {
            below
        } else {
            above.max(18.0)
        };
        let menu = rect(
            menu_left,
            menu_top,
            menu_left + menu_width,
            menu_top + menu_height,
        );

        self.popover_surface(menu)?;

        let cell_width = (menu_width - 12.0) / columns as f32;
        for (index, item) in menu_state.items.iter().enumerate() {
            let column = index % columns;
            let row = index / columns;
            let item_area = rect(
                menu.left + 6.0 + column as f32 * cell_width,
                menu.top + 6.0 + row as f32 * item_height,
                menu.left + 6.0 + (column + 1) as f32 * cell_width,
                menu.top + 6.0 + (row + 1) as f32 * item_height,
            );
            let action = Action::SelectSettingsOption(index);
            let selected = menu_state.selected == Some(index);
            let highlighted = menu_state.highlighted == index || self.is_hovered(&action);
            if highlighted {
                self.tint(item_area, 0.08, RADIUS_SMALL - 2.0)?;
            }
            if selected {
                if columns > 1 {
                    self.tint(item_area, 0.12, RADIUS_SMALL - 2.0)?;
                } else {
                    // a macOS menu marks the current choice with a check, not a bar
                    let center_y = if item.detail.is_some() {
                        item_area.top + 16.5
                    } else {
                        (item_area.top + item_area.bottom) / 2.0
                    };
                    self.glyph(
                        Glyph::Check,
                        rect(
                            item_area.left + 5.0,
                            center_y - 6.0,
                            item_area.left + 17.0,
                            center_y + 6.0,
                        ),
                        self.palette.primary,
                    )?;
                }
            }
            if columns > 1 {
                self.text(
                    &item.label,
                    item_area,
                    &self.body_center.clone(),
                    self.palette.primary,
                )?;
            } else if let Some(detail) = &item.detail {
                self.text(
                    &item.label,
                    rect(
                        item_area.left + 20.0,
                        item_area.top + 4.0,
                        item_area.right - 12.0,
                        item_area.top + 26.0,
                    ),
                    &self.body.clone(),
                    self.palette.primary,
                )?;
                self.text(
                    detail,
                    rect(
                        item_area.left + 20.0,
                        item_area.top + 24.0,
                        item_area.right - 12.0,
                        item_area.bottom - 3.0,
                    ),
                    &self.small.clone(),
                    self.palette.secondary,
                )?;
            } else {
                self.text(
                    &item.label,
                    rect(
                        item_area.left + 20.0,
                        item_area.top,
                        item_area.right - 12.0,
                        item_area.bottom,
                    ),
                    &self.body.clone(),
                    self.palette.primary,
                )?;
            }
            self.hits.push(HitRegion {
                rect: item_area,
                action,
            });
        }
        Ok(())
    }

    fn render_context_menu(
        &mut self,
        model: &UiModel,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let Some(context) = model.context_menu else {
            return Ok(());
        };
        let Some(clip) = model.clips.get(context.clip) else {
            return Ok(());
        };
        self.hits.push(HitRegion {
            rect: rect(0.0, 0.0, width, height),
            action: Action::DismissContextMenu,
        });

        let visible_collections = model.collections.len().min(6);
        let collection_rows = if visible_collections == 0 {
            0
        } else {
            visible_collections + 1
        };
        let menu_width = 252.0;
        let menu_height = 66.0 + (6 + collection_rows) as f32 * 44.0 + 18.0;
        let left = context.x.min(width - menu_width - 16.0).max(16.0);
        let top = context.y.min(height - menu_height - 16.0).max(16.0);
        let menu = rect(left, top, left + menu_width, top + menu_height);
        self.popover_surface(menu)?;
        self.text(
            self.strings.clip_actions,
            rect(left + 16.0, top + 12.0, menu.right - 16.0, top + 30.0),
            &self.small.clone(),
            self.palette.secondary,
        )?;
        self.text(
            &clip.title,
            rect(left + 16.0, top + 31.0, menu.right - 16.0, top + 58.0),
            &self.body.clone(),
            self.palette.primary,
        )?;

        let mut row_top = top + 66.0;
        for (label, action) in [
            (
                if model.is_favorite(context.clip) {
                    self.strings.favorite_remove
                } else {
                    self.strings.favorite_add
                },
                Action::ToggleFavorite(context.clip),
            ),
            (
                self.strings.open_in_explorer,
                Action::OpenClipExternally(context.clip),
            ),
            (self.strings.edit_clip, Action::EditClip(context.clip)),
            (self.strings.rename, Action::RenameClip(context.clip)),
            (self.strings.select_multiple, Action::ToggleSelectionMode),
        ] {
            self.context_menu_row(
                rect(left + 8.0, row_top, menu.right - 8.0, row_top + 40.0),
                label,
                action,
                false,
            )?;
            row_top += 44.0;
        }

        if visible_collections > 0 {
            self.text(
                self.strings.move_to_collection,
                rect(
                    left + 16.0,
                    row_top + 4.0,
                    menu.right - 16.0,
                    row_top + 28.0,
                ),
                &self.small.clone(),
                self.palette.secondary,
            )?;
            row_top += 44.0;
            for (collection, item) in model
                .collections
                .iter()
                .take(visible_collections)
                .enumerate()
            {
                self.context_menu_row(
                    rect(left + 8.0, row_top, menu.right - 8.0, row_top + 40.0),
                    &item.name,
                    Action::MoveClipToCollection {
                        clip: context.clip,
                        collection,
                    },
                    false,
                )?;
                row_top += 44.0;
            }
        }

        self.fill(
            rect(left + 16.0, row_top + 1.0, menu.right - 16.0, row_top + 2.0),
            self.palette.border,
            0.0,
        )?;
        row_top += 6.0;
        self.context_menu_row(
            rect(left + 8.0, row_top, menu.right - 8.0, row_top + 40.0),
            self.strings.delete_clip,
            Action::DeleteClip(context.clip),
            true,
        )
    }

    fn context_menu_row(
        &mut self,
        area: LogicalRect,
        label: &str,
        action: Action,
        dangerous: bool,
    ) -> Result<(), String> {
        if self.is_hovered(&action) {
            if dangerous {
                self.fill_alpha(area, self.palette.destructive, 0.16, RADIUS_SMALL - 2.0)?;
            } else {
                self.tint(area, 0.08, RADIUS_SMALL - 2.0)?;
            }
        }
        self.text(
            label,
            rect(area.left + 12.0, area.top, area.right - 12.0, area.bottom),
            &self.body.clone(),
            if dangerous {
                self.palette.destructive
            } else {
                self.palette.primary
            },
        )?;
        self.hits.push(HitRegion { rect: area, action });
        Ok(())
    }

    fn render_delete_modal(
        &mut self,
        model: &UiModel,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let Some(target) = &model.pending_delete else {
            return Ok(());
        };
        let overlay = rect(0.0, 0.0, width, height);
        self.fill_alpha(overlay, 0x000000, 0.38, 0.0)?;
        self.hits.push(HitRegion {
            rect: overlay,
            action: Action::CancelDelete,
        });
        let modal_width = 460.0_f32.min(width - 40.0);
        let modal_height = 224.0;
        let left = (width - modal_width) / 2.0;
        let top = (height - modal_height) / 2.0;
        let modal = rect(left, top, left + modal_width, top + modal_height);
        self.plate_surface(modal)?;
        let (title, detail, confirmation) = match target {
            DeleteTarget::Clip(index) => {
                let name = model
                    .clips
                    .get(*index)
                    .map_or(self.strings.this_clip, |clip| clip.title.as_str());
                (
                    self.strings.delete_clip_question,
                    self.strings.delete_clip_body(name),
                    self.strings.delete_clip,
                )
            }
            DeleteTarget::Collection(path) => {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(self.strings.this_collection);
                (
                    self.strings.delete_collection_question,
                    self.strings.delete_collection_body(name),
                    self.strings.delete,
                )
            }
        };
        self.text(
            title,
            rect(left + 24.0, top + 20.0, modal.right - 24.0, top + 50.0),
            &self.heading.clone(),
            self.palette.primary,
        )?;
        self.text(
            &detail,
            rect(left + 28.0, top + 66.0, modal.right - 28.0, top + 108.0),
            &self.body.clone(),
            self.palette.secondary,
        )?;
        self.pill(
            rect(
                modal.right - 250.0,
                modal.bottom - 50.0,
                modal.right - 142.0,
                modal.bottom - 20.0,
            ),
            self.palette.surface,
            self.strings.cancel,
            self.palette.primary,
            Some(Action::CancelDelete),
        )?;
        self.pill(
            rect(
                modal.right - 134.0,
                modal.bottom - 50.0,
                modal.right - 20.0,
                modal.bottom - 20.0,
            ),
            self.palette.destructive,
            confirmation,
            0xffffff,
            Some(Action::ConfirmDelete),
        )
    }

    fn render_prompt_modal(
        &mut self,
        model: &UiModel,
        width: f32,
        height: f32,
    ) -> Result<(), String> {
        let Some(prompt) = &model.prompt else {
            return Ok(());
        };
        let overlay = rect(0.0, 0.0, width, height);
        self.fill_alpha(overlay, 0x000000, 0.38, 0.0)?;
        self.hits.push(HitRegion {
            rect: overlay,
            action: Action::CancelPrompt,
        });
        let modal_width = 460.0_f32.min(width - 40.0);
        let modal_height = 232.0;
        let left = (width - modal_width) / 2.0;
        let top = (height - modal_height) / 2.0;
        let modal = rect(left, top, left + modal_width, top + modal_height);
        self.plate_surface(modal)?;
        self.hits.push(HitRegion {
            rect: modal,
            action: Action::DismissNotice,
        });
        self.text(
            prompt.title(self.strings),
            rect(left + 24.0, top + 20.0, modal.right - 24.0, top + 50.0),
            &self.heading.clone(),
            self.palette.primary,
        )?;
        self.text(
            prompt.label(self.strings),
            rect(left + 28.0, top + 64.0, modal.right - 28.0, top + 84.0),
            &self.small.clone(),
            self.palette.secondary,
        )?;
        let field = rect(left + 24.0, top + 92.0, modal.right - 24.0, top + 128.0);
        self.tint(field, 0.08, 9.0)?;
        self.stroke(
            field,
            mix(self.palette.surface, self.palette.primary, 0.3),
            9.0,
            1.0,
        )?;
        self.render_text_input(
            &prompt.input,
            rect(
                field.left + 14.0,
                field.top,
                field.right - 14.0,
                field.bottom,
            ),
            "",
            true,
            TextInputTarget::Prompt,
        )?;
        self.text(
            self.strings.prompt_hint,
            rect(left + 28.0, top + 142.0, modal.right - 28.0, top + 162.0),
            &self.small.clone(),
            self.palette.secondary,
        )?;
        self.pill(
            rect(
                modal.right - 250.0,
                modal.bottom - 50.0,
                modal.right - 142.0,
                modal.bottom - 20.0,
            ),
            self.palette.surface,
            self.strings.cancel,
            self.palette.primary,
            Some(Action::CancelPrompt),
        )?;
        self.pill(
            rect(
                modal.right - 134.0,
                modal.bottom - 50.0,
                modal.right - 20.0,
                modal.bottom - 20.0,
            ),
            self.palette.accent,
            prompt.confirm(self.strings),
            self.palette.accent_text,
            Some(Action::ConfirmPrompt),
        )
    }

    fn empty_state(
        &mut self,
        message: &str,
        left: f32,
        right: f32,
        top: f32,
    ) -> Result<(), String> {
        self.text(
            message,
            rect(left, top, right, top + 34.0),
            &self.section.clone(),
            self.palette.secondary,
        )
    }

    fn pill(
        &mut self,
        area: LogicalRect,
        background: u32,
        label: &str,
        foreground: u32,
        action: Option<Action>,
    ) -> Result<(), String> {
        let hovered = action
            .as_ref()
            .is_some_and(|candidate| self.is_hovered(candidate));
        let radius = ((area.bottom - area.top) / 2.0).min(RADIUS_SMALL + 2.0);
        let filled = background == self.palette.accent || background == self.palette.destructive;
        if filled {
            let lit = if background == self.palette.accent {
                self.palette.accent_hover
            } else {
                mix(background, 0xffffff, 0.15)
            };
            self.fill(
                area,
                if hovered {
                    mix(background, lit, self.hover_amount(1.0))
                } else {
                    background
                },
                radius,
            )?;
        } else {
            self.tint(
                area,
                if hovered {
                    0.08 + 0.05 * self.hover_amount(1.0)
                } else {
                    0.08
                },
                radius,
            )?;
        }
        self.text(label, area, &self.button.clone(), foreground)?;
        if let Some(action) = action {
            self.hits.push(HitRegion { rect: area, action });
        }
        Ok(())
    }

    fn floating_glyph(
        &mut self,
        area: LogicalRect,
        glyph: Glyph,
        foreground: u32,
        action: Option<Action>,
    ) -> Result<(), String> {
        let hovered = action
            .as_ref()
            .is_some_and(|candidate| self.is_hovered(candidate));
        let color = if hovered {
            mix(foreground, self.palette.primary, self.hover_amount(0.72))
        } else {
            foreground
        };
        let size = 20.0;
        let center_x = (area.left + area.right) / 2.0;
        let center_y = (area.top + area.bottom) / 2.0;
        self.glyph(
            glyph,
            rect(
                center_x - size / 2.0,
                center_y - size / 2.0,
                center_x + size / 2.0,
                center_y + size / 2.0,
            ),
            color,
        )?;
        if let Some(action) = action {
            self.hits.push(HitRegion { rect: area, action });
        }
        Ok(())
    }

    fn fill(&self, area: LogicalRect, fill: u32, radius: f32) -> Result<(), String> {
        let target = self.target.as_ref().expect("render target exists");
        let brush = unsafe { target.CreateSolidColorBrush(&color(fill), None) }
            .map_err(|error| error.to_string())?;
        unsafe {
            if radius > 0.0 {
                target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: area.d2d(),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &brush,
                );
            } else {
                target.FillRectangle(&area.d2d(), &brush);
            }
        }
        Ok(())
    }

    fn stroke(
        &self,
        area: LogicalRect,
        stroke_color: u32,
        radius: f32,
        width: f32,
    ) -> Result<(), String> {
        use windows::Win32::Graphics::Direct2D::ID2D1StrokeStyle;

        let target = self.target.as_ref().expect("render target exists");
        let brush = unsafe { target.CreateSolidColorBrush(&color(stroke_color), None) }
            .map_err(|error| error.to_string())?;
        unsafe {
            if radius > 0.0 {
                target.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: area.d2d(),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &brush,
                    width,
                    None::<&ID2D1StrokeStyle>,
                );
            } else {
                target.DrawRectangle(&area.d2d(), &brush, width, None::<&ID2D1StrokeStyle>);
            }
        }
        Ok(())
    }

    fn fill_alpha(
        &self,
        area: LogicalRect,
        fill: u32,
        alpha: f32,
        radius: f32,
    ) -> Result<(), String> {
        let target = self.target.as_ref().expect("render target exists");
        let mut fill = color(fill);
        fill.a = alpha.clamp(0.0, 1.0);
        let brush = unsafe { target.CreateSolidColorBrush(&fill, None) }
            .map_err(|error| error.to_string())?;
        unsafe {
            if radius > 0.0 {
                target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: area.d2d(),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &brush,
                );
            } else {
                target.FillRectangle(&area.d2d(), &brush);
            }
        }
        Ok(())
    }

    fn glyph(&self, glyph: Glyph, area: LogicalRect, fill: u32) -> Result<(), String> {
        let target = self.target.as_ref().expect("render target exists");
        let brush = unsafe { target.CreateSolidColorBrush(&color(fill), None) }
            .map_err(|error| error.to_string())?;
        // every icon is laid out on a centred square so a non-square area cannot
        // stretch it out of shape
        let span = (area.right - area.left).min(area.bottom - area.top);
        let center_x = (area.left + area.right) / 2.0;
        let center_y = (area.top + area.bottom) / 2.0;
        let scale = span / 24.0;
        let weight = (span * 0.078).clamp(1.3, 1.8);
        let unit = |x: f32, y: f32| Vector2 {
            X: center_x + (x - 12.0) * scale,
            Y: center_y + (y - 12.0) * scale,
        };
        let path = |points: &[(f32, f32)], closed: bool| -> Result<(), String> {
            let mapped = points.iter().map(|(x, y)| unit(*x, *y)).collect::<Vec<_>>();
            self.stroke_path(&mapped, closed, &brush, weight)
        };
        let solid = |points: &[(f32, f32)]| -> Result<(), String> {
            let mapped = points.iter().map(|(x, y)| unit(*x, *y)).collect::<Vec<_>>();
            self.fill_path(&mapped, &brush)
        };
        let circle = |x: f32, y: f32, radius: f32| -> Result<(), String> {
            unsafe {
                target.DrawEllipse(
                    &D2D1_ELLIPSE {
                        point: unit(x, y),
                        radiusX: radius * scale,
                        radiusY: radius * scale,
                    },
                    &brush,
                    weight,
                    Some(&self.round_stroke),
                );
            }
            Ok(())
        };
        let dot = |x: f32, y: f32, radius: f32| -> Result<(), String> {
            unsafe {
                target.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: unit(x, y),
                        radiusX: radius * scale,
                        radiusY: radius * scale,
                    },
                    &brush,
                );
            }
            Ok(())
        };
        let rounded =
            |left: f32, top: f32, right: f32, bottom: f32, radius: f32| -> Result<(), String> {
                let start = unit(left, top);
                let end = unit(right, bottom);
                unsafe {
                    target.DrawRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: start.X,
                                top: start.Y,
                                right: end.X,
                                bottom: end.Y,
                            },
                            radiusX: radius * scale,
                            radiusY: radius * scale,
                        },
                        &brush,
                        weight,
                        Some(&self.round_stroke),
                    );
                }
                Ok(())
            };
        let bar = |left: f32, top: f32, right: f32, bottom: f32| -> Result<(), String> {
            let start = unit(left, top);
            let end = unit(right, bottom);
            let radius = (end.X - start.X) / 2.0;
            unsafe {
                target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: start.X,
                            top: start.Y,
                            right: end.X,
                            bottom: end.Y,
                        },
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &brush,
                );
            }
            Ok(())
        };
        let arc = |from: (f32, f32), to: (f32, f32), radius: f32| -> Result<(), String> {
            self.stroke_arc(
                unit(from.0, from.1),
                unit(to.0, to.1),
                radius * scale,
                &brush,
                weight,
            )
        };

        match glyph {
            Glyph::Library => {
                rounded(3.4, 5.4, 20.6, 18.6, 3.4)?;
                path(&[(10.4, 9.4), (14.6, 12.0), (10.4, 14.6)], true)?;
            }
            Glyph::Collections => {
                rounded(3.4, 8.6, 20.6, 20.0, 2.8)?;
                path(&[(6.4, 5.8), (17.6, 5.8)], false)?;
                path(&[(8.8, 3.2), (15.2, 3.2)], false)?;
            }
            Glyph::Settings => {
                // gear.shape: eight squared teeth around a hub
                let teeth = (0..8)
                    .flat_map(|tooth| {
                        let base = tooth as f32 * std::f32::consts::FRAC_PI_4;
                        [(-0.27, 6.9), (-0.17, 8.9), (0.17, 8.9), (0.27, 6.9)].map(
                            |(offset, radius): (f32, f32)| {
                                (
                                    12.0 + radius * (base + offset).cos(),
                                    12.0 + radius * (base + offset).sin(),
                                )
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                path(&teeth, true)?;
                circle(12.0, 12.0, 2.9)?;
            }
            Glyph::Folder => {
                rounded(3.4, 7.6, 20.6, 19.0, 2.4)?;
                path(
                    &[(3.8, 9.0), (3.8, 6.2), (4.8, 5.2), (9.0, 5.2), (10.8, 7.6)],
                    false,
                )?;
            }
            Glyph::Search => {
                circle(10.4, 10.4, 6.4)?;
                path(&[(15.2, 15.2), (20.4, 20.4)], false)?;
            }
            Glyph::Grid => {
                rounded(4.0, 4.0, 10.9, 10.9, 1.8)?;
                rounded(13.1, 4.0, 20.0, 10.9, 1.8)?;
                rounded(4.0, 13.1, 10.9, 20.0, 1.8)?;
                rounded(13.1, 13.1, 20.0, 20.0, 1.8)?;
            }
            Glyph::List => {
                for y in [6.6, 12.0, 17.4] {
                    dot(5.0, y, 1.2)?;
                    path(&[(9.0, y), (19.6, y)], false)?;
                }
            }
            Glyph::More => {
                for x in [6.2, 12.0, 17.8] {
                    dot(x, 12.0, 1.35)?;
                }
            }
            Glyph::Clock => {
                circle(12.0, 12.0, 8.2)?;
                path(&[(12.0, 7.4), (12.0, 12.0), (15.6, 14.1)], false)?;
            }
            Glyph::Monitor => {
                rounded(3.2, 4.6, 20.8, 16.4, 2.6)?;
                path(&[(12.0, 16.4), (12.0, 19.6)], false)?;
                path(&[(8.6, 19.8), (15.4, 19.8)], false)?;
            }
            Glyph::Audio => {
                path(
                    &[
                        (3.6, 9.6),
                        (7.4, 9.6),
                        (11.8, 5.4),
                        (11.8, 18.6),
                        (7.4, 14.4),
                        (3.6, 14.4),
                    ],
                    true,
                )?;
                arc((15.0, 9.6), (15.0, 14.4), 2.6)?;
                arc((17.6, 7.2), (17.6, 16.8), 5.2)?;
            }
            Glyph::Quality => {
                for (x, top) in [(5.6, 16.4), (10.0, 12.8), (14.4, 9.2), (18.8, 5.6)] {
                    path(&[(x, 18.6), (x, top)], false)?;
                }
            }
            Glyph::Filter => {
                path(&[(4.4, 7.0), (19.6, 7.0)], false)?;
                path(&[(7.2, 12.0), (16.8, 12.0)], false)?;
                path(&[(10.0, 17.0), (14.0, 17.0)], false)?;
            }
            Glyph::Microphone => {
                bar(10.1, 3.4, 13.9, 12.6)?;
                arc((7.2, 10.6), (16.8, 10.6), 4.8)?;
                path(&[(12.0, 16.6), (12.0, 20.4)], false)?;
            }
            Glyph::Star | Glyph::StarFilled => {
                let corners = (0..10)
                    .map(|step| {
                        let radius = if step % 2 == 0 { 8.8 } else { 4.0 };
                        let angle =
                            -std::f32::consts::FRAC_PI_2 + step as f32 * std::f32::consts::PI / 5.0;
                        (12.0 + radius * angle.cos(), 12.4 + radius * angle.sin())
                    })
                    .collect::<Vec<_>>();
                if matches!(glyph, Glyph::StarFilled) {
                    solid(&corners)?;
                } else {
                    path(&corners, true)?;
                }
            }
            Glyph::External => {
                path(
                    &[
                        (10.4, 5.4),
                        (4.6, 5.4),
                        (4.6, 19.4),
                        (18.6, 19.4),
                        (18.6, 13.6),
                    ],
                    false,
                )?;
                path(&[(13.4, 4.6), (19.4, 4.6), (19.4, 10.6)], false)?;
                path(&[(19.4, 4.6), (12.2, 11.8)], false)?;
            }
            Glyph::Play => {
                path(&[(8.6, 5.4), (18.4, 12.0), (8.6, 18.6)], true)?;
            }
            Glyph::Pause => {
                bar(8.2, 5.4, 10.8, 18.6)?;
                bar(13.2, 5.4, 15.8, 18.6)?;
            }
            Glyph::ChevronLeft => {
                path(&[(14.6, 5.6), (8.6, 12.0), (14.6, 18.4)], false)?;
            }
            Glyph::ChevronRight => {
                path(&[(9.4, 5.6), (15.4, 12.0), (9.4, 18.4)], false)?;
            }
            Glyph::ChevronDown => {
                path(&[(5.6, 9.4), (12.0, 15.4), (18.4, 9.4)], false)?;
            }
            Glyph::Close => {
                path(&[(7.8, 7.8), (16.2, 16.2)], false)?;
                path(&[(16.2, 7.8), (7.8, 16.2)], false)?;
            }
            Glyph::Sidebar => {
                rounded(3.4, 5.0, 20.6, 19.0, 2.8)?;
                path(&[(9.4, 5.2), (9.4, 18.8)], false)?;
            }
            Glyph::Check => {
                path(&[(6.4, 12.6), (10.2, 16.2), (17.6, 8.2)], false)?;
            }
            Glyph::Record => {
                circle(12.0, 12.0, 8.2)?;
                dot(12.0, 12.0, 4.4)?;
            }
            Glyph::Info => {
                circle(12.0, 12.0, 8.6)?;
                path(&[(12.0, 11.0), (12.0, 16.2)], false)?;
                dot(12.0, 7.9, 1.2)?;
            }
            Glyph::Undo => {
                path(&[(8.6, 6.0), (4.8, 9.8), (8.6, 13.6)], false)?;
                path(&[(4.8, 9.8), (14.4, 9.8)], false)?;
                arc((14.4, 9.8), (14.4, 18.6), 4.4)?;
                path(&[(14.4, 18.6), (9.0, 18.6)], false)?;
            }
            Glyph::Redo => {
                path(&[(15.4, 6.0), (19.2, 9.8), (15.4, 13.6)], false)?;
                path(&[(19.2, 9.8), (9.6, 9.8)], false)?;
                arc((9.6, 18.6), (9.6, 9.8), 4.4)?;
                path(&[(9.6, 18.6), (15.0, 18.6)], false)?;
            }
            Glyph::Muted => {
                path(
                    &[
                        (3.6, 9.6),
                        (7.4, 9.6),
                        (11.8, 5.4),
                        (11.8, 18.6),
                        (7.4, 14.4),
                        (3.6, 14.4),
                    ],
                    true,
                )?;
                path(&[(15.4, 9.4), (20.2, 14.6)], false)?;
                path(&[(20.2, 9.4), (15.4, 14.6)], false)?;
            }
            Glyph::UpDown => {
                path(&[(7.6, 9.4), (12.0, 5.0), (16.4, 9.4)], false)?;
                path(&[(7.6, 14.6), (12.0, 19.0), (16.4, 14.6)], false)?;
            }
            Glyph::Pencil => {
                path(
                    &[
                        (5.2, 18.8),
                        (6.8, 14.2),
                        (16.4, 4.6),
                        (19.4, 7.6),
                        (9.8, 17.2),
                    ],
                    true,
                )?;
                path(&[(6.8, 14.2), (9.8, 17.2)], false)?;
            }
            Glyph::Fullscreen => {
                path(&[(4.4, 9.8), (4.4, 4.4), (9.8, 4.4)], false)?;
                path(&[(14.2, 4.4), (19.6, 4.4), (19.6, 9.8)], false)?;
                path(&[(19.6, 14.2), (19.6, 19.6), (14.2, 19.6)], false)?;
                path(&[(9.8, 19.6), (4.4, 19.6), (4.4, 14.2)], false)?;
            }
        }
        Ok(())
    }

    fn stroke_path(
        &self,
        points: &[Vector2],
        closed: bool,
        brush: &ID2D1SolidColorBrush,
        weight: f32,
    ) -> Result<(), String> {
        if points.len() < 2 {
            return Ok(());
        }
        let geometry = self.build_path(points, closed, D2D1_FIGURE_BEGIN_HOLLOW)?;
        let target = self.target.as_ref().expect("render target exists");
        unsafe { target.DrawGeometry(&geometry, brush, weight, Some(&self.round_stroke)) };
        Ok(())
    }

    fn fill_path(&self, points: &[Vector2], brush: &ID2D1SolidColorBrush) -> Result<(), String> {
        if points.len() < 3 {
            return Ok(());
        }
        let geometry = self.build_path(points, true, D2D1_FIGURE_BEGIN_FILLED)?;
        let target = self.target.as_ref().expect("render target exists");
        unsafe { target.FillGeometry(&geometry, brush, None) };
        Ok(())
    }

    fn build_path(
        &self,
        points: &[Vector2],
        closed: bool,
        begin: D2D1_FIGURE_BEGIN,
    ) -> Result<ID2D1PathGeometry, String> {
        let geometry =
            unsafe { self.d2d_factory.CreatePathGeometry() }.map_err(|error| error.to_string())?;
        let sink = unsafe { geometry.Open() }.map_err(|error| error.to_string())?;
        unsafe {
            sink.BeginFigure(points[0], begin);
            sink.AddLines(&points[1..]);
            sink.EndFigure(if closed {
                D2D1_FIGURE_END_CLOSED
            } else {
                D2D1_FIGURE_END_OPEN
            });
        }
        unsafe { sink.Close() }.map_err(|error| error.to_string())?;
        Ok(geometry)
    }

    fn stroke_arc(
        &self,
        from: Vector2,
        to: Vector2,
        radius: f32,
        brush: &ID2D1SolidColorBrush,
        weight: f32,
    ) -> Result<(), String> {
        let geometry =
            unsafe { self.d2d_factory.CreatePathGeometry() }.map_err(|error| error.to_string())?;
        let sink = unsafe { geometry.Open() }.map_err(|error| error.to_string())?;
        unsafe {
            sink.BeginFigure(from, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: to,
                size: D2D_SIZE_F {
                    width: radius,
                    height: radius,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: D2D1_ARC_SIZE_SMALL,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
        }
        unsafe { sink.Close() }.map_err(|error| error.to_string())?;
        let target = self.target.as_ref().expect("render target exists");
        unsafe { target.DrawGeometry(&geometry, brush, weight, Some(&self.round_stroke)) };
        Ok(())
    }

    fn text(
        &self,
        value: &str,
        area: LogicalRect,
        format: &IDWriteTextFormat,
        fill: u32,
    ) -> Result<(), String> {
        let target = self.target.as_ref().expect("render target exists");
        let brush = unsafe { target.CreateSolidColorBrush(&color(fill), None) }
            .map_err(|error| error.to_string())?;
        let value = value.encode_utf16().collect::<Vec<_>>();
        unsafe {
            target.DrawText(
                &value,
                format,
                &area.d2d(),
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
        Ok(())
    }

    fn draw_thumbnail(
        &mut self,
        path: &Path,
        destination: LogicalRect,
        radius: f32,
    ) -> Result<bool, String> {
        if !self.thumbnails.contains_key(path) && !self.unavailable_thumbnails.contains(path) {
            match self.load_thumbnail(path) {
                Ok(bitmap) => {
                    self.thumbnails.insert(path.to_path_buf(), bitmap);
                    self.thumbnail_order.push_back(path.to_path_buf());
                    self.evict_cold_thumbnails();
                }
                Err(_) => {
                    self.unavailable_thumbnails.insert(path.to_path_buf());
                }
            }
        } else {
            self.touch_thumbnail(path);
        }
        let Some(bitmap) = self.thumbnails.get(path) else {
            return Ok(false);
        };
        let target = self.target.as_ref().expect("render target exists");
        let source = unsafe { bitmap.GetSize() };
        if source.width <= 0.0 || source.height <= 0.0 {
            return Ok(false);
        }
        let properties = D2D1_BITMAP_BRUSH_PROPERTIES {
            extendModeX: D2D1_EXTEND_MODE_CLAMP,
            extendModeY: D2D1_EXTEND_MODE_CLAMP,
            interpolationMode: D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
        };
        let brush = unsafe { target.CreateBitmapBrush(bitmap, Some(&properties), None) }
            .map_err(|error| error.to_string())?;
        let transform = Matrix3x2 {
            M11: (destination.right - destination.left) / source.width,
            M12: 0.0,
            M21: 0.0,
            M22: (destination.bottom - destination.top) / source.height,
            M31: destination.left,
            M32: destination.top,
        };
        unsafe {
            brush.SetTransform(&transform);
            target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: destination.d2d(),
                    radiusX: radius,
                    radiusY: radius,
                },
                &brush,
            );
        }
        Ok(true)
    }

    fn clip_duration(&mut self, path: &Path) -> Option<u64> {
        if let Some(duration) = self.clip_durations.get(path) {
            return *duration;
        }
        let duration = shell_clip_duration(path);
        self.clip_durations.insert(path.to_path_buf(), duration);
        duration
    }

    fn load_thumbnail(&self, path: &Path) -> Result<ID2D1Bitmap, String> {
        let path = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let shell: IShellItemImageFactory =
            unsafe { SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None::<&IBindCtx>) }
                .map_err(|error| error.to_string())?;
        let bitmap = unsafe {
            shell.GetImage(
                windows::Win32::Foundation::SIZE { cx: 320, cy: 180 },
                SIIGBF_RESIZETOFIT,
            )
        }
        .map_err(|error| error.to_string())?;
        let wic = unsafe {
            self.wic_factory.CreateBitmapFromHBITMAP(
                bitmap,
                HPALETTE::default(),
                WICBitmapIgnoreAlpha,
            )
        }
        .map_err(|error| error.to_string());
        let _ = unsafe { DeleteObject(bitmap.into()) };
        let wic = wic?;
        let target = self.target.as_ref().expect("render target exists");
        unsafe { target.CreateBitmapFromWicBitmap(&wic, None) }.map_err(|error| error.to_string())
    }
}

fn text_format(
    factory: &IDWriteFactory,
    family: PCWSTR,
    size: f32,
    semibold: bool,
    centered: bool,
) -> Result<IDWriteTextFormat, String> {
    let format = unsafe {
        factory.CreateTextFormat(
            family,
            None::<&IDWriteFontCollection>,
            if semibold {
                DWRITE_FONT_WEIGHT_SEMI_BOLD
            } else {
                DWRITE_FONT_WEIGHT_NORMAL
            },
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("en-US"),
        )
    }
    .map_err(|error| error.to_string())?;
    unsafe {
        format
            .SetTextAlignment(if centered {
                DWRITE_TEXT_ALIGNMENT_CENTER
            } else {
                DWRITE_TEXT_ALIGNMENT_LEADING
            })
            .map_err(|error| error.to_string())?;
        format
            .SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
            .map_err(|error| error.to_string())?;
        format
            .SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)
            .map_err(|error| error.to_string())?;
    }
    Ok(format)
}

struct LibraryLayout {
    columns: usize,
    card_width: f32,
    card_height: f32,
    row_pitch: f32,
    sections: Vec<f32>,
    height: f32,
}

fn hover_blend_amount(progress: f32, strength: f32, weight: f32) -> f32 {
    (progress * strength * weight).clamp(0.0, 1.0)
}

fn clip_columns(width: f32) -> usize {
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

fn library_layout(counts: &[usize], width: f32, grid: bool) -> LibraryLayout {
    let columns = if grid { clip_columns(width) } else { 1 };
    let card_width = if grid {
        ((width - CLIP_COLUMN_GAP * (columns - 1) as f32) / columns as f32).max(120.0)
    } else {
        width.max(240.0)
    };
    let card_height = if grid {
        (card_width * 9.0 / 16.0).round() + CLIP_META_HEIGHT
    } else {
        CLIP_LIST_ROW_HEIGHT
    };
    let row_pitch = if grid {
        card_height + CLIP_ROW_GAP
    } else {
        CLIP_LIST_ROW_HEIGHT
    };
    let mut sections = Vec::with_capacity(counts.len());
    let mut offset = 0.0;
    for count in counts {
        sections.push(offset);
        let rows = count.div_ceil(columns).max(1);
        offset +=
            CLIP_SECTION_HEADER + (rows - 1) as f32 * row_pitch + card_height + CLIP_GROUP_GAP;
    }
    LibraryLayout {
        columns,
        card_width,
        card_height,
        row_pitch,
        sections,
        height: (offset - CLIP_GROUP_GAP).max(0.0),
    }
}

pub fn sidebar_width(collapsed: bool) -> f32 {
    if collapsed {
        SIDEBAR_COLLAPSED_WIDTH
    } else {
        SIDEBAR_WIDTH
    }
}

fn page_has_chrome(page: Page) -> bool {
    matches!(page, Page::Library | Page::Collections | Page::Settings)
}

fn content_top() -> f32 {
    STAGE_INSET + 22.0
}

fn content_bottom(height: f32, chrome: bool) -> f32 {
    if chrome {
        height - STAGE_INSET - 8.0
    } else {
        height
    }
}

/// Height the folder rows need beyond the column, for the wheel handler.
fn folder_column_overflow_in(rows: LogicalRect, collections: usize) -> f32 {
    let content = (collections + 1) as f32 * (FOLDER_ROW_HEIGHT + 2.0);
    (content - (rows.bottom - rows.top)).max(0.0)
}

pub fn folder_column_overflow(model: &UiModel, width: f32, height: f32) -> f32 {
    if model.page != Page::Collections {
        return 0.0;
    }
    let left = sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING;
    let rows = rect(
        left,
        content_top() + LIBRARY_BODY_OFFSET + 26.0,
        (left + FOLDER_COLUMN_WIDTH).min(width),
        content_bottom(height, true),
    );
    folder_column_overflow_in(rows, model.collections.len())
}

/// True while the pointer sits over the collections folder column.
pub fn folder_column_contains(model: &UiModel, x: f32) -> bool {
    model.page == Page::Collections
        && x < sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING + FOLDER_COLUMN_WIDTH
}

pub fn clips_overflow(model: &UiModel, width: f32, height: f32) -> f32 {
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
        .clip_day_groups(&indices, today)
        .iter()
        .map(|group| group.indices.len())
        .collect::<Vec<_>>();
    let mut left = sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING;
    let mut top = content_top() + LIBRARY_BODY_OFFSET;
    if collections {
        left += FOLDER_COLUMN_WIDTH + 12.0 + FOLDER_COLUMN_GAP;
        top = content_top() + LIBRARY_BODY_OFFSET + 34.0;
    }
    let area_width = width - CONTENT_PADDING - left - CLIP_SCROLL_RESERVE;
    let layout = library_layout(&counts, area_width, model.library_grid);
    let selecting = model.selection_mode && !model.selected_clips.is_empty();
    let mut bottom = content_bottom(height, true);
    if selecting {
        bottom -= 60.0;
    }
    (layout.height - (bottom - top).max(0.0)).max(0.0)
}

fn text_format_trailing(
    factory: &IDWriteFactory,
    family: PCWSTR,
    size: f32,
) -> Result<IDWriteTextFormat, String> {
    let format = text_format(factory, family, size, false, false)?;
    unsafe { format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING) }
        .map_err(|error| error.to_string())?;
    Ok(format)
}

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> LogicalRect {
    LogicalRect {
        left,
        top,
        right,
        bottom,
    }
}

fn shell_clip_duration(path: &Path) -> Option<u64> {
    const PKEY_MEDIA_DURATION: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0x64440490_4c8b_11d1_8b70_080036b11a03),
        pid: 3,
    };
    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let store: IPropertyStore = unsafe {
        SHGetPropertyStoreFromParsingName(PCWSTR(wide.as_ptr()), None::<&IBindCtx>, GPS_DEFAULT)
    }
    .ok()?;
    let value = unsafe { store.GetValue(&PKEY_MEDIA_DURATION) }.ok()?;
    let hundred_nanoseconds = u64::try_from(&value).ok()?;
    (hundred_nanoseconds > 0).then(|| ((hundred_nanoseconds + 5_000_000) / 10_000_000).max(1))
}

fn format_clip_badge_duration(total_seconds: u64) -> String {
    let hours = total_seconds / 3_600;
    let minutes = total_seconds % 3_600 / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// A panel divides its own height by the rows it carries, so a long group stays
/// inside its box instead of running into the one below.
fn mix(from: u32, to: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |shift: u32| {
        let from = ((from >> shift) & 0xff) as f32;
        let to = ((to >> shift) & 0xff) as f32;
        from.mul_add(1.0 - amount, to * amount).round() as u32
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

fn color(rgb: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((rgb >> 16) & 0xff) as f32 / 255.0,
        g: ((rgb >> 8) & 0xff) as f32 / 255.0,
        b: (rgb & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    }
}

fn format_storage_limit(megabytes: u32) -> String {
    if megabytes >= 1_048_576 && megabytes % 1_048_576 == 0 {
        format!("{} TB", megabytes / 1_048_576)
    } else if megabytes >= 1_024 && megabytes % 1_024 == 0 {
        format!("{} GB", megabytes / 1_024)
    } else {
        format!("{megabytes} MB")
    }
}

fn format_player_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn format_editor_time(value: Duration) -> String {
    let total_millis = value.as_millis();
    let minutes = total_millis / 60_000;
    let seconds = total_millis % 60_000 / 1_000;
    let millis = total_millis % 1_000;
    format!("{minutes:02}:{seconds:02}.{millis:03}")
}

fn age(modified: SystemTime) -> String {
    let elapsed = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::ZERO);
    if elapsed.as_secs() < 60 {
        "now".into()
    } else if elapsed.as_secs() < 3_600 {
        format!("{}m ago", elapsed.as_secs() / 60)
    } else if elapsed.as_secs() < 86_400 {
        format!("{}h ago", elapsed.as_secs() / 3_600)
    } else {
        format!("{}d ago", elapsed.as_secs() / 86_400)
    }
}

fn hotkey_capture_label(modifiers: &[String], text: &Strings) -> String {
    let modifiers = modifiers
        .iter()
        .map(|modifier| match modifier.as_str() {
            "SUPER" => "Win",
            "CTRL" => "Ctrl",
            "ALT" => "Alt",
            "SHIFT" => "Shift",
            value => value,
        })
        .collect::<Vec<_>>();
    if modifiers.is_empty() {
        text.hotkey_prompt.to_owned()
    } else {
        format!("{} + …", modifiers.join(" + "))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CLIP_GROUP_GAP, CLIP_ROW_GAP, CLIP_SCROLL_RESERVE, CLIP_SECTION_HEADER, CONTENT_PADDING,
        FOLDER_ROW_HEIGHT, Page, Palette, SIDEBAR_COLLAPSED_WIDTH, SIDEBAR_WIDTH, STAGE_INSET,
        Theme, clip_columns, content_bottom, content_top, folder_column_overflow_in, format_bytes,
        format_storage_limit, hover_blend_amount, library_layout, page_has_chrome, palette_for,
        rect, settings_gain_percent, sidebar_width,
    };

    fn luminance(color: u32) -> f32 {
        let channel = |shift: u32| ((color >> shift) & 0xff) as f32 / 255.0;
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    fn contrast(palette: Palette) -> f32 {
        (luminance(palette.primary) - luminance(palette.canvas)).abs()
    }

    #[test]
    fn every_theme_keeps_text_readable_and_the_thumbnail_bed_quiet() {
        for theme in Theme::OPTIONS {
            let palette = palette_for(theme);
            assert!(
                contrast(palette) > 0.6,
                "{theme:?} does not separate text from the canvas"
            );
            assert!(
                (luminance(palette.stage) - luminance(palette.canvas)).abs() < 0.1,
                "{theme:?} makes the thumbnail bed shout"
            );
            assert_ne!(palette.accent, palette.accent_text);
        }

        assert!(luminance(palette_for(Theme::Light).canvas) > 0.7);
        assert!(luminance(palette_for(Theme::Dark).canvas) < 0.1);
    }

    #[test]
    fn only_the_tinted_themes_spend_an_accent_on_live_indicators() {
        assert_eq!(
            palette_for(Theme::Dark).live,
            palette_for(Theme::Dark).primary
        );
        assert_eq!(
            palette_for(Theme::Light).live,
            palette_for(Theme::Light).primary
        );
        assert_ne!(
            palette_for(Theme::Cafe).live,
            palette_for(Theme::Cafe).primary
        );
        assert_ne!(
            palette_for(Theme::Pink).live,
            palette_for(Theme::Pink).primary
        );
        assert_ne!(
            palette_for(Theme::Candy).live,
            palette_for(Theme::Candy).primary
        );
    }

    #[test]
    fn the_light_themes_are_the_ones_that_carry_a_light_window_frame() {
        for theme in Theme::OPTIONS {
            assert_eq!(
                theme.is_light(),
                luminance(palette_for(theme).canvas) > 0.5,
                "{theme:?} disagrees with its window frame"
            );
        }
    }

    #[test]
    fn hover_strength_scales_the_blend_and_can_switch_it_off() {
        assert_eq!(hover_blend_amount(1.0, 0.0, 1.0), 0.0);
        assert_eq!(hover_blend_amount(1.0, 0.55, 1.0), 0.55);
        assert_eq!(hover_blend_amount(0.5, 1.0, 1.0), 0.5);
        assert_eq!(hover_blend_amount(1.0, 1.6, 1.0), 1.0);
        assert_eq!(hover_blend_amount(1.0, 1.0, 0.72), 0.72);
    }

    #[test]
    fn storage_sizes_use_mb_gb_and_tb_labels() {
        assert_eq!(format_bytes(512 * 1_024), "0.5 MB");
        assert_eq!(format_bytes(20 * 1_048_576), "20.0 MB");
        assert_eq!(format_bytes(5 * 1_073_741_824), "5.0 GB");
        assert_eq!(format_storage_limit(512), "512 MB");
        assert_eq!(format_storage_limit(10_240), "10 GB");
        assert_eq!(format_storage_limit(1_048_576), "1 TB");
    }

    #[test]
    fn the_stage_card_frames_the_application_pages() {
        assert!(page_has_chrome(Page::Library));
        assert!(page_has_chrome(Page::Collections));
        assert!(page_has_chrome(Page::Settings));
        assert!(!page_has_chrome(Page::Player));
        assert!(!page_has_chrome(Page::Editor));

        assert!(content_top() >= STAGE_INSET + 16.0);
        assert_eq!(content_bottom(900.0, false), 900.0);
        assert!(900.0 - content_bottom(900.0, true) >= STAGE_INSET);
    }

    #[test]
    fn the_clips_grid_fills_the_window_without_oversized_cards() {
        let clips_width = |window: f32, collapsed: bool| {
            window - sidebar_width(collapsed) - CONTENT_PADDING * 2.0 - CLIP_SCROLL_RESERVE
        };

        assert_eq!(clip_columns(clips_width(1_440.0, false)), 4);
        assert_eq!(clip_columns(clips_width(1_920.0, false)), 5);
        assert_eq!(clip_columns(clips_width(1_600.0, true)), 5);
        assert_eq!(clip_columns(clips_width(1_100.0, false)), 3);
        assert_eq!(clip_columns(500.0), 2);
        assert_eq!(clip_columns(320.0), 1);

        assert_eq!(sidebar_width(false), SIDEBAR_WIDTH);
        assert_eq!(sidebar_width(true), SIDEBAR_COLLAPSED_WIDTH);
    }

    #[test]
    fn day_sections_stack_without_overlapping_their_rows() {
        let layout = library_layout(&[12, 8], 1_200.0, true);

        assert_eq!(layout.columns, 4);
        assert_eq!(layout.row_pitch, layout.card_height + CLIP_ROW_GAP);
        let first_section =
            CLIP_SECTION_HEADER + 2.0 * layout.row_pitch + layout.card_height + CLIP_GROUP_GAP;
        assert_eq!(layout.sections, vec![0.0, first_section]);
        assert_eq!(
            layout.height,
            first_section + CLIP_SECTION_HEADER + layout.row_pitch + layout.card_height
        );

        let single = library_layout(&[1], 1_200.0, true);
        assert_eq!(single.sections, vec![0.0]);
        assert_eq!(single.height, CLIP_SECTION_HEADER + single.card_height);
    }

    #[test]
    fn the_clips_list_lays_out_one_row_per_clip() {
        let layout = library_layout(&[3], 900.0, false);

        assert_eq!(layout.columns, 1);
        assert_eq!(layout.card_width, 900.0);
        assert_eq!(
            layout.height,
            CLIP_SECTION_HEADER + 3.0 * layout.card_height
        );
    }

    #[test]
    fn the_folder_column_scrolls_once_its_rows_pass_the_bottom() {
        let rows = rect(0.0, 0.0, 200.0, 200.0);

        assert_eq!(folder_column_overflow_in(rows, 2), 0.0);
        let overflow = folder_column_overflow_in(rows, 20);
        assert!(overflow > 0.0);
        assert_eq!(overflow, 21.0 * (FOLDER_ROW_HEIGHT + 2.0) - 200.0);
    }

    #[test]
    fn audio_gain_slider_maps_its_full_width_to_zero_through_two_hundred_percent() {
        let rail = rect(100.0, 20.0, 300.0, 24.0);

        assert_eq!(settings_gain_percent(rail, 50.0), 0);
        assert_eq!(settings_gain_percent(rail, 100.0), 0);
        assert_eq!(settings_gain_percent(rail, 200.0), 100);
        assert_eq!(settings_gain_percent(rail, 300.0), 200);
        assert_eq!(settings_gain_percent(rail, 350.0), 200);
    }
}
