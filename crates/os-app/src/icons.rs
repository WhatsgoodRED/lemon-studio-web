// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Toolbar and window-button icons, and the Lemon Studio logo. Placeholders taken from Tabler Icons (MIT, see `tabler.rs`);
//! each only suggests its function (no artwork is copied from any other program). Where Tabler has nothing that fits,
//! Lemon Studio draws its own on the same grid (`KICK`).

use os_ui::{Canvas, Color, Glyph, Rect};

use os_ui::tabler::{self as tb, TablerIcon};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Minimize,
    Restore,
    Close,
    Play,
    Stop,
    Metronome,
    WaitInput,
    Countdown,
    Blend,
    LoopRecord,
    Undo,
    /// `Undo` mirrored: an undone step of the undo history (the browser's History).
    Redo,
    Cut,
    Mic,
    Help,
    Save,
    SaveRender,
    Info,
    TypingKeyboard,
    AutoScroll,
    StepEdit,
    Slide,
    Link,
    MidiControl,
    Magnet,
    Playlist,
    PianoRoll,
    ChannelRack,
    Mixer,
    Browser,
    Project,
    Plug,
    Kick,
    Lock,
    Touch,
    ArrowRight,
    ArrowLeft,
    ArrowUp,
    Back,
    Wave,
    Page,
    Star,
    Folder,
    Search,
    Pencil,
    Brush,
    Delete,
    Mute,
    Slip,
    Slice,
    Select,
    Zoom,
    Speaker,
    Plus,
    Note,
    FolderOpen,
    FolderPlus,
    FileAudio,
    FileProject,
    FileText,
    FilePreset,
    FileZip,
    FileBlank,
}

/// A kick drum seen from the side on its two legs, its pedal's beater against the head (the Tempo tapper). Lemon
/// Studio's own, in Tabler's style: 24-unit grid, outline, absolute M/L/C/Z only.
const KICK: TablerIcon = TablerIcon { filled: false, d: "M 13.5 10.5 C 13.5 14.366 12.381 17.5 11 17.5 C 9.619 17.5 8.5 14.366 8.5 10.5 C 8.5 6.634 9.619 3.5 11 3.5 C 12.381 3.5 13.5 6.634 13.5 10.5 Z M 11 3.5 L 19 3.5 M 11 17.5 L 19 17.5 M 19 3.5 C 20.381 3.5 21.5 6.634 21.5 10.5 C 21.5 14.366 20.381 17.5 19 17.5 M 3 21 L 8 19.5 M 4.8 20.5 L 6.4 12.6 M 8.3 10.8 C 8.3 11.628 7.628 12.3 6.8 12.3 C 5.972 12.3 5.3 11.628 5.3 10.8 C 5.3 9.972 5.972 9.3 6.8 9.3 C 7.628 9.3 8.3 9.972 8.3 10.8 Z M 12 17.3 L 11 21 M 18 17.3 L 19 21" };

/// Tabler's `arrow-back-up` mirrored left to right (an undone step, spec: ui/settings-choices.md "Undo history: the
/// four checks (2026-10-16)").
const REDO: TablerIcon = TablerIcon { filled: false, d: "M 15 14 L 19 10 L 15 6 M 19 10 L 8 10 C 5.791 10 4 11.791 4 14 C 4 16.209 5.791 18 8 18 L 9 18" };

/// A padlock, Lemon Studio's own on Tabler's grid: a channel locked to a note source (spec: ui/channel-rack.md §18).
const LOCK: TablerIcon = TablerIcon { filled: false, d: "M 6 11 L 18 11 C 18.552 11 19 11.448 19 12 L 19 20 C 19 20.552 18.552 21 18 21 L 6 21 C 5.448 21 5 20.552 5 20 L 5 12 C 5 11.448 5.448 11 6 11 Z M 8 11 L 8 7 C 8 4.791 9.791 3 12 3 C 14.209 3 16 4.791 16 7 L 16 11 M 12 15 L 12 17" };

