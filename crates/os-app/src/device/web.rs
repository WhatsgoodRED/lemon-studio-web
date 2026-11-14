// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! The web build's output device: the page's audio pump (`web/static/lemon-web.js`) asks for blocks of the mix
//! (`lemon_audio_render`) on the page's main thread and posts them to an AudioWorklet, which plays them from a ring of
//! a few blocks. There are no threads on the web, so the transport's commands (`transport::web::pump`) and the
//! stream's feeder (`Renderer::inline`) run in the same call, before the mix.
//!
//! The microphone comes the other way: the page's input worklet posts what it records, and `lemon_audio_input` writes
//! it into the device's input ring, where the mixer tracks that hear the input read it (`input_reader`).

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use wasm_bindgen::prelude::*;

use super::{input_ring, queues, DeviceInfo, Mix, Open, Stream};

/// `DeviceInfo::host` of the browser's output.
pub const HOST: &str = "Web Audio";
/// Frames per block the page asks for.
pub const BLOCK: u32 = 512;

thread_local! {
    /// The open stream's mix (`None`: closed).
    static MIX: RefCell<Option<Mix>> = const { RefCell::new(None) };
    /// The page's AudioContext rate, set before the first open (`lemon_audio_set_rate`).
    static RATE: Cell<u32> = const { Cell::new(48_000) };
    /// The page has a microphone stream (`lemon_audio_mic`), and its channels.
    static MIC: Cell<usize> = const { Cell::new(0) };
    /// Whether the mixer wants the input open (`SystemInput`); the page starts and stops the microphone with it.
    static MIC_WANTED: Cell<bool> = const { Cell::new(false) };
}

/// The page's output while it is open: dropped, the mix goes.
pub(crate) struct WebOut;

impl Drop for WebOut {
    fn drop(&mut self) {
        MIX.with(|m| {
            if let Ok(mut m) = m.try_borrow_mut() {
                *m = None;
            }
        });
    }
}

pub(crate) fn rate() -> u32 {
    RATE.with(Cell::get)
}

pub(super) fn open(xruns: &Arc<AtomicU64>) -> Result<Open, String> {
    let (inbox, outbox, errors) = queues(xruns);
    MIX.with(|m| *m.borrow_mut() = Some(Mix::new(inbox.clone(), outbox.clone())));
    let rate = rate();
    let info = DeviceInfo {
        host: HOST,
        name: "Browser audio".into(),
        rate,
        fixed: Some(BLOCK),
        buffers: None,
        channels: 2,
        inputs: 0,
        out_names: Vec::new(),
        in_names: Vec::new(),
        input_latency: None,
        rates: vec![rate],
        sizes: vec![BLOCK],
        latency: None,
        instead: None,
    };
    Ok(Open { _stream: Stream::Web(WebOut), inbox, outbox, info, note: None, failed: errors.failed, fault: errors.fault, input: None, input_failed: false })
}

/// The microphone the mixer can hear (`system_input::info`), once the page has one.
pub(crate) fn mic_channels() -> usize {
    MIC.with(Cell::get)
}

/// The mixer opens (`true`) or closes the input.
pub(crate) fn want_mic(on: bool, block: usize) {
    MIC_WANTED.with(|w| w.set(on));
    if on {
        input_ring().open(mic_channels().max(1), block);
    } else {
        input_ring().open(0, 0);
    }
}

/// The page's AudioContext runs at `rate`: the next open plays at it.
#[wasm_bindgen]
pub fn lemon_audio_set_rate(rate: u32) {
    if rate > 0 {
        RATE.with(|r| r.set(rate));
    }
}

/// The page has a microphone stream of `channels` channels (0: none).
#[wasm_bindgen]
pub fn lemon_audio_mic(channels: usize) {
    MIC.with(|m| m.set(channels.min(super::INPUT_CHANNELS)));
    super::system_input::forget();
}

/// Whether the mixer has the input open (the page records only then).
#[wasm_bindgen]
pub fn lemon_audio_mic_wanted() -> bool {
    MIC_WANTED.with(Cell::get)
}

/// What the microphone recorded: `channels` interleaved.
#[wasm_bindgen]
pub fn lemon_audio_input(samples: &[f32], channels: usize) {
    let ch = channels.max(1);
    let frames = samples.len() / ch;
    input_ring().push(frames, |c, f| samples[f * ch + c.min(ch - 1)]);
    // One-click recording takes it too while it records.
    crate::capture::capture().push(frames, |c, f| samples[f * ch + c.min(ch - 1)]);
}

/// Counts blocks rendered (the page's pump reads it to tell a live stream).
static RENDERED: AtomicU64 = AtomicU64::new(0);

/// The next `frames` frames of the mix (interleaved stereo, clipped), after the transport's commands. Silence while
/// the device is closed.
#[wasm_bindgen]
pub fn lemon_audio_render(frames: usize) -> Vec<f32> {
    crate::transport::web::pump();
    os_web::tick();
    RENDERED.fetch_add(1, Ordering::Relaxed);
    MIX.with(|m| {
        let Ok(mut m) = m.try_borrow_mut() else { return vec![0.0; 2 * frames] };
        let Some(mix) = m.as_mut() else { return vec![0.0; 2 * frames] };
        mix.block(frames).iter().map(|v| v.clamp(-1.0, 1.0)).collect()
    })
}

/// Whether the device is open (a stream plays on it).
#[wasm_bindgen]
pub fn lemon_audio_open() -> bool {
    MIX.with(|m| m.try_borrow().map(|m| m.is_some()).unwrap_or(true))
}
