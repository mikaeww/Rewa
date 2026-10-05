//! Icons drawn on a 24 grid with round caps, after Lucide and SF Symbols; no
//! Unicode or emoji stand-ins.

use gtk::{graphene, gsk};

use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Glyph {
    Library,
    Collections,
    Settings,
    Folder,
    Game,
    Search,
    Grid,
    GridCompact,
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

impl Renderer {
    pub(crate) fn glyph(&self, glyph: Glyph, area: LogicalRect, fill: u32) -> Result<(), String> {
        // every icon is laid out on a centred square so a non-square area cannot
        // stretch it out of shape
        let span = (area.right - area.left).min(area.bottom - area.top);
        let center_x = (area.left + area.right) / 2.0;
        let center_y = (area.top + area.bottom) / 2.0;
        let scale = span / 24.0;
        let weight = (span * 0.078).clamp(1.3, 1.8);
        let unit = |x: f32, y: f32| (center_x + (x - 12.0) * scale, center_y + (y - 12.0) * scale);
        let outline = |points: &[(f32, f32)], closed: bool| -> gsk::Path {
            let builder = gsk::PathBuilder::new();
            for (index, (x, y)) in points.iter().enumerate() {
                let (x, y) = unit(*x, *y);
                if index == 0 {
                    builder.move_to(x, y);
                } else {
                    builder.line_to(x, y);
                }
            }
            if closed {
                builder.close();
            }
            builder.to_path()
        };
        let path = |points: &[(f32, f32)], closed: bool| -> Result<(), String> {
            if points.len() >= 2 {
                self.stroke_path(&outline(points, closed), fill, weight);
            }
            Ok(())
        };
        let solid = |points: &[(f32, f32)]| -> Result<(), String> {
            if points.len() >= 3 {
                self.fill_path(&outline(points, true), fill);
            }
            Ok(())
        };
        let disc = |x: f32, y: f32, radius: f32| -> gsk::Path {
            let builder = gsk::PathBuilder::new();
            let (x, y) = unit(x, y);
            builder.add_circle(&graphene::Point::new(x, y), radius * scale);
            builder.to_path()
        };
        let circle = |x: f32, y: f32, radius: f32| -> Result<(), String> {
            self.stroke_path(&disc(x, y, radius), fill, weight);
            Ok(())
        };
        let dot = |x: f32, y: f32, radius: f32| -> Result<(), String> {
            self.fill_path(&disc(x, y, radius), fill);
            Ok(())
        };
        let corners = |left: f32, top: f32, right: f32, bottom: f32| {
            let (left, top) = unit(left, top);
            let (right, bottom) = unit(right, bottom);
            rect(left, top, right, bottom)
        };
        let rounded =
            |left: f32, top: f32, right: f32, bottom: f32, radius: f32| -> Result<(), String> {
                let builder = gsk::PathBuilder::new();
                builder.add_rounded_rect(&painter::rounded(
                    corners(left, top, right, bottom),
                    radius * scale,
                ));
                self.stroke_path(&builder.to_path(), fill, weight);
                Ok(())
            };
        let bar = |left: f32, top: f32, right: f32, bottom: f32| -> Result<(), String> {
            let area = corners(left, top, right, bottom);
            self.fill(area, fill, (area.right - area.left) / 2.0)
        };
        // Direct2D's clockwise small arc is SVG's positive sweep in a y-down frame
        let arc = |from: (f32, f32), to: (f32, f32), radius: f32| -> Result<(), String> {
            let builder = gsk::PathBuilder::new();
            let (from_x, from_y) = unit(from.0, from.1);
            let (to_x, to_y) = unit(to.0, to.1);
            builder.move_to(from_x, from_y);
            builder.svg_arc_to(radius * scale, radius * scale, 0.0, false, true, to_x, to_y);
            self.stroke_path(&builder.to_path(), fill, weight);
            Ok(())
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
            Glyph::Game => {
                rounded(2.8, 6.6, 21.2, 17.4, 5.2)?;
                path(&[(6.4, 12.0), (10.0, 12.0)], false)?;
                path(&[(8.2, 10.2), (8.2, 13.8)], false)?;
                dot(15.4, 11.0, 1.1)?;
                dot(17.8, 13.2, 1.1)?;
            }
            Glyph::GridCompact => {
                for x in [4.0, 10.2, 16.4] {
                    for y in [4.0, 10.2, 16.4] {
                        rounded(x, y, x + 3.6, y + 3.6, 1.0)?;
                    }
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
}
