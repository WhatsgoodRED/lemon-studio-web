// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Skins: the shapes and base colours of a look over the same layout. A theme (`.flstheme`) colours whichever skin is
//! in use.
//!
//! `Citrus` (formerly Classic) is the measured default look. `Lemon` is Lemon Studio's own look: neutral greys, one
//! lemon-yellow brand colour and rounded-square controls (knobs stay round). It re-colours every token and every
//! role-mapped literal (`Palette::s/a/h/tx/led`) through a `DesignMap` (a neutral lightness ramp plus key colours), sets
//! the tokens the map can't get right outright, and tells the widget kit which shape language to draw (`Style`). A
//! theme other than the stock Dark one is laid over Lemon's colours by a `Retint`.
// spec: ui/lemon-design.md

// This code is a part of L e m o n S t u d i o.

use crate::fl::{self, FlTheme};
use crate::{Color, Palette};

/// A skin: the look's shapes and base colours (Settings > Theme > Skins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Design {
    /// The measured default look (named Classic before 2026-10-17).
    #[default]
    Citrus,
    /// Lemon Studio's own look: lemon on neutral greys, rounded squares.
    Lemon,
}

impl Design {
    /// In the order Settings > Theme > Skins shows them.
    pub const ALL: [Design; 2] = [Design::Lemon, Design::Citrus];

    pub fn name(self) -> &'static str {
        match self {
            Design::Citrus => "Citrus",
            Design::Lemon => "Lemon",
        }
    }

    /// A skin by name; `Classic` (Citrus's old name, still in older `design.txt` files) is Citrus.
    pub fn parse(s: &str) -> Option<Design> {
        if s.trim().eq_ignore_ascii_case("classic") {
            return Some(Design::Citrus);
        }
        Design::ALL.into_iter().find(|d| d.name().eq_ignore_ascii_case(s.trim()))
    }

    pub fn next(self) -> Design {
        let i = Design::ALL.iter().position(|d| *d == self).unwrap_or(0);
        Design::ALL[(i + 1) % Design::ALL.len()]
    }

    pub fn style(self) -> Style {
        match self {
            Design::Citrus => Style::CITRUS,
            Design::Lemon => Style::LEMON,
        }
    }
}

/// Shape language of a design. Sizes that affect layout are not here: the layout never changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub design: Design,
    /// Corner radius of buttons, boxes and combos.
    pub radius: f32,
    /// Corner radius of small parts (steps, slots, LEDs, notes).
    pub radius_small: f32,
    /// Corner radius of panels, popups and floating windows.
    pub radius_panel: f32,
}

impl Style {
    pub const CITRUS: Style = Style { design: Design::Citrus, radius: 3.0, radius_small: 2.5, radius_panel: 4.0 };
    pub const LEMON: Style = Style { design: Design::Lemon, radius: 6.0, radius_small: 4.0, radius_panel: 10.0 };

    pub fn is(&self, d: Design) -> bool {
        self.design == d
    }

    pub fn lemon(&self) -> bool {
        self.design == Design::Lemon
    }
}

impl Default for Style {
    fn default() -> Self {
        Style::CITRUS
    }
}

/// How a design re-colours the measured (Classic) colours, by role.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignMap {
    /// Surface lightness: (classic lightness, design lightness) points, ascending input.
    pub ramp: &'static [(f32, f32)],
    /// Hue (degrees) and saturation of the design's neutrals.
    pub tint: (f32, f32),
    /// Classic key colours -> design key colours.
    pub accent: Color,
    pub highlight: Color,
    pub mute: Color,
    /// Text: the design's text colour and the surface it is read on (dimmer classic text moves toward it).
    pub text: Color,
    pub text_bg: Color,
}

/// Classic text and its toolbar background (the reference for `DesignMap::text_of`).
const CLASSIC_TEXT_L: f32 = 0.85;
const CLASSIC_BG_L: f32 = 0.19;

impl DesignMap {
    fn ramp_l(&self, l: f32) -> f32 {
        let r = self.ramp;
        if l <= r[0].0 {
            return r[0].1;
        }
        for w in r.windows(2) {
            let ((a, fa), (b, fb)) = (w[0], w[1]);
            if l <= b {
                let t = if b > a { (l - a) / (b - a) } else { 1.0 };
                return fa + (fb - fa) * t;
            }
        }
        r[r.len() - 1].1
    }

    /// A classic surface colour in this design: lightness through the ramp, neutrals take the design's tint;
    /// clearly coloured surfaces keep their hue with reduced saturation.
    pub fn surface(&self, c: Color) -> Color {
        let (h, s, l) = c.hsl();
        let l2 = self.ramp_l(l).clamp(0.0, 1.0);
        let (th, ts) = self.tint;
        // Coloured = visible chroma (HSL saturation overrates the stock look's blue-tinted dark greys).
        let chroma = f32::from(c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)) / 255.0;
        let (h2, s2) = if chroma > 0.09 { (h, (s * 0.6).max(ts)) } else { (th, ts) };
        Color::from_hsl_pub(h2, s2.clamp(0.0, 1.0), l2, c.a)
    }

    pub fn accent_of(&self, c: Color) -> Color {
        fl::rekey(c, classic_keys().selected, self.accent)
    }
    pub fn highlight_of(&self, c: Color) -> Color {
        fl::rekey(c, classic_keys().highlight, self.highlight)
    }
    pub fn mute_of(&self, c: Color) -> Color {
        fl::rekey(c, classic_keys().mute, self.mute)
    }

    /// A classic text colour: its position between the classic background and text is kept between the
    /// design's text background and text colour (so dim text stays dim).
    pub fn text_of(&self, c: Color) -> Color {
        let (_, s, l) = c.hsl();
        let t = ((l - CLASSIC_BG_L) / (CLASSIC_TEXT_L - CLASSIC_BG_L)).clamp(-0.2, 1.25);
        let (th, ts, tl) = self.text.hsl();
        let bl = self.text_bg.hsl().2;
        let l2 = (bl + (tl - bl) * t).clamp(0.0, 1.0);
        Color::from_hsl_pub(th, if s > 0.25 { s * 0.6 } else { ts }, l2, c.a)
    }
}

/// Key colours of the stock look (Classic was measured with them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keys {
    pub selected: Color,
    pub highlight: Color,
    pub text: Color,
    pub mute: Color,
}

