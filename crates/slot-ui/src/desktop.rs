//! The shelf's desktop frame, rasterised in code instead of shipped as a `System/Desktop.png`.
//!
//! A rounded-rect window is cut out of a solid frame: twenty pixels of frame at the top, ten
//! at the left and right, and fifty at the bottom — the bar the carts tuck into. Two pieces
//! are cut or added on top of that:
//!
//! * The **nameplate** hangs off the top of the screen and is where the row's title is
//!   printed — 360 wide, 54 deep, its sides straight and its bottom closed over by a round
//!   corner, filled in the slot wall's own colour and outlined in the machine's highlight.
//! * The **card slot** is a 360-wide notch in the bottom bar, cut clear so the shelf's own
//!   carts tuck into it. Its two side walls have the **reverse-S** profile of 2026-10-24: a
//!   small arc that flares the mouth outward, a straight run, then a big arc that closes the
//!   seat in — the two arcs curve *opposite ways*, which is what makes the edge read as an S
//!   rather than as a plain bracket.
//!
//! Everything but the window and the slot is opaque; the rest of the shelf draws over it (the
//! letter index, the clock, the title, the battery).
//!
//! **The geometry here is measured, not guessed.** Every number below was read off a
//! reference drawing of this frame — the alpha channel row by row, and the colours off the
//! pixels — and the comments say which measurement each one came from. Guessing this shape is
//! what went wrong before: a frame that is "about right" reads as broken, because the eye
//! sees the difference between a plate that hangs off the screen and one that floats below
//! the top rail, and between a slot that opens upward into the bar and one cut off at the
//! foot of the screen.
//!
//! **The colour is the machine's own plastic** (2026-10-23, 六六): the board is painted in
//! `slot_chrome::housing()` — the shell's dark grey in the dark mode, the light grey in the
//! light one, and the look's own plastic when a palette is up, the same rule every other
//! plastic on the machine follows. The board and the slot chrome that appears around a cart
//! being inserted are therefore one piece rather than two. The texture is the **cut edge**:
//! two pixels of lit chamfer where the window is cut out of the board, the same treatment
//! the nameplate's own edges have. The nameplate and the slot's six pixels are the same
//! colour — the inside of the hole (`opening`), one piece of plastic rather than two — and
//! the plate carries a one-pixel outline in the machine's highlight besides.

use std::f32::consts::{FRAC_PI_2, PI};

use slot_gfx::{OUT_H, OUT_W};

use crate::slot_chrome;

// ── The card slot's own geometry ────────────────────────────────────────────────────────────
//
// ★★★ 2026-10-24（六六）：槽口上下两角是**同一个内收圆角，关于槽口中线互为镜像**。
// 他拿手绘钉死的（上排红叉 = 现在的尖刺，下排绿勾 = 这个圆角保持不动）：
// ```text
//        ╭──────────────╮        ← 顶角：圆心在洞内，顶边比直壁窄 R
//        │              │        ← 中间 8 行直壁，两条竖边始终垂直
//        ╰──────────────╯        ← 底角：同一个圆角，镜像
// ```
// 两角都取「圆心在洞**内**」那一支：顶角圆心 `(X0 + R, Y0 + R)`、底角圆心 `(X0 + R, Y1 − R)`，
// 弧的中点都朝洞心收 ⇒ 顶边与底边各比直壁**窄 `R`**，两条弧的两端都**相切**于直壁，全程
// 没有折角。左右两条竖边因此从头到尾是一条直线 —— 这就是「竖边还是垂直的」。
//
// ⚠️ **顶角不许再「外扩」。** 上一版顶角是圆心在洞外的外扩弧（洞口比直壁宽 9px），洞口的
// 顶边就横出直壁之外，六像素的壁在顶边被切断、收口成两根**尖刺** —— 他连着两轮说的
// 「尖尖」就是它。**判据是这条：洞的左右边界永远不许超出直壁 `X0` / `X1`。**
/// 直壁围出来的矩形：上下两角各内收 `SLOT_CORNER_R` 之后，中间剩一条直壁。
///
/// ★★ 2026-10-29（640 树）：**洞宽 360 一点没变，只是居中换了个地方。**
/// 720 上 `(720−360)/2 = 180`；本树 `(640−360)/2 = 140`。
/// 宽度为什么不能跟着缩：六六给的数是**横版卡带收束成 360 宽**，卡带宽是卡带的属性、
/// 不是屏幕的属性 —— 所以洞必须还是 360，否则卡带一插就对不上（这条 2026-10-24 定的）。
/// 面板窄了 80px，代价全部由两侧的塑料承担，洞本身一个像素都不动。
const SLOT_X0: f32 = 140.0;
const SLOT_X1: f32 = 500.0;
const SLOT_Y0: f32 = 430.0;
const SLOT_Y1: f32 = 474.0;

