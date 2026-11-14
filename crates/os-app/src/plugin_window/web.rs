// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Plugin windows in the web build (MetroBar Studio in the browser): the frame (`super`) drawn into the main canvas.
//! There is one canvas and no native child window, so a frame is a rectangle the shell composites over the app's own
//! painting (`composite`), clipped like a docked window on the other platforms (`clip_to`: the workspace minus the
//! app's windows and popups above it). Mouse input over a frame goes to it (`event`): the frame's own parts through
//! the shared `State`, the editor area to the plugin's editor (`os_plugkit::web::deliver`). The plugin editors run in
//! `os_plugkit::web`, which also paints their menus and type-in boxes; the shell draws those above everything.
//!
//! Screen points here are main-canvas device px (the canvas is the whole window).
//!
// spec: ui/plugin-window.md, plugins/host-link.md

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::{Rc, Weak};

use os_plugkit::web as pk;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};

use super::*;

/// What a press landed on: a frame's own parts, its editor, or the open plugin menu.
#[derive(Clone)]
enum Grab {
    Frame(Weak<Frame>),
    Editor(Weak<Frame>),
}

thread_local! {
    /// The open frames, bottom to top.
    static FRAMES: RefCell<Vec<Weak<Frame>>> = const { RefCell::new(Vec::new()) };
    static NEXT_ID: Cell<u32> = const { Cell::new(1) };
    /// The mouse in the canvas (device px), once it moved.
    static POINTER: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
    /// Buttons held since a press on a frame, its editor or a plugin menu: their moves and release go there.
    static GRAB: RefCell<Option<Grab>> = const { RefCell::new(None) };
    static HELD: Cell<u8> = const { Cell::new(0) };
    /// The editor the mouse was last over (it gets a `Leave` when the mouse goes).
    static HOVER: RefCell<Option<Weak<Frame>>> = const { RefCell::new(None) };
    /// Canvas rects (device px: x0, y0, x1, y1) to paint again: where frames and menus were or are now.
    static DAMAGE: RefCell<Vec<[i32; 4]>> = const { RefCell::new(Vec::new()) };
    /// A press at this time and place, for double clicks.
    static LAST_PRESS: Cell<Option<(f64, i32, i32)>> = const { Cell::new(None) };
    /// The last plugin menu's bounds, damaged when it closes.
    static OVERLAY_AT: Cell<Option<[i32; 4]>> = const { Cell::new(None) };
    /// Last editor tick (ms).
    static LAST_TICK: Cell<f64> = const { Cell::new(0.0) };
}

fn now_ms() -> f64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

fn frames() -> Vec<Rc<Frame>> {
    FRAMES.with(|f| f.borrow().iter().filter_map(Weak::upgrade).collect())
}

fn damage(r: [i32; 4]) {
    if r[2] > r[0] && r[3] > r[1] {
        DAMAGE.with(|d| d.borrow_mut().push(r));
    }
}

/// Nothing to present again by itself: what frames change is damaged (`take_damage`).
pub fn take_exposed() -> bool {
    false
}

/// Canvas point to canvas point: the same place.
pub fn screen_to_main(x: i32, y: i32) -> Option<(i32, i32)> {
    Some((x, y))
}

/// A new plugin window's place: right of the browser, below the toolbar (logical offset, as on the desktop).
pub fn default_position(_owner: isize, scale: f32) -> (i32, i32) {
    let n = FRAMES.with(|f| f.borrow().len()) as f32;
    (((420.0 + 24.0 * n) * scale).round() as i32, ((160.0 + 24.0 * n) * scale).round() as i32)
}

/// No native windows to lift app popups over: frames are clipped around them instead.
pub fn lift_show(_main: isize, _frame: isize, _origin: (i32, i32), _size: (i32, i32), _pixels: Vec<u32>) {}

pub fn lift_hide() {}

pub fn take_lift_input() -> Vec<LiftInput> {
    Vec::new()
}

