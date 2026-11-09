// 量一下两种字体在各自设计字号下的真实字面度量，用于重排版面。
const fs = require('fs'), path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');
const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const ARK = path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf');
const ARK10 = path.join(ROOT, 'slot-main/crates/slot-ui/assets/ark-pixel-10px-proportional-zh_hans.otf');
const CHILL = path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf');
const EXE = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: EXE, args: ['--no-sandbox', '--disable-gpu'] });
  const page = await browser.newPage();
  await page.setContent('<canvas id=c width=8 height=8></canvas>');
  await page.addStyleTag({ content:
    `@font-face{font-family:"ArkPixel";src:url(data:font/otf;base64,${fs.readFileSync(ARK).toString('base64')}) format("opentype");}` +
    `@font-face{font-family:"ArkPixel10";src:url(data:font/otf;base64,${fs.readFileSync(ARK10).toString('base64')}) format("opentype");}` +
    `@font-face{font-family:"ChillBitmap";src:url(data:font/ttf;base64,${fs.readFileSync(CHILL).toString('base64')}) format("truetype");}` });
  await page.evaluate(async () => {
    await document.fonts.load('16px "ChillBitmap"', '国逃A-');
    await document.fonts.load('12px "ArkPixel"', '国逃A-1');
    await document.fonts.load('10px "ArkPixel10"', 'PLAY4:12[REPEAT]');
    await document.fonts.ready;
  });
  const out = await page.evaluate(() => {
    const c = document.getElementById('c').getContext('2d');
    const m = (ch, px, fam) => {
      c.font = `${px}px "${fam}"`;
      const t = c.measureText(ch);
      const r = (v) => (v === undefined ? null : +v.toFixed(2));
      return { adv: r(t.width), inkAsc: r(t.actualBoundingBoxAscent), inkDesc: r(t.actualBoundingBoxDescent), fontAsc: r(t.fontBoundingBoxAscent), fontDesc: r(t.fontBoundingBoxDescent) };
    };
    const set = (px, fam, chars) => { const o = {}; for (const ch of chars) o[ch] = m(ch, px, fam); return o; };
    return {
      chill16: set(16, 'ChillBitmap', ['国', '逃', '星', 'A', '-', '·', ' ']),
      ark12: set(12, 'ArkPixel', ['国', '逃', '星', 'A', '1', '4', ':', '[', ' ', '-']),
      ark10: set(10, 'ArkPixel10', ['国', '逃', '星', 'A', '1', '4', ':', '[', ' ', '-']),
      ark24: set(24, 'ArkPixel', ['国', '逃', '星']),
    };
  });
  console.log(JSON.stringify(out, null, 2));
  await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
