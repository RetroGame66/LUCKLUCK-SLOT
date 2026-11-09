// 探针：`panel.html` 里 `cfg.plan` 换 CJK 面到底有没有生效。
const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');

const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const HTML = path.join(ROOT, 'slot-main/crates/render_palettes/panel.html');
const EXE = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';

const face = (fam, file, fmt) =>
  `@font-face{font-family:"${fam}";src:url(data:font/${fmt};base64,${fs.readFileSync(file).toString('base64')}) format("${fmt === 'otf' ? 'opentype' : 'truetype'}");}`;

(async () => {
  const b = await chromium.launch({ headless: true, executablePath: EXE, args: ['--no-sandbox', '--disable-gpu'] });
  const p = await b.newPage({ viewport: { width: 720, height: 480 } });
  await p.goto('file://' + HTML);
  await p.addStyleTag({ content:
    face('ChillBitmap', path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf'), 'ttf') +
    face('Unifont', path.join(ROOT, '_scratch/pxfonts-2026-10-07/unifont.otf'), 'otf') +
    face('ArkPixel', path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf'), 'otf') });
  const r = await p.evaluate(async () => {
    await document.fonts.load('16px "Unifont"', '汉');
    await document.fonts.load('16px "ChillBitmap"', '汉');
    await document.fonts.ready;
    const c = document.createElement('canvas').getContext('2d');
    const w = {};
    for (const f of ['"ChillBitmap"', '"Unifont"', 'monospace']) {
      c.font = '16px ' + f;
      w[f] = c.measureText('汉').width;
    }
    return {
      unifont_loaded: document.fonts.check('16px "Unifont"'),
      chill_loaded: document.fonts.check('16px "ChillBitmap"'),
      widths: w,
      hasRender: typeof window.renderPanel,
    };
  });
  console.log(JSON.stringify(r, null, 1));

  // 再直接比两次渲染的 dataURL
  const base = { ramp: [[2,234,117],[1,177,88],[0,121,60],[0,65,32]], title: '夜航西飞', artist: '柏桦',
                 transport: 'playing', mode: 'REPEAT', eq: 'Rock', elapsed: 73, length: 252, grid: true,
                 face: 'name', lines: [] };
  // ★ 拿一个**根本不存在**的字体名做对照：如果它也不改画面，说明 `cfg.plan` 被整个忽略了。
  const probe = await p.evaluate(c => {
    window.renderPanel(c);
    return { planCjk: (typeof PLAN !== 'undefined') ? PLAN.cjkFont : '(PLAN 不可见)' };
  }, { ...base, plan: { cjkFont: '"NoSuchFontXYZ"', cjkPx: 16, latinFont: '"ArkPixel"', latinPx: 12 } });
  console.log('页面里 PLAN.cjkFont =', JSON.stringify(probe));

  // ★ 直接比"同一个字在两张字体下栅格化出来的位图"——把问题夹到最里层。
  const ras = await p.evaluate(() => {
    const one = (ch, fam) => {
      const px = 16;
      const probe = document.createElement('canvas').getContext('2d');
      probe.font = `${px}px ${fam}`;
      const m = probe.measureText(ch), pad = px * 2 + 4;
      const w = Math.max(1, Math.ceil(m.width + pad * 2)), h = px + 6;
      const c = document.createElement('canvas'); c.width = w; c.height = h;
      const x = c.getContext('2d'); x.font = `${px}px ${fam}`;
      x.textBaseline = 'top'; x.fillStyle = '#000';
      x.clearRect(0, 0, w, h); x.fillText(ch, pad, 1);
      const d = x.getImageData(0, 0, w, h).data;
      let ink = 0; const bits = [];
      for (let i = 0; i < d.length; i += 4) { const on = d[i + 3] >= 128; if (on) ink++; bits.push(on ? '1' : '0'); }
      return { w, h, ink, sig: bits.join('') };
    };
    const out = {};
    for (const fam of ['"ChillBitmap"', '"Unifont"', '"NoSuchFontXYZ"', 'monospace', 'serif']) {
      const r = one('夜', fam);
      out[fam] = { ink: r.ink, w: r.w, sig: r.sig.slice(0, 40) };
    }
    return out;
  });
  console.log('四路对照:', JSON.stringify(ras, null, 1));

  const a = await p.evaluate(c => window.renderPanel(c), { ...base, plan: { cjkFont: '"ChillBitmap"', cjkPx: 16, latinFont: '"ArkPixel"', latinPx: 12 } });
  const u = await p.evaluate(c => window.renderPanel(c), { ...base, plan: { cjkFont: '"Unifont"', cjkPx: 16, latinFont: '"ArkPixel"', latinPx: 12 } });
  const z = await p.evaluate(c => window.renderPanel(c), { ...base, plan: { cjkFont: '"NoSuchFontXYZ"', cjkPx: 16, latinFont: '"ArkPixel"', latinPx: 12 } });
  console.log('chill==unifont :', a === u);
  console.log('chill==bogus  :', a === z);
  await b.close();
})().catch(e => { console.error(e); process.exit(1); });
