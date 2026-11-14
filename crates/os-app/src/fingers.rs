// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Touch screens (the web build on phones and tablets): fingers become the mouse input the app and the plugin windows
//! already take (`Shell::touch_out` turns each [`Out`] into a winit mouse event).
//!
//! - One finger: the pointer moves there at once (hover). Lifting it before it travels is a click; travelling past
//!   [`SLOP`] presses the left button where it went down and drags. A tap twice in a row is a double click.
//! - Held still for [`HOLD`]: a right click (the press is never sent, so nothing is clicked first).
//! - Two fingers: moving them together scrolls (a wheel turn); spreading or pinching them zooms (Ctrl + wheel). A drag
//!   that was running is let go first.

use web_time::{Duration, Instant};
use winit::event::{Touch, TouchPhase};

/// How long a finger is held still for a right click.
pub const HOLD: Duration = Duration::from_millis(500);
/// How far a finger may wander (CSS px) and still be a tap or a hold.
pub const SLOP: f64 = 9.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Btn {
    Left,
    Right,
}

/// Mouse input for the app, positions and deltas in physical canvas pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    Move(f64, f64),
    Down(Btn),
    Up(Btn),
    /// A wheel turn in pixels (40 px a notch, `PixelDelta`): positive y scrolls up, as the content follows the fingers.
    Wheel(f64, f64),
    /// Ctrl + wheel by this many pixels: positive zooms in.
    Zoom(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Two {
    /// Not decided yet: how far the middle has moved and how much the span changed, since the second finger landed.
    Undecided(f64, f64),
    Pan,
    Pinch,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Idle,
    /// One finger down, not moved yet: a tap, a hold or the start of a drag.
    Pending { id: u64, at: (f64, f64), since: Instant },
    Drag { id: u64 },
    /// Two fingers: their middle and span at the last move.
    Two { kind: Two, mid: (f64, f64), span: f64 },
    /// A gesture is over (a hold answered, or fingers left after two): nothing more until every finger is up.
    Spent,
}

pub struct Touches {
    fingers: Vec<(u64, (f64, f64))>,
    mode: Mode,
}

impl Default for Touches {
    fn default() -> Self {
        Touches { fingers: Vec::new(), mode: Mode::Idle }
    }
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

impl Touches {
    /// One touch event; `k` = device pixels per CSS pixel.
    pub fn feed(&mut self, t: &Touch, k: f64, now: Instant) -> Vec<Out> {
        let p = (t.location.x, t.location.y);
        self.phase(t.id, t.phase, p, k, now)
    }

    fn pair(&self) -> Option<((f64, f64), f64)> {
        let [(_, a), (_, b), ..] = self.fingers[..] else { return None };
        Some((((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), dist(a, b)))
    }

    fn start_two(&mut self, out: &mut Vec<Out>) {
        if let Mode::Drag { .. } = self.mode {
            out.push(Out::Up(Btn::Left));
        }
        self.mode = match self.pair() {
            Some((mid, span)) => {
                out.push(Out::Move(mid.0, mid.1));
                Mode::Two { kind: Two::Undecided(0.0, 0.0), mid, span }
            }
            None => Mode::Spent,
        };
    }

    pub(crate) fn phase(&mut self, id: u64, phase: TouchPhase, p: (f64, f64), k: f64, now: Instant) -> Vec<Out> {
        let mut out = Vec::new();
        let slop = SLOP * k.max(0.5);
        match phase {
            TouchPhase::Started => {
                self.fingers.retain(|f| f.0 != id);
                self.fingers.push((id, p));
                match (self.fingers.len(), self.mode) {
                    (1, _) => {
                        out.push(Out::Move(p.0, p.1));
                        self.mode = Mode::Pending { id, at: p, since: now };
                    }
                    (2, Mode::Pending { .. } | Mode::Drag { .. } | Mode::Idle) => self.start_two(&mut out),
                    _ => {}
                }
            }
            TouchPhase::Moved => {
                let Some(f) = self.fingers.iter_mut().find(|f| f.0 == id) else { return out };
                f.1 = p;
                match self.mode {
                    Mode::Pending { id: i, at, .. } if i == id && dist(at, p) > slop => {
                        out.push(Out::Down(Btn::Left));
                        out.push(Out::Move(p.0, p.1));
                        self.mode = Mode::Drag { id };
                    }
                    Mode::Drag { id: i } if i == id => out.push(Out::Move(p.0, p.1)),
                    Mode::Two { kind, mid, span } => {
                        let Some((m, s)) = self.pair() else { return out };
                        let (dx, dy, ds) = (m.0 - mid.0, m.1 - mid.1, s - span);
                        let kind = match kind {
                            Two::Undecided(moved, spread) => {
                                let (moved, spread) = (moved + dx.hypot(dy), spread + ds.abs());
                                if moved.max(spread) < slop {
                                    Two::Undecided(moved, spread)
                                } else if spread > moved {
                                    Two::Pinch
                                } else {
                                    Two::Pan
                                }
                            }
                            k => k,
                        };
                        match kind {
                            Two::Pan => {
                                out.push(Out::Move(m.0, m.1));
                                out.push(Out::Wheel(dx, dy));
                            }
                            Two::Pinch => out.push(Out::Zoom(ds)),
                            Two::Undecided(..) => {}
                        }
                        self.mode = Mode::Two { kind, mid: m, span: s };
                    }
                    _ => {}
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let ended = phase == TouchPhase::Ended;
                self.fingers.retain(|f| f.0 != id);
                match self.mode {
                    Mode::Pending { id: i, .. } if i == id => {
                        if ended {
                            out.push(Out::Down(Btn::Left));
                            out.push(Out::Up(Btn::Left));
                        }
                        self.mode = Mode::Idle;
                    }
                    Mode::Drag { id: i } if i == id => {
                        if ended {
                            out.push(Out::Move(p.0, p.1));
                        }
                        out.push(Out::Up(Btn::Left));
                        self.mode = Mode::Idle;
                    }
                    Mode::Two { .. } | Mode::Spent if self.fingers.is_empty() => self.mode = Mode::Idle,
                    Mode::Two { .. } if self.fingers.len() < 2 => self.mode = Mode::Spent,
                    Mode::Two { kind, .. } => {
                        if let Some((mid, span)) = self.pair() {
                            self.mode = Mode::Two { kind, mid, span };
                        }
                    }
                    _ if self.fingers.is_empty() => self.mode = Mode::Idle,
                    _ => {}
                }
            }
        }
        out
    }

    /// A finger held still long enough: the right click, once.
    pub fn hold(&mut self, now: Instant) -> Vec<Out> {
        match self.mode {
            Mode::Pending { since, .. } if now.duration_since(since) >= HOLD => {
                self.mode = Mode::Spent;
                vec![Out::Down(Btn::Right), Out::Up(Btn::Right)]
            }
            _ => Vec::new(),
        }
    }

    /// When `hold` should be asked next.
    pub fn deadline(&self) -> Option<Instant> {
        match self.mode {
            Mode::Pending { since, .. } => Some(since + HOLD),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Btn::*;
    use Out::*;
    use TouchPhase::*;

    #[test]
    fn tap_is_a_click_where_the_finger_was() {
        let (mut t, now) = (Touches::default(), Instant::now());
        assert_eq!(t.phase(1, Started, (100.0, 50.0), 2.0, now), vec![Move(100.0, 50.0)]);
        assert_eq!(t.phase(1, Moved, (104.0, 52.0), 2.0, now), vec![]);
        assert_eq!(t.phase(1, Ended, (104.0, 52.0), 2.0, now), vec![Down(Left), Up(Left)]);
        assert_eq!(t.hold(now + HOLD), vec![]);
    }

    #[test]
    fn travel_presses_at_the_start_then_drags() {
        let (mut t, now) = (Touches::default(), Instant::now());
        t.phase(1, Started, (100.0, 50.0), 1.0, now);
        assert_eq!(t.phase(1, Moved, (100.0, 70.0), 1.0, now), vec![Down(Left), Move(100.0, 70.0)]);
        assert_eq!(t.phase(1, Moved, (100.0, 90.0), 1.0, now), vec![Move(100.0, 90.0)]);
        assert_eq!(t.hold(now + HOLD * 2), vec![]);
        assert_eq!(t.phase(1, Ended, (100.0, 95.0), 1.0, now), vec![Move(100.0, 95.0), Up(Left)]);
    }

    #[test]
    fn hold_is_a_right_click_and_nothing_after() {
        let (mut t, now) = (Touches::default(), Instant::now());
        t.phase(1, Started, (10.0, 10.0), 1.0, now);
        assert_eq!(t.deadline(), Some(now + HOLD));
        assert_eq!(t.hold(now + Duration::from_millis(100)), vec![]);
        assert_eq!(t.hold(now + HOLD), vec![Down(Right), Up(Right)]);
        assert_eq!(t.phase(1, Moved, (60.0, 10.0), 1.0, now), vec![]);
        assert_eq!(t.phase(1, Ended, (60.0, 10.0), 1.0, now), vec![]);
        assert_eq!(t.phase(2, Started, (5.0, 5.0), 1.0, now), vec![Move(5.0, 5.0)]);
    }

    #[test]
    fn two_fingers_scroll_and_let_a_drag_go() {
        let (mut t, now) = (Touches::default(), Instant::now());
        t.phase(1, Started, (100.0, 100.0), 1.0, now);
        t.phase(1, Moved, (100.0, 130.0), 1.0, now);
        assert_eq!(t.phase(2, Started, (200.0, 130.0), 1.0, now), vec![Up(Left), Move(150.0, 130.0)]);
        // The middle moves down 20 px: past the slop, a pan.
        assert_eq!(t.phase(1, Moved, (100.0, 150.0), 1.0, now), vec![Move(150.0, 140.0), Wheel(0.0, 10.0)]);
        assert_eq!(t.phase(2, Moved, (200.0, 150.0), 1.0, now), vec![Move(150.0, 150.0), Wheel(0.0, 10.0)]);
        t.phase(1, Ended, (100.0, 150.0), 1.0, now);
        assert_eq!(t.phase(2, Moved, (200.0, 190.0), 1.0, now), vec![]);
        assert_eq!(t.phase(2, Ended, (200.0, 190.0), 1.0, now), vec![]);
        assert_eq!(t.phase(3, Started, (1.0, 1.0), 1.0, now), vec![Move(1.0, 1.0)]);
    }

    #[test]
    fn spreading_zooms() {
        let (mut t, now) = (Touches::default(), Instant::now());
        t.phase(1, Started, (100.0, 100.0), 1.0, now);
        t.phase(2, Started, (200.0, 100.0), 1.0, now);
        t.phase(1, Moved, (90.0, 100.0), 1.0, now);
        assert_eq!(t.phase(2, Moved, (210.0, 100.0), 1.0, now), vec![Zoom(10.0)]);
        assert_eq!(t.phase(2, Moved, (230.0, 100.0), 1.0, now), vec![Zoom(20.0)]);
    }
}
