//! 卷闸：换机种时从顶上放下来、盖满整屏、再卷回去的那道帘子。
//!
//! ★★ 2026-10-24（六六）：「**卷闸转场**」—— 2026-10-22 13:5x 定方案、当天出了 GIF 预览
//! （`_tools/make_volgate_gif5.py` → `outputs/volgate_scifi_topbar_v5.gif`），2026-10-24 20:0x
//! 第一次落进 Rust。**v1 落错了**：把 CHANGE 钉在屏幕顶上、只在满闸时淡入；而预览里
//! 顶栏是**帘子的下沿**（"顶栏尾帘"）—— 名字先淡成 CHANGE，再一路跟着帘子下到屏外，
//! 卷回时新顶栏又从下沿长回来。v2（本文件）按预览的四段重写。
//!
//! ## 四段（60 帧 @30fps = 2.0 s，与预览同一套比例 12/18/12/18）
//!
//! | 段 | 时长 | 发生什么 |
//! |---|---|---|
//! | P1 | 0.40 s | 帘子不动。**顶栏里的名字淡出、CHANGE 淡入**（顶栏用内腔色盖平） |
//! | P2 | 0.60 s | 帘子从顶上放下，**顶栏挂在帘子下沿一起走**，CHANGE 全程亮着 |
//! | P3 | 0.40 s | 满闸，只有帘片 —— 顶栏已经跟着走到屏外。**换机种就在这一段** |
//! | P4 | 0.60 s | 帘子卷回，**新顶栏从下沿淡入**，帘子退完时它正好回到 y=0 |
//!
//! ## 配色**跟主题/色板走**（六六：「卷闸颜色随主题色板」）
//!
//! 没有任何写死的青。`ink()` 每次都现取：
//! - 帘体上/下 = `housing()` 与 `accent`、`opening()` 的混合；
//! - 片缝高光、CHANGE 的字色 = **`palette::ridge_ink()`** —— 换色板（18 档 ramp）时它是
//!   ramp 的 `bright`，换明暗主题时它是 0xd8d4cc / 0x6b645b；
//! - 金属高光 = `edge()` 提亮；缝 = `opening()`。
//! 于是换主题/换色板，帘子与 CHANGE 一起变。
//!
//! ## 为什么是矩形不是贴图
//!
//! 预览里的拟真感（金属拉丝噪点、竖直数据流、逐行高光）是逐像素画的；`Draw` 只有矩形，
//! 硬要贴图就得每帧上传 480 行 × 720 的 PNG，`H700` 上不该这么干。这里用矩形近似：
//! 卷筒 4 条 + 每片 2 条（2px 高光 + 11px 帘体）+ 帘底压暗，满闸约 60 个 `Draw::Rect`，
//! 与货架一帧几百个矩形同一个量级。

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};

use crate::palette::ridge_ink;
use crate::slot_chrome::{edge, housing, opening};

/// 卷筒（帘子卷在里面的那根轴）的高度。
const DRUM_H: f32 = 20.0;
/// 一片帘的总高：帘体 + 缝。
const PITCH: f32 = 16.0;
/// 帘体本身，剩下的 3px 是缝。
const PIECE: f32 = 13.0;
/// 帘体顶部那 2px 的金属高光。
const EDGE_H: f32 = 2.0;
/// 帘底压暗的高度（配重块的"重"只靠明暗，不画亮线）。
const WEIGHT: f32 = 13.0;
/// 配重块压暗的比例。
const WEIGHT_DIM: f32 = 0.45;

