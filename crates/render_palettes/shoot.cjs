// 用已安装的 playwright + chromium 把 panel.html 的 LCD 面板渲成 PNG（每个色板/每个面各一张）。
// 字体用真实的 Ark Pixel 12px OTF（base64 内嵌，避免 file:// CORS）。
const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');

const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const FONT = path.join(ROOT, '_scratch/lcdcard/System/fonts/ark-pixel-12px-proportional-zh_hans.otf');
const CHILL = path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf');
const HTML = path.join(ROOT, 'slot-main/crates/render_palettes/panel.html');
const OUT  = path.join(ROOT, '_scratch/shots_mock/pal');

// 8 个 PixelShift 色板（ramp lightest-first: [0]=纸 .. [3]=墨）
const PALETTES = [
  ['PS01', [[196,240,194],[90,185,168],[30,96,110],[45,27,0]]],
  ['PS02', [[0,0,0],[0,0,0],[255,255,255],[255,255,255]]],
  ['PS10', [[2,234,117],[1,177,88],[0,121,60],[0,65,32]]],
  ['PS17', [[253,169,16],[248,136,6],[233,107,8],[131,52,3]]],
  ['PS23', [[255,255,147],[194,222,0],[83,101,0],[0,53,0]]],
  ['PS33', [[232,232,232],[160,160,160],[88,88,88],[16,16,16]]],
  ['PS37', [[5,22,12],[6,45,26],[13,178,84],[27,195,100]]],
  ['PS44', [[217,7,8],[150,7,8],[83,6,7],[7,3,3]]],
];

const TITLE = '夜空中最亮的星';
const ARTIST = '逃跑计划';
const LYRICS = ['夜空中最亮的星','能否听清','那仰望的人','心底的孤独和叹息','夜空中最亮的星'];
const ELAPSED = 73.0, LENGTH = 252.0;

const configs = [];
for (const [name, ramp] of PALETTES) {
  configs.push({ file:`play-${name}.png`, label:`${name} · 播放(Name)`, ramp, face:'name',
                 title:TITLE, artist:ARTIST, transport:'playing', mode:'REPEAT', eq:'Flat', elapsed:ELAPSED, length:LENGTH, lines:LYRICS, grid:true });
  configs.push({ file:`lyric-${name}.png`, label:`${name} · 歌词(Lyrics)`, ramp, face:'lyrics',
                 title:TITLE, artist:ARTIST, transport:'playing', mode:'REPEAT', eq:'Rock', elapsed:ELAPSED, length:LENGTH, lines:LYRICS, grid:true });
}
// PS10 下三种走带状态（演示 transport 图标 + 模式词 + 进度）
const ps10 = PALETTES.find(p=>p[0]==='PS10')[1];
// 同色板无网格字形对照：把 LCD 格点带来的颗粒与字体笔画区分开。
configs.push({ file:'play-PS10-nogrid.png', label:'PS10 · 播放（无网格字形对照）', ramp:ps10, face:'name',
               title:TITLE, artist:ARTIST, transport:'playing', mode:'REPEAT', eq:'Flat', elapsed:ELAPSED, length:LENGTH, lines:LYRICS, grid:false });
configs.push({ file:'lyric-PS10-nogrid.png', label:'PS10 · 歌词（无网格字形对照）', ramp:ps10, face:'lyrics',
               title:TITLE, artist:ARTIST, transport:'playing', mode:'REPEAT', eq:'Rock', elapsed:ELAPSED, length:LENGTH, lines:LYRICS, grid:false });
