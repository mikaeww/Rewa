//! Pointer input on the main surface and the fullscreen controls: hit testing,
//! hover, drags of sliders, handles, clips and text carets, and the wheel.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;

use super::{App, library, media};
use crate::model::{Action, ClipContextMenu, Page};
use crate::render::{
    clips_overflow, editor_timeline_fraction, editor_timeline_rail, folder_column_contains,
    folder_column_overflow, folder_width_at, fullscreen_timeline_rail, fullscreen_volume_rail,
    player_timeline_rail, player_volume_rail, settings_audio_gain_rail, settings_gain_percent,
};

/// A wheel notch scrolls this far, as on Windows.
const LIBRARY_WHEEL_STEP: f32 = 104.0;

#[derive(Clone, Copy)]
pub(crate) enum SliderDrag {
    DesktopGain,
    MicrophoneGain,
    PlayerSeek,
    PlayerVolume,
    EditorPlayhead,
    FolderColumn,
    FullscreenSeek,
    FullscreenVolume,
}

#[derive(Clone, Copy)]
pub(crate) enum TextDrag {
    Search,
    Prompt,
}

pub(super) fn install(app: &Rc<RefCell<App>>) {
    let (surface, controls) = {
        let app = app.borrow();
        (app.surface.clone(), app.controls.clone())
    };
    let primary = gtk::GestureClick::new();
    primary.set_button(gdk::BUTTON_PRIMARY);
    let weak = Rc::downgrade(app);
    primary.connect_pressed(move |gesture, _, x, y| {
        with(&weak, |app| {
            app.pointer_device = gesture.device();
            let extend = gesture
                .current_event_state()
                .contains(gdk::ModifierType::SHIFT_MASK);
            left_down(app, x as f32, y as f32, extend);
        });
    });
    let weak = Rc::downgrade(app);
    primary.connect_released(move |_, _, x, y| {
        with(&weak, |app| left_up(app, x as f32, y as f32));
    });
    surface.add_controller(primary);

    let secondary = gtk::GestureClick::new();
    secondary.set_button(gdk::BUTTON_SECONDARY);
    let weak = Rc::downgrade(app);
    secondary.connect_released(move |_, _, x, y| {
        with(&weak, |app| right_up(app, x as f32, y as f32));
    });
    surface.add_controller(secondary);

    let motion = gtk::EventControllerMotion::new();
    let weak = Rc::downgrade(app);
    motion.connect_motion(move |_, x, y| with(&weak, |app| pointer_moved(app, x as f32, y as f32)));
    let weak = Rc::downgrade(app);
    motion.connect_leave(move |_| {
        with(&weak, |app| {
            app.pointer = None;
            if app.renderer.clear_hover() {
                app.redraw();
            }
        });
    });
    surface.add_controller(motion);

    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let weak = Rc::downgrade(app);
    scroll.connect_scroll(move |controller, _, dy| {
        let pixels = controller.unit() == gdk::ScrollUnit::Surface;
        with(&weak, |app| wheel(app, dy as f32, pixels));
        gtk::glib::Propagation::Stop
    });
    surface.add_controller(scroll);

    install_fullscreen_controls(app, &controls);
}

/// Runs `handler` on the app unless it is gone or already borrowed by a callback
/// higher up the stack (GTK can re-enter while a handler runs).
pub(super) fn with(weak: &std::rc::Weak<RefCell<App>>, handler: impl FnOnce(&mut App)) {
    if let Some(app) = weak.upgrade()
        && let Ok(mut app) = app.try_borrow_mut()
    {
        handler(&mut app);
    }
}

