// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! The web build's way between the page and the app: what the page asks (a file dropped or imported, open, save,
//! export) waits in a queue the main window's loop takes in (`run`), and what the app hands back (exported files to
//! download) waits for the page (`lemon_take_downloads`). The files themselves are in `os_web::fs`.

use std::cell::RefCell;
use std::path::PathBuf;

use wasm_bindgen::prelude::*;

use crate::app::App;

enum Cmd {
    /// A file dropped at (x, y) in CSS pixels of the canvas.
    Drop(PathBuf, f32, f32),
    /// An audio file imported with the page's button: a new audio clip on the first free arranger track.
    Import(PathBuf),
    Open(PathBuf),
    Save,
    Export,
    /// MetroBar's day / night switch: the MetroBar Night theme (true) or the MetroBar (day) theme.
    Night(bool),
    /// Browser tests (`web/test`): "gen:<name>" adds an instrument channel, "fx:<name>" an effect on mixer track 1.
    Test(String),
}

thread_local! {
    /// The plugin lists for `lemon_test_plugins`.
    static APP_PLUGINS: RefCell<String> = const { RefCell::new(String::new()) };
    static QUEUE: RefCell<Vec<Cmd>> = const { RefCell::new(Vec::new()) };
    static DOWNLOADS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    static STATE: RefCell<String> = const { RefCell::new(String::new()) };
}

fn push(c: Cmd) {
    QUEUE.with(|q| q.borrow_mut().push(c));
    crate::wake_ui();
}

/// Browser tests: see `Cmd::Test`.
#[wasm_bindgen]
pub fn lemon_test(cmd: &str) {
    push(Cmd::Test(cmd.into()));
}

/// Browser tests: the built-in instruments and effects, as `{"gen":[...],"fx":[...]}`.
#[wasm_bindgen]
pub fn lemon_test_plugins() -> String {
    APP_PLUGINS.with(|p| p.borrow().clone())
}

#[wasm_bindgen]
pub fn lemon_drop(path: &str, x: f32, y: f32) {
    push(Cmd::Drop(path.into(), x, y));
}

#[wasm_bindgen]
pub fn lemon_import(path: &str) {
    push(Cmd::Import(path.into()));
}

#[wasm_bindgen]
pub fn lemon_open(path: &str) {
    push(Cmd::Open(path.into()));
}

#[wasm_bindgen]
pub fn lemon_save() {
    push(Cmd::Save);
}

#[wasm_bindgen]
pub fn lemon_export() {
    push(Cmd::Export);
}

/// The page's day / night switch was flipped: the app takes the MetroBar Night (true) or MetroBar theme, live.
#[wasm_bindgen]
pub fn lemon_set_night(night: bool) {
    push(Cmd::Night(night));
}

#[wasm_bindgen]
extern "C" {
    /// The look the page's day / night switch is on (`metrobar.bookDay`, shared with the MetroBar app): true = night.
    #[wasm_bindgen(js_namespace = lemonWeb, js_name = night)]
    fn page_night() -> bool;
}

/// The look the page asks for at start (`app::wanted_night`).
pub(crate) fn night() -> bool {
    page_night()
}

/// Files the app wrote for the user to keep (an export): one path a line, each once.
#[wasm_bindgen]
pub fn lemon_take_downloads() -> String {
    DOWNLOADS.with(|d| d.borrow_mut().drain(..).map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join("\n"))
}

/// The app as the page shows it (JSON): the project's file, unsaved changes, the last message, the arranger's clips
/// and the channels' samples.
#[wasm_bindgen]
pub fn lemon_state() -> String {
    STATE.with(|s| s.borrow().clone())
}

/// An export wrote `files`: the page offers them as downloads.
pub(crate) fn exported(files: &[PathBuf]) {
    DOWNLOADS.with(|d| d.borrow_mut().extend(files.iter().cloned()));
}

fn esc(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            c if (c as u32) < 0x20 => vec![' '],
            c => vec![c],
        })
        .collect()
}