pub fn classic_keys() -> Keys {
    let d = FlTheme::dark();
    Keys {
        selected: d.selected.unwrap_or(Color::rgb(0xa8e44a)),
        highlight: d.highlight.unwrap_or(Color::rgb(0xfb9244)),
        text: d.text_color.unwrap_or(Color::rgb(0xc9d0d3)),
        mute: d.mute.unwrap_or(Color::rgb(0x78d008)),
    }
}

/// The Lemon brand palette (owner's base colours, 2026-10-12) and the in-between shades derived from it.
pub mod lemon {
    use crate::Color;
    /// Main colour (pale yellow): "on" controls, selection, the playhead, lit steps.
    pub const MAIN: Color = Color::rgb(0xffe066);
    /// Highlight: the bright top of lit controls, hover on lit controls, selected notes.
    pub const HIGHLIGHT: Color = Color::rgb(0xfff1b8);
    /// Dark: the main surface (toolbar).
    pub const DARK: Color = Color::rgb(0x3b3b3b);
    /// Dark highlight: raised faces on hover, hairlines, knob tracks.
    pub const DARK_HIGHLIGHT: Color = Color::rgb(0x666666);
    /// Coloured dark: glyphs and text on lemon, unlit LEDs, hovered menu rows.
    pub const COLORED_DARK: Color = Color::rgb(0x473900);
    /// In-between: a darker lemon for edges of lit controls and meter bodies.
    pub const DEEP: Color = Color::rgb(0xc9a92c);
    /// Text on the greys.
    pub const TEXT: Color = Color::rgb(0xececec);
}

/// MetroBar Studio's palette: metrobarz.com, which is always light (`:root[data-theme="light"]`): deep red on pale
/// green-tinted greys, near-black ink, near-white buttons with thin borders.
pub mod metrobar {
    use crate::Color;
    /// Accent: "on" controls, selection, the playhead, record (flat, no glow).
    pub const ACCENT: Color = Color::rgb(0xaf0000);
    /// Text and glyphs on the accent.
    pub const ON_ACCENT: Color = Color::rgb(0xffffff);
    pub const BG: Color = Color::rgb(0xeef0ec);
    pub const PANEL: Color = Color::rgb(0xe2e5df);
    pub const FG: Color = Color::rgb(0x15181a);
    pub const MUTED: Color = Color::rgb(0x5d6662);
    pub const LINE: Color = Color::rgb(0xc5cbc4);
    pub const CELL: Color = Color::rgb(0xd3d8d1);
    /// Buttons and input boxes: near white, with a thin `LINE` border.
    pub const FACE: Color = Color::rgb(0xf8f9f7);
    pub const S1: Color = Color::rgb(0x4b5551);
    pub const S2: Color = Color::rgb(0x626c68);
    pub const S3: Color = Color::rgb(0x7a847f);
    pub const S4: Color = Color::rgb(0x939c97);
    pub const S5: Color = Color::rgb(0xabb3ae);
    /// The colour set for clips and channels (the light page's `--r0`..`--r7`).
    pub const SET: [Color; 8] = [
        Color::rgb(0xa87800),
        Color::rgb(0x1f7aa8),
        Color::rgb(0xb83c72),
        Color::rgb(0x2f8a42),
        Color::rgb(0x7a52b8),
        Color::rgb(0xb35416),
        Color::rgb(0x1d8f82),
        Color::rgb(0x8a8a10),
    ];
    /// Lemon skin lightness -> MetroBar lightness. Lemon is dark, MetroBar light, so the ramp turns over: Lemon's
    /// workspace (0.149) lands on BG, its toolbar (0.18) on PANEL, its raised faces on CELL / LINE, its dark highlight
    /// (0.40) a step darker, its text (0.93) on FG; its near-black wells are lighter than BG, and pure black (edges,
    /// hairlines) a visible line.
    pub(crate) const RAMP: &[(f32, f32)] = &[
        (0.0, 0.72),
        (0.04, 0.80),
        (0.07, 0.965),
        (0.122, 0.95),
        (0.149, 0.93),
        (0.18, 0.886),
        (0.231, 0.86),
        (0.278, 0.83),
        (0.333, 0.78),
        (0.40, 0.70),
        (0.5, 0.58),
        (0.6, 0.46),
        (0.84, 0.25),
        (1.0, 0.09),
    ];
    /// Hue and saturation of the greys.
    pub(crate) const TINT: (f32, f32) = (110.0, 0.08);
}

/// MetroBar Studio's night look (the first MetroBar theme, metrobarz.com's dark palette): the same deep red on
/// green-tinted near-black greys, white on red, no glow.
pub mod metrobar_night {
    use crate::Color;
    /// Accent: "on" controls, selection, the playhead, record (flat, no glow).
    pub const ACCENT: Color = Color::rgb(0xaf0000);
    /// Text and glyphs on the accent.
    pub const ON_ACCENT: Color = Color::rgb(0xffffff);
    pub const BG: Color = Color::rgb(0x111413);
    pub const PANEL: Color = Color::rgb(0x1a1e1d);
    pub const FG: Color = Color::rgb(0xe9ece8);
    pub const MUTED: Color = Color::rgb(0x8e9893);
    pub const LINE: Color = Color::rgb(0x2c3331);
    pub const CELL: Color = Color::rgb(0x252b29);
    pub const S1: Color = Color::rgb(0x4b5652);
    pub const S2: Color = Color::rgb(0x66716c);
    pub const S3: Color = Color::rgb(0x838d88);
    pub const S4: Color = Color::rgb(0xa1aaa5);
    pub const S5: Color = Color::rgb(0xbfc6c2);
    /// The colour set for clips and channels.
    pub const SET: [Color; 8] = [
        Color::rgb(0xd9a62e),
        Color::rgb(0x3fa7d6),
        Color::rgb(0xe0679c),
        Color::rgb(0x59b36b),
        Color::rgb(0xa77fe0),
        Color::rgb(0xe07b39),
        Color::rgb(0x3cbfb0),
        Color::rgb(0xc9c94a),
    ];
    /// Lemon skin lightness -> MetroBar lightness (Lemon's workspace lands on BG, its toolbar on PANEL, its raised
    /// faces on CELL / LINE, its dark highlight on S1).
    pub(crate) const RAMP: &[(f32, f32)] = &[
        (0.0, 0.0),
        (0.07, 0.035),
        (0.122, 0.055),
        (0.149, 0.0725),
        (0.18, 0.09),
        (0.231, 0.11),
        (0.278, 0.157),
        (0.333, 0.186),
        (0.40, 0.30),
        (0.5, 0.40),
        (0.6, 0.52),
        (0.84, 0.755),
        (1.0, 1.0),
    ];
    /// Hue and saturation of the greys.
    pub(crate) const TINT: (f32, f32) = (160.0, 0.075);
}

