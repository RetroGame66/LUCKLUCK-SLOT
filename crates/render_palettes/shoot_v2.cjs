// 新版播放面板（CJK 16px 寒蝉 / 拉丁 12px Ark；歌名行合并；走带状态改单词；
// 走带条文字在条顶与进度条之间居中）在 PS10 色板下的样张，附几种歌词行距比较。
const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');

const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const ARK   = path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf');
const CHILL = path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf');
const HTML  = path.join(ROOT, 'slot-main/crates/render_palettes/panel.html');
const OUT   = path.join(ROOT, '_scratch/shots_mock/panel-v2-2026-10-05');
const EXE   = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';

const PS10 = [[2,234,117],[1,177,88],[0,121,60],[0,65,32]];
const TITLE = '夜空中最亮的星', ARTIST = '逃跑计划';
const L5 = ['夜空中最亮的星','能否听清','那仰望的人','心底的孤独和叹息','夜空中最亮的星'];
const L4 = ['能否听清','那仰望的人','心底的孤独和叹息','夜空中最亮的星'];
const ELAPSED = 73.0, LENGTH = 252.0;

const base = { ramp:PS10, title:TITLE, artist:ARTIST, transport:'playing', mode:'REPEAT', eq:'Rock',
               elapsed:ELAPSED, length:LENGTH, grid:true };

const configs = [
  { file:'v2-lyrics-4x21.png', label:'歌词 · 4 行 / 行距 21（默认）', ...base, face:'lyrics', lines:L4 },
  { file:'v2-lyrics-5x17.png', label:'歌词 · 5 行 / 行距 17（对照：塞得下但很紧）', ...base, face:'lyrics', lines:L5, layout:{lyricStep:17} },
  { file:'v2-name.png', label:'频谱(Name) 面', ...base, face:'name', eq:'Flat' },
  { file:'v2-paused.png', label:'走带状态 = PAUSE', ...base, face:'lyrics', lines:L4, transport:'paused', mode:'SHUFFLE' },
  { file:'v2-stopped.png', label:'走带状态 = STOP', ...base, face:'name', lines:L4, transport:'stopped', mode:'ONE', eq:'Flat' },
  // 边界：歌手+歌名过长时，歌名行裁到内容区（不会压到边框），真实播放器需要跑马灯或截断
  { file:'v2-longtitle.png', label:'边界 · 超长「歌手名 - 歌名」（超出部分裁切）', ...base, face:'lyrics', lines:L4,
    artist:'五月天', title:'我不愿让你一个人（诺亚方舟 Live 版）' },
];

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch({ headless: true, executablePath: EXE,
    args: ['--no-sandbox','--disable-gpu','--force-color-profile=srgb'] });
  const page = await browser.newPage({ viewport: { width: 720, height: 480 }, deviceScaleFactor: 1 });
  await page.goto('file://' + HTML);
  await page.addStyleTag({ content:
    `@font-face{font-family:"ArkPixel";src:url(data:font/otf;base64,${fs.readFileSync(ARK).toString('base64')}) format("opentype");}` +
    `@font-face{font-family:"ChillBitmap";src:url(data:font/ttf;base64,${fs.readFileSync(CHILL).toString('base64')}) format("truetype");}` });
  await page.evaluate(async () => {
    await document.fonts.load('12px "ArkPixel"', 'PLAY1234[]:');
    await document.fonts.load('16px "ChillBitmap"', '夜空中最亮的星逃跑计划');
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

  // 旧版基准图搬过来方便并排
  const oldRef = path.join(ROOT, '_scratch/shots_mock/font-swap-2026-10-05/ps10-ark-current.png');
  if (fs.existsSync(oldRef)) fs.copyFileSync(oldRef, path.join(OUT, 'ref-old-design.png'));

  let h = `<!doctype html><html lang=zh><head><meta charset=utf-8><title>播放面板 v2 · PS10</title>`
    + `<style>body{background:#2b2b2b;color:#ddd;font:13px/1.5 system-ui,sans-serif;margin:0;padding:18px}`
    + `h1{font-size:18px;margin:0 0 4px}h2{font-size:13px;color:#9a9a9a;margin:0 0 16px;font-weight:400}`
    + `.grid{display:grid;grid-template-columns:repeat(auto-fill,720px);gap:18px}`
    + `figure{margin:0;background:#1c1c1c;border:1px solid #3a3a3a;border-radius:8px;overflow:hidden;width:720px}`
    + `figcaption{padding:8px 10px;font-size:12px;border-top:1px solid #3a3a3a}`
    + `img{display:block;width:720px;height:480px;image-rendering:pixelated;background:#000}</style></head><body>`;
  h += `<h1>播放面板 v2 · PS10</h1>`;
  h += `<h2>汉字 16px 寒蝉点阵体 / 拉丁·数字·符号 12px Ark Pixel，逐字形按设计字号栅格化 · ×3 最近邻 · 原始 1:1 像素显示</h2>`;
  h += '<div class=grid>';
  if (fs.existsSync(path.join(OUT, 'ref-old-design.png')))
    h += `<figure><img src="ref-old-design.png" alt="旧版"><figcaption>参照 · 旧版（标题 Ark 24px / 走带图标 / 独立歌手行）</figcaption></figure>`;
  for (const [cap, file] of sheet) h += `<figure><img src="${file}" alt="${cap}"><figcaption>${cap}</figcaption></figure>`;
  h += '</div></body></html>';
  fs.writeFileSync(path.join(OUT, 'index.html'), h);
  console.log('done -> ' + OUT);
})().catch(e => { console.error(e); process.exit(1); });