fn left_down(app: &mut App, x: f32, y: f32, extend: bool) {
    app.surface.grab_focus();
    media::reveal_fullscreen_controls(app);
    let hit = app.renderer.hit_test(x, y);
    close_floating_panels(app, hit.as_ref());
    app.clip_drag = if matches!(app.model.page, Page::Library | Page::Collections) {
        match hit.as_ref() {
            Some(Action::OpenClip(index) | Action::ToggleClipSelection(index)) => {
                Some(library::ClipDrag::new(*index, x, y))
            }
            _ => None,
        }
    } else {
        None
    };
    app.editor_drag = match hit.as_ref() {
        Some(Action::DragEditorStart) => Some(media::EditorDrag::Start),
        Some(Action::DragEditorEnd) => Some(media::EditorDrag::End),
        _ => None,
    };
    app.editor_drag_origin = app
        .editor_drag
        .map(|_| (app.model.editor_start, app.model.editor_end));
    app.slider_drag = match hit.as_ref() {
        Some(Action::DragDesktopGain) => Some(SliderDrag::DesktopGain),
        Some(Action::DragMicrophoneGain) => Some(SliderDrag::MicrophoneGain),
        Some(Action::DragPlayerSeek) => Some(SliderDrag::PlayerSeek),
        Some(Action::DragPlayerVolume) => Some(SliderDrag::PlayerVolume),
        Some(Action::DragEditorPlayhead) => Some(SliderDrag::EditorPlayhead),
        Some(Action::DragFolderDivider) => Some(SliderDrag::FolderColumn),
        _ => None,
    };
    app.text_drag = match hit.as_ref() {
        Some(Action::PlaceSearchCaret(position)) => {
            app.model.search_focused = true;
            app.model.search.move_caret(*position, extend);
            Some(TextDrag::Search)
        }
        Some(Action::PlacePromptCaret(position)) => {
            if let Some(prompt) = &mut app.model.prompt {
                prompt.input.move_caret(*position, extend);
            }
            Some(TextDrag::Prompt)
        }
        _ => None,
    };
    if app.editor_drag.is_some()
        || app.clip_drag.is_some()
        || app.slider_drag.is_some()
        || app.text_drag.is_some()
    {
        if app.editor_drag.is_some() {
            media::update_editor_drag(app, x, true);
        }
        if app.slider_drag.is_some() {
            update_slider_drag(app, x, y, true);
        }
        app.redraw();
    }
}

/// Popovers close on any click outside them, as on Windows.
fn close_floating_panels(app: &mut App, hit: Option<&Action>) {
    if app.model.collection_picker_open
        && !matches!(
            hit,
            Some(Action::MoveSelectedToCollection(_) | Action::ToggleCollectionPicker)
        )
    {
        app.model.collection_picker_open = false;
    }
    if app.model.filter_panel_open
        && !matches!(
            hit,
            Some(
                Action::Ignore
                    | Action::ToggleFilterPanel
                    | Action::ChooseTimeFilter
                    | Action::ChooseCollectionFilter
                    | Action::ChooseTypeFilter
                    | Action::ChooseSizeFilter
                    | Action::ChooseClipSort
                    | Action::ResetFilters
                    | Action::SelectSettingsOption(_)
                    | Action::DismissSettingsMenu
            )
        )
    {
        app.model.filter_panel_open = false;
    }
    if app.model.capture_panel_open
        && !matches!(
            hit,
            Some(
                Action::Ignore
                    | Action::ToggleCapturePanel
                    | Action::ChooseDuration
                    | Action::ChooseDisplay
                    | Action::ChooseQuality
                    | Action::ChooseAudioMode
                    | Action::ToggleMicrophoneTest
                    | Action::SelectSettingsOption(_)
                    | Action::DismissSettingsMenu
            )
        )
    {
        app.model.capture_panel_open = false;
    }
}

fn pointer_moved(app: &mut App, x: f32, y: f32) {
    app.pointer = Some((x, y));
    if app.fullscreen {
        media::reveal_fullscreen_controls(app);
    }
    let menu_highlight_changed = if let Some(Action::SelectSettingsOption(index)) =
        app.renderer.hit_test(x, y)
        && let Some(menu) = &mut app.model.settings_menu
        && menu.highlighted != index
    {
        menu.highlighted = index;
        true
    } else {
        false
    };
    let hover_changed = app.renderer.update_hover(x, y);
    if app.editor_drag.is_some() {
        media::update_editor_drag(app, x, false);
    }
    if app.clip_drag.is_some() {
        library::update_clip_drag(app, x, y);
    }
    if app.slider_drag.is_some() {
        update_slider_drag(app, x, y, false);
    }
    if app.text_drag.is_some() {
        update_text_drag(app, x, y);
    }
    update_cursor(app);
    if menu_highlight_changed
        || hover_changed
        || app.editor_drag.is_some()
        || app.clip_drag.is_some()
        || app.slider_drag.is_some()
        || app.text_drag.is_some()
    {
        app.redraw();
    }
}

