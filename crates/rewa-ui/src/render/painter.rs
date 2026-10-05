//! The palettes and the drawing primitives every page uses: rounded fills and
//! hairlines, translucent washes, text in the fixed type styles, clips and pictures.
//! They draw into the GTK snapshot of the frame being painted.

use gtk::prelude::*;
use gtk::{gdk, graphene, gsk, pango};

use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Palette {
    pub canvas: u32,
    pub rail: u32,
    pub stage: u32,
    pub surface: u32,
    pub surface_raised: u32,
    pub border: u32,
    pub hairline: u32,
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
pub(crate) const DARK_PALETTE: Palette = Palette {
    canvas: 0x141414,
    rail: 0x141414,
    stage: 0x111111,
    surface: 0x1c1c1c,
    surface_raised: 0x2b2b2b,
    border: 0x333333,
    hairline: 0x2a2a2a,
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

pub(crate) const LIGHT_PALETTE: Palette = Palette {
    canvas: 0xf2f2f2,
    rail: 0xf2f2f2,
    stage: 0xe9e9e9,
    surface: 0xffffff,
    surface_raised: 0xffffff,
    border: 0xe0e0e0,
    hairline: 0xe8e8e8,
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

pub(crate) const CAFE_PALETTE: Palette = Palette {
    canvas: 0x0d0c0b,
    rail: 0x100f0d,
    stage: 0x100f0e,
    surface: 0x151412,
    surface_raised: 0x191715,
    border: 0x2a2621,
    hairline: 0x201d1a,
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

pub(crate) const PINK_PALETTE: Palette = Palette {
    canvas: 0x120b0f,
    rail: 0x160d12,
    stage: 0x150d11,
    surface: 0x1d1218,
    surface_raised: 0x23161d,
    border: 0x402834,
    hairline: 0x2e1d27,
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

pub(crate) const CANDY_PALETTE: Palette = Palette {
    canvas: 0xfff0f6,
    rail: 0xffe3ef,
    stage: 0xffdfec,
    surface: 0xffffff,
    surface_raised: 0xffffff,
    border: 0xf7b9d4,
    hairline: 0xffd3e5,
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

pub(crate) const fn palette_for(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => DARK_PALETTE,
        Theme::Light => LIGHT_PALETTE,
        Theme::Cafe => CAFE_PALETTE,
        Theme::Pink => PINK_PALETTE,
        Theme::Candy => CANDY_PALETTE,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Alignment {
    Leading,
    Center,
    Trailing,
}

/// One of the fixed type styles; the Windows renderer's text formats.
#[derive(Clone)]
pub(crate) struct Font {
    description: pango::FontDescription,
    alignment: Alignment,
    wrap: bool,
    /// Measurements are cached per style; the id tells the styles apart.
    pub(crate) id: usize,
}

impl Font {
    pub(crate) fn new(
        family: &str,
        size: f32,
        semibold: bool,
        alignment: Alignment,
        id: usize,
    ) -> Self {
        let mut description = pango::FontDescription::new();
        description.set_family(family);
        description.set_absolute_size(f64::from(size) * f64::from(pango::SCALE));
        description.set_weight(if semibold {
            pango::Weight::Semibold
        } else {
            pango::Weight::Normal
        });
        Self {
            description,
            alignment,
            wrap: false,
            id,
        }
    }

    /// Wrapping text starts at the top of its area instead of its middle.
    pub(crate) fn wrapping(mut self) -> Self {
        self.wrap = true;
        self
    }
}

pub(crate) fn mix(from: u32, to: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |shift: u32| {
        let from = ((from >> shift) & 0xff) as f32;
        let to = ((to >> shift) & 0xff) as f32;
        from.mul_add(1.0 - amount, to * amount).round() as u32
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

fn rgba(rgb: u32, alpha: f32) -> gdk::RGBA {
    gdk::RGBA::new(
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        alpha.clamp(0.0, 1.0),
    )
}

/// Direct2D quietly shrinks a radius that does not fit; GSK needs it done up front.
pub(crate) fn rounded(area: LogicalRect, radius: f32) -> gsk::RoundedRect {
    let radius = radius
        .min((area.right - area.left) / 2.0)
        .min((area.bottom - area.top) / 2.0)
        .max(0.0);
    gsk::RoundedRect::from_rect(area.graphene(), radius)
}

impl Renderer {
    pub(crate) fn snapshot(&self) -> &gtk::Snapshot {
        self.snapshot
            .as_ref()
            .expect("invariant: primitives only run inside paint, which sets the snapshot")
    }

    pub(crate) fn fill(&self, area: LogicalRect, fill: u32, radius: f32) -> Result<(), String> {
        self.fill_alpha(area, fill, 1.0, radius)
    }

    pub(crate) fn fill_alpha(
        &self,
        area: LogicalRect,
        fill: u32,
        alpha: f32,
        radius: f32,
    ) -> Result<(), String> {
        if area.right <= area.left || area.bottom <= area.top {
            return Ok(());
        }
        let snapshot = self.snapshot();
        if radius > 0.0 {
            snapshot.push_rounded_clip(&rounded(area, radius));
            snapshot.append_color(&rgba(fill, alpha), &area.graphene());
            snapshot.pop();
        } else {
            snapshot.append_color(&rgba(fill, alpha), &area.graphene());
        }
        Ok(())
    }

    /// A hairline centred on the edge, as Direct2D strokes it.
    pub(crate) fn stroke(
        &self,
        area: LogicalRect,
        stroke_color: u32,
        radius: f32,
        width: f32,
    ) -> Result<(), String> {
        let builder = gsk::PathBuilder::new();
        builder.add_rounded_rect(&rounded(area, radius));
        self.snapshot().append_stroke(
            &builder.to_path(),
            &gsk::Stroke::new(width),
            &rgba(stroke_color, 1.0),
        );
        Ok(())
    }

    pub(crate) fn stroke_path(&self, path: &gsk::Path, fill: u32, weight: f32) {
        let stroke = gsk::Stroke::new(weight);
        stroke.set_line_cap(gsk::LineCap::Round);
        stroke.set_line_join(gsk::LineJoin::Round);
        self.snapshot()
            .append_stroke(path, &stroke, &rgba(fill, 1.0));
    }

    pub(crate) fn fill_path(&self, path: &gsk::Path, fill: u32) {
        self.snapshot()
            .append_fill(path, gsk::FillRule::Winding, &rgba(fill, 1.0));
    }

    fn layout(&self, value: &str, format: &Font) -> pango::Layout {
        let layout = pango::Layout::new(&self.pango);
        layout.set_font_description(Some(&format.description));
        layout.set_text(value);
        layout
    }

    pub(crate) fn measure_uncached(&self, value: &str, format: &Font) -> f32 {
        let (_, logical) = self.layout(value, format).pixel_extents();
        logical.width() as f32
    }

    /// Text clipped to its area: one centred line, or wrapped lines from the top.
    pub(crate) fn text(
        &self,
        value: &str,
        area: LogicalRect,
        format: &Font,
        fill: u32,
    ) -> Result<(), String> {
        let width = area.right - area.left;
        if value.is_empty() || width <= 0.0 || area.bottom <= area.top {
            return Ok(());
        }
        let layout = self.layout(value, format);
        let (x, y) = if format.wrap {
            layout.set_width((width * pango::SCALE as f32) as i32);
            layout.set_wrap(pango::WrapMode::WordChar);
            layout.set_alignment(match format.alignment {
                Alignment::Leading => pango::Alignment::Left,
                Alignment::Center => pango::Alignment::Center,
                Alignment::Trailing => pango::Alignment::Right,
            });
            (area.left, area.top)
        } else {
            let (_, logical) = layout.pixel_extents();
            let text_width = logical.width() as f32;
            let x = match format.alignment {
                Alignment::Leading => area.left,
                Alignment::Center => area.left + (width - text_width) / 2.0,
                Alignment::Trailing => area.right - text_width,
            };
            (
                x,
                area.top + (area.bottom - area.top - logical.height() as f32) / 2.0,
            )
        };
        let snapshot = self.snapshot();
        snapshot.push_clip(&area.graphene());
        snapshot.save();
        snapshot.translate(&graphene::Point::new(x, y));
        snapshot.append_layout(&layout, &rgba(fill, 1.0));
        snapshot.restore();
        snapshot.pop();
        Ok(())
    }

    pub(crate) fn push_clip(&self, area: LogicalRect) -> Result<(), String> {
        self.snapshot().push_clip(&area.graphene());
        Ok(())
    }

    pub(crate) fn pop_clip(&self) {
        self.snapshot().pop();
    }

    /// A picture stretched over `area` with rounded corners, the way the Windows
    /// renderer paints thumbnails with a bitmap brush.
    pub(crate) fn picture(
        &self,
        texture: &gdk::Texture,
        area: LogicalRect,
        radius: f32,
        opacity: f32,
    ) {
        let snapshot = self.snapshot();
        let faded = opacity < 1.0;
        if faded {
            snapshot.push_opacity(f64::from(opacity.max(0.0)));
        }
        if radius > 0.0 {
            snapshot.push_rounded_clip(&rounded(area, radius));
        }
        snapshot.append_scaled_texture(texture, gsk::ScalingFilter::Linear, &area.graphene());
        if radius > 0.0 {
            snapshot.pop();
        }
        if faded {
            snapshot.pop();
        }
    }
}
