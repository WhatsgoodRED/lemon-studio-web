// Day / night switch of the web build: node daynight.js [studio url] [Highlight Zone url].
// (1) Day is the default; the switch turns the app (canvas pixels) and the page to night and back, live, and the
//     choice (metrobar.bookDay in localStorage) survives a reload in both states.
// (2) The switch is MetroBar's own: with the Highlight Zone's page given, the same moments of both switches'
//     transitions are compared (the knob's travel as a share of its way, the night sky's opacity) and kept as frames.
// Writes toggle-day.png, toggle-night.png, toggle-frames/*.png and daynight.json to web-proof/.
const { chromium } = require('/tmp/pw/node_modules/playwright-core');
const fs = require('fs');
const OUT = process.env.PROOF_DIR || (__dirname + '/../../../web-proof/');
const FRAMES = OUT + 'toggle-frames/';
// bg: the page; grid: the canvas's most common colour, the empty arranger / channel rack grid (`grid` token).
const DAY = { bg: 0xeef0ec, grid: 0xf4f5f2, fg: '#15181a' }, NIGHT = { bg: 0x111413, grid: 0x161a19, fg: '#e9ece8' };
const hex = (n) => '#' + n.toString(16).padStart(6, '0');
const near = (a, b, d = 6) => [16, 8, 0].every(s => Math.abs(((a >> s) & 255) - ((b >> s) & 255)) <= d);
const MOMENTS = [0, 50, 100, 150, 200, 250, 300, 400, 500];

