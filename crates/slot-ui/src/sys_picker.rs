//! 机种选择：长按 Y 之后**帘子照旧落下**，满闸后停在原来那 0.4 秒的位置上不走了，
//! 十格机种浮在帘子上等用户指 —— 指完了帘子接着卷回去。
//!
//! # 为什么要有这一块
//!
//! 换机种一直是「帘子落下 → 帘子后面换装 → 帘子卷起」。中途那 0.4 秒满闸（`HOLD_S`）
//! 是留给"换装"的：屏幕看不见，`show_system` 就在这一段跑。
//!
//! 机种只有三台时，长按 Y 直接换到下一台，这一段停留没人会注意。现在卡上排了**十台**，
//! `cycle_system` 那条「按顺序找下一台有卡的」就成了猜谜：按一下跳到哪台，取决于哪几台
//! 有卡、以及上一台是哪台，而用户要的一直是「某一台」。
//!
//! 所以那 0.4 秒不再是一个固定的停顿，而是**一段等你说话的时间**：帘子停住、十格浮上来，
//! 方向键走、A 确认、B 取消，帘子再卷。
//!
//! # 面板画在帘子上，不是盖在货架上
//!
//! 这一点是这块屏的全部：它**不是另一个浮层**，而是原来那 0.4 秒里帘子上多出来的东西。
//! 所以顺序永远是「货架 → 帘子 → 一层纱 → 十格」：帘子已经盖满了整屏，这里只需要把帘子
//! 本身压暗一点，让格子从帘片的花纹里浮出来。（一开始没加这层纱，格子的 `housing`
//! 和帘体是同一档颜色 —— 两者都是"机器壳"，撞在一起就什么都没有了。）
//!
//! # 两态的表达方式
//!
//! 手里的那一格**放大到 120、描边加粗、四周加一圈随主题走的光**，底色从 `recess`
//! 抬到 `edge`。`recess` 而不是 `housing`：`housing` 就是帘子的颜色，而 `recess` 在
//! 两种主题下都比它低一档（"从壳往下退"，这是它本来的语义）。
//!
//! # 标识的两套
//!
//! 卡上给的十张是**白色**的。白标识在深色主题的暗格子里清清楚楚，可亮色主题的格子
//! 是浅塑料（`LIGHT_CASE.housing` 已经是 0xCF），白压白就是没有标识。
//!
//! 所以同一个形状备两份：卡上那张原样用于深色，**烘焙时把 RGB 换成主题的墨色**做第二份
//! 用于亮色。这不是新发明的规矩 —— 卡上本来就同时有 `type_gb.png` 与
//! `type_gb_light.png` 这一对，机种标识从来就是"两套"。
//!
//! 换主题（SELECT+START）那一刻选哪一份是**每帧现读** `palette::mode()` 决定的。
//!
//! # 没卡的机种
//!
//! 照样占着格子 —— 十台是**机器的事实**，不是这张卡的目录。它只是先自己暗下去
//! （图标 alpha 0.30），按 A 会被拒绝并抖一下。藏起来的话，用户会以为这台机器不存在，
//! 而真相是"这张卡上还没放它的游戏"。

use crate::draw::{Draw, TexId, OUT_H, OUT_W};
use crate::palette;
use crate::plate::UndoFace;
use crate::slot_chrome::{ease, edge, recess};

/// 一行五格，两行 —— 十台机器，就是卡上 `root::MACHINES` 的那十台。
pub const COLS: usize = 5;
pub const ROWS: usize = 2;
pub const COUNT: usize = COLS * ROWS;

/// 一格静止时的边长。
pub const BUTTON: f32 = 100.0;
/// 手里那一格的边长。
pub const BUTTON_HOT: f32 = 120.0;

/// 圆角按边长取固定比例，所以放大只是同一个形状变大，不是另一个形状。
const RADIUS_SHARE: f32 = 0.16;
/// 格与格的中心距。横向 136 让 120 的两格之间还剩 16 —— 放大时不会咬到邻居。
const PITCH_X: f32 = 136.0;
/// 行距。两行都放大时仍有 30 的空隙，读得出是两行。
const PITCH_Y: f32 = 150.0;
/// 第一行的中心 y。上面留给标题，下面留给那行键位提示。
///
/// ★★ 2026-10-27（六六）：**两行按钮整体垂直居中**。行距 150、两行 ⇒ 网格中心 =
/// `FIRST_CY + 75`，要它落在屏中心（240）上，`FIRST_CY = 165`。改之前是 205 —— 两行
/// 中心落在 280，比屏中心低 40 px，读作"整块板往下坠"。
const FIRST_CY: f32 = 165.0;

