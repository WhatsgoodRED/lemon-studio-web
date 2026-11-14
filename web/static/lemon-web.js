// MetroBar Studio web build (based on Lemon Studio by ItsAlphaNeon): the page side. GPL-3.0-or-later.
//
// Loads the wasm build of Lemon Studio (it draws into #lemon-canvas through winit and softbuffer), and runs:
// - the audio: the browser has no audio thread for the engine, so this pump asks the engine for blocks on the main
//   thread (`lemon_audio_render`) and keeps an AudioWorklet ("lemon-out") a little ahead. No SharedArrayBuffer is
//   used, so the page needs no COOP/COEP headers.
// - the microphone: an AudioWorklet ("lemon-in") posts what it records to the engine (`lemon_audio_input`).
// - the files: the app's files live in an in-memory file system (`lemon_fs_*`); this page keeps them in IndexedDB
//   between visits, puts dropped and imported audio files in, and downloads exports and saved projects.

// TODO(owner): the public URL of this build's source (GPL-3.0 §6). The repository is private for now.
const SOURCE_URL = 'https://github.com/WhatsgoodRED/lemon-studio-web/tree/web';

import init, * as lemon from './pkg/lemonstudio.js';

const $ = (id) => document.getElementById(id);
const status = (t) => { $('status').textContent = t; };
$('source-link').href = SOURCE_URL.startsWith('TODO') ? 'https://github.com/ItsAlphaNeon/Lemon-Studio' : SOURCE_URL;

const BLOCK = 512;          // frames per render call (`device::web::BLOCK`)
const AHEAD = 4 * BLOCK;    // frames kept queued in the worklet (about 43 ms at 48 kHz)
const KEEP = ['/user'];     // folders kept in IndexedDB

const web = window.lemonWeb = {
  ctx: null, out: null, mic: null, analyser: null, lemon,
  produced: 0, read: 0, peak: 0, maxPeak: 0, under: 0, played: 0, renders: 0, micFrames: 0,
  ready: false, error: null, preload: new Uint8Array(0), downloads: [],
  takePreload() { const p = web.preload; web.preload = new Uint8Array(0); return p; },
};

// ---- IndexedDB: the files kept between visits -------------------------------------------------------------------

function db() {
  return new Promise((ok, fail) => {
    const r = indexedDB.open('lemon-studio-web', 1);
    r.onupgradeneeded = () => r.result.createObjectStore('files');
    r.onsuccess = () => ok(r.result);
    r.onerror = () => fail(r.error);
  });
}

async function loadKept() {
  try {
    const d = await db();
    const all = await new Promise((ok, fail) => {
      const out = [];
      const req = d.transaction('files').objectStore('files').openCursor();
      req.onsuccess = () => { const c = req.result; if (c) { out.push([c.key, c.value]); c.continue(); } else ok(out); };
      req.onerror = () => fail(req.error);
    });
    const enc = new TextEncoder();
    let size = 0;
    const parts = all.map(([k, v]) => { const p = enc.encode(k); const b = new Uint8Array(v); size += 8 + p.length + b.length; return [p, b]; });
    const pack = new Uint8Array(size);
    const dv = new DataView(pack.buffer);
    let at = 0;
    for (const [p, b] of parts) {
      dv.setUint32(at, p.length, true); pack.set(p, at + 4); at += 4 + p.length;
      dv.setUint32(at, b.length, true); pack.set(b, at + 4); at += 4 + b.length;
    }
    web.preload = pack;
    web.keptCount = all.length;
  } catch (e) {
    console.warn('IndexedDB not available: files are not kept', e);
  }
}

let lastVersion = -1;
async function keep() {
  if (!web.ready) return;   // a reload before the app has started: nothing of its files to keep yet
  const v = lemon.lemon_fs_version();
  if (v === lastVersion) return;
  lastVersion = v;
  try {
    const d = await db();
    const now = new Map();
    for (const dir of KEEP) {
      for (const line of lemon.lemon_fs_list(dir).split('\n')) {
        if (!line) continue;
        const [path, len, ms] = line.split('\t');
        if (path.endsWith('.part')) continue;
        now.set(path, len + ':' + ms);
      }
    }
    const tx = d.transaction('files', 'readwrite');
    const st = tx.objectStore('files');
    web.stamps ??= new Map();
    for (const [path, stamp] of now) {
      if (web.stamps.get(path) === stamp) continue;
      const bytes = lemon.lemon_fs_read(path);
      if (bytes) st.put(bytes.buffer, path);
      web.stamps.set(path, stamp);
    }
    for (const path of [...web.stamps.keys()]) {
      if (!now.has(path)) { st.delete(path); web.stamps.delete(path); }
    }
  } catch (e) {
    console.warn('keeping files failed', e);
  }
}

