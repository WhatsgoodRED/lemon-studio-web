// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! What the web build (wasm32) has in place of the operating system: no threads and no file system there.
//!
//! - [`spawn`]: a job for a thread of its own; on the web it runs at once, on the calling thread.
//! - [`every`]: a loop a thread would run (drain a queue every few ms); on the web the page calls [`tick`] for it.
//! - [`fs`]: the file system calls the app makes; on native exactly `std::fs`, on the web an in-memory file system
//!   the page fills (imported samples) and reads (exports, recordings, saved projects).

pub mod fs;

/// `std::env::temp_dir`, which panics on the web: there, `/tmp` of the in-memory file system.
pub fn temp_dir() -> std::path::PathBuf {
    #[cfg(target_arch = "wasm32")]
    return std::path::PathBuf::from("/tmp");
    #[cfg(not(target_arch = "wasm32"))]
    std::env::temp_dir()
}

/// `std::process::id`, which panics on the web: there, 1.
pub fn pid() -> u32 {
    #[cfg(target_arch = "wasm32")]
    return 1;
    #[cfg(not(target_arch = "wasm32"))]
    std::process::id()
}

/// Runs `job` on a thread named `name`, or at once on this one when there are no threads (the web).
pub fn spawn<F: FnOnce() + Send + 'static>(name: &str, job: F) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        job();
    }
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::Builder::new().name(name.into()).spawn(job).expect("spawn thread");
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static TICKERS: std::cell::RefCell<Vec<Box<dyn FnMut() -> bool>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The web build: `step` runs at every `tick` until it returns false. Native builds never call it (they run a thread).
#[cfg(target_arch = "wasm32")]
pub fn every(step: impl FnMut() -> bool + 'static) {
    TICKERS.with(|t| t.borrow_mut().push(Box::new(step)));
}

/// Runs the steps `every` registered (the page calls it about every 10 ms).
#[cfg(target_arch = "wasm32")]
pub fn tick() {
    let mut steps = TICKERS.with(|t| std::mem::take(&mut *t.borrow_mut()));
    steps.retain_mut(|s| s());
    TICKERS.with(|t| {
        let mut t = t.borrow_mut();
        let added = std::mem::take(&mut *t);
        *t = steps;
        t.extend(added);
    });
}
