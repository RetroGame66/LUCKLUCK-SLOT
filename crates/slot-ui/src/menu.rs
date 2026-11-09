//! The menu behind MENU: a grid of rounded buttons, and what each one opens.
//!
//! This screen used to be one long card — the label, then thirty-nine rows of shortcut — and a
//! reader who wanted the fourth row scrolled past three screens of the first. The card is a
//! screen of its own now, and what is left here is the thing MENU is actually for: a short list
//! of the places this frontend can go that are not a game.
//!
//! **Three buttons since 2026-10-21** (六六：「删除像素阅读器……把像素播放器的按钮扩到整列」):
//! the reader is off the build, so the grid lost its fourth button and the player — the one
//! shelf left — takes the whole of the second row instead of half of it. A button's width is
//! `Item::span()` columns, not a constant, so "one button, one row" is the same layout code
//! with a different span. (`mould` and `opening_rgb` are `pub(crate)` because the settings screen — a
//! list of rows rather than a grid of buttons — is a plate with the same three greys and the
//! same moulding on it, and a second copy of that arithmetic would be a second answer to what
//! a panel is made of.)

use crate::draw::{OUT_H, OUT_W};
use crate::lang;
use crate::palette;
use crate::plate::UndoFace;
use crate::slot_chrome::{edge, housing, opening};
use crate::sticker::STICKER_H;
use crate::text;

/// What a button opens.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Item {
    /// The shortcut card, as a screen of its own.
    Shortcuts,
    /// The card's music, as discs on a shelf.
    Discs,
    /// What the device itself is set to: the lid timer, and the clock (2026-10-10, 六六).
    Settings,
}

impl Item {
    /// How many columns this button takes. `COLS` is "the whole row".
    ///
    /// ★ 2026-10-21 六六：「把像素播放器的按钮扩到整列」—— 阅读器下线后剩下三颗，
    ///   播放器独占第二行。**跨度是按钮自己的属性**，不是布局里的特例：`slot_at`/`button`
    ///   都从这一个数推宽度，所以"整行"和"半行"是同一套算术。
    pub fn span(self) -> usize {
        match self {
            // 播放器是唯一一条货架，给它整行 —— 半行的话这一行右边会空出一格。
            Item::Discs => COLS,
            Item::Shortcuts | Item::Settings => 1,
        }
    }

    /// The grid in reading order. `None` is a slot this build has no screen for: it is drawn as
    /// an empty recess and cannot be landed on, so the shape of what is coming is visible
    /// without a button that does nothing when it is pressed.
    ///
    /// ★ **Reordered 2026-10-10 (六六)**, then **shortened 2026-10-21**: the top row is the two
    /// *reference* screens — "read about the machine" above "change the machine" — and the
    /// bottom row is the one *shelf* left. `All Keys | Parameters` over `Pixel Player`.
    pub const GRID: [Option<Item>; SLOTS] = [
        Some(Item::Shortcuts),
        Some(Item::Settings),
        Some(Item::Discs),
    ];

    pub fn label(self) -> &'static str {
        match self {
            Item::Shortcuts => lang::MENU_SHORTCUTS,
            Item::Discs => lang::MENU_DISCS,
            Item::Settings => lang::MENU_SETTINGS,
        }
    }
}

/// The panel's top padding, which is where the label starts.
pub const PAD: f32 = 26.0;
/// Between the label's foot and the grid.
pub const LABEL_GAP: f32 = 20.0;
/// Where the grid starts, and therefore where the label has to end.
pub const TOP: f32 = PAD + STICKER_H as f32 + LABEL_GAP;

/// Two columns, two rows, **three buttons** — two on the first row, one across the second.
pub const COLS: usize = 2;
pub const ROWS: usize = 2;
/// The number of buttons. **Not `COLS * ROWS` any more**: a full-row button is one button that
/// happens to occupy two cells, so the count is the grid's own length.
pub const SLOTS: usize = 3;