fn shape(icon: Icon) -> &'static TablerIcon {
    match icon {
        Icon::Minimize => &tb::MINUS,
        Icon::Restore => &tb::SQUARES,
        Icon::Close => &tb::X,
        Icon::Play => &tb::PLAYER_PLAY,
        Icon::Stop => &tb::PLAYER_STOP,
        Icon::Metronome => &tb::METRONOME,
        Icon::WaitInput => &tb::HOURGLASS,
        Icon::Countdown => &tb::STOPWATCH,
        Icon::Blend => &tb::BLEND_MODE,
        Icon::LoopRecord => &tb::REPEAT,
        Icon::Undo => &tb::ARROW_BACK_UP,
        Icon::Redo => &REDO,
        Icon::Cut => &tb::CUT,
        Icon::Mic => &tb::MICROPHONE,
        Icon::Help => &tb::HELP,
        Icon::Save => &tb::DEVICE_FLOPPY,
        Icon::SaveRender => &tb::FILE_EXPORT,
        Icon::Info => &tb::INFO_CIRCLE,
        Icon::TypingKeyboard => &tb::KEYBOARD,
        Icon::AutoScroll => &tb::ARROW_AUTOFIT_RIGHT,
        Icon::StepEdit => &tb::GRID_DOTS,
        Icon::Slide => &tb::TRENDING_UP,
        Icon::Link => &tb::LINK,
        Icon::MidiControl => &tb::ADJUSTMENTS_HORIZONTAL,
        Icon::Magnet => &tb::MAGNET,
        Icon::Playlist => &tb::PLAYLIST,
        Icon::PianoRoll => &tb::PIANO,
        Icon::ChannelRack => &tb::LAYOUT_ROWS,
        Icon::Mixer => &tb::ADJUSTMENTS,
        Icon::Browser => &tb::FOLDERS,
        Icon::Project => &tb::FILE,
        Icon::Page => &tb::FILE_TEXT,
        Icon::Plug => &tb::PLUG,
        Icon::Kick => &KICK,
        Icon::Lock => &LOCK,
        Icon::Touch => &tb::HAND_FINGER,
        Icon::ArrowRight => &tb::CARET_RIGHT,
        Icon::ArrowLeft => &tb::CARET_LEFT,
        Icon::ArrowUp => &tb::CARET_UP,
        Icon::Back => &tb::ARROW_LEFT,
        Icon::Wave => &tb::WAVE_SINE,
        Icon::Star => &tb::STAR,
        Icon::Folder => &tb::FOLDER,
        Icon::Search => &tb::SEARCH,
        Icon::Pencil => &tb::PENCIL,
        Icon::Brush => &tb::BRUSH,
        Icon::Delete => &tb::ERASER,
        Icon::Mute => &tb::VOLUME_OFF,
        Icon::Slip => &tb::ARROWS_HORIZONTAL,
        Icon::Slice => &tb::SLICE,
        Icon::Select => &tb::MARQUEE_2,
        Icon::Zoom => &tb::ZOOM_IN,
        Icon::Speaker => &tb::VOLUME,
        Icon::Plus => &tb::PLUS,
        Icon::Note => &tb::MUSIC,
        Icon::FolderOpen => &tb::FOLDER_OPEN,
        Icon::FolderPlus => &tb::FOLDER_PLUS,
        Icon::FileAudio => &tb::FILE_MUSIC,
        Icon::FileProject => &tb::FILE_STAR,
        Icon::FileText => &tb::FILE_TYPOGRAPHY,
        Icon::FilePreset => &tb::FILE_SETTINGS,
        Icon::FileZip => &tb::FILE_ZIP,
        Icon::FileBlank => &tb::FILE,
    }
}

/// Draw `icon` centered in `r` with color `c`. The 24-unit Tabler grid is fitted to the shorter side.
pub fn draw(cv: &mut Canvas, icon: Icon, r: Rect, c: Color) {
    cv.icon(shape(icon), r, c);
}

/// The MetroBar Studio wordmark (`wordmark.rs`), `h` px tall (its capitals), left edge at `x`, vertically centred on
/// `cy`: "METROBAR" in `fg`, "STUDIO" in `sub`. Returns its width.
pub fn wordmark(cv: &mut Canvas, x: f32, cy: f32, h: f32, fg: Color, sub: Color) -> f32 {
    use crate::wordmark as wm;
    let k = h / wm::CAP;
    let (ox, oy) = (x - wm::X0 * k, cy - wm::CAP * k / 2.0);
    let mut a = Glyph::new();
    a.commands(wm::METROBAR, ox, oy, k, None);
    cv.glyph(fg, &a);
    let mut b = Glyph::new();
    b.commands(wm::STUDIO, ox, oy, k, None);
    cv.glyph(sub, &b);
    wm::W * k
}

/// Width of `wordmark` at capital height `h`.
pub fn wordmark_w(h: f32) -> f32 {
    crate::wordmark::W * h / crate::wordmark::CAP
}

