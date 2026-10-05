//! The window and its state: one surface the renderer paints, the fullscreen
//! controls above it, a 30 fps poll for background work and a frame clock that
//! only runs while something moves. Mirrors `rewa-win-ui/src/app.rs`.

mod actions;
mod daemon;
mod input;
mod keys;
mod library;
mod media;
mod menus;
mod system;

use std::cell::RefCell;
use std::process::ExitCode;
use std::rc::{Rc, Weak};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};

use crate::model::{DaemonSnapshot, NoticeExpiry, Page, UiModel};
use crate::player::Player;
use crate::render::{Renderer, fullscreen_picture};
use crate::surface::Surface;

const APP_ID: &str = "io.github.mikaeww.Rewa";
const POLL_INTERVAL: Duration = Duration::from_millis(33);
const NOTICE_LIFETIME: Duration = Duration::from_secs(3);
const FULLSCREEN_CONTROLS_LIFETIME: Duration = Duration::from_secs(3);
const FULLSCREEN_CONTROLS_HEIGHT: f32 = 84.0;

pub(crate) struct App {
    /// For callbacks that finish later, such as the folder picker.
    this: Weak<RefCell<App>>,
    model: UiModel,
    renderer: Renderer,
    fullscreen_renderer: Renderer,
    player: Player,
    window: gtk::ApplicationWindow,
    surface: Surface,
    controls: Surface,
    width: f32,
    height: f32,
    pointer: Option<(f32, f32)>,
    /// The device of the last press, which starts a drag out of the window.
    pointer_device: Option<gdk::Device>,
    outgoing_drag: Option<gdk::Drag>,
    text_drag: Option<input::TextDrag>,
    clip_drag: Option<library::ClipDrag>,
    slider_drag: Option<input::SliderDrag>,
    editor_drag: Option<media::EditorDrag>,
    editor_drag_origin: Option<(Duration, Duration)>,
    trim_updates: mpsc::Receiver<media::TrimUpdate>,
    trim_sender: mpsc::Sender<media::TrimUpdate>,
    hotkey_updates: mpsc::Receiver<daemon::HotkeyUpdate>,
    hotkey_sender: mpsc::Sender<daemon::HotkeyUpdate>,
    status_updates: mpsc::Receiver<DaemonSnapshot>,
    status_sender: mpsc::Sender<DaemonSnapshot>,
    status_pending: bool,
    status_due: Instant,
    replay_updates: Option<mpsc::Receiver<Result<rewa_core::ipc::Response, String>>>,
    microphone: Option<system::MicrophoneProbe>,
    microphone_due: Instant,
    player_seek: Option<Instant>,
    preview_seek: Option<Instant>,
    fullscreen: bool,
    fullscreen_controls_until: Option<Instant>,
    fullscreen_controls_visible: bool,
    pointer_over_controls: bool,
    notice_expiry: NoticeExpiry,
    frame_clock: Option<gtk::TickCallbackId>,
}