/// How far in from the panel's sides the grid sits.
///
/// ★ **它等于贴纸自己的左边距**（2026-10-06 六六：「行宽度应该对齐上面的关于页贴纸」）。
///   贴纸由 `sticker::draw_sticker_at` **居中**画的 ⇒ 左右各让 `(OUT_W − STICKER_W) / 2`。
///   原先这里是 56：格子比贴纸每边窄 26px，上下两块并排看着就是"没对齐"。
///   ⚠️ **改这个数就是改 `BUTTON_W`**（下面那条从它推），两处不会各自跑偏。
///
/// ★★ 2026-10-29（640 树）：**从字面量 660 改成 `sticker::STICKER_W`。**
///   这里原来写的是 `(OUT_W - 660.0) / 2.0`，在 640 屏上算出来是 **−10**；
///   而且 660 是第二份真值 —— 贴纸一改尺寸，这里不知道，两边就各算各的。
///   现在它读的是同一个常量，`sticker` 那一处是唯一的旋钮。
pub const MARGIN: f32 = (OUT_W as f32 - crate::sticker::STICKER_W as f32) / 2.0;
/// Between the buttons, both ways.
pub const GUTTER: f32 = 14.0;
/// The panel is 720 across, so this is what is left after the two margins and the one gutter.
pub const BUTTON_W: f32 = (OUT_W as f32 - 2.0 * MARGIN - GUTTER) / COLS as f32;
/// Tall enough to be a target rather than a line of type, short enough that two rows and the
/// hint bar all fit under a 228 px label on a 480 px panel.
///
/// ★★ **64, and it was 70 until 2026-10-10 — the second row's bottom 12 px were under the hint
/// bar.** The bar is opaque and drawn last, so what the user saw was the lower row losing its
/// bottom edge, which reads as the panel being cut rather than as a layout being 12 px too tall.
///
/// The arithmetic, all of which was already in `PAD`/`LABEL_GAP`/`GUTTER`:
/// `TOP` = 26 + 228 + 20 = 274, the bar tops out at `OUT_H - shortcuts::ROW_H` = **416**, and two
/// rows plus the gutter are `2·BUTTON_H + 14`. `2·70 + 14 = 154` reaches 428 (12 over);
/// **`2·64 + 14 = 142` reaches exactly 416.** Any height in between would fit, and 64 is the one
/// that is *also* the bar's own height — the row ends where the bar begins, which is the tidiest
/// of the values that work.
///
/// 六六 2026-10-10:「MENU页按键做扁一点不就不用加滚动了」—— this is that, taken to the number the
/// bar forces rather than to a guess.
pub const BUTTON_H: f32 = 64.0;
/// How round. Enough to read as a key rather than as a card, not enough to read as a pill.
pub const RADIUS: f32 = 12.0;
/// The rim, in pixels. The same weight on every button, so the grid reads as one object.
const RIM_PX: f32 = 1.5;

/// Where slot `i` sits, in offscreen pixels, as its top left corner.
///
/// ★ 2026-10-21：**位置从 `span` 推出来**，不再写死 `i % COLS` / `i / COLS` —— 整行那颗
///   占两格，按旧的取模会把自己算到第二列上、并且和第二行重叠。
pub fn slot_at(i: usize) -> (f32, f32) {
    let (row, col, _) = slot_place(i);
    (
        MARGIN + col as f32 * (BUTTON_W + GUTTER),
        TOP + row as f32 * (BUTTON_H + GUTTER),
    )
}

/// Slot `i`'s `(row, first column, span)`. Flow layout: walked in `GRID` order, `COLS` columns
/// to a row, and a button takes as many columns as it declares.
pub fn slot_place(i: usize) -> (usize, usize, usize) {
    let span_of = |k: usize| {
        Item::GRID
            .get(k)
            .copied()
            .flatten()
            .map_or(1, Item::span)
    };
    let mut row = 0usize;
    let mut col = 0usize;
    for k in 0..i.min(SLOTS) {
        col += span_of(k);
        if col >= COLS {
            row += 1;
            col = 0;
        }
    }
    (row, col, span_of(i.min(SLOTS.saturating_sub(1))))
}

/// How wide slot `i` is: `span` cells and the gutters inside it, so one full-row button is
/// exactly as wide as the two buttons above it plus the gap between them.
pub fn slot_w(i: usize) -> f32 {
    let span = slot_place(i).2 as f32;
    span * BUTTON_W + (span - 1.0) * GUTTER
}