/// Lemon: Classic lightness -> Lemon lightness. Classic's toolbar (0.19) lands on DARK (#3b3b3b, 0.231) and the
/// 0.33 raised greys on DARK_HIGHLIGHT (#666666, 0.40); the near-black lists and rulers lift to about #151515.
const LEMON_RAMP: &[(f32, f32)] = &[
    (0.0, 0.07),
    (0.03, 0.085),
    (0.06, 0.10),
    (0.10, 0.125),
    (0.13, 0.15),
    (0.155, 0.18),
    (0.19, 0.231),
    (0.23, 0.285),
    (0.27, 0.33),
    (0.33, 0.40),
    (0.5, 0.56),
    (0.8, 0.84),
    (1.0, 1.0),
];

fn map_of(d: Design) -> Option<DesignMap> {// This code is a part of L e m o n S t u d i o.
    match d {
        Design::Citrus => None,
        Design::Lemon => Some(DesignMap {
            ramp: LEMON_RAMP,
            tint: (48.0, 0.0),
            accent: lemon::MAIN,
            highlight: lemon::MAIN,
            mute: lemon::MAIN,
            text: lemon::TEXT,
            text_bg: lemon::DARK,
        }),
    }
}

/// Tokens a design sets outright (the ones the role map can't get right: signal colours, notes, keys, steps).
fn overrides(d: Design) -> Vec<(&'static str, Color)> {
    let c = Color::rgb;
    use lemon::*;
    match d {
        Design::Citrus => vec![],
        Design::Lemon => vec![
            ("toolbar", DARK),
            ("toolbar_line", c(0x2a2a2a)),
            ("toolbar_edge", c(0x4a4a4a)),
            ("sunken", c(0x2b2b2b)),
            ("sunken_dark", c(0x232323)),
            ("time_panel", c(0x232323)),
            ("menu_panel", c(0x333333)),
            ("menu_text", TEXT),
            ("news_panel", c(0x333333)),
            ("button", c(0x474747)),
            ("button_hi", c(0x555555)),
            ("glyph", c(0xe6e6e6)),
            ("glyph_dim", c(0x8c8c8c)),
            ("text", TEXT),
            ("text_dim", c(0xa3a3a3)),
            ("text_faint", c(0x7f7f7f)),
            ("on_top", HIGHLIGHT),
            ("on_bottom", MAIN),
            ("on_glyph", COLORED_DARK),
            ("pill", MAIN),
            ("pill_text", COLORED_DARK),
            ("record", c(0xff6159)),
            ("tempo_box", c(0x2b2b2b)),
            ("accent", MAIN),
            ("accent_dim", COLORED_DARK),
            ("workspace", c(0x262626)),
            ("watermark", c(0x2e2e2e)),
            ("version_text", c(0x5a5a5a)),
            ("browser_title", c(0x333333)),
            ("browser_tabs", c(0x2b2b2b)),
            ("browser_tab_outline", MAIN),
            ("browser_list", c(0x1f1f1f)),
            ("browser_row_hi", c(0x333333)),
            ("browser_folder", c(0xd9d9d9)),
            ("browser_footer", c(0x2b2b2b)),
            ("browser_search", MAIN),
            ("cat_project", MAIN),
            ("cat_recent", c(0xc8e3a6)),
            ("cat_plugin_db", c(0xa9cbe6)),
            ("cat_plugin_presets", c(0xe6b3cf)),
            ("cat_channel_presets", c(0xd4bce8)),
            ("cat_mixer_presets", c(0xbcc2ec)),
            ("splitter", c(0x262626)),
            ("splitter_dark", c(0x1c1c1c)),
            ("window_title", c(0x333333)),
            ("window_title_float", c(0x333333)),
            ("window_title_hi", c(0x3d3d3d)),
            ("window_body", c(0x2e2e2e)),
            ("window_border", c(0x1a1a1a)),
            ("picker", c(0x2b2b2b)),
            ("track_header", c(0x383838)),
            ("overview", c(0x555555)),
            ("ruler", c(0x232323)),
            ("ruler_text", c(0x9e9e9e)),
            ("grid", c(0x282828)),
            ("grid_alt", c(0x2d2d2d)),
            ("grid_gap", c(0x202020)),
            ("grid_bar", c(0x161616)),
            ("scrollbar", c(0x2b2b2b)),
            ("playhead", MAIN),
            ("mixer_bg", c(0x262626)),
            ("mixer_strip", c(0x303030)),
            ("mixer_strip_dark", c(0x242424)),
            ("mixer_panel", c(0x333333)),
            ("slot", c(0x2b2b2b)),
            ("slot_text", c(0x707070)),
            ("slot_text_used", c(0xdedede)),
            ("fader_handle", c(0x5c5c5c)),
            ("meter_low", DEEP),
            ("meter_high", HIGHLIGHT),
            ("meter_clip", c(0xff6159)),
            ("rack_row_box", c(0x3a3a3a)),
            ("name_button", c(0x474747)),
            ("name_button_sel", c(0x5e5e5e)),
            ("name_text", c(0xe2e2e2)),
            ("rack_inset", c(0x333333)),
            ("rack_inset_border", c(0x262626)),
            ("rack_control_border", c(0x222222)),
            ("step_a", c(0x4d4d4d)),
            ("step_b", c(0x5a4d1f)),
            ("step_dim_a", c(0x3a3a3a)),
            ("step_dim_b", c(0x3d3827)),
            ("step_dim_border", c(0x2e2e2e)),
            ("step_on", MAIN),
            ("add_hover", MAIN),
            ("menu_drop", c(0x333333)),
            ("menu_drop_hi", c(0x4d4d4d)),
            ("menu_drop_text", TEXT),
            ("menu_drop_disabled", c(0x7f7f7f)),
            ("menu_pop", c(0x333333)),
            ("menu_pop_border", c(0x1f1f1f)),
            ("menu_pop_hi", COLORED_DARK),
            ("menu_pop_text", TEXT),
            ("menu_pop_shortcut", c(0xa3a3a3)),
            ("menu_pop_dim", c(0x7f7f7f)),
            ("menu_pop_head", c(0x2b2b2b)),
            ("menu_pop_head_text", c(0x8c8c8c)),
            ("clip", c(0x8a7a3a)),
            ("pr_row", c(0x2b2b2b)),
            ("pr_row_black", c(0x242424)),
            ("pr_row_sel", c(0x2f2c22)),
            ("pr_row_black_sel", c(0x29261c)),
            ("pr_bar", c(0x141414)),
            ("note_top", MAIN),
            ("note_bottom", c(0xf2d052)),
            ("note_rim", c(0x8a7420)),
            ("note_text", COLORED_DARK),
            ("note_sel_top", c(0xffffff)),
            ("note_sel_bottom", HIGHLIGHT),
            ("note_sel_rim", COLORED_DARK),
            ("note_sel_text", COLORED_DARK),
            ("key_white", c(0xededed)),
            ("key_white_left", c(0xd2d2d2)),
            ("key_black", c(0x3b3b3b)),
            ("key_black_left", c(0x262626)),
            ("key_c", c(0xdddddd)),
            ("key_label", c(0x8a7420)),
            ("key_lit", HIGHLIGHT),
            ("key_lit_left", MAIN),
            ("key_lit_edge", DEEP),
            ("key_black_lit", MAIN),
            ("key_black_lit_left", c(0xa88f25)),
            ("velocity", MAIN),
            ("velocity_dot", COLORED_DARK),
        ],
    }
}

