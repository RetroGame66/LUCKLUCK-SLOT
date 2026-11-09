// 720×480 原生像素阅读器样张：字号阶梯 / 色板 / 遮罩 / 书架。
// 字体与播放面板同一对：汉字 16px 寒蝉点阵体、拉丁 12px Ark Pixel，按整数倍栅格化。
const fs = require('fs');
const path = require('path');
const { chromium } = require('C:/Users/C.R.A.Z.Y/.workbuddy-ai/binaries/node/workspace/node_modules/playwright');

const ROOT = 'F:/WorkBuddyWorkSpace/RGSP_SLOT_66MOD';
const ARK   = path.join(ROOT, '字体/ark-pixel-12px-proportional-zh_hans.otf');
const CHILL = path.join(ROOT, '_scratch/2026-10-04-fonts/chill16.ttf');
// 候选字体（2026-10-07 收进 `_scratch/pxfonts-2026-10-07/`，见
// `slot-点阵中文字体-候选与实测-2026-10-07.md`）
const PXF   = path.join(ROOT, '_scratch/pxfonts-2026-10-07');
const FUSION8 = path.join(PXF, 'fusion8-zh_hans.otf');
const CUBIC11 = path.join(PXF, 'Cubic11.ttf');
const WQY     = path.join(PXF, 'wqy-bitmap-song.ttf');
const ARK10   = path.join(ROOT, 'slot-main/crates/slot-ui/assets/ark-pixel-10px-proportional-zh_hans.otf');
const UNIFONT = path.join(PXF, 'unifont.otf');
const MISAKI  = path.join(PXF, 'misaki_gothic.ttf');
const HTML  = path.join(ROOT, 'slot-main/crates/render_palettes/reader.html');
const OUT   = path.join(ROOT, '_scratch/shots_reader');
const EXE   = 'C:/Users/C.R.A.Z.Y/AppData/Local/ms-playwright/chromium_headless_shell-1148/chrome-win/headless_shell.exe';

// 真实 PixelShift 色板（取自 slot/src/palettes.rs，索引 0=纸 … 3=墨）
const PS = {
  PS10: [[2,234,117],[1,177,88],[0,121,60],[0,65,32]],           // Game Boy Light
  PS17: [[253,169,16],[248,136,6],[233,107,8],[131,52,3]],       // GB Backlight Orange
  PS16: [[255,255,255],[226,226,226],[158,158,158],[128,128,128]], // GB Backlight Faded
  PS15: [[63,193,255],[6,142,255],[0,88,255],[0,0,109]],         // GB Backlight Blue
  PS33: [[232,232,232],[160,160,160],[88,88,88],[16,16,16]],     // Gray Green Mix
  PS11: [[146,151,117],[101,106,85],[83,88,73],[40,44,38]],      // Game Boy Pocket
  PS21: [[180,176,173],[125,122,120],[70,69,68],[16,16,16]],     // GB Hunter
};

