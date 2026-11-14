// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! The web build's editor windows (MetroBar Studio in the browser): the `x11` API without threads or a display. The
//! page has one canvas and one thread, so an editor cannot block in a loop of its own: `spawn` builds the editor at
//! once and `crate::drive` keeps it here, and the host (os-app's `plugin_window::web`) hands it its input one event at
//! a time (`deliver`, `tick`) and composites what it painted (`pixels`) into the main canvas.
//!
//! Popup menus and the "Type in value" box are overlays the host paints above everything (`overlay`, `overlay_input`).
//! The native ones are modal and return the pick; here the call returns `None` at once and the menu stays open. When
//! the user picks, the input that opened it is delivered again with the answer waiting, and the same `popup` /
//! `type_in` call returns it this time (`Answer`): the editors' code stays as it is on Linux and macOS.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use web_time::{Duration, Instant};

use os_theme::{Color, Palette};
use os_ui::kit::menu::{self as kmenu, MenuItem, MenuLayout, MenuStyle};
use os_ui::{Canvas, Font, Rect};

pub use crate::input::{keysym, keysym_char, Button, Input};
use crate::{MenuDef, Step};

/// The host's window: an id the host gives each editor area (`plugin_window::web`).
pub type Parent = u32;

/// Marks an open editor for a look again (a parameter changed). Any thread in the native builds; here only the one.
#[derive(Clone, Default)]
pub struct Waker(Arc<AtomicBool>);

impl Waker {
    pub fn wake(&self) {
        self.0.store(true, Ordering::Release);
    }
}

/// Where an editor keeps its open window's waker.
pub type Slot = Arc<Mutex<Option<Waker>>>;

/// A parameter changed: the open editor (if any) gets a tick soon.
pub fn notify(slot: &Slot) {
    if let Ok(s) = slot.lock() {
        if let Some(w) = s.as_ref() {
            w.wake();
        }
    }
}

/// An open editor. Dropping it closes the editor.
pub struct EditorThread {
    parent: Parent,
    slot: Slot,
}

impl Drop for EditorThread {
    fn drop(&mut self) {
        if let Ok(mut s) = self.slot.lock() {
            *s = None;
        }
        close(self.parent);
    }
}

/// What a reopened `popup` or `type_in` returns at once (see the module's notes).
#[derive(Clone, Debug)]
enum Answer {
    /// The picked row: indices from the top menu down through its submenus.
    Menu(Vec<usize>),
    Text(String),
}

/// One editor window's state, shared by its `Window` (the editor's) and the host side of this module.
struct Shared {
    parent: Parent,
    /// The editor area's top-left in the main canvas (device px).
    origin: Cell<(i32, i32)>,
    /// The size the editor asked for (device px).
    size: Cell<(u32, u32)>,
    /// The last frame the editor painted (XRGB rows) and its size.
    px: RefCell<(Vec<u32>, u32, u32)>,
    /// The pointer's last place in the window, and whether the cursor is held (`lock`: moves become `Input::Delta`).
    last: Cell<(f32, f32)>,
    locked: Cell<bool>,
    /// Answers waiting for the `popup` / `type_in` calls of the input delivered again, in order: a menu row that opens a
    /// box ("Enter value…") needs the row, then the text.
    answers: RefCell<std::collections::VecDeque<Answer>>,
    /// The answers taken while handling the current input: a menu or box opened after them is reached through them.
    taken: RefCell<Vec<Answer>>,
    /// The input being handled: what opens a menu, delivered again with the pick.
    trigger: RefCell<Option<Input>>,
    wake: Waker,
}

thread_local! {
    static WINS: RefCell<Vec<Rc<Shared>>> = const { RefCell::new(Vec::new()) };
    /// The editors by window; `None` while one is handling an input (taken out, so it may open or close others).
    static EDS: RefCell<Vec<(Parent, Option<Box<dyn Step>>)>> = const { RefCell::new(Vec::new()) };
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
    /// Something to composite again since the last `take_dirty`.
    static DIRTY: Cell<bool> = const { Cell::new(false) };
    /// Ctrl, Shift, Alt at the last input the host passed.
    static MODS: Cell<(bool, bool, bool)> = const { Cell::new((false, false, false)) };
    /// The main canvas (device px): where menus must fit.
    static SCREEN: Cell<(i32, i32)> = const { Cell::new((1280, 800)) };
}