/// The palette of skin `design` coloured by `theme` (a `.flstheme`). The stock Dark theme (whatever its options), or
/// none, leaves a skin its own colours: Citrus the measured look, Lemon the brand palette; any other theme colours
/// Citrus through `fl::palette` and is laid over Lemon's colours by a `Retint`. The theme's workspace picture goes with
/// it on either skin, also where it keeps the skin's own colours (spec: ui/flstheme.md §3).
pub fn palette(design: Design, theme: Option<&FlTheme>) -> Palette {
    let mut p = skin_palette(design, theme);
    p.wallpaper = theme.and_then(fl::wallpaper);
    if let Some(t) = theme.filter(|t| t.metrobar) {
        for (name, c) in if t.metrobar_night { metrobar_night_tokens() } else { metrobar_tokens() } {
            if let Some(slot) = p.get_mut(name) {
                *slot = c;
            }
        }
    }
    p
}

/// The MetroBar Night theme's tokens, set outright over either skin.
fn metrobar_night_tokens() -> Vec<(&'static str, Color)> {
    let c = Color::rgb;
    use metrobar_night::*;
    vec![
        ("toolbar", PANEL),
        ("toolbar_line", BG),
        ("toolbar_edge", LINE),
        ("sunken", c(0x151917)),
        ("sunken_dark", c(0x0e100f)),
        ("time_panel", c(0x0e100f)),
        ("menu_panel", c(0x161a19)),
        ("menu_text", FG),
        ("news_panel", c(0x161a19)),
        ("button", CELL),
        ("button_hi", c(0x333b38)),
        ("glyph", FG),
        ("glyph_dim", MUTED),
        ("text", FG),
        ("text_dim", S4),
        ("text_faint", S3),
        ("on_top", ACCENT),
        ("on_bottom", ACCENT),
        ("on_glyph", ON_ACCENT),
        ("pill", ACCENT),
        ("pill_text", ON_ACCENT),
        ("record", ACCENT),
        ("tempo_box", BG),
        ("accent", ACCENT),
        ("accent_dim", c(0x4a1210)),
        ("workspace", BG),
        ("watermark", c(0x181c1b)),
        ("version_text", S1),
        ("browser_title", c(0x161a19)),
        ("browser_tabs", BG),
        ("browser_tab_outline", ACCENT),
        ("browser_list", c(0x0e1110)),
        ("browser_row_hi", CELL),
        ("browser_folder", c(0xd5dad6)),
        ("browser_footer", BG),
        ("browser_search", ACCENT),
        ("cat_project", FG),
        ("cat_recent", SET[3]),
        ("cat_plugin_db", SET[1]),
        ("cat_plugin_presets", SET[2]),
        ("cat_channel_presets", SET[4]),
        ("cat_mixer_presets", SET[6]),
        ("splitter", BG),
        ("splitter_dark", c(0x0b0d0c)),
        ("window_title", PANEL),
        ("window_title_float", PANEL),
        ("window_title_hi", c(0x222826)),
        ("window_body", c(0x161a19)),
        ("window_border", c(0x0b0d0c)),
        ("picker", c(0x151917)),
        ("track_header", c(0x1f2422)),
        ("overview", c(0x3a4340)),
        ("ruler", c(0x0e100f)),
        ("ruler_text", MUTED),
        ("grid", c(0x161a19)),
        ("grid_alt", PANEL),
        ("grid_gap", c(0x101312)),
        ("grid_bar", c(0x0a0c0b)),
        ("scrollbar", BG),
        ("playhead", ACCENT),
        ("mixer_bg", BG),
        ("mixer_strip", PANEL),
        ("mixer_strip_dark", c(0x131615)),
        ("mixer_panel", PANEL),
        ("slot", c(0x151917)),
        ("slot_text", S2),
        ("slot_text_used", c(0xd5dad6)),
        ("fader_handle", S1),
        // Meters stay green / yellow / red so levels read.
        ("meter_low", SET[3]),
        ("meter_high", SET[0]),
        ("meter_clip", c(0xe5352b)),
        ("rack_row_box", CELL),
        ("name_button", CELL),
        ("name_button_sel", c(0x3a4340)),
        ("name_text", c(0xdfe3df)),
        ("rack_inset", PANEL),
        ("rack_inset_border", BG),
        ("rack_control_border", c(0x0e100f)),
        ("step_a", LINE),
        ("step_b", c(0x3a2725)),
        ("step_dim_a", c(0x1f2422)),
        ("step_dim_b", c(0x261b1a)),
        ("step_dim_border", c(0x161a19)),
        ("step_on", ACCENT),
        ("add_hover", ACCENT),
        ("menu_drop", PANEL),
        ("menu_drop_hi", LINE),
        ("menu_drop_text", FG),
        ("menu_drop_disabled", S2),
        ("menu_pop", PANEL),
        ("menu_pop_border", c(0x0b0d0c)),
        ("menu_pop_hi", c(0x6e0000)),
        ("menu_pop_text", FG),
        ("menu_pop_shortcut", S4),
        ("menu_pop_dim", S2),
        ("menu_pop_head", c(0x151917)),
        ("menu_pop_head_text", MUTED),
        ("clip", c(0x2f6f8c)),
        ("pr_row", c(0x161a19)),
        ("pr_row_black", c(0x121514)),
        ("pr_row_sel", c(0x1f1a1a)),
        ("pr_row_black_sel", c(0x1a1515)),
        ("pr_bar", c(0x0a0c0b)),
        ("note_top", c(0xb81010)),
        ("note_bottom", ACCENT),
        ("note_rim", c(0x5c0000)),
        ("note_text", ON_ACCENT),
        ("note_sel_top", c(0xf2dede)),
        ("note_sel_bottom", c(0xe6c4c4)),
        ("note_sel_rim", c(0x6e0000)),
        ("note_sel_text", c(0x3a0000)),
        ("key_white", FG),
        ("key_white_left", c(0xc9cfcb)),
        ("key_black", PANEL),
        ("key_black_left", BG),
        ("key_c", c(0xd8ddd9)),
        ("key_label", ACCENT),
        ("key_lit", c(0xc41a1a)),
        ("key_lit_left", ACCENT),
        ("key_lit_edge", c(0x7d0000)),
        ("key_black_lit", ACCENT),
        ("key_black_lit_left", c(0x6e0000)),
        ("velocity", ACCENT),
        ("velocity_dot", c(0x5c0000)),
    ]
}

