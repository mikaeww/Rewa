//! Settings lines in their hairline cards, and the controls a line can carry.

use crate::render::*;

pub(crate) enum LineControl<'a> {
    Switch(bool, Action),
    Popup(&'a str, Action),
    /// A value to press; the flag marks a capture in progress.
    Pill(&'a str, Action, bool),
    Slider(u16, Action),
}

impl Renderer {
    pub(crate) fn settings_card(&self, first: LogicalRect, lines: usize) -> Result<(), String> {
        self.stroke(
            rect(
                first.left,
                first.top,
                first.right,
                first.top + lines as f32 * SETTINGS_LINE_HEIGHT,
            ),
            self.palette.border,
            11.0,
            1.0,
        )
    }

    /// One Leech settings line: a name, a quiet detail and the control on the
    /// right; returns where the control landed.
    pub(crate) fn settings_line(
        &mut self,
        line: LogicalRect,
        index: usize,
        title: &'static str,
        detail: &str,
        control: LineControl<'_>,
    ) -> Result<LogicalRect, String> {
        self.settings_line_toned(line, index, title, detail, self.palette.muted, control)
    }

    pub(crate) fn settings_line_toned(
        &mut self,
        line: LogicalRect,
        index: usize,
        title: &'static str,
        detail: &str,
        detail_tone: u32,
        control: LineControl<'_>,
    ) -> Result<LogicalRect, String> {
        if index > 0 {
            self.fill(
                rect(line.left + 14.0, line.top, line.right, line.top + 1.0),
                self.palette.hairline,
                0.0,
            )?;
        }
        let center = (line.top + line.bottom) / 2.0;
        let control_area = match &control {
            LineControl::Switch(_, _) => rect(
                line.right - 44.0,
                center - 9.0,
                line.right - 14.0,
                center + 9.0,
            ),
            LineControl::Popup(value, _) => {
                let width = (self.measure(value, &self.body) + 36.0).clamp(80.0, 230.0);
                rect(
                    line.right - 14.0 - width,
                    center - 12.0,
                    line.right - 14.0,
                    center + 12.0,
                )
            }
            LineControl::Pill(value, _, _) => {
                let width = (self.measure(value, &self.small) + 26.0).clamp(64.0, 260.0);
                rect(
                    line.right - 14.0 - width,
                    center - 12.0,
                    line.right - 14.0,
                    center + 12.0,
                )
            }
            LineControl::Slider(_, _) => {
                let track = settings_slider_track(line);
                rect(track.left - 54.0, center - 12.0, track.right, center + 12.0)
            }
        };
        let words_right = control_area.left - 20.0;
        self.text(
            title,
            rect(line.left + 14.0, center - 19.0, words_right, center + 1.0),
            &self.body.clone(),
            self.palette.primary,
        )?;
        self.text(
            &self.shorten(detail, &self.small, words_right - line.left - 14.0),
            rect(line.left + 14.0, center + 1.0, words_right, center + 19.0),
            &self.small.clone(),
            detail_tone,
        )?;
        match control {
            LineControl::Switch(on, action) => {
                self.switch(control_area, on, title)?;
                self.hits.push(HitRegion {
                    rect: rect(
                        control_area.left - 6.0,
                        line.top + 8.0,
                        control_area.right + 6.0,
                        line.bottom - 8.0,
                    ),
                    action,
                });
            }
            LineControl::Popup(value, action) => {
                let hovered = self.is_hovered(&action);
                self.fill(
                    control_area,
                    if hovered {
                        mix(
                            self.palette.surface_raised,
                            self.palette.primary,
                            0.05 * self.hover_amount(1.0),
                        )
                    } else {
                        self.palette.surface_raised
                    },
                    6.0,
                )?;
                self.stroke(
                    control_area,
                    mix(self.palette.surface_raised, self.palette.primary, 0.08),
                    6.0,
                    1.0,
                )?;
                let text_right = control_area.right - 24.0;
                self.text(
                    &self.shorten(value, &self.body, text_right - control_area.left - 10.0),
                    rect(
                        control_area.left + 10.0,
                        control_area.top,
                        text_right,
                        control_area.bottom,
                    ),
                    &self.body.clone(),
                    self.palette.primary,
                )?;
                self.glyph(
                    Glyph::UpDown,
                    rect(
                        control_area.right - 20.0,
                        center - 6.0,
                        control_area.right - 8.0,
                        center + 6.0,
                    ),
                    self.palette.muted,
                )?;
                self.hits.push(HitRegion {
                    rect: control_area,
                    action,
                });
            }
            LineControl::Pill(value, action, capturing) => {
                let hovered = self.is_hovered(&action);
                if hovered || capturing {
                    self.tint(control_area, if capturing { 0.1 } else { 0.06 }, 12.0)?;
                }
                self.stroke(
                    control_area,
                    if capturing {
                        self.palette.primary
                    } else {
                        self.palette.border
                    },
                    12.0,
                    1.0,
                )?;
                self.text(
                    &self.shorten(
                        value,
                        &self.small,
                        control_area.right - control_area.left - 20.0,
                    ),
                    control_area,
                    &self.small_center.clone(),
                    self.palette.primary,
                )?;
                self.hits.push(HitRegion {
                    rect: control_area,
                    action,
                });
            }
            LineControl::Slider(value, action) => {
                let track = settings_slider_track(line);
                self.text(
                    &format!("{} %", value.min(200)),
                    rect(
                        control_area.left,
                        control_area.top,
                        track.left - 14.0,
                        control_area.bottom,
                    ),
                    &self.small_right.clone(),
                    self.palette.muted,
                )?;
                self.slider(
                    track,
                    f32::from(value.min(200)) / 200.0,
                    self.is_hovered(&action),
                )?;
                self.hits.push(HitRegion {
                    rect: rect(
                        track.left - 8.0,
                        line.top + 12.0,
                        track.right + 8.0,
                        line.bottom - 12.0,
                    ),
                    action,
                });
            }
        }
        Ok(control_area)
    }

    /// Leech's switch: 30 × 18, the knob settles across on the settle spring.
    pub(crate) fn switch(
        &mut self,
        area: LogicalRect,
        enabled: bool,
        key: &'static str,
    ) -> Result<(), String> {
        let now = Instant::now();
        let value = if enabled { 1.0 } else { 0.0 };
        let reduced = self.reduced_motion;
        let motion = self
            .toggle_motions
            .entry(key)
            .or_insert_with(|| crate::motion::Motion::new(value));
        motion.retarget(value, now, reduced);
        let travel = motion.value(now);
        self.navigation_was_moving |= motion.active(now);
        let off = mix(self.palette.surface, self.palette.primary, 0.22);
        self.fill(
            area,
            mix(off, self.palette.primary, travel.clamp(0.0, 1.0)),
            9.0,
        )?;
        let left = area.left + 2.0 + 12.0 * travel;
        let knob = rect(left, area.top + 2.0, left + 14.0, area.bottom - 2.0);
        self.fill_alpha(
            rect(knob.left, knob.top + 1.0, knob.right, knob.bottom + 1.0),
            0x000000,
            0.22,
            7.0,
        )?;
        self.fill(knob, self.palette.surface, 7.0)
    }
}
