//! The desktop frame's shape and colour, pinned to the measurements.
//!
//! The geometry is the board this code draws (2026-10-23's final spec): a 720×480 plate with
//! a 700×420 window cut out of it — ten pixels of plate at the top, the left and the right,
//! fifty at the bottom — and a 360-wide card slot cut into the bottom bar. The colour is the
//! machine's own plastic: the board is `housing()`, the slot's six pixels and the nameplate
//! are `opening()` (one piece, 2026-10-23), the nameplate's outside carries a one-pixel
//! `rim()` outline, and the window's cut carries the highlight for two pixels in.
//! Guessing either is what went wrong before, so both are pinned here.

use std::sync::Mutex;

use slot_ui::palette::{self, Mode};
use slot_ui::{desktop_rgba, housing, opening, rim};

/// The alpha of the frame at `(x, y)`: 0 where the frame is cut away, 255 where it is solid.
fn alpha_at(buf: &[u8], x: usize, y: usize) -> u8 {
    let i = (y * 720 + x) * 4;
    buf[i + 3]
}

/// The colour of the frame at `(x, y)`.
fn rgb_at(buf: &[u8], x: usize, y: usize) -> (u8, u8, u8) {
    let i = (y * 720 + x) * 4;
    (buf[i], buf[i + 1], buf[i + 2])
}

/// The chrome's accessors hand back `[f32; 4]`; the raster is bytes.
fn bytes(c: [f32; 4]) -> (u8, u8, u8) {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    (b(c[0]), b(c[1]), b(c[2]))
}

/// Halfway between the cut's highlight and the board's colour — the row behind the lit one.
fn half(hi: (u8, u8, u8), base: (u8, u8, u8)) -> (u8, u8, u8) {
    // The raster rounds its blend, so this rounds too — a truncated 56 against the raster's
    // 57 is the difference between a passing test and a lying one.
    let m = |a: u8, b: u8| ((a as f32 + b as f32) / 2.0).round() as u8;
    (m(hi.0, base.0), m(hi.1, base.1), m(hi.2, base.2))
}

/// The mode is a process-wide fact and these tests run in parallel: every test that reads a
/// colour takes the lock and pins the mode, so the two of them cannot interleave — the same
/// shape `slot::core`'s tests use for the environment.
static LOCK: Mutex<()> = Mutex::new(());

fn in_dark() -> std::sync::MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    palette::set_mode(Mode::Dark);
    g
}

#[test]
fn window_is_700_by_420_with_ten_pixel_rails() {
    let buf = desktop_rgba();
    // The top rail is ten deep: solid to y 9, open from y 10.
    assert_eq!(alpha_at(&buf, 60, 9), 255, "y 9 is still the top rail");
    assert_eq!(alpha_at(&buf, 60, 10), 0, "the window opens at y 10");
    // The sides are ten wide — and so is the right one, which is the half-pixel the
    // inclusive bound used to cost.
    assert_eq!(alpha_at(&buf, 9, 200), 255, "x 9 is still the left rail");
    assert_eq!(alpha_at(&buf, 10, 200), 0, "the window opens at x 10");
    assert_eq!(alpha_at(&buf, 709, 200), 0, "x 709 is inside the window");
    assert_eq!(alpha_at(&buf, 710, 200), 255, "x 710 is the right rail");
    // 420 tall, and the bar is the last fifty rows.
    assert_eq!(alpha_at(&buf, 60, 429), 0, "y 429 is inside the window");
    assert_eq!(alpha_at(&buf, 60, 430), 255, "y 430 is the bottom bar");
    assert_eq!(alpha_at(&buf, 60, 479), 255, "the bar reaches the foot of the screen");
    // ★★★ 2026-10-24（六六）：「插卡位挖孔是一个**上直角下圆角**的矩形」——
    //   上角是直角 ⇒ 洞口上沿直接与板子顶边（= 窗口底边 y 430）齐平，窗口一行都不被压。
    for x in [200, 360, 520] {
        assert_eq!(alpha_at(&buf, x, 429), 0, "the game is untouched right above the mouth (x {x})");
    }
}

