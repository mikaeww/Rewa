//! Time-based, interruptible motion for native controls. No animation runs at rest.
use std::time::{Duration, Instant};

/// The three curves Leech takes from Search: two SwiftUI springs and a short ease.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve {
    /// Things that travel: the selection pill, segment thumbs.
    Glide,
    /// Things that arrive: popovers, hover lifts.
    Settle,
    /// Colour and opacity changes, 140 ms.
    Quick,
}

impl Curve {
    const fn spring(self) -> Option<(f32, f32)> {
        match self {
            Self::Glide => Some((0.34, 0.82)),
            Self::Settle => Some((0.30, 0.86)),
            Self::Quick => None,
        }
    }

    pub fn duration(self) -> Duration {
        match self.spring() {
            Some((response, damping)) => {
                let (decay, _) = spring_terms(response, damping);
                let mut seconds = 0.05_f32;
                while seconds < 3.0
                    && (1.0 - spring_at(response, damping, seconds)).abs()
                        + (-decay * seconds).exp()
                        > 0.002
                {
                    seconds += 0.01;
                }
                Duration::from_secs_f32(seconds)
            }
            None => Duration::from_millis(140),
        }
    }

    /// Progress at `t` in [0, 1] of the curve's duration; springs may overshoot 1 slightly.
    pub fn ease(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if t >= 1.0 {
            return 1.0;
        }
        match self.spring() {
            Some((response, damping)) => {
                spring_at(response, damping, t * self.duration().as_secs_f32())
            }
            None => cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
        }
    }
}

fn spring_terms(response: f32, damping: f32) -> (f32, f32) {
    let omega = std::f32::consts::TAU / response;
    (damping * omega, omega * (1.0 - damping * damping).sqrt())
}

fn spring_at(response: f32, damping: f32, seconds: f32) -> f32 {
    let (decay, damped) = spring_terms(response, damping);
    1.0 - (-decay * seconds).exp()
        * ((damped * seconds).cos() + decay / damped * (damped * seconds).sin())
}

fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let axis = |a: f32, b: f32, s: f32| {
        let inverse = 1.0 - s;
        3.0 * inverse * inverse * s * a + 3.0 * inverse * s * s * b + s * s * s
    };
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let middle = (low + high) / 2.0;
        if axis(x1, x2, middle) < x {
            low = middle;
        } else {
            high = middle;
        }
    }
    axis(y1, y2, (low + high) / 2.0)
}

#[derive(Debug)]
pub struct Motion {
    from: f32,
    target: f32,
    started: Instant,
    curve: Curve,
    duration: Duration,
}

impl Motion {
    pub fn new(value: f32) -> Self {
        Self::with_curve(value, Curve::Settle)
    }

    pub fn with_curve(value: f32, curve: Curve) -> Self {
        Self {
            from: value,
            target: value,
            started: Instant::now(),
            curve,
            duration: curve.duration(),
        }
    }

    pub fn value(&self, now: Instant) -> f32 {
        let t =
            now.saturating_duration_since(self.started).as_secs_f32() / self.duration.as_secs_f32();
        self.from + (self.target - self.from) * self.curve.ease(t)
    }

    pub fn retarget(&mut self, target: f32, now: Instant, reduced: bool) {
        if target == self.target && !reduced {
            return;
        }
        self.from = if reduced { target } else { self.value(now) };
        self.target = target;
        self.started = now;
    }

    pub fn active(&self, now: Instant) -> bool {
        self.from != self.target && now.saturating_duration_since(self.started) < self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_motion_starts_at_visible_position_and_settles() {
        let now = Instant::now();
        let mut motion = Motion::new(0.0);
        motion.retarget(100.0, now, false);
        let halfway = now + Duration::from_millis(110);
        let visible = motion.value(halfway);
        assert!(visible > 50.0 && visible < 105.0);
        motion.retarget(20.0, halfway, false);
        assert_eq!(motion.value(halfway), visible);
        let end = halfway + Curve::Settle.duration();
        assert_eq!(motion.value(end), 20.0);
        assert!(!motion.active(end));
    }

    #[test]
    fn reduced_motion_is_immediate() {
        let now = Instant::now();
        let mut motion = Motion::new(0.0);
        motion.retarget(100.0, now, true);
        assert_eq!(motion.value(now), 100.0);
        assert!(!motion.active(now));
    }

    #[test]
    fn springs_match_leech_and_quick_is_the_css_ease() {
        // Leech's sampled linear() springs last about 0.4 s; the glide one is the slower of the two
        let glide = Curve::Glide.duration().as_secs_f32();
        let settle = Curve::Settle.duration().as_secs_f32();
        assert!(
            (0.3..0.6).contains(&settle) && glide > settle,
            "{glide} {settle}"
        );
        let peak = (0..=100)
            .map(|step| Curve::Glide.ease(step as f32 / 100.0))
            .fold(0.0_f32, f32::max);
        assert!(peak > 1.0 && peak < 1.02, "glide overshoots gently: {peak}");
        assert!((Curve::Quick.ease(0.5) - 0.8024).abs() < 0.01);
        assert_eq!(Curve::Quick.ease(1.0), 1.0);
    }
}