pub fn run() -> ExitCode {
    let flags = if std::env::var_os("REWA_UI_NON_UNIQUE").is_some() {
        gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::empty()
    };
    let application = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(flags)
        .build();
    application.connect_activate(|application| {
        if let Some(window) = application.active_window() {
            window.present();
            return;
        }
        if let Err(error) = build(application) {
            eprintln!("rewa-ui: {error}");
            application.quit();
        }
    });
    let status = application.run();
    if status == glib::ExitCode::SUCCESS {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn build(application: &gtk::Application) -> Result<(), String> {
    let mut model = UiModel::load()?;
    load_machine(&mut model);

    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Rewa")
        .default_width(1440)
        .default_height(900)
        .build();
    window.set_size_request(980, 680);
    let surface = Surface::new();
    let controls = Surface::new();
    controls.set_valign(gtk::Align::End);
    controls.set_vexpand(false);
    controls.set_size_request(-1, FULLSCREEN_CONTROLS_HEIGHT as i32);
    controls.set_visible(false);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&surface));
    overlay.add_overlay(&controls);
    window.set_child(Some(&overlay));

    let settings = gtk::Settings::default();
    let family = interface_family();
    let reduced_motion = settings
        .as_ref()
        .is_some_and(|settings| !settings.is_gtk_enable_animations());
    let thumbnails = model.paths.thumbnail_dir.clone();
    let renderer = Renderer::new(
        surface.pango_context(),
        &family,
        thumbnails.clone(),
        reduced_motion,
    );
    let fullscreen_renderer = Renderer::new(
        controls.pango_context(),
        &family,
        thumbnails,
        reduced_motion,
    );

    let (trim_sender, trim_updates) = mpsc::channel();
    let (hotkey_sender, hotkey_updates) = mpsc::channel();
    let (status_sender, status_updates) = mpsc::channel();
    let app = Rc::new(RefCell::new(App {
        this: Weak::new(),
        model,
        renderer,
        fullscreen_renderer,
        player: Player::new(),
        window: window.clone(),
        surface: surface.clone(),
        controls: controls.clone(),
        width: 1440.0,
        height: 900.0,
        pointer: None,
        pointer_device: None,
        outgoing_drag: None,
        text_drag: None,
        clip_drag: None,
        slider_drag: None,
        editor_drag: None,
        editor_drag_origin: None,
        trim_updates,
        trim_sender,
        hotkey_updates,
        hotkey_sender,
        status_updates,
        status_sender,
        status_pending: false,
        status_due: Instant::now(),
        replay_updates: None,
        microphone: None,
        microphone_due: Instant::now(),
        player_seek: None,
        preview_seek: None,
        fullscreen: false,
        fullscreen_controls_until: None,
        fullscreen_controls_visible: false,
        pointer_over_controls: false,
        notice_expiry: NoticeExpiry::default(),
        frame_clock: None,
    }));
    app.borrow_mut().this = Rc::downgrade(&app);
    install_painters(&app);
    install_fullscreen_watch(&app);
    input::install(&app);
    keys::install(&app);
    install_poll(&app);
    install_video_redraw(&app);
    // callbacks hold the state weakly; the window keeps it alive until it closes
    let keep = RefCell::new(Some(app.clone()));
    window.connect_close_request(move |_| {
        keep.take();
        glib::Propagation::Proceed
    });
    window.present();
    surface.grab_focus();
    Ok(())
}

/// What the settings and capture popover show about this computer: displays,
/// audio devices and whether the recorder starts at sign-in.
pub(crate) fn load_machine(model: &mut UiModel) {
    model.autostart_enabled = system::autostart_enabled();
    system::refresh_displays(model);
    system::refresh_microphones(model);
    system::refresh_outputs(model);
}

/// The desktop's interface font, as GTK applications use it; Windows draws in
/// its system font, Segoe UI Variable.
pub(crate) fn interface_family() -> String {
    gtk::Settings::default()
        .and_then(|settings| settings.gtk_font_name())
        .map(|name| pango::FontDescription::from_string(&name))
        .and_then(|description| description.family().map(|family| family.to_string()))
        .unwrap_or_else(|| "sans-serif".to_owned())
}

fn install_painters(app: &Rc<RefCell<App>>) {
    let weak = Rc::downgrade(app);
    let surface = app.borrow().surface.clone();
    surface.set_painter(move |snapshot, width, height| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let Ok(mut app) = app.try_borrow_mut() else {
            return;
        };
        app.width = width.max(1.0);
        app.height = height.max(1.0);
        let fullscreen = app.fullscreen;
        let App {
            renderer, model, ..
        } = &mut *app;
        if let Err(error) = renderer.paint(snapshot, model, width, height, fullscreen) {
            model.notice = Some(format!("{}: {error}", model.strings().notice_render_failed));
        }
        app.keep_frame_clock();
    });
    let weak = Rc::downgrade(app);
    let controls = app.borrow().controls.clone();
    controls.set_painter(move |snapshot, width, height| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let Ok(mut app) = app.try_borrow_mut() else {
            return;
        };
        let picture = fullscreen_picture(&app.model, app.width as u32, app.height as u32);
        let App {
            fullscreen_renderer,
            model,
            ..
        } = &mut *app;
        let _ =
            fullscreen_renderer.paint_fullscreen_controls(snapshot, model, width, height, picture);
    });
}