#[test]
fn nameplate_is_400_wide_and_54_deep_over_the_slot() {
    // ★ 2026-10-24（六六）：顶栏 400 宽 = 左 40 信息 / 中 320 名称 / 右 40 信息。
    let buf = desktop_rgba();
    // It covers the top rail: solid at the very first row.
    assert_eq!(alpha_at(&buf, 360, 0), 255, "the plate reaches the top of the screen");
    // 400 wide, from x 160 to x 559（比插卡位洞两边各宽 20）。
    assert_eq!(alpha_at(&buf, 159, 30), 0, "left of the plate is open");
    assert_eq!(alpha_at(&buf, 160, 30), 255, "the plate starts at x 160");
    assert_eq!(alpha_at(&buf, 559, 30), 255, "the plate ends at x 559");
    assert_eq!(alpha_at(&buf, 560, 30), 0, "right of the plate is open");
    // 54 deep.
    assert_eq!(alpha_at(&buf, 360, 53), 255, "the plate is still there at y 53");
    assert_eq!(alpha_at(&buf, 360, 54), 0, "the plate ends at y 53");
}

#[test]
fn card_slot_is_a_hole_with_six_pixel_walls_and_a_solid_floor() {
    let _g = in_dark();
    let buf = desktop_rgba();
    let wall = bytes(opening());
    // The straight run of the wall: six pixels each side, in the slot's own dark.
    // y 450 sits in the middle of the straight run (446..457) — clear of both corners.
    assert_eq!(rgb_at(&buf, 174, 450), wall, "the wall starts at x 174");
    assert_eq!(rgb_at(&buf, 179, 450), wall, "the left wall is six wide");
    assert_eq!(rgb_at(&buf, 540, 450), wall, "the right wall starts at x 540");
    assert_eq!(rgb_at(&buf, 545, 450), wall, "the right wall ends at x 545");
    // The hole through the straight run: cut clear, so the cart shows through it, 360 wide.
    assert_eq!(alpha_at(&buf, 360, 450), 0, "the hole is cut clear");
    for x in 180..=539 {
        assert_eq!(alpha_at(&buf, x, 450), 0, "x {x} is inside the hole");
    }
    // 44 deep — measured down the middle, where neither arc reaches.
    for y in 430..=473 {
        assert_eq!(alpha_at(&buf, 360, y), 0, "the hole is 44 deep at y {y}");
    }
    // ★★★ 2026-10-24（六六）：「**两个角是对称的圆弧边，圆弧是对着的 `)(`**」——
    //   口部两张弧凸向洞外（洞口比直壁宽），底部两张弧内收（R18）。
    //   实测（`cargo run --example slot_profile`）：430→173 … 437→180，438..456 恒 180；
    //   底角 457→181 …… 473→197，474 整行关闭。右壁恒为 `719 − 左壁`。
    for y in 441..=456 {
        assert_eq!(alpha_at(&buf, 180, y), 0, "and the straight side holds x 180 at y {y}");
        assert_eq!(alpha_at(&buf, 179, y), 255, "and x 179 is wall at y {y}");
    }
    // 直壁那一段的壁：整整六格（y 450）。
    for x in 174..=179 {
        assert_eq!(rgb_at(&buf, x, 450), wall, "the wall is six wide at y 450 (x {x})");
    }
    for x in 540..=545 {
        assert_eq!(rgb_at(&buf, x, 450), wall, "the right wall at y 450 (x {x})");
    }
    // 底角：R18 的圆角，逐行收进。
    assert_eq!(alpha_at(&buf, 181, 457), 0, "the foot corner starts turning at y 457");
    assert_eq!(alpha_at(&buf, 197, 473), 0, "and its last open row is x 197");
    assert_eq!(alpha_at(&buf, 522, 473), 0, "the right foot corner mirrors it at x 522");
    assert_eq!(alpha_at(&buf, 360, 474), 255, "the hole is fully closed at y 474");
    // The floor under the hole is solid, in the slot's own dark, to the foot of the screen.
    // ⚠️ 洞的最后一行是 **473**，不是 474：底角那 18 行从 456 走到 474，到 474 时左壁已经
    //    收到 x 195、右壁在 x 524 之外 —— 两侧的洞合拢（整行不再有「全部采样都是洞」的
    //    像素）。这一行的 `alpha` 已被壁占满，所以它在读数上就是地板。
    assert_eq!(alpha_at(&buf, 360, 473), 0, "the hole's own last open row");
    assert_eq!(alpha_at(&buf, 360, 474), 255, "the floor starts under it");
    assert_eq!(alpha_at(&buf, 360, 479), 255, "the floor reaches the bottom");
    assert_eq!(rgb_at(&buf, 360, 479), wall, "the floor is the slot's dark");
}