const TEXT = `夜里十一点，列车离开站台。车窗上凝着一层薄薄的水汽，把外面的灯火揉成一片模糊的橘色。

我坐在靠窗的位置，手里是一本翻旧了的书。纸页发黄，边角起了毛，像被许多人摸过。列车每经过一个道岔，车身就轻轻晃一下，字也跟着晃一下，于是同一句话要读两遍才读得进去。

这是一趟慢车。它不着急，我也不着急。沿途的小站名字都很好听：青石桥、柳家渡、白鹤岭、十三里铺——每一个都像是从旧小说里拆下来的。车停的时候，站台上只有一盏灯，和一个穿蓝布褂子的值班员。他举着小旗，旗子在风里抖了两下，火车就又开动了。

有人说过，旅行最迷人的部分不是抵达，而是这段谁也不认识你、你也不认识谁的时间。窗外的黑是完整的黑，偶尔有一两点灯光划过去，像有人在山里擦亮一根火柴，又很快按灭了。

我把书合上，放在膝盖上。封面上印着几个已经褪色的字，看不清了。奇怪的是，我明明记得它的名字，却想不起是谁送我的。也许是高中的同桌，也许是大学宿舍楼下那个旧书摊的老板。他总把书按斤称，论斤卖，说是"知识不打折，纸打折"。

那时候我还不懂得，一本书真正的重量不在纸上。它在读它的人身上。你读到哪一页停下来，那一页就替你记着当时的天色、气味和心情。多年以后你重新翻开，字还是那些字，但停在那一页的理由已经变了。

列车进隧道的时候，车厢里的灯一下子亮了。对面座位上的女人抬起头看了我一眼，又低下去继续织她的毛衣。毛线是深红色的，针脚很密。我猜她是要送给谁的——也许是女儿，也许是孙女，也许只是一个很久没见的人。人在无事可做的时候，手里的活计就成了时间的形状。

隧道很长。灯灭了又亮，亮了又灭。我开始数隧道壁上掠过去的灯，数到第三十七盏的时候放弃了。

凌晨两点，列车停在一个我叫不出名字的站。站牌被雨淋得发白，上面的字只剩下半边。我下车走了几步，空气是凉的，带着草和泥土的味道。远处有一排平房，只有一扇窗户亮着。

我忽然想起小时候，家里也是这样一盏灯。母亲坐在灯下缝补，我在旁边写作业。那时的夜很安静，安静到能听见笔尖划过纸面的沙沙声。后来我长大了，灯换了几盏，房子换了几间，那种安静却再也没有找回来过。

汽笛响了。我转身走回车厢，把书重新翻开，翻到刚才停下的那一页。

——就到这里吧。剩下的路，留给下一个读它的人。`;

const BOOK = '夜航西飞 · 第一章';
const base = { ramp: PS.PS10, text: TEXT, book: BOOK, screen: 'read',
               pageLabel: '第 3/128 页', stripLeft: '第 3/128 页', stripRight: '2.3%', frac: 0.023 };