/// The first slot with something in it, which is where the caret lands when MENU opens.
///
/// A function rather than the literal `0`, so that reordering `GRID` — or emptying its first
/// slot while a feature is rebuilt — cannot leave the menu opening with nothing selected.
pub fn first_slot() -> usize {
    Item::GRID.iter().position(|s| s.is_some()).unwrap_or(0)
}

/// Where the caret goes when it is pushed `(dx, dy)` slots from `index`.
///
/// **Geometric, not `index ± 1`** (2026-10-21): with a full-row button the indices no longer
/// tell you what is beside what. Up and down go to the nearest button on the other row (nearest
/// by column, measured as the gap between the two buttons' column spans), and left and right
/// walk the row and wrap. A row with one button on it has no left or right — moving the caret
/// inside a button that is the whole row would be a key that does nothing, which is worse than a
/// key that stays put.
pub fn step(index: usize, dx: isize, dy: isize) -> usize {
    if dx == 0 && dy == 0 {
        return index;
    }
    let (row, col, span) = slot_place(index);

    if dy != 0 {
        let want = (row as isize + dy).rem_euclid(ROWS as isize) as usize;
        let mut best = index;
        let mut best_gap = isize::MAX;
        for k in 0..SLOTS {
            let (r, c, sp) = slot_place(k);
            if r != want {
                continue;
            }
            let gap = if c + sp <= col {
                (col - (c + sp)) as isize
            } else if col + span <= c {
                (c - (col + span)) as isize
            } else {
                0
            };
            if gap < best_gap {
                best_gap = gap;
                best = k;
            }
        }
        return best;
    }

    let mut same: Vec<usize> = (0..SLOTS).filter(|k| slot_place(*k).0 == row).collect();
    same.sort_by_key(|k| slot_place(*k).1);
    if same.len() < 2 {
        return index;
    }
    let at = same.iter().position(|k| *k == index).unwrap_or(0) as isize;
    let n = same.len() as isize;
    same[((at + dx.signum()).rem_euclid(n)) as usize]
}

/// The foot of the grid, for a caller checking it clears the hint bar.
pub fn bottom() -> f32 {
    TOP + ROWS as f32 * BUTTON_H + (ROWS - 1) as f32 * GUTTER
}

/// The height of whatever is pinned at the foot of this screen, which is what the grid has to
/// clear — `shortcuts::ROW_H`, because the menu's foot line is built by `shortcut_hint_face`,
/// the same call the shortcut card makes.
///
/// ⚠️ **Not `plate::HINT_H` (24).** That is the three-hint legends' height — the small strip a
/// game screen wears — and the menu's own hint is a `shortcut_hint_face`. Measuring against the
/// 24 would leave the guard below 16 px too permissive, which is exactly how the second row sat
/// 12 px under the bar from the day the grid was written until 2026-10-10: the old assert only
/// asked whether the grid fit inside the *panel*.
pub const HINT_BAR_H: f32 = crate::shortcuts::ROW_H as f32;

/// The line printed on the pinned bar at the foot of this screen. Here rather than in `lang.rs`
/// for the same reason the shortcut card's is in `shortcuts.rs`: it is a description of a layout
/// and reads wrong away from it.
#[cfg(feature = "lang-en")]
pub const HINT: &str = "Arrows to choose      A to open      B to go back";
#[cfg(not(feature = "lang-en"))]
pub const HINT: &str = "方向键选择    A 进入    B 返回";

/// The two faces one button is drawn with.
pub struct Button {
    pub idle: UndoFace,
    pub chosen: UndoFace,
}

/// Bake one button, idle and chosen.
///
/// Two faces rather than one face and a bar behind it — which is what the power menu does — for
/// the reason a rounded rect is baked at all: a bar cannot be rounded, and a selection that
/// squares off the thing it selects is a selection that changes the button's shape.
pub fn button(item: Item) -> Button {
    let w = button_w(item);
    Button {
        idle: face(Some(item.label()), false, w),
        chosen: face(Some(item.label()), true, w),
    }
}

/// A slot with nothing in it yet: the recess, and no words.
pub fn empty() -> UndoFace {
    face(None, false, BUTTON_W)
}

