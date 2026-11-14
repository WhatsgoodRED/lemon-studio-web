// MetroBar checks of the web build: node metrobar.js [url]. (1) A browser that kept the first, dark MetroBar theme in
// IndexedDB opens light. (2) "<- MetroBar" goes to ../landing-beat-meter.html, without a prompt when nothing is
// unsaved and with the browser's leave prompt when something is.
const { chromium } = require('/tmp/pw/node_modules/playwright-core');
const OUT = process.env.PROOF_DIR || (__dirname + '/../../../web-proof/');
const DARK = 'Hue=42\nSaturation=-50\nLightness=-200\nContrast=-8\nText=0\nSelected=11468800\nHighlight=175\nMute=175\nOption=175\nOverrideClips=1\nTextColor=15330536\nBackMode=1\nBackColor=1250321\nLemonStudioColors=MetroBar\n';
(async () => {
  const url = process.argv[2] || 'http://127.0.0.1:8765/studio-lemon/';
  const browser = await chromium.launch({ executablePath: process.env.HOME + '/Library/Caches/ms-playwright/chromium-1243/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing', headless: true, args: ['--autoplay-policy=no-user-gesture-required'] });
  const ctx = await browser.newContext({ viewport: { width: 1600, height: 900 } });
  const page = await ctx.newPage();
  const R = {};
  const ready = () => page.waitForFunction(() => window.lemonWeb && window.lemonWeb.ready, null, { timeout: 60000 });
  await page.goto(url);
  await ready();
  await page.waitForTimeout(800);
  // Where the settings live, then the dark theme put there as an earlier visit left it.
  const dir = await page.evaluate(() => lemonWeb.lemon.lemon_fs_list('/user').split('\n').map(l => l.split('\t')[0]).find(p => /\/[^/]+\.txt$/.test(p))?.replace(/\/[^/]+$/, ''));
  const dirGuess = dir || '/user/Lemon Studio';
  await page.evaluate(([p, text]) => new Promise((ok, fail) => {
    const r = indexedDB.open('lemon-studio-web', 1);
    r.onsuccess = () => { const tx = r.result.transaction('files', 'readwrite'); tx.objectStore('files').put(new TextEncoder().encode(text).buffer, p); tx.oncomplete = ok; tx.onerror = () => fail(tx.error); };
  }), [dirGuess + '/current.flstheme', DARK]);
  await page.goto('about:blank');
  await page.goto(url);
  await ready();
  await page.waitForTimeout(1500);
  const kept = await page.evaluate((p) => { const b = lemonWeb.lemon.lemon_fs_read(p); return b ? new TextDecoder().decode(b) : null; }, dirGuess + '/current.flstheme');
  await page.screenshot({ path: OUT + 'metrobar-migrated.png' });
  R.settingsDir = dirGuess;
  R.darkThemeStored = !!kept && kept.includes('Lightness=-200');
  // The return link.
  R.backHref = await page.$eval('#back', a => a.href);
  R.backResolves = R.backHref === new URL('../landing-beat-meter.html', url).href;
  let dialogs = 0;
  page.on('dialog', d => { dialogs++; d.dismiss(); });
  R.unsavedAtStart = await page.evaluate(() => lemonWeb.state().unsaved);
  await Promise.all([page.waitForURL('**/landing-beat-meter.html', { timeout: 10000 }).catch(() => {}), page.click('#back')]);
  R.landedOn = page.url();
  R.promptsWhenSaved = dialogs;
  // With an unsaved change (an imported sample) the browser asks; dismissed, the page stays.
  await page.goto(url);
  await ready();
  await page.waitForTimeout(800);
  await page.setInputFiles('#file-audio', __dirname + '/test-beat.wav');
  await page.waitForFunction(() => lemonWeb.state().unsaved, null, { timeout: 15000 }).catch(() => {});
  R.unsavedAfterImport = await page.evaluate(() => lemonWeb.state().unsaved);
  dialogs = 0;
  await page.mouse.click(800, 500);
  await page.click('#back');
  await page.waitForTimeout(1500);
  R.promptsWhenUnsaved = dialogs;
  R.stayedWhenDismissed = page.url() === url;
  console.log(JSON.stringify(R, null, 1));
  await browser.close();
})();