/// ★★ 2026-10-24（六六）：**插卡动画改绑到这个开口**，不再绑 `slot_chrome` 里那条程序化的
/// 底条。理由是他给的数：横版卡带收束成 **360 宽**，而这里的开口正好也是 360
/// （`SLOT_X1 - SLOT_X0`）。两套几何本来各画各的，卡带一插就对不上，所以现在只剩这一套 ——
/// 洞在哪里，卡带就插到哪里。
pub const SLOT_OPEN_X: f32 = SLOT_X0;
pub const SLOT_OPEN_W: f32 = SLOT_X1 - SLOT_X0;
pub const SLOT_OPEN_Y: f32 = SLOT_Y0;
pub const SLOT_OPEN_H: f32 = SLOT_Y1 - SLOT_Y0;
/// 底部两角的圆角半径（「下圆角」）。
const SLOT_CORNER_R: f32 = 18.0;
/// 口部两角的外扩量：**两张弧「相对」`)(`** —— 弧凸向洞外，洞口在口部比直壁**宽 `R`**。
/// 六六的原话：「我要这两个角是对称的圆弧边！圆弧是对着的，）（ 这样！！！！不是（ ）这样」。
/// 圆心因此在洞**外**：左 `(X0 − R, Y0 + R)`、右 `(X1 + R, Y0 + R)`。
const SLOT_MOUTH_R: f32 = 12.0;
/// The six pixels of board round the hole: the hole's outline pushed out by this much.
const SLOT_WALL: f32 = 6.0;
/// The box `fill` has to look inside before asking about the hole. **It must be the hole's
/// outline grown by `SLOT_WALL` on every side.** 洞再也不会比直壁宽，所以这里就是直壁外扩
/// 六像素；写成常量而不是照着洞最宽处抄，是为了让「不外扩」这条自己看得见。
/// ⚠️ 口部外扩之后洞口比直壁宽 `SLOT_MOUTH_R`，盒子必须跟着放宽，否则外扩那一段
/// 会「有洞没有壁」。
const SLOT_BOX_X0: f32 = SLOT_X0 - SLOT_MOUTH_R - SLOT_WALL;
const SLOT_BOX_X1: f32 = SLOT_X1 + SLOT_MOUTH_R + SLOT_WALL;
/// ★★★ 2026-10-24（六六）：「插卡位挖孔是一个**上直角下圆角**的矩形」——
/// 洞的上沿与板子顶边（= 窗口底边 y 430）在同一条线上，所以上面两个角必须是**直角**：
/// 圆角会在那条线上**相切**，相切处的 AA 渐变被拉成一根细长的尖（「你怎么又变回尖尖了」）。
/// 上角是直角 ⇒ 两侧的壁是一条**竖直的 6px 带**，到顶边平头收口，不会斜挂出尖。
const SLOT_BOX_Y0: f32 = SLOT_Y0;
/// The foot of the screen: the wall under the hole runs out of rows there, not before.
const SLOT_BOX_Y1: f32 = 480.0;

