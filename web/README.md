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
`node web/test/proof.js` (Playwright; writes screenshots and `results.json`). `node web/test/metrobar.js <url of studio-lemon/>` checks the "← MetroBar" link (to `../landing-beat-meter.html`, the leave prompt only with unsaved changes) and that a browser holding the first, dark MetroBar theme opens with the light one. `node web/test/daynight.js <url> [Highlight Zone url]` checks the day / night switch.

## Phones and tablets

- **Fit:** the canvas fills the page above the bar. Where it is smaller than the layouts' 1024 x 600 CSS px, `ui_scale`
  (os-app `main.rs`) draws the UI smaller until it fits (an iPhone 14 held sideways lays out 1422 x 600 at 0.59), so
  everything is on screen and pointer positions need no conversion. Plugin windows open at the same scale.
- **Touch** (`fingers.rs`): winit hands finger input over as `WindowEvent::Touch`; `Shell::touch_out` turns it into the
  mouse events the app and plugin windows take. A tap is a click (twice = double click), moving a finger more than
  9 CSS px presses where it went down and drags (knobs, sliders, clips), holding it still for 0.5 s is a right click
  (menus), two fingers scroll, spreading or pinching them is Ctrl + wheel (zoom).
- **Page:** the bottom bar scrolls sideways and is taller on touch screens; a phone held upright shows "Turn your phone
  sideways" (`#turn`, with a way to go on anyway). Any tap resumes an audio context the system stopped (Safari only
  allows it inside a gesture), the Start audio button comes back while it is stopped, and iOS plays through the
  ringer switch (`navigator.audioSession.type = 'playback'`, Safari 17+).
- **Check:** `node web/test/mobile.js <url> <out dir>` (iPhone 14 sideways 844 x 390 and an Android tablet 1280 x 800,
  CDP touch events: Start audio by tap, rack, channel menu by long press, Lemon Sampler, knob drag, play / stop, two-finger scroll, the
  bar, portrait). Headless Chromium reports the canvas's device-pixel size at 1x whatever device scale is emulated, so
  it runs at 1x; on a real phone the canvas is full resolution.

## Day and night

Day (the built-in MetroBar theme, light) is the default; night is the built-in MetroBar Night theme (the first, dark
MetroBar palette). The switch at the right of the bottom bar, before the credits, is MetroBar's own sun / moon switch
(the Highlight Zone's `.smt`, CSS copied from `landing-beat-meter.html`). It flips the app's theme live
(`lemon_set_night`) and the page's colours (`html.night`), and keeps the choice as `metrobar.bookDay` in localStorage
(true or missing = day), the key the MetroBar app uses, so both share it. At start the app asks the page
(`lemonWeb.night`): a kept MetroBar theme of the other look, or of the first dark look, is replaced by the built-in one
the switch is on (`app::metrobar_current`); other themes are left alone. Picking MetroBar or MetroBar Night in
Settings ▸ Theme moves the switch too (`look` in `lemon_state`).

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

## Plugin windows

Every built-in instrument and effect opens its window inside the canvas (`plugin_window::web` in os-app, the editors in
`os_plugkit::web`). An editor cannot block in a loop of its own on the page's one thread, so each editor's loop body is
its `os_plugkit::Step` and `os_plugkit::drive` either loops (native) or keeps it for the host to hand it each input
(web). Popup menus and "Type in value" boxes are overlays: `popup` / `type_in` return `None` at once, and when the user
picks, the input that opened them is delivered again with the answers waiting (a menu row that opens a box gets the row,
then the text). Not on the web: plugins' own file dialogs (load a sample by dropping it on the channel instead), Lemon
Drop's floating window, keys typed into an editor (they stay the app's). `node web/test/plugins.js <out> <url>` opens
all of them (`lemon_test`, `lemon_test_plugins`).

## Lemon Tune (pitch correction)

Lemon Studio's own pitch correction for vocals (`plugins/lemon-small-fx/src/tune.rs`, module crate `plugins/lemon-tune`,
in the mixer's effect list as "Lemon Tune"): Key, Scale (Chromatic, Major, Minor, Harmonic minor, Major/Minor
pentatonic, Blues), Retune speed (Hard = 0 ms, default 20 ms) and Amount. YIN pitch detection on a low-passed copy at
about 12 kHz every 5 ms, TD-PSOLA resynthesis; 36 ms latency at 48 kHz, reported to the host (`Sound::latency`), so
playback and export stay in time. `cargo test --release tune` in plugins/lemon-small-fx; the ignored `real_vocal` test
tunes a raw f32 stereo file (`LEMON_TUNE_IN`, `LEMON_TUNE_SET`, `LEMON_TUNE_OUT`) and prints how far its notes sit from
the nearest semitone before and after.

## Not done yet

- Time stretching and pitch shifting at quality: build Signalsmith Stretch with a wasm libc++ (wasi-sdk) or port it.
- MP3 / OGG export: LAME and libvorbis need a C toolchain for wasm (wasi-sdk or emscripten).
- Audio runs on the page's main thread: heavy UI work can starve it (the worklet keeps about 43 ms). Moving the
  engine into the worklet or a worker needs wasm threads (nightly `build-std` with atomics, SharedArrayBuffer and
  COOP/COEP headers).
- MIDI devices (Web MIDI), the clipboard, file associations, the updater and the control socket are not wired.