/// 顶栏（跟着帘子下沿走的那个"尾"）：400 × 54，与桌面图里的名称窗同宽同位。
///
/// ★★ 2026-10-29（640 树）：宽 400 不动（与名称窗同为内容决定），位置重新居中：
/// 720 上 `(720−400)/2 = 160`，本树 `(640−400)/2 = 120`。
/// `CHANGE_X` 由 `BAR_X` 派生，自动跟着走；`CHANGE_W = 360` 也仍在 400 内，不必动。
const BAR_X: f32 = 120.0;
const BAR_W: f32 = 400.0;
const BAR_H: f32 = 54.0;
/// 名称窗底部两个圆角的半径。
///
/// ★ 2026-10-24 20:42（六六）：「**下拉时顶栏的包边得优化一下**」—— 桌面图里的名称窗是
/// 圆角 + 一圈 1px `edge()` 亮边，跟着帘子走的那块如果只是一块硬矩形，它读成"屏幕上贴了
/// 张黑纸"，跟机器上那块完全不是一件东西。
///
/// ★★ 2026-10-24 20:53（六六）：「**顶栏两侧的圆角你怎么反着来？？**」—— 两处错：
///   ① 半径猜成 6，**实测桌面图的名称窗是 13**（左竖边在 y=160..40 是直的，y40→y53 走弧、
///      同时 x160→x173，两个方向各 13px）；
///   ② 弧的 `d`（距底边多少）算反了 —— 我按"从角区顶部往下数"取的 `d = i + 0.5`，
///      于是**越靠底越收**，画出来是个凹口（反着的圆角）。
///   现在 `d = R - i - 0.5`（真正距底边），弧才朝外鼓。
const BAR_R: f32 = 13.0;
/// CHANGE 压在顶栏里：360×50，x 居中、y 距栏顶 2px（与预览 `CHANGE_X/YOFF` 同一套数）。
const CHANGE_W: f32 = 360.0;
const CHANGE_H: f32 = 50.0;
const CHANGE_X: f32 = BAR_X + (BAR_W - CHANGE_W) / 2.0;
const CHANGE_Y: f32 = 2.0;

// ── 四段的时长，秒 ───────────────────────────────────────────────────────────────────────
// 预览 12/18/12/18 帧 @30fps。六六 10-22 说 1~2 s 可接受，这里取 2.0 s 的整份。
pub const P1_S: f32 = 0.40;
pub const DOWN_S: f32 = 0.60;
pub const HOLD_S: f32 = 0.40;
pub const UP_S: f32 = 0.60;
/// 帘子完全盖住的那一刻 —— 换机种必须发生在这里（P2 结束、P3 开头）。
pub const SWAP_AT_S: f32 = P1_S + DOWN_S;
/// 整段转场。
pub const TOTAL_S: f32 = P1_S + DOWN_S + HOLD_S + UP_S;

/// 一帧里帘子的三个量。
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Shutter {
    /// 帘子盖住了多少：0 全开，1 满闸。
    pub cover: f32,
    /// 帘子下沿那块顶栏的不透明度。P3 时是 0（它已经走到屏外）。
    pub bar: f32,
    /// CHANGE 的不透明度。
    pub change: f32,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// 从转场开始算起、过了 `t` 秒时的三个量。
///
/// 单独拿出来是为了能测：`App` 只把 `t` 递进来，"多满 / 顶栏在不在 / CHANGE 亮不亮"
/// 都在这里算，于是"什么时候可以换机种"只有一个答案（`SWAP_AT_S` 之后 `cover == 1.0`）。
pub fn at(t: f32) -> Shutter {
    if t <= 0.0 {
        return Shutter { cover: 0.0, bar: 1.0, change: 0.0 };
    }
    if t < P1_S {
        // P1：帘子不动，顶栏由名字淡成 CHANGE。
        return Shutter { cover: 0.0, bar: 1.0, change: smooth(t / P1_S) };
    }
    if t < SWAP_AT_S {
        // P2：帘子放下，顶栏挂在它下沿。CHANGE 全程亮着。
        return Shutter {
            cover: smooth((t - P1_S) / DOWN_S),
            bar: 1.0,
            change: 1.0,
        };
    }
    if t < SWAP_AT_S + HOLD_S {
        // P3：满闸，帘片上什么都没有 —— 顶栏跟着走到屏外了。换机种在这段里发生。
        return Shutter { cover: 1.0, bar: 0.0, change: 0.0 };
    }
    // P4：卷回，新顶栏从下沿淡入。
    let u = ((t - SWAP_AT_S - HOLD_S) / UP_S).clamp(0.0, 1.0);
    Shutter {
        cover: 1.0 - smooth(u),
        bar: smooth(u),
        change: 0.0,
    }
}

fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        1.0,
    ]
}

fn with_alpha(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], a]
}

fn rect(out: &mut Vec<Draw>, x: f32, y: f32, w: f32, h: f32, colour: [f32; 4]) {
    if h > 0.0 && colour[3] > 0.0 {
        out.push(Draw::Rect { x, y, w, h, colour });
    }
}

/// 当前主题/色板下帘子要用的五个颜色。**每帧现取**，所以换色板不用重建什么。
struct Ink {
    /// 片与片之间那道缝（也是整块帘子的底）。
    seam: [f32; 4],
    body_top: [f32; 4],
    body_bot: [f32; 4],
    /// 金属高光。
    metal: [f32; 4],
    /// 片缝高光与 CHANGE 的色 —— 就是主题的 `ridge_ink()`。
    accent: [f32; 4],
    drum: [f32; 4],
}