/// Rasterise the desktop frame into a 720×480 RGBA buffer, opaque where the frame is and clear
/// in the window and the card slot, with the nameplate hanging from the top.
pub fn desktop_rgba() -> Vec<u8> {
    let w = OUT_W as usize;
    let h = OUT_H as usize;
    let mut buf = vec![0u8; w * h * 4];

    // ── The frame's own colour ───────────────────────────────────────────────────────────────
    // ★ 2026-10-23（六六）：「暗色模式底色还是按原来的深灰色，亮色模式按浅灰色」——
    //   板子是**机器自己的塑料**，不是一块自调色的灰。`housing()` 暗色下是机壳的深灰、
    //   亮色下是浅灰，色板（look）激活时整台机器的塑料归色板所有 —— 和插卡时出现的机种框
    //   同一种塑料，整台机器读作一件。**一个色铺满整块板**（整体渲染那轮的结论没变）：
    //   板上不做渐变，质感全在割口那道亮边（见下面的 chamfer）。
    let frame = bytes(slot_chrome::housing());
    // 插卡位那圈 6px 壁 = 洞里头看见的色（`opening`）：暗色下近黑、亮色下灰。
    //   六六早先「插卡位要深色点」的系统化落点 —— 深色不是另调一个灰，是 opening 自己。
    let slot = bytes(slot_chrome::opening());
    // 窗口割口的亮边 = 机器自己的拟真高光（`rim`：色相留色板的、明度夹回默认那一档，
    //   鲜亮色板的高光线也不会刺到像画上去的边框）。
    let lit = bytes(slot_chrome::rim());
    // How wide the lit chamfer along the window's cut is, in pixels.
    const CHAMFER_W: f32 = 2.0;
    // The plate's charcoal, and the one-step-lighter line along its own edges.
    // ★ 2026-10-23（六六）：「画面上方的名称背板颜色是和插卡位的边一致的，而且需要渲染一下
    //   轮廓线」—— 背板不再是独立炭黑件：**填色 = 插卡位壁同一个 `opening`**（暗色近黑、
    //   亮色灰，随主题/色板走），背板与槽壁因此读作一块塑料。倒角端点全部从机器的色派生
    //   （侧边从 `rim` 淡到背板色、顶/底从四成混合淡到背板色），外缘再勾 1px 轮廓线
    //   （`rim`）—— 与窗口割口亮边同一套词汇。
    // The plate's own fill: the slot wall's colour, so the two read as one piece.
    let tab = slot;
    // The plate's side bevel runs to the machine's own highlight, as the window's cut does.
    let tab_lit = lit;
    // The plate's cap bevel runs to a two-fifths mix of the two — shorter, and fainter.
    let tab_edge = mix(slot, lit, 0.4);
    // How wide the plate's chamfer is, in pixels.
    const TAB_CHAMFER: f32 = 5.0;
    // How wide the plate's bevel across its top and bottom is — shorter, and fainter.
    const TAB_CAP: f32 = 2.0;
    // The outline drawn round the plate's outside, in the machine's highlight.
    const TAB_OUTLINE: f32 = 1.0;
    // How solid the slot's hole is. **Nothing**: the hole is cut clear, so the cart in it
    // shows through undimmed and the clock printed on the bar reads straight across it.
    const SLOT_ALPHA: f32 = 0.0;

    // ── The frame ────────────────────────────────────────────────────────────────────────────
    // Measured off the device's own rendering of the drawing: the window's cut-out starts at
    // y 10 and x 10, and its corners are round enough to take fourteen pixels to open. The
    // bottom bar is the last fifty rows.
    const TOP: f32 = 10.0;
    const SIDE: f32 = 10.0;
    const BOTTOM: f32 = 50.0;
    // The window's corner radius.
    const WINDOW_R: f32 = 14.0;
    // `rounded_rect` 把 x1/y1 算作**窗口内的最后一列/行**（闭区间）—— 所以不减 1：
    // x1 = 720-10 = 710 时，窗口占满 x10..709 共 700 列，右轨 x710..719 正好 10px，
    // 和左轨 x0..9 对称。减 1 反而会弄成 11px（闭区间与半开区的差别，踩过一次）。
    let (hx0, hy0, hx1, hy1) = (SIDE, TOP, OUT_W as f32 - SIDE, OUT_H as f32 - BOTTOM);

    // ── The card slot ───────────────────────────────────────────────────────────────────────
    // **A second hole in the same board, with the board's own six pixels around it.**
    //
    // The board has the window cut out of it — seven hundred by four hundred and twenty, so
    // ten pixels of board on the top, the left and the right, and fifty at the bottom. At the
    // foot of that cut-out, centred, the slot is cut as well: three hundred and sixty wide by
    // forty-four deep, with the two mirrored corners the constants above spell out.
    //
    // And around that hole, on its left, its right and its foot, there are **six pixels of
    // board**: the same plastic as the rest of it, the same colour, catching the same light.
    // It is the board showing through around the hole, not a rim drawn in another colour —
    // which is the thing that made the reference read as two pieces rather than one.
    // The slot's profile is the **mirrored pair** above (`SLOT_CORNER_R`): one inward corner at
    // the mouth, the same one again at the seat, and a straight vertical wall between them.
    // 洞的左右边界从不越过直壁，所以那两根尖刺不可能再出现。

    // ── The nameplate ───────────────────────────────────────────────────────────────────────
    // The plate is a block that hangs off the top of the screen: the full 360 wide, its top
    // two corners square where it meets the screen edge, its two free bottom corners rounded
    // with a clean quarter-circle each. A square foot reads as a bar; the round foot is what
    // reads as a nameplate.
    // ★ 2026-10-24（六六）：顶栏（原「名称栏」）**400 宽** = 左 40 信息 / 中 320 名称 / 右 40 信息，
    //   所以 x160..559；仍是 y0..53（54 深）。
    // ★★ 2026-10-29（640 树）：宽度 400 是**内容**决定的（40+320+40，六六自己给的切法），
    //   不是屏幕决定的 —— 所以它也不缩，只在 640 里重新居中：`(640−400)/2 = 120`。
    //   内部三段的比例原样保留，`plate.rs` 的 `plate_fav_x`/`plate_battery_x` 全部由此派生。
    const NP_X0: f32 = 120.0;
    const NP_X1: f32 = 520.0;
    const NP_H: f32 = 54.0;
    // The plate's bottom corners: a clean quarter-circle each — the top two corners stay
    // square where the plate meets the screen edge, only the bottom two are rounded.
    const NP_R: f32 = 16.0;

    let (nx0, nx1) = (NP_X0, NP_X1);
    let (ny0, ny1) = (0.0, NP_H);

    // Which colour owns this point. Order is the priority: the slot's solid block, then the
    // hole cut out of it, then the nameplate over the top rail and the window's top edge,
    // then the window itself, and everything left is the frame.
    enum Fill {
        None,
        Frame,
        Tab,
        Slot,
        SlotFloor,
    }
    let fill = |px: f32, py: f32| -> Fill {
        // 洞 = 圆角矩形的内部；壁 = 同一条轮廓**等距外扩 `SLOT_WALL`** 的那一圈。
        //
        // ★★★ 2026-10-24（六六）：「你怎么又变回尖尖了」—— 问题不在洞，在**壁**。
        //   原来是拿「洞向左/右/上/下各问 6px」拼出来的（曼哈顿膨胀），在口部两角，
        //   那个「下 6px」的采样会落在更宽的一行上，壁因此在顶边鼓出一块、外轮廓收成
        //   一个**斜尖角**。改成距离场之后，壁就是真·6px 等距带，外轮廓是**圆弧**。
        if px >= SLOT_BOX_X0 && px <= SLOT_BOX_X1 && py >= SLOT_BOX_Y0 && py <= SLOT_BOX_Y1 {
            let d = slot_sdf(px, py);
            if d < 0.0 {
                return Fill::Slot; // the hole itself: cut clear, so the cart shows through
            }
            // 壁 = `0 ≤ d ≤ SLOT_WALL` 的**等距带**，跟着洞的圆角走（口部 12、底部 18）。
            if d <= SLOT_WALL {
                return Fill::SlotFloor; // the six pixels of wall round it, in shadow
            }
        }
        if round_bottom_rect(px, py, nx0, ny0, nx1, ny1, NP_R) {
            return Fill::Tab; // the nameplate, overprinting the rail and the window
        }
        if rounded_rect(px, py, hx0, hy0, hx1, hy1, WINDOW_R) {
            return Fill::None; // inside the window
        }
        Fill::Frame // inside the frame
    };

    // The signed distance to the window's cut, for the chamfer: **negative inside the
    // window, positive out in the frame, zero on the cut.** The standard rounded-box
    // formula — `min(max(q), 0) + length(max(q, 0)) - r` against the half-extents — so the
    // chamfer is stated as a distance rather than as a second shape to keep in sync with
    // the first.
    let window_sdf = |px: f32, py: f32| -> f32 {
        let cx = (hx0 + hx1) * 0.5;
        let cy = (hy0 + hy1) * 0.5;
        let qx = (px - cx).abs() - ((hx1 - hx0) * 0.5 - WINDOW_R);
        let qy = (py - cy).abs() - ((hy1 - hy0) * 0.5 - WINDOW_R);
        qx.max(qy).min(0.0) + (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() - WINDOW_R
    };

    // ★ 2026-10-24（六六）：「圆角能再圆润一点嘛（不是增大 R 角，是边缘不光滑）」——
    //   洞与壁的交界改成**覆盖度抗锯齿**：槽口框内每像素 `SLOT_AA²` 个采样（框外仍是 2×2），
    //   洞的采样占比直接折成透明度。弧线因此是 `SLOT_AA²` 级（现 64 级）过渡，而不是「非 0 即 255」的台阶。
    //   桌面图只在启动 / 换主题时烘一次（`frontend::upload_desktop`），这点采样量不进每帧预算。
    const SLOT_AA: usize = 8;
    let slot_aa_step = 1.0 / SLOT_AA as f32;

    for y in 0..h {
        let fy = y as f32;
        for x in 0..w {
            let fx = x as f32;
            // **只有四个弧角要细采样。** 直壁与顶/底边都落在整像素上，2×2 就够；把整块槽口
            //   都提到 8×8 会让「换色板重烘桌面」多算 130 万个采样 —— 那是按一次 Y 就走一遍的
            //   路径，所以这里只圈四小块（左右 × 上下），每块约 30×24 像素。
            let near_slot = {
                let side = (fx >= SLOT_X0 - SLOT_MOUTH_R - SLOT_WALL - 3.0
                    && fx <= SLOT_X0 + SLOT_CORNER_R + 3.0)
                    || (fx >= SLOT_X1 - SLOT_CORNER_R - 3.0
                        && fx <= SLOT_X1 + SLOT_MOUTH_R + SLOT_WALL + 3.0);
                let end = fy < SLOT_Y0 + SLOT_CORNER_R + 3.0
                    || fy > SLOT_Y1 - SLOT_CORNER_R - 3.0;
                side && end && fy >= SLOT_BOX_Y0 - 3.0 && fy <= SLOT_Y1 + 3.0
            };
            let mut tab_n = 0usize;
            let mut frame_n = 0usize;
            let mut slot_n = 0usize;
            let mut floor_n = 0usize;
            let mut take = |ox: f32, oy: f32| match fill(fx + ox, fy + oy) {
                Fill::None => {}
                Fill::Tab => tab_n += 1,
                Fill::Slot => slot_n += 1,
                Fill::SlotFloor => floor_n += 1,
                Fill::Frame => frame_n += 1,
            };
            let total;
            if near_slot {
                total = SLOT_AA * SLOT_AA;
                for iy in 0..SLOT_AA {
                    for ix in 0..SLOT_AA {
                        take(
                            (ix as f32 + 0.5) * slot_aa_step,
                            (iy as f32 + 0.5) * slot_aa_step,
                        );
                    }
                }
            } else {
                // 2×2 supersampling keeps the window's rounded corners and the plate's clean.
                total = 4;
                for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    take(ox, oy);
                }
            }
            let opaque_n = tab_n + frame_n + slot_n + floor_n;
            if opaque_n == 0 {
                continue;
            }
            // The slot is its own block of dark plastic and the plate is the darker accent, so
            // where either of them meets the frame on one pixel their own colour wins; the
            // overall alpha is how much of the pixel is opaque at all.
            let i = (y * w + x) * 4;
            if floor_n > 0 {
                // The floor under the cut is solid: the reference's last rows are opaque, in
                // the slot's own dark.
                // ★ 2026-10-24（六六）：「边缘不光滑」也指壁的**外沿**（壁与板子的交界）。
                //   两边都不透明，alpha 帮不上忙，所以按采样占比把颜色混一下 —— 外沿因此
                //   也是平滑过渡，不是 1px 台阶。
                let c = if frame_n > 0 {
                    mix(slot, frame, frame_n as f32 / (floor_n + frame_n) as f32)
                } else {
                    slot
                };
                buf[i] = c[0];
                buf[i + 1] = c[1];
                buf[i + 2] = c[2];
            } else if slot_n > 0 {
                buf[i] = slot[0];
                buf[i + 1] = slot[1];
                buf[i + 2] = slot[2];
                // **The slot is a dark wall, not a hole and not a solid block.** Cut clear,
                // the place a cart plugs into has no shape at all — it merges with the window
                // above it and the eye finds nothing there. Solid, it hides the cart going
                // in. `SLOT_ALPHA` is the reading in between; at 0 the hole is fully clear and
                // the alpha below is left to the wall's own coverage.
            } else if tab_n > 0 {
                // ★ 2026-10-23（六六）：「背板颜色和插卡位的边一致 + 渲染轮廓线」—— 填色就是
                //   插卡位壁那一个色（`tab = slot`），倒角端点从机器的色派生：侧边 5px 从
                //   `rim` 淡到背板色、顶/底 2px 从四成混合淡到背板色；外缘 1px 轮廓线用
                //   `rim`。板子因此读作从整块塑料上切出来的一件，背板不再是贴上去的纸条。
                let side = (fx - nx0).min(nx1 - fx);
                // ★ 2026-10-24（六六）：「上缘的线条去掉」不只指那 1px 描边 —— 顶边那道
                //   2px 的倒角也一起去掉，板子从屏幕外垂下来，上缘不该有任何"边"。
                //   所以 `cap` 只量**底边**：顶端落进"侧边倒角"那一支，也就是平的。
                let _ = ny0;
                let cap = ny1 - fy;
                // ★ 2026-10-24（六六）：「名称栏**上缘的线条去掉**（这样才像是屏幕外垂落的
                //   告示牌）；下缘两侧的圆角线条要和横边竖边连得上」。
                //   所以描边不再用 `side.min(cap)` 这种**按矩形**量的做法 —— 那样上缘会有一道
                //   横线，而圆角处又量不到(半径 16 > 1px 描边)，线在拐角就断了。
                //   改成量**真实轮廓**：左/右/底三条直边 + 两个下角圆弧，取最小；不量上缘，
                //   于是上缘没有横线，两侧的线顺着弧一直走到屏幕外。
                let edge = {
                    let mut d = side.min(ny1 - fy);
                    if fx < nx0 + NP_R && fy > ny1 - NP_R {
                        d = d.min(NP_R - ((fx - (nx0 + NP_R)).powi(2)
                            + (fy - (ny1 - NP_R)).powi(2)).sqrt());
                    }
                    if fx > nx1 - NP_R && fy > ny1 - NP_R {
                        d = d.min(NP_R - ((fx - (nx1 - NP_R)).powi(2)
                            + (fy - (ny1 - NP_R)).powi(2)).sqrt());
                    }
                    d
                };
                let c = if edge <= TAB_OUTLINE {
                    // The outline: the machine's own highlight, one pixel round the outside.
                    tab_lit
                } else if side <= cap {
                    // Along a side: the wider, brighter bevel.
                    let t = (side / TAB_CHAMFER).clamp(0.0, 1.0);
                    mix(tab_lit, tab, t)
                } else {
                    // Across the top or the bottom: the shorter, fainter one.
                    let t = (cap / TAB_CAP).clamp(0.0, 1.0);
                    mix(tab_edge, tab, t)
                };
                buf[i] = c[0];
                buf[i + 1] = c[1];
                buf[i + 2] = c[2];
            } else {
                // **One colour for the face, and the light on the cut.** A ramp across the
                // board would read as two pieces of plastic meeting rather than one frame —
                // so the face stays a single colour and the texture lives where the plastic
                // is actually cut: two pixels of chamfer along the window's edge, from the
                // machine's own highlight at the very cut to the board's colour two pixels
                // in. It is the nameplate's treatment, on the frame's cut.
                // The pixel's nearest corner to the cut: the chamfer is measured in
                // plastic, and a centre sample would sit the gradient a pixel off the cut.
                let d = [(fx, fy), (fx + 1.0, fy), (fx, fy + 1.0), (fx + 1.0, fy + 1.0)]
                    .iter()
                    .map(|(px, py)| window_sdf(*px, *py))
                    .fold(f32::MAX, f32::min);
                let c = if d < CHAMFER_W {
                    let t = (d / CHAMFER_W).clamp(0.0, 1.0);
                    [
                        (lit[0] as f32 + (frame[0] as f32 - lit[0] as f32) * t).round() as u8,
                        (lit[1] as f32 + (frame[1] as f32 - lit[1] as f32) * t).round() as u8,
                        (lit[2] as f32 + (frame[2] as f32 - lit[2] as f32) * t).round() as u8,
                    ]
                } else {
                    frame
                };
                buf[i] = c[0];
                buf[i + 1] = c[1];
                buf[i + 2] = c[2];
            }
            // **透明度 = 不透明的采样占比。** 洞的采样按 `SLOT_ALPHA` 折算（现在 0 ⇒ 全透），
            //   于是「洞 + 壁」混着的边缘像素自动拿到 0..255 之间的中间值：弧线就是这么变滑的。
            //   槽口框内 `total = SLOT_AA²`、框外 `total = 4`，两者在同一行代码里各自换算。
            let scale = 255.0 / total as f32;
            let a = ((opaque_n - slot_n) as f32 + (slot_n as f32) * SLOT_ALPHA) * scale;
            buf[i + 3] = a.round().clamp(0.0, 255.0) as u8;
        }
    }

    buf
}