/// The MetroBar Studio theme's tokens, set outright over either skin (metrobarz.com's light palette).
fn metrobar_tokens() -> Vec<(&'static str, Color)> {
    let c = Color::rgb;
    use metrobar::*;
    vec![
        ("toolbar", PANEL),
        ("toolbar_line", LINE),
        ("toolbar_edge", LINE),
        ("sunken", c(0xf4f5f2)),
        ("sunken_dark", FACE),
        ("time_panel", CELL),
        ("menu_panel", PANEL),
        ("menu_text", FG),
        ("news_panel", PANEL),
        ("button", FACE),
        ("button_hi", c(0xe9ebe7)),
        ("glyph", FG),
        ("glyph_dim", S2),
        ("text", FG),
        ("text_dim", MUTED),
        ("text_faint", S3),
        ("on_top", ACCENT),
        ("on_bottom", ACCENT),
        ("on_glyph", ON_ACCENT),
        ("pill", ACCENT),
        ("pill_text", ON_ACCENT),
        ("record", ACCENT),
        ("tempo_box", FACE),
        ("accent", ACCENT),
        // The page's --land-soft: the accent 38 % into a cell.
        ("accent_dim", c(0xc58682)),
        ("workspace", BG),
        ("watermark", c(0xd9ddd6)),
        ("version_text", S4),
        ("browser_title", PANEL),
        ("browser_tabs", BG),
        ("browser_tab_outline", ACCENT),
        ("browser_list", c(0xf4f5f2)),
        ("browser_row_hi", CELL),
        ("browser_folder", FG),
        ("browser_footer", PANEL),
        ("browser_search", ACCENT),
        ("cat_project", FG),
        ("cat_recent", SET[3]),
        ("cat_plugin_db", SET[1]),
        ("cat_plugin_presets", SET[2]),
        ("cat_channel_presets", SET[4]),
        ("cat_mixer_presets", SET[6]),
        ("splitter", LINE),
        ("splitter_dark", S5),
        ("window_title", PANEL),
        ("window_title_float", PANEL),
        ("window_title_hi", CELL),
        ("window_body", BG),
        ("window_border", S5),
        ("picker", c(0xf4f5f2)),
        ("track_header", c(0xe7eae4)),
        ("overview", LINE),
        ("ruler", PANEL),
        ("ruler_text", MUTED),
        ("grid", c(0xf4f5f2)),
        ("grid_alt", c(0xe9ece6)),
        ("grid_gap", c(0xdde1da)),
        ("grid_bar", LINE),
        ("scrollbar", BG),
        ("playhead", ACCENT),
        ("mixer_bg", BG),
        ("mixer_strip", PANEL),
        ("mixer_strip_dark", c(0xd9ddd6)),
        ("mixer_panel", PANEL),
        ("slot", FACE),
        ("slot_text", S3),
        ("slot_text_used", FG),
        ("fader_handle", S1),
        // Meters stay green / yellow / red so levels read.
        ("meter_low", c(0x2f9e47)),
        ("meter_high", c(0xe0ae1c)),
        ("meter_clip", c(0xe5352b)),
        ("rack_row_box", CELL),
        ("name_button", FACE),
        ("name_button_sel", LINE),
        ("name_text", FG),
        ("rack_inset", PANEL),
        ("rack_inset_border", LINE),
        ("rack_control_border", S5),
        ("step_a", LINE),
        ("step_b", c(0xdfc4c0)),
        ("step_dim_a", PANEL),
        ("step_dim_b", c(0xecdcda)),
        ("step_dim_border", LINE),
        ("step_on", ACCENT),
        ("add_hover", ACCENT),
        ("menu_drop", FACE),
        ("menu_drop_hi", CELL),
        ("menu_drop_text", FG),
        ("menu_drop_disabled", S4),
        ("menu_pop", FACE),
        ("menu_pop_border", S5),
        ("menu_pop_hi", c(0xecd3d0)),
        ("menu_pop_text", FG),
        ("menu_pop_shortcut", S2),
        ("menu_pop_dim", S4),
        ("menu_pop_head", PANEL),
        ("menu_pop_head_text", MUTED),
        ("clip", c(0x3f8db3)),
        ("pr_row", c(0xf4f5f2)),
        ("pr_row_black", c(0xe6e9e3)),
        ("pr_row_sel", c(0xf3e9e8)),
        ("pr_row_black_sel", c(0xe8dcdb)),
        ("pr_bar", LINE),
        ("note_top", c(0xc41a1a)),
        ("note_bottom", ACCENT),
        ("note_rim", c(0x6e0000)),
        ("note_text", ON_ACCENT),
        // Selected notes: ink, as the page's lit tiles under the cursor.
        ("note_sel_top", c(0x2a2f31)),
        ("note_sel_bottom", FG),
        ("note_sel_rim", c(0x000000)),
        ("note_sel_text", ON_ACCENT),
        ("key_white", c(0xffffff)),
        ("key_white_left", c(0xe9ebe7)),
        ("key_black", c(0x2a2f2d)),
        ("key_black_left", FG),
        ("key_c", c(0xf1f2ef)),
        ("key_label", ACCENT),
        ("key_lit", c(0xc41a1a)),
        ("key_lit_left", ACCENT),
        ("key_lit_edge", c(0x7d0000)),
        ("key_black_lit", ACCENT),
        ("key_black_lit_left", c(0x6e0000)),
        ("velocity", ACCENT),
        ("velocity_dot", c(0x7d0000)),
    ]
}

