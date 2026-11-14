const { chromium } = require('/tmp/pw/node_modules/playwright-core');
// Brand screenshots of the web build: node brand.js [url] [out-dir]. UNDOCK=x,y floats the mixer, ABOUT=x,y opens About
// (1563,96 and 1548,42 at 1600 x 900).
const OUT = process.argv[3] || (__dirname + '/../../../web-proof/');
(async () => {
  const url = process.argv[2] || 'http://127.0.0.1:8765/';
  const browser = await chromium.launch({ executablePath: process.env.HOME + '/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing', headless: true, args: ['--autoplay-policy=no-user-gesture-required'] });
  const ctx = await browser.newContext({ viewport: { width: 1600, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  await page.route('**/*.wasm', async (r) => { await new Promise((d) => setTimeout(d, 2500)); r.continue(); });
  await page.goto(url);
  await page.waitForTimeout(1200);
  await page.screenshot({ path: OUT + 'brand-loading.png' });
  await page.waitForFunction(() => window.lemonWeb && window.lemonWeb.ready, null, { timeout: 60000 });
  await page.waitForTimeout(2000);
  await page.screenshot({ path: OUT + 'brand-start.png' });
  await page.mouse.click(800, 600);
  await page.keyboard.press(process.env.MIXKEY || 'F9');
  await page.waitForTimeout(800);
  await page.screenshot({ path: OUT + 'brand-main.png' });
  if (process.env.UNDOCK) { const [x, y] = process.env.UNDOCK.split(',').map(Number); await page.mouse.click(x, y); await page.waitForTimeout(800); await page.screenshot({ path: OUT + 'brand-main2.png' }); }
  if (process.env.ABOUT) { const [x, y] = process.env.ABOUT.split(',').map(Number); await page.mouse.click(x, y); await page.waitForTimeout(800); }
  await page.screenshot({ path: OUT + 'brand-about.png' });
  console.log(JSON.stringify({ errors, title: await page.title() }));
  await browser.close();
})();
