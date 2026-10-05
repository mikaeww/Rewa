//! The clip player and the trim editor: opening and switching clips, seeking,
//! volume, keyframe-snapped trim handles, cutting on a worker, and fullscreen.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gtk::prelude::*;

use super::{App, FULLSCREEN_CONTROLS_LIFETIME};
use crate::model::Page;
use crate::render::{editor_timeline_fraction, editor_timeline_rail};

/// Seeks while dragging are throttled; the release always seeks.
const PLAYER_SEEK_INTERVAL: Duration = Duration::from_millis(50);
const PREVIEW_SEEK_INTERVAL: Duration = Duration::from_millis(33);
const PREVIEW_SEEK_SETTLE: Duration = Duration::from_millis(160);
const PREVIEW_SLACK_SECONDS: f64 = 0.05;

#[derive(Clone, Copy)]
pub(crate) enum EditorDrag {
    Start,
    End,
}

pub(crate) enum TrimUpdate {
    Timing {
        source: PathBuf,
        result: Result<rewa_core::trim::ClipTiming, String>,
    },
    Finished {
        source: PathBuf,
        replacing: bool,
        result: Result<rewa_core::trim::TrimReport, String>,
    },
}

pub(super) fn open_current_clip(app: &mut App) {
    app.model.reset_player_state();
    let Some(path) = app.model.active_clip().map(|clip| clip.path.clone()) else {
        return;
    };
    if let Err(error) = app.player.open(&path, app.model.player_volume_percent) {
        app.model.notice = Some(format!(
            "{}: {error}",
            app.model.strings().notice_cannot_play
        ));
    }
    app.renderer.set_video(Some(app.player.paintable()));
}

pub(super) fn stop_player(app: &mut App) {
    app.player.close();
    app.renderer.set_video(None);
    app.player_seek = None;
    app.preview_seek = None;
    sync_player_state(app);
}

pub(super) fn switch_clip(app: &mut App, offset: isize) {
    if app.model.page != Page::Player || !app.model.select_adjacent_clip(offset) {
        return;
    }
    open_current_clip(app);
}

pub(super) fn set_player_volume(app: &mut App, percent: u8) {
    if app.model.page != Page::Player {
        return;
    }
    app.model.set_player_volume(percent);
    if let Err(error) = app.player.set_volume(app.model.player_volume_percent) {
        app.model.notice = Some(format!("Cannot set playback volume: {error}"));
    }
}

pub(super) fn toggle_player_mute(app: &mut App) {
    if app.model.page != Page::Player {
        return;
    }
    app.model.toggle_player_mute();
    if let Err(error) = app.player.set_volume(app.model.player_volume_percent) {
        app.model.notice = Some(format!("Cannot change playback mute: {error}"));
    }
}

/// Moves the playhead with the pointer at once and the video at most every 50 ms.
pub(super) fn seek_player(app: &mut App, fraction: f64, settle: bool) {
    app.model.player_position_seconds = app.model.player_duration_seconds * fraction;
    let due = settle
        || app
            .player_seek
            .is_none_or(|issued| issued.elapsed() >= PLAYER_SEEK_INTERVAL);
    if due {
        let _ = app.player.seek_fraction(fraction);
        app.player_seek = Some(Instant::now());
    }
}

pub(super) fn sync_player_state(app: &mut App) {
    let snapshot = app.player.snapshot();
    app.model.player_ready = snapshot.ready;
    app.model.player_playing = snapshot.playing;
    let dragging_playhead = app.editor_drag.is_some()
        || matches!(
            app.slider_drag,
            Some(
                super::input::SliderDrag::PlayerSeek
                    | super::input::SliderDrag::EditorPlayhead
                    | super::input::SliderDrag::FullscreenSeek
            )
        );
    app.model.player_duration_seconds = snapshot.duration_seconds;
    if !dragging_playhead {
        app.model.player_position_seconds = if snapshot.duration_seconds > 0.0 {
            snapshot
                .position_seconds
                .clamp(0.0, snapshot.duration_seconds)
        } else {
            0.0
        };
    }
    app.model.player_aspect_ratio = snapshot.aspect_ratio;
    app.model.player_video_width = snapshot.video_width;
    app.model.player_video_height = snapshot.video_height;
}