/// 某一行的中心 y —— **由 `PITCH_Y` 算出来，不是写死两个数**：两行之间的距离就是
/// 这里那一个常量，改了它不可能只改了一半。
fn row_cy(row: usize) -> f32 {
    FIRST_CY + (row.min(ROWS - 1)) as f32 * PITCH_Y
}

/// 标题的中心 y。
///
/// ★★ 2026-10-27（六六）：标题从渲染文字换成卡上的**贴图**（`SLOT_SELECT.png`，720×120），
/// 中心 y 从 92 提到 60 —— 贴图全高 120，中心 60 就是**顶部对齐**（`y = 60 - 60 = 0`），
/// 内容只占中间那 39 px，底边在 y≈78，离第一行格子顶（165-50=115）还有 37 px。
const TITLE_CY: f32 = 60.0;
/// 键位提示的中心 y。
const FOOT_CY: f32 = 446.0;

/// 图标在格子里占的比例。留出的边是让标识不贴着按钮的圆角。
const ICON_SHARE: f32 = 0.62;

/// 图标该按多大预缩 —— 前端烘贴图时问这一句，画的时候再缩到当前格子的尺寸。
///
/// 按**放大态**的尺寸给：缩小的那一路（120→100 的按钮）由 GPU 的线性插值做，几乎无损；
/// 放大的那一路（源 120 画到 120 上下）才是会糊的那一边，所以源图就取这里的尺寸。
pub fn icon_px() -> u32 {
    (BUTTON_HOT * ICON_SHARE).round() as u32
}

/// 光晕从放大后的按钮边缘再向外扩多少像素。
pub const GLOW_PAD: f32 = 16.0;

/// 一格的中心，按 `i` 在 5×2 网格里的位置。
pub fn centre(i: usize) -> (f32, f32) {
    let col = (i % COLS) as f32;
    let row = i / COLS;
    // 整片网格横向居中：五格连同四个间距一起落到屏幕正中。
    let left = (OUT_W as f32 - (COLS as f32 - 1.0) * PITCH_X) / 2.0;
    (left + col * PITCH_X, row_cy(row))
}

/// 手里那一格现在多大。
pub fn size_of(hot: f32) -> f32 {
    BUTTON + (BUTTON_HOT - BUTTON) * hot.clamp(0.0, 1.0)
}

/// 方向键走一步。**两个方向都环回**：这十格是一个圈，走到边角再按一下回到对面那一格，
/// 比「卡在边上、要用户自己记住按反方向」好 —— 环回对着 `core_picker` 的两格是同一条
/// 理由（任何一边按都是另一边的反操作）。
pub fn step(cursor: usize, dx: i32, dy: i32) -> usize {
    let col = (cursor % COLS) as i32;
    let row = (cursor / COLS) as i32;
    let col = (col + dx).rem_euclid(COLS as i32);
    let row = (row + dy).rem_euclid(ROWS as i32);
    (row as usize) * COLS + col as usize
}

/// 一格按钮的底。**两个态是同一张 120 的图**，画的时候缩到 `size_of(hot)`：
/// 一张 100 的、一张 120 的会让过渡变成两张图交叉淡入，圆角的弧线在那儿会抖一下。
///
/// 静止用 `recess()`、手里用 `edge()` —— **不是** `housing`。`housing` 正是帘体的颜色
/// （帘片就是 `housing` 与 `opening` 的混合），格子用它就等于没画。`recess` 在两种主题下
/// 都比 `housing` 低一档（"从壳往下退"是它本来的语义），`edge` 都比它高一档。
pub fn button_face(hot: bool) -> UndoFace {
    let n = BUTTON_HOT.round() as u32;
    let mut rgba = vec![0u8; (n as usize) * (n as usize) * 4];
    let radius = BUTTON_HOT * RADIUS_SHARE;
    let rim_px = if hot { 2.0 } else { 1.0 };
    let (fill, rim) = if hot {
        (edge(), palette::chosen_rim())
    } else {
        (recess(), edge())
    };
    crate::rounded::plate(&mut rgba, n, n, radius, rim_px, fill, Some(rim));
    // 模压感与菜单键同一套（顶亮底暗 + 内棱），所以这块屏和菜单看起来是一台机器上的东西。
    crate::menu::mould(&mut rgba, n, n, radius, rim_px);
    UndoFace { rgba, w: n, h: n }
}

