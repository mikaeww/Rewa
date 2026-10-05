//! Keyboard input: menu navigation, the search and prompt text fields (through an
//! input method, so compose keys and dead keys work), shortcut capture and the
//! window-wide keys of the Windows application.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gtk::gdk;
use gtk::glib::Propagation;
use gtk::prelude::*;

use super::input::with;
use super::{App, daemon, media};
use crate::model::{Action, Page, TextInput};

pub(super) fn install(app: &Rc<RefCell<App>>) {
    let surface = app.borrow().surface.clone();
    let input_method = gtk::IMMulticontext::new();
    input_method.set_client_widget(Some(&surface));
    let weak = Rc::downgrade(app);
    input_method.connect_commit(move |_, text| {
        with(&weak, |app| {
            if let Some(input) = active_input(app) {
                input.insert_text(text);
                app.redraw();
            }
        });
    });

    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(app);
    let method = input_method.clone();
    keys.connect_key_pressed(move |controller, key, _, state| {
        let mut handled = false;
        let mut forward = false;
        with(&weak, |app| match key_pressed(app, &weak, key, state) {
            Some(stop) => handled = stop,
            None => forward = active_input(app).is_some(),
        });
        if handled {
            return Propagation::Stop;
        }
        if forward
            && let Some(event) = controller.current_event()
            && method.filter_keypress(&event)
        {
            return Propagation::Stop;
        }
        Propagation::Proceed
    });
    let weak = Rc::downgrade(app);
    keys.connect_key_released(move |_, key, _, state| {
        with(&weak, |app| {
            if app.model.hotkey_capture && !is_modifier(key) {
                return;
            }
            if app.model.hotkey_capture {
                app.model.hotkey_modifiers = modifiers(key, state, false);
                app.redraw();
            }
        });
    });
    surface.add_controller(keys);

    let focus = gtk::EventControllerFocus::new();
    let weak = Rc::downgrade(app);
    let method = input_method.clone();
    focus.connect_enter(move |_| method.focus_in());
    let method = input_method;
    focus.connect_leave(move |_| {
        method.focus_out();
        with(&weak, |app| {
            if app.model.hotkey_capture {
                daemon::cancel_hotkey_capture(app);
                app.redraw();
            }
        });
    });
    surface.add_controller(focus);
}

fn active_input(app: &mut App) -> Option<&mut TextInput> {
    if app.model.settings_menu.is_some() || app.model.hotkey_capture {
        return None;
    }
    if let Some(prompt) = &mut app.model.prompt {
        return Some(&mut prompt.input);
    }
    app.model.search_focused.then_some(&mut app.model.search)
}

/// `Some(true)` when the key was used, `Some(false)` when it falls through to GTK,
/// `None` when a text field should see it as typed text.
fn key_pressed(
    app: &mut App,
    weak: &Weak<RefCell<App>>,
    key: gdk::Key,
    state: gdk::ModifierType,
) -> Option<bool> {
    let extend = state.contains(gdk::ModifierType::SHIFT_MASK);
    let control = state.contains(gdk::ModifierType::CONTROL_MASK);
    if app.model.settings_menu.is_some() {
        menu_key(app, key);
        return Some(true);
    }
    if app.model.prompt.is_some() {
        if matches!(key, gdk::Key::Return | gdk::Key::KP_Enter) {
            super::actions::handle_action(app, Action::ConfirmPrompt);
            return Some(true);
        }
        if key == gdk::Key::Escape {
            super::actions::handle_action(app, Action::CancelPrompt);
            return Some(true);
        }
        return text_key(app, weak, key, control, extend).then_some(true);
    }
    if app.model.search_focused {
        if matches!(
            key,
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::Escape
        ) {
            app.model.search_focused = false;
            app.redraw();
            return Some(true);
        }
        return text_key(app, weak, key, control, extend).then_some(true);
    }
    if app.model.hotkey_capture {
        capture_hotkey(app, key, state);
        app.redraw();
        return Some(true);
    }
    Some(window_key(app, key, control))
}