(async () => {
  const url = process.argv[2] || 'http://127.0.0.1:8765/';
  const hzUrl = process.argv[3];
  fs.mkdirSync(FRAMES, { recursive: true });
  const browser = await chromium.launch({ executablePath: process.env.HOME + '/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing', headless: true, args: ['--autoplay-policy=no-user-gesture-required'] });
  const ctx = await browser.newContext({ viewport: { width: 1600, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(String(e).slice(0, 300)));
  const R = { steps: [] };
  const ready = async () => { await page.waitForFunction(() => window.lemonWeb && window.lemonWeb.ready, null, { timeout: 60000 }); await page.waitForTimeout(1200); };
  // What the canvas shows (its most common colour, the workspace) and the page around it.
  const look = () => page.evaluate(() => {
    const c = document.getElementById('lemon-canvas'), g = document.createElement('canvas'); g.width = c.width; g.height = c.height;
    const x = g.getContext('2d'); x.drawImage(c, 0, 0);
    const d = x.getImageData(0, 0, g.width, g.height).data, n = new Map();
    for (let i = 0; i < d.length; i += 4 * 13) { const k = (d[i] << 16) | (d[i + 1] << 8) | d[i + 2]; n.set(k, (n.get(k) || 0) + 1); }
    const top = [...n].sort((a, b) => b[1] - a[1])[0][0];
    const css = (el, p) => getComputedStyle(el)[p];
    const sw = document.getElementById('b-night');
    return {
      canvasTop: top,
      barBg: css(document.getElementById('bar'), 'backgroundColor'), loadingBg: css(document.getElementById('loading'), 'backgroundColor'),
      wordmark: css(document.querySelector('#bar .mark .ink'), 'fill'), studio: document.querySelector('#bar .mark path:not(.ink)').getAttribute('fill'),
      night: document.documentElement.classList.contains('night'), checked: sw.getAttribute('aria-checked'),
      stored: localStorage.getItem('metrobar.bookDay'), appLook: lemonWeb.state().look,
    };
  });
  const rgb = (n) => `rgb(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255})`;
  const check = (name, l, want) => {
    const isNight = want === NIGHT;
    const ok = near(l.canvasTop, want.grid, 1) && l.barBg === rgb(isNight ? 0x1a1e1d : 0xe2e5df) && l.loadingBg === rgb(want.bg)
      && l.wordmark === rgb(parseInt(want.fg.slice(1), 16)) && l.studio === '#af0000' && l.night === isNight
      && l.checked === String(isNight) && l.appLook === (isNight ? 'night' : 'day');
    R.steps.push({ name, pass: ok, ...l, canvasTop: hex(l.canvasTop) });
    return ok;
  };

  // A fresh browser: day.
  await page.goto(url);
  await ready();
  await page.evaluate(() => localStorage.removeItem('metrobar.bookDay'));
  await page.reload();
  await ready();
  check('fresh: day', await look(), DAY);
  await page.screenshot({ path: OUT + 'toggle-day.png' });
  // Day -> night, live (no reload).
  const navs = [];
  page.on('framenavigated', f => { if (f === page.mainFrame()) navs.push(f.url()); });
  await page.click('#b-night');
  await page.waitForTimeout(900);
  check('switched to night', await look(), NIGHT);
  await page.screenshot({ path: OUT + 'toggle-night.png' });
  R.noReload = navs.length === 0;
  // Night survives a reload.
  await page.reload();
  await ready();
  check('night after reload', await look(), NIGHT);
  // Night -> day, live, and day survives a reload.
  await page.click('#b-night');
  await page.waitForTimeout(900);
  check('switched back to day', await look(), DAY);
  await page.reload();
  await ready();
  check('day after reload', await look(), DAY);
  // A night theme kept in the app's settings does not win over the switch on day (the stored-theme fix follows it).
  await page.click('#b-night'); await page.waitForTimeout(900);
  await page.evaluate(() => localStorage.setItem('metrobar.bookDay', 'true'));
  await page.reload(); await ready();
  check('stored Night theme, switch on day', await look(), DAY);
  await page.evaluate(() => localStorage.setItem('metrobar.bookDay', 'false'));
  await page.reload(); await ready();
  check('stored Day theme, switch on night', await look(), NIGHT);
  await page.click('#b-night'); await page.waitForTimeout(900);

  // The switch's transition: frames at fixed moments (CSS transitions paused and set by the Web Animations API).
  const sample = async (p, sel, tag, travel) => {
    const out = [];
    const box = await p.$eval(sel, el => { const r = el.getBoundingClientRect(); return { x: r.x - 6, y: r.y - 6, width: r.width + 12, height: r.height + 12 }; });
    await p.evaluate((sel) => document.querySelector(sel).click(), sel);
    for (const t of MOMENTS) {
      const v = await p.evaluate(([sel, t]) => {
        const el = document.querySelector(sel);
        for (const a of document.getAnimations()) if (a instanceof CSSTransition && el.contains(a.effect.target)) { a.pause(); a.currentTime = t; }
        const k = el.querySelector('.smt-k');
        const tr = getComputedStyle(k).translate;
        return { x: parseFloat(tr) || 0, sky: +getComputedStyle(el, '::before').opacity, knob: getComputedStyle(k).backgroundColor, n: document.getAnimations().filter(a => a instanceof CSSTransition && el.contains(a.effect.target)).length };
      }, [sel, t]);
      await p.screenshot({ path: `${FRAMES}${tag}-${String(t).padStart(3, '0')}.png`, clip: box });
      out.push({ t, travel: +(v.x / travel).toFixed(3), sky: +v.sky.toFixed(3), knob: v.knob, transitions: v.n });
    }
    await p.evaluate(() => { for (const a of document.getAnimations()) if (a instanceof CSSTransition) a.finish(); });
    return out;
  };
  R.studioFrames = await sample(page, '#b-night', 'studio', 18);
  R.studioStyles = await page.$eval('#b-night .smt-k', k => ({ transition: getComputedStyle(k).transition }));
  if (hzUrl) {
    const hz = await ctx.newPage();
    await hz.goto(hzUrl);
    await hz.waitForSelector('#bsDay', { state: 'attached', timeout: 30000 });
    await hz.evaluate(() => { const b = document.getElementById('bsDay'); if (b.getAttribute('aria-checked') === 'true') b.click(); for (const a of document.getAnimations()) a.finish?.(); });
    await hz.waitForTimeout(800);
    const big = await hz.$eval('#bsDay', b => b.classList.contains('smt-lg') && getComputedStyle(b).display !== 'none');
    R.hzFrames = await sample(hz, '#bsDay', 'hz', big ? 30 : 18);
    R.hzStyles = await hz.$eval('#bsDay .smt-k', k => ({ transition: getComputedStyle(k).transition }));
    R.hzSize = big ? '64x34 (.smt-lg)' : '40x22';
    R.animationMatches = R.studioStyles.transition === R.hzStyles.transition
      && R.studioFrames.every((f, i) => Math.abs(f.travel - R.hzFrames[i].travel) < 0.02 && Math.abs(f.sky - R.hzFrames[i].sky) < 0.02 && f.knob === R.hzFrames[i].knob);
    await hz.close();
  }
  R.errors = errors;
  R.pass = R.steps.every(s => s.pass) && R.noReload && !errors.length && (hzUrl ? R.animationMatches : true);
  fs.writeFileSync(OUT + 'daynight.json', JSON.stringify(R, null, 1));
  console.log(JSON.stringify(R, null, 1));
  await browser.close();
  process.exit(R.pass ? 0 : 1);
})();
