// 把面板里某个逻辑区域按最终像素（含 ×3 放大后的取色）打出来，确认字形实际长什么样。
const fs = require('fs'), path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');
const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const ARK12 = path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf');
const ARK10 = path.join(ROOT, 'slot-main/crates/slot-ui/assets/ark-pixel-10px-proportional-zh_hans.otf');
const CHILL = path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf');
const HTML = path.join(ROOT, 'slot-main/crates/render_palettes/panel.html');
const EXE = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';
const PS10 = [[2,234,117],[1,177,88],[0,121,60],[0,65,32]];
const base = { ramp:PS10, title:'夜空中最亮的星', artist:'逃跑计划', transport:'playing', mode:'REPEAT', eq:'Rock',
               elapsed:73.0, length:252.0, lines:['能否听清','那仰望的人','心底的孤独和叹息','夜空中最亮的星'], face:'lyrics', grid:false };

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: EXE, args: ['--no-sandbox', '--disable-gpu'] });
  const page = await browser.newPage({ viewport: { width: 720, height: 480 }, deviceScaleFactor: 1 });
  await page.goto('file://' + HTML);
  await page.addStyleTag({ content:
    `@font-face{font-family:"ArkPixel";src:url(data:font/otf;base64,${fs.readFileSync(ARK12).toString('base64')}) format("opentype");}` +
    `@font-face{font-family:"ArkPixel10";src:url(data:font/otf;base64,${fs.readFileSync(ARK10).toString('base64')}) format("opentype");}` +
    `@font-face{font-family:"ChillBitmap";src:url(data:font/ttf;base64,${fs.readFileSync(CHILL).toString('base64')}) format("truetype");}` });
  await page.evaluate(async () => {
    await document.fonts.load('12px "ArkPixel"', 'PLAY4:12');
    await document.fonts.load('10px "ArkPixel10"', 'PLAY4:12');
    await document.fonts.load('16px "ChillBitmap"', '夜空');
    await document.fonts.ready;
  });
  for (const plan of [{ tag:'12px', latinPx:12, latinFont:'"ArkPixel"' }, { tag:'10px', latinPx:10, latinFont:'"ArkPixel10"' }]) {
    const url = await page.evaluate(c => window.renderPanel(c), { ...base, plan });
    const ascii = await page.evaluate(async (args) => {
      const img = new Image(); img.src = args.dataUrl; await img.decode();
      const cv = document.createElement('canvas'); cv.width = 720; cv.height = 480;
      const c = cv.getContext('2d'); c.drawImage(img, 0, 0);
      const d = c.getImageData(0, 0, 720, 480).data;
      const ramp = args.ramp, CH = [' ', '~', '-', '#'];
      function near(r, g, b) { let bi = 0, bd = 1e9; ramp.forEach((s, i) => { const dd = (s[0]-r)**2 + (s[1]-g)**2 + (s[2]-b)**2; if (dd < bd) { bd = dd; bi = i; } }); return bi; }
      const lines = [];
      for (let y = args.y0; y < args.y1; y++) {
        let s = String(y).padStart(3, ' ') + ' ';
        for (let x = args.x0; x < args.x1; x++) {
          const px = (y * 3 + 1) * 720 + (x * 3 + 1);
          s += CH[near(d[px*4], d[px*4+1], d[px*4+2])];
        }
        lines.push(s);
      }
      return lines.join('\n');
    }, { dataUrl: url, ramp: PS10, x0: 4, x1: 60, y0: 14, y1: 44 });
    console.log(`\n=== 歌名行底 + 元信息行 · 拉丁 ${plan.tag}（' '=纸 '~'=边 '-'=规 '#'=墨）===\n${ascii}`);
    const strip = await page.evaluate(async (args) => {
      const img = new Image(); img.src = args.dataUrl; await img.decode();
      const cv = document.createElement('canvas'); cv.width = 720; cv.height = 480;
      const c = cv.getContext('2d'); c.drawImage(img, 0, 0);
      const d = c.getImageData(0, 0, 720, 480).data;
      const ramp = args.ramp, CH = [' ', '~', '-', '#'];
      function near(r, g, b) { let bi = 0, bd = 1e9; ramp.forEach((s, i) => { const dd = (s[0]-r)**2 + (s[1]-g)**2 + (s[2]-b)**2; if (dd < bd) { bd = dd; bi = i; } }); return bi; }
      const lines = [];
      for (let y = 126; y < 158; y++) {
        let s = String(y).padStart(3, ' ') + ' ';
        for (let x = 4; x < 60; x++) {
          const px = (y * 3 + 1) * 720 + (x * 3 + 1);
          s += CH[near(d[px*4], d[px*4+1], d[px*4+2])];
        }
        lines.push(s);
      }
      return lines.join('\n');
    }, { dataUrl: url, ramp: PS10 });
    console.log(`=== 走带条左端 · 拉丁 ${plan.tag} ===\n${strip}`);
  }
  await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