/// The baked width of `item`'s button — its span's worth of cells and gutters.
pub fn button_w(item: Item) -> f32 {
    let span = item.span() as f32;
    span * BUTTON_W + (span - 1.0) * GUTTER
}

fn face(label: Option<&str>, chosen: bool, width: f32) -> UndoFace {
    let (w, h) = (width.round() as u32, BUTTON_H.round() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];

    // The palette's own three greys, in the roles the rest of the device gives them: a control
    // sits on `housing`, a thing you look into is `opening`, and the one in hand is lit with
    // `edge` — the lightest value in the theme, which is what the power menu's bar uses and for
    // the same reason.
    let (fill, ink) = match (label, chosen) {
        (None, _) => (opening(), palette::dim_ink()),
        // ★ **The lit row's label is the one colour here that is chosen, not derived** — see
        //   `palette::chosen_ink`. `opening` was what it used to be, and on the light palette this
        //   device ships with that was 2.13:1, a label the same tone as its own button. It is
        //   still `opening` for the device's own palette, so nothing about the shipped look moves.
        (Some(_), true) => (edge(), palette::chosen_ink()),
        // ★ **未选中键的字退后一档**（2026-10-16 六六：「选中键为亮色，未选中的为暗色」）。
        //   原先两态都是 `ink()`，量过：**设备自带配色下选中与未选中的字是同一个色号**
        //   （深 `#F5F2EF` / 浅 `#1C1B1F`）⇒ 所谓"选中的态弱"，不止弱在 lit 面上，
        //   在字上根本就没拉开过。
        //   ⚠️ 第一版这里直接写了 `dim_ink()` —— 设备自带配色下对（7.80 / 5.16:1），
        //   **但 12 档 ramp 上全崩**：`dim` 是"从 ramp 的**地面**退后一档"，而未选中键画在
        //   ramp 的**塑料壳色**上，是另一个颜色。实测 RED 1.30:1 / VIOLET 1.57 / AMBER 1.98
        //   ⇒ 六六在 RED 那张上看到字没了："红色那张换过来就对了吧"。
        //   现在走 `palette::idle_ink()`：**在同一套候选里挑"够读且最钝"的那个**，
        //   设备自带配色下结果与 `dim_ink` 逐字节相同（零回归），ramp 上退回该 look 自己的墨色。
        (Some(_), false) => (housing(), palette::idle_ink()),
    };
    // `edge`-on-`edge` is 1.00:1 — the in-hand button had no outline, only `mould`'s gradient
    // between it and its neighbours. The lit face takes `palette::chosen_rim`.
    let rim = if chosen { palette::chosen_rim() } else { edge() };
    crate::rounded::plate(&mut rgba, w, h, RADIUS, RIM_PX, fill, Some(rim));
    mould(&mut rgba, w, h, RADIUS, RIM_PX);

    if let Some(label) = label {
        if let Some(font) = text::label_font() {
            let inset = 2.0 * PAD;
            let layout = text::fit(
                font,
                label,
                w as f32 - inset,
                1,
                LABEL_PX,
                LABEL_MIN_PX,
            );
            // ★ **按墨迹居中，不按 em 框**（2026-10-06 六六：「名称也没居中」）。
            //   量过：按 em 框居中时 `按键指南` 的墨迹中心落在 36.5，而框中心是 34.5 —— **低 2px**。
            //   根因是 Ark Pixel 的 ascent/descent 不对称（32.5 / 7.5 @30px），
            //   墨迹在 em 框里本来就偏下。这一课在液晶面板上已经吃过一次（`text::plan_ink_span`）。
            text::draw_centred_ink(&mut rgba, w, h, &layout, ink);
        }
    }
    UndoFace { rgba, w, h }
}