/// MetroBar's metronome (metrobarz.com's "pyramid metronome drawn like a line sketch", `.metro` in
/// landing-beat-meter.html): ink outlines on a pale case, seen straight on, the arm upright on its low pivot with the
/// sliding weight, a winding key on the side and the diamond badge lit red. Fitted so its height is `2.2 r`, centred on
/// (`cx`, `cy`): outlines in `ink`, the case in `case`, pivot and badge in `accent`.
pub fn metronome(cv: &mut Canvas, cx: f32, cy: f32, r: f32, ink: Color, case: Color, accent: Color) {
    // The page's drawing: viewBox 0 0 140 212, ink from y 3 (the cap's peak) to 208 (the feet), x 24..118 (the key).
    let k = 2.2 * r / 205.0;
    let (ox, oy) = (cx - 71.0 * k, cy - 105.5 * k);
    // The page draws 1.1 px outlines at any size (non-scaling strokes): about a pixel, a little more on big drawings.
    let line = (1.1 * k * 1.43).max(1.0);
    let face = case.mix(ink, 0.08);
    let outline = |cv: &mut Canvas, d: &str, fill: Color, w: f32| {
        let mut g = Glyph::new();
        g.commands(d, ox, oy, k, None);
        cv.glyph(fill, &g);
        let mut g = Glyph::new();
        g.commands(d, ox, oy, k, Some(w));
        cv.glyph(ink, &g);
    };
    let stroke = |cv: &mut Canvas, d: &str, c: Color, w: f32| {
        let mut g = Glyph::new();
        g.commands(d, ox, oy, k, Some(w));
        cv.glyph(c, &g);
    };
    // Feet, plinth with its trim line, body, face, tempo strip.
    let mut feet = Glyph::new();
    feet.commands("M 34 203 L 46 203 L 46 208 L 34 208 Z M 94 203 L 106 203 L 106 208 L 94 208 Z", ox, oy, k, None);
    cv.glyph(ink, &feet);
    outline(cv, "M 34 141 L 106 141 L 116 204 L 24 204 Z", case, line);
    stroke(cv, "M 29 198 L 111 198", ink, line * 0.75);
    outline(cv, "M 40 138 L 100 138 L 84 24 L 56 24 Z", case, line);
    outline(cv, "M 48 132 L 92 132 L 80 34 L 60 34 Z", face, line * 0.85);
    outline(cv, "M 63 36 L 77 36 L 77 130 L 63 130 Z", case, line * 0.75);
    if r >= 20.0 {
        // The tempo scale, where there is room for it.
        stroke(cv, "M 66 44 L 74 44 M 64 53.1 L 76 53.1 M 66 62.3 L 74 62.3 M 64 71.4 L 76 71.4 M 66 80.6 L 74 80.6 M 64 89.7 L 76 89.7 M 66 98.9 L 74 98.9 M 64 108 L 76 108", case.mix(ink, 0.55), line * 0.7);
    }
    // Cap, lip, the slot the arm swings in.
    outline(cv, "M 54 24 L 86 24 L 80 9 L 60 9 Z", case, line);
    stroke(cv, "M 60 9 L 70 3 L 80 9", ink, line * 0.75);
    outline(cv, "M 31 146 L 109 146 L 107 138 L 33 138 Z", case, line);
    stroke(cv, "M 52 141.5 L 88 141.5", ink, (2.4 * k * 1.43).max(1.5));
    // The arm, upright, with its weight (the page's `translate(0 70)`) and two screws.
    outline(cv, "M 67.6 136 L 72.4 136 L 71.6 20 L 68.4 20 Z", case, line);
    outline(cv, "M 60 63 L 80 63 L 76 77 L 64 77 Z", case, line);
    let dot = (2.0 * k).max(0.9);
    cv.fill_circle(ox + 66.5 * k, oy + 68.5 * k, dot, ink);
    cv.fill_circle(ox + 73.5 * k, oy + 68.5 * k, dot, ink);
    cv.fill_circle(ox + 70.0 * k, oy + 136.0 * k, (2.6 * k).max(1.2), accent);
    // The badge, lit as on the tick, with its wheel.
    outline(cv, "M 70 160 L 84 170 L 70 180 L 56 170 Z", accent, line);
    if r >= 14.0 {
        cv.stroke_circle(ox + 70.0 * k, oy + 170.0 * k, 4.4 * k, line * 0.75, case);
    }
    // The winding key.
    stroke(cv, "M 96 112 L 102 112", ink, (1.6 * k * 1.43).max(1.0));
    cv.fill_circle(ox + 102.0 * k, oy + 112.0 * k, (2.8 * k).max(1.0), ink);
    outline(cv, "M 104 112 L 109 104 L 118 102 L 116 112 L 118 122 L 109 120 Z", case, line);
}

/// Lemon Studio logo (greyscale mark, `logo.rs`) in one tint, fitted into a circle of radius `r`.
#[allow(dead_code)]
pub fn logo(cv: &mut Canvas, cx: f32, cy: f32, r: f32, c: Color) {
    use crate::logo::{PATHS, VIEW_H, VIEW_W};
    let k = 2.0 * r / VIEW_W.max(VIEW_H);
    let (ox, oy) = (cx - VIEW_W * k / 2.0, cy - VIEW_H * k / 2.0);
    let mut g = Glyph::new();
    for d in PATHS {
        g.commands(d, ox, oy, k, None);
    }
    cv.glyph(c, &g);
}
