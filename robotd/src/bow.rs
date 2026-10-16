//! The polite bow — the one trick the daemon choreographs rather than swaps a network in for.
//!
//! A bow is a lean and a nod, held a moment. The gait already knows how to lean: velstand (set
//! v7 on) tracks a commanded body pitch of about ±12°, and every gait takes head targets in its
//! command. So nothing has to be trained to bow. This writes those two commands over time —
//! forward over `down_s` on an ease-in-ease-out curve, held for `hold_s`, back over `up_s` — and
//! the network balancing the robot does the rest, with the fall reflex still armed, because no
//! skill network is running: it is the gait, asked to lean.
//!
//! It rides on the command the way the idle glance and the chorale's sway do, added to the head
//! and to the body pose, and the loop holds the body-pose mode while it runs, which zeroes the
//! twist: a robot does not walk and bow at once. `robot.do bow` starts it, from a robot standing
//! with nothing else in flight; a second request during a bow is refused rather than queued, so a
//! held button is one bow and not a nodding fit.
//!
//! A trained bow replaces it without touching this: a skill table entry *named* `bow` is what
//! `robot.do bow` runs then, and the built-in steps aside. `[bow]` in `robotd.toml` is how deep,
//! how far the head drops, and how long each part takes ([`BowParams`]).

use std::time::Instant;

use robotd_params::BowParams;

/// What the bow adds to the command this tick — zero when there is no bow.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BowOffset {
    /// Neck pitch, head pitch, head yaw, head roll — the head command's order. Positive pitch
    /// is down, as `robot.look` has it.
    pub head: [f64; 4],
    /// z, roll, pitch — the body command's order. Positive pitch is forward.
    pub body: [f64; 3],
}

/// One bow at a time, timed from when it started.
#[derive(Debug)]
pub struct Bow {
    tuning: BowParams,
    since: Option<Instant>,
}

impl Bow {
    pub fn new(tuning: BowParams) -> Self {
        Self {
            tuning,
            since: None,
        }
    }

    /// Whether a bow is in flight — the loop holds the body-pose mode while it is.
    pub fn active(&self) -> bool {
        self.since.is_some()
    }

    /// Start one now. Refused while one runs: a held button is one bow.
    pub fn start(&mut self, now: Instant) -> Result<(), &'static str> {
        if self.since.is_some() {
            return Err("a bow is already running");
        }
        self.since = Some(now);
        Ok(())
    }

    /// What to add this tick. A bow past its end ends itself here, so the loop never has to.
    pub fn tick(&mut self, now: Instant) -> BowOffset {
        let Some(since) = self.since else {
            return BowOffset::default();
        };
        let t = now.saturating_duration_since(since).as_secs_f64();
        if t >= total(&self.tuning) {
            self.since = None;
            return BowOffset::default();
        }
        let d = depth(t, &self.tuning);
        BowOffset {
            head: [
                self.tuning.neck_pitch * d,
                self.tuning.head_pitch * d,
                0.0,
                0.0,
            ],
            body: [0.0, 0.0, self.tuning.pitch * d],
        }
    }
}

/// Seconds from the start of a bow to its end.
fn total(p: &BowParams) -> f64 {
    p.down_s + p.hold_s + p.up_s
}

/// How far into the bow the robot is at `t` seconds: 0 upright, 1 fully bowed. Eased down over
/// `down_s`, held at 1 for `hold_s`, eased back over `up_s`, and 0 from the end on.
///
/// The easing is smoothstep, so the lean starts and stops with zero rate — the gait is tracking
/// this, and a command that jumps is a lurch it has to catch.
pub fn depth(t: f64, p: &BowParams) -> f64 {
    let down_end = p.down_s;
    let hold_end = down_end + p.hold_s;
    let up_end = hold_end + p.up_s;
    if t <= 0.0 {
        0.0
    } else if t < down_end {
        ease(t / p.down_s)
    } else if t < hold_end {
        1.0
    } else if t < up_end {
        ease(1.0 - (t - hold_end) / p.up_s)
    } else {
        0.0
    }
}

fn ease(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn tuning() -> BowParams {
        BowParams {
            pitch: 0.26,
            neck_pitch: 0.15,
            head_pitch: 0.35,
            down_s: 0.8,
            hold_s: 0.6,
            up_s: 0.8,
        }
    }

    /// **The profile is the bow**: down, held, up, and nothing before or after. A profile that
    /// did not return to zero would leave the gait leaning for good.
    #[test]
    fn the_profile_goes_down_holds_and_comes_back() {
        let p = tuning();
        assert_eq!(depth(-1.0, &p), 0.0);
        assert_eq!(depth(0.0, &p), 0.0);
        assert!(
            (depth(0.4, &p) - 0.5).abs() < 1e-9,
            "halfway down is half the lean"
        );
        assert_eq!(depth(0.8, &p), 1.0);
        assert_eq!(depth(1.1, &p), 1.0, "held");
        assert!((depth(1.8, &p) - 0.5).abs() < 1e-9, "halfway up");
        assert_eq!(depth(2.2, &p), 0.0);
        assert_eq!(depth(9.0, &p), 0.0);
    }

    /// **Smooth at both ends, and monotone in between.** A lean that overshoots or jitters on
    /// the way is a command the gait has to catch, and the real robot's servos would show it.
    #[test]
    fn the_lean_never_reverses_mid_move() {
        let p = tuning();
        let mut last = 0.0;
        let mut t = 0.0;
        while t <= p.down_s {
            let d = depth(t, &p);
            assert!(d >= last - 1e-12, "down is monotone at t={t}");
            last = d;
            t += 0.02;
        }
        let mut last = 1.0;
        let mut t = p.down_s + p.hold_s;
        while t <= p.down_s + p.hold_s + p.up_s {
            let d = depth(t, &p);
            assert!(d <= last + 1e-12, "up is monotone at t={t}");
            last = d;
            t += 0.02;
        }
        // Zero rate at the very start: the first 20 ms tick asks for almost nothing.
        assert!(depth(0.02, &p) < 0.01);
    }

    /// **A bow runs itself out, and one at a time.** The loop only ever asks "what now" — if the
    /// bow did not end itself, nothing would.
    #[test]
    fn a_bow_starts_once_runs_its_course_and_ends_itself() {
        let mut bow = Bow::new(tuning());
        let t0 = Instant::now();
        assert!(!bow.active());
        assert_eq!(bow.tick(t0), BowOffset::default(), "nothing before a start");

        assert_eq!(bow.start(t0), Ok(()));
        assert!(bow.active());
        assert_eq!(bow.start(t0), Err("a bow is already running"));

        let mid_hold = t0 + Duration::from_millis(1100);
        let at = bow.tick(mid_hold);
        assert!(
            (at.body[2] - 0.26).abs() < 1e-9,
            "the lean is the configured pitch"
        );
        assert!((at.head[0] - 0.15).abs() < 1e-9 && (at.head[1] - 0.35).abs() < 1e-9);
        assert_eq!(
            at.head[2..],
            [0.0, 0.0],
            "a bow does not turn or tilt the head"
        );
        assert_eq!(at.body[..2], [0.0, 0.0], "nor crouch or roll");

        let after = t0 + Duration::from_millis(2300);
        assert_eq!(bow.tick(after), BowOffset::default());
        assert!(!bow.active(), "ended itself");
        assert_eq!(bow.start(after), Ok(()), "and may run again");
    }
}