/// Puts the docked frames `hwnds` (bottom to top) in that order.
pub fn stack(_parent: isize, hwnds: &[isize]) {
    FRAMES.with(|fr| {
        let mut fr = fr.borrow_mut();
        let key = |w: &Weak<Frame>| w.upgrade().and_then(|f| hwnds.iter().position(|&h| h == f.id as isize)).map_or(-1, |i| i as isize);
        let before: Vec<isize> = fr.iter().map(key).collect();
        fr.sort_by_key(key);
        let after: Vec<isize> = fr.iter().map(key).collect();
        if before != after {
            for f in fr.iter().filter_map(Weak::upgrade) {
                damage(f.bounds());
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The frame.

struct Frame {
    id: u32,
    /// The editor area's id: the "window" the plugin's editor is given (`os_plugkit::web`).
    area: u32,
    /// Top-left in the canvas (device px).
    pos: Cell<(i32, i32)>,
    state: RefCell<State>,
    canvas: RefCell<Canvas>,
    scale: Cell<f32>,
    /// Hidden (minimized, or not raised yet).
    hidden: Cell<bool>,
    /// The parts shown (canvas device px: x0, y0, x1, y1); `None`: all of it.
    shown: RefCell<Option<Vec<[i32; 4]>>>,
    /// The chrome needs painting again.
    stale: Cell<bool>,
    wheel: Cell<f32>,
}

/// A plugin window. Dropping it removes it (close the plugin's editor first).
pub struct FrameWindow {
    f: Rc<Frame>,
}

impl FrameWindow {
    /// A hidden frame for an editor of `editor` logical px at canvas point `pos`; `raise` shows it.
    pub fn new(_owner: isize, info: ChannelInfo, editor: (f32, f32), scale: f32, pos: (i32, i32), _detached: bool) -> Result<FrameWindow, String> {
        let id = NEXT_ID.with(|n| n.replace(n.get() + 2));
        let state = State::new(info, editor, false);
        let f = Rc::new(Frame {
            id,
            area: id + 1,
            pos: Cell::new(pos),
            state: RefCell::new(state),
            canvas: RefCell::new(Canvas::new(1, 1, 1.0).map_err(|e| e.to_string())?),
            scale: Cell::new(scale.max(0.5)),
            hidden: Cell::new(true),
            shown: RefCell::new(None),
            stale: Cell::new(true),
            wheel: Cell::new(0.0),
        });
        f.layout();
        FRAMES.with(|fr| fr.borrow_mut().push(Rc::downgrade(&f)));
        Ok(FrameWindow { f })
    }

    /// Frames are always docked in the browser.
    pub fn detached(&self) -> bool {
        false
    }

    pub fn set_detached(&self, _owner: isize, _on: bool) {}

    /// Clips the frame to `work` minus `covers` (canvas device px, x0 y0 x1 y1).
    pub fn clip_to(&self, work: [i32; 4], covers: &[[i32; 4]]) {
        self.f.state.borrow_mut().clip = Some((work, covers.to_vec()));
        self.f.refit();
        self.f.apply_clip();
    }

    pub fn hwnd(&self) -> isize {
        self.f.id as isize
    }

    /// The editor area's id, as plugin APIs take a window (an X11-style id: the CLAP X11 API in the browser).
    pub fn area(&self) -> *mut c_void {
        self.f.area as usize as *mut c_void
    }

    pub fn raise(&self) {
        let f = &self.f;
        f.hidden.set(false);
        f.front();
        f.stale.set(true);
        damage(f.bounds());
    }

    pub fn set_info(&self, info: ChannelInfo) {
        let mut st = self.f.state.borrow_mut();
        if st.info != info && st.drag.is_none() && st.misc_ui.drag.is_none() {
            st.info = info;
            drop(st);
            self.f.repaint();
        }
    }

    pub fn set_pages(&self, pages: Vec<PageIcon>, active: usize) {
        let mut st = self.f.state.borrow_mut();
        if !pages.is_empty() && (st.pages != pages || st.page != active) {
            st.page = active.min(pages.len() - 1);
            st.pages = pages;
            drop(st);
            self.f.repaint();
        }
    }

    pub fn set_resizable(&self, min: Option<(f32, f32)>) {
        let mut st = self.f.state.borrow_mut();
        if st.resize_min != min {
            st.resize_min = min;
            if min.is_none() {
                st.sizing = None;
            }
        }
    }

    pub fn set_page(&self, page: usize) {
        let mut st = self.f.state.borrow_mut();
        if st.page != page && page < st.pages.len() {
            st.page = page;
            drop(st);
            self.f.repaint();
        }
    }

    pub fn set_hint(&self, hint: String) {
        let mut st = self.f.state.borrow_mut();
        if st.hint != hint {
            st.hint = hint;
            drop(st);
            self.f.repaint();
        }
    }

    pub fn set_preset(&self, name: Option<String>) {
        let mut st = self.f.state.borrow_mut();
        if st.preset != name {
            st.preset = name;
            drop(st);
            self.f.repaint();
        }
    }

    pub fn scale(&self) -> f32 {
        self.f.scale.get()
    }

    /// A point of the editor (logical px from its top-left) in the canvas.
    pub fn editor_to_screen(&self, x: f32, y: f32) -> (i32, i32) {
        let ay = self.f.state.borrow().area_y();
        self.f.to_screen(BORDER + x, ay + y)
    }

    #[allow(dead_code)]
    pub fn set_scale(&self, scale: f32) {
        if scale <= 0.0 || self.f.scale.get() == scale {
            return;
        }
        self.f.scale.set(scale);
        self.f.state.borrow_mut().clip_key = None;
        self.f.layout();
    }

    pub fn set_editor_size(&self, editor: (f32, f32)) {
        if self.f.state.borrow().editor != editor {
            self.f.state.borrow_mut().editor = editor;
            self.f.layout();
        }
    }

    pub fn set_strip(&self, on: bool) {
        if self.f.state.borrow().strip != on {
            self.f.state.borrow_mut().strip = on;
            self.f.layout();
        }
    }

    pub fn take_events(&self) -> Vec<FrameEvent> {
        std::mem::take(&mut self.f.state.borrow_mut().events)
    }
}

impl Drop for FrameWindow {
    fn drop(&mut self) {
        let f = &self.f;
        damage(f.bounds());
        FRAMES.with(|fr| fr.borrow_mut().retain(|w| w.upgrade().is_some_and(|g| !Rc::ptr_eq(&g, f))));
        GRAB.with(|g| {
            let mut g = g.borrow_mut();
            if matches!(&*g, Some(Grab::Frame(w) | Grab::Editor(w)) if w.upgrade().is_none_or(|g| Rc::ptr_eq(&g, f))) {
                *g = None;
            }
        });
    }
}

impl Frame {
    /// Size in device px.
    fn device_size(&self) -> (i32, i32) {
        let (w, h) = self.state.borrow().size();
        let k = self.scale.get();
        ((w * k).round().max(1.0) as i32, (h * k).round().max(1.0) as i32)
    }

    /// x0, y0, x1, y1 in the canvas.
    fn bounds(&self) -> [i32; 4] {
        let (x, y) = self.pos.get();
        let (w, h) = self.device_size();
        [x, y, x + w, y + h]
    }

    /// The editor area in the canvas (x, y, w, h device px), `None` while the Misc page shows instead.
    fn area_rect(&self) -> Option<[i32; 4]> {
        let st = self.state.borrow();
        if st.misc {
            return None;
        }
        let k = self.scale.get();
        let (x, y) = self.pos.get();
        let (w, h) = pk::size(self.area).map_or(((st.editor.0 * k) as i32, (st.editor.1 * k) as i32), |(w, h)| (w as i32, h as i32));
        Some([x + (BORDER * k).round() as i32, y + (st.area_y() * k).round() as i32, w, h])
    }

    fn to_screen(&self, x: f32, y: f32) -> (i32, i32) {
        let k = self.scale.get();
        let (px, py) = self.pos.get();
        (px + (x * k).round() as i32, py + (y * k).round() as i32)
    }

    fn top_left(&self) -> (i32, i32) {
        self.pos.get()
    }

    fn place(&self, at: (i32, i32)) {
        if at != self.pos.get() {
            damage(self.bounds());
            self.pos.set(at);
            self.state.borrow_mut().clip_key = None;
            self.apply_clip();
            damage(self.bounds());
        }
    }

    fn layout(&self) {
        damage(self.bounds());
        let (w, h) = self.device_size();
        let k = self.scale.get();
        self.canvas.borrow_mut().resize(w as u32, h as u32, k);
        self.state.borrow_mut().clip_key = None;
        self.apply_clip();
        self.repaint();
    }

    fn repaint(&self) {
        self.stale.set(true);
        damage(self.bounds());
    }

    fn front(&self) {
        FRAMES.with(|fr| {
            let mut fr = fr.borrow_mut();
            if let Some(i) = fr.iter().position(|w| w.upgrade().is_some_and(|g| g.id == self.id)) {
                let me = fr.remove(i);
                fr.push(me);
            }
        });
        damage(self.bounds());
    }

    fn shows(&self, p: (i32, i32)) -> bool {
        if self.hidden.get() {
            return false;
        }
        let b = self.bounds();
        let inside = |r: &[i32; 4]| p.0 >= r[0] && p.1 >= r[1] && p.0 < r[2] && p.1 < r[3];
        inside(&b) && self.shown.borrow().as_ref().is_none_or(|rs| rs.iter().any(inside))
    }

    fn apply_clip(&self) {
        let me = self.bounds();
        let rects = {
            let mut st = self.state.borrow_mut();
            let Some((work, covers)) = st.clip.as_ref() else { return };
            let hit = |c: &[i32; 4]| c[0] < me[2] && c[2] > me[0] && c[1] < me[3] && c[3] > me[1];
            let covers: Vec<[i32; 4]> = covers.iter().copied().filter(hit).collect();
            let work = *work;
            let key: Vec<i32> = me.into_iter().chain(work).chain(covers.iter().flatten().copied()).collect();
            if st.clip_key.as_ref() == Some(&key) {
                return;
            }
            st.clip_key = Some(key);
            let inside = work[0] <= me[0] && work[1] <= me[1] && work[2] >= me[2] && work[3] >= me[3];
            (!inside || !covers.is_empty()).then(|| visible(me, work, &covers))
        };
        *self.shown.borrow_mut() = rects;
        damage(me);
    }

    fn max_target(&self) -> Option<((i32, i32), (f32, f32))> {
        let k = self.scale.get();
        let work = self.state.borrow().clip.as_ref()?.0;
        Some(((work[0], work[1]), ((work[2] - work[0]) as f32 / k, (work[3] - work[1]) as f32 / k)))
    }

    fn refit(&self) {
        if self.state.borrow().maxed.is_none() {
            return;
        }
        let Some((at, size)) = self.max_target() else { return };
        let fill = {
            let mut st = self.state.borrow_mut();
            let fill = st.fill(size);
            if let Some((w, h)) = fill.filter(|&(w, h)| (w as f32, h as f32) != st.editor) {
                st.events.push(FrameEvent::Resize(w, h));
                os_clap::wake();
            }
            fill
        };
        if fill.is_some() {
            self.place(at);
        }
    }

    fn toggle_max(&self) {
        let target = self.max_target();
        let at = self.state.borrow_mut().maximize(self.top_left(), target);
        if let Some(at) = at {
            self.place(at);
            self.front();
        }
    }

    /// Frame point (logical px) of canvas point `p`.
    fn local(&self, p: (i32, i32)) -> (f32, f32) {
        let k = self.scale.get();
        let (x, y) = self.pos.get();
        ((p.0 - x) as f32 / k, (p.1 - y) as f32 / k)
    }

    fn reply(&self, r: Reply, p: (i32, i32)) {
        match r {
            Reply::None => {}
            Reply::Repaint | Reply::Capture | Reply::Hold => self.repaint(),
            Reply::Relayout => self.layout(),
            Reply::Move => {
                let (wx, wy) = self.top_left();
                self.state.borrow_mut().moving = Some((p.0, p.1, wx, wy));
                self.repaint();
            }
            Reply::Maximize => self.toggle_max(),
            Reply::Minimize => {
                self.hidden.set(true);
                damage(self.bounds());
            }
        }
    }

    fn clicked(&self) {
        self.front();
        self.state.borrow_mut().events.push(FrameEvent::Raise);
        os_clap::wake();
    }

    fn press(&self, p: (i32, i32), b: Btn, dbl: bool) {
        let (x, y) = self.local(p);
        let screen = |x: f32, y: f32| self.to_screen(x, y);
        let r = {
            let mut st = self.state.borrow_mut();
            match b {
                Btn::Left => st.left_down(x, y, dbl, &screen),
                Btn::Right => st.right_down(x, y, &screen),
                Btn::Middle => st.middle_down(x, y, &screen),
            }
        };
        self.reply(r, p);
    }

    fn motion(&self, p: (i32, i32), fine: bool) {
        let moving = self.state.borrow().moving;
        if let Some((cx, cy, wx, wy)) = moving {
            self.place((wx + p.0 - cx, wy + p.1 - cy));
            return;
        }
        let (x, y) = self.local(p);
        let r = self.state.borrow_mut().motion(x, y, fine);
        self.reply(r, p);
    }

    fn up(&self, p: (i32, i32)) {
        let (x, y) = self.local(p);
        let r = self.state.borrow_mut().left_up(x, y);
        self.reply(r, p);
    }

    fn wheel(&self, p: (i32, i32), notches: f32) {
        let total = self.wheel.get() + notches;
        let whole = total.trunc();
        self.wheel.set(total - whole);
        if whole != 0.0 {
            let (x, y) = self.local(p);
            let r = self.state.borrow_mut().wheel(x, y, whole);
            self.reply(r, p);
        }
    }

    /// Hands an input to the editor (window device px).
    fn editor_input(&self, input: pk::Input) {
        pk::deliver(self.area, input);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The shell's side: input, ticks, damage and compositing.

/// The X11 key symbol of a key, for plugin menus and type-in boxes.
fn keysym(key: &Key) -> Option<u32> {
    use os_plugkit::input::keysym as ks;
    Some(match key {
        Key::Named(NamedKey::Enter) => ks::RETURN,
        Key::Named(NamedKey::Escape) => ks::ESCAPE,
        Key::Named(NamedKey::Backspace) => ks::BACKSPACE,
        Key::Named(NamedKey::Delete) => ks::DELETE,
        Key::Named(NamedKey::ArrowLeft) => ks::LEFT,
        Key::Named(NamedKey::ArrowRight) => ks::RIGHT,
        Key::Named(NamedKey::ArrowUp) => ks::UP,
        Key::Named(NamedKey::ArrowDown) => ks::DOWN,
        Key::Named(NamedKey::Home) => ks::HOME,
        Key::Named(NamedKey::End) => ks::END,
        Key::Named(NamedKey::Space) => 0x20,
        Key::Character(s) => {
            let c = s.chars().next()?;
            let u = c as u32;
            if (0x20..=0x7e).contains(&u) || (0xa0..=0xff).contains(&u) { u } else { 0x0100_0000 + u }
        }
        _ => return None,
    })
}

/// The frame under canvas point `p`, the topmost.
fn frame_at(p: (i32, i32)) -> Option<Rc<Frame>> {
    frames().into_iter().rev().find(|f| f.shows(p))
}

fn inside(r: [i32; 4], p: (i32, i32)) -> bool {
    p.0 >= r[0] && p.1 >= r[1] && p.0 < r[0] + r[2] && p.1 < r[1] + r[3]
}

/// Input in the main window that a plugin window or plugin menu takes: true when it took it (the app does not see
/// it). `inset`: where the canvas sits in the window (physical px).
pub fn event(e: &WindowEvent, inset: (u32, u32), mods: ModifiersState) -> bool {
    let ctrl = mods.control_key() || mods.super_key();
    pk::set_mods(ctrl, mods.shift_key(), mods.alt_key());
    let at = |x: f64, y: f64| (x as i32 - inset.0 as i32, y as i32 - inset.1 as i32);
    match e {
        WindowEvent::CursorMoved { position, .. } => {
            let p = at(position.x, position.y);
            POINTER.with(|c| c.set(Some(p)));
            mouse_move(p, ctrl)
        }
        WindowEvent::MouseInput { state, button, .. } => {
            let Some(p) = POINTER.with(Cell::get) else { return false };
            let b = match button {
                MouseButton::Left => pk::Button::Left,
                MouseButton::Right => pk::Button::Right,
                MouseButton::Middle => pk::Button::Middle,
                _ => return false,
            };
            if *state == ElementState::Pressed { mouse_down(p, b) } else { mouse_up(p, b) }
        }
        WindowEvent::MouseWheel { delta, .. } => {
            let Some(p) = POINTER.with(Cell::get) else { return false };
            let notches = match delta {
                MouseScrollDelta::LineDelta(_, y) => *y,
                MouseScrollDelta::PixelDelta(d) => d.y as f32 / 40.0,
            };
            if pk::overlay_open() {
                return true;
            }
            let Some(f) = frame_at(p) else { return false };
            match f.area_rect().filter(|r| inside(*r, p)) {
                Some(r) => f.editor_input(pk::Input::Wheel { x: (p.0 - r[0]) as f32, y: (p.1 - r[1]) as f32, notches }),
                None => f.wheel(p, notches),
            }
            true
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if !pk::overlay_open() {
                return false;
            }
            if event.state == ElementState::Pressed {
                if let Some(sym) = keysym(&event.logical_key) {
                    pk::overlay_input(pk::Input::Key { sym, text: os_plugkit::input::keysym_char(sym) });
                    note_overlay();
                }
            }
            true
        }
        _ => false,
    }
}

/// A plugin menu opened, changed or closed: its old and new places need painting.
fn note_overlay() {
    let now = pk::overlay(|b, _| [b[0], b[1], b[0] + b[2], b[1] + b[3]]);
    if let Some(old) = OVERLAY_AT.with(|o| o.replace(now)) {
        damage(old);
    }
    if let Some(r) = now {
        damage(r);
    }
}

fn mouse_move(p: (i32, i32), fine: bool) -> bool {
    if pk::overlay_open() {
        pk::overlay_input(pk::Input::Move { x: p.0 as f32, y: p.1 as f32 });
        note_overlay();
        return true;
    }
    let grab = GRAB.with(|g| g.borrow().clone());
    match grab {
        Some(Grab::Frame(w)) => {
            if let Some(f) = w.upgrade() {
                f.motion(p, fine);
            }
            true
        }
        Some(Grab::Editor(w)) => {
            if let Some(f) = w.upgrade() {
                let r = f.area_rect().unwrap_or([0, 0, 0, 0]);
                f.editor_input(pk::Input::Move { x: (p.0 - r[0]) as f32, y: (p.1 - r[1]) as f32 });
            }
            true
        }
        None => {
            let over = frame_at(p);
            // Over an editor: it hovers; leaving one tells it.
            let ed = over.as_ref().and_then(|f| f.area_rect().filter(|r| inside(*r, p)).map(|r| (f.clone(), r)));
            let was = HOVER.with(|h| h.borrow().as_ref().and_then(Weak::upgrade));
            if let Some(w) = was.filter(|w| ed.as_ref().is_none_or(|(f, _)| f.id != w.id)) {
                w.editor_input(pk::Input::Leave);
            }
            HOVER.with(|h| *h.borrow_mut() = ed.as_ref().map(|(f, _)| Rc::downgrade(f)));
            match (over, ed) {
                (_, Some((f, r))) => {
                    f.editor_input(pk::Input::Move { x: (p.0 - r[0]) as f32, y: (p.1 - r[1]) as f32 });
                    true
                }
                (Some(f), None) => {
                    f.motion(p, fine);
                    true
                }
                (None, None) => false,
            }
        }
    }
}

fn mouse_down(p: (i32, i32), b: pk::Button) -> bool {
    if pk::overlay_open() {
        pk::overlay_input(pk::Input::Down { x: p.0 as f32, y: p.1 as f32, btn: b, dbl: false });
        note_overlay();
        // A press outside the menu closes it and is not the app's either.
        HELD.with(|h| h.set(h.get() | bit(b)));
        return true;
    }
    if HELD.with(Cell::get) == 0 {
        GRAB.with(|g| *g.borrow_mut() = None);
    }
    let held = GRAB.with(|g| g.borrow().clone());
    let target = match held {
        Some(Grab::Frame(w) | Grab::Editor(w)) => w.upgrade(),
        None => frame_at(p),
    };
    let Some(f) = target else { return false };
    let t = now_ms();
    let dbl = b == pk::Button::Left && LAST_PRESS.with(Cell::get).is_some_and(|(at, x, y)| t - at < 400.0 && (x - p.0).abs() < 4 && (y - p.1).abs() < 4);
    LAST_PRESS.with(|l| l.set(if dbl || b != pk::Button::Left { None } else { Some((t, p.0, p.1)) }));
    HELD.with(|h| h.set(h.get() | bit(b)));
    f.clicked();
    match f.area_rect().filter(|r| inside(*r, p)) {
        Some(r) => {
            GRAB.with(|g| *g.borrow_mut() = Some(Grab::Editor(Rc::downgrade(&f))));
            f.editor_input(pk::Input::Down { x: (p.0 - r[0]) as f32, y: (p.1 - r[1]) as f32, btn: b, dbl });
        }
        None => {
            GRAB.with(|g| *g.borrow_mut() = Some(Grab::Frame(Rc::downgrade(&f))));
            let btn = match b {
                pk::Button::Left => Btn::Left,
                pk::Button::Right => Btn::Right,
                pk::Button::Middle => Btn::Middle,
            };
            f.press(p, btn, dbl);
        }
    }
    note_overlay();
    true
}

fn bit(b: pk::Button) -> u8 {
    match b {
        pk::Button::Left => 1,
        pk::Button::Right => 2,
        pk::Button::Middle => 4,
    }
}

fn mouse_up(p: (i32, i32), b: pk::Button) -> bool {
    let was = HELD.with(|h| h.replace(h.get() & !bit(b)));
    if pk::overlay_open() {
        // The pointer's place first: the release picks the row under it.
        pk::overlay_input(pk::Input::Move { x: p.0 as f32, y: p.1 as f32 });
        pk::overlay_input(pk::Input::Up { btn: b });
        note_overlay();
        return true;
    }
    let grab = GRAB.with(|g| g.borrow().clone());
    if HELD.with(Cell::get) == 0 {
        GRAB.with(|g| *g.borrow_mut() = None);
    }
    match grab {
        Some(Grab::Frame(w)) => {
            if let Some(f) = w.upgrade() {
                if b == pk::Button::Left {
                    f.up(p);
                } else {
                    f.repaint();
                }
            }
            note_overlay();
            true
        }
        Some(Grab::Editor(w)) => {
            if let Some(f) = w.upgrade() {
                f.editor_input(pk::Input::Up { btn: b });
            }
            note_overlay();
            true
        }
        // The release of a press that closed a plugin menu.
        None => was & bit(b) != 0 && frame_at(p).is_some(),
    }
}

/// The editors' polling tick (about every 30 ms) and what changed since: true when something needs painting.
pub fn tick() -> bool {
    let t = now_ms();
    if t - LAST_TICK.with(Cell::get) >= 30.0 {
        LAST_TICK.with(|l| l.set(t));
        pk::tick(false);
    } else {
        pk::tick(true);
    }
    if pk::take_dirty() {
        for f in frames() {
            if !f.hidden.get() {
                damage(f.bounds());
            }
        }
        note_overlay();
    }
    DAMAGE.with(|d| !d.borrow().is_empty())
}

/// Canvas rects (device px: x0, y0, x1, y1) to paint again since the last call.
pub fn take_damage() -> Vec<[i32; 4]> {
    DAMAGE.with(|d| std::mem::take(&mut *d.borrow_mut()))
}

/// Paints the plugin windows and the open plugin menu over the app's canvas.
pub fn composite(canvas: &mut Canvas) {
    let (cw, ch) = (canvas.pixmap.width() as i32, canvas.pixmap.height() as i32);
    pk::set_screen(cw, ch);
    for f in frames() {
        if f.hidden.get() {
            continue;
        }
        if f.stale.replace(false) {
            let st = f.state.borrow();
            super::paint(&mut f.canvas.borrow_mut(), &st);
        }
        let b = f.bounds();
        let area = f.area_rect();
        if let Some(a) = area {
            pk::set_origin(f.area, (a[0], a[1]));
        }
        let shown = f.shown.borrow().clone().unwrap_or_else(|| vec![b]);
        let fc = f.canvas.borrow();
        let fw = fc.pixmap.width() as i32;
        let src = fc.pixmap.data();
        let dst = canvas.pixmap.data_mut();
        for r in shown {
            let (x0, y0, x1, y1) = (r[0].max(b[0]).max(0), r[1].max(b[1]).max(0), r[2].min(b[2]).min(cw), r[3].min(b[3]).min(ch));
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
            // The chrome.
            for y in y0..y1 {
                let s = (((y - b[1]) * fw + (x0 - b[0])) * 4) as usize;
                let d = ((y * cw + x0) * 4) as usize;
                let n = ((x1 - x0) * 4) as usize;
                if s + n <= src.len() && d + n <= dst.len() {
                    dst[d..d + n].copy_from_slice(&src[s..s + n]);
                }
            }
            // The editor's picture over its area.
            if let Some(a) = area {
                pk::pixels(f.area, |px, w, h| {
                    let (ex0, ey0) = (x0.max(a[0]), y0.max(a[1]));
                    let (ex1, ey1) = (x1.min(a[0] + w as i32).min(a[0] + a[2]), y1.min(a[1] + h as i32).min(a[1] + a[3]));
                    for y in ey0..ey1 {
                        for x in ex0..ex1 {
                            let p = px[((y - a[1]) as u32 * w + (x - a[0]) as u32) as usize];
                            let d = ((y * cw + x) * 4) as usize;
                            dst[d] = (p >> 16) as u8;
                            dst[d + 1] = (p >> 8) as u8;
                            dst[d + 2] = p as u8;
                            dst[d + 3] = 255;
                        }
                    }
                });
            }
        }
    }
    // A plugin's menu or type-in box: above everything.
    pk::overlay(|r, px| {
        let dst = canvas.pixmap.data_mut();
        for y in r[1].max(0)..(r[1] + r[3]).min(ch) {
            for x in r[0].max(0)..(r[0] + r[2]).min(cw) {
                let p = px[((y - r[1]) * r[2] + (x - r[0])) as usize];
                if p & pk::ALPHA == 0 {
                    continue;
                }
                let d = ((y * cw + x) * 4) as usize;
                dst[d] = (p >> 16) as u8;
                dst[d + 1] = (p >> 8) as u8;
                dst[d + 2] = p as u8;
                dst[d + 3] = 255;
            }
        }
    });
}

/// A plugin window is open: the app keeps ticking for its editor (meters, knob moves, paints).
pub fn busy() -> bool {
    !frames().is_empty() || pk::overlay_open()
}
