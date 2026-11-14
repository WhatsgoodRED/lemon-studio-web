// Opens the web build in headless Chromium and saves a screenshot and the console.
const { chromium } = require('/tmp/pw/node_modules/playwright-core');
(async () => {
  const url = process.argv[2] || 'http://127.0.0.1:8765/';
  const shot = process.argv[3] || 'shot.png';
  const wait = +(process.argv[4] || 8000);
  const browser = await chromium.launch({ executablePath: process.env.HOME + '/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing', headless: true, args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--autoplay-policy=no-user-gesture-required', '--enable-unsafe-swiftshader'] });
  const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
  page.on('console', m => console.log('[console]', m.type(), m.text().slice(0, 400)));
  page.on('pageerror', e => console.log('[pageerror]', String(e).slice(0, 2000)));
  await page.goto(url);
  await page.waitForTimeout(wait);
  await page.screenshot({ path: shot });
  console.log(JSON.stringify(await page.evaluate(() => { const w = window.lemonWeb || {}; return { ready: w.ready, error: w.error, produced: w.produced, read: w.read, maxPeak: w.maxPeak, renders: w.renders, state: w.ctx && w.ctx.state }; })));
  await browser.close();
})();