fn ink() -> Ink {
    let a = ridge_ink();
    let accent = [a[0] as f32 / 255.0, a[1] as f32 / 255.0, a[2] as f32 / 255.0, 1.0];
    let housing = housing();
    let opening = opening();
    let edge = edge();
    let white = [1.0, 1.0, 1.0, 1.0];
    Ink {
        seam: with_alpha(opening, 1.0),
        // 帘体上沿带一点主题色、下沿压到内腔色 —— 冷调从主题来，不是写死的青。
        body_top: mix(housing, accent, 0.30),
        body_bot: mix(opening, housing, 0.45),
        metal: mix(edge, white, 0.45),
        accent,
        drum: mix(housing, opening, 0.45),
    }
}

/// 画这一帧。`glyph` 是 CHANGE 的字，`glow` 是它预烘的那张柔光（见 [`glow_face`]）。
pub fn draw(
    s: Shutter,
    glyph: Option<(TexId, u32, u32)>,
    glow: Option<(TexId, u32, u32)>,
    out: &mut Vec<Draw>,
) {    let ink = ink();
    let cover = s.cover.clamp(0.0, 1.0);
    let bottom = (cover * OUT_H as f32).round();

    if bottom > 0.0 {
        // 底色 = 缝。一条 rect 就是所有缝隙。
        rect(out, 0.0, 0.0, OUT_W as f32, bottom, ink.seam);

        // ── 卷筒 ─────────────────────────────────────────────────────────────────────────
        let drum = DRUM_H.min(bottom);
        let mut y = 0.0;
        while y < drum {
            let h = (drum - y).min(4.0);
            let t = y / DRUM_H;
            let mut c = mix(ink.drum, ink.accent, 0.30 - t * 0.45);
            if y == 0.0 {
                c = mix(ink.drum, ink.metal, 0.7);
            } else if (y - 8.0).abs() < 2.0 {
                // 轴心那道冷光
                c = mix(ink.accent, ink.drum, 0.5);
            }
            rect(out, 0.0, y, OUT_W as f32, h, c);
            y += h;
        }

        // ── 帘片 ─────────────────────────────────────────────────────────────────────────
        let mut y = DRUM_H;
        while y < bottom {
            let frac = y / OUT_H as f32;
            let vert = mix(ink.body_top, ink.body_bot, 0.15 + 0.7 * frac);
            rect(out, 0.0, y, OUT_W as f32, EDGE_H.min(bottom - y), mix(vert, ink.accent, 0.5));
            let body_y = y + EDGE_H;
            let body_h = (PIECE - EDGE_H).min(bottom - body_y);
            if body_h > 0.0 {
                rect(out, 0.0, body_y, OUT_W as f32, body_h, mix(vert, ink.metal, 0.16));
            }
            y += PITCH;
        }

        // ── 配重压暗 ─────────────────────────────────────────────────────────────────────
        // 只压暗，不画亮线（六六 16:38 明确否掉过亮线）。
        if bottom > DRUM_H + WEIGHT {
            rect(
                out,
                0.0,
                bottom - WEIGHT,
                OUT_W as f32,
                WEIGHT,
                [0.0, 0.0, 0.0, WEIGHT_DIM],
            );
        }
    }

    // ── 顶栏：挂在帘子下沿（"顶栏尾帘"）────────────────────────────────────────────────────
    //
    // 它就是桌面图里那个名称窗，只是**跟着帘子走**：P1 停在 y=0（原位），P2 随帘子下到屏外，
    // P4 又随卷帘回到 y=0。栏内填内腔色 —— 这一块同时也是把旧名字盖平的那张纸：
    // 六六 16:36 的坑正是"旧游戏名叠在 CHANGE 底下"，所以这里必须不透明，不能淡入。
    let bar_a = s.bar.clamp(0.0, 1.0);
    if bar_a > 0.0 {
        let bar_y = bottom;
        // 栏体 + 包边：跟机器上那块名称窗一个做法（R6 圆角 + 1px 亮边）。
        bar_plate(
            out,
            BAR_X,
            bar_y,
            BAR_W,
            BAR_H,
            with_alpha(opening(), bar_a),
            with_alpha(edge(), bar_a),
        );
        // CHANGE 就印在这块栏里：先铺光，再压字。
        //
        // ★ 2026-10-24 20:42（六六）：「**CHANGE 贴图出现的时候能发光吗？可能看起来更好看**」。
        //   光晕是**开机时烘好的一张图**（[`glow_face`]：把字的 alpha 糊开再着成主题亮色），
        //   不是运行期把字叠十几遍 —— 叠出来的是一圈硬阴影，真正的发光是一条柔和的边。
        //   运行期代价就是一个额外 quad。
        let a = s.change.clamp(0.0, 1.0) * bar_a;
        if a > 0.0 {
            if let Some((tex, gw, gh)) = glow {
                if gw > 0 && gh > 0 {
                    let pad = GLOW_PAD as f32;
                    out.push(Draw::Tex {
                        x: CHANGE_X - pad,
                        y: bar_y + CHANGE_Y - pad,
                        w: gw as f32,
                        h: gh as f32,
                        tex,
                        alpha: a,
                    });
                }
            }
            if let Some((tex, sw, sh)) = glyph {
                if sw > 0 && sh > 0 {
                    out.push(Draw::Tex {
                        x: CHANGE_X,
                        y: bar_y + CHANGE_Y,
                        w: CHANGE_W,
                        h: CHANGE_H,
                        tex,
                        alpha: a,
                    });
                }
            }
        }
    }
}

