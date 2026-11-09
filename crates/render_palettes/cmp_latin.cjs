// 拉丁面 12px（Ark Pixel 12px）vs 10px（Ark Pixel 10px，项目已随包内置的那一面）对照。
// 出：整面板 ×2 + 关键区域放大 ×5（PLAY 的 A、走带时长的 4）。
const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');

const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const ARK12 = path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf');
const ARK10 = path.join(ROOT, 'slot-main/crates/slot-ui/assets/ark-pixel-10px-proportional-zh_hans.otf');
const CHILL = path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf');
const HTML  = path.join(ROOT, 'slot-main/crates/render_palettes/panel.html');
const OUT   = path.join(ROOT, '_scratch/shots_mock/latin-size-2026-10-05');
const EXE   = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';

const PS10 = [[2,234,117],[1,177,88],[0,121,60],[0,65,32]];
const TITLE = '夜空中最亮的星', ARTIST = '逃跑计划';
const L4 = ['能否听清','那仰望的人','心底的孤独和叹息','夜空中最亮的星'];
const ELAPSED = 73.0, LENGTH = 252.0;
const base = { ramp:PS10, title:TITLE, artist:ARTIST, transport:'playing', mode:'REPEAT', eq:'Rock',
               elapsed:ELAPSED, length:LENGTH, lines:L4, face:'lyrics', grid:true };

const SIZES = [
  { tag:'latin12', label:'拉丁面 = Ark Pixel 12px（@12px，em 框 16）', plan:{ latinPx:12, latinFont:'"ArkPixel"' } },
  { tag:'latin10', label:'拉丁面 = Ark Pixel 10px（@10px，em 框 14）', plan:{ latinPx:10, latinFont:'"ArkPixel10"' } },
];
// 放大窗口（逻辑像素坐标）
const WINDOWS = [
  { x:4,   y:15, w:48, h:27, cap:'状态词 PLAY —— 看 A' },
  { x:186, y:126, w:48, h:30, cap:'走带时长 4:12 —— 看 4' },
  { x:150, y:15, w:48, h:27, cap:'EQ 标签 [Rock]' },
];
const ZOOM = 6;

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch({ headless: true, executablePath: EXE,
    args: ['--no-sandbox','--disable-gpu','--force-color-profile=srgb'] });
  const page = await browser.newPage({ viewport: { width: 720, height: 480 }, deviceScaleFactor: 1 });
  await page.goto('file://' + HTML);
  await page.addStyleTag({ content:
    `@font-face{font-family:"ArkPixel";src:url(data:font/otf;base64,${fs.readFileSync(ARK12).toString('base64')}) format("opentype");}` +
    `@font-face{font-family:"ArkPixel10";src:url(data:font/otf;base64,${fs.readFileSync(ARK10).toString('base64')}) format("opentype");}` +
    `@font-face{font-family:"ChillBitmap";src:url(data:font/ttf;base64,${fs.readFileSync(CHILL).toString('base64')}) format("truetype");}` });
  await page.evaluate(async () => {
    await document.fonts.load('12px "ArkPixel"', 'PLAY4:12[REPEAT]');
    await document.fonts.load('10px "ArkPixel10"', 'PLAY4:12[REPEAT]');
    await document.fonts.load('16px "ChillBitmap"', '夜空中最亮的星逃跑计划');
    await document.fonts.ready;
  });

  const sheet = [];
  for (const s of SIZES) {
    const url = await page.evaluate(c => window.renderPanel(c), { ...base, plan: s.plan });
    fs.writeFileSync(path.join(OUT, `panel-${s.tag}.png`), Buffer.from(url.split(',')[1], 'base64'));
    sheet.push([s.label, `panel-${s.tag}.png`]);

    // 放大看纯字形：去掉 LCD 格点（grid:false），否则格点会把笔画切出锯齿
    const plain = await page.evaluate(c => window.renderPanel(c), { ...base, plan: s.plan, grid: false });
    const zoomUrl = await page.evaluate(async (args) => {
      const img = new Image(); img.src = args.dataUrl; await img.decode();
      const Z = args.zoom, cv = document.createElement('canvas');
      const rowH = Math.max(...args.windows.map(w => w.h * Z)) + 40;
      cv.width = 720; cv.height = 34 + rowH;
      const c = cv.getContext('2d'); c.imageSmoothingEnabled = false;
      c.fillStyle = '#efeee9'; c.fillRect(0, 0, cv.width, cv.height);
      c.fillStyle = '#111'; c.font = 'bold 15px system-ui,sans-serif';
      c.fillText(args.label, 14, 22);
      args.windows.forEach((w, i) => {
        const x = 14 + i * 236, y = 34;
        c.drawImage(img, w.x*3, w.y*3, w.w*3, w.h*3, x, y, w.w*Z, w.h*Z);
        c.strokeStyle = '#b9b6ad'; c.lineWidth = 1; c.strokeRect(x+.5, y+.5, w.w*Z-1, w.h*Z-1);
        c.fillStyle = '#444'; c.font = '12px system-ui,sans-serif';
        c.fillText(w.cap, x, y + w.h*Z + 15);
      });
      return cv.toDataURL('image/png');
    }, { dataUrl: plain, windows: WINDOWS, zoom: ZOOM, label: s.label });
    fs.writeFileSync(path.join(OUT, `zoom-${s.tag}.png`), Buffer.from(zoomUrl.split(',')[1], 'base64'));
    sheet.push(['放大 ×6 · 无 LCD 格点 · ' + s.label, `zoom-${s.tag}.png`]);
    console.log('wrote', s.tag);
  }
  await browser.close();

  let h = `<!doctype html><html lang=zh><head><meta charset=utf-8><title>拉丁面 12px vs 10px</title>`
    + `<style>body{background:#2b2b2b;color:#ddd;font:13px/1.5 system-ui,sans-serif;margin:0;padding:18px}`
    + `h1{font-size:18px;margin:0 0 4px}h2{font-size:13px;color:#9a9a9a;margin:0 0 16px;font-weight:400}`
    + `.grid{display:grid;grid-template-columns:repeat(auto-fill,720px);gap:18px}`
    + `figure{margin:0;background:#1c1c1c;border:1px solid #3a3a3a;border-radius:8px;overflow:hidden;width:720px}`
    + `figcaption{padding:8px 10px;font-size:12px;border-top:1px solid #3a3a3a}`
    + `img{display:block;width:720px;height:auto;image-rendering:pixelated;background:#000}</style></head><body>`;
  h += `<h1>拉丁面 12px vs 10px · PS10</h1>`;
  h += `<h2>汉字固定 16px 寒蝉；只换拉丁/数字/符号那一面 · 逐字形按设计字号栅格化 · 原始 1:1 像素显示</h2>`;
  h += '<div class=grid>';
  for (const [cap, file] of sheet) h += `<figure><img src="${file}" alt="${cap}"><figcaption>${cap}</figcaption></figure>`;
  h += '</div></body></html>';
  fs.writeFileSync(path.join(OUT, 'index.html'), h);
  console.log('done -> ' + OUT);
})().catch(e => { console.error(e); process.exit(1); });
