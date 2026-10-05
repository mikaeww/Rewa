//! Where everything sits: the shared measures, the stage rectangles the player and
//! the input handlers need, and the clip grid and folder column arithmetic.
//! Drawing and hit testing both read these, so they always agree.

use gtk::graphene;

use super::*;

pub(crate) const RADIUS: f32 = 10.0;
pub(crate) const RADIUS_SMALL: f32 = 8.0;
pub(crate) const RADIUS_LARGE: f32 = 12.0;
pub(crate) const SIDEBAR_WIDTH: f32 = 212.0;
pub(crate) const SIDEBAR_COLLAPSED_WIDTH: f32 = 64.0;
pub(crate) const CONTENT_PADDING: f32 = 28.0;
/// Frame visible around the stage card on its three free sides.
pub(crate) const STAGE_INSET: f32 = 8.0;
pub(crate) const SIDEBAR_ROW_INSET: f32 = 12.0;
/// Row icons share one column in both sidebar states, so nothing jumps while it folds.
pub(crate) const SIDEBAR_ICON_LEFT: f32 = 24.0;
pub(crate) const LIBRARY_BODY_OFFSET: f32 = 58.0;
pub(crate) const POPOVER_WIDTH: f32 = 304.0;
/// Top of the video stage in preview and editor; the player child window uses it too.
pub(crate) const PLAYER_TOP: f32 = STAGE_INSET + 22.0 + 54.0;
/// Black band above the fullscreen video that holds the way back.
pub(crate) const FULLSCREEN_HEADER_HEIGHT: f32 = 78.0;
pub(crate) const CAPTURE_ROW_HEIGHT: f32 = 36.0;
pub(crate) const FILTER_PANEL_WIDTH: f32 = 272.0;
pub(crate) const CLIP_GAP: f32 = 18.0;
pub(crate) const CLIP_META_HEIGHT: f32 = 48.0;
pub(crate) const COMPACT_GAP: f32 = 10.0;
pub(crate) const COMPACT_CARD_WIDTH: f32 = 150.0;
pub(crate) const COMPACT_META_HEIGHT: f32 = 28.0;
pub(crate) const CLIP_SECTION_HEADER: f32 = 34.0;
pub(crate) const CLIP_GROUP_GAP: f32 = 22.0;
pub(crate) const CLIP_SCROLL_RESERVE: f32 = 14.0;
pub(crate) const FILTER_ROW_PITCH: f32 = 56.0;
pub(crate) const FOLDER_COLUMN_WIDTH: f32 = 236.0;
/// Room above the folder rows for the column's search field.
pub(crate) const FOLDER_SEARCH_HEIGHT: f32 = 34.0;
pub(crate) const FOLDER_COLUMN_MIN: f32 = 180.0;
pub(crate) const FOLDER_COLUMN_MAX: f32 = 420.0;
pub(crate) const FOLDER_COLUMN_GAP: f32 = 24.0;
pub(crate) const FOLDER_ROW_HEIGHT: f32 = 30.0;
pub(crate) const SETTINGS_RAIL_WIDTH: f32 = 184.0;
pub(crate) const SETTINGS_LINE_HEIGHT: f32 = 58.0;
pub(crate) const SETTINGS_CONTENT_WIDTH: f32 = 640.0;
pub(crate) const METER_WIDTH: f32 = 84.0;
pub(crate) const NAVIGATION_TOP: f32 = 62.0;
pub(crate) const NAVIGATION_HEIGHT: f32 = 30.0;
pub(crate) const NAVIGATION_PITCH: f32 = 32.0;
pub(crate) const EDITOR_BOTTOM_RESERVE: f32 = 226.0;
pub(crate) const PLAYER_ARROW_GUTTER: f32 = 48.0;
/// Where the seek rail starts under the video, after play and the time.
pub(crate) const PLAYER_RAIL_LEFT: f32 = 118.0;
/// Middle of the playback row, measured from the foot of the video.
pub(crate) const PLAYBACK_ROW_CENTER: f32 = 38.0;
pub(crate) const EDITOR_TIMELINE_HEIGHT: f32 = 118.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct LogicalRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl LogicalRect {
    pub(crate) fn contains(self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }

    pub(crate) fn graphene(self) -> graphene::Rect {
        graphene::Rect::new(
            self.left,
            self.top,
            self.right - self.left,
            self.bottom - self.top,
        )
    }
}

pub(crate) fn player_bounds(model: &UiModel, width: u32, height: u32) -> LogicalRect {
    video_stage(
        sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING,
        width as f32 - CONTENT_PADDING,
        height as f32,
        model,
    )
}

pub(crate) fn editor_player_bounds(model: &UiModel, width: u32, height: u32) -> LogicalRect {
    player_bounds(model, width, height)
}

