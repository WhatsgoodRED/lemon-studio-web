// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Lemon Tune: pitch correction for vocals (MetroBar Studio). It hears the note being sung and moves it to the nearest
//! note of a key and scale: hard and robotic at retune speed 0, smooth and natural at slower speeds, the amount setting
//! scaling how far.
//!
//! The pitch is found with YIN (de Cheveigné & Kawahara 2002) on a copy of the input taken down to about 12 kHz, about
//! every 10 ms. The voice is moved with pitch-synchronous overlap-add (TD-PSOLA): grains two periods long, taken one
//! period apart from the input and laid down closer together (higher) or further apart (lower), so the voice keeps its
//! formants and nothing smears. Unvoiced sound (breaths, s and t) and silence pass as they are. The effect delays the
//! sound by a fixed 36 ms (at 48 kHz), which it reports to the host (`Sound::latency`) so the mixer keeps everything in time.
//!
//! Lemon Studio's own effect (no FL original): its saved state is the settings as little-endian ints.

use nih_plug::prelude::*;
use os_theme::design::lemon as brand;
use os_ui::kit::lemon::plug;
use os_ui::{Canvas, Color, Font, Rect};

use crate::{Card, Control, Display, Kind, Mark, Setting, SmallFx, Sound, Spec};

pub const KEY: usize = 0;
pub const SCALE: usize = 1;
pub const SPEED: usize = 2;
pub const AMOUNT: usize = 3;

pub const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];