fn update_cursor(app: &App) {
    let name = if matches!(app.slider_drag, Some(SliderDrag::FolderColumn))
        || app.renderer.hovered() == Some(&Action::DragFolderDivider)
    {
        Some("ew-resize")
    } else if app.renderer.hovered() == Some(&Action::Home) {
        Some("pointer")
    } else {
        None
    };
    app.surface.set_cursor_from_name(name);
}

fn left_up(app: &mut App, x: f32, y: f32) {
    if app.clip_drag.is_some() {
        library::finish_clip_drag(app, x, y);
        app.redraw();
    } else if app.editor_drag.is_some() {
        media::update_editor_drag(app, x, true);
        app.editor_drag = None;
        if let Some(previous) = app.editor_drag_origin.take() {
            app.model.commit_editor_trim_change(previous);
        }
        app.redraw();
    } else if app.slider_drag.is_some() {
        update_slider_drag(app, x, y, true);
        if matches!(app.slider_drag, Some(SliderDrag::FolderColumn)) {
            super::menus::persist_appearance(&mut app.model);
        }
        app.slider_drag = None;
        app.redraw();
    } else if app.text_drag.is_some() {
        update_text_drag(app, x, y);
        app.text_drag = None;
        app.redraw();
    } else if let Some(action) = app.renderer.hit_test(x, y) {
        super::actions::handle_action(app, action);
    }
}

fn right_up(app: &mut App, x: f32, y: f32) {
    if let Some(Action::OpenClip(index)) = app.renderer.hit_test(x, y) {
        app.model.context_menu = Some(ClipContextMenu { clip: index, x, y });
        app.redraw();
    }
}

fn wheel(app: &mut App, delta: f32, pixels: bool) {
    if !matches!(app.model.page, Page::Library | Page::Collections)
        || app.model.settings_menu.is_some()
        || app.model.context_menu.is_some()
    {
        return;
    }
    let pointer_x = app.pointer.map_or(0.0, |(x, _)| x);
    let over_folders = folder_column_contains(&app.model, pointer_x);
    let overflow = if over_folders {
        folder_column_overflow(&app.model, app.width, app.height)
    } else {
        clips_overflow(&app.model, app.width, app.height)
    };
    let step = if pixels {
        delta
    } else {
        delta * LIBRARY_WHEEL_STEP
    };
    let scrolled = if over_folders {
        app.model.scroll_folders_by(step, overflow)
    } else {
        app.model.scroll_library_by(step, overflow)
    };
    if scrolled {
        // the cards moved under a still pointer; hover follows on the next paint
        if let Some((x, y)) = app.pointer {
            app.renderer.update_hover(x, y);
        }
        app.redraw();
    }
}

pub(super) fn update_slider_drag(app: &mut App, x: f32, y: f32, settle: bool) {
    let Some(drag) = app.slider_drag else {
        return;
    };
    let (width, height) = (app.width as u32, app.height as u32);
    match drag {
        SliderDrag::DesktopGain => {
            let rail = settings_audio_gain_rail(&app.model, width, height, 2);
            app.model.config.audio.desktop_gain_percent = settings_gain_percent(rail, x);
        }
        SliderDrag::MicrophoneGain => {
            let rail = settings_audio_gain_rail(&app.model, width, height, 4);
            app.model.config.audio.microphone_gain_percent = settings_gain_percent(rail, x);
        }
        SliderDrag::PlayerSeek => {
            let rail = player_timeline_rail(&app.model, width, height);
            let fraction = ((x - rail.left) / (rail.right - rail.left).max(1.0)).clamp(0.0, 1.0);
            media::seek_player(app, f64::from(fraction), settle);
        }
        SliderDrag::PlayerVolume => {
            let rail = player_volume_rail(&app.model, width, height);
            let fraction =
                (1.0 - (y - rail.top) / (rail.bottom - rail.top).max(1.0)).clamp(0.0, 1.0);
            media::set_player_volume(app, (fraction * 100.0).round() as u8);
        }
        SliderDrag::EditorPlayhead => {
            let Some(timing) = &app.model.editor_timing else {
                return;
            };
            let rail = editor_timeline_rail(&app.model, width, height);
            let thousandths = editor_timeline_fraction(rail, x);
            let requested = timing.duration.mul_f64(f64::from(thousandths) / 1_000.0);
            let position = requested
                .max(app.model.editor_start)
                .min(app.model.editor_end);
            media::seek_editor_preview(app, position, settle);
        }
        SliderDrag::FolderColumn => {
            app.model.config.appearance.folder_width = Some(folder_width_at(&app.model, x));
        }
        SliderDrag::FullscreenSeek | SliderDrag::FullscreenVolume => {}
    }
}