/// 圆角矩形的有向距离：形状内为负、边界为零、外面为它到边界的距离。
///
/// 光晕要的是「按钮边上最亮、往外 16 px 收干净」，所以它必须知道离形状**多远**，
/// 而不只是「在不在里面」。
fn outside_distance(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let qx = (x - w * 0.5).abs() - (w * 0.5 - r);
    let qy = (y - h * 0.5).abs() - (h * 0.5 - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

/// 手里那一格四周的光。
///
/// 一张 `BUTTON_HOT + 2·GLOW_PAD` 见方的图，中间那块（就是按钮本身）**全透明**，
/// 外面由边界处的最亮衰减到零。画的时候整张跟着 `size_of` 一起缩，所以光永远贴在那条
/// 圆角边上，不会在放大缩小时离开它。
///
/// 颜色取 `palette::ink()` —— **主题里最"跳"的那一个**（深色主题是近白，亮色主题是墨）。
/// 用 `edge()` 试过：它在深色主题下只是中灰（`#4D4D57`），压在近黑的板上一圈几乎看不出来，
/// 而"边缘亮呼吸"正是这一格唯一的动效。换成 ink 之后暗色主题是一圈真正的亮边；
/// 亮色主题那一边由 `light::gain()` 收到 0.42，读作一层淡淡的深晕 —— 白纸上没有光，
/// 能做的只有把边压深一点。
///
/// 呼吸只改这张图的 alpha（`light::breathe`），不改它的形状：光的形状是几何，不是时间。
pub fn glow_face() -> (u32, u32, Vec<u8>) {
    let n = (BUTTON_HOT + 2.0 * GLOW_PAD).round() as u32;
    let mut rgba = vec![0u8; (n as usize) * (n as usize) * 4];
    let c = palette::ink();
    let (r, g, b) = (c[0], c[1], c[2]);
    let radius = BUTTON_HOT * RADIUS_SHARE;
    for y in 0..n {
        for x in 0..n {
            let px = x as f32 + 0.5 - GLOW_PAD;
            let py = y as f32 + 0.5 - GLOW_PAD;
            let d = outside_distance(px, py, BUTTON_HOT, BUTTON_HOT, radius);
            if d <= 0.0 {
                continue;
            }
            // `t^1.5` 而不是 `t^2`：平方衰减把紧贴边缘的那几像素也拉下去了，而"边在发光"
            // 靠的正是紧贴边缘的那几像素。1.5 次方让最内侧一格保持接近全亮，最外一格收回零。
            let t = 1.0 - (d / GLOW_PAD).clamp(0.0, 1.0);
            let a = (t.powf(1.5) * 255.0) as u8;
            if a == 0 {
                continue;
            }
            let i = ((y * n + x) * 4) as usize;
            rgba[i] = r;
            rgba[i + 1] = g;
            rgba[i + 2] = b;
            rgba[i + 3] = a;
        }
    }
    (n, n, rgba)
}

/// 一个机种的格子：它的标识，以及这张卡上它有没有游戏。
pub struct Cell {
    /// 同一个形状的两份：深色主题用的白标识（卡上那张原样）与亮色主题用的墨标识。
    /// `None` 表示卡上没这张贴图 —— 那一格仍然占位，只是画成一块空的圆角格子。
    pub icon: Option<(Icon, Icon)>,
    /// 这一台在卡上一张卡都没有。
    pub empty: bool,
}

/// 一张已经上传的贴图：句柄，以及它自己的像素尺寸。
pub type Icon = (TexId, u32, u32);

/// 把一张标识染成墨色 —— 亮色主题的那一份就是这么来的。
///
/// 只换 RGB，**一个 alpha 都不动**：形状是卡上的图给的，这份代码只负责"同一条笔画，
/// 换成这台机器上的墨"。白色像素染完是墨色，抗锯齿留下的半透明边缘也跟着变成半透明的墨 ──
/// 所以它在浅塑料上仍然是一条边缘干净的笔画，不是一块毛边色块。
pub fn ink_icon(rgba: &[u8], ink: [u8; 3]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for px in out.chunks_exact_mut(4) {
        if px[3] == 0 {
            continue;
        }
        px[0] = ink[0];
        px[1] = ink[1];
        px[2] = ink[2];
    }
    out
}

/// ★★ 2026-10-27（六六）：机种面板**标题**用卡上的贴图 `System/Carts/SLOT_SELECT.png`。
///
/// 用户给的图是**品红标记**（`#FF00FF`，像素字 "SYSTEM SELECT"）—— 颜色只是占位，真正要的
/// 是那条笔画形状。所以这里把 RGB 全换成主题墨色（`ink`：深色=近白、亮色=墨），alpha 一个
/// 不动 —— 与 [`ink_icon`] 同一套"形状是卡上的、颜色是这台机器的"逻辑。
///
/// 返回原尺寸 `(rgba, w, h)`（720×120，画的时候 1:1，不需要预缩 —— 它不是缩到格子里，
/// 而是全宽铺在顶部）。卡上没这张图就 `None`，调用方回退到渲染文字（老卡兼容）。
pub fn title_face(dir: &std::path::Path, ink: [u8; 3]) -> Option<(Vec<u8>, u32, u32)> {
    let (rgba, w, h) = crate::decode_rgba(&dir.join("SLOT_SELECT.png"))?;
    Some((ink_icon(&rgba, ink), w, h))
}

/// 这一块屏要用的、开机烘好就不再变的脸。
pub struct Faces {
    /// 静止那一格的底（120 见方，画的时候缩）。
    pub idle: (TexId, u32, u32),
    /// 手里那一格的底。
    pub hot: (TexId, u32, u32),
    /// 那一圈光。
    pub glow: (TexId, u32, u32),
    /// 标题。
    pub title: Option<(TexId, u32, u32)>,
    /// 底部的键位提示，从左到右。
    pub foot: Vec<(TexId, u32)>,
}

impl Faces {
    /// 一张脸都没烘好时（前端还没轮到这一批）不画 —— 一块空纱比半块屏更难看。
    pub fn ready(&self) -> bool {
        self.title.is_some() || !self.foot.is_empty()
    }
}

/// 一帧的读数。
pub struct View<'a> {
    /// 手里那一格。
    pub cursor: usize,
    /// 上一格。移动的那 180 ms 里它的放大在退回去。
    pub prev: usize,
    /// 那一次移动走了多少，0..1。1.0 = 已经落定。
    pub moved: f32,
    /// 整块屏的不透明度（开与关各一段）。
    pub alpha: f32,
    /// 呼吸值，0.55..1.0，来自 `light::breathe`。
    pub breathe: f32,
    /// 手里那一格横着偏多少像素 —— 选了台没卡的机种时抖的是**这一格**，不是整屏。
    /// 与核心选择器同一条规矩：抖的必须是"被拒绝的那个东西"，两个一起抖读成两次失败。
    pub shake: f32,
    pub cells: &'a [Cell],
}

/// 一格现在被放大到多少。
///
/// 移动时**两格同时动**：新的一格从 100 长到 120，旧的一格从 120 收回去。只让新的长
/// 就会在旧的那格上突然出现一个「两个都小」的中间帧，看着像光标的框闪了一下。
pub fn hot_of(view: &View, i: usize) -> f32 {
    if i == view.cursor {
        ease(view.moved.clamp(0.0, 1.0))
    } else if i == view.prev {
        1.0 - ease(view.moved.clamp(0.0, 1.0))
    } else {
        0.0
    }
}

/// 画这一整块屏 —— **画在帘子之上**。
///
/// 顺序是**纱 → 标题 → 九格 → 手里那一格 → 提示**。纱压的是帘子本身（帘子已经把货架盖住
/// 了），目的是让格子从帘片花纹里浮出来，不是再盖一层浮层。手里的那格最后画，因为它在过渡
/// 中会压到邻居的边上，而「谁盖谁」在 180 ms 里必须一直说同一件事。
pub fn draw(faces: &Faces, view: &View, out: &mut Vec<Draw>) {
    let a = view.alpha.clamp(0.0, 1.0);
    if a <= 0.0 || !faces.ready() {
        return;
    }
    // 把帘子压下去一档。**不是"再盖一层浮层"**：帘子已经盖满了，这层纱只是让十格从
    // 帘片的花纹里浮出来 —— 少了它，格子的 `recess` 和帘体是同一档颜色（都是机器壳）。
    let s = palette::scrim();
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        colour: [s[0], s[1], s[2], s[3] * a],
    });
    if let Some((tex, w, h)) = faces.title {
        // ★★ 2026-10-27（六六）：标题**呼吸光效**。把灯那条 0.55..1.0 的曲线重映射到
        // 0.75..1.0 —— 标题是字不是灯，压到 0.55 会读成闪。起伏 0.25 仍然看得见，但它是
        // "亮着的字在呼吸"，不是一颗在闪的灯。
        let tb = 0.75 + 0.25 * ((view.breathe - 0.55) / 0.45).clamp(0.0, 1.0);
        out.push(Draw::Tex {
            x: ((OUT_W as f32 - w as f32) / 2.0).round(),
            y: (TITLE_CY - h as f32 / 2.0).round(),
            w: w as f32,
            h: h as f32,
            tex,
            alpha: a * tb,
        });
    }
    let n = view.cells.len().min(COUNT);
    // 两遍：先没有光标的九格，再手里这一格。这样甚至不需要给「谁该在上面」写特例 ——
    // 它只是最后一个画的。
    for pass in 0..2 {
        for i in 0..n {
            let is_cursor = i == view.cursor;
            if (pass == 1) != is_cursor {
                continue;
            }
            draw_cell(faces, view, i, a, out);
        }
    }
    // 底部的键位提示：一排，整体居中。
    if !faces.foot.is_empty() {
        let gaps = (faces.foot.len() - 1) as f32 * crate::plate::HINT_GAP;
        let total: f32 = faces.foot.iter().map(|(_, w)| *w as f32).sum::<f32>() + gaps;
        let mut x = (OUT_W as f32 - total) / 2.0;
        for (tex, w) in &faces.foot {
            out.push(Draw::Tex {
                x: x.round(),
                y: (FOOT_CY - crate::plate::HINT_H as f32 / 2.0).round(),
                w: *w as f32,
                h: crate::plate::HINT_H as f32,
                tex: *tex,
                alpha: a,
            });
            x += *w as f32 + crate::plate::HINT_GAP;
        }
    }
}