pub(super) fn update_editor_drag(app: &mut App, x: f32, settle: bool) {
    let Some(handle) = app.editor_drag else {
        return;
    };
    let rail = editor_timeline_rail(&app.model, app.width as u32, app.height as u32);
    let thousandths = editor_timeline_fraction(rail, x);
    match handle {
        EditorDrag::Start => app.model.set_editor_start(thousandths),
        EditorDrag::End => app.model.set_editor_end(thousandths),
    }
    let position = match handle {
        EditorDrag::Start => app.model.editor_start,
        EditorDrag::End => app.model.editor_end,
    };
    seek_editor_preview(app, position, settle);
}

pub(super) fn seek_editor_preview(app: &mut App, position: Duration, settle: bool) {
    if !app.model.player_ready {
        return;
    }
    app.model.player_position_seconds = position.as_secs_f64();
    if !settle
        && app
            .preview_seek
            .is_some_and(|issued| issued.elapsed() < PREVIEW_SEEK_INTERVAL)
    {
        return;
    }
    let duration = app.model.player_duration_seconds;
    if duration <= f64::EPSILON {
        return;
    }
    let _ = app.player.seek_fraction(position.as_secs_f64() / duration);
    app.preview_seek = Some(Instant::now());
}

/// The preview plays the selection only: past the end it jumps back to the start.
pub(super) fn keep_editor_preview_inside_selection(app: &mut App) {
    if app.model.page != Page::Editor
        || app.model.editor_timing.is_none()
        || app.editor_drag.is_some()
        || !app.model.player_ready
    {
        return;
    }
    let position = app.model.player_position_seconds;
    let start = app.model.editor_start.as_secs_f64();
    let end = app.model.editor_end.as_secs_f64();
    if position + PREVIEW_SLACK_SECONDS >= start && position < end {
        app.preview_seek = None;
        return;
    }
    if app
        .preview_seek
        .is_some_and(|issued| issued.elapsed() < PREVIEW_SEEK_SETTLE)
    {
        return;
    }
    let resume = app.model.player_playing;
    let start = app.model.editor_start;
    seek_editor_preview(app, start, true);
    if resume {
        let _ = app.player.play();
    }
}

pub(super) fn begin_editor(app: &mut App) {
    let Some(source) = app.model.active_clip().map(|clip| clip.path.clone()) else {
        app.model.notice = Some(app.model.strings().notice_clip_gone.to_owned());
        return;
    };
    if !app.model.edit_active_clip() {
        return;
    }
    let updates = app.trim_sender.clone();
    let spawned = std::thread::Builder::new()
        .name("rewa-editor-timing".into())
        .spawn(move || {
            let backend = rewa_core::trim_ffmpeg::FfmpegTrimmer;
            let result =
                rewa_core::trim::timing(&backend, &source).map_err(|error| error.to_string());
            let _ = updates.send(TrimUpdate::Timing { source, result });
        });
    if spawned.is_err() {
        app.model.editor_loading = false;
        app.model.notice = Some(app.model.strings().notice_cannot_open_editor.to_owned());
    }
}

pub(super) fn save_cut(app: &mut App, output: rewa_core::trim::TrimOutput) {
    if app.model.editor_working || app.model.editor_timing.is_none() {
        return;
    }
    let Some(source) = app.model.active_clip().map(|clip| clip.path.clone()) else {
        app.model.notice = Some(app.model.strings().notice_clip_gone.to_owned());
        return;
    };
    let replacing = matches!(output, rewa_core::trim::TrimOutput::Replace);
    let request = rewa_core::trim::TrimRequest {
        source: source.clone(),
        start: app.model.editor_start,
        end: app.model.editor_end,
        mode: rewa_core::trim::TrimMode::Auto,
        output,
    };
    let thumbnails = app.model.paths.thumbnail_dir.clone();
    let updates = app.trim_sender.clone();
    if replacing {
        // the player holds the clip open while it is being replaced
        stop_player(app);
    }
    app.model.editor_working = true;
    let text = app.model.strings();
    app.model.notice = Some(if replacing {
        text.notice_replace_running.to_owned()
    } else {
        text.notice_cut_running.to_owned()
    });
    let spawned = std::thread::Builder::new()
        .name("rewa-editor-cut".into())
        .spawn(move || {
            let backend = rewa_core::trim_ffmpeg::FfmpegTrimmer;
            let result = rewa_core::trim::trim(&backend, &request, &thumbnails)
                .map_err(|error| error.to_string());
            let _ = updates.send(TrimUpdate::Finished {
                source,
                replacing,
                result,
            });
        });
    if spawned.is_err() {
        app.model.editor_working = false;
        app.model.notice = Some(app.model.strings().notice_cannot_cut.to_owned());
        if replacing {
            open_current_clip(app);
        }
    }
}