/// ★★★ 2026-10-24（六六）：「**我要这两个角是对称的圆弧边！圆弧是对着的，）（ 这样！
/// 不是（ ）这样**」。
///
/// 这条把形状钉死：**口部两张弧凸向洞外、左右相对 `)(`，洞口比直壁宽**；
/// 底部是 R18 圆角、单调收进。做成内收 `( )` 就当场报错。
#[test]
fn the_mouth_flares_out_and_the_foot_rounds_in() {
    let buf = desktop_rgba();
    let left = |y: usize| (170..=200).find(|&x| alpha_at(&buf, x, y) == 0).unwrap();
    // 口部：**外扩** —— 洞口在顶边比直壁宽（430→173，往下一路收回 437→180）。
    //   两张弧凸向洞外、左右相对 `)(`；收成 `( )`（内收圆角）就是错的。
    let mouth: Vec<usize> = (430..=437).map(left).collect();
    assert_eq!(mouth, vec![173, 175, 176, 177, 178, 179, 179, 180], "the mouth's flare");
    assert!(mouth[0] < 180, "the mouth must be WIDER than the straight wall");
    for y in 441..=456 {
        assert_eq!(left(y), 180, "the straight side at y {y}");
    }
    // 底部：从 457 起单调收进，到 473 收到 197。
    assert_eq!(left(457), 181, "the foot corner starts at y 457");
    for y in 458..=473 {
        assert!(left(y) >= left(y - 1), "the foot corner only ever closes in (y {y})");
    }
    assert_eq!(left(473), 197, "and its last open row is x 197");
    // 口部**必须**比直壁宽（外扩 `)(`）：一旦有一行窄过 180，就是又退回收口的 `( )` 了。
    assert!(
        (430..=436).all(|y| left(y) < 180),
        "the mouth must flare wider than the straight wall `)(`"
    );
}

/// ★★★ 2026-10-24（六六）：「圆角能再圆润一点嘛（不是增大 R 角，是边缘不光滑）」。
/// 这条钉的是**边缘光滑度**：两段弧上每一行，紧挨着洞的那个像素必须是**半透明**的
/// （0 < alpha < 255），即「壁 → 洞」之间有一层覆盖度过渡；一旦它直接就是 255，
/// 说明弧线又退回「非 0 即 255」的台阶了。
#[test]
fn the_slots_arcs_are_antialiased() {
    let buf = desktop_rgba();
    let left = |y: usize| (170..=200).find(|&x| alpha_at(&buf, x, y) == 0).unwrap();
    // ★ 2026-10-24（六六）：「上直角下圆角」—— 上面是直角，边缘落在整像素上，本来就不需要
    //   过渡；只有**底部**那段弧要查。直壁段同理。
    for y in 457..=473 {
        let edge = alpha_at(&buf, left(y) - 1, y);
        assert!(
            edge > 0 && edge < 255,
            "the arc at y {y} needs a partial pixel at x {}, got alpha {edge}",
            left(y) - 1
        );
    }
    // 而且要有足够多的档位：整条弧上出现的不同中间值不能只有一两种（否则就是硬边）。
    let mut levels = std::collections::BTreeSet::new();
    for y in 457..=473 {
        let edge = alpha_at(&buf, left(y) - 1, y);
        if edge > 0 && edge < 255 {
            levels.insert(edge);
        }
    }
    assert!(
        levels.len() >= 6,
        "the arc's coverage should ramp through several levels, saw {levels:?}"
    );
}