fn skin_palette(design: Design, theme: Option<&FlTheme>) -> Palette {
    // The Lemon and Citrus themes (`skin_text`): the Lemon theme on Citrus is Lemon's colours with Citrus's shapes; the
    // Citrus theme on Lemon is the stock Dark theme laid over Lemon (stock Dark without the key counts as Lemon's own);
    // Citrus with the Citrus theme is the stock Dark theme, as usual.
    match (design, theme.and_then(|t| t.colors)) {
        (Design::Lemon, Some(Design::Lemon)) => return palette(design, None),
        (Design::Citrus, Some(Design::Lemon)) => {
            let mut p = palette(Design::Lemon, None);
            p.style = Design::Citrus.style();
            return p;
        }
        (Design::Lemon, Some(Design::Citrus)) => {
            let mut p = palette(Design::Lemon, None);
            retint(&mut p, &FlTheme { colors: None, ..theme.expect("a theme").clone() });
            return p;
        }
        _ => {}
    }
    let Some(map) = map_of(design) else {
        return match theme {
            Some(th) => fl::palette(th),
            None => Palette::default(),
        };
    };
    let classic = Palette::default();
    let mut p = classic.clone();
    for name in Palette::NAMES {
        let c = classic.get(name).expect("token");
        let v = match fl::role(name) {
            fl::Role::Surface | fl::Role::StepEven | fl::Role::StepOdd | fl::Role::Fixed => map.surface(c),
            fl::Role::Accent => map.accent_of(c),
            fl::Role::Highlight => map.highlight_of(c),
            fl::Role::Text => map.text_of(c),
            fl::Role::Mute => map.mute_of(c),
        };
        *p.get_mut(name).expect("token") = v;
    }
    for (name, c) in overrides(design) {
        if let Some(slot) = p.get_mut(name) {
            *slot = c;
        }
    }
    p.xf = fl::Xform { design: Some(map), ..fl::Xform::default() };
    p.style = design.style();
    if let Some(th) = theme.filter(|t| !t.is_stock()) {
        retint(&mut p, th);
    }
    p
}

/// FL's base palette toolbar (spec: ui/flstheme.md §5): its slight blue tint is what a theme's Hue and Saturation turn.
const BASE_TINT: Color = Color::rgb(0x434c51);

/// A theme laid over a skin's own colours: the theme's surface adjustment tints and lights the skin's greys (read as
/// stock Dark surfaces, given FL's base tint), its selection / highlight / mute colours take the place of the skin's
/// key colour, and its text colour the skin's text, keeping each text's place between the skin's background and text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retint {
    /// The theme's surface adjustment.
    pub surface: fl::Surface,
    /// The skin's key colour (Lemon: Main) and the theme's keys (`None` = keep the skin's).
    pub key: Color,
    pub accent: Option<Color>,
    pub highlight: Option<Color>,
    pub mute: Option<Color>,
    /// (text, background) of the skin, and of the theme.
    pub text_from: (Color, Color),
    pub text_to: (Color, Color),
    /// The MetroBar Studio theme: greys through `metrobar::RAMP`, key colours kept flat (no lighter glow).
    pub metrobar: bool,
    /// Its night look: greys through `metrobar_night::RAMP`.
    pub metrobar_night: bool,
}

impl Retint {
    /// `th` over the Lemon skin.
    pub fn lemon(th: &FlTheme) -> Retint {
        let mut r = Retint {
            surface: th.surface,
            key: lemon::MAIN,
            accent: th.selected,
            highlight: th.highlight,
            mute: th.mute,
            text_from: (lemon::TEXT, lemon::DARK),
            text_to: (lemon::TEXT, lemon::DARK),
            metrobar: th.metrobar,
            metrobar_night: th.metrobar_night,
        };
        let bg = r.surface(lemon::DARK);
        let text = th.text_color.unwrap_or(if bg.hsl().2 > 0.5 { Color::rgb(0x1e1e1e) } else { lemon::TEXT });
        r.text_to = (fl::readable(text, bg, 0.38), bg);
        r
    }

