// Opens every built-in instrument and effect in the web build and screenshots each: node plugins.js <out dir> <url of studio-lemon/>
const { chromium } = require('/tmp/pw/node_modules/playwright-core');
const OUT = process.argv[2];
(async () => {
  const browser = await chromium.launch({ executablePath: process.env.HOME + '/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing', headless: true, args: ['--autoplay-policy=no-user-gesture-required'] });
  const page = await (await browser.newContext({ viewport: { width: 1600, height: 900 } })).newPage();
  const errs = [];
  page.on('pageerror', e => errs.push(String(e).slice(0, 300)));
  page.on('console', m => { const t = m.text(); if (m.type() === 'error' || t.startsWith('lemon_test')) errs.push(t.slice(0, 400)); });
  await page.goto(process.argv[3]);
  await page.waitForFunction(() => window.lemonWeb && window.lemonWeb.ready, null, { timeout: 60000 });
  await page.waitForTimeout(2500);
  const mod = await page.evaluate(async () => { const m = await import(document.querySelector('script[type=module]').src.replace('lemon-web.js', 'pkg/lemonstudio.js').replace(/\?v=.*/, '') + '?v=' + (document.querySelector('script[type=module]').src.split('?v=')[1] || '')); return JSON.parse(m.lemon_test_plugins() || '{}'); });
  console.log('plugins', JSON.stringify(mod));
  const results = [];
  for (const kind of ['gen', 'fx']) for (const name of (mod[kind] || [])) {
    const before = errs.length;
    await page.evaluate(async ([k, n, i]) => { const src = document.querySelector('script[type=module]').src; const m = await import(src.replace('lemon-web.js', 'pkg/lemonstudio.js').replace(/\?v=.*/, '') + '?v=' + (src.split('?v=')[1] || '')); m.lemon_test(k === 'fx' ? 'fx:' + (2 + Math.floor(i / 8)) + ':' + n : k + ':' + n); }, [kind, name, results.length]);
    await page.waitForTimeout(1500);
    const f = `${OUT}/${kind}-${name.replace(/[^a-z0-9]+/gi, '_')}.png`;
    await page.screenshot({ path: f });
    results.push({ kind, name, errors: errs.slice(before) });
    if (errs.slice(before).some(e => e.includes('unreachable') || e.includes('panicked'))) { console.log('PANIC at', name, errs.slice(before)); break; }
  }
  console.log(JSON.stringify(results, null, 1));
  await browser.close();
})();
