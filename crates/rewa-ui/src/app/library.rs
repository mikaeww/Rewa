//! Clip management behind the pages: renaming and creating through the prompt,
//! deleting, moving clips into collections, and dragging cards onto a folder or
//! out of the window into another program.

use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{gdk, gio, graphene};

use super::App;
use crate::model::{Action, ClipDragPreview, DeleteTarget, PromptKind, UiModel};

/// Pointer travel before a press on a card becomes a drag.
const DRAG_THRESHOLD_SQUARED: f32 = 36.0;

pub(crate) struct ClipDrag {
    clip: usize,
    start_x: f32,
    start_y: f32,
    active: bool,
}

impl ClipDrag {
    pub(super) fn new(clip: usize, start_x: f32, start_y: f32) -> Self {
        Self {
            clip,
            start_x,
            start_y,
            active: false,
        }
    }
}

pub(super) fn confirm_prompt(app: &mut App) {
    let Some(prompt) = app.model.prompt.take() else {
        return;
    };
    let name = prompt.input.value.trim().to_owned();
    let text = app.model.strings();
    let outcome = match prompt.kind {
        PromptKind::NewCollection => {
            let directory = app.model.config.storage.directory.clone();
            match rewa_core::clips::create_collection(&directory, &name) {
                Ok(path) => {
                    app.model.active_collection = Some(path);
                    app.model.active_game = None;
                    Ok(text.notice_collection_created)
                }
                Err(error) => Err(format!("{}: {error}", text.notice_cannot_create_collection)),
            }
        }
        PromptKind::RenameClip(index) => {
            let Some(clip) = app.model.clips.get(index).cloned() else {
                app.model.notice = Some(text.notice_clip_gone.to_owned());
                return;
            };
            let thumbnails = app.model.paths.thumbnail_dir.clone();
            match rewa_core::clips::rename(&clip, &name, &thumbnails) {
                Ok(renamed) => {
                    app.model.favorites.relocate(&clip.path, &renamed);
                    let _ = app.model.favorites.save();
                    Ok(text.notice_clip_renamed)
                }
                Err(error) => Err(format!("{}: {error}", text.notice_cannot_rename_clip)),
            }
        }
        PromptKind::RenameCollection(collection) => {
            let directory = app.model.config.storage.directory.clone();
            match rewa_core::clips::rename_collection(&directory, &collection, &name) {
                Ok(path) => {
                    app.model.active_collection = Some(path);
                    app.model.active_game = None;
                    Ok(text.notice_collection_renamed)
                }
                Err(error) => Err(format!("{}: {error}", text.notice_cannot_rename_collection)),
            }
        }
    };
    match outcome {
        Ok(message) => {
            let refreshed = app.model.refresh();
            if refreshed.is_ok() {
                app.renderer.retry_unavailable_thumbnails();
            }
            super::daemon::set_result(&mut app.model, refreshed, message);
        }
        Err(error) => app.model.notice = Some(error),
    }
}

pub(super) fn confirm_delete(model: &mut UiModel) -> bool {
    let Some(target) = model.pending_delete.take() else {
        return false;
    };
    match target {
        DeleteTarget::Clip(index) => {
            let Some(clip) = model.clips.get(index).cloned() else {
                model.notice = Some(model.strings().notice_clip_gone.to_owned());
                return false;
            };
            match rewa_core::clips::delete(&clip, &model.paths.thumbnail_dir) {
                Ok(()) => {
                    model.favorites.remove(&clip.path);
                    let _ = model.favorites.save();
                    let result = model.refresh();
                    let message = model.strings().notice_clip_deleted;
                    super::daemon::set_result(model, result, message);
                    true
                }
                Err(error) => {
                    model.notice = Some(format!(
                        "{}: {error}",
                        model.strings().notice_cannot_delete_clip
                    ));
                    false
                }
            }
        }
        DeleteTarget::Collection(collection) => match rewa_core::clips::delete_collection(
            &model.config.storage.directory,
            &collection,
            &model.paths.thumbnail_dir,
        ) {
            Ok(()) => {
                model.active_collection = None;
                let result = model.refresh();
                let message = model.strings().notice_collection_deleted;
                super::daemon::set_result(model, result, message);
                true
            }
            Err(error) => {
                model.notice = Some(format!(
                    "{}: {error}",
                    model.strings().notice_cannot_delete_collection
                ));
                false
            }
        },
    }
}

pub(super) fn update_clip_drag(app: &mut App, x: f32, y: f32) {
    let Some(drag) = &mut app.clip_drag else {
        return;
    };
    if !drag.active {
        let delta_x = x - drag.start_x;
        let delta_y = y - drag.start_y;
        if delta_x.mul_add(delta_x, delta_y * delta_y) < DRAG_THRESHOLD_SQUARED {
            return;
        }
        drag.active = true;
    }
    let clip = drag.clip;
    let inside = x >= 0.0 && y >= 0.0 && x < app.width && y < app.height;
    // inside the window the drag stays Rewa's own light chip; the desktop's drag,
    // which other programs understand, only takes over once the clip leaves
    if !inside {
        let paths = dragged_clip_paths(&app.model, clip);
        drag_clips_out(app, &paths, x, y);
        return;
    }
    let count = app
        .model
        .clips
        .get(clip)
        .filter(|clip| app.model.selected_clips.contains(&clip.path))
        .map_or(1, |_| app.model.selected_clips.len().max(1));
    let target_collection = match app.renderer.hit_test(x, y) {
        Some(Action::SelectCollection(Some(index))) => Some(index),
        _ => None,
    };
    app.model.clip_drag_preview = Some(ClipDragPreview {
        clip,
        count,
        x,
        y,
        target_collection,
    });
}