/// **把平整的板子压出模压感** —— 一块塑料键不是一块色块。
///
/// 三件东西，都是"光从上面来"这一条：
/// * **竖向渐变**：顶亮一点、底暗一点（各约 7% / 11%），读作一块有厚度的片而不是一张纸；
/// * **内侧 1px 上棱**：往白走 16%，是模具的分模线；
/// * **内侧 1px 下棱**：往黑走 18%，是它自己的暗边。
///
/// ⚠️ **一律用"混"不用"乘"**：`chosen` 那一档的底色是主题里最亮的一个（`edge()`），
/// 乘法一过 1 就削顶，亮按钮会变成一块死白。混色天然不会越界。
///
/// ⚠️ **描边那一圈不碰**（`outer - inner > 0.25` 就跳过）：描边是形状的一部分，
/// 往它上面压渐变会把圆角咬出毛边。
pub(crate) fn mould(rgba: &mut [u8], w: u32, h: u32, radius: f32, rim_px: f32) {    const GRAD_TOP: f32 = 0.07;
    const GRAD_BOTTOM: f32 = 0.11;
    const LIP_TOP: f32 = 0.16;
    const LIP_BOTTOM: f32 = 0.18;

    let (fw, fh) = (w as f32, h as f32);
    // 内侧那块（描边以内）的圆角与位置，和 `plate` 里那道描边用的是同一组数。
    let (iw, ih, ir) = (fw - 2.0 * rim_px, fh - 2.0 * rim_px, (radius - rim_px).max(1.0));

    for y in 0..h {
        let py = y as f32 + 0.5;
        let t = (py - rim_px) / ih.max(1.0); // 0 = 内区顶，1 = 内区底
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if rgba[i + 3] == 0 {
                continue;
            }
            let px = x as f32 + 0.5;
            let outer = crate::rounded::cover(px, py, fw, fh, radius);
            let inner = crate::rounded::cover(px - rim_px, py - rim_px, iw, ih, ir);
            if outer - inner > 0.25 {
                continue; // 这一像素属于描边
            }
            let mut k_top = GRAD_TOP * (1.0 - t).clamp(0.0, 1.0);
            let mut k_bot = GRAD_BOTTOM * t.clamp(0.0, 1.0);
            if py - rim_px < 1.0 {
                k_top += LIP_TOP;
            }
            if fh - rim_px - py < 1.0 {
                k_bot += LIP_BOTTOM;
            }
            for c in 0..3 {
                let v = rgba[i + c] as f32;
                let up = v + (255.0 - v) * k_top; // 往白
                rgba[i + c] = (up * (1.0 - k_bot)).clamp(0.0, 255.0) as u8; // 往黑
            }
        }
    }
}

/// The label's size. Large enough to be read at arm's length on a 720-wide panel, and the floor
/// is what it may shrink to when a translated label runs long.
const LABEL_PX: f32 = 30.0;
const LABEL_MIN_PX: f32 = 18.0;

/// The width of a laid-out line, measured the way `text::coverage` will draw it — per glyph,
/// through `face_for`, so a mixed-script line's width matches its ink.
///
/// Public to the crate because the settings screen measures two cells against each other with
/// the same arithmetic a button measures one with, and a second copy of this fold is a second
/// answer to how wide a word is.
pub(crate) fn line_w(font: &fontdue::Font, layout: &text::Layout) -> f32 {
    layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max)
}

/// `opening` as bytes, for the type on a lit button. Taken from the same function the fill is,
/// so a theme that flips both flips them together.
pub(crate) fn opening_rgb() -> [u8; 3] {
    let c = opening();
    [
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

/// Kept honest by the compiler rather than by a comment: the grid the layout is built for is
/// `GRID`'s own length.
const _: () = assert!(Item::GRID.len() == SLOTS);

/// The panel's height is not a parameter of anything here — the grid is placed from the top and
/// the hint bar owns the bottom — but a slot that ran past the panel would be a button nobody
/// could press, so the arithmetic is checked at compile time.
///
/// ★★ **And then the bar, which is the tighter of the two** (2026-10-10). The panel check alone
/// let the second row sit 12 px under an opaque bar for as long as the grid has existed; both
/// facts have to hold, and the bar's is the one that decides `BUTTON_H`.
const _: () = assert!(TOP + ROWS as f32 * BUTTON_H + (ROWS - 1) as f32 * GUTTER < OUT_H as f32);
const _: () = assert!(
    TOP + ROWS as f32 * BUTTON_H + (ROWS - 1) as f32 * GUTTER <= OUT_H as f32 - HINT_BAR_H
);