/// The video stage of preview and editor; the player child window takes the same
/// rectangle. The preview keeps a gutter on both sides for the clip arrows.
pub(crate) fn video_stage(left: f32, right: f32, height: f32, model: &UiModel) -> LogicalRect {
    let detail = if right - left >= 960.0 { 300.0 } else { 0.0 };
    let (gutter, bottom) = if model.page == Page::Editor {
        (0.0, (height - EDITOR_BOTTOM_RESERVE).max(360.0))
    } else {
        (PLAYER_ARROW_GUTTER, (height - 178.0).max(390.0))
    };
    fit_aspect(
        rect(left + gutter, PLAYER_TOP, right - detail - gutter, bottom),
        model.player_aspect_ratio,
    )
}

pub(crate) fn editor_timeline_rail(model: &UiModel, width: u32, height: u32) -> LogicalRect {
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

pub(crate) fn editor_timeline_fraction(rail: LogicalRect, x: f32) -> u16 {
    (((x - rail.left) / (rail.right - rail.left).max(1.0)).clamp(0.0, 1.0) * 1000.0).round() as u16
}

pub(crate) fn player_timeline_rail(model: &UiModel, width: u32, height: u32) -> LogicalRect {
    let stage = player_bounds(model, width, height);
    playback_rail(
        stage.left,
        stage.right,
        stage.bottom + PLAYBACK_ROW_CENTER,
        false,
    )
}

/// Where the seek rail runs in a playback row; play sits before it, the doors after.
pub(crate) fn playback_rail(left: f32, right: f32, center: f32, with_mute: bool) -> LogicalRect {
    rect(
        left + PLAYER_RAIL_LEFT,
        center - 2.0,
        right - if with_mute { 76.0 } else { 40.0 },
        center + 2.0,
    )
}

/// The picture inside the fullscreen video area, once its aspect is kept.
pub(crate) fn fullscreen_picture(model: &UiModel, width: u32, height: u32) -> LogicalRect {
    fit_aspect(
        rect(0.0, FULLSCREEN_HEADER_HEIGHT, width as f32, height as f32),
        model.player_aspect_ratio,
    )
}

pub(crate) fn player_volume_rail(model: &UiModel, width: u32, height: u32) -> LogicalRect {
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

pub(crate) fn fullscreen_timeline_rail(
    model: &UiModel,
    width: u32,
    height: u32,
    controls_height: f32,
) -> LogicalRect {
    let picture = fullscreen_picture(model, width, height);
    playback_rail(picture.left, picture.right, controls_height / 2.0, true)
}

pub(crate) fn fullscreen_volume_rail(width: u32, height: u32) -> LogicalRect {
    let width = width as f32;
    let height = height as f32;
    rect(204.0, height - 34.0, width.min(334.0), height - 28.0)
}

pub(crate) fn settings_audio_gain_rail(
    model: &UiModel,
    width: u32,
    _height: u32,
    row: usize,
) -> LogicalRect {
    let left = sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING;
    let right = width as f32 - CONTENT_PADDING;
    settings_slider_track(settings_line_rect(left, right, content_top(), row))
}

pub(crate) fn device_name<'a>(
    devices: &'a [(String, String)],
    chosen: Option<&String>,
    fallback: &'a str,
) -> &'a str {
    chosen
        .and_then(|id| devices.iter().find(|(device_id, _)| device_id == id))
        .map_or(fallback, |(_, name)| name.as_str())
}

pub(crate) fn settings_content(left: f32, right: f32) -> (f32, f32) {
    let content_left = left + SETTINGS_RAIL_WIDTH + 33.0;
    (
        content_left,
        right.min(content_left + SETTINGS_CONTENT_WIDTH),
    )
}

pub(crate) fn settings_line_rect(left: f32, right: f32, top: f32, index: usize) -> LogicalRect {
    let (content_left, content_right) = settings_content(left, right);
    let card_top = top + 52.0;
    rect(
        content_left,
        card_top + index as f32 * SETTINGS_LINE_HEIGHT,
        content_right,
        card_top + (index + 1) as f32 * SETTINGS_LINE_HEIGHT,
    )
}

pub(crate) fn settings_slider_track(line: LogicalRect) -> LogicalRect {
    let center = (line.top + line.bottom) / 2.0;
    rect(
        line.right - 14.0 - 150.0,
        center - 2.0,
        line.right - 14.0,
        center + 2.0,
    )
}

pub(crate) fn settings_gain_percent(rail: LogicalRect, x: f32) -> u16 {
    (((x - rail.left) / (rail.right - rail.left).max(1.0)).clamp(0.0, 1.0) * 200.0).round() as u16
}