/// 一块名称窗：`R6` 圆角 + 1px 包边，与桌面图里顶栏那一块同形。
///
/// 帘子下沿挂着的这块栏**必须看起来是机器上的塑料**，不然它就是一贴在帘子上的黑条。
/// 矩形画不出圆角，这里按圆的剖面把底部 6 行逐行内收；包边是同一剖面上一圈 1px 的
/// 亮色 —— 左/右竖线、底横线、两角的阶梯各补一个点。
fn bar_plate(out: &mut Vec<Draw>, x: f32, y: f32, w: f32, h: f32, fill: [f32; 4], rim: [f32; 4]) {
    let r = BAR_R;
    // 主体（圆角以上那整块）。
    rect(out, x, y, w, h - r, fill);
    // 两条竖边。**上缘没有线** —— 跟桌面图里的名称窗一样，上缘是内腔直接到顶。
    rect(out, x, y, 1.0, h - r, rim);
    rect(out, x + w - 1.0, y, 1.0, h - r, rim);
    // 底部圆角那 r 行：`corner_inset(i)` 是这一行左边界的收进量，越靠底越大。
    let mut i = 0;
    while i < r as usize {
        let row = y + h - r + i as f32;
        let inset = corner_inset(i, r);
        rect(out, x + inset, row, w - 2.0 * inset, 1.0, fill);
        rect(out, x + inset, row, 1.0, 1.0, rim);
        rect(out, x + w - inset - 1.0, row, 1.0, 1.0, rim);
        i += 1;
    }
    // 底边亮线：接着**最后一行**的两个端帽，不然弧和底边之间会缺一截。
    let last = corner_inset(r as usize - 1, r);
    rect(out, x + last, y + h - 1.0, w - 2.0 * last, 1.0, rim);
}

/// 圆角区第 `i` 行（0 = 最上面那行，`r-1` = 最底那行）的左边收进量。
///
/// **`d` 是距底边多少**，不是距角区顶部多少 —— 这个方向搞反过一次，画出来的就是"反着的
/// 圆角"（越靠底越收，成了个凹口）。
fn corner_inset(i: usize, r: f32) -> f32 {
    let d = r - i as f32 - 0.5;
    let k = (r - d).max(0.0);
    r - (r * r - k * k).max(0.0).sqrt()
}

/// 光晕往外铺多少像素，四周对称。贴图自带这圈留白，所以它比字本身大 `2*GLOW_PAD`。
///
/// ★ 2026-10-24 20:52（六六）：「**辉光再小一点，现在字体看不太清**」—— 8 太肥：字的封闭
/// 空腔（A 的三角、G 的缺口）被光糊住一半，字就糊了。收到 6，配合下面的窄核。
pub const GLOW_PAD: u32 = 6;

