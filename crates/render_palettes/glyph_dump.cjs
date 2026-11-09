// 把 'A' / '4' / 'P' 的覆盖率矩阵按 ASCII 打出来，判断字形是「本来就长这样」还是被阈值切坏了。
const fs = require('fs'), path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');
const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const ARK12 = path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf');
const ARK10 = path.join(ROOT, 'slot-main/crates/slot-ui/assets/ark-pixel-10px-proportional-zh_hans.otf');
const EXE = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: EXE, args: ['--no-sandbox', '--disable-gpu'] });
  const page = await browser.newPage();
  await page.setContent('<canvas id=c width=8 height=8></canvas>');
  await page.addStyleTag({ content:
    `@font-face{font-family:"ArkPixel";src:url(data:font/otf;base64,${fs.readFileSync(ARK12).toString('base64')}) format("opentype");}` +
    `@font-face{font-family:"ArkPixel10";src:url(data:font/otf;base64,${fs.readFileSync(ARK10).toString('base64')}) format("opentype");}` });
  await page.evaluate(async () => {
    await document.fonts.load('12px "ArkPixel"', 'A4P');
    await document.fonts.load('24px "ArkPixel"', 'A4P');
    await document.fonts.load('10px "ArkPixel10"', 'A4P');
    await document.fonts.ready;
  });
  const dump = await page.evaluate(() => {
    const c = document.getElementById('c').getContext('2d');
    function grid(ch, px, fam) {
      const pad = px * 2 + 4, w = px * 4 + pad * 2, h = px + 8;
      const cv = document.createElement('canvas'); cv.width = w; cv.height = h;
      const x = cv.getContext('2d');
      x.font = `${px}px "${fam}"`; x.textBaseline = 'top'; x.fillStyle = '#000';
      x.clearRect(0, 0, w, h); x.fillText(ch, pad, 1);
      const d = x.getImageData(0, 0, w, h).data;
      const rows = [];
      for (let y = 0; y < h; y++) {
        let row = '';
        for (let xx = 0; xx < w; xx++) {
          const a = d[(y * w + xx) * 4 + 3];
          row += a === 0 ? '.' : a < 64 ? ':' : a < 128 ? '+' : a < 192 ? '*' : a < 255 ? '#' : '@';
        }
        rows.push(row.replace(/\.+$/, ''));
      }
      // 去掉整行空白
      const first = rows.findIndex(r => r.length), last = rows.length - 1 - [...rows].reverse().findIndex(r => r.length);
      return { ch, px, fam, rows: rows.slice(first, last + 1) };
    }
    const out = [];
    for (const ch of ['A', '4']) {
      out.push(grid(ch, 12, 'ArkPixel'));
      out.push(grid(ch, 24, 'ArkPixel'));
      out.push(grid(ch, 10, 'ArkPixel10'));
    }
    return out;
  });
  const legend = "图例: '.'=0  ':'=<64  '+'=<128  '*'=<192  '#'=<255  '@'=255";
  for (const g of dump) {
    console.log(`\n=== '${g.ch}' @ ${g.px}px ${g.fam} ===   ${legend}`);
    g.rows.forEach(r => console.log(r));
  }
  await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