pub(crate) fn fit_aspect(area: LogicalRect, aspect_ratio: f32) -> LogicalRect {
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

pub(crate) fn hover_blend_amount(progress: f32, strength: f32, weight: f32) -> f32 {
    (progress * strength * weight).clamp(0.0, 1.0)
}

pub(crate) fn sidebar_width(collapsed: bool) -> f32 {
    if collapsed {
        SIDEBAR_COLLAPSED_WIDTH
    } else {
        SIDEBAR_WIDTH
    }
}

pub(crate) fn page_has_chrome(page: Page) -> bool {
    matches!(page, Page::Library | Page::Collections | Page::Settings)
}

pub(crate) fn content_top() -> f32 {
    STAGE_INSET + 22.0
}

pub(crate) fn content_bottom(height: f32, chrome: bool) -> f32 {
    if chrome {
        height - STAGE_INSET - 8.0
    } else {
        height
    }
}

/// One line of the collections column: all clips, the recorded games, then the
/// folders; drawing and the wheel handler both walk this list.
pub(crate) enum FolderEntry {
    Caption(&'static str),
    Note(&'static str),
    Row {
        glyph: Glyph,
        label: String,
        action: Action,
        active: bool,
    },
}

impl FolderEntry {
    pub(crate) fn pitch(&self) -> f32 {
        match self {
            Self::Caption(_) => 34.0,
            Self::Note(_) | Self::Row { .. } => FOLDER_ROW_HEIGHT + 2.0,
        }
    }
}

pub(crate) fn folder_entries(model: &UiModel) -> Vec<FolderEntry> {
    let text = model.strings();
    let mut entries = vec![FolderEntry::Row {
        glyph: Glyph::Library,
        label: text.all_clips.to_owned(),
        action: Action::SelectCollection(None),
        active: model.active_collection.is_none() && model.active_game.is_none(),
    }];
    let games = model.visible_games();
    if !games.is_empty() {
        entries.push(FolderEntry::Caption(text.games_heading));
        entries.extend(games.into_iter().map(|(index, game)| FolderEntry::Row {
            glyph: Glyph::Game,
            active: model.active_game.as_ref() == Some(&game),
            label: game,
            action: Action::SelectGame(index),
        }));
    }
    entries.push(FolderEntry::Caption(text.collections_heading));
    let collections = model.visible_collection_indices();
    if collections.is_empty() {
        entries.push(FolderEntry::Note(text.no_collections));
    }
    entries.extend(collections.into_iter().map(|index| {
        let collection = &model.collections[index];
        FolderEntry::Row {
            glyph: Glyph::Folder,
            label: collection.name.clone(),
            action: Action::SelectCollection(Some(index)),
            active: model.active_collection.as_ref() == Some(&collection.path),
        }
    }));
    entries
}

/// Height the folder rows need beyond the column, for the wheel handler.
pub(crate) fn folder_column_overflow_in(rows: LogicalRect, entries: &[FolderEntry]) -> f32 {
    let content = entries.iter().map(FolderEntry::pitch).sum::<f32>();
    (content - (rows.bottom - rows.top)).max(0.0)
}

pub(crate) fn folder_column_overflow(model: &UiModel, width: f32, height: f32) -> f32 {
    let column_width = folder_column_width(model);
    if model.page != Page::Collections || column_width == 0.0 {
        return 0.0;
    }
    let left = sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING;
    let rows = rect(
        left,
        content_top() + LIBRARY_BODY_OFFSET + FOLDER_SEARCH_HEIGHT,
        (left + column_width).min(width),
        content_bottom(height, true),
    );
    folder_column_overflow_in(rows, &folder_entries(model))
}

/// True while the pointer sits over the collections folder column.
pub(crate) fn folder_column_contains(model: &UiModel, x: f32) -> bool {
    model.page == Page::Collections
        && x < sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING + folder_column_width(model)
}

/// The collections column as dragged, or nothing while it is folded away.
pub(crate) fn folder_column_width(model: &UiModel) -> f32 {
    let appearance = &model.config.appearance;
    if appearance.folders_collapsed {
        return 0.0;
    }
    appearance
        .folder_width
        .map_or(FOLDER_COLUMN_WIDTH, f32::from)
        .clamp(FOLDER_COLUMN_MIN, FOLDER_COLUMN_MAX)
}

/// Where the clips start on the collections page, right of the column and its divider.
pub(crate) fn folder_area_left(model: &UiModel, left: f32) -> f32 {
    let width = folder_column_width(model);
    if width > 0.0 {
        left + width + 12.0 + FOLDER_COLUMN_GAP
    } else {
        left
    }
}

/// The column width a divider dragged to `x` asks for.
pub(crate) fn folder_width_at(model: &UiModel, x: f32) -> u16 {
    let left = sidebar_width(model.sidebar_collapsed) + CONTENT_PADDING;
    // the divider sits 12 px right of the column's edge
    (x - 12.0 - left)
        .clamp(FOLDER_COLUMN_MIN, FOLDER_COLUMN_MAX)
        .round() as u16
}

pub(crate) fn rect(left: f32, top: f32, right: f32, bottom: f32) -> LogicalRect {
    LogicalRect {
        left,
        top,
        right,
        bottom,
    }
}
