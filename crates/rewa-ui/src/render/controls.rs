//! Small shared controls in Leech's style: popover and plate surfaces, doors,
//! segmented controls, pills, washes and the level meter.

use super::*;

pub(crate) enum Segment<'a> {
    Label(&'a str),
    Icon(Glyph),
}

#[derive(Clone, Copy)]
pub(crate) enum PlateButton {
    Plain,
    Primary,
    Destructive,
}

impl Renderer {
    /// Menus and popovers: one raised surface with a hairline edge and a soft,
    /// offset shadow built from stacked translucent layers.
    pub(crate) fn popover_surface(&self, area: LogicalRect) -> Result<(), String> {
        for (spread, alpha) in [(14.0, 0.05), (9.0, 0.07), (5.0, 0.09), (2.0, 0.12)] {
            self.fill_alpha(
                rect(
                    area.left - spread,
                    area.top - spread + 8.0,
                    area.right + spread,
                    area.bottom + spread + 8.0,
                ),
                0x000000,
                alpha,
                RADIUS_LARGE + spread,
            )?;
        }
        self.fill(area, self.palette.surface_raised, RADIUS_LARGE)?;
        self.stroke(
            area,
            mix(self.palette.surface_raised, self.palette.primary, 0.1),
            RADIUS_LARGE,
            1.0,
        )
    }

    /// Leech's plate: the dialog surface, a larger radius and a deeper shadow than a menu.
    pub(crate) fn plate_surface(&self, area: LogicalRect) -> Result<(), String> {
        for (spread, alpha) in [(26.0, 0.04), (16.0, 0.06), (8.0, 0.08), (3.0, 0.1)] {
            self.fill_alpha(
                rect(
                    area.left - spread,
                    area.top - spread + 12.0,
                    area.right + spread,
                    area.bottom + spread + 12.0,
                ),
                0x000000,
                alpha,
                16.0 + spread,
            )?;
        }
        self.fill(area, self.palette.surface, 16.0)?;
        self.stroke(area, self.palette.border, 16.0, 1.0)
    }

    /// Translucent ink over whatever is below: hover and pressed washes.
    pub(crate) fn tint(&self, area: LogicalRect, amount: f32, radius: f32) -> Result<(), String> {
        self.fill_alpha(area, self.palette.primary, amount, radius)
    }

    /// Leech's door: a 26 px icon button that only shows a wash on hover.
    pub(crate) fn door(
        &mut self,
        area: LogicalRect,
        glyph: Glyph,
        active: bool,
        action: Action,
    ) -> Result<(), String> {
        let hovered = self.is_hovered(&action);
        if active {
            self.tint(area, 0.1, RADIUS_SMALL)?;
        } else if hovered {
            self.tint(area, 0.07 * self.hover_amount(1.0), RADIUS_SMALL)?;
        }
        let center_x = (area.left + area.right) / 2.0;
        let center_y = (area.top + area.bottom) / 2.0;
        self.glyph(
            glyph,
            rect(
                center_x - 8.0,
                center_y - 8.0,
                center_x + 8.0,
                center_y + 8.0,
            ),
            if active {
                self.palette.primary
            } else {
                mix(
                    self.palette.muted,
                    self.palette.primary,
                    if hovered {
                        0.5 * self.hover_amount(1.0)
                    } else {
                        0.0
                    },
                )
            },
        )?;
        self.hits.push(HitRegion { rect: area, action });
        Ok(())
    }

