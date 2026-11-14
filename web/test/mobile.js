// Touch check of the web build on a phone and a tablet (emulated): node mobile.js <url of studio-lemon/> <out dir>
// Fingers only (CDP touch events): close and reopen the instrument rack, open Lemon Sampler's window, drag one of its
// knobs, play and stop, a long press (right click), and a two-finger scroll. Screenshots per step; results.json.
// Headless Chromium reports the canvas's device-pixel size at 1x whatever devicePixelRatio is emulated, so the runs
// use deviceScaleFactor 1 (real browsers agree with themselves).
const { chromium } = require('/tmp/pw/node_modules/playwright-core');
const fs = require('fs');
const URL = process.argv[2] || 'http://127.0.0.1:8771/', OUT = process.argv[3] || '/tmp/mobile-proof';
fs.mkdirSync(OUT, { recursive: true });
const DEVICES = [
  // Where things are, in CSS px (from the first screenshot of each): the rack's close box, the toolbar's rack button,
  // Lemon Sampler's button on the rack, Play, Stop, an empty grid cell and a spot on the playlist for two fingers.
  { name: 'iphone14-landscape', viewport: { width: 844, height: 390 }, at: { close: [483, 124], rackBtn: [623, 37], sampler: [279, 152], play: [258, 12], stop: [277, 12], cell: [640, 300], scroll: [600, 260], blank: [60, 250], knob: [436, 168] }, ua: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1' },
  { name: 'android-tablet', viewport: { width: 1280, height: 800 }, at: { close: [814, 209], rackBtn: [1053, 61], sampler: [471, 256], play: [435, 20], stop: [466, 20], cell: [1000, 650], scroll: [1000, 560], blank: [100, 500], knob: [735, 282] }, ua: 'Mozilla/5.0 (Linux; Android 14; SM-X710) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36' },
];
(async () => {
  const browser = await chromium.launch({ executablePath: process.env.HOME + '/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing', headless: true });
  const all = {};
  for (const d of DEVICES) {
    const ctx = await browser.newContext({ viewport: d.viewport, deviceScaleFactor: 1, isMobile: true, hasTouch: true, userAgent: d.ua });
    const page = await ctx.newPage();
    const cdp = await ctx.newCDPSession(page);
    const errs = [];
    page.on('pageerror', e => errs.push(String(e).slice(0, 300)));
    page.on('console', m => { if (m.type() === 'error') errs.push(m.text().slice(0, 300)); });
    const R = {}; all[d.name] = R;
    const shot = (n) => page.screenshot({ path: `${OUT}/${d.name}-${n}.png` });
    const wait = (ms) => page.waitForTimeout(ms);
    const touch = (type, pts) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: pts.map(([x, y], i) => ({ x, y, id: i })) });
    const tap = async (x, y) => { await page.touchscreen.tap(x, y); await wait(400); };
    const drag = async (x0, y0, x1, y1, steps = 12) => {
      await touch('touchStart', [[x0, y0]]);
      for (let i = 1; i <= steps; i++) { await touch('touchMove', [[x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps]]); await wait(16); }
      await touch('touchEnd', []); await wait(400);
    };
    const dbg = () => page.evaluate(() => lemonWeb.lemon.lemon_debug());
    // The canvas's pixels at CSS point (x, y) of a fresh screenshot, to see whether something changed there.
    const region = async (x, y, w, h) => (await page.screenshot({ clip: { x, y, width: w, height: h } })).toString('base64');

    await page.goto(URL);
    await page.waitForFunction(() => window.lemonWeb && window.lemonWeb.ready, null, { timeout: 60000 });
    await wait(2500);
    const geo = await page.evaluate(() => { const c = document.getElementById('lemon-canvas'), b = document.getElementById('bar'); return { canvas: [c.width, c.height], bar: [b.scrollWidth, b.clientWidth, b.offsetHeight], audio: lemonWeb.ctx && lemonWeb.ctx.state }; });
    // The app lays out 1024 x 600 at least, drawn at k = min(1, w/1024, h/600) CSS px per logical px.
    const k = Math.min(1, geo.canvas[0] / 1024, geo.canvas[1] / 600);
    const A = d.at;
    R.geometry = { ...geo, k, logical: [geo.canvas[0] / k, geo.canvas[1] / k] };
    R.fits = { pass: geo.canvas[0] / k >= 1023 && geo.canvas[1] / k >= 599 && geo.canvas[0] <= d.viewport.width };
    await shot('1-start');

    // Audio starts suspended on phones (always on iOS, often in headless Chromium): the "Start audio" veil covers the
    // canvas until a tap. Tap its button; the context runs and the veil goes, so the next taps reach the app.
    const audio = () => page.evaluate(() => ({ ctx: lemonWeb.ctx.state, veil: getComputedStyle(document.getElementById('start')).display }));
    const a0 = await audio();
    if (a0.veil !== 'none') {
      const r = await page.evaluate(() => { const b = document.getElementById('start-btn').getBoundingClientRect(); return [b.x + b.width / 2, b.y + b.height / 2]; });
      await shot('1a-start-audio');
      await tap(...r);
    }
    const a1 = await audio();
    R.start_audio = { pass: a1.ctx === 'running' && a1.veil === 'none', before: a0, after: a1 };

    // Instrument rack: its window's close box, then the toolbar's rack button opens it again.
    const rackOpen = () => region(A.sampler[0] - 30, A.sampler[1] - 12, 60, 24);
    const before = await rackOpen();
    await tap(...A.close);
    await shot('2-rack-closed');
    const closed = await rackOpen();
    // After its close box the toolbar button can take two presses to bring it back (the mouse's clicks do the same).
    let presses = 0, reopened;
    do { await tap(...A.rackBtn); presses++; await wait(300); reopened = await rackOpen(); } while (reopened !== before && presses < 2);
    await shot('3-rack-open');
    R.rack = { pass: closed !== before && reopened === before, presses };

    // A long press on Lemon Sampler's button is its right click: the channel's menu opens. A tap on an empty spot
    // puts it away.
    const menuArea = () => region(A.sampler[0], A.sampler[1], 160, 120);
    const m0 = await menuArea();
    await touch('touchStart', [A.sampler]); await wait(800); await touch('touchEnd', []); await wait(400);
    await shot('3b-long-press-menu');
    R.long_press_menu = { pass: (await menuArea()) !== m0 };
    await tap(...A.blank); await wait(300);
    R.menu_closed = { pass: (await menuArea()) === m0 };

    // Lemon Sampler's button on the rack opens its window.
    const winArea = () => region(0, 0, d.viewport.width, d.viewport.height - 40);
    const noWin = await winArea();
    await tap(...A.sampler);
    await wait(1200);
    await shot('4-sampler');
    R.sampler = { pass: (await winArea()) !== noWin };
    R.errors = errs.slice();
    all[d.name] = R;
    if (process.env.STOP_AT_SAMPLER) { await ctx.close(); continue; }

    // The sampler's Time stretching Pitch knob: drag it up; its picture changes.
    const [kx, ky] = A.knob;
    {
      const knob = () => region(kx - 14, ky - 14, 28, 28);
      const k0 = await knob();
      await drag(kx, ky, kx, ky - 60);
      await shot('5-knob-dragged');
      R.knob = { pass: (await knob()) !== k0 };
    }

    // Play and stop (the transport's buttons).
    await tap(...A.play); await wait(600);
    const playing = await dbg();
    await shot('6-playing');
    await tap(...A.stop); await wait(400);
    const stopped = await dbg();
    R.play_stop = { pass: /state=1/.test(playing) && /state=0/.test(stopped), playing: playing.match(/state=\d/)[0], stopped: stopped.match(/state=\d/)[0] };

    // Two fingers moving up together over the playlist: it scrolls.
    const grid = () => region(0, 80, d.viewport.width, d.viewport.height - 130);
    const g0 = await grid();
    const [sx, sy] = A.scroll;
    await touch('touchStart', [[sx, sy], [sx + 60, sy]]);
    for (let i = 1; i <= 10; i++) { await touch('touchMove', [[sx, sy - i * 12], [sx + 60, sy - i * 12]]); await wait(16); }
    await touch('touchEnd', []); await wait(500);
    await shot('8-two-finger-scroll');
    R.scroll = { pass: (await grid()) !== g0 };

    // The bar scrolls sideways when it is wider than the screen.
    R.bar = { pass: geo.bar[0] <= geo.bar[1] || await page.evaluate(() => { const b = document.getElementById('bar'); b.scrollLeft = 9999; return b.scrollLeft > 0; }), scrollWidth: geo.bar[0], width: geo.bar[1], height: geo.bar[2] };
    R.errors = errs;
    await ctx.close();
  }
  // Portrait phone: the "turn sideways" note.
  const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 1, isMobile: true, hasTouch: true });
  const page = await ctx.newPage();
  await page.goto(URL);
  await page.waitForFunction(() => window.lemonWeb && window.lemonWeb.ready, null, { timeout: 60000 });
  await page.waitForTimeout(1500);
  await page.screenshot({ path: `${OUT}/iphone14-portrait.png` });
  all.portrait = { pass: await page.evaluate(() => getComputedStyle(document.getElementById('turn')).display === 'flex') };
  await page.touchscreen.tap(195, 520); await page.waitForTimeout(300);
  all.portrait.dismissed = await page.evaluate(() => getComputedStyle(document.getElementById('turn')).display === 'none');
  await page.screenshot({ path: `${OUT}/iphone14-portrait-anyway.png` });
  await browser.close();
  fs.writeFileSync(`${OUT}/results.json`, JSON.stringify(all, null, 1));
  console.log(JSON.stringify(all, null, 1));
})();