pub(super) fn finish_clip_drag(app: &mut App, x: f32, y: f32) {
    let Some(drag) = app.clip_drag.take() else {
        return;
    };
    let target_collection = app
        .model
        .clip_drag_preview
        .as_ref()
        .and_then(|preview| preview.target_collection);
    app.model.clip_drag_preview = None;
    if drag.active {
        if let Some(collection) = target_collection {
            let paths = dragged_clip_paths(&app.model, drag.clip);
            move_clip_paths_to_collection(app, &paths, collection);
        }
        return;
    }
    let clicked_same_clip = matches!(
        app.renderer.hit_test(x, y),
        Some(Action::OpenClip(index) | Action::ToggleClipSelection(index)) if index == drag.clip
    );
    if !clicked_same_clip {
        return;
    }
    if app.model.selection_mode {
        app.model.toggle_clip_selection(drag.clip);
    } else {
        super::actions::handle_action(app, Action::OpenClip(drag.clip));
    }
}

/// Hands the clips to the desktop's drag and drop as a file list, so they drop
/// into a file manager, chat or browser; copy only, the library keeps them.
fn drag_clips_out(app: &mut App, paths: &[PathBuf], x: f32, y: f32) {
    app.clip_drag = None;
    app.model.clip_drag_preview = None;
    let files = paths.iter().map(gio::File::for_path).collect::<Vec<_>>();
    if files.is_empty() {
        return;
    }
    let started = (|| {
        let device = app.pointer_device.clone()?;
        let native = app.surface.native()?;
        let surface = native.surface()?;
        let (offset_x, offset_y) = native.surface_transform();
        let point = app
            .surface
            .compute_point(&native, &graphene::Point::new(x, y))?;
        let content =
            gdk::ContentProvider::for_value(&gdk::FileList::from_array(&files).to_value());
        gdk::Drag::begin(
            &surface,
            &device,
            &content,
            gdk::DragAction::COPY,
            f64::from(point.x()) + offset_x,
            f64::from(point.y()) + offset_y,
        )
    })();
    match started {
        Some(drag) => app.outgoing_drag = Some(drag),
        None => {
            app.model.notice = Some(app.model.strings().notice_cannot_drag.to_owned());
        }
    }
}

fn dragged_clip_paths(model: &UiModel, dragged: usize) -> Vec<PathBuf> {
    let Some(clip) = model.clips.get(dragged) else {
        return Vec::new();
    };
    if !model.selected_clips.contains(&clip.path) {
        return vec![clip.path.clone()];
    }
    model
        .clips
        .iter()
        .filter(|clip| model.selected_clips.contains(&clip.path))
        .map(|clip| clip.path.clone())
        .collect()
}

pub(super) fn move_clip_paths_to_collection(app: &mut App, paths: &[PathBuf], collection: usize) {
    let Some(collection) = app.model.collections.get(collection).cloned() else {
        app.model.notice = Some(app.model.strings().notice_collection_gone.to_owned());
        return;
    };
    let clips = paths
        .iter()
        .filter_map(|path| {
            app.model
                .clips
                .iter()
                .find(|clip| &clip.path == path)
                .cloned()
        })
        .filter(|clip| clip.path.parent() != Some(collection.path.as_path()))
        .collect::<Vec<_>>();
    if clips.is_empty() {
        app.model.notice = Some(app.model.strings().notice_already_in_collection.to_owned());
        return;
    }
    if let Some(existing) = clips.iter().find_map(|clip| {
        let destination = collection.path.join(clip.path.file_name()?);
        destination.exists().then_some(destination)
    }) {
        app.model.notice = Some(format!(
            "{}: {} → {}",
            app.model.strings().notice_cannot_move_clips,
            existing.file_name().map_or_else(
                || String::from("?"),
                |name| name.to_string_lossy().into_owned()
            ),
            collection.name
        ));
        return;
    }
    let mut moved = 0usize;
    let mut failure = None;
    for clip in clips {
        match rewa_core::clips::move_to_collection(
            &clip,
            &app.model.config.storage.directory,
            &collection.path,
            &app.model.paths.thumbnail_dir,
        ) {
            Ok(destination) => {
                app.model.favorites.relocate(&clip.path, &destination);
                moved += 1;
            }
            Err(error) => {
                failure = Some(error.to_string());
                break;
            }
        }
    }
    let _ = app.model.favorites.save();
    let refresh = app.model.refresh();
    app.renderer.retry_unavailable_thumbnails();
    app.model.clear_clip_selection();
    let moved_text = app.model.strings().moved_clips(moved, &collection.name);
    app.model.notice = match (refresh, failure) {
        (Err(error), _) => Some(error),
        (Ok(()), Some(error)) => Some(format!("{moved_text} · {error}")),
        (Ok(()), None) => Some(moved_text),
    };
}