fn shared(parent: Parent) -> Option<Rc<Shared>> {
    WINS.with(|w| w.borrow().iter().find(|s| s.parent == parent).cloned())
}

fn dirty() {
    DIRTY.with(|d| d.set(true));
}

/// Opens an editor window of `size` device px for host area `parent` and builds the editor with it (`run`) at once.
pub fn spawn(parent: Parent, size: (u32, u32), slot: &Slot, run: impl FnOnce(Window)) -> Option<EditorThread> {
    close(parent);
    let wake = Waker::default();
    let sh = Rc::new(Shared {
        parent,
        origin: Cell::new((0, 0)),
        size: Cell::new(size),
        px: RefCell::new((Vec::new(), 0, 0)),
        last: Cell::new((0.0, 0.0)),
        locked: Cell::new(false),
        answers: RefCell::new(Default::default()),
        taken: RefCell::new(Vec::new()),
        trigger: RefCell::new(None),
        wake: wake.clone(),
    });
    WINS.with(|w| w.borrow_mut().push(sh.clone()));
    if let Ok(mut s) = slot.lock() {
        *s = Some(wake);
    }
    run(Window { sh });
    // The editor gave up before `drive` (no canvas, say): nothing is open.
    if !EDS.with(|e| e.borrow().iter().any(|(p, _)| *p == parent)) {
        WINS.with(|w| w.borrow_mut().retain(|s| s.parent != parent));
        return None;
    }
    dirty();
    Some(EditorThread { parent, slot: slot.clone() })
}

/// A floating window of a plugin's own: none in the browser.
pub fn spawn_floating(_rect: [i32; 4], _min: u32, _title: &str, _run: impl FnOnce(Window)) -> Option<EditorThread> {
    None
}

/// Keeps a built editor (`crate::drive`): it gets its input from the host from now on.
pub(crate) fn register(ed: Box<dyn Step>, parent: Parent) {
    EDS.with(|e| {
        let mut e = e.borrow_mut();
        e.retain(|(p, _)| *p != parent);
        e.push((parent, Some(ed)));
    });
    // A first paint.
    run(parent, Input::Expose);
}

fn close(parent: Parent) {
    let ed = EDS.with(|e| {
        let mut e = e.borrow_mut();
        let i = e.iter().position(|(p, _)| *p == parent)?;
        Some(e.remove(i).1)
    });
    // Dropped outside the borrow: an editor's drop may reach the host.
    drop(ed);
    WINS.with(|w| w.borrow_mut().retain(|s| s.parent != parent));
    OVERLAY.with(|o| {
        let mut o = o.borrow_mut();
        if o.as_ref().is_some_and(|ov| ov.parent == parent) {
            *o = None;
        }
    });
    dirty();
}

/// Hands `input` to the editor of `parent`; false when it closed itself.
fn run(parent: Parent, input: Input) -> bool {
    let Some(mut ed) = EDS.with(|e| e.borrow_mut().iter_mut().find(|(p, _)| *p == parent).and_then(|(_, ed)| ed.take())) else { return false };
    let keep = ed.step(input);
    let back = EDS.with(|e| {
        let mut e = e.borrow_mut();
        match e.iter_mut().find(|(p, _)| *p == parent) {
            Some((_, slot)) if keep && slot.is_none() => {
                *slot = Some(ed);
                None
            }
            // Closed while it ran (or it is done): drop it after the borrow.
            _ => Some(ed),
        }
    });
    if back.is_some() {
        drop(back);
        return false;
    }
    true
}

// -------------------------------------------------------------------------------------------------------------------
// The host's side (os-app `plugin_window::web`).