    pub fn surface(&self, c: Color) -> Color {
        if self.metrobar {
            let (_, s, l) = c.hsl();
            let (ramp, tint) = if self.metrobar_night { (metrobar_night::RAMP, metrobar_night::TINT) } else { (metrobar::RAMP, metrobar::TINT) };
            let map = DesignMap { ramp, tint, ..map_of(Design::Lemon).expect("lemon") };
            let l2 = map.ramp_l(l).clamp(0.0, 1.0);
            let chroma = f32::from(c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)) / 255.0;
            if chroma > 0.09 {
                // Lemon's lemon-tinted wells and hovers: a red tint of the same (turned over) lightness.
                let (kh, _, _) = metrobar::ACCENT.hsl();
                return Color::from_hsl_pub(kh, (s * 0.6).min(0.45), l2, c.a);
            }
            return Color::from_hsl_pub(tint.0, tint.1, l2, c.a);
        }
        let l = fl::DARK.invert(c).hsl().2;
        let (h, s, _) = BASE_TINT.hsl();
        let (h2, s2, l2) = self.surface.apply(Color::from_hsl_pub(h, s, l, c.a)).hsl();
        // A coloured surface (Lemon's lemon-tinted wells and hovers) takes the theme's selection hue.
        let chroma = f32::from(c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)) / 255.0;
        let (h2, s2) = if chroma > 0.09 {
            let (kh, ks, _) = self.accent(c).hsl();
            (kh, ks)
        } else {
            (h2, s2)
        };
        Color::from_hsl_pub(h2, s2, l2, c.a)
    }

    fn key(&self, c: Color, to: Option<Color>) -> Color {
        if self.metrobar && let Some(t) = to {
            // Flat: shades lighter than the key stay close to it, darker ones keep some of the key's lightness.
            let (h, s, l) = c.hsl();
            let (hf, sf, lf) = self.key.hsl();
            let (ht, st, lt) = t.hsl();
            let s2 = if sf > 0.02 { (s * st / sf).clamp(0.0, 1.0) } else { st };
            let h2 = if sf > 0.02 { h + (ht - hf) } else { ht };
            let l2 = if l <= lf { lt * (0.35 + 0.65 * l / lf.max(1e-3)) } else { lt + (l - lf) / (1.0 - lf).max(1e-3) * 0.12 };
            return Color::from_hsl_pub(h2, s2, l2.clamp(0.0, 1.0), c.a);
        }
        to.map_or(c, |t| fl::rekey(c, self.key, t))
    }
    pub fn accent(&self, c: Color) -> Color {
        self.key(c, self.accent)
    }
    pub fn highlight(&self, c: Color) -> Color {
        self.key(c, self.highlight)
    }
    pub fn mute(&self, c: Color) -> Color {
        self.key(c, self.mute)
    }

    pub fn text(&self, c: Color) -> Color {
        let (_, s, l) = c.hsl();
        let ((ft, fb), (tt, tb)) = (self.text_from, self.text_to);
        let (fl_, bl) = (ft.hsl().2, fb.hsl().2);
        let t = if (fl_ - bl).abs() > 1e-3 { ((l - bl) / (fl_ - bl)).clamp(-0.2, 1.25) } else { 1.0 };
        let (th, ts, tl) = tt.hsl();
        let l2 = tb.hsl().2 + (tl - tb.hsl().2) * t;
        Color::from_hsl_pub(th, if s > 0.25 { s * 0.6 } else { ts }, l2.clamp(0.0, 1.0), c.a)
    }
}

/// A skin's own colours as theme text: where Settings ▸ Theme ▸ Adjustments start with no theme applied. Citrus: the
/// stock Dark theme. Lemon: the stock Dark surface with saturation at its lowest (Lemon's neutral greys), Lemon's Main
/// as selection, highlight and mute, Lemon's text, and no step colours, so it colours Lemon as Lemon is (spec:
/// ui/theme-presets.md §C).
pub fn default_text(d: Design) -> String {
    match d {
        Design::Citrus => fl::DARK_TEXT.to_string(),
        Design::Lemon => {
            let v = |key: &str, c: Color| format!("{key}={}\n", fl::colour_value(key, c));
            format!(
                "Hue=0\nSaturation=-256\nLightness=-153\nContrast=-8\nText=0\n{}{}{}{}BackMode=2\n",
                v("Selected", lemon::MAIN),
                v("Highlight", lemon::MAIN),
                v("Mute", lemon::MAIN),
                v("TextColor", lemon::TEXT)
            )
        }
    }
}

/// The Lemon and Citrus themes (Settings ▸ Theme ▸ Themes): skin `d`'s own colours as a theme file, marked with
/// `fl::COLORS_KEY` so either skin draws them exactly (spec: ui/theme-presets.md §B). The rest of the file is
/// `default_text(d)`, which is what FL reads.
pub fn skin_text(d: Design) -> String {
    format!("{}{}={}\n", default_text(d), fl::COLORS_KEY, d.name())
}

