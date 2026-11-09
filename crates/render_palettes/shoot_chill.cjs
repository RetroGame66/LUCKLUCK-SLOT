// 播放器面板「换字体」效果样张：以 PS10 一个色板为准，比较几种字体接入方式。
// 三种字体面同时注入：ArkPixel(12px 原生)、ChillBitmap(16px 原生)。逐字形按各自
// 设计字号栅格化后，以 ×3 最近邻放大——和 slot_ui::lcd::render_scaled 一致。
const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');

const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const ARK   = path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf');
const CHILL = path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf');
const HTML  = path.join(ROOT, 'slot-main/crates/render_palettes/panel.html');
const OUT   = path.join(ROOT, '_scratch/shots_mock/font-swap-2026-10-05');
const EXE   = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';

const PS10 = [[2,234,117],[1,177,88],[0,121,60],[0,65,32]];
const TITLE = '夜空中最亮的星';
const ARTIST = '逃跑计划';
const L5 = ['夜空中最亮的星','能否听清','那仰望的人','心底的孤独和叹息','夜空中最亮的星'];
const L3 = ['夜空中最亮的星','那仰望的人','心底的孤独和叹息'];
const ELAPSED = 73.0, LENGTH = 252.0;

const ARK_F = '"ArkPixel"', CHILL_F = '"ChillBitmap"';

const configs = [
  // 基准：项目当前排版（Ark 24px 标题 + Ark 12px 正文）
  { file:'ps10-ark-current.png', label:'① 现状 · Ark Pixel（标题 24px / 正文 12px）',
    ramp:PS10, face:'lyrics', title:TITLE, artist:ARTIST, transport:'playing', mode:'REPEAT', eq:'Rock',
    elapsed:ELAPSED, length:LENGTH, lines:L5, grid:true },
  // 推荐：只把播放器大标题换成寒蝉 16px，其余小字保持 Ark 12px 原生
  { file:'ps10-chill-title16.png', label:'② 推荐 · 标题寒蝉 16px，正文保持 Ark 12px',
    ramp:PS10, face:'lyrics', title:TITLE, titleFont:CHILL_F, titlePx:16,
    artist:ARTIST, transport:'playing', mode:'REPEAT', eq:'Rock',
    elapsed:ELAPSED, length:LENGTH, lines:L5, grid:true },
  // 全量：标题 + 正文全部寒蝉 16px（歌词改为 3 行以容纳 16px 行高）
  { file:'ps10-chill-all16.png', label:'③ 全量 · 标题与正文都用寒蝉 16px（歌词 3 行）',
    ramp:PS10, face:'lyrics', title:TITLE, titleFont:CHILL_F, titlePx:16,
    bodyFont:CHILL_F, bodyPx:16, lineStep:18,
    artist:ARTIST, transport:'playing', mode:'REPEAT', eq:'Rock',
    elapsed:ELAPSED, length:LENGTH, lines:L3, grid:true },
  // 推荐方案的 Name(频谱) 面，确认非歌词页同样成立
  { file:'ps10-chill-title16-name.png', label:'④ 推荐 · 频谱(Name) 面 · 标题寒蝉 16px',
    ramp:PS10, face:'name', title:TITLE, titleFont:CHILL_F, titlePx:16,
    artist:ARTIST, transport:'playing', mode:'REPEAT', eq:'Flat',
    elapsed:ELAPSED, length:LENGTH, lines:L5, grid:true },
];

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const ark64 = fs.readFileSync(ARK).toString('base64');
  const chill64 = fs.readFileSync(CHILL).toString('base64');
  const browser = await chromium.launch({
    headless: true, executablePath: EXE,
    args: ['--no-sandbox','--disable-gpu','--force-color-profile=srgb']
  });
  const page = await browser.newPage({ viewport: { width: 720, height: 480 }, deviceScaleFactor: 1 });
  await page.goto('file://' + HTML);
  await page.addStyleTag({ content:
    `@font-face{font-family:"ArkPixel";src:url(data:font/otf;base64,${ark64}) format("opentype");}` +
    `@font-face{font-family:"ChillBitmap";src:url(data:font/ttf;base64,${chill64}) format("truetype");}`
  });
  await page.evaluate(async () => {
    await document.fonts.load('24px "ArkPixel"', '夜空测试');
    await document.fonts.load('12px "ArkPixel"', '夜空测试');
    await document.fonts.load('16px "ChillBitmap"', '夜空中最亮的星');
    await document.fonts.ready;
  });

  const sheet = [];
  for (const cfg of configs) {
    const url = await page.evaluate(c => window.renderPanel(c), cfg);
    fs.writeFileSync(path.join(OUT, cfg.file), Buffer.from(url.split(',')[1], 'base64'));
    sheet.push([cfg.label, cfg.file]);
    console.log('wrote', cfg.file);
  }
  await browser.close();

  let h = `<!doctype html><html lang=zh><head><meta charset=utf-8><title>播放面板 · 换字体效果（PS10）</title>`
    + `<style>body{background:#2b2b2b;color:#ddd;font:13px/1.5 system-ui,sans-serif;margin:0;padding:18px}`
    + `h1{font-size:18px;margin:0 0 4px}h2{font-size:13px;color:#9a9a9a;margin:0 0 16px;font-weight:400}`
    + `.grid{display:grid;grid-template-columns:repeat(auto-fill,720px);gap:18px}`
    + `figure{margin:0;background:#1c1c1c;border:1px solid #3a3a3a;border-radius:8px;overflow:hidden;width:720px}`
    + `figcaption{padding:8px 10px;font-size:12px;border-top:1px solid #3a3a3a}`
    + `img{display:block;width:720px;height:480px;image-rendering:pixelated;background:#000}</style></head><body>`;
  h += `<h1>播放面板换字体效果 · PS10（绿色 Game Boy 色板）</h1>`;
  h += `<h2>全部按各自设计字号逐字形栅格化（Ark 12/24px、寒蝉 16px），×3 最近邻放大，含像素栅格 · 原始 1:1 像素显示</h2>`;
  h += '<div class=grid>';
  for (const [cap, file] of sheet) h += `<figure><img src="${file}" alt="${cap}"><figcaption>${cap}</figcaption></figure>`;
  h += '</div></body></html>';
  fs.writeFileSync(path.join(OUT, 'index.html'), h);
  console.log('done -> ' + OUT);
})().catch(e => { console.error(e); process.exit(1); });