/// The open editors' windows.
pub fn editors() -> Vec<Parent> {
    WINS.with(|w| w.borrow().iter().map(|s| s.parent).collect())
}

/// Where the editor area of `parent` is in the main canvas (device px).
pub fn set_origin(parent: Parent, at: (i32, i32)) {
    if let Some(s) = shared(parent) {
        s.origin.set(at);
    }
}

/// The main canvas size (device px).
pub fn set_screen(w: i32, h: i32) {
    SCREEN.with(|s| s.set((w.max(1), h.max(1))));
}

/// The size the editor of `parent` asked for (device px).
pub fn size(parent: Parent) -> Option<(u32, u32)> {
    shared(parent).map(|s| s.size.get())
}

/// The editor's last frame: XRGB rows, width, height.
pub fn pixels<R>(parent: Parent, f: impl FnOnce(&[u32], u32, u32) -> R) -> Option<R> {
    let s = shared(parent)?;
    let px = s.px.borrow();
    Some(f(&px.0, px.1, px.2))
}

/// Whether anything was painted since the last call.
pub fn take_dirty() -> bool {
    DIRTY.with(|d| d.replace(false))
}

/// Ctrl, Shift, Alt for the inputs that follow.
pub fn set_mods(ctrl: bool, shift: bool, alt: bool) {
    MODS.with(|m| m.set((ctrl, shift, alt)));
}

/// Mouse or keyboard input for the editor of `parent` (window device px).
pub fn deliver(parent: Parent, input: Input) {
    let Some(s) = shared(parent) else { return };
    let input = match input {
        Input::Move { x, y } => {
            let (lx, ly) = s.last.replace((x, y));
            if s.locked.get() {
                let (dx, dy) = (x - lx, y - ly);
                if dx == 0.0 && dy == 0.0 {
                    return;
                }
                Input::Delta { dx, dy }
            } else {
                input
            }
        }
        Input::Down { x, y, .. } | Input::Wheel { x, y, .. } => {
            s.last.set((x, y));
            input
        }
        _ => input,
    };
    if !matches!(input, Input::Tick | Input::Expose | Input::Move { .. } | Input::Delta { .. } | Input::Leave) {
        *s.trigger.borrow_mut() = Some(input.clone());
        s.answers.borrow_mut().clear();
        s.taken.borrow_mut().clear();
    }
    run(parent, input);
}