/// 给一张字面烘一圈柔光：**把 alpha 糊开，再按主题亮色铺成一张比字大一圈的图**。
///
/// 六六 20:42：「CHANGE 贴图出现的时候能发光吗？可能看起来更好看」。运行期叠十几遍同一张图
/// 得到的是硬阴影（NEAREST 采样），真正的"发光"要一条连续的衰减边 —— 所以在这里烘一次，
/// 之后每帧只多画一个 quad。
///
/// 做法：alpha 取出来做两遍**分离式盒式模糊**（半径 `BLUR_R`，两遍叠起来接近三角形核），
/// 再过一条 `a^0.8` 把中间提亮、边缘留长尾，最后整张涂成 `colour`。字本身另画，所以这里
/// 不用保留原色。
pub fn glow_face(rgba: &[u8], w: u32, h: u32, colour: [u8; 3]) -> Option<(Vec<u8>, u32, u32)> {
    // ★ 半径 5 两遍 = σ≈4，光铺出去 9px、把字内部空腔糊住（六六 20:52：「字体看不太清」）。
    //   收到 **2 两遍**（σ≈1.6，出界约 4px）：只在笔画外缘留一道窄边，字的形状不受影响。
    glow_face_spread(rgba, w, h, colour, GLOW_PAD, 2, 0.40)
}