// ---- audio ------------------------------------------------------------------------------------------------------

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
  if (ctx.state !== 'running') $('start').style.display = 'flex';
  // iPhone and iPad: play through the ringer switch (Safari 17+), and a context the system stopped (a call, the tab
  // in the background: "interrupted" or "suspended") asks for a tap again.
  try { if (navigator.audioSession) navigator.audioSession.type = 'playback'; } catch (e) {}
  ctx.onstatechange = () => { $('start').style.display = ctx.state === 'running' || ctx.state === 'closed' ? 'none' : 'flex'; };
}

// Any tap or click resumes a context that is not running: Safari only allows it inside the gesture, and counts
// touchend (not touchstart) as one.
function unlockAudio() {
  const ctx = web.ctx;
  if (ctx && ctx.state !== 'running' && ctx.state !== 'closed') ctx.resume().catch(() => {});
}
for (const t of ['touchend', 'pointerup', 'mousedown', 'keydown']) document.addEventListener(t, unlockAudio, { capture: true, passive: true });

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
    await web.ctx.resume();
    const stream = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false } });
    const src = web.ctx.createMediaStreamSource(stream);
    const node = new AudioWorkletNode(web.ctx, 'lemon-in', { numberOfInputs: 1, numberOfOutputs: 0 });
    node.port.onmessage = (e) => { lemon.lemon_audio_input(e.data, 1); web.micFrames += e.data.length; };
    src.connect(node);
    web.mic = { stream, src, node };
    lemon.lemon_audio_mic(1);
    status('Microphone on: record with the toolbar microphone, or arm a mixer track with it as input');
    return true;
  } catch (e) {
    status('Microphone not available: ' + e.message);
    return false;
  }
}
web.startMic = startMic;

// ---- files ------------------------------------------------------------------------------------------------------

const AUDIO = /\.(wav|mp3|flac|ogg|oga|aif|aiff|m4a|aac|wv)$/i;
const PROJECT = /\.(lemon|flp|zip)$/i;

async function putFile(file, dir) {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const name = file.name.replace(/[\\/:]/g, '_');
  const path = `${dir}/${name}`;
  lemon.lemon_fs_write(path, bytes);
  return path;
}

async function importFiles(files, at) {
  for (const f of files) {
    if (PROJECT.test(f.name)) {
      const p = await putFile(f, '/user/Projects');
      lemon.lemon_open(p);
      status('Opening ' + f.name);
    } else if (AUDIO.test(f.name)) {
      const p = await putFile(f, '/user/Imported');
      if (at) lemon.lemon_drop(p, at[0], at[1]); else lemon.lemon_import(p);
      status('Imported ' + f.name);
    } else {
      const p = await putFile(f, '/user/Imported');
      if (at) lemon.lemon_drop(p, at[0], at[1]);
      status('Added ' + f.name);
    }
  }
}
web.importFiles = importFiles;

function download(path) {
  const bytes = lemon.lemon_fs_read(path);
  if (!bytes) return;
  const name = path.split('/').pop();
  const type = /\.wav$/i.test(name) ? 'audio/wav' : 'application/octet-stream';
  const url = URL.createObjectURL(new Blob([bytes], { type }));
  const a = document.createElement('a');
  a.href = url; a.download = name; document.body.appendChild(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10000);
  web.downloads.push({ path, size: bytes.length });
  status('Downloaded ' + name);
}
web.download = download;

function pollDownloads() {
  const list = lemon.lemon_take_downloads();
  for (const p of list.split('\n')) if (p) download(p);
}

web.state = () => JSON.parse(lemon.lemon_state() || '{}');

// ---- day / night: MetroBar's own switch (the Highlight Zone's sun / moon toggle) and its choice, metrobar.bookDay in
// localStorage (true or missing = day), shared with the MetroBar app on this site. Day = the MetroBar theme, night =
// MetroBar Night; the app reads the choice at start (`lemonWeb.night`) and takes a flip live (`lemon_set_night`).

