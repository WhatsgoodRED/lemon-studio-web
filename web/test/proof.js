// Pass/fail proof of the web build: node proof.js [url]. Writes screenshots and results to ../web-proof/.
const { chromium } = require('/tmp/pw/node_modules/playwright-core');
const fs = require('fs');
const OUT = process.env.PROOF_DIR || (__dirname + '/../../../web-proof/');
const R = {};
const peakOfWav = (b) => {
  let i = 12, peak = 0, frames = 0, fmt = null;
  while (i + 8 <= b.length) {
    const id = b.toString('latin1', i, i + 4), n = b.readUInt32LE(i + 4);
    if (id === 'fmt ') fmt = { tag: b.readUInt16LE(i + 8), ch: b.readUInt16LE(i + 10), rate: b.readUInt32LE(i + 12), bits: b.readUInt16LE(i + 22) };
    if (id === 'data') {
      const bytes = fmt.bits / 8;
      for (let k = i + 8; k + bytes <= i + 8 + n; k += bytes) {
        const v = fmt.tag === 3 ? b.readFloatLE(k) : b.readInt16LE(k) / 32768;
        peak = Math.max(peak, Math.abs(v));
      }
      frames = n / bytes / fmt.ch;
    }
    i += 8 + n + (n & 1);
  }
  return { fmt, frames, peak };
};
(async () => {
  const url = process.argv[2] || 'http://127.0.0.1:8765/';
  const browser = await chromium.launch({ executablePath: process.env.HOME + '/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing', headless: true,
    args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--autoplay-policy=no-user-gesture-required'] });
  const ctx = await browser.newContext({ viewport: { width: 1600, height: 900 }, acceptDownloads: true });
  await ctx.grantPermissions(['microphone']);
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(String(e).slice(0, 500)));
  page.on('console', m => { if (m.type() === 'error' && !m.text().includes('404')) errors.push(m.text().slice(0, 500)); });
  const downloads = [];
  page.on('download', async d => { const p = '/tmp/proof-' + d.suggestedFilename(); await d.saveAs(p); downloads.push(p); });
  const ev = (js) => page.evaluate(js);
  const dbg = () => ev('lemonWeb.lemon.lemon_debug()');

  const t0 = Date.now();
  await page.goto(url);
  await page.waitForFunction(() => window.lemonWeb && window.lemonWeb.ready, null, { timeout: 60000 });
  await page.waitForTimeout(2500);
  R.load_ms = Date.now() - t0;
  // (a) UI renders: the canvas has many distinct colours.
  await page.screenshot({ path: OUT + 'a-ui.png' });
  const colours = await ev(`(() => { const c = document.getElementById('lemon-canvas'); const g = document.createElement('canvas'); g.width = c.width; g.height = c.height; const x = g.getContext('2d'); x.drawImage(c, 0, 0); const d = x.getImageData(0, 0, g.width, g.height).data; const s = new Set(); for (let i = 0; i < d.length; i += 4 * 97) s.add((d[i] << 16) | (d[i+1] << 8) | d[i+2]); return { w: c.width, h: c.height, colours: s.size }; })()`);
  R.a_ui = { pass: colours.colours > 20 && !errors.length, ...colours };

  // (b) clicks reach the UI: the Play button starts the transport, Stop stops it.
  await page.mouse.click(441, 22); await page.waitForTimeout(700);
  const playing = await dbg();
  await page.mouse.click(472, 22); await page.waitForTimeout(500);
  const stopped = await dbg();
  R.b_clicks = { pass: /state=1/.test(playing) && /state=0/.test(stopped), playing: playing.match(/state=\d/)[0], stopped: stopped.match(/state=\d/)[0] };
  await page.screenshot({ path: OUT + 'b-after-clicks.png' });

  // (e) import of a WAV (the page's Import button): it becomes an audio clip.
  await page.setInputFiles('#file-audio', __dirname + '/test-beat.wav');
  await page.waitForTimeout(1500);
  const st = await ev('lemonWeb.state()');
  R.e_import = { pass: st.clips.length === 1 && st.samples.some(s => s.endsWith('test-beat.wav')), message: st.message, clips: st.clips, samples: st.samples };
  await page.screenshot({ path: OUT + 'e-wav-imported.png' });

  // (c) audio plays: SONG mode, Play; the AudioWorklet's output peak and the analyser's RMS.
  await page.mouse.click(408, 29); await page.waitForTimeout(300);
  await ev('lemonWeb.maxPeak = 0');
  await page.mouse.click(441, 22);
  await page.waitForTimeout(500);
  const audio = await ev(`new Promise((done) => { const a = lemonWeb.analyser; const b = new Float32Array(a.fftSize); let rms = 0, n = 0; const t = setInterval(() => { a.getFloatTimeDomainData(b); let s = 0; for (const v of b) s += v * v; rms = Math.max(rms, Math.sqrt(s / b.length)); if (++n === 20) { clearInterval(t); done({ workletPeak: lemonWeb.maxPeak, analyserRmsMax: rms, rate: lemonWeb.ctx.sampleRate, ctx: lemonWeb.ctx.state, baseLatency: lemonWeb.ctx.baseLatency }); } }, 50); })`);
  await page.waitForTimeout(1000);
  audio.underrunsAfter = await ev('lemonWeb.under');
  await page.screenshot({ path: OUT + 'c-playing.png' });
  await page.mouse.click(472, 22); await page.waitForTimeout(500);
  R.c_audio = { pass: audio.workletPeak > 0.05 && audio.analyserRmsMax > 0.001, ...audio };

  // (f) export to WAV: the page's Export button opens the app's Export form; its Export button renders and the page
  // downloads the file.
  await page.click('#b-export'); await page.waitForTimeout(800);
  await page.screenshot({ path: OUT + 'f-export-form.png' });
  await page.keyboard.press('Enter');
  for (let i = 0; i < 40 && !downloads.length; i++) await page.waitForTimeout(250);
  const wav = downloads[0] ? peakOfWav(fs.readFileSync(downloads[0])) : null;
  if (downloads[0]) fs.copyFileSync(downloads[0], OUT + 'f-exported.wav');
  R.f_export = { pass: !!wav && wav.frames > 44100 && wav.peak > 0.05, file: downloads[0], ...wav };

  // (d) a microphone recording lands as an audio clip: toolbar microphone, In 1, record 3 s, Stop.
  const before = (await ev('lemonWeb.state()')).clips.length;
  await page.mouse.click(1296, 22); await page.waitForTimeout(500);
  await page.mouse.click(1310, 88); await page.waitForTimeout(3000);
  await page.screenshot({ path: OUT + 'd-recording.png' });
  await page.mouse.click(472, 22); await page.waitForTimeout(1500);
  const st2 = await ev('lemonWeb.state()');
  const rec = st2.samples.find(s => s.includes('/Recorded/'));
  let recWav = null;
  if (rec) {
    const bytes = await ev(`Array.from(lemonWeb.lemon.lemon_fs_read(${JSON.stringify(rec)}))`);
    recWav = peakOfWav(Buffer.from(bytes));
  }
  R.d_mic_recording = { pass: !!rec && st2.clips.length === before + 1 && recWav && recWav.peak > 0.01, clipsBefore: before, clipsAfter: st2.clips.length, file: rec, micFrames: await ev('lemonWeb.micFrames'), ...recWav };
  await page.screenshot({ path: OUT + 'd-recorded-clip.png' });

  // Kept between visits: reload; the recording and the import are back in the file system.
  await ev('lemonWeb.lemon.lemon_fs_version()');
  await page.waitForTimeout(3500);
  await page.reload();
  await page.waitForFunction(() => window.lemonWeb && window.lemonWeb.ready, null, { timeout: 60000 });
  await page.waitForTimeout(1500);
  const kept = await ev(`lemonWeb.lemon.lemon_fs_list('/user')`);
  R.kept_after_reload = { pass: kept.includes('test-beat.wav') && kept.includes('/Recorded/'), files: kept.split('\n').filter(Boolean).map(l => l.split('\t')[0]) };

  R.errors = errors;
  R.wasm_bytes = fs.statSync((process.env.SITE_DIR || (__dirname + '/../../../studio-lemon')) + '/pkg/lemonstudio_bg.wasm').size;
  fs.writeFileSync(OUT + 'results.json', JSON.stringify(R, null, 2));
  console.log(JSON.stringify(R, null, 2));
  await browser.close();
})();