/// The scales: name and the semitones above the key in it.
pub const SCALES: [(&str, &[u8]); 7] = [
    ("Chromatic", &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
    ("Major", &[0, 2, 4, 5, 7, 9, 11]),
    ("Minor", &[0, 2, 3, 5, 7, 8, 10]),
    ("Harmonic minor", &[0, 2, 3, 5, 7, 8, 11]),
    ("Major pentatonic", &[0, 2, 4, 7, 9]),
    ("Minor pentatonic", &[0, 3, 5, 7, 10]),
    ("Blues", &[0, 3, 5, 6, 7, 10]),
];
const SCALE_NAMES: [&str; 7] = ["Chromatic", "Major", "Minor", "Harmonic minor", "Major pentatonic", "Minor pentatonic", "Blues"];

/// Retune speed: setting v is v × 4 ms (0 = hard).
const SPEED_MS: f32 = 4.0;

pub const SETTINGS: [Setting; 4] = [
    Setting { id: "key", name: "Key", range: (0, 11), start: 0, magnet: None },
    Setting { id: "scale", name: "Scale", range: (0, 6), start: 0, magnet: None },
    Setting { id: "speed", name: "Retune speed", range: (0, 100), start: 5, magnet: None },
    Setting { id: "amount", name: "Amount", range: (0, 100), start: 100, magnet: None },
];

const CARDS: [Card; 3] = [
    Card { x0: 8.0, x1: 175.0, y0: 28.0, y1: 103.0, heading: "TUNE" },
    Card { x0: 182.0, x1: 311.0, y0: 28.0, y1: 171.0, heading: "SCALE" },
    Card { x0: 8.0, x1: 175.0, y0: 120.0, y1: 171.0, heading: "NOTES" },
];

/// The knobs show their value under them (the key's name, the speed, the amount) and their name over them.
const CONTROLS: [Control; 4] = [
    Control::Knob { id: "key", cx: 38.0, cy: 66.0, r: 15.0, caption: "", bipolar: false },
    Control::Knob { id: "speed", cx: 92.0, cy: 66.0, r: 15.0, caption: "", bipolar: false },
    Control::Knob { id: "amount", cx: 146.0, cy: 66.0, r: 15.0, caption: "", bipolar: false },
    Control::Choice { id: "scale", x: 192.0, top: 46.0, names: &SCALE_NAMES },
];

pub const SPEC: &Spec = &Spec {
    name: "Lemon Tune",
    word: "TUNE",
    about: "Lemon Studio's pitch correction",
    settings: &SETTINGS,
    version: None,
    any_version: false,
    fields: &[],
    tail: &[],
    blank: None,
    locate: None,
    size: (320, 214),
    cards: &CARDS,
    controls: &CONTROLS,
    display: Some(Display { r: Rect::new(16.0, 135.0, 152.0, 30.0), draw: keys }),
    mark: Mark { x: 311.0, y: 194.0, anchor: 1.0, stacked: false },
    hint: Some(Rect::new(8.0, 184.0, 180.0, 20.0)),
    labels: &[(38.0, 38.0, "KEY"), (92.0, 38.0, "SPEED"), (146.0, 38.0, "AMOUNT")],
    text,
    parse,
    sound,
    rings: false,
};

fn text(k: usize, v: i32) -> String {
    match k {
        KEY => NOTE_NAMES[v.clamp(0, 11) as usize].to_string(),
        SCALE => SCALE_NAMES[v.clamp(0, 6) as usize].to_string(),
        SPEED if v <= 0 => "Hard".into(),
        SPEED => format!("{} ms", v * SPEED_MS as i32),
        _ => format!("{v}%"),
    }
}

fn parse(k: usize, s: &str) -> Option<i32> {
    let s = s.trim();
    match k {
        KEY => NOTE_NAMES.iter().position(|n| n.eq_ignore_ascii_case(s)).map(|i| i as i32).or_else(|| match s.to_ascii_uppercase().as_str() {
            "DB" => Some(1),
            "D#" => Some(3),
            "GB" => Some(6),
            "G#" => Some(8),
            "A#" => Some(10),
            _ => None,
        }),
        SCALE => SCALE_NAMES.iter().position(|n| n.eq_ignore_ascii_case(s)).map(|i| i as i32),
        SPEED if s.eq_ignore_ascii_case("hard") => Some(0),
        SPEED => {
            let n: f32 = s.trim_end_matches("ms").trim().parse().ok()?;
            Some(((n / SPEED_MS).round() as i32).clamp(0, 100))
        }
        _ => {
            let n: f32 = s.trim_end_matches('%').trim().parse().ok()?;
            Some((n.round() as i32).clamp(0, 100))
        }
    }
}

/// Whether semitone `pc` (0 = C) is in the scale of the settings.
pub fn in_scale(v: &[i32], pc: i32) -> bool {
    let (key, scale) = (v[KEY].rem_euclid(12), v[SCALE].clamp(0, 6) as usize);
    SCALES[scale].1.contains(&((pc - key).rem_euclid(12) as u8))
}

/// The display: one octave as a small piano, the scale's notes lit and the key in lemon.
fn keys(c: &mut Canvas, r: Rect, v: &[i32]) {
    const WHITE: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    // Black keys by the white key they sit right of.
    const BLACK: [(usize, i32); 5] = [(0, 1), (1, 3), (3, 6), (4, 8), (5, 10)];
    let key = v[KEY].rem_euclid(12);
    let colour = |pc: i32, black: bool| {
        if pc == key {
            brand::MAIN
        } else if in_scale(v, pc) {
            if black { Color::rgb(0x9a9a9a) } else { Color::rgb(0xe8e8e8) }
        } else if black {
            Color::rgb(0x1a1a1a)
        } else {
            plug::GRID
        }
    };
    let w = r.w / 7.0;
    let f = Font::new(9).bold();
    for (i, &pc) in WHITE.iter().enumerate() {
        let x = (r.x + i as f32 * w).round();
        let cell = Rect::new(x, r.y, (w - 1.0).max(1.0), r.h);
        c.fill_rect(cell, colour(pc, false));
        let name = NOTE_NAMES[pc as usize];
        let ink = if in_scale(v, pc) { Color::rgb(0x202020) } else { Color::rgb(0x8a8a8a) };
        let tw = c.text_width(f, name);
        c.text(f, cell.x + (cell.w - tw) / 2.0, r.y + r.h - 12.0, ink, name);
    }
    for (i, pc) in BLACK {
        let bw = (w * 0.6).round();
        let x = (r.x + (i + 1) as f32 * w - bw / 2.0).round();
        c.fill_rect(Rect::new(x - 1.0, r.y, bw + 1.0, r.h * 0.55 + 1.0), Color::rgb(0x101010));
        c.fill_rect(Rect::new(x, r.y, bw - 1.0, r.h * 0.55), colour(pc, true));
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The sound.

/// The rate the pitch is looked for at (about).
const DETECT_RATE: f32 = 12_000.0;
/// Lowest and highest pitch looked for (Hz): a low male voice to a high female one.
const F_MIN: f32 = 75.0;
const F_MAX: f32 = 1000.0;
/// YIN's threshold on the cumulative mean normalised difference.
const YIN_THRESHOLD: f32 = 0.15;
/// Below this level (RMS) nothing is corrected.
const GATE: f32 = 0.003;
/// A biquad low-pass (RBJ), for the detection copy: what is above the 12 kHz copy's band would fold into it.
#[derive(Clone, Copy, Default)]
struct Lp {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Lp {
    fn new(rate: f32, hz: f32) -> Lp {
        let w = 2.0 * std::f32::consts::PI * hz / rate;
        let (sn, cs) = w.sin_cos();
        let alpha = sn / (2.0 * std::f32::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        let b1 = (1.0 - cs) / a0;
        Lp { b0: b1 / 2.0, b1, b2: b1 / 2.0, a1: -2.0 * cs / a0, a2: (1.0 - alpha) / a0, z1: 0.0, z2: 0.0 }
    }

    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

pub struct Tune {
    rate: f32,
    v: [i32; 4],
    // Detection: input taken down by `q`, the last `win` samples.
    q: usize,
    dec_sum: f32,
    dec_n: usize,
    dec: Vec<f32>,
    dec_at: usize,
    dec_count: usize,
    hop: usize,
    win: usize,
    yin: Vec<f32>,
    /// The span YIN looks at, copied in order out of the ring.
    lin: Vec<f32>,
    /// How aperiodic the last reading was (YIN's dip, 0 = perfectly periodic): tests measure roughness with it.
    dip: f32,
    /// A reading far from the last one, waiting for the next to agree (a consonant's stray reading is not followed).
    pending: Option<f32>,
    // The voice now: its period (samples), whether it is voiced, the note it is held to (MIDI), the shift (semitones).
    period: f32,
    voiced: bool,
    /// Hops since the last pitched one: short gaps (a reading lost in a consonant) keep the voice as it was.
    unpitched: u32,
    note: Option<f32>,
    shift: f32,
    /// The pitch ratio for the shift (2^(shift/12)), worked out once a hop.
    ratio: f64,
    target: f32,
    /// Two low-passes before the detection copy is taken down.
    lp: [Lp; 2],
    // PSOLA: input rings (a power of two long, sized from the delay at the sample rate), the overlap-add rings and their window sums; positions are absolute sample counts.
    in_l: Vec<f32>,
    in_r: Vec<f32>,
    acc_l: Vec<f32>,
    acc_r: Vec<f32>,
    acc_w: Vec<f32>,
    n: u64,
    delay: u64,
    /// How far behind the input a grain is laid down: where the last pitch reading is centred (so a gliding voice is
    /// corrected with its own pitch, not the pitch of 20 ms earlier).
    lead: f64,
    syn: f64,
    ana: f64,
}

impl Tune {
    pub fn new(rate: u32, v: &[i32]) -> Tune {
        let mut t = Tune {
            rate: 0.0,
            v: [0, 0, 5, 100],
            q: 1,
            dec_sum: 0.0,
            dec_n: 0,
            dec: Vec::new(),
            dec_at: 0,
            dec_count: 0,
            hop: 1,
            win: 1,
            yin: Vec::new(),
            lin: Vec::new(),
            dip: 1.0,
            pending: None,
            period: 0.0,
            voiced: false,
            unpitched: 0,
            note: None,
            shift: 0.0,
            ratio: 1.0,
            target: 0.0,
            lp: [Lp::default(); 2],
            in_l: Vec::new(),
            in_r: Vec::new(),
            acc_l: Vec::new(),
            acc_r: Vec::new(),
            acc_w: Vec::new(),
            n: 0,
            delay: 0,
            lead: 0.0,
            syn: 0.0,
            ana: 0.0,
        };
        t.set_settings(v);
        t.set_rate(rate);
        t
    }

    fn set_settings(&mut self, v: &[i32]) {
        for (k, d) in self.v.iter_mut().enumerate() {
            if let Some(&x) = v.get(k) {
                *d = x;
            }
        }
    }

    fn set_rate(&mut self, rate: u32) {
        let rate = rate.max(8000) as f32;
        if rate == self.rate {
            return;
        }
        self.rate = rate;
        self.q = (rate / DETECT_RATE).round().max(1.0) as usize;
        let dr = rate / self.q as f32;
        let tau_max = (dr / F_MIN).ceil() as usize;
        self.win = (tau_max * 2).next_power_of_two().max(256);
        self.dec = vec![0.0; self.win];
        self.yin = vec![0.0; tau_max + 2];
        self.lin = vec![0.0; self.win / 2 + tau_max];
        // A reading every 5 ms: the shift follows a gliding voice closely.
        self.hop = ((dr * 0.005) as usize).max(8);
        // YIN looks at the newest `win / 2 + tau_max` samples, every lag centred on their middle (`detect`): the reading
        // is about that middle, and it is used for up to one hop after it is made.
        let span = self.win / 2 + tau_max;
        self.lead = ((span / 2 + self.hop / 2) * self.q) as f64;
        // A grain reaches one period past its mark: the output waits for the last one.
        self.delay = (self.lead + f64::from(rate / F_MIN)).ceil() as u64 + 16;
        // The rings hold the delay and a grain's reach behind it, at any sample rate.
        let ring = (self.delay as usize + 4 * (rate / F_MIN) as usize + 64).next_power_of_two();
        for b in [&mut self.in_l, &mut self.in_r, &mut self.acc_l, &mut self.acc_r, &mut self.acc_w] {
            *b = vec![0.0; ring];
        }
        // Flat to 3.5 kHz (a voice's pitch and its lower formants), steep above: little folds into the 6 kHz band.
        self.lp = [Lp::new(rate, 3500.0); 2];
        self.clear();
    }

    fn clear(&mut self) {
        for b in [&mut self.in_l, &mut self.in_r, &mut self.acc_l, &mut self.acc_r, &mut self.acc_w] {
            b.iter_mut().for_each(|x| *x = 0.0);
        }
        self.dec.iter_mut().for_each(|x| *x = 0.0);
        for f in &mut self.lp {
            (f.z1, f.z2) = (0.0, 0.0);
        }
        (self.dec_sum, self.dec_n, self.dec_at, self.dec_count) = (0.0, 0, 0, 0);
        self.period = self.rate / 200.0;
        (self.voiced, self.note, self.shift, self.target, self.ratio, self.pending) = (false, None, 0.0, 0.0, 1.0, None);
        self.n = 0;
        self.syn = 0.0;
        self.ana = 0.0;
    }

    /// The sound's delay (samples).
    pub fn latency(&self) -> u32 {
        self.delay as u32
    }

    /// The detected pitch now (Hz), for tests.
    pub fn pitch(&self) -> Option<f32> {
        self.voiced.then(|| self.rate / self.period)
    }

    /// The note the voice goes to for detected note `m` (MIDI, fractional), held while it stays near (no flutter
    /// between two notes when the voice sits halfway).
    fn target_note(&mut self, m: f32) -> f32 {
        if let Some(n) = self.note {
            if (m - n).abs() < 0.65 && in_scale(&self.v, n as i32) {
                return n;
            }
        }
        let base = m.round() as i32;
        let best = (-6..=6).map(|d| base + d).filter(|&n| in_scale(&self.v, n)).min_by(|a, b| (*a as f32 - m).abs().total_cmp(&(*b as f32 - m).abs()));
        let n = best.unwrap_or(base) as f32;
        self.note = Some(n);
        n
    }

    /// YIN on the last `win` detection samples: the period in input samples, or `None` when nothing is pitched.
    fn detect(&mut self) -> Option<f32> {
        let w = self.win;
        let half = w / 2;
        let tau_max = (self.yin.len() - 2).min(half - 1);
        let dr = self.rate / self.q as f32;
        let tau_min = ((dr / F_MAX).floor() as usize).max(2);
        // The newest `half + tau_max` samples, oldest first (the ring's oldest is at `dec_at`).
        let span = half + tau_max;
        let base = self.dec_at + w - span;
        for (i, v) in self.lin.iter_mut().enumerate().take(span) {
            *v = self.dec[(base + i) % w];
        }
        let x = &self.lin[..span];
        let energy: f32 = x.iter().map(|v| v * v).sum();
        if (energy / span as f32).sqrt() < GATE {
            return None;
        }
        let d = &mut self.yin;
        d[0] = 1.0;
        let mut sum = 0.0;
        for tau in 1..=tau_max {
            // Every lag compares the same stretch, centred on the span's middle (the reading's time does not move with
            // the pitch).
            let off = (tau_max - tau) / 2;
            let s: f32 = x[off..off + half].iter().zip(&x[off + tau..off + tau + half]).map(|(a, b)| (a - b) * (a - b)).sum();
            sum += s;
            d[tau] = if sum > 0.0 { s * tau as f32 / sum } else { 1.0 };
        }
        let mut tau = tau_min;
        let first = loop {
            if tau >= tau_max {
                break None;
            }
            if d[tau] < YIN_THRESHOLD {
                while tau + 1 < tau_max && d[tau + 1] < d[tau] {
                    tau += 1;
                }
                break Some(tau);
            }
            tau += 1;
        };
        // Inside a sung note a breathy or raspy stretch may not dip under the threshold: its deepest dip still counts
        // when it is clear enough.
        let found = first.or_else(|| {
            if !self.voiced {
                return None;
            }
            let (t, v) = (tau_min..tau_max).map(|t| (t, d[t])).min_by(|a, b| a.1.total_cmp(&b.1))?;
            (v < 0.3).then_some(t)
        })?;
        self.dip = d[found];
        // Parabolic interpolation around the dip.
        let (a, b, c) = (d[found - 1], d[found], d[found + 1]);
        let den = a - 2.0 * b + c;
        let off = if den.abs() > 1e-9 { 0.5 * (a - c) / den } else { 0.0 };
        Some((found as f32 + off.clamp(-1.0, 1.0)) * self.q as f32)
    }

    /// A new pitch estimate (every hop): the period, and where the correction heads.
    fn analyse(&mut self) {
        let hop_s = self.hop as f32 * self.q as f32 / self.rate;
        match self.detect() {
            // A jump of more than 2 semitones from the voice's last reading waits for the next reading to agree.
            Some(p) if self.voiced && (12.0 * (self.period / p).log2()).abs() > 2.0 && self.pending.is_none_or(|q| (12.0 * (q / p).log2()).abs() > 1.0) => {
                self.pending = Some(p);
            }
            Some(p) => {
                self.pending = None;
                self.period = p;
                self.voiced = true;
                self.unpitched = 0;
                let m = 69.0 + 12.0 * (self.rate / p / 440.0).log2();
                let n = self.target_note(m);
                self.target = (n - m) * self.v[AMOUNT].clamp(0, 100) as f32 / 100.0;
            }
            // Up to 8 hops (40 ms) without a reading inside a sung note: keep going as before.
            None if self.voiced && self.unpitched < 8 => self.unpitched += 1,
            None => {
                self.pending = None;
                self.voiced = false;
                self.note = None;
                self.target = 0.0;
            }
        }
        let tau = self.v[SPEED].clamp(0, 100) as f32 * SPEED_MS / 1000.0;
        if tau <= 0.0 || !self.voiced {
            self.shift = self.target;
        } else {
            self.shift += (self.target - self.shift) * (1.0 - (-hop_s / tau).exp());
        }
        self.ratio = f64::from(2f32.powf(self.shift / 12.0));
    }

    /// One grain at synthesis mark `os`, taken from the analysis mark nearest it.
    fn grain(&mut self, os: f64, t: f64) {
        // Nothing to correct (unvoiced, amount 0): grains are taken where they are laid, so the output is the input
        // itself and a note starts from it in phase (no comb from mixing two copies).
        if !self.voiced || self.v[AMOUNT] <= 0 {
            self.ana = os;
        }
        while self.ana + t * 0.5 < os {
            self.ana += t;
        }
        if self.ana > os + t {
            self.ana = os;
        }
        let (a, s, ti) = (self.ana.round() as i64, os.round() as i64, t.round().max(2.0) as i64);
        let m = self.in_l.len() as i64 - 1;
        for k in -ti..=ti {
            let w = 0.5 * (1.0 + (std::f32::consts::PI * k as f32 / ti as f32).cos());
            let src = ((a + k) & m) as usize;
            let dst = ((s + k) & m) as usize;
            self.acc_l[dst] += w * self.in_l[src];
            self.acc_r[dst] += w * self.in_r[src];
            self.acc_w[dst] += w;
        }
    }

    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let m = self.in_l.len() - 1;
        for i in 0..left.len().min(right.len()) {
            let (l, r) = (left[i], right[i]);
            let at = (self.n as usize) & m;
            self.in_l[at] = l;
            self.in_r[at] = r;
            // Detection copy.
            let lo = self.lp[0].run(0.5 * (l + r));
            let lo = self.lp[1].run(lo);
            self.dec_sum += lo;
            self.dec_n += 1;
            if self.dec_n == self.q {
                self.dec[self.dec_at] = self.dec_sum / self.q as f32;
                self.dec_at = (self.dec_at + 1) % self.win;
                (self.dec_sum, self.dec_n) = (0.0, 0);
                self.dec_count += 1;
                if self.dec_count % self.hop == 0 {
                    self.analyse();
                }
            }
            // Grains whose input has arrived.
            let now = self.n as f64;
            let t = f64::from(self.period.max(2.0));
            let earliest = now - self.delay as f64 + t + 1.0;
            if self.syn < earliest {
                self.syn = earliest;
                self.ana = earliest;
            }
            let step = t / self.ratio;
            let lead = self.lead.max(1.5 * t);
            while self.syn + lead <= now {
                let os = self.syn;
                self.grain(os, t);
                self.syn += step;
            }
            // The output, `delay` behind.
            let o = (self.n.wrapping_sub(self.delay) as usize) & m;
            let w = self.acc_w[o];
            let (dl, dr) = (self.in_l[o], self.in_r[o]);
            let (wl, wr) = if w > 0.05 { (self.acc_l[o] / w, self.acc_r[o] / w) } else { (dl, dr) };
            (self.acc_l[o], self.acc_r[o], self.acc_w[o]) = (0.0, 0.0, 0.0);
            if self.n < self.delay {
                left[i] = 0.0;
                right[i] = 0.0;
            } else {
                left[i] = wl;
                right[i] = wr;
            }
            self.n += 1;
        }
    }
}

impl Sound for Tune {
    /// New settings (the host gives them every block): the held note stays; `target_note` lets go of it when it
    /// leaves the scale.
    fn set(&mut self, v: &[i32]) {
        self.set_settings(v);
    }
    fn set_sample_rate(&mut self, rate: u32) {
        self.set_rate(rate);
    }
    fn reset(&mut self) {
        self.clear();
    }
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.process_stereo(left, right);
    }
    fn latency(&self) -> u32 {
        Tune::latency(self)
    }
}

fn sound(rate: u32, v: &[i32]) -> Box<dyn Sound> {
    Box::new(Tune::new(rate, v))
}

pub struct TuneKind;

impl Kind for TuneKind {
    const SPEC: &'static Spec = SPEC;
    const CLAP_ID: &'static str = "org.openstudio.lemon-tune";
    const CLAP_DESCRIPTION: &'static str = "Lemon Studio's pitch correction";
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::PitchCorrection, ClapFeature::Stereo];
    const VST3_CLASS_ID: [u8; 16] = *b"LemonStudioTune1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx, Vst3SubCategory::PitchShift];
}

pub type LemonTune = SmallFx<TuneKind>;

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// A sung-like tone: a sawtooth softened (harmonics falling at 1/n²), `secs` long at `hz`.
    fn voice(hz: f32, secs: f32) -> Vec<f32> {
        let n = (secs * RATE as f32) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                (1..12).map(|h| (2.0 * std::f32::consts::PI * hz * h as f32 * t).sin() / (h * h) as f32).sum::<f32>() * 0.3
            })
            .collect()
    }

    /// The pitch of `x` measured by zero crossings of its fundamental over the second half (Hz).
    fn measure(x: &[f32]) -> f32 {
        let mut t = Tune::new(RATE, &[0, 0, 0, 0]);
        let mut l = x.to_vec();
        let mut r = x.to_vec();
        t.process_stereo(&mut l, &mut r);
        t.pitch().unwrap_or(0.0)
    }

    fn run(v: [i32; 4], x: &[f32]) -> Vec<f32> {
        let mut t = Tune::new(RATE, &v);
        let mut l = x.to_vec();
        let mut r = x.to_vec();
        for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            t.process_stereo(cl, cr);
        }
        l
    }

    #[test]
    fn hears_the_pitch() {
        for hz in [98.0, 220.0, 452.0, 700.0] {
            let p = measure(&voice(hz, 0.3));
            assert!((p / hz - 1.0).abs() < 0.01, "{hz} Hz heard as {p}");
        }
    }

    #[test]
    fn moves_a_flat_note_to_the_scale() {
        // 452 Hz is A4 + 47 cents; in C major it goes to A4 (440 Hz), hard.
        let out = run([0, 1, 0, 100], &voice(452.0, 1.0));
        let p = measure(&out[24_000..]);
        assert!((p - 440.0).abs() < 4.0, "corrected to {p}");
        // Amount 0 leaves it where it was.
        let out = run([0, 1, 0, 0], &voice(452.0, 1.0));
        let p = measure(&out[24_000..]);
        assert!((p - 452.0).abs() < 4.0, "left at {p}");
        // A note out of the scale goes to its nearest neighbour in it: 466 Hz (Bb4) in C major goes to B4 or A4.
        let out = run([0, 1, 0, 100], &voice(470.0, 1.0));
        let p = measure(&out[24_000..]);
        assert!((p - 493.9).abs() < 5.0 || (p - 440.0).abs() < 5.0, "out-of-scale note went to {p}");
    }

    #[test]
    fn an_in_tune_note_passes_unchanged() {
        // A4 in A minor: nothing to correct. The grains are laid one period apart as they were taken, so the output is
        // the input (shifted by up to half a period, the analysis marks' phase): same pitch, same level, same shape.
        let x = voice(440.0, 0.6);
        let out = run([9, 2, 0, 100], &x);
        let d = Tune::new(RATE, &[0; 4]).latency() as i64;
        let p = measure(&out[14_400..]);
        assert!((p - 440.0).abs() < 2.0, "pitch {p}");
        let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
        let (a, b) = (rms(&out[20_000..26_000]), rms(&x[20_000..26_000]));
        assert!((a / b - 1.0).abs() < 0.05, "level {a} vs {b}");
        let best = (-60..=60)
            .map(|lag| {
                let e: f32 = (20_000..21_000).map(|i| (out[i] - x[(i as i64 - d + lag) as usize]).powi(2)).sum();
                (e / 1000.0).sqrt() / b
            })
            .fold(f32::MAX, f32::min);
        assert!(best < 0.05, "shape error {best}");
    }

    #[test]
    fn silence_stays_silent_and_values_read() {
        let out = run([0, 0, 10, 100], &vec![0.0; 9600]);
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(text(KEY, 9), "A");
        assert_eq!(text(SPEED, 0), "Hard");
        assert_eq!(text(SPEED, 10), "40 ms");
        assert_eq!(parse(KEY, "a#"), Some(10));
        assert_eq!(parse(SCALE, "minor"), Some(2));
        assert!(in_scale(&[9, 2, 0, 0], 0) && !in_scale(&[9, 2, 0, 0], 1));
    }

    /// Cents from the nearest semitone of every voiced 10 ms hop of `x` (mono), as the tuner hears it.
    fn cents_off(x: &[f32]) -> Vec<f32> {
        cents_and_dips(x).0
    }

    /// `cents_off`, and the YIN dip of every hop with a reading (lower = cleaner, more periodic).
    fn cents_and_dips(x: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let mut t = Tune::new(RATE, &[0, 0, 0, 0]);
        let (mut out, mut dips) = (Vec::new(), Vec::new());
        for c in x.chunks(240) {
            let (mut l, mut r) = (c.to_vec(), c.to_vec());
            t.process_stereo(&mut l, &mut r);
            if let Some(p) = t.pitch() {
                let m = 69.0 + 12.0 * (p / 440.0).log2();
                out.push((m - m.round()) * 100.0);
                dips.push(t.dip);
            }
        }
        (out, dips)
    }

    /// A real vocal: `LEMON_TUNE_IN` (raw f32 stereo at 48 kHz) tuned with `LEMON_TUNE_SET` ("key,scale,speed,amount")
    /// into `LEMON_TUNE_OUT`, with how far off the notes sit before and after.
    #[test]
    #[ignore]
    fn real_vocal() {
        let path = std::env::var("LEMON_TUNE_IN").expect("LEMON_TUNE_IN");
        let set: Vec<i32> = std::env::var("LEMON_TUNE_SET").unwrap_or("0,0,0,100".into()).split(',').map(|v| v.parse().unwrap()).collect();
        let raw = std::fs::read(&path).unwrap();
        let f: Vec<f32> = raw.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let (mut l, mut r): (Vec<f32>, Vec<f32>) = (f.iter().step_by(2).copied().collect(), f.iter().skip(1).step_by(2).copied().collect());
        let mono_in: Vec<f32> = l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect();
        let mut t = Tune::new(RATE, &set);
        let started = std::time::Instant::now();
        for (cl, cr) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
            t.process_stereo(cl, cr);
        }
        let took = started.elapsed().as_secs_f32();
        let d = t.latency() as usize;
        let mono_out: Vec<f32> = l.iter().zip(&r).skip(d).map(|(a, b)| 0.5 * (a + b)).collect();
        let stats = |v: &[f32]| {
            let mean = v.iter().map(|c| c.abs()).sum::<f32>() / v.len().max(1) as f32;
            let within = v.iter().filter(|c| c.abs() < 10.0).count() as f32 / v.len().max(1) as f32 * 100.0;
            (v.len(), mean, within)
        };
        eprintln!("settings {set:?}; {:.1} s of audio processed in {took:.2} s", mono_in.len() as f32 / RATE as f32);
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
        let (ci, di) = cents_and_dips(&mono_in);
        let (co, d_o) = cents_and_dips(&mono_out);
        eprintln!("before: voiced hops, mean cents off, % within 10 cents = {:?}; roughness {:.4}", stats(&ci), mean(&di));
        eprintln!("after:  voiced hops, mean cents off, % within 10 cents = {:?}; roughness {:.4}", stats(&co), mean(&d_o));
        if let Ok(out) = std::env::var("LEMON_TUNE_OUT") {
            let bytes: Vec<u8> = l.iter().zip(&r).flat_map(|(a, b)| a.to_le_bytes().into_iter().chain(b.to_le_bytes())).collect();
            std::fs::write(out, bytes).unwrap();
        }
    }
}
