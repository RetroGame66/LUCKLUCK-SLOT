// 拉丁面候选对照：把每个候选字体接进panel.html，**在真实面板上**出图。
// 与 cmp_latin.cjs 的区别：那个只比 Ark 的两个面，这个比"任意候选"，
// 且每个候选都用它自己的设计字号栅格化（不缩放位图）。
const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');

const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const D    = path.join(ROOT, '_scratch/2026-10-05-fonts');
const ARK12 = path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf');
const ARK10 = path.join(ROOT, 'slot-main/crates/slot-ui/assets/ark-pixel-10px-proportional-zh_hans.otf');
const CHILL = path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf');
const HTML  = path.join(ROOT, 'slot-main/crates/render_palettes/panel.html');
const OUT   = path.join(D, 'shots/latin-candidates-2026-10-05');
const EXE   = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';

// 候选：{ 文件, 字号, 族名, 说明 }
const CANDS = [
  { tag:'ark12',  label:'★ 现状：Ark Pixel 12px @12px',       file:ARK12, px:12, fam:'ArkPixel'  },
  { tag:'ark10',  label:'Ark Pixel 10px @10px（项目已内置）',   file:ARK10, px:10, fam:'ArkPixel10'},
  { tag:'ps2p',   label:'Press Start 2P @8px（OFL-1.1）',     file:path.join(D,'pressstart2p.ttf'), px:8,  fam:'C_PS2P' },
  { tag:'micro5', label:'Micro 5x7 @8px（OFL-1.1）',          file:path.join(D,'micro5x7.ttf'),    px:8,  fam:'C_M5X7' },
];

const PS10 = [[2,234,117],[1,177,88],[0,121,60],[0,65,32]];
const base = { ramp:PS10, title:'夜空中最亮的星', artist:'逃跑计划', transport:'playing',
               mode:'REPEAT', eq:'Rock', elapsed:73.0, length:252.0,
               lines:['能否听清','那仰望的人','心底的孤独和叹息','夜空中最亮的星'],
               face:'lyrics', grid:true };

// 窗口坐标由 dump_region.cjs 的 ASCII 输出量出来（逻辑像素），
// 别凭眼睛估 —— 第一版就估歪了，截到的是空白区。
const WINDOWS = [
  { x:3,   y:24, w:50, h:18, cap:'元信息行 PLAY —— 看 A' },
  { x:150, y:24, w:50, h:18, cap:'元信息行 [EQ] —— 看 R' },
  { x:5,   y:133, w:50, h:16, cap:'走带条 0:00 —— 看 0' },
];
const ZOOM = 6;

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch({ headless: true, executablePath: EXE,
    args: ['--no-sandbox','--disable-gpu','--force-color-profile=srgb'] });
  const page = await browser.newPage({ viewport: { width: 720, height: 480 }, deviceScaleFactor: 1 });
  await page.goto('file://' + HTML);

  // 注入全部候选字体（base64 内嵌，file:// 下外链有 CORS 限制）
  let css =
    `@font-face{font-family:"ArkPixel";src:url(data:font/otf;base64,${fs.readFileSync(ARK12).toString('base64')}) format("opentype");}` +
    `@font-face{font-family:"ArkPixel10";src:url(data:font/otf;base64,${fs.readFileSync(ARK10).toString('base64')}) format("opentype");}` +
    `@font-face{font-family:"ChillBitmap";src:url(data:font/ttf;base64,${fs.readFileSync(CHILL).toString('base64')}) format("truetype");}`;
  const present = CANDS.filter(c => fs.existsSync(c.file));
  for (const c of present) {
    const ext = c.file.toLowerCase().endsWith('.otf') ? 'otf' : 'truetype';
    css += `@font-face{font-family:"${c.fam}";src:url(data:font/${ext};base64,${fs.readFileSync(c.file).toString('base64')}) format("${ext==='otf'?'opentype':'truetype'}");}`;
  }
  await page.addStyleTag({ content: css });
  await page.evaluate(async (fams) => {
    for (const f of fams) { await document.fonts.load(`16px "${f}"`, 'PLAY4:12[Rock]国逃A-'); }
    await document.fonts.load('16px "ChillBitmap"', '夜空中最亮的星逃跑计划');
    await document.fonts.ready;
  }, present.map(c => c.fam));

  const sheet = [];
  for (const c of present) {
    const plan = { latinPx: c.px, latinFont: `"${c.fam}"` };
    const url = await page.evaluate(a => window.renderPanel(a), { ...base, plan });
    fs.writeFileSync(path.join(OUT, `panel-${c.tag}.png`), Buffer.from(url.split(',')[1], 'base64'));
    sheet.push([c.label + `（${c.px}px）`, `panel-${c.tag}.png`]);

    // 放大看纯字形：去掉 LCD 格点，否则 3px 网格会把笔画切出锯齿
    const plain = await page.evaluate(a => window.renderPanel(a), { ...base, plan, grid: false });
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
    }, { dataUrl: plain, windows: WINDOWS, zoom: ZOOM, label: c.label });
    fs.writeFileSync(path.join(OUT, `zoom-${c.tag}.png`), Buffer.from(zoomUrl.split(',')[1], 'base64'));
    sheet.push(['放大 ×6 · 无 LCD 格点 · ' + c.label, `zoom-${c.tag}.png`]);
    console.log('wrote', c.tag);
  }
  await browser.close();

  let h = `<!doctype html><html lang=zh><head><meta charset=utf-8><title>拉丁点阵字体候选对照</title>`
    + `<style>body{background:#2b2b2b;color:#ddd;font:13px/1.6 system-ui,sans-serif;margin:0;padding:18px}`
    + `h1{font-size:18px;margin:0 0 4px}h2{font-size:13px;color:#9a9a9a;margin:0 0 6px;font-weight:400}`
    + `.note{font-size:12px;color:#c8c8c8;background:#232323;border:1px solid #3a3a3a;border-radius:6px;padding:10px 12px;margin:0 0 16px;max-width:900px}`
    + `.grid{display:grid;grid-template-columns:repeat(auto-fill,720px);gap:18px}`
    + `figure{margin:0;background:#1c1c1c;border:1px solid #3a3a3a;border-radius:8px;overflow:hidden;width:720px}`
    + `figcaption{padding:8px 10px;font-size:12px;border-top:1px solid #3a3a3a}`
    + `img{display:block;width:720px;height:auto;image-rendering:pixelated;background:#000}</style></head><body>`;
  h += `<h1>拉丁/数字点阵字体候选对照 · PS10 色板</h1>`;
  h += `<h2>汉字固定 16px 寒蝉点阵体；只有拉丁/数字/符号那一面在换· 每个候选都按它自己的设计字号栅格化，不缩放位图</h2>`;
  h += `<p class=note>筛选判据（fontdue 实测，与设备同一个栅格化器）：覆盖度直方图必须<b>双峰</b>（半调占比 ≤2%），
  否则阈值化后是锯齿不是硬像素。半调占比不达标的字体本就不该进这个面板。</p>`;
  h += '<div class=grid>';
  for (const [cap, file] of sheet) h += `<figure><img src="${file}" alt="${cap}"><figcaption>${cap}</figcaption></figure>`;
  h += '</div></body></html>';
  fs.writeFileSync(path.join(OUT, 'index.html'), h);
  console.log('done -> ' + OUT);
})().catch(e => { console.error(e); process.exit(1); });