/// The compositor can end fullscreen on its own; the player then follows.
fn install_fullscreen_watch(app: &Rc<RefCell<App>>) {
    let weak = Rc::downgrade(app);
    let window = app.borrow().window.clone();
    window.connect_fullscreened_notify(move |window| {
        let fullscreened = window.is_fullscreen();
        input::with(&weak, |app| {
            if app.fullscreen && !fullscreened {
                media::exit_player_fullscreen(app);
            }
        });
    });
}

/// The background poll of the Windows window timer: workers, recorder state,
/// the microphone meter, notice expiry and the playing clip.
fn install_poll(app: &Rc<RefCell<App>>) {
    let weak = Rc::downgrade(app);
    glib::timeout_add_local(POLL_INTERVAL, move || {
        let Some(app) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if let Ok(mut app) = app.try_borrow_mut() {
            app.poll();
        }
        glib::ControlFlow::Continue
    });
}

/// New video frames repaint the surface, as the Windows child window repaints itself.
fn install_video_redraw(app: &Rc<RefCell<App>>) {
    let app = app.borrow();
    let surface = app.surface.downgrade();
    let controls = app.controls.downgrade();
    app.player.media().connect_invalidate_contents(move |_| {
        if let Some(surface) = surface.upgrade() {
            surface.queue_draw();
        }
    });
    app.player.media().connect_notify_local(None, move |_, _| {
        if let Some(controls) = controls.upgrade() {
            controls.queue_draw();
        }
    });
}

impl App {
    pub(crate) fn redraw(&self) {
        self.surface.queue_draw();
        if self.fullscreen_controls_visible {
            self.controls.queue_draw();
        }
    }

    fn poll(&mut self) {
        let trim_changed = media::poll_trim_updates(self);
        let replay_changed = daemon::poll_replay_save(self);
        let settings_changed = daemon::poll_settings_reload(&mut self.model);
        let hotkey_changed = daemon::poll_hotkey_updates(self);
        let recorder_changed = daemon::poll_recorder_status(self);
        let test_stopped = matches!(self.model.page, Page::Player | Page::Editor)
            && self.model.stop_microphone_test();
        if test_stopped {
            self.microphone = None;
        }
        let notice_changed =
            self.notice_expiry
                .tick(&mut self.model.notice, Instant::now(), NOTICE_LIFETIME);
        let images_changed = self.renderer.collect_images();
        let visibility_changed = media::update_fullscreen_controls_visibility(self);
        let playing = matches!(self.model.page, Page::Player | Page::Editor);
        if playing {
            media::sync_player_state(self);
            media::keep_editor_preview_inside_selection(self);
        }
        if trim_changed
            || replay_changed
            || settings_changed
            || hotkey_changed
            || recorder_changed
            || test_stopped
            || notice_changed
            || images_changed
            || visibility_changed
            || playing
        {
            self.redraw();
        }
    }

    /// Springs need every frame while they move; at rest nothing ticks.
    fn keep_frame_clock(&mut self) {
        let animating = self.renderer.is_animating() || self.fullscreen_renderer.is_animating();
        if animating && self.frame_clock.is_none() {
            self.frame_clock = Some(self.surface.add_tick_callback(|surface, _| {
                surface.queue_draw();
                glib::ControlFlow::Continue
            }));
        } else if !animating && let Some(clock) = self.frame_clock.take() {
            clock.remove();
        }
        let advanced = self.renderer.advance_motion();
        if advanced && self.frame_clock.is_none() {
            self.surface.queue_draw();
        }
        self.fullscreen_renderer.advance_motion();
    }
}
