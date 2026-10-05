//! `rewa-ui --render OUT.png [page] [theme] [WIDTHxHEIGHT]`: paints one page into a
//! PNG without opening a window, for checking the interface against the Windows
//! one. It reads the real configuration and clips but never writes them.

use std::path::Path;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{graphene, gsk};
use rewa_core::config::Theme;

use crate::model::{Page, SettingsSection, UiModel};
use crate::render::Renderer;

pub fn render(arguments: &[String]) -> Result<(), String> {
    let [output, rest @ ..] = arguments else {
        return Err("usage: rewa-ui --render OUT.png [page] [theme] [WIDTHxHEIGHT]".into());
    };
    gtk::init().map_err(|error| error.to_string())?;
    let mut model = UiModel::load()?;
    crate::app::load_machine(&mut model);
    let (mut width, mut height) = (1440.0, 900.0);
    for argument in rest {
        apply(&mut model, argument, &mut width, &mut height)?;
    }
    let widget = gtk::DrawingArea::new();
    let family = crate::app::interface_family();
    let mut renderer = Renderer::new(
        widget.pango_context(),
        &family,
        model.paths.thumbnail_dir.clone(),
        true,
    );
    // the first frame asks for thumbnails; wait until they stop arriving
    paint(&mut renderer, &model, width, height)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut quiet = 0;
    while Instant::now() < deadline && quiet < 10 {
        std::thread::sleep(Duration::from_millis(100));
        quiet = if renderer.collect_images() {
            0
        } else {
            quiet + 1
        };
        paint(&mut renderer, &model, width, height)?;
    }
    let node = paint(&mut renderer, &model, width, height)?;
    let display = gtk::gdk::Display::default().ok_or("no display")?;
    let cairo = gsk::CairoRenderer::new();
    cairo
        .realize_for_display(&display)
        .map_err(|error| error.to_string())?;
    let texture = cairo.render_texture(&node, Some(&graphene::Rect::new(0.0, 0.0, width, height)));
    cairo.unrealize();
    texture
        .save_to_png(Path::new(output))
        .map_err(|error| error.to_string())
}

fn paint(
    renderer: &mut Renderer,
    model: &UiModel,
    width: f32,
    height: f32,
) -> Result<gsk::RenderNode, String> {
    let snapshot = gtk::Snapshot::new();
    renderer.paint(&snapshot, model, width, height, false)?;
    snapshot
        .to_node()
        .ok_or_else(|| "nothing was painted".into())
}

fn apply(
    model: &mut UiModel,
    argument: &str,
    width: &mut f32,
    height: &mut f32,
) -> Result<(), String> {
    if let Some((w, h)) = argument.split_once('x')
        && let (Ok(w), Ok(h)) = (w.parse(), h.parse())
    {
        (*width, *height) = (w, h);
        return Ok(());
    }
    let theme = Theme::OPTIONS
        .into_iter()
        .find(|theme| format!("{theme:?}").eq_ignore_ascii_case(argument));
    if let Some(theme) = theme {
        model.config.appearance.theme = theme;
        return Ok(());
    }
    match argument {
        "clips" => model.page = Page::Library,
        "collections" => model.page = Page::Collections,
        "folded" => model.sidebar_collapsed = true,
        "capture" => model.capture_panel_open = true,
        "filters" => model.filter_panel_open = true,
        "general" | "recording" | "audio" | "storage" | "about" => {
            model.page = Page::Settings;
            model.settings_section = match argument {
                "general" => SettingsSection::General,
                "recording" => SettingsSection::Capture,
                "audio" => SettingsSection::Audio,
                "storage" => SettingsSection::Storage,
                _ => SettingsSection::About,
            };
        }
        "english" => {
            model.config.appearance.language = rewa_core::config::Language::English;
            model.refresh_language();
        }
        other => return Err(format!("unknown render option: {other}")),
    }
    Ok(())
}