const configs = [
  // A · 字号阶梯（同一页文，1×/2×/3×；字体都在设计尺寸的整数倍上）
  { file:'r-a1-16px.png', label:'A1 · 正文 16px（1×，汉字 16 / 拉丁 12）', ...base, scale:1, pageLabel:'16px' },
  { file:'r-a2-32px.png', label:'A2 · 正文 32px（2×，汉字 32 / 拉丁 24）', ...base, scale:2, pageLabel:'32px' },
  { file:'r-a3-48px.png', label:'A3 · 正文 48px（3×，汉字 48 / 拉丁 36）', ...base, scale:3, pageLabel:'48px' },
  // B · 色板（32px，PixelShift 通用轮换）
  { file:'r-b1-ps10.png', label:'B1 · 32px + PS10 Game Boy Light', ...base, scale:2, pageLabel:'PS10', ramp:PS.PS10 },
  { file:'r-b2-ps17.png', label:'B2 · 32px + PS17 GB Backlight Orange', ...base, scale:2, pageLabel:'PS17', ramp:PS.PS17 },
  { file:'r-b3-ps16.png', label:'B3 · 32px + PS16 GB Backlight Faded', ...base, scale:2, pageLabel:'PS16', ramp:PS.PS16 },
  { file:'r-b4-ps15.png', label:'B4 · 32px + PS15 GB Backlight Blue', ...base, scale:2, pageLabel:'PS15', ramp:PS.PS15 },
  { file:'r-b5-ps33.png', label:'B5 · 32px + PS33 Gray Green Mix', ...base, scale:2, pageLabel:'PS33', ramp:PS.PS33 },
  // C · 遮罩（PXReader 的做法：扫描线 / 点阵 / 叠加）
  { file:'r-c0-none.png', label:'C0 · 32px 无遮罩', ...base, scale:2, mask:'none' },
  { file:'r-c1-scan.png', label:'C1 · 扫描线（每 3 行一条）', ...base, scale:2, mask:'scan' },
  { file:'r-c2-dot.png',  label:'C2 · 点阵（每 4px 一个 2×2）', ...base, scale:2, mask:'dot' },
  { file:'r-c3-both.png', label:'C3 · 扫描线 + 点阵', ...base, scale:2, mask:'both' },
  // D · 书架（货架从 DISC 变成书籍）
  { file:'r-d1-shelf.png', label:'D1 · 书架（书脊货架）', ...base, screen:'shelf', count:'共 128 本' },
  // E · 12px 网格字体（Ark Pixel）——多出 24px 这一档
  { file:'r-e1-ark24.png', label:'E1 · Ark Pixel 12px 网格 @ 24px（2×，汉字 24 / 拉丁 24）', ...base, scale:2, font:'ark12', pageLabel:'24px' },
  { file:'r-e2-ark36.png', label:'E2 · Ark Pixel 12px 网格 @ 36px（3×，汉字 36 / 拉丁 36）', ...base, scale:3, font:'ark12', pageLabel:'36px' },
  { file:'r-e3-ark24-ps16.png', label:'E3 · 同上 24px，换 PS16 纸白', ...base, scale:2, font:'ark12', pageLabel:'PS16', ramp:PS.PS16 },
  { file:'r-e4-ark24-tight.png', label:'E4 · 24px + 紧行距（行高 28，em 框本来是 32）', ...base, scale:2, font:'ark12', lineH:28, pageLabel:'24px 紧' },
  { file:'r-e5-ark32-tight.png', label:'E5 · 32px 紧行距（行高 38）', ...base, scale:2, lineH:38, pageLabel:'32px 紧' },
  // F · 字体对照：**同一页文、同一字号 24px** —— 三张候选在 24 上都是硬像素（实测）
  { file:'r-f1-ark12-24.png',   label:'F1 · Ark Pixel 12px @ 24（12×2 · OFL · 3.5 MB）',        ...base, scale:2, font:'ark12',   pageLabel:'Ark12' },
  { file:'r-f2-fusion8-24.png', label:'F2 · Fusion Pixel 8px @ 24（8×3 · **MIT** · 2.5 MB）★',    ...base, scale:3, font:'fusion8', pageLabel:'Fusion8' },
  { file:'r-f3-cubic11-24.png', label:'F3 · Cubic 11 @ 24（实测对齐 12×2 · OFL · 2.8 MB）',      ...base, scale:2, font:'cubic11', pageLabel:'Cubic11' },
  { file:'r-f4-chill16-32.png', label:'F4 · 寒蝉 16px @ 32（16×2 · OFL · 12.6 MB · 面板现用）',   ...base, scale:2, font:'chill',   pageLabel:'寒蝉16' },
  { file:'r-f5-wqy18-36.png',   label:'F5 · 文泉驿点阵宋体 @ 36（18×2 · 唯一的宋体）',            ...base, scale:2, font:'wqy',     pageLabel:'文泉驿' },
  // G · 全覆盖那两张：**基本区 100%**，与上面那些"砍字库换体积"的候选对比
  { file:'r-g1-unifont-32.png', label:'G1 · GNU Unifont @ 32（16×2 · 覆盖 100%/100% · 5.3 MB）',  ...base, scale:2, font:'unifont', pageLabel:'Unifont' },
  { file:'r-g2-misaki-32.png',  label:'G2 · 美咲 8×8 @ 32（8×4 · JIS 字集，中文仅 30.3%）',        ...base, scale:4, font:'misaki',  pageLabel:'美咲' },
  // H · 16px 原尺寸对照（Unifont 的设计尺寸就是 16）
  { file:'r-h1-unifont-16.png', label:'H1 · GNU Unifont **原生 16px**（1× · 真书 100% · 5.3 MB）', ...base, scale:1, font:'unifont', pageLabel:'Unifont' },
  { file:'r-h2-chill-16.png',   label:'H2 · 寒蝉 **原生 16px**（1× · 真书 100% · 12.6 MB）',       ...base, scale:1, font:'chill',   pageLabel:'寒蝉' },
];

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const browser = await chromium.launch({ headless: true, executablePath: EXE,
    args: ['--no-sandbox','--disable-gpu','--force-color-profile=srgb'] });
  const page = await browser.newPage({ viewport: { width: 720, height: 480 }, deviceScaleFactor: 1 });
  page.on('console', m => { if (m.type()==='error') console.error('PAGE ERROR:', m.text()); });
  page.on('pageerror', e => console.error('PAGE EXCEPTION:', e.message));
  await page.goto('file://' + HTML);
  const face = (fam, file, fmt) =>
    `@font-face{font-family:"${fam}";src:url(data:font/${fmt};base64,${fs.readFileSync(file).toString('base64')}) format("${fmt === 'otf' ? 'opentype' : 'truetype'}");}`;
  await page.addStyleTag({ content:
    face('ArkPixel', ARK, 'otf') +
    face('ChillBitmap', CHILL, 'ttf') +
    face('ArkPixel10', ARK10, 'otf') +
    face('Fusion8', FUSION8, 'otf') +
    face('Cubic11', CUBIC11, 'ttf') +
    face('WqyBitmap', WQY, 'ttf') +
    face('Unifont', UNIFONT, 'otf') +
    face('Misaki', MISAKI, 'ttf') });
  await page.evaluate(async () => {
    // 每张字体按**它自己的设计网格**预热（见 `reader.html` 的 FAMS）。
    for (const [f, s] of [['ChillBitmap',16],['ArkPixel',12],['ArkPixel10',10],['Fusion8',8],['Cubic11',12],['WqyBitmap',18],['Unifont',16],['Misaki',8]])
      await document.fonts.load(`${s}px "${f}"`, '夜航西飞第一章ABC123');
    await document.fonts.ready;
  });

  const measures = [];
  for (const [f,n,lh] of [['chill',1,null],['chill',2,null],['chill',3,null],
                          ['ark12',2,null],['ark12',3,null],
                          ['chill',2,38],['ark12',2,28]])
    measures.push(await page.evaluate(a => window.measure(a[0], a[1], a[2]), [f,n,lh]));
  console.log('measures:', JSON.stringify(measures));

  const sheet = [];
  for (const cfg of configs) {
    const url = await page.evaluate(c => window.render(c), cfg);
    fs.writeFileSync(path.join(OUT, cfg.file), Buffer.from(url.split(',')[1], 'base64'));
    sheet.push([cfg.label, cfg.file]);
    console.log('wrote', cfg.file);
  }
  await browser.close();

  let h = `<!doctype html><html lang=zh><head><meta charset=utf-8><title>像素阅读器 · 720×480 原生样张</title>`
    + `<style>body{background:#2b2b2b;color:#ddd;font:13px/1.5 system-ui,sans-serif;margin:0;padding:18px}`
    + `h1{font-size:18px;margin:0 0 4px}h2{font-size:13px;color:#9a9a9a;margin:0 0 6px;font-weight:400}`
    + `table{border-collapse:collapse;margin:0 0 18px;font-size:12px}td,th{border:1px solid #3a3a3a;padding:3px 9px;text-align:right}th{color:#9a9a9a;font-weight:400}`
    + `.grid{display:grid;grid-template-columns:repeat(auto-fill,720px);gap:18px}`
    + `figure{margin:0;background:#1c1c1c;border:1px solid #3a3a3a;border-radius:8px;overflow:hidden;width:720px}`
    + `figcaption{padding:8px 10px;font-size:12px;border-top:1px solid #3a3a3a}`
    + `img{display:block;width:720px;height:480px;image-rendering:pixelated;background:#000}</style></head><body>`;
  h += `<h1>像素阅读器 · 720×480 原生样张</h1>`;
  h += `<h2>逻辑帧就是 720×480，1:1 出图，<b>无格网</b>。字体按各自网格取整数倍栅格化 · 色板 = PixelShift 通用轮换</h2>`;
  h += `<table><tr><th>字体</th><th>档</th><th>汉字 px</th><th>拉丁 px</th><th>行高</th><th>行/页</th><th>字/行</th><th>字/页</th></tr>`;
  for (const m of measures) h += `<tr><td>${m.font==='chill'?'寒蝉 16px 网格':'Ark 12px 网格'}</td><td>${m.n}×</td><td>${m.cjk}</td><td>${m.latin}</td><td>${m.lineH}</td><td>${m.rows}</td><td>${m.cjkPerLine}</td><td>${m.rows*m.cjkPerLine}</td></tr>`;
  h += `</table><div class=grid>`;
  for (const [cap, file] of sheet) h += `<figure><img src="${file}" alt="${cap}"><figcaption>${cap}</figcaption></figure>`;
  h += '</div></body></html>';
  fs.writeFileSync(path.join(OUT, 'index.html'), h);
  console.log('done -> ' + OUT);
})().catch(e => { console.error(e); process.exit(1); });
