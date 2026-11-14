// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Web build (wasm32) stand-in for the `signalsmith-stretch` binding, whose C++ does not build for
//! wasm32-unknown-unknown without a wasm libc++. It keeps the binding's API so `stretch` is unchanged, and resamples
//! linearly: lengths come out right, but pitch follows speed (no formant or pitch-independent stretch yet).
// TODO(web): build Signalsmith Stretch with wasi-sdk's libc++, or port it to Rust, and drop this file.

pub struct Stretch {
    channels: usize,
    transpose: f32,
}

impl Stretch {
    pub fn new(channel_count: u32, _block_length: usize, _interval: usize) -> Self {
        Stretch { channels: channel_count.max(1) as usize, transpose: 1.0 }
    }
    pub fn preset_default(channel_count: u32, _sample_rate: u32) -> Self {
        Self::new(channel_count, 0, 0)
    }
    pub fn preset_cheaper(channel_count: u32, _sample_rate: u32) -> Self {
        Self::new(channel_count, 0, 0)
    }
    pub fn reset(&mut self) {}
    pub fn input_latency(&self) -> usize {
        0
    }
    pub fn output_latency(&self) -> usize {
        0
    }
    pub fn set_transpose_factor(&mut self, multiplier: f32, _tonality_limit: Option<f32>) {
        self.transpose = multiplier;
    }
    pub fn set_transpose_factor_semitones(&mut self, semitones: f32, _tonality_limit: Option<f32>) {
        self.transpose = 2f32.powf(semitones / 12.0);
    }
    pub fn set_formant_factor(&mut self, _multiplier: f32, _compensate_pitch: bool) {}
    pub fn set_formant_factor_semitones(&mut self, _semitones: f32, _compensate_pitch: bool) {}
    pub fn signalsmith_stretch_set_formant_base(&self, _frequency: f32) {}
    pub fn seek(&mut self, _input: impl AsRef<[f32]>, _playback_rate: f64) {}
    /// Interleaved `input` resampled linearly onto the whole of interleaved `output`.
    pub fn process(&mut self, input: impl AsRef<[f32]>, mut output: impl AsMut<[f32]>) {
        let (input, output) = (input.as_ref(), output.as_mut());
        let ch = self.channels;
        let (n_in, n_out) = (input.len() / ch, output.len() / ch);
        if n_out == 0 {
            return;
        }
        if n_in == 0 {
            output.fill(0.0);
            return;
        }
        let step = n_in as f64 / n_out as f64;
        for i in 0..n_out {
            let pos = i as f64 * step;
            let a = (pos as usize).min(n_in - 1);
            let b = (a + 1).min(n_in - 1);
            let t = (pos - a as f64) as f32;
            for c in 0..ch {
                output[i * ch + c] = input[a * ch + c] * (1.0 - t) + input[b * ch + c] * t;
            }
        }
    }
    pub fn exact(&mut self, input: impl AsRef<[f32]>, output: impl AsMut<[f32]>) -> bool {
        self.process(input, output);
        true
    }
    pub fn flush(&mut self, mut output: impl AsMut<[f32]>) {
        output.as_mut().fill(0.0);
    }
}