/// The polling tick for every open editor (the host calls it about every 30 ms); `woken_only`: only those a parameter
/// change marked.
pub fn tick(woken_only: bool) {
    for p in editors() {
        let Some(s) = shared(p) else { continue };
        let woke = s.wake.0.swap(false, Ordering::AcqRel);
        if woke || !woken_only {
            run(p, Input::Tick);
        }
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The editor's window.

/// An editor's window: the host's area of the main canvas.
pub struct Window {
    sh: Rc<Shared>,
}

impl Window {
    pub(crate) fn parent(&self) -> Parent {
        self.sh.parent
    }

    /// Input comes one event at a time: always a good moment to paint.
    pub fn idle(&self) -> bool {
        true
    }

    pub fn mods(&self) -> (bool, bool, bool) {
        MODS.with(Cell::get)
    }

    /// Editors here get their input from `deliver`, never by waiting: a loop asking for more ends.
    pub fn next(&mut self) -> Option<Input> {
        None
    }

    /// Takes `px` (XRGB rows of `w`) as the window's picture.
    pub fn blit(&self, px: &[u32], w: u32, h: u32) {
        let mut b = self.sh.px.borrow_mut();
        b.0.clear();
        b.0.extend_from_slice(&px[..(w * h) as usize]);
        b.1 = w;
        b.2 = h;
        dirty();
    }

    pub fn resize(&self, w: u32, h: u32) {
        self.sh.size.set((w.max(1), h.max(1)));
        dirty();
    }

    /// A window point in the main canvas (device px).
    pub fn to_root(&self, x: i32, y: i32) -> (i32, i32) {
        let (ox, oy) = self.sh.origin.get();
        (ox + x, oy + y)
    }

    pub fn map(&self) {}

    pub fn rect_of(&self, _w: u32) -> Option<[i32; 4]> {
        None
    }

    pub fn take_configured(&mut self) -> bool {
        false
    }

    pub fn set_rect(&self, _r: [i32; 4]) {}

    pub fn monitors(&self) -> Vec<[i32; 4]> {
        let (w, h) = SCREEN.with(Cell::get);
        vec![[0, 0, w, h]]
    }

    pub fn top_owner(&self, _w: u32) -> Option<u32> {
        None
    }

    pub fn set_owner(&self, _owner: Option<u32>) {}

    pub fn set_above(&self, _on: bool) {}

    pub fn set_click_through(&self, _on: bool) {}

    pub fn manager_drag(&self, _size: bool) {}

    pub fn set_grip_cursor(&self, _on: bool) {}

    /// Runs a desktop dialog: here, at once.
    pub fn dialog<R: 'static>(&self, f: impl FnOnce() -> R + 'static) -> Option<R> {
        Some(f())
    }

    pub fn locked(&self) -> bool {
        self.sh.locked.get()
    }

    /// Movement arrives as `Input::Delta` until `unlock` (the cursor itself stays visible in the browser).
    pub fn lock(&mut self) {
        self.sh.locked.set(true);
    }

    pub fn unlock(&mut self) {
        self.sh.locked.set(false);
    }

    /// A menu at main-canvas point (`x`, `y`): the pick when this is the input delivered again after one, else `None`
    /// with the menu left open (see the module's notes).
    pub fn popup<C: Clone>(&mut self, x: i32, y: i32, def: &MenuDef<C>, pal: &Palette, scale: f32) -> Option<C> {
        self.unlock();
        let answer = self.sh.answers.borrow_mut().pop_front();
        if let Some(a @ Answer::Menu(_)) = answer {
            self.sh.taken.borrow_mut().push(a.clone());
            let Answer::Menu(path) = a else { return None };
            return pick(def, &path);
        }
        let trigger = self.sh.trigger.borrow().clone()?;
        let prior = self.sh.taken.borrow().clone();
        let ov = Overlay::menu(self.sh.parent, trigger, prior, Tree::of(def), (x, y), pal.clone(), scale);
        OVERLAY.with(|o| *o.borrow_mut() = Some(ov));
        dirty();
        None
    }

    /// `popup` (no live picture behind it here).
    pub fn popup_live<C: Clone>(&mut self, x: i32, y: i32, def: &MenuDef<C>, pal: &Palette, scale: f32, _live: &mut dyn FnMut(&Window)) -> Option<C> {
        self.popup(x, y, def, pal, scale)
    }

    /// "Type in value" at main-canvas point (`x`, `y`), as `popup`: the text when delivered again, else `None`.
    pub fn type_in(&mut self, x: i32, y: i32, text: &str, scale: f32) -> Option<String> {
        self.unlock();
        let answer = self.sh.answers.borrow_mut().pop_front();
        if let Some(a @ Answer::Text(_)) = answer {
            self.sh.taken.borrow_mut().push(a.clone());
            let Answer::Text(t) = a else { return None };
            return Some(t);
        }
        let trigger = self.sh.trigger.borrow().clone()?;
        let prior = self.sh.taken.borrow().clone();
        let ov = Overlay::type_in(self.sh.parent, trigger, prior, (x, y), text, scale);
        OVERLAY.with(|o| *o.borrow_mut() = Some(ov));
        dirty();
        None
    }
}

/// The command at `path` in `def`, if the menu is still the one shown.
fn pick<C: Clone>(def: &MenuDef<C>, path: &[usize]) -> Option<C> {
    let (&i, rest) = path.split_first()?;
    if rest.is_empty() {
        return def.items.get(i).filter(|it| it.clickable()).and(def.cmds.get(i)).cloned();
    }
    pick(def.sub(i)?, rest)
}

// -------------------------------------------------------------------------------------------------------------------
// Overlays: menus and the type-in box, painted for the host above everything.

/// A menu without its commands (they stay with the editor).
#[derive(Clone)]
struct Tree {
    items: Vec<MenuItem>,
    style: MenuStyle,
    current: Option<usize>,
    subs: Vec<(usize, Tree)>,
}

impl Tree {
    fn of<C: Clone>(d: &MenuDef<C>) -> Tree {
        Tree { items: d.items.clone(), style: d.style, current: d.current, subs: d.subs.iter().map(|(i, s)| (*i, Tree::of(s))).collect() }
    }

    fn sub(&self, i: usize) -> Option<&Tree> {
        self.subs.iter().find(|(j, _)| *j == i).map(|(_, t)| t)
    }
}

/// One open level of a menu: its tree, where it is (main canvas px), its layout and the hovered row.
struct Level {
    tree: Tree,
    /// The row of the level above that opened it.
    from: Option<usize>,
    rect: [i32; 4],
    layout: MenuLayout,
    hover: Option<usize>,
}

enum Kind {
    Menu { levels: Vec<Level>, opened: Instant, start: Option<(i32, i32)>, moved: bool },
    Type { s: Vec<char>, caret: usize, all: bool, rect: [i32; 4] },
}

struct Overlay {
    parent: Parent,
    trigger: Input,
    /// The answers that led to this menu or box (given again first).
    prior: Vec<Answer>,
    kind: Kind,
    pal: Palette,
    scale: f32,
    /// Painted picture: XRGB of the bounding rect.
    px: Vec<u32>,
    bounds: [i32; 4],
}

/// Where the overlay is and its pixels: (x, y, w, h) in the main canvas, XRGB rows.
pub fn overlay<R>(f: impl FnOnce([i32; 4], &[u32]) -> R) -> Option<R> {
    OVERLAY.with(|o| o.borrow().as_ref().map(|ov| f(ov.bounds, &ov.px)))
}

fn inside(r: &[i32; 4], (x, y): (i32, i32)) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

impl Overlay {
    fn menu(parent: Parent, trigger: Input, prior: Vec<Answer>, tree: Tree, at: (i32, i32), pal: Palette, scale: f32) -> Overlay {
        let mut ov = Overlay {
            parent,
            trigger,
            prior,
            kind: Kind::Menu { levels: Vec::new(), opened: Instant::now(), start: None, moved: false },
            pal,
            scale,
            px: Vec::new(),
            bounds: [0, 0, 0, 0],
        };
        ov.open_level(tree, at, None, None);
        ov.paint();
        ov
    }

    fn type_in(parent: Parent, trigger: Input, prior: Vec<Answer>, (x, y): (i32, i32), text: &str, scale: f32) -> Overlay {
        let (w, h) = ((140.0 * scale).round() as i32, (22.0 * scale).round() as i32);
        let (sw, sh) = SCREEN.with(Cell::get);
        let rect = [x.min(sw - w).max(0), y.min(sh - h).max(0), w, h];
        let s: Vec<char> = text.chars().collect();
        let caret = s.len();
        let mut ov = Overlay {
            parent,
            trigger,
            prior,
            kind: Kind::Type { s, caret, all: true, rect },
            pal: crate::palette(),
            scale,
            px: Vec::new(),
            bounds: rect,
        };
        ov.paint();
        ov
    }

    /// Opens `tree` at (x, y), kept on the screen: left of `parent_rect` when it would leave it on the right.
    fn open_level(&mut self, tree: Tree, (x, y): (i32, i32), parent_rect: Option<[i32; 4]>, from: Option<usize>) {
        let Kind::Menu { levels, .. } = &mut self.kind else { return };
        let Ok(mut measure) = Canvas::new(8, 8, self.scale) else { return };
        let layout = kmenu::layout(&mut measure, &tree.items, tree.style);
        let (w, h) = ((layout.w * self.scale).ceil() as i32, (layout.h * self.scale).ceil() as i32);
        let (sw, sh) = SCREEN.with(Cell::get);
        let (mut px, mut py) = (x, y);
        if px + w > sw {
            px = match parent_rect {
                Some(p) => p[0] - w,
                None => sw - w,
            }
            .max(0);
        }
        if py + h > sh {
            py = (sh - h).max(0);
        }
        levels.push(Level { tree, from, rect: [px, py, w.max(1), h.max(1)], layout, hover: None });
    }

    fn paint(&mut self) {
        let scale = self.scale;
        match &self.kind {
            Kind::Menu { levels, .. } => {
                let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
                for l in levels {
                    x0 = x0.min(l.rect[0]);
                    y0 = y0.min(l.rect[1]);
                    x1 = x1.max(l.rect[0] + l.rect[2]);
                    y1 = y1.max(l.rect[1] + l.rect[3]);
                }
                if levels.is_empty() {
                    self.bounds = [0, 0, 0, 0];
                    self.px.clear();
                    return;
                }
                let (bw, bh) = ((x1 - x0).max(1), (y1 - y0).max(1));
                // Transparent between levels: 0 alpha is "not part of the overlay" for the host (`ALPHA`).
                let mut out = vec![0u32; (bw * bh) as usize];
                for l in levels {
                    let Ok(mut c) = Canvas::new(l.rect[2] as u32, l.rect[3] as u32, scale) else { continue };
                    c.begin_frame(None);
                    kmenu::paint(&mut c, &self.pal, &l.tree.items, &l.layout, l.tree.style, 0.0, 0.0, l.hover, l.tree.current);
                    let mut px = vec![0u32; (l.rect[2] * l.rect[3]) as usize];
                    c.copy_to_xrgb(&mut px);
                    for row in 0..l.rect[3] {
                        let dst = ((l.rect[1] - y0 + row) * bw + (l.rect[0] - x0)) as usize;
                        let src = (row * l.rect[2]) as usize;
                        for (d, s) in out[dst..dst + l.rect[2] as usize].iter_mut().zip(&px[src..src + l.rect[2] as usize]) {
                            *d = s | ALPHA;
                        }
                    }
                }
                self.bounds = [x0, y0, bw, bh];
                self.px = out;
            }
            Kind::Type { s, caret, all, rect } => {
                let (w, h) = (rect[2], rect[3]);
                let Ok(mut canvas) = Canvas::new(w as u32, h as u32, scale) else { return };
                let (lw, lh) = (w as f32 / scale, h as f32 / scale);
                let f = Font::new(14);
                canvas.begin_frame(None);
                canvas.clear(Color::rgb(0xffffff));
                for r in [Rect::new(0.0, 0.0, lw, 1.0), Rect::new(0.0, lh - 1.0, lw, 1.0), Rect::new(0.0, 0.0, 1.0, lh), Rect::new(lw - 1.0, 0.0, 1.0, lh)] {
                    canvas.fill_rect(r, Color::rgb(0x646464));
                }
                let text: String = s.iter().collect();
                let line = canvas.line_height(f);
                let ty = ((lh - line) / 2.0).round();
                if *all && !text.is_empty() {
                    let tw = canvas.text_width(f, &text);
                    canvas.fill_rect(Rect::new(4.0, ty, tw, line), Color::rgb(0x0078d7));
                    canvas.text(f, 4.0, ty, Color::rgb(0xffffff), &text);
                } else {
                    canvas.text(f, 4.0, ty, Color::rgb(0x000000), &text);
                    let before: String = s[..*caret].iter().collect();
                    let cx = 4.0 + canvas.text_width(f, &before);
                    canvas.fill_rect(Rect::new(cx, ty, 1.0, line), Color::rgb(0x000000));
                }
                let mut px = vec![0u32; (w * h) as usize];
                canvas.copy_to_xrgb(&mut px);
                for p in &mut px {
                    *p |= ALPHA;
                }
                self.bounds = *rect;
                self.px = px;
            }
        }
    }
}

/// Marks overlay pixels as shown (the host skips pixels without it: the gaps between menu levels).
pub const ALPHA: u32 = 0xff00_0000;

/// What the overlay did with an input.
enum Done {
    /// Still open.
    Open,
    Cancel,
    Menu(Vec<usize>),
    Text(String),
}

/// Input for the open overlay, in main-canvas device px (`Down`, `Up`, `Move`, `Wheel` and `Key` are looked at). False
/// when there is no overlay (the input is the host's or an editor's).
pub fn overlay_input(input: Input) -> bool {
    let Some(mut ov) = OVERLAY.with(|o| o.borrow_mut().take()) else { return false };
    let done = match &mut ov.kind {
        Kind::Menu { .. } => menu_input(&mut ov, &input),
        Kind::Type { .. } => type_input(&mut ov, &input),
    };
    match done {
        Done::Open => {
            ov.paint();
            OVERLAY.with(|o| *o.borrow_mut() = Some(ov));
        }
        Done::Cancel => {}
        Done::Menu(path) => answer(ov.parent, ov.trigger, ov.prior, Answer::Menu(path)),
        Done::Text(t) => answer(ov.parent, ov.trigger, ov.prior, Answer::Text(t)),
    }
    dirty();
    true
}

/// Delivers the input that opened a menu or box again, with the answer waiting for it.
fn answer(parent: Parent, trigger: Input, prior: Vec<Answer>, a: Answer) {
    let Some(s) = shared(parent) else { return };
    *s.answers.borrow_mut() = prior.into_iter().chain([a]).collect();
    s.taken.borrow_mut().clear();
    *s.trigger.borrow_mut() = Some(trigger.clone());
    run(parent, trigger);
    // Not asked for again (the editor took another way): they are dropped.
    s.answers.borrow_mut().clear();
    s.taken.borrow_mut().clear();
}

/// The level and row under a point, deepest level first.
fn row_at(levels: &[Level], scale: f32, p: (i32, i32)) -> Option<(usize, Option<usize>)> {
    levels.iter().enumerate().rev().find(|(_, l)| inside(&l.rect, p)).map(|(n, l)| {
        let row = l.layout.item_at(&l.tree.items, (p.0 - l.rect[0]) as f32 / scale, (p.1 - l.rect[1]) as f32 / scale);
        (n, row.filter(|&i| l.tree.items[i].clickable()))
    })
}

fn path_to(levels: &[Level], n: usize, row: usize) -> Vec<usize> {
    let mut path: Vec<usize> = levels[1..=n].iter().filter_map(|l| l.from).collect();
    path.push(row);
    path
}

fn menu_input(ov: &mut Overlay, input: &Input) -> Done {
    let scale = ov.scale;
    let Kind::Menu { levels, opened, start, moved } = &mut ov.kind else { return Done::Cancel };
    match *input {
        Input::Move { x, y } => {
            let p = (x as i32, y as i32);
            match start {
                None => *start = Some(p),
                Some(s) if (p.0 - s.0).abs() + (p.1 - s.1).abs() > 2 => *moved = true,
                _ => {}
            }
            let Some((n, row)) = row_at(levels, scale, p) else {
                for l in levels.iter_mut() {
                    l.hover = None;
                }
                return Done::Open;
            };
            // Back on a level above: the levels below it close unless the row is the one that opened them.
            if n + 1 < levels.len() && row.is_some() && levels[n + 1].from != row {
                levels.truncate(n + 1);
            }
            levels[n].hover = row;
            if let Some(i) = row {
                if n + 1 == levels.len() {
                    if let Some(sub) = levels[n].tree.sub(i).cloned() {
                        let l = &levels[n];
                        let r = l.layout.rows[i];
                        let at = (l.rect[0] + ((r.x + r.w) * scale) as i32 - 2, l.rect[1] + (r.y * scale) as i32 - 2);
                        let pr = l.rect;
                        ov.open_level(sub, at, Some(pr), Some(i));
                    }
                }
            }
            Done::Open
        }
        Input::Down { x, y, .. } => {
            if row_at(levels, scale, (x as i32, y as i32)).is_none() {
                return Done::Cancel;
            }
            *moved = true;
            Done::Open
        }
        Input::Up { btn } if btn != Button::Middle => {
            // The release of the click that opened the menu does not pick.
            if !*moved && opened.elapsed() < Duration::from_millis(400) {
                return Done::Open;
            }
            match release_pick(levels) {
                Some(path) => Done::Menu(path),
                None => Done::Open,
            }
        }
        Input::Key { sym, .. } => match sym {
            keysym::ESCAPE => Done::Cancel,
            keysym::DOWN | keysym::UP => {
                let Some(l) = levels.last_mut() else { return Done::Cancel };
                let n = l.tree.items.len();
                if n == 0 {
                    return Done::Open;
                }
                let down = sym == keysym::DOWN;
                let mut i = l.hover.or(l.tree.current).unwrap_or(if down { n - 1 } else { 0 });
                for _ in 0..n {
                    i = if down { (i + 1) % n } else { (i + n - 1) % n };
                    if l.tree.items[i].clickable() {
                        break;
                    }
                }
                l.hover = Some(i);
                Done::Open
            }
            keysym::RETURN | keysym::KP_ENTER => {
                let n = levels.len() - 1;
                match levels[n].hover.filter(|&i| levels[n].tree.items[i].clickable() && levels[n].tree.sub(i).is_none()) {
                    Some(i) => Done::Menu(path_to(levels, n, i)),
                    None => Done::Open,
                }
            }
            _ => Done::Open,
        },
        _ => Done::Open,
    }
}

/// The release that picks needs the pointer's place: the host sends a `Move` before every `Up`, so the hovered row is
/// the one under it.
fn release_pick(levels: &[Level]) -> Option<Vec<usize>> {
    let n = levels.iter().rposition(|l| l.hover.is_some())?;
    let i = levels[n].hover?;
    (levels[n].tree.items[i].clickable() && levels[n].tree.sub(i).is_none()).then(|| path_to(levels, n, i))
}

fn type_input(ov: &mut Overlay, input: &Input) -> Done {
    let Kind::Type { s, caret, all, rect } = &mut ov.kind else { return Done::Cancel };
    match *input {
        Input::Down { x, y, .. } => {
            if !inside(rect, (x as i32, y as i32)) {
                return Done::Cancel;
            }
            *all = false;
            *caret = s.len();
            Done::Open
        }
        Input::Key { sym, .. } => {
            let ctrl = MODS.with(Cell::get).0;
            match sym {
                keysym::RETURN | keysym::KP_ENTER => return Done::Text(s.iter().collect()),
                keysym::ESCAPE => return Done::Cancel,
                keysym::BACKSPACE | keysym::DELETE if *all => {
                    s.clear();
                    *caret = 0;
                    *all = false;
                }
                keysym::BACKSPACE if *caret > 0 => {
                    *caret -= 1;
                    s.remove(*caret);
                }
                keysym::DELETE if *caret < s.len() => {
                    s.remove(*caret);
                }
                keysym::LEFT => {
                    *caret = if *all { 0 } else { caret.saturating_sub(1) };
                    *all = false;
                }
                keysym::RIGHT => {
                    *caret = if *all { s.len() } else { (*caret + 1).min(s.len()) };
                    *all = false;
                }
                keysym::HOME => (*caret, *all) = (0, false),
                keysym::END => (*caret, *all) = (s.len(), false),
                _ if ctrl && keysym_char(sym).is_some_and(|c| c.eq_ignore_ascii_case(&'a')) => *all = true,
                _ if !ctrl => {
                    if let Some(ch) = keysym_char(sym) {
                        if *all {
                            s.clear();
                            *caret = 0;
                            *all = false;
                        }
                        s.insert(*caret, ch);
                        *caret += 1;
                    }
                }
                _ => {}
            }
            Done::Open
        }
        _ => Done::Open,
    }
}

/// Whether a menu or box is open.
pub fn overlay_open() -> bool {
    OVERLAY.with(|o| o.borrow().is_some())
}
