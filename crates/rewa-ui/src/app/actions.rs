//! What a click or key does: the Windows `handle_action`, with the window, player
//! and shell calls swapped for their GTK and Linux counterparts.

use std::time::Instant;

use gtk::prelude::*;

use super::{App, daemon, library, media, menus, system};
use crate::model::{Action, ClipContextMenu, ClipTab, DeleteTarget, Page, SettingsSection};

pub(crate) fn handle_action(app: &mut App, action: Action) {
    app.model.notice = None;
    match action {
        Action::Navigate(page) => navigate(app, page),
        Action::Home => {
            handle_action(app, Action::Navigate(Page::Library));
            app.model.search.clear();
            app.model.set_clip_tab(ClipTab::All);
            app.model.library_scroll = 0.0;
        }
        Action::SettingsSection(section) => {
            if app.model.page != Page::Settings {
                app.model.navigate(Page::Settings);
                app.model.autostart_enabled = system::autostart_enabled();
            }
            app.model.settings_menu = None;
            daemon::cancel_hotkey_capture(app);
            app.model.hotkey_error = None;
            app.model.settings_section = section;
        }
        Action::OpenClip(index) => {
            app.model.context_menu = None;
            app.model.open_clip(index);
            media::open_current_clip(app);
        }
        Action::OpenClipMenu(index) => {
            if let Some((x, y)) = app.pointer {
                app.model.context_menu = Some(ClipContextMenu { clip: index, x, y });
            }
        }
        Action::EditClip(index) => {
            app.model.context_menu = None;
            app.model.open_clip(index);
            media::open_current_clip(app);
            media::begin_editor(app);
        }
        Action::RenameClip(index) => {
            app.model.context_menu = None;
            app.model.begin_rename(index);
        }
        Action::RenameActiveClip => {
            app.model.context_menu = None;
            if let Some(index) = app.model.active_clip {
                app.model.begin_rename(index);
            }
        }
        Action::DeleteClip(index) => {
            media::exit_player_fullscreen(app);
            app.model.context_menu = None;
            app.model.pending_delete = Some(DeleteTarget::Clip(index));
        }
        Action::MoveClipToCollection { clip, collection } => {
            app.model.context_menu = None;
            match app.model.clips.get(clip).map(|clip| clip.path.clone()) {
                Some(path) => library::move_clip_paths_to_collection(app, &[path], collection),
                None => app.model.notice = Some(app.model.strings().notice_clip_gone.to_owned()),
            }
        }
        Action::ToggleSelectionMode => {
            app.model.context_menu = None;
            app.model.toggle_selection_mode();
        }
        Action::ToggleClipSelection(index) => {
            app.model.toggle_clip_selection(index);
        }
        Action::SelectAllVisibleClips => app.model.select_all_visible_clips(),
        Action::ToggleCollectionPicker => {
            if !app.model.selected_clips.is_empty() && !app.model.collections.is_empty() {
                app.model.collection_picker_open = !app.model.collection_picker_open;
            }
        }
        Action::MoveSelectedToCollection(collection) => {
            let paths = app.model.selected_clips.iter().cloned().collect::<Vec<_>>();
            library::move_clip_paths_to_collection(app, &paths, collection);
        }
        Action::DismissContextMenu => app.model.context_menu = None,
        Action::Back => {
            media::exit_player_fullscreen(app);
            media::stop_player(app);
            let previous = app.model.previous_page;
            app.model.navigate(previous);
            app.renderer.retry_unavailable_thumbnails();
        }
        Action::Refresh => {
            let result = app.model.refresh();
            if result.is_ok() {
                app.renderer.retry_unavailable_thumbnails();
            }
            let message = app.model.strings().notice_library_refreshed;
            daemon::set_result(&mut app.model, result, message);
        }
        Action::SetLibraryView(view) => {
            app.model.config.appearance.library_view = view;
            app.model.library_scroll = 0.0;
            menus::persist_appearance(&mut app.model);
        }
        Action::SetClipTab(tab) => app.model.set_clip_tab(tab),
        Action::ToggleFilterPanel => app.model.filter_panel_open = !app.model.filter_panel_open,
        Action::ToggleCapturePanel => {
            app.model.capture_panel_open = !app.model.capture_panel_open;
        }
        Action::ToggleSidebar => app.model.toggle_sidebar(),
        Action::ToggleMicrophoneTest => {
            app.model.toggle_microphone_test();
            if app.model.microphone_test {
                system::start_microphone_test(app);
            } else {
                app.microphone = None;
            }
            app.microphone_due = Instant::now();
        }
        Action::ChooseTheme => menus::choose_theme(&mut app.model),
        Action::ChooseLanguage => menus::choose_language(&mut app.model),
        Action::ChooseHoverStyle => menus::choose_hover_style(&mut app.model),
        Action::ChooseHoverStrength => menus::choose_hover_strength(&mut app.model),
        Action::ChooseTimeFilter => menus::choose_time_filter(&mut app.model),
        Action::ChooseCollectionFilter => menus::choose_collection_filter(&mut app.model),
        Action::ChooseTypeFilter => menus::choose_type_filter(&mut app.model),
        Action::ChooseSizeFilter => menus::choose_size_filter(&mut app.model),
        Action::ChooseClipSort => menus::choose_clip_sort(&mut app.model),
        Action::ResetFilters => app.model.reset_filters(),
        Action::ToggleFavorite(index) => {
            app.model.context_menu = None;
            if let Err(error) = app.model.toggle_favorite(index) {
                app.model.notice = Some(error);
            }
        }
        Action::OpenClipExternally(index) | Action::ShowClipInExplorer(index) => {
            app.model.context_menu = None;
            match app.model.clips.get(index).map(|clip| clip.path.clone()) {
                Some(path) if matches!(action, Action::ShowClipInExplorer(_)) => {
                    system::show_in_file_manager(&app.window, &path);
                }
                Some(path) => system::open_path(&app.window, &path),
                None => app.model.notice = Some(app.model.strings().notice_clip_gone.to_owned()),
            }
        }
        Action::SaveReplay => daemon::start_replay_save(app),
        Action::OpenClipsFolder => {
            let directory = app.model.config.storage.directory.clone();
            system::open_path(&app.window, &directory);
        }
        Action::Search => {
            if !matches!(app.model.page, Page::Library | Page::Collections) {
                media::exit_player_fullscreen(app);
                media::stop_player(app);
                app.model.navigate(Page::Library);
            }
            app.model.search_focused = true;
            app.model.search.select_all();
        }
        Action::ClearSearch => {
            app.model.search.clear();
            app.model.active_collection = None;
            app.model.active_game = None;
        }
        Action::Ignore | Action::PlaceSearchCaret(_) | Action::PlacePromptCaret(_) => {}
        Action::DismissNotice => app.model.notice = None,
        Action::MinimizeWindow => app.window.minimize(),
        Action::ToggleMaximizeWindow => {
            if app.window.is_maximized() {
                app.window.unmaximize();
            } else {
                app.window.maximize();
            }
        }
        Action::CloseWindow => app.window.close(),
        Action::ToggleAutostart => {
            let enabled = !app.model.autostart_enabled;
            match system::set_autostart(enabled) {
                Ok(()) => app.model.autostart_enabled = enabled,
                Err(error) => app.model.notice = Some(error),
            }
        }
        Action::ToggleCursor => app.model.config.capture.cursor = !app.model.config.capture.cursor,
        Action::ToggleDesktopAudio => {
            app.model.config.audio.desktop = !app.model.config.audio.desktop;
        }
        Action::ChooseDesktopDevice => menus::choose_desktop_device(&mut app.model),
        Action::ChooseDesktopGain => menus::choose_desktop_gain(&mut app.model),
        Action::ChooseAudioMode => menus::choose_audio_mode(&mut app.model),
        Action::ToggleMicrophone => {
            app.model.config.audio.microphone = !app.model.config.audio.microphone;
        }
        Action::ChooseDuration => menus::choose_duration(&mut app.model),
        Action::ChooseFrameRate => menus::choose_frame_rate(&mut app.model),
        Action::ChooseCodec => menus::choose_codec(&mut app.model),
        Action::ChooseQuality => menus::choose_quality(&mut app.model),
        Action::ChooseDisplay => menus::choose_display(&mut app.model),
        Action::ChooseMicrophone => menus::choose_microphone(&mut app.model),
        Action::ChooseMicrophoneGain => menus::choose_microphone_gain(&mut app.model),
        Action::ChooseStorageLimit => menus::choose_storage_limit(&mut app.model),
        Action::DismissSettingsMenu => app.model.settings_menu = None,
        Action::SelectSettingsOption(index) => menus::select_settings_option(&mut app.model, index),
        Action::CaptureHotkey => {
            if !app.model.hotkey_pending {
                app.model.hotkey_capture = true;
                app.model.hotkey_modifiers.clear();
                app.model.hotkey_error = None;
                app.model.notice = None;
            }
        }
        Action::ClearHotkey => {
            if !app.model.hotkey_pending {
                daemon::cancel_hotkey_capture(app);
                daemon::begin_hotkey_update(app, rewa_core::config::HotkeyConfig::unbound());
            }
        }
        Action::ChooseStorage => system::choose_storage(app),
        Action::SaveSettings => {
            let message = app.model.strings().notice_settings_saved;
            daemon::save_settings(&mut app.model, message);
        }
        Action::CreateCollection => app.model.begin_new_collection(),
        Action::CancelPrompt => app.model.prompt = None,
        Action::ConfirmPrompt => library::confirm_prompt(app),
        Action::DeleteActiveCollection => {
            if let Some(collection) = app.model.active_collection.clone() {
                app.model.pending_delete = Some(DeleteTarget::Collection(collection));
            }
        }
        Action::RenameActiveCollection => {
            app.model.begin_rename_collection();
        }
        Action::CancelDelete => app.model.pending_delete = None,
        Action::ConfirmDelete => confirm_delete(app),
        Action::ToggleFolderColumn => {
            let appearance = &mut app.model.config.appearance;
            appearance.folders_collapsed = !appearance.folders_collapsed;
            app.model.folder_scroll = 0.0;
            menus::persist_appearance(&mut app.model);
        }
        Action::DragFolderDivider => {}
        Action::SelectGame(index) => {
            app.model.active_collection = None;
            app.model.active_game = app.model.games().into_iter().nth(index);
            app.model.selected_clips.clear();
            app.model.collection_picker_open = false;
            app.model.library_scroll = 0.0;
        }
        Action::SelectCollection(index) => {
            app.model.active_game = None;
            app.model.active_collection = index
                .and_then(|index| app.model.collections.get(index))
                .map(|collection| collection.path.clone());
            app.model.selected_clips.clear();
            app.model.collection_picker_open = false;
            app.model.library_scroll = 0.0;
        }
        Action::PreviousClip => media::switch_clip(app, -1),
        Action::NextClip => media::switch_clip(app, 1),
        Action::PlayPause => {
            if let Err(error) = app.player.toggle() {
                app.model.notice = Some(error);
            }
        }
        Action::DragDesktopGain
        | Action::DragMicrophoneGain
        | Action::DragPlayerSeek
        | Action::DragPlayerVolume
        | Action::DragEditorPlayhead
        | Action::DragEditorStart
        | Action::DragEditorEnd => {}
        Action::ToggleMute => media::toggle_player_mute(app),
        Action::ToggleFullscreen => media::toggle_player_fullscreen(app),
        Action::EditActiveClip => {
            media::exit_player_fullscreen(app);
            media::begin_editor(app);
        }
        Action::SetTrimReplace(replace) => app.model.trim_replace_original = replace,
        Action::UndoEditorTrim => {
            if app.model.undo_editor_trim() {
                let start = app.model.editor_start;
                media::seek_editor_preview(app, start, true);
            }
        }
        Action::RedoEditorTrim => {
            if app.model.redo_editor_trim() {
                let start = app.model.editor_start;
                media::seek_editor_preview(app, start, true);
            }
        }
        Action::ResetEditorTrim => app.model.reset_editor_trim(),
        Action::SetEditorStartToPlayhead => app.model.set_editor_start_to_playhead(),
        Action::SetEditorEndToPlayhead => app.model.set_editor_end_to_playhead(),
        Action::SaveCut => media::save_cut(app, rewa_core::trim::TrimOutput::NewClip(None)),
        Action::ReplaceCut => media::save_cut(app, rewa_core::trim::TrimOutput::Replace),
    }
    app.redraw();
}

fn navigate(app: &mut App, page: Page) {
    if matches!(app.model.page, Page::Player | Page::Editor)
        && !matches!(page, Page::Player | Page::Editor)
    {
        media::exit_player_fullscreen(app);
        media::stop_player(app);
    }
    if page == Page::Settings {
        app.model.autostart_enabled = system::autostart_enabled();
        app.model.settings_section = SettingsSection::General;
    }
    app.model.navigate(page);
    if matches!(page, Page::Library | Page::Collections) {
        app.renderer.retry_unavailable_thumbnails();
    }
}

fn confirm_delete(app: &mut App) {
    let open_clip = matches!(app.model.page, Page::Player | Page::Editor)
        && matches!(
            app.model.pending_delete,
            Some(DeleteTarget::Clip(index)) if app.model.active_clip == Some(index)
        );
    if open_clip {
        // the player holds the file open; let go of it before the file goes
        media::stop_player(app);
    }
    let deleted = library::confirm_delete(&mut app.model);
    if open_clip {
        if deleted {
            handle_action(app, Action::Home);
        } else {
            media::open_current_clip(app);
        }
    }
}