fn menu_key(app: &mut App, key: gdk::Key) {
    let mut select = None;
    if key == gdk::Key::Escape {
        app.model.settings_menu = None;
    } else if let Some(menu) = &mut app.model.settings_menu {
        match key {
            gdk::Key::Up => menu.move_highlight(-1),
            gdk::Key::Down => menu.move_highlight(1),
            gdk::Key::Home => menu.highlighted = 0,
            gdk::Key::End => menu.highlighted = menu.items.len().saturating_sub(1),
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::space => {
                select = Some(menu.highlighted);
            }
            _ => {}
        }
    }
    match select {
        Some(index) => super::actions::handle_action(app, Action::SelectSettingsOption(index)),
        None => app.redraw(),
    }
}

/// Caret, deletion and clipboard keys of a text field; typed text arrives through
/// the input method instead.
fn text_key(
    app: &mut App,
    weak: &Weak<RefCell<App>>,
    key: gdk::Key,
    control: bool,
    extend: bool,
) -> bool {
    let clipboard = app.surface.clipboard();
    let Some(input) = active_input(app) else {
        return false;
    };
    if control {
        match key.to_lower() {
            gdk::Key::a => input.select_all(),
            gdk::Key::c => {
                let selected = input.selected_text();
                if !selected.is_empty() {
                    clipboard.set_text(&selected);
                }
            }
            gdk::Key::x => {
                let selected = input.selected_text();
                if !selected.is_empty() {
                    clipboard.set_text(&selected);
                    input.delete();
                }
            }
            gdk::Key::v => {
                let weak = weak.clone();
                clipboard.read_text_async(None::<&gtk::gio::Cancellable>, move |text| {
                    if let Ok(Some(text)) = text {
                        with(&weak, |app| {
                            if let Some(input) = active_input(app) {
                                input.insert_text(&text);
                                app.redraw();
                            }
                        });
                    }
                });
            }
            _ => return false,
        }
    } else {
        match key {
            gdk::Key::Left | gdk::Key::KP_Left => input.caret_left(extend),
            gdk::Key::Right | gdk::Key::KP_Right => input.caret_right(extend),
            gdk::Key::Home | gdk::Key::KP_Home => input.caret_home(extend),
            gdk::Key::End | gdk::Key::KP_End => input.caret_end(extend),
            gdk::Key::Delete | gdk::Key::KP_Delete => input.delete(),
            gdk::Key::BackSpace => input.backspace(),
            _ => return false,
        }
    }
    app.redraw();
    true
}

fn window_key(app: &mut App, key: gdk::Key, control: bool) -> bool {
    match key {
        gdk::Key::k | gdk::Key::K if control => {
            super::actions::handle_action(app, Action::Search);
        }
        gdk::Key::space => {
            if app.model.prompt.is_none()
                && matches!(app.model.page, Page::Player | Page::Editor)
                && let Err(error) = app.player.toggle()
            {
                app.model.notice = Some(error);
            }
            app.redraw();
        }
        gdk::Key::F5 => super::actions::handle_action(app, Action::Refresh),
        gdk::Key::F11 => {
            if app.model.page == Page::Player {
                media::toggle_player_fullscreen(app);
            }
            app.redraw();
        }
        gdk::Key::Escape => escape(app),
        _ => return false,
    }
    true
}

/// Escape closes the innermost thing that is open, one per press.
fn escape(app: &mut App) {
    app.redraw();
    if app.fullscreen {
        media::exit_player_fullscreen(app);
        return;
    }
    if std::mem::take(&mut app.model.collection_picker_open)
        || app.model.close_filter_panel()
        || std::mem::take(&mut app.model.capture_panel_open)
    {
        return;
    }
    if app.model.selection_mode {
        app.model.clear_clip_selection();
        return;
    }
    app.model.search_focused = false;
    app.model.hotkey_capture = false;
    app.model.notice = None;
    app.model.pending_delete = None;
    app.model.prompt = None;
}

fn is_modifier(key: gdk::Key) -> bool {
    matches!(
        key,
        gdk::Key::Super_L
            | gdk::Key::Super_R
            | gdk::Key::Meta_L
            | gdk::Key::Meta_R
            | gdk::Key::Hyper_L
            | gdk::Key::Hyper_R
            | gdk::Key::Shift_L
            | gdk::Key::Shift_R
            | gdk::Key::Control_L
            | gdk::Key::Control_R
            | gdk::Key::Alt_L
            | gdk::Key::Alt_R
    )
}