/// Takes in what the page asked since the last frame; `scale` turns CSS pixels into the app's units. True when
/// something was done (the window paints again).
pub(crate) fn run(app: &mut App, scale: f32) -> bool {
    let cmds = QUEUE.with(|q| std::mem::take(&mut *q.borrow_mut()));
    let any = !cmds.is_empty();
    for c in cmds {
        match c {
            Cmd::Drop(path, x, y) => {
                app.mouse_move(x * scale, y * scale);
                app.drop_file(path);
            }
            Cmd::Import(path) => import(app, path),
            Cmd::Open(path) => app.drop_file(path),
            Cmd::Save => save(app),
            Cmd::Export => app.open_export(false),
            Cmd::Night(night) => app.set_metrobar_night(night),
            Cmd::Test(t) => {
                let msg = if let Some(name) = t.strip_prefix("gen:") {
                    app.plugins.index_of(name).and_then(|i| crate::plugin_menu::add_generator(app, i, None))
                } else if let Some(rest) = t.strip_prefix("fx:") {
                    // "fx:<track>:<name>", or "fx:<name>" for track 1.
                    let (track, name) = rest.split_once(':').and_then(|(n, name)| Some((n.parse::<u16>().ok()?, name))).unwrap_or((1, rest));
                    app.state.selected_mixer = track;
                    app.plugins.effect_index(name).and_then(|i| crate::plugin_menu::add_effect(app, i, None))
                } else {
                    None
                };
                if let Some(m) = msg {
                    web_sys::console::log_1(&format!("lemon_test {t}: {m}").into());
                }
            }
        }
        app.invalidate();
    }
    publish(app);
    if APP_PLUGINS.with(|p| !p.borrow().is_empty()) && !any {
        return any;
    }
    let list = |v: &[crate::plugins::Available]| v.iter().map(|a| format!("\"{}\"", esc(&a.desc.name))).collect::<Vec<_>>().join(",");
    let json = format!("{{\"gen\":[{}],\"fx\":[{}]}}", list(&app.plugins.available), list(&app.plugins.effects));
    APP_PLUGINS.with(|p| *p.borrow_mut() = json);
    any
}

fn import(app: &mut App, path: PathBuf) {
    let s = &mut app.state;
    let track = crate::playlist::clips(s).iter().map(|c| c.track + 1).max().unwrap_or(0);
    let name = path.to_string_lossy().into_owned();
    s.message = Some(match crate::playlist::add_audio_clip(s, &name, track, 0) {
        Some(_) => format!("Added audio clip {}", path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()),
        None => format!("Could not read {} as audio", path.display()),
    });
}

/// Saves to the project's file, else to `/user/Projects/<name>.lemon`; the page then offers it as a download.
fn save(app: &mut App) {
    let path = app.state.path.clone().filter(|p| !crate::project_files::is_flp(p)).unwrap_or_else(|| {
        let n = app.state.path.as_ref().and_then(|p| p.file_stem()).map_or_else(|| "Untitled".to_string(), |n| n.to_string_lossy().into_owned());
        PathBuf::from(format!("/user/Projects/{n}.{}", crate::project_files::EXT))
    });
    app.dialog_done(crate::dialog::Then::SaveProject(None), crate::dialog::Reply::File(Some(path.clone())));
    if os_web::fs::exists(&path) {
        exported(&[path]);
    }
}

fn publish(app: &App) {
    let s = &app.state;
    // The MetroBar theme's look, for the page's switch and colours (a theme picked in Settings counts too).
    let th = s.fl_theme();
    let look = if !th.metrobar { "other" } else if th.metrobar_night { "night" } else { "day" };
    let clips: Vec<String> = crate::playlist::clips(s).iter().map(|c| format!("{{\"track\":{},\"position\":{},\"length\":{}}}", c.track, c.position, c.length)).collect();
    let samples: Vec<String> = s.project.channels.iter().filter_map(|c| c.sample_path.as_deref()).map(|p| format!("\"{}\"", esc(p))).collect();
    let json = format!(
        "{{\"path\":\"{}\",\"unsaved\":{},\"message\":\"{}\",\"clips\":[{}],\"samples\":[{}],\"channels\":{},\"exporting\":{},\"look\":\"{}\"}}",
        esc(&s.path.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()),
        app.unsaved(),
        esc(s.message.as_deref().unwrap_or("")),
        clips.join(","),
        samples.join(","),
        s.project.channels.len(),
        app.export.is_some(),
        look,
    );
    STATE.with(|st| *st.borrow_mut() = json);
}