fn draw_cell(faces: &Faces, view: &View, i: usize, a: f32, out: &mut Vec<Draw>) {
    let Some(cell) = view.cells.get(i) else {
        return;
    };
    let hot = hot_of(view, i);
    let size = size_of(hot);
    let (mut cx, cy) = centre(i);
    // 拒绝的那一下只推手里这一格，而且只推横向 —— 竖向推会让它看起来要离开网格。
    if i == view.cursor {
        cx += view.shake;
    }
    let x = (cx - size / 2.0).round();
    let y = (cy - size / 2.0).round();
    // 光在底下面：它是从格子边上漏出来的，不是压在格子上的一层雾。
    if hot > 0.0 {
        let g = size + 2.0 * GLOW_PAD;
        out.push(Draw::Tex {
            x: (cx - g / 2.0).round(),
            y: (cy - g / 2.0).round(),
            w: g.round(),
            h: g.round(),
            tex: faces.glow.0,
            alpha: hot * view.breathe * crate::light::gain() * a,
        });
    }
    // 两张底按 `hot` 交叉淡入 —— 尺寸相同，所以交叉的是颜色，形状一直在同一个地方。
    if hot < 1.0 {
        out.push(Draw::Tex {
            x,
            y,
            w: size.round(),
            h: size.round(),
            tex: faces.idle.0,
            alpha: (1.0 - hot) * a,
        });
    }
    if hot > 0.0 {
        out.push(Draw::Tex {
            x,
            y,
            w: size.round(),
            h: size.round(),
            tex: faces.hot.0,
            alpha: hot * a,
        });
    }
    // 标识：按当前主题选那一份。**每帧现读**，所以面板开着切主题也不会留着上一套。
    let icon = match cell.icon {
        Some((dark, light)) => Some(match palette::mode() {
            palette::Mode::Dark => dark,
            palette::Mode::Light => light,
        }),
        None => None,
    };
    if let Some((tex, _, _)) = icon {
        let icon = (size * ICON_SHARE).round();
        out.push(Draw::Tex {
            x: (cx - icon / 2.0).round(),
            y: (cy - icon / 2.0).round(),
            w: icon,
            h: icon,
            tex,
            // 没卡的机种先自己暗下去：它的标识还在（这台机器存在），但它不是能去的地方。
            alpha: if cell.empty { 0.30 * a } else { a },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 十格必须铺满整屏且左右对称 —— 五个中心关于 360 镜像。
    #[test]
    fn the_grid_is_centred() {
        let half = OUT_W as f32 / 2.0;
        for row in 0..ROWS {
            let first = centre(row * COLS).0;
            let last = centre(row * COLS + COLS - 1).0;
            assert!(
                ((first + last) / 2.0 - half).abs() < 0.01,
                "row {row}: {} .. {} is not centred on {half}",
                first,
                last
            );
        }
        // 上下两行同样横向对齐，否则第二行看起来会歪。
        assert_eq!(centre(0).0, centre(COLS).0);
    }

    /// 放大那一格不许咬到邻居：中心距减去两个半宽，还剩得下一条缝。
    #[test]
    fn a_lit_button_clears_its_neighbours() {
        assert!(PITCH_X - BUTTON_HOT > 0.0, "columns overlap when lit");
        assert!(PITCH_Y - BUTTON_HOT > 0.0, "rows overlap when lit");
        // 而且留得出来的缝要够放那一圈光，否则光会糊到邻居的描边上。
        assert!(PITCH_X - BUTTON_HOT >= 8.0);
    }

    /// 网格是一圈：四个方向各按一下都还在十格里，走到边上会绕回去。
    #[test]
    fn the_cursor_wraps_both_ways() {
        assert_eq!(step(0, -1, 0), COLS - 1, "left off the first column wraps");
        assert_eq!(step(COLS - 1, 1, 0), 0, "right off the last column wraps");
        assert_eq!(step(0, 0, -1), (ROWS - 1) * COLS, "up off the first row wraps");
        assert_eq!(step((ROWS - 1) * COLS, 0, 1), 0, "down off the last row wraps");
        // 右上角往右：留在第一行，只是回到行首。
        assert_eq!(step(COLS - 1, 1, 0), 0);
        for i in 0..COUNT {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                assert!(step(i, dx, dy) < COUNT, "step left the grid from {i}");
            }
        }
    }

    /// 每一格都落在屏内，连放大后的那一圈光也算上。
    #[test]
    fn every_cell_is_on_screen() {
        for i in 0..COUNT {
            let (cx, cy) = centre(i);
            let reach = BUTTON_HOT / 2.0 + GLOW_PAD;
            assert!(cx - reach >= 0.0 && cx + reach <= OUT_W as f32, "cell {i} off the sides");
            assert!(cy - reach >= 0.0 && cy + reach <= OUT_H as f32, "cell {i} off the top or bottom");
        }
    }

    /// 静止与放大真的是两个尺寸，而且中间的过渡是连续的。
    #[test]
    fn the_lit_size_is_bigger() {
        assert_eq!(size_of(0.0), BUTTON);
        assert_eq!(size_of(1.0), BUTTON_HOT);
        assert!(size_of(0.5) > BUTTON && size_of(0.5) < BUTTON_HOT);
    }

    /// 光晕：形状里面一个像素都不许亮（亮在格子里就是一块盖住标识的色块），
    /// 形状外面立刻亮，最远处收干净。
    #[test]
    fn the_glow_ring_leaves_the_button_alone() {
        let (w, h, rgba) = glow_face();
        assert_eq!(w, (BUTTON_HOT + 2.0 * GLOW_PAD) as u32);
        assert_eq!(h, w);
        let pad = GLOW_PAD as u32;
        // 正中、以及形状里面靠边的一圈：全透明。
        let at = |x: u32, y: u32| rgba[((y * w + x) * 4 + 3) as usize];
        assert_eq!(at(w / 2, h / 2), 0, "the middle of the button must be clear");
        assert_eq!(at(pad + 2, h / 2), 0, "just inside the left edge must be clear");
        // 紧贴形状外面：亮。
        assert!(at(pad - 1, h / 2) > 200, "the ring must be bright at the edge");
        // 四角外面那一点点：已经收干净（圆角把角让出去了）。
        assert_eq!(at(0, 0), 0, "the corner must be fully faded out");
    }
}