    /// A macOS segmented control; the thumb glides between segments.
    pub(crate) fn segmented(
        &mut self,
        area: LogicalRect,
        segments: &[(Segment<'_>, Action)],
        selected: usize,
        key: &'static str,
    ) -> Result<(), String> {
        self.tint(area, 0.06, RADIUS_SMALL)?;
        let width = (area.right - area.left - 4.0) / segments.len() as f32;
        let now = Instant::now();
        let reduced = self.reduced_motion;
        let motion = self.toggle_motions.entry(key).or_insert_with(|| {
            crate::motion::Motion::with_curve(selected as f32, crate::motion::Curve::Glide)
        });
        motion.retarget(selected as f32, now, reduced);
        let position = motion.value(now);
        self.navigation_was_moving |= motion.active(now);
        let thumb = rect(
            area.left + 2.0 + position * width,
            area.top + 2.0,
            area.left + 2.0 + (position + 1.0) * width,
            area.bottom - 2.0,
        );
        self.fill_alpha(
            rect(thumb.left, thumb.top + 1.0, thumb.right, thumb.bottom + 1.0),
            0x000000,
            0.18,
            RADIUS_SMALL - 1.0,
        )?;
        self.fill(thumb, self.palette.surface_raised, RADIUS_SMALL - 1.0)?;
        for (index, (segment, action)) in segments.iter().enumerate() {
            let cell = rect(
                area.left + 2.0 + index as f32 * width,
                area.top,
                area.left + 2.0 + (index + 1) as f32 * width,
                area.bottom,
            );
            let active = index == selected;
            let hovered = !active && self.is_hovered(action);
            let tone = if active {
                self.palette.primary
            } else if hovered {
                self.palette.secondary
            } else {
                self.palette.muted
            };
            match segment {
                Segment::Label(label) => {
                    self.text(label, cell, &self.button.clone(), tone)?;
                }
                Segment::Icon(glyph) => {
                    let center_x = (cell.left + cell.right) / 2.0;
                    let center_y = (cell.top + cell.bottom) / 2.0;
                    self.glyph(
                        *glyph,
                        rect(
                            center_x - 7.5,
                            center_y - 7.5,
                            center_x + 7.5,
                            center_y + 7.5,
                        ),
                        tone,
                    )?;
                }
            }
            self.hits.push(HitRegion {
                rect: cell,
                action: action.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn status_dot(&self, x: f32, y: f32, live: bool) -> Result<(), String> {
        self.fill(
            rect(x - 4.0, y - 4.0, x + 4.0, y + 4.0),
            if live {
                self.palette.live
            } else {
                self.palette.muted
            },
            4.0,
        )
    }

    pub(crate) fn action_button(
        &mut self,
        area: LogicalRect,
        label: &str,
        action: Action,
    ) -> Result<(), String> {
        let background = if self.is_hovered(&action) {
            mix(
                self.palette.accent,
                self.palette.accent_hover,
                self.hover_amount(1.0),
            )
        } else {
            self.palette.accent
        };
        self.fill(area, background, RADIUS_SMALL)?;
        self.text(label, area, &self.button.clone(), self.palette.accent_text)?;
        self.hits.push(HitRegion { rect: area, action });
        Ok(())
    }

    pub(crate) fn level_meter(
        &self,
        area: LogicalRect,
        enabled: bool,
        level: u8,
        hold: u8,
    ) -> Result<(), String> {
        const BARS: usize = 16;
        let pitch = (area.right - area.left) / BARS as f32;
        let bars = |value: u8| (f32::from(value) / 100.0 * BARS as f32).round() as usize;
        let lit = if enabled { bars(level) } else { 0 };
        let marker = if enabled { bars(hold) } else { 0 };
        for index in 0..BARS {
            let x = area.left + index as f32 * pitch;
            let scale = 0.45 + 0.55 * (index as f32 / (BARS - 1) as f32);
            let height = (area.bottom - area.top) * scale;
            let bar = rect(
                x,
                area.bottom - height,
                x + (pitch - 2.0).max(1.5),
                area.bottom,
            );
            self.fill(
                bar,
                if index < lit {
                    self.palette.live
                } else if marker > 0 && index + 1 == marker {
                    self.palette.secondary
                } else if enabled {
                    self.palette.border
                } else {
                    mix(self.palette.canvas, self.palette.border, 0.6)
                },
                1.0,
            )?;
        }
        Ok(())
    }

    pub(crate) fn empty_state(
        &mut self,
        message: &str,
        left: f32,
        right: f32,
        top: f32,
    ) -> Result<(), String> {
        self.text(
            message,
            rect(left, top, right, top + 34.0),
            &self.section.clone(),
            self.palette.secondary,
        )
    }

    pub(crate) fn pill(
        &mut self,
        area: LogicalRect,
        background: u32,
        label: &str,
        foreground: u32,
        action: Option<Action>,
    ) -> Result<(), String> {
        let hovered = action
            .as_ref()
            .is_some_and(|candidate| self.is_hovered(candidate));
        let radius = ((area.bottom - area.top) / 2.0).min(RADIUS_SMALL + 2.0);
        let filled = background == self.palette.accent || background == self.palette.destructive;
        if filled {
            let lit = if background == self.palette.accent {
                self.palette.accent_hover
            } else {
                mix(background, 0xffffff, 0.15)
            };
            self.fill(
                area,
                if hovered {
                    mix(background, lit, self.hover_amount(1.0))
                } else {
                    background
                },
                radius,
            )?;
        } else {
            self.tint(
                area,
                if hovered {
                    0.08 + 0.05 * self.hover_amount(1.0)
                } else {
                    0.08
                },
                radius,
            )?;
        }
        self.text(label, area, &self.button.clone(), foreground)?;
        if let Some(action) = action {
            self.hits.push(HitRegion { rect: area, action });
        }
        Ok(())
    }

    pub(crate) fn floating_glyph(
        &mut self,
        area: LogicalRect,
        glyph: Glyph,
        foreground: u32,
        action: Option<Action>,
    ) -> Result<(), String> {
        let hovered = action
            .as_ref()
            .is_some_and(|candidate| self.is_hovered(candidate));
        let color = if hovered {
            mix(foreground, self.palette.primary, self.hover_amount(0.72))
        } else {
            foreground
        };
        let size = 20.0;
        let center_x = (area.left + area.right) / 2.0;
        let center_y = (area.top + area.bottom) / 2.0;
        self.glyph(
            glyph,
            rect(
                center_x - size / 2.0,
                center_y - size / 2.0,
                center_x + size / 2.0,
                center_y + size / 2.0,
            ),
            color,
        )?;
        if let Some(action) = action {
            self.hits.push(HitRegion { rect: area, action });
        }
        Ok(())
    }
}
