# Lemon Studio in the browser (`web` branch)

Lemon Studio built for `wasm32-unknown-unknown` and run in a web page: the same app (UI, engine, the built-in
plugins' sound), drawn into a `<canvas>` by winit's web backend and softbuffer, with the browser's audio, microphone
and files in place of the operating system's.

## Build

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129   # the version in Cargo.lock
brew install binaryen                              # optional: wasm-opt, about 30 % smaller
web/build.sh [out-dir]                             # default ../studio-lemon; PROFILE=dev for a fast build
```

The output is plain static files (`index.html`, `lemon-web.js`, `lemon-worklets.js`, `pkg/`, the licence files and an
`.htaccess` for Apache). No COOP/COEP headers are needed. Check it with `python3 web/test/serve.py <out-dir> 8765` and
`node web/test/proof.js` (Playwright; writes screenshots and `results.json`). `node web/test/metrobar.js <url of studio-lemon/>` checks the "← MetroBar" link (to `../landing-beat-meter.html`, the leave prompt only with unsaved changes) and that a browser holding the first, dark MetroBar theme opens with the light one.

## How it works

| Native | Web build |
|---|---|
| Audio thread + feeder thread, cpal callback | `transport::web::pump` and the feeder run inline when the page's pump calls `lemon_audio_render`; the blocks go to an AudioWorklet (`lemon-out`) over its port |
| Input device (cpal) | getUserMedia → AudioWorklet `lemon-in` → `lemon_audio_input` → the device's input ring and the capture ring |
| Threads (`std::thread`) | `os_web::spawn` runs the job at once; loops become `os_web::every` tickers |
| `std::fs` | `os_web::fs`: in-memory, kept in IndexedDB by the page (`/user`) and put back before the app starts |
| `Instant`, `SystemTime::now` | `web-time` (vendored, with headroom below `now()`) |
| Native plugins (CLAP, VST3, VST2) | not available: `libloading` (vendored) never opens a library |
| nih-plug's event loop | vendored nih-plug: tasks run where they are scheduled |
| Signalsmith Stretch (C++) | `os_dsp::web_stretch`: linear resampling (no pitch-independent stretch yet) |
| LAME, libvorbis (C) | MP3 and OGG export say they are not available |

Everything web-only is behind `cfg(target_arch = "wasm32")`; native builds are unchanged (`os_web::fs` re-exports
`std::fs`, `VPath::vexists` is `Path::exists`, and so on).

## Not done yet

- Plugin editors: the built-in plugins' editors draw with os-ui but run in windows of their own, each on its own
  thread with a blocking event loop (`os_plugkit::gui`). On the web a plugin's window does not open, so its sound
  works with its saved or default settings, but it cannot be edited. Needed: an `os_plugkit::web` shell (a floating
  panel in the main canvas or a second canvas), each plugin's `gui::Ed::run` loop turned into a per-event handler,
  and the modal popups and type-in boxes made non-blocking.
- Time stretching and pitch shifting at quality: build Signalsmith Stretch with a wasm libc++ (wasi-sdk) or port it.
- MP3 / OGG export: LAME and libvorbis need a C toolchain for wasm (wasi-sdk or emscripten).
- Audio runs on the page's main thread: heavy UI work can starve it (the worklet keeps about 43 ms). Moving the
  engine into the worklet or a worker needs wasm threads (nightly `build-std` with atomics, SharedArrayBuffer and
  COOP/COEP headers).
- MIDI devices (Web MIDI), the clipboard, file associations, the updater and the control socket are not wired.