/// The held modifiers in the config's order. GDK reports the state before the
/// event, so the key itself counts while pressed and drops out when released.
fn modifiers(key: gdk::Key, state: gdk::ModifierType, pressed: bool) -> Vec<String> {
    let held = |mask: gdk::ModifierType, keys: &[gdk::Key]| {
        if keys.contains(&key) {
            pressed
        } else {
            state.intersects(mask)
        }
    };
    [
        (
            held(
                gdk::ModifierType::SUPER_MASK
                    | gdk::ModifierType::META_MASK
                    | gdk::ModifierType::HYPER_MASK,
                &[
                    gdk::Key::Super_L,
                    gdk::Key::Super_R,
                    gdk::Key::Meta_L,
                    gdk::Key::Meta_R,
                    gdk::Key::Hyper_L,
                    gdk::Key::Hyper_R,
                ],
            ),
            "SUPER",
        ),
        (
            held(
                gdk::ModifierType::CONTROL_MASK,
                &[gdk::Key::Control_L, gdk::Key::Control_R],
            ),
            "CTRL",
        ),
        (
            held(
                gdk::ModifierType::ALT_MASK,
                &[gdk::Key::Alt_L, gdk::Key::Alt_R],
            ),
            "ALT",
        ),
        (
            held(
                gdk::ModifierType::SHIFT_MASK,
                &[gdk::Key::Shift_L, gdk::Key::Shift_R],
            ),
            "SHIFT",
        ),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .map(|(_, name)| name.to_owned())
    .collect()
}

fn capture_hotkey(app: &mut App, key: gdk::Key, state: gdk::ModifierType) {
    if key == gdk::Key::Escape {
        daemon::cancel_hotkey_capture(app);
        app.model.hotkey_error = None;
        app.model.notice = None;
        return;
    }
    let held = modifiers(key, state, true);
    if is_modifier(key) {
        app.model.hotkey_modifiers = held;
        return;
    }
    app.model.hotkey_modifiers = held.clone();
    let Some(name) = key.to_upper().name() else {
        app.model.hotkey_error = Some(app.model.strings().hotkey_key_unusable.to_owned());
        return;
    };
    let expression = held
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(name.as_str()))
        .collect::<Vec<_>>()
        .join("+");
    match rewa_core::config::HotkeyConfig::parse(&expression) {
        Ok(hotkey) if allowed(&hotkey) => daemon::begin_hotkey_update(app, hotkey),
        Ok(_) => app.model.hotkey_error = Some(app.model.strings().hotkey_rule.to_owned()),
        Err(_) => {
            app.model.hotkey_error = Some(app.model.strings().hotkey_key_unusable.to_owned());
        }
    }
    app.model.notice = None;
}

/// A bare key would fire while typing anywhere, so it needs a modifier unless it
/// is a function key or Print, which nothing types.
fn allowed(hotkey: &rewa_core::config::HotkeyConfig) -> bool {
    let standalone = hotkey.key == "PRINT"
        || hotkey
            .key
            .strip_prefix('F')
            .and_then(|number| number.parse::<u8>().ok())
            .is_some_and(|number| (1..=24).contains(&number));
    !hotkey.modifiers.is_empty() || standalone
}

#[cfg(test)]
mod tests {
    use super::{allowed, modifiers};
    use gtk::gdk;
    use rewa_core::config::HotkeyConfig;

    #[test]
    fn a_shortcut_needs_a_modifier_unless_nothing_types_its_key() {
        assert!(allowed(
            &HotkeyConfig::parse("SUPER+SHIFT+R").expect("valid")
        ));
        assert!(allowed(&HotkeyConfig::parse("F9").expect("valid")));
        assert!(allowed(&HotkeyConfig::parse("PRINT").expect("valid")));
        assert!(!allowed(&HotkeyConfig::parse("R").expect("valid")));
    }

    #[test]
    fn the_pressed_modifier_counts_and_the_released_one_does_not() {
        let held = modifiers(gdk::Key::Shift_L, gdk::ModifierType::SUPER_MASK, true);
        assert_eq!(held, ["SUPER", "SHIFT"]);
        let released = modifiers(
            gdk::Key::Shift_L,
            gdk::ModifierType::SUPER_MASK | gdk::ModifierType::SHIFT_MASK,
            false,
        );
        assert_eq!(released, ["SUPER"]);
    }
}
