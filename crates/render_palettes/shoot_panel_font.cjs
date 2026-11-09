// 播放面板换 CJK 面：**寒蝉 16px vs GNU Unifont 16px** 并排。
//
// 用 `panel.html`（几何对齐 `lcd.rs` 的那个渲染器）——它本来就支持 `cfg.plan` 覆盖字体，
// 所以不用改渲染器。两张面各出「频谱」与「歌词」两态。
//
// ⚠️ 换面的前提已实测：两者在 16px 的 `line_h` **都是 16**（`font_size_audit`），
//    所以**纵向版式一个数都不用动**，换的只是字形。
const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');

const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const ARK      = path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf');
const CHILL    = path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf');
const UNIFONT  = path.join(ROOT, '_scratch/pxfonts-2026-10-07/unifont.otf');
const HTML     = path.join(ROOT, 'slot-main/crates/render_palettes/panel.html');
const OUT      = path.join(ROOT, '_scratch/shots_panel_font');
const EXE      = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';

const face = (fam, file, fmt) =>
  `@font-face{font-family:"${fam}";src:url(data:font/${fmt};base64,${fs.readFileSync(file).toString('base64')}) format("${fmt === 'otf' ? 'opentype' : 'truetype'}");}`;

const PS10 = [[2, 234, 117], [1, 177, 88], [0, 121, 60], [0, 65, 32]];
const TITLE = '夜航西飞', ARTIST = '柏桦';
const L4 = ['离开站台的那一刻', '灯火被揉成橘色', '车窗外是完整的黑', '字也跟着晃了一下'];

// 两种"面"：**不是改字体名，而是把面板读的那个文件换掉** ——
// 真实换字体就是换 `Music/Fonts/chill-bitmap-16px.ttf` 这个文件，
// 所以这里把 Unifont 的字节挂在**同一个 family 名**下，等于"卡上那个文件是 Unifont"。
// （⚠️ headless 里给 Unifont 另起一个 family 名再靠 `cfg.plan` 覆盖，实测**不生效** ——
//   四路对照：ChillBitmap=65、Unifont=65（同一个）、不存在的名字/monospace/serif=73。
//   换成"换字节"这条路之后一切正常，而且它才是真实场景。）
const SHEETS = [
  { tag: 'chill',   cjkFile: CHILL,   fmt: 'ttf' },
  { tag: 'unifont', cjkFile: UNIFONT, fmt: 'otf' },
];

const base = { ramp: PS10, title: TITLE, artist: ARTIST, transport: 'playing', mode: 'REPEAT',
               eq: 'Rock', elapsed: 73.0, length: 252.0, grid: true, lines: L4 };

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch({ headless: true, executablePath: EXE,
    args: ['--no-sandbox', '--disable-gpu', '--force-color-profile=srgb'] });
  const sheet = [];
  for (const s of SHEETS) {
    const page = await browser.newPage({ viewport: { width: 720, height: 480 }, deviceScaleFactor: 1 });
    page.on('pageerror', e => console.error('PAGE EXCEPTION:', e.message));
    await page.goto('file://' + HTML);
    await page.addStyleTag({ content:
      face('ArkPixel', ARK, 'otf') +
      // ★ CJK 面挂在 `ChillBitmap` 这个名字下 —— 面板就是按这个名字取字的。
      face('ChillBitmap', s.cjkFile, s.fmt) });
    await page.evaluate(async () => {
      await document.fonts.load('16px "ChillBitmap"', '夜航西飞柏桦离开站台');
      await document.fonts.load('12px "ArkPixel"', '0:37[]REPEATRock');
      await document.fonts.ready;
    });
    for (const cfg of [
      { file: `p-${s.tag}-name.png`,   label: `${s.tag} · 频谱面`, ...base, face: 'name' },
      { file: `p-${s.tag}-lyrics.png`, label: `${s.tag} · 歌词面`, ...base, face: 'lyrics' },
    ]) {
      const url = await page.evaluate(c => window.renderPanel(c), cfg);
      fs.writeFileSync(path.join(OUT, cfg.file), Buffer.from(url.split(',')[1], 'base64'));
      sheet.push([cfg.label, cfg.file]);
      console.log('wrote', cfg.file);
    }
    await page.close();
  }
  await browser.close();

  let h = `<!doctype html><html lang=zh><head><meta charset=utf-8><title>播放面板 · 换 CJK 面</title>`
    + `<style>body{background:#2b2b2b;color:#ddd;font:13px/1.5 system-ui,sans-serif;margin:0;padding:18px}`
    + `h1{font-size:18px;margin:0 0 4px}h2{font-size:13px;color:#9a9a9a;margin:0 0 16px;font-weight:400}`
    + `.grid{display:grid;grid-template-columns:repeat(auto-fill,720px);gap:18px}`
    + `figure{margin:0;background:#1c1c1c;border:1px solid #3a3a3a;border-radius:8px;overflow:hidden;width:720px}`
    + `figcaption{padding:8px 10px;font-size:12px;border-top:1px solid #3a3a3a}`
    + `img{display:block;width:720px;height:480px;image-rendering:pixelated;background:#000}</style></head><body>`;
  h += `<h1>播放面板 · CJK 面 寒蝉 16px ↔ GNU Unifont 16px</h1>`;
  h += `<h2>两者 @16px 的 line_h 都是 16 ⇒ <b>纵向版式一个数都不用动</b>，换的只是字形</h2>`;
  h += '<div class=grid>';
  for (const [cap, file] of sheet) h += `<figure><img src="${file}" alt="${cap}"><figcaption>${cap}</figcaption></figure>`;
  h += '</div></body></html>';
  fs.writeFileSync(path.join(OUT, 'index.html'), h);
  console.log('done -> ' + OUT);
})().catch(e => { console.error(e); process.exit(1); });