for (const [suffix, transport, mode] of [['paused','paused','SHUFFLE'],['stopped','stopped','ONE']]) {
  configs.push({ file:`bar-PS10-${suffix}.png`, label:`PS10 · 走带 ${suffix}`, ramp:ps10, face:'name',
                 title:TITLE, artist:ARTIST, transport, mode, eq:'Flat', elapsed:ELAPSED, length:LENGTH, lines:LYRICS, grid:true });
}

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const b64 = fs.readFileSync(FONT).toString('base64');
  const browser = await chromium.launch({
    headless: true,
    executablePath: 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe',
    args: ['--no-sandbox','--disable-gpu','--force-color-profile=srgb']
  });
  const page = await browser.newPage({ viewport: { width: 720, height: 480 }, deviceScaleFactor: 1 });
  await page.goto('file://' + HTML);
  const chill64 = fs.readFileSync(CHILL).toString('base64');
  await page.addStyleTag({ content:
    `@font-face{font-family:"ArkPixel";src:url(data:font/otf;base64,${b64}) format("opentype");}` +
    `@font-face{font-family:"ChillBitmap";src:url(data:font/ttf;base64,${chill64}) format("truetype");}` });
  await page.evaluate(async () => {
    await document.fonts.load('12px "ArkPixel"', 'PLAY1234[]:');
    await document.fonts.load('16px "ChillBitmap"', '夜空中最亮的星逃跑计划');
    await document.fonts.ready;
  });

  const sheet = [];
  for (const cfg of configs) {
    const url = await page.evaluate(c => window.renderPanel(c), cfg);
    const b = Buffer.from(url.split(',')[1], 'base64');
    fs.writeFileSync(path.join(OUT, cfg.file), b);
    sheet.push([cfg.label, cfg.file]);
    console.log('wrote', cfg.file);
  }
  await browser.close();

  // 联系表
  let h = `<!doctype html><html lang=zh><head><meta charset=utf-8><title>播放面板 · 色板示例</title>`
    + `<style>body{background:#2b2b2b;color:#ddd;font:13px/1.4 system-ui,sans-serif;margin:0;padding:18px}`
    + `h1{font-size:18px;margin:0 0 4px}h2{font-size:13px;color:#9a9a9a;margin:0 0 18px}`
    + `.grid{display:grid;grid-template-columns:repeat(auto-fill,720px);gap:18px}`
    + `figure{margin:0;background:#1c1c1c;border:1px solid #3a3a3a;border-radius:8px;overflow:hidden;width:720px}`
    + `figcaption{padding:8px 10px;font-size:12px;border-top:1px solid #3a3a3a}`
    + `img{display:block;width:720px;height:480px;image-rendering:pixelated;background:#000}`
    + `.legend{display:flex;flex-wrap:wrap;gap:10px;margin:0 0 18px}`
    + `.chip{display:flex;align-items:center;gap:6px;background:#1c1c1c;border:1px solid #3a3a3a;border-radius:6px;padding:4px 8px}`
    + `.sw{display:flex}.sw i{width:16px;height:16px;display:block}</style></head><body>`;
  h += `<h1>复古 Game Boy LCD 音乐播放面板 · 色板示例</h1>`;
  h += `<h2>汉字 16px 寒蝉点阵体 / 拉丁·数字·符号 12px Ark Pixel，逐字形按设计字号栅格化 · 歌名行「歌手名 - 歌名」 · 走带状态为单词 · 720×480 · 原始1:1像素显示</h2>`;
  h += '<div class=legend>';
  for (const [name, ramp] of PALETTES) {
    h += `<div class=chip>${name}<span class=sw>`;
    for (const s of ramp) h += `<i style="background:rgb(${s[0]},${s[1]},${s[2]})"></i>`;
    h += '</span></div>';
  }
  h += '</div><div class=grid>';
  for (const [cap, file] of sheet) h += `<figure><img src="${file}" alt="${cap}"><figcaption>${cap}</figcaption></figure>`;
  h += '</div></body></html>';
  fs.writeFileSync(path.join(OUT, 'index.html'), h);
  console.log('done: ' + sheet.length + ' PNGs + index.html -> ' + OUT);
})().catch(e => { console.error(e); process.exit(1); });