/// 同上，但铺开量与亮度可调 —— 给"大件"用。
///
/// ★ 2026-10-26：货架区那块 `SLOT_LOADING` 的「GAME START LOADING…」是 **720×240 的大字**，
/// 用卷闸 CHANGE 那套（`GLOW_PAD` 6 / 半径 2 / 上限 0.40）只能糊出 4px 窄边，在这么大的字上
/// 等于没有光。所以把三个参数开出来：`pad`（四周留白，贴图比字大 `2·pad`）、`blur`（单遍
/// 盒式模糊半径，跑两遍）、`peak`（光晕 alpha 上限，0..1）。
///
/// 卷闸那条路走的是 [`glow_face`]，参数没变，一个像素都不动。
pub fn glow_face_spread(
    rgba: &[u8],
    w: u32,
    h: u32,
    colour: [u8; 3],
    pad: u32,
    blur: i32,
    peak: f32,
) -> Option<(Vec<u8>, u32, u32)> {
    let blur_r: i32 = blur.max(1);
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    let (w, h) = (w as i32, h as i32);
    let (gw, gh) = (w + 2 * pad as i32, h + 2 * pad as i32);
    // 留白的画布上放 alpha（字摆中间）。
    let mut a = vec![0f32; (gw * gh) as usize];
    for y in 0..h {
        for x in 0..w {
            let src = ((y * w + x) * 4 + 3) as usize;
            let dst = ((y + pad as i32) * gw + (x + pad as i32)) as usize;
            a[dst] = rgba[src] as f32 / 255.0;
        }
    }
    // 两遍分离式模糊：先横后竖，各一遍 —— 再重复一次凑出更长的尾巴。
    let mut tmp = vec![0f32; a.len()];
    for _ in 0..2 {
        for y in 0..gh {
            let row = (y * gw) as usize;
            for x in 0..gw {
                let lo = (x - blur_r).max(0);
                let hi = (x + blur_r).min(gw - 1);
                let mut sum = 0.0;
                for xx in lo..=hi {
                    sum += a[row + xx as usize];
                }
                tmp[row + x as usize] = sum / (2 * blur_r + 1) as f32;
            }
        }
        for x in 0..gw {
            for y in 0..gh {
                let lo = (y - blur_r).max(0);
                let hi = (y + blur_r).min(gh - 1);
                let mut sum = 0.0;
                for yy in lo..=hi {
                    sum += tmp[(yy * gw + x) as usize];
                }
                a[(y * gw + x) as usize] = sum / (2 * blur_r + 1) as f32;
            }
        }
    }
    let mut out = vec![0u8; (gw * gh * 4) as usize];
    for i in 0..(gw * gh) as usize {
        // 亮度和半径一起收：`a^0.9 × 0.9`、上限 `peak` —— 光晕最多只有字本身那么亮的一截，
        // 一眼看得出"字在发光"，但绝不抢字的边缘。
        let v = (a[i].powf(0.9) * 0.9).min(peak.max(0.0));
        out[i * 4] = colour[0];
        out[i * 4 + 1] = colour[1];
        out[i * 4 + 2] = colour[2];
        out[i * 4 + 3] = (v * 255.0).round() as u8;
    }
    Some((out, gw as u32, gh as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn covered_px(out: &[Draw]) -> f32 {
        out.iter()
            .filter_map(|d| match *d {
                Draw::Rect { x, w, h, colour, .. } if x == 0.0 && w == OUT_W as f32 => {
                    (colour[3] > 0.9).then_some(h)
                }
                _ => None,
            })
            .fold(0.0, f32::max)
    }

    #[test]
    fn an_open_shutter_draws_no_curtain_and_a_shut_one_covers_the_panel() {
        // 0 秒：帘子一行都没有（只有顶栏那块板）。
        let mut out = Vec::new();
        draw(at(0.0), None, None, &mut out);
        assert_eq!(covered_px(&out), 0.0, "an open shutter drew a curtain");

        let mut out = Vec::new();
        draw(at(SWAP_AT_S), None, None, &mut out);
        assert!(
            (covered_px(&out) - OUT_H as f32).abs() < 1.0,
            "a shut shutter covers {} of {} px",
            covered_px(&out),
            OUT_H
        );
    }

    #[test]
    fn the_four_beats_run_in_order_and_end_where_they_started() {
        // P1：帘子不动，CHANGE 从 0 淡到 1。
        let p1 = at(P1_S * 0.5);
        assert_eq!(p1.cover, 0.0, "the curtain moved during P1");
        assert!(p1.change > 0.0 && p1.change < 1.0, "CHANGE did not fade in: {p1:?}");
        assert_eq!(at(P1_S).change, 1.0, "CHANGE is not up when P1 ends");

        // P2：帘子放下，顶栏与 CHANGE 都亮着。
        let p2 = at(P1_S + DOWN_S * 0.5);
        assert!(p2.cover > 0.0 && p2.cover < 1.0, "not mid-drop: {p2:?}");
        assert_eq!(p2.bar, 1.0);
        assert_eq!(p2.change, 1.0);

        // P3：满闸，帘片上什么都没有（顶栏跟着走下屏外）。
        let p3 = at(SWAP_AT_S + HOLD_S * 0.5);
        assert_eq!(p3.cover, 1.0);
        assert_eq!(p3.bar, 0.0, "the bar is still on a shut shutter");
        assert_eq!(p3.change, 0.0);

        // P4：卷回，新顶栏淡入；结束回到全开。
        let p4 = at(SWAP_AT_S + HOLD_S + UP_S * 0.5);
        assert!(p4.cover > 0.0 && p4.cover < 1.0, "not mid-rise: {p4:?}");
        assert!(p4.bar > 0.0, "the new bar does not come back with the curtain");
        assert!(at(TOTAL_S).cover < 1e-4, "not open again at the end");
    }

    #[test]
    fn the_cover_never_goes_backwards_while_it_is_dropping() {
        let mut last = 0.0;
        let mut t = P1_S;
        while t < SWAP_AT_S {
            let c = at(t).cover;
            assert!(c >= last, "went backwards at {t}: {c} after {last}");
            last = c;
            t += 1.0 / 60.0;
        }
        assert!((last - 1.0).abs() < 0.02, "the drop did not finish: {last}");
    }

    #[test]
    fn the_one_shut_window_is_where_the_machine_may_change() {
        // 换机种发生在 cover == 1.0 的那一段，那一段必须够长，不然换装可能露馅。
        let mut shut = 0.0;
        let mut t = SWAP_AT_S;
        while t < SWAP_AT_S + HOLD_S {
            if at(t).cover >= 1.0 {
                shut += 1.0 / 60.0;
            }
            t += 1.0 / 60.0;
        }
        assert!(shut > 0.35, "the fully shut window is only {shut}s");
    }

    /// ★★ 2026-10-24 20:53（六六）：「顶栏两侧的圆角你怎么反着来？？」—— 弧的 `d` 取反过，
    /// 画出来是个凹口。钉住方向：**越靠底，收得越多**（弧朝外鼓），且最上面一行几乎不收。
    #[test]
    fn the_bar_corner_bulges_outwards() {
        let r = BAR_R;
        assert!(corner_inset(0, r) < 0.1, "the top of the arc already insets");
        assert!(
            corner_inset(r as usize - 1, r) > r * 0.6,
            "the bottom of the arc is barely rounded: {}",
            corner_inset(r as usize - 1, r)
        );
        let mut last = -1.0;
        for i in 0..r as usize {
            let v = corner_inset(i, r);
            assert!(v >= last, "the arc goes back out at row {i}: {v} after {last}");
            last = v;
        }
    }

    #[test]
    fn change_rides_the_bar_and_is_gone_on_a_bare_shutter() {
        let tex = TexId::from_raw(1);
        for (t, want) in [
            (0.0, false),
            (P1_S * 0.5, true),
            (P1_S + DOWN_S * 0.5, true),
            (SWAP_AT_S + HOLD_S * 0.5, false),
        ] {
            let mut out = Vec::new();
            draw(at(t), Some((tex, 360, 50)), None, &mut out);
            let got = out.iter().any(|d| matches!(d, Draw::Tex { .. }));
            assert_eq!(got, want, "CHANGE at t={t}");
        }
    }
}
