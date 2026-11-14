// Lemon Studio web build: the page side. GPL-3.0-or-later.
//
// Loads the wasm build of Lemon Studio (it draws into #lemon-canvas through winit and softbuffer), and runs the
// audio: the browser has no audio thread for the engine, so this pump asks the engine for blocks on the main thread
// (`lemon_audio_render`) and keeps an AudioWorklet ("lemon-out") a little ahead. No SharedArrayBuffer is used, so
// the page needs no COOP/COEP headers.

// TODO(owner): the public URL of this build's source (GPL-3.0 §6). The repository is private for now.
const SOURCE_URL = 'TODO-public-source-repository-url';

import init, * as lemon from './pkg/lemonstudio.js';

const $ = (id) => document.getElementById(id);
const status = (t) => { $('status').textContent = t; };
$('source-link').href = SOURCE_URL.startsWith('TODO') ? 'https://github.com/ItsAlphaNeon/Lemon-Studio' : SOURCE_URL;

const BLOCK = 512;          // frames per render call (`device::web::BLOCK`)
const AHEAD = 4 * BLOCK;    // frames kept queued in the worklet (about 43 ms at 48 kHz)

const web = window.lemonWeb = {
  ctx: null, out: null, mic: null, analyser: null,
  produced: 0, read: 0, peak: 0, maxPeak: 0, under: 0, played: 0, renders: 0, micFrames: 0,
  ready: false, error: null,
};

async function startAudio() {
  if (web.ctx) return;
  const ctx = new AudioContext({ latencyHint: 'interactive' });
  web.ctx = ctx;
  lemon.lemon_audio_set_rate(ctx.sampleRate);
  await ctx.audioWorklet.addModule('lemon-worklets.js');
  const out = new AudioWorkletNode(ctx, 'lemon-out', { numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [2] });
  out.port.onmessage = (e) => {
    const m = e.data;
    web.read = m.read; web.played = m.played; web.under = m.under;
    web.peak = m.peak; if (m.peak > web.maxPeak) web.maxPeak = m.peak;
  };
  const analyser = ctx.createAnalyser();
  analyser.fftSize = 2048;
  out.connect(analyser);
  analyser.connect(ctx.destination);
  web.out = out; web.analyser = analyser;
  setInterval(pump, 5);
  if (ctx.state !== 'running') {
    $('start').style.display = 'flex';
  }
}

function pump() {
  if (!web.out) return;
  let n = 0;
  while (web.produced - web.read < AHEAD && n < 16) {
    const buf = lemon.lemon_audio_render(BLOCK);
    web.out.port.postMessage(buf, [buf.buffer]);
    web.produced += BLOCK;
    web.renders++;
    n++;
  }
}

async function startMic() {
  if (web.mic) return true;
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false } });
    const src = web.ctx.createMediaStreamSource(stream);
    const node = new AudioWorkletNode(web.ctx, 'lemon-in', { numberOfInputs: 1, numberOfOutputs: 0 });
    node.port.onmessage = (e) => { lemon.lemon_audio_input(e.data, 1); web.micFrames += e.data.length; };
    src.connect(node);
    web.mic = { stream, src, node };
    lemon.lemon_audio_mic(1);
    status('Microphone on: pick it as a mixer track input to record');
    return true;
  } catch (e) {
    status('Microphone not available: ' + e.message);
    return false;
  }
}
web.startMic = startMic;

$('start-btn').onclick = async () => {
  await web.ctx.resume();
  $('start').style.display = 'none';
  $('lemon-canvas').focus();
};
$('b-mic').onclick = async () => { await web.ctx?.resume(); startMic(); };

async function main() {
  try {
    await init();
  } catch (e) {
    // winit's web event loop ends `main` by throwing its control-flow exception: not an error.
    if (!String(e).includes('Using exceptions for control flow')) {
      web.error = String(e);
      console.error(e);
    }
  }
  $('loading').style.display = 'none';
  web.ready = true;
}

await main();
await startAudio();