pub(super) fn poll_trim_updates(app: &mut App) -> bool {
    let mut changed = false;
    while let Ok(update) = app.trim_updates.try_recv() {
        let active = app.model.editor_source.clone();
        match update {
            TrimUpdate::Timing { source, result } if active.as_ref() == Some(&source) => {
                changed = true;
                apply_timing(app, result);
            }
            TrimUpdate::Finished {
                source,
                replacing,
                result,
            } if active.as_ref() == Some(&source) => {
                changed = true;
                finish_cut(app, &source, replacing, result);
            }
            _ => {}
        }
    }
    changed
}

fn apply_timing(app: &mut App, result: Result<rewa_core::trim::ClipTiming, String>) {
    match result {
        Ok(timing) if !timing.duration.is_zero() => {
            app.model.apply_editor_timing(timing);
            app.model.notice = None;
        }
        Ok(_) => {
            app.model.editor_loading = false;
            app.model.notice = Some("This clip has no readable duration".into());
        }
        Err(error) => {
            app.model.editor_loading = false;
            app.model.notice = Some(format!(
                "{}: {error}",
                app.model.strings().notice_cannot_open_editor
            ));
        }
    }
}

fn finish_cut(
    app: &mut App,
    source: &std::path::Path,
    replacing: bool,
    result: Result<rewa_core::trim::TrimReport, String>,
) {
    app.model.editor_working = false;
    let succeeded = match result {
        Ok(report) => {
            if replacing {
                app.renderer.forget_clip(source);
            }
            let refreshed = app.model.refresh();
            if refreshed.is_ok() {
                app.renderer.retry_unavailable_thumbnails();
            }
            let name = report.path.file_name().map_or_else(
                || report.path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            let text = app.model.strings();
            let message = format!(
                "{} · {name}",
                match (report.reencoded, replacing) {
                    (true, true) => text.notice_replaced_reencoded,
                    (true, false) => text.notice_cut_reencoded,
                    (false, true) => text.notice_replaced_lossless,
                    (false, false) => text.notice_cut_lossless,
                }
            );
            super::daemon::set_result(&mut app.model, refreshed, &message);
            true
        }
        Err(error) => {
            app.model.notice = Some(format!(
                "{}: {error}",
                app.model.strings().notice_cannot_cut
            ));
            false
        }
    };
    if succeeded && app.model.page == Page::Editor {
        // a finished cut is done with: back to the clips, where the result sits
        stop_player(app);
        app.model.navigate(Page::Library);
    } else if replacing && !succeeded {
        open_current_clip(app);
    }
}

pub(super) fn toggle_player_fullscreen(app: &mut App) {
    if app.fullscreen {
        exit_player_fullscreen(app);
        return;
    }
    if app.model.page != Page::Player {
        return;
    }
    app.fullscreen = true;
    app.window.fullscreen();
    reveal_fullscreen_controls(app);
    app.redraw();
}

pub(super) fn exit_player_fullscreen(app: &mut App) {
    if !app.fullscreen {
        return;
    }
    app.fullscreen = false;
    app.fullscreen_controls_until = None;
    app.fullscreen_controls_visible = false;
    app.pointer_over_controls = false;
    app.controls.set_visible(false);
    app.window.unfullscreen();
    app.redraw();
}

pub(super) fn reveal_fullscreen_controls(app: &mut App) {
    if !app.fullscreen {
        return;
    }
    app.fullscreen_controls_until = Some(Instant::now() + FULLSCREEN_CONTROLS_LIFETIME);
    if !app.fullscreen_controls_visible {
        app.fullscreen_controls_visible = true;
        app.controls.set_visible(true);
    }
    app.controls.queue_draw();
}

/// The controls float over the foot of the video and hide a few seconds after
/// the pointer last moved, unless it rests on them or drags a slider.
pub(super) fn update_fullscreen_controls_visibility(app: &mut App) -> bool {
    let dragging = matches!(
        app.slider_drag,
        Some(super::input::SliderDrag::FullscreenSeek | super::input::SliderDrag::FullscreenVolume)
    );
    let should_show = app.fullscreen
        && (app.pointer_over_controls
            || dragging
            || app
                .fullscreen_controls_until
                .is_some_and(|deadline| Instant::now() < deadline));
    if should_show == app.fullscreen_controls_visible {
        return false;
    }
    app.fullscreen_controls_visible = should_show;
    app.controls.set_visible(should_show);
    true
}