fn update_text_drag(app: &mut App, x: f32, y: f32) {
    let Some(drag) = app.text_drag else {
        return;
    };
    match (drag, app.renderer.hit_test(x, y)) {
        (TextDrag::Search, Some(Action::PlaceSearchCaret(position))) => {
            app.model.search.move_caret(position, true);
        }
        (TextDrag::Prompt, Some(Action::PlacePromptCaret(position))) => {
            if let Some(prompt) = &mut app.model.prompt {
                prompt.input.move_caret(position, true);
            }
        }
        _ => {}
    }
}

/// The controls strip over the fullscreen video: its own renderer and hit regions.
fn install_fullscreen_controls(app: &Rc<RefCell<App>>, controls: &crate::surface::Surface) {
    let motion = gtk::EventControllerMotion::new();
    let weak = Rc::downgrade(app);
    motion.connect_motion(move |_, x, y| {
        with(&weak, |app| {
            app.pointer_over_controls = true;
            media::reveal_fullscreen_controls(app);
            let hover_changed = app.fullscreen_renderer.update_hover(x as f32, y as f32);
            if matches!(
                app.slider_drag,
                Some(SliderDrag::FullscreenSeek | SliderDrag::FullscreenVolume)
            ) {
                update_fullscreen_slider_drag(app, x as f32, false);
            }
            if hover_changed || app.slider_drag.is_some() {
                app.controls.queue_draw();
            }
        });
    });
    let weak = Rc::downgrade(app);
    motion.connect_leave(move |_| {
        with(&weak, |app| {
            app.pointer_over_controls = false;
            app.fullscreen_controls_until =
                Some(std::time::Instant::now() + super::FULLSCREEN_CONTROLS_LIFETIME);
            if app.fullscreen_renderer.clear_hover() {
                app.controls.queue_draw();
            }
        });
    });
    controls.add_controller(motion);

    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_PRIMARY);
    let weak = Rc::downgrade(app);
    click.connect_pressed(move |_, _, x, y| {
        with(&weak, |app| {
            media::reveal_fullscreen_controls(app);
            app.slider_drag = match app.fullscreen_renderer.hit_test(x as f32, y as f32) {
                Some(Action::DragPlayerSeek) => Some(SliderDrag::FullscreenSeek),
                Some(Action::DragPlayerVolume) => Some(SliderDrag::FullscreenVolume),
                _ => None,
            };
            if app.slider_drag.is_some() {
                update_fullscreen_slider_drag(app, x as f32, true);
                app.controls.queue_draw();
            }
        });
    });
    let weak = Rc::downgrade(app);
    click.connect_released(move |_, _, x, y| {
        with(&weak, |app| {
            if matches!(
                app.slider_drag,
                Some(SliderDrag::FullscreenSeek | SliderDrag::FullscreenVolume)
            ) {
                update_fullscreen_slider_drag(app, x as f32, true);
                app.slider_drag = None;
                app.controls.queue_draw();
            } else if let Some(action) = app.fullscreen_renderer.hit_test(x as f32, y as f32) {
                super::actions::handle_action(app, action);
                app.controls.queue_draw();
            }
        });
    });
    controls.add_controller(click);
}

fn update_fullscreen_slider_drag(app: &mut App, x: f32, settle: bool) {
    let width = app.width as u32;
    let height = super::FULLSCREEN_CONTROLS_HEIGHT as u32;
    match app.slider_drag {
        Some(SliderDrag::FullscreenSeek) => {
            let rail =
                fullscreen_timeline_rail(&app.model, width, app.height as u32, height as f32);
            let fraction = ((x - rail.left) / (rail.right - rail.left).max(1.0)).clamp(0.0, 1.0);
            media::seek_player(app, f64::from(fraction), settle);
        }
        Some(SliderDrag::FullscreenVolume) => {
            let rail = fullscreen_volume_rail(width, height);
            let fraction = ((x - rail.left) / (rail.right - rail.left).max(1.0)).clamp(0.0, 1.0);
            media::set_player_volume(app, (fraction * 100.0).round() as u8);
        }
        _ => {}
    }
    media::reveal_fullscreen_controls(app);
}