/// Lemon's palette `p` coloured by `th` (see `Retint`). Step faces are the theme's own (as Citrus draws them); the
/// fixed tokens drawn in the lemon key colour (lit steps, notes, key labels, the default clip, the project category)
/// follow the theme's selection colour; then what the theme colours directly (`fl::finish`).
fn retint(p: &mut Palette, th: &FlTheme) {
    let r = Retint::lemon(th);
    let citrus = fl::palette(th);
    let skin = p.clone();
    for name in Palette::NAMES {
        let c = skin.get(name).expect("token");
        let v = match fl::role(name) {
            fl::Role::Surface => r.surface(c),
            fl::Role::Accent => r.accent(c),
            fl::Role::Highlight => r.highlight(c),
            fl::Role::Text => r.text(c),
            fl::Role::Mute => r.mute(c),
            // A theme without step colours leaves the skin's own step faces (only re-greyed like its surfaces).
            fl::Role::StepEven if th.step_even.is_none() => r.surface(c),
            fl::Role::StepOdd if th.step_odd.is_none() => r.surface(c),
            fl::Role::StepEven | fl::Role::StepOdd => citrus.get(name).expect("token"),
            fl::Role::Fixed => match *name {
                "step_on" | "note_top" | "note_bottom" | "note_rim" | "note_text" | "note_sel_top" | "note_sel_bottom"
                | "note_sel_rim" | "note_sel_text" | "key_label" | "clip" | "cat_project" | "velocity" | "velocity_dot" => r.accent(c),
                "meter_low" | "meter_high" => r.highlight(c),
                _ => c,
            },
        };
        *p.get_mut(name).expect("token") = v;
    }
    fl::finish(p, th, skin.note_top, true);
    p.xf.retint = Some(r);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast(a: Color, b: Color) -> f32 {
        (a.hsl().2 - b.hsl().2).abs()
    }

    /// Adjustments start from the skin's own colours as a theme: Citrus's is the stock Dark theme; Lemon's colours
    /// Lemon as it is (within rounding), so the first knob turn changes only what it turns.
    #[test]
    fn default_text_is_the_skins_own_colours() {
        assert!(FlTheme::parse(&default_text(Design::Citrus)).is_stock());
        let own = palette(Design::Lemon, None);
        let th = FlTheme::parse(&default_text(Design::Lemon));
        assert!(!th.is_stock());
        let p = palette(Design::Lemon, Some(&th));
        let worst = Palette::NAMES
            .iter()
            .map(|n| {
                let (a, b) = (own.get(n).unwrap(), p.get(n).unwrap());
                let d = [a.r.abs_diff(b.r), a.g.abs_diff(b.g), a.b.abs_diff(b.b)].into_iter().max().unwrap();
                (d, *n)
            })
            .max()
            .unwrap();
        assert!(worst.0 <= 12, "{worst:?}");
        assert_eq!(p.a(0x2d7cd0), own.a(0x2d7cd0), "accents stay Lemon's Main");
    }

    #[test]
    fn classic_is_unchanged() {
        assert_eq!(palette(Design::Citrus, None), Palette::default());
        let th = FlTheme::parse("Hue=120\nSaturation=60\nLightness=-90\nContrast=0\nSelected=16744576\n");
        assert_eq!(palette(Design::Citrus, Some(&th)), fl::palette(&th));
    }

    #[test]
    fn lemon_uses_the_brand_colours() {
        let p = palette(Design::Lemon, None);
        assert!(p.style.lemon());
        assert_eq!(p.toolbar, lemon::DARK);
        assert_eq!(p.pill, lemon::MAIN);
        assert_eq!(p.on_glyph, lemon::COLORED_DARK);
        // Role-mapped literals follow the design: Classic's raised grey lands on the dark highlight,
        // orange highlights on lemon.
        let g = p.s(0x505355);
        assert!(g.r.abs_diff(0x66) <= 6 && g.r == g.g && g.g == g.b, "{g}");
        let (h, s, _) = p.h(0xfb9244).hsl();
        assert!((h - lemon::MAIN.hsl().0).abs() < 4.0 && s > 0.8, "{}", p.h(0xfb9244));
        assert_eq!(p.a(0xa8e44a), lemon::MAIN);
    }

    #[test]
    fn lemon_text_is_readable() {
        let p = palette(Design::Lemon, None);
        for (fg, bg) in [
            (p.text, p.window_title),
            (p.text_dim, p.window_title),
            (p.glyph, p.toolbar),
            (p.menu_text, p.menu_panel),
            (p.browser_folder, p.browser_list),
            (p.pill_text, p.pill),
            (p.on_glyph, p.on_bottom),
            (p.menu_pop_text, p.menu_pop_hi),
            (p.name_text, p.name_button),
            (p.ruler_text, p.ruler),
            (p.slot_text_used, p.slot),
            (p.note_text, p.note_top),
        ] {
            assert!(contrast(fg, bg) >= 0.38, "{fg} on {bg}");
        }
    }

    /// A theme colours the Lemon skin (spec: ui/theme-presets.md "Skins"): Lemon's shapes stay, the theme's
    /// selection replaces Main, its surfaces are light where Citrus's clearly are, and text stays readable on them.
    #[test]
    fn themes_colour_lemon() {
        for (name, text) in crate::presets::PRESETS {
            let th = FlTheme::parse(text);
            let p = palette(Design::Lemon, Some(&th));
            assert!(p.style.lemon(), "{name}");
            assert_eq!(p.accent, th.selected.unwrap(), "{name}");
            assert_eq!(p.a(0xa8e44a), th.selected.unwrap(), "{name}");
            let citrus = palette(Design::Citrus, Some(&th));
            // Light where Citrus is clearly light, dark where it is clearly dark (a mid-grey theme may sit either side).
            let (l, cl) = (p.toolbar.hsl().2, citrus.toolbar.hsl().2);
            assert!(!(cl > 0.6 && l <= 0.5) && !(cl < 0.4 && l >= 0.5), "{name}: light like Citrus ({l} vs {cl})");
            for (fg, bg) in [(p.text, p.window_title), (p.glyph, p.toolbar), (p.menu_text, p.menu_panel), (p.name_text, p.name_button)] {
                assert!(contrast(fg, bg) >= 0.3, "{name}: {fg} on {bg}");
            }
        }
    }

    /// The MetroBar theme sets every token, on either skin, and keeps Lemon's shapes on Lemon.
    #[test]
    fn metrobar_sets_every_token() {
        let names: Vec<&str> = metrobar_tokens().iter().map(|(n, _)| *n).collect();
        for n in Palette::NAMES {
            assert!(names.contains(n), "{n}");
        }
        let (_, text) = crate::presets::PRESETS.iter().find(|(n, _)| *n == "MetroBar").expect("MetroBar preset");
        let th = FlTheme::parse(text);
        assert!(th.metrobar);
        for d in Design::ALL {
            let p = palette(d, Some(&th));
            assert_eq!(p.accent, metrobar::ACCENT);
            assert_eq!(p.style.design, d);
        }
        let p = palette(Design::Lemon, Some(&th));
        assert_eq!(p.brand_lit(lemon::MAIN), metrobar::ACCENT);
        assert_eq!(p.brand_sel(lemon::MAIN), metrobar::ACCENT);
        assert!(p.brand_lit(lemon::HIGHLIGHT).hsl().2 < 0.45, "no glow: {}", p.brand_lit(lemon::HIGHLIGHT));
        // Light: Lemon's dark greys come out light, its text dark.
        let g = p.s(0x313436);
        assert!(g.hsl().2 > 0.8, "{g}");
        assert!(p.text.hsl().2 < 0.2 && p.workspace.hsl().2 > 0.85);
    }

    /// MetroBar Night: every token set, the first (dark) MetroBar palette, the same flat red.
    #[test]
    fn metrobar_night_is_the_dark_palette() {
        let names: Vec<&str> = metrobar_night_tokens().iter().map(|(n, _)| *n).collect();
        for n in Palette::NAMES {
            assert!(names.contains(n), "{n}");
        }
        let (_, text) = crate::presets::PRESETS.iter().find(|(n, _)| *n == "MetroBar Night").expect("MetroBar Night preset");
        let th = FlTheme::parse(text);
        assert!(th.metrobar && th.metrobar_night);
        let p = palette(Design::Lemon, Some(&th));
        assert_eq!((p.workspace, p.toolbar, p.text, p.accent), (metrobar_night::BG, metrobar_night::PANEL, metrobar_night::FG, metrobar::ACCENT));
        assert_eq!(p.brand_lit(lemon::MAIN), metrobar::ACCENT);
        assert!(p.brand_lit(lemon::HIGHLIGHT).hsl().2 < 0.45, "no glow");
        assert!(p.s(0x313436).hsl().2 < 0.2);
        let day = crate::presets::PRESETS.iter().find(|(n, _)| *n == "MetroBar").map(|(_, t)| FlTheme::parse(t)).unwrap();
        assert!(day.metrobar && !day.metrobar_night);
    }

    #[test]
    fn names_round_trip() {
        for d in Design::ALL {
            assert_eq!(Design::parse(d.name()), Some(d));
        }
        assert_eq!(Design::Lemon.next(), Design::Citrus);
        assert_eq!(Design::parse("Classic"), Some(Design::Citrus));
    }
}
