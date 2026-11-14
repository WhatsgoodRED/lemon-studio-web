// Drives the web build in headless Chromium: node e2e.js <steps.json> ; steps: [{click:[x,y]}, {key:"Space"}, {wait:ms},
// {shot:"file.png"}, {eval:"js"}, {upload:{sel, files}}, {dblclick:[x,y]}, {move:[x,y]}, {type:"text"}]
const { chromium } = require('/tmp/pw/node_modules/playwright-core');
const fs = require('fs');
(async () => {
  const steps = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
  const browser = await chromium.launch({ executablePath: process.env.HOME + '/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing', headless: true,
    args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--autoplay-policy=no-user-gesture-required'] });
  const ctx = await browser.newContext({ viewport: { width: 1600, height: 900 }, acceptDownloads: true });
  await ctx.grantPermissions(['microphone']);
  const page = await ctx.newPage();
  page.on('console', m => { const t = m.text(); if (!t.includes('404')) console.log('[console]', m.type(), t.slice(0, 600)); });
  page.on('pageerror', e => console.log('[pageerror]', String(e).slice(0, 1500)));
  page.on('download', async d => { const p = '/tmp/dl-' + d.suggestedFilename(); await d.saveAs(p); console.log('[download]', p, fs.statSync(p).size); });
  await page.goto(process.argv[3] || 'http://127.0.0.1:8765/');
  await page.waitForFunction(() => window.lemonWeb && window.lemonWeb.ready, null, { timeout: 60000 });
  await page.waitForTimeout(2500);
  for (const s of steps) {
    if (s.click) await page.mouse.click(s.click[0], s.click[1], { button: s.button || 'left' });
    if (s.dblclick) await page.mouse.dblclick(s.dblclick[0], s.dblclick[1]);
    if (s.move) await page.mouse.move(s.move[0], s.move[1]);
    if (s.down) { await page.mouse.move(s.down[0], s.down[1]); await page.mouse.down(); }
    if (s.up) { await page.mouse.move(s.up[0], s.up[1], { steps: 8 }); await page.mouse.up(); }
    if (s.key) await page.keyboard.press(s.key);
    if (s.keydown) await page.keyboard.down(s.keydown);
    if (s.keyup) await page.keyboard.up(s.keyup);
    if (s.type) await page.keyboard.type(s.type);
    if (s.wait) await page.waitForTimeout(s.wait);
    if (s.shot) await page.screenshot({ path: s.shot });
    if (s.upload) await page.setInputFiles(s.upload.sel, s.upload.files);
    if (s.eval) console.log('[eval]', JSON.stringify(await page.evaluate(s.eval)));
    if (s.drop) {
      const data = fs.readFileSync(s.drop.file).toString('base64');
      await page.evaluate(async ([b64, name, x, y]) => {
        const bin = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
        const dt = new DataTransfer(); dt.items.add(new File([bin], name));
        const el = document.elementFromPoint(x, y);
        for (const t of ['dragenter', 'dragover', 'drop']) el.dispatchEvent(new DragEvent(t, { dataTransfer: dt, bubbles: true, cancelable: true, clientX: x, clientY: y }));
      }, [data, s.drop.name, s.drop.at[0], s.drop.at[1]]);
    }
  }
  await browser.close();
})();