#[test]
fn the_board_is_the_machines_own_plastic() {
    let _g = in_dark();
    let buf = desktop_rgba();
    let base = bytes(housing());
    // One colour for the whole face — no ramp across the board, which is the conclusion the
    // 整体渲染 round reached and this keeps.
    for (x, y) in [(60, 5), (5, 200), (715, 200), (60, 450), (700, 460)] {
        assert_eq!(rgb_at(&buf, x, y), base, "the face at ({x}, {y})");
    }
    // ★ 2026-10-23（六六）：「背板颜色和插卡位的边一致 + 轮廓线」—— 填色就是槽壁那一个
    //   色，外缘 1px 是机器自己的高光。
    let wall = bytes(opening());
    let hi = bytes(rim());
    assert_eq!(rgb_at(&buf, 360, 30), wall, "the plate is the slot's colour");
    assert_eq!(rgb_at(&buf, 160, 30), hi, "the plate's left outline");
    assert_eq!(rgb_at(&buf, 559, 30), hi, "the plate's right outline");
    // ★ 2026-10-24（六六）：「名称栏上缘的线条去掉（才像是屏幕外垂落的告示牌）」——
    //   顶边只剩板子的填色，没有那道高光横线。
    assert_eq!(rgb_at(&buf, 360, 0), wall, "no outline along the plate's top edge");
}

#[test]
fn the_cut_carries_the_machines_highlight() {
    let _g = in_dark();
    let buf = desktop_rgba();
    let base = bytes(housing());
    let hi = bytes(rim());
    assert_ne!(hi, base, "the highlight must read against the board");
    // The top cut: the row against it carries the highlight, the one behind it half of it,
    // and the board's own colour from there in — the nameplate's treatment on the frame.
    assert_eq!(rgb_at(&buf, 100, 9), hi, "the row against the cut is lit");
    assert_eq!(rgb_at(&buf, 100, 8), half(hi, base), "the row behind is half lit");
    assert_eq!(rgb_at(&buf, 100, 6), base, "the board's colour from there in");
    // The left cut, the same.
    assert_eq!(rgb_at(&buf, 9, 200), hi, "the left cut is lit");
    assert_eq!(rgb_at(&buf, 7, 200), base, "the board's colour three in");
}

/// ★ 2026-10-23（六六）：「暗色模式底色还是按原来的深灰色，亮色模式按浅灰色」—— 板子是机器
/// 自己的塑料，所以明暗切换时整块板换色，槽壁跟着换（它俩都是主题自己的色）。
#[test]
fn the_board_follows_the_mode() {
    let _g = in_dark();
    let dark = desktop_rgba();
    let dark_base = bytes(housing());
    let dark_wall = bytes(opening());
    assert_eq!(rgb_at(&dark, 60, 5), dark_base, "the dark board");
    assert_eq!(rgb_at(&dark, 176, 450), dark_wall, "the dark slot wall");

    palette::set_mode(Mode::Light);
    let light = desktop_rgba();
    let light_base = bytes(housing());
    let light_wall = bytes(opening());
    assert_eq!(rgb_at(&light, 60, 5), light_base, "the light board");
    assert_eq!(rgb_at(&light, 176, 450), light_wall, "the light slot wall");
    // And the two are on opposite sides of the middle, which is the whole of the ask.
    let mid = |c: (u8, u8, u8)| (c.0 as u16 + c.1 as u16 + c.2 as u16) / 3;
    assert!(mid(dark_base) < 128, "the dark board is a dark grey");
    assert!(mid(light_base) > 128, "the light board is a light grey");

    palette::set_mode(Mode::Dark);
}