const DAY_KEY = 'metrobar.bookDay';
const isDay = () => { try { return JSON.parse(localStorage.getItem(DAY_KEY) || 'true') !== false; } catch (_) { return true; } };
web.night = () => !isDay();
function showLook(day) {
  document.documentElement.classList.toggle('night', !day);
  document.querySelector('meta[name="theme-color"]').content = day ? '#eef0ec' : '#111413';
  const b = $('b-night');
  b.title = day ? 'Night mode' : 'Day mode';
  b.setAttribute('aria-label', 'Night mode');
  b.setAttribute('aria-checked', String(!day));
}
function setLook(day, tellApp) {
  try { localStorage.setItem(DAY_KEY, JSON.stringify(day)); } catch (_) {}
  showLook(day);
  if (tellApp && web.ready) lemon.lemon_set_night(!day);
}
web.setLook = setLook;
showLook(isDay());
$('b-night').onclick = () => setLook(!isDay(), true);
// Flipped in the MetroBar app in another tab.
window.addEventListener('storage', (e) => { if (e.key === DAY_KEY || e.key === null) { showLook(isDay()); if (web.ready) lemon.lemon_set_night(!isDay()); } });
// The MetroBar or MetroBar Night theme picked in Settings ▸ Theme: the switch and the page follow it.
let lastLook = null;
function followApp() {
  let look;
  try { look = web.state().look; } catch (_) { return; }
  if (look === lastLook) return;
  lastLook = look;
  if ((look === 'day' || look === 'night') && (look === 'day') !== isDay()) setLook(look === 'day', false);
}

// ---- page wiring ------------------------------------------------------------------------------------------------

$('start-btn').onclick = async () => {
  try { await web.ctx.resume(); } catch (e) {}
  if (web.ctx.state === 'running') $('start').style.display = 'none';
  $('lemon-canvas').focus();
};
// A phone held upright: the "turn sideways" note (index.html #turn) can be put away.
$('turn-anyway').onclick = () => document.documentElement.classList.add('upright');
$('b-mic').onclick = () => startMic();
$('b-import').onclick = () => $('file-audio').click();
$('b-open').onclick = () => $('file-project').click();
$('b-save').onclick = () => { lemon.lemon_save(); };
$('b-export').onclick = () => { lemon.lemon_export(); $('lemon-canvas').focus(); };
// Leaving (the "<- MetroBar" link, a reload, closing the tab): the browser asks first only while the project has
// unsaved changes. Settings and imported files are kept in IndexedDB; the open project is not saved by itself.
window.addEventListener('beforeunload', (e) => {
  let unsaved = false;
  try { unsaved = web.ready && !!web.state().unsaved; } catch (_) {}
  keep();
  if (unsaved) { e.preventDefault(); e.returnValue = ''; }
});
$('file-audio').onchange = (e) => { importFiles([...e.target.files]); e.target.value = ''; };
$('file-project').onchange = (e) => { importFiles([...e.target.files]); e.target.value = ''; };

const canvas = $('lemon-canvas');
let dragDepth = 0;
window.addEventListener('dragenter', (e) => { e.preventDefault(); dragDepth++; $('drop').style.display = 'flex'; });
window.addEventListener('dragleave', (e) => { e.preventDefault(); if (--dragDepth <= 0) { dragDepth = 0; $('drop').style.display = 'none'; } });
window.addEventListener('dragover', (e) => e.preventDefault());
window.addEventListener('drop', (e) => {
  e.preventDefault();
  dragDepth = 0; $('drop').style.display = 'none';
  const r = canvas.getBoundingClientRect();
  importFiles([...(e.dataTransfer?.files || [])], [e.clientX - r.left, e.clientY - r.top]);
});

async function main() {
  await loadKept();
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
  setInterval(pollDownloads, 250);
  setInterval(followApp, 250);
  setInterval(keep, 3000);
  window.addEventListener('pagehide', keep);
  document.addEventListener('visibilitychange', () => { if (document.hidden) keep(); });
}

await main();
await startAudio();
// Ask for the microphone at once where it was allowed before (no prompt then).
navigator.permissions?.query({ name: 'microphone' }).then((p) => { if (p.state === 'granted') startMic(); }).catch(() => {});