/// `[f32; 4]` from the chrome's accessors, back to bytes for the raster. The chrome keeps
/// its own copy private, and this is the one place the desktop needs it.
fn bytes(c: [f32; 4]) -> [u8; 3] {
    [
        (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[2].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

/// A channel-wise mix of two raster colours: `a` toward `b` by `t` (0..1), rounded the way
/// the raster rounds its own blends.
fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// Signed distance to the card-slot hole, **negative inside** — the hole is an ordinary
/// rounded rectangle, radius `SLOT_CORNER_R` at all four corners, `X0..X1` × `Y0..Y1`.
///
/// ★★★ 2026-10-24（六六）：上下两角同一个半径、互为镜像（「下面不动，上面两个圆弧面是相对的」），
/// 左右竖边全程垂直，洞的边界从不越过直壁。**洞的形状没变，变的是壁**：
/// 壁原来是把洞向左/右/上/下各推 6px（曼哈顿膨胀），在口部两角会收出尖角；
/// 现在壁 = `0 ≤ d ≤ SLOT_WALL` 的等距带，外轮廓是真圆弧。
///
/// 标准圆角矩形 SDF（`min(max(q),0) + length(max(q,0)) − r`），和窗口割口用的是同一条公式。
fn slot_sdf(px: f32, py: f32) -> f32 {
    // ★ 洞的真实形状：**口部外扩 `)(`**（弧凸向洞外）+ 直壁 + 底部 R18 圆角。
    //   距离用「边界原语」算：顶边一段、底边一段、左壁三段（弧/竖/弧），右壁由镜像得到。
    //   这样壁才是**处处精确 6px** 的等距带 —— 之前拿几个 max/min 拼 SDF，在口部会把厚
    //   度算歪，壁上就长出鼓包。
    let r = SLOT_MOUTH_R;
    let rb = SLOT_CORNER_R;
    let mirror = SLOT_X0 + SLOT_X1;

    let edge = |x: f32| -> f32 {
        // 到左壁边界的距离（只算左半边的三段），右半边靠镜像
        let flare = arc_dist(x, py, SLOT_X0 - r, SLOT_Y0 + r, r, -FRAC_PI_2, 0.0);
        let wall = seg_dist(x, py, SLOT_X0, SLOT_Y0 + r, SLOT_X0, SLOT_Y1 - rb);
        let foot = arc_dist(x, py, SLOT_X0 + rb, SLOT_Y1 - rb, rb, FRAC_PI_2, PI);
        flare.min(wall).min(foot)
    };
    let top = seg_dist(px, py, SLOT_X0 - r, SLOT_Y0, SLOT_X1 + r, SLOT_Y0);
    let bottom = seg_dist(px, py, SLOT_X0 + rb, SLOT_Y1, SLOT_X1 - rb, SLOT_Y1);
    let d = edge(px).min(edge(mirror - px)).min(top).min(bottom);

    // 内外判定：逐行区间（口部由外扩弧给出，底部由圆角给出）。
    let dy = py - SLOT_Y0;
    let dyb = py - (SLOT_Y1 - rb);
    let left = if dy < 0.0 || py > SLOT_Y1 {
        SLOT_X0
    } else if dy < r {
        SLOT_X0 - r + (r * r - (r - dy) * (r - dy)).max(0.0).sqrt()
    } else if dyb > 0.0 {
        SLOT_X0 + rb - (rb * rb - dyb * dyb).max(0.0).sqrt()
    } else {
        SLOT_X0
    };
    let inside = py >= SLOT_Y0 && py <= SLOT_Y1 && px >= left && px <= mirror - left;
    if inside {
        -d
    } else {
        d
    }
}

/// 到线段的距离。
fn seg_dist(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (vx, vy) = (bx - ax, by - ay);
    let len2 = vx * vx + vy * vy;
    let t = if len2 <= 0.0 {
        0.0
    } else {
        (((px - ax) * vx + (py - ay) * vy) / len2).clamp(0.0, 1.0)
    };
    let (qx, qy) = (ax + vx * t, ay + vy * t);
    ((px - qx).powi(2) + (py - qy).powi(2)).sqrt()
}

/// 到一段圆弧的距离（角度区间 `a0..=a1`，单位弧度）。
fn arc_dist(px: f32, py: f32, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> f32 {
    let ang = (py - cy).atan2(px - cx);
    let a = ang.clamp(a0.min(a1), a0.max(a1));
    let (qx, qy) = (cx + r * a.cos(), cy + r * a.sin());
    ((px - qx).powi(2) + (py - qy).powi(2)).sqrt()
}

/// True when `(px, py)` is inside the rounded rectangle `(x0,y0,x1,y1)` with corner radius `r`.
/// The canvas edge is left sharp; only the rectangle itself is rounded.
fn rounded_rect(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    if px < x0 || px > x1 || py < y0 || py > y1 {
        return false;
    }
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    let (cx, cy) = if px < x0 + r && py < y0 + r {
        (x0 + r, y0 + r)
    } else if px > x1 - r && py < y0 + r {
        (x1 - r, y0 + r)
    } else if px < x0 + r && py > y1 - r {
        (x0 + r, y1 - r)
    } else if px > x1 - r && py > y1 - r {
        (x1 - r, y1 - r)
    } else {
        return true;
    };
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= r * r
}

/// True when `(px, py)` is inside a rectangle whose **top corners are square** and whose
/// **bottom corners are rounded** (a quarter-circle of radius `r` at each).
///
/// This is the shape the nameplate and the card slot actually are: the pieces hang off a
/// straight edge, so the two corners they meet stay right angles, and only the two free
/// bottom corners are rounded. A clean radius keeps the curve smooth — the hand-fitted
/// per-row draw-in that kept getting redrawn with facets is gone.
fn round_bottom_rect(
    px: f32,
    py: f32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    r: f32,
) -> bool {
    if px < x0 || px >= x1 || py < y0 || py >= y1 {
        return false;
    }
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    let (cx, cy) = if px < x0 + r && py > y1 - r {
        // bottom-left quarter
        (x0 + r, y1 - r)
    } else if px > x1 - r && py > y1 - r {
        // bottom-right quarter
        (x1 - r, y1 - r)
    } else {
        // the top and the middle: all square, no corner to test
        return true;
    };
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= r * r
}
