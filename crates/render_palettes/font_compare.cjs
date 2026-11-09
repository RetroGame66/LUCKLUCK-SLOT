const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');

const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const OUT = path.join(ROOT, '_scratch/shots_mock/font-compare-2026-10-05');
const ARK = path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf');
const CHILL = path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf');
const EXE = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const ark64 = fs.readFileSync(ARK).toString('base64');
  const chill64 = fs.readFileSync(CHILL).toString('base64');
  const browser = await chromium.launch({
    headless: true,
    executablePath: EXE,
    args: ['--no-sandbox', '--disable-gpu', '--force-color-profile=srgb']
  });
  const page = await browser.newPage({ viewport: { width: 720, height: 700 }, deviceScaleFactor: 1 });
  await page.setContent('<!doctype html><html><head><meta charset="utf-8"></head><body style="margin:0;background:#efeee9"><canvas id="out" width="720" height="700"></canvas></body></html>');
  await page.addStyleTag({ content:
    `@font-face{font-family:"ArkPixel";src:url(data:font/otf;base64,${ark64}) format("opentype");}` +
    `@font-face{font-family:"ChillBitmap";src:url(data:font/ttf;base64,${chill64}) format("truetype");}`
  });
  await page.evaluate(async () => {
    await document.fonts.load('24px "ArkPixel"', '夜空中最亮的星音');
    await document.fonts.load('16px "ChillBitmap"', '夜空中最亮的星音');
    await document.fonts.ready;
  });
  const dataUrl = await page.evaluate(() => {
    const OUT_W = 720, SCALE = 3, THRESH = 128;
    const PHRASE = '夜空中最亮的星';
    const ZOOM_CHAR = '音';
    const bg = '#f3f1ea', ink = '#171717';
    const sheet = document.getElementById('out');
    const sctx = sheet.getContext('2d');
    sctx.fillStyle = bg; sctx.fillRect(0, 0, sheet.width, sheet.height);
    sctx.fillStyle = '#202020'; sctx.font = 'bold 21px system-ui, sans-serif';
    sctx.fillText('LCD 播放标题字体对照', 24, 31);
    sctx.font = '13px system-ui, sans-serif'; sctx.fillStyle = '#555';
    sctx.fillText('同一句中文；原字号栅格化 → 1-bit 阈值 → ×3 最近邻。无遮罩，单看字形。下方单字为 ×6 放大。', 24, 50);

    function glyph(ch, family, px) {
      const probe = document.createElement('canvas').getContext('2d');
      probe.font = `${px}px "${family}"`;
      const m = probe.measureText(ch), pad = px * 2 + 4;
      const w = Math.max(1, Math.ceil(m.width + pad * 2)), h = px + 6;
      const c = document.createElement('canvas'); c.width = w; c.height = h;
      const x = c.getContext('2d'); x.font = probe.font; x.textBaseline = 'top'; x.fillStyle = '#000';
      x.fillText(ch, pad, 1);
      const d = x.getImageData(0, 0, w, h).data;
      let minX = w, minY = h, maxX = -1, maxY = -1;
      for (let y = 0; y < h; y++) for (let xx = 0; xx < w; xx++) {
        if (d[(y * w + xx) * 4 + 3] >= THRESH) {
          minX = Math.min(minX, xx); minY = Math.min(minY, y);
          maxX = Math.max(maxX, xx); maxY = Math.max(maxY, y);
        }
      }
      const gw = maxX < minX ? 0 : maxX - minX + 1;
      const gh = maxY < minY ? 0 : maxY - minY + 1;
      const bits = new Uint8Array(gw * gh);
      for (let y = 0; y < gh; y++) for (let xx = 0; xx < gw; xx++) {
        bits[y * gw + xx] = d[((minY + y) * w + minX + xx) * 4 + 3] >= THRESH ? 1 : 0;
      }
      return { w: gw, h: gh, bits, dx: minX - pad, dy: minY - 1, advance: m.width };
    }
    function wordWidth(text, family, px) {
      const chars = Array.from(text);
      return chars.reduce((sum, ch) => sum + glyph(ch, family, px).advance, 0);
    }
    function drawPixel(ctx, x, y, size, color) {
      if (x < 0 || x + size > OUT_W || y < 0 || y + size > 700) return;
      ctx.fillStyle = color; ctx.fillRect(x, y, size, size);
    }
    function drawLine(ctx, text, family, px, yPx) {
      const chars = Array.from(text);
      const width = wordWidth(text, family, px);
      let pen = (OUT_W / SCALE - width) / 2;
      for (const ch of chars) {
        const g = glyph(ch, family, px);
        for (let gy = 0; gy < g.h; gy++) for (let gx = 0; gx < g.w; gx++) {
          if (!g.bits[gy * g.w + gx]) continue;
          const x0 = Math.round((pen + g.dx + gx) * SCALE);
          const y0 = yPx + (g.dy + gy) * SCALE;
          drawPixel(ctx, x0, y0, SCALE, ink);
        }
        pen += g.advance;
      }
    }
    function drawZoom(ctx, family, px, yPx) {
      const g = glyph(ZOOM_CHAR, family, px), size = 6;
      const xStart = Math.round(OUT_W / 2 - g.w * size / 2);
      for (let gy = 0; gy < g.h; gy++) for (let gx = 0; gx < g.w; gx++) {
        if (!g.bits[gy * g.w + gx]) continue;
        drawPixel(ctx, xStart + gx * size, yPx + gy * size, size, ink);
      }
    }
    function card(y, family, px, name, sub, detail) {
      const cardH = 294;
      sctx.fillStyle = '#ffffff'; sctx.fillRect(16, y, 688, cardH);
      sctx.strokeStyle = '#d1cfc8'; sctx.lineWidth = 1; sctx.strokeRect(16.5, y + .5, 687, cardH - 1);
      sctx.fillStyle = '#151515'; sctx.font = 'bold 17px system-ui, sans-serif'; sctx.fillText(name, 30, y + 25);
      sctx.fillStyle = '#5b5b5b'; sctx.font = '12px system-ui, sans-serif'; sctx.fillText(sub, 30, y + 44);
      drawLine(sctx, PHRASE, family, px, y + 60);
      sctx.strokeStyle = '#e0ded7'; sctx.beginPath(); sctx.moveTo(30, y + 137); sctx.lineTo(690, y + 137); sctx.stroke();
      sctx.fillStyle = '#686868'; sctx.font = '12px system-ui, sans-serif'; sctx.fillText('单字「音」放大 ×6（每块为一个原始字体像素）', 30, y + 158);
      drawZoom(sctx, family, px, y + 170);
      sctx.fillStyle = '#606060'; sctx.font = '12px system-ui, sans-serif'; sctx.fillText(detail, 30, y + 281);
    }
    card(64, 'ArkPixel', 24, 'A · Ark Pixel 12px @ 24px',
      '项目当前大标题方案：12px 设计字形的 2 倍；fontdue 实测 24px 半调 0.0%，但字面密、视觉较重。',
      '原生标题：24px 字形 / ×3 显示；没有额外加粗或字间扩张。');
    card(370, 'ChillBitmap', 16, 'B · 寒蝉点阵体 16px @ 16px',
      '建议优先试用：16px 原生点阵；fontdue 实测 16px 半调 0.0%，与 Ark 24px 同为硬像素。',
      '原生标题：16px 字形 / ×3 显示；通常会显得更轻、更松。');
    return sheet.toDataURL('image/png');
  });
  const png = Buffer.from(dataUrl.split(',')[1], 'base64');
  fs.writeFileSync(path.join(OUT, 'font-candidates.png'), png);
  const html = `<!doctype html><html lang="zh"><head><meta charset="utf-8"><title>播放界面字体候选对照</title><style>body{margin:0;background:#efeee9;color:#222;font:14px/1.5 system-ui,sans-serif}main{width:720px;margin:20px auto}img{display:block;width:720px;height:auto;image-rendering:pixelated}p{color:#555;margin:8px 0 16px}@media(max-width:760px){main{margin:0;width:720px}}</style></head><body><main><p>复古 LCD 播放标题候选。图片按原始 720px 宽查看；下方「音」字放大用于比较单字笔画与留白。</p><img src="data:image/png;base64,${png.toString('base64')}" width="720" height="700" alt="Ark Pixel 24px 与寒蝉点阵体 16px 字体对照"><p>结论：如果嫌当前标题黏连，先试寒蝉点阵体 16px，而不是把 Ark Pixel 缩放到 16px；两者均是按自身合适字号栅格化。该候选字体包较大（约 12.6 MB），实际接入前还需确认发布包体积与许可证文件一并携带。</p></main></body></html>`;
  fs.writeFileSync(path.join(OUT, 'font-candidates.html'), html, 'utf8');
  await browser.close();
  console.log('Wrote 720px comparison:', path.join(OUT, 'font-candidates.png'));
  console.log('Wrote standalone preview:', path.join(OUT, 'font-candidates.html'));
})().catch(err => { console.error(err); process.exit(1); });
