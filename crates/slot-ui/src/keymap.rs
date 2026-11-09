//! The operation keymap sheet: which game operation each of the six keys takes.
//!
//! **A list, not a grid, and one row per key.** The rows the sheet offers are `A`, `B`, `X`,
//! `Y`, `L1` and `R1` — the six a thumb and a finger can actually reach on this machine — and
//! one rate row at the bottom for the turbo targets. What a row is *set to* is picked from the
//! list `keys::TARGETS` holds: nothing, a game button, a pair of them, or a turbo.
//!
//! **The name on the left, the value on the right.** The left column is the thing the row *is*,
//! which is what has to be read while the thumb is already moving; the right column is what it
//! currently does, and it is the shorter of the two. Both are baked into the row face, so a row
//! is one texture and the sheet never rasterises on a frame.
//!
//! **The chord table is not here.** The `SELECT` chords keep the gesture layer's own table;
//! this module carries only the six operation-key rows the sheet edits.
//!
//! **No scroll on the rows.** Seven rows fit under the title and above the foot line, so a row's
//! place is a function of its index alone. The *picker* is the list that scrolls — the whole
//! of `keys::TARGETS` under a `VISIBLE`-row window — and `pick_top` is the whole of its scroll.

use crate::draw::{Draw, TexId};
use crate::palette;
use crate::plate::UndoFace;
use crate::rounded;
use crate::slot_chrome::{edge, housing};
use crate::text;

/// How wide a row is. ★ **2026-10-21 六六：「有点紧凑不好看」** —— 行宽收到和游戏菜单那块板
/// 一样（板 640 宽，左右各留 24），于是两张板一样宽，字也不用再挤到边上。
pub const ROW_W: f32 = 592.0;
/// One row. ★ **2026-10-21：26 → 36。** 26 是照着"十三行 chord 表"配的密度，可那张表
/// 已经不在这儿了；七行摆在一屏上，再按十三行的密度排就只剩"一坨小字"。
pub const ROW_H: f32 = 36.0;
/// The air between two rows, for the **picker** — `VISIBLE` of the target list show at once and it
/// moves under a fixed thumb, so this one stays tight.
pub const GAP: f32 = 4.0;
pub const PITCH: f32 = ROW_H + GAP;
/// Where the panel sits.
///
/// ★ 2026-10-21：板 640 宽放在 720 屏上 ⇒ 左右各留 40，与游戏菜单同一块板。
/// ★★ 2026-10-29（640 树）：本树面板正好 640，**板就是满宽**，所以 `PANEL_X = 0`。
/// `PANEL_W` 不必改（它本来就叫 640），`ROW_W = 592` 也原样 —— 满宽 640 里留 24 边距，
/// 与 720 上"板 640 内留 24"是**同一个内边距**，行宽看着一样宽。
pub const PANEL_X: f32 = 0.0;
pub const PANEL_Y: f32 = 8.0;
pub const PANEL_W: f32 = 640.0;
pub const PANEL_H: f32 = 470.0;
/// The top of the first row, below the title.
pub const TOP: f32 = 68.0;
/// The air between the plate's edge and the type at either end of it.
const PAD: f32 = 24.0;
/// How wide the right-hand cell is. A row's own value sits right-aligned inside it.
const KEY_W: f32 = 220.0;
/// How round. Flatter than a settings row: these are keys rather than fields.
const RADIUS: f32 = 7.0;
const RIM_PX: f32 = 1.0;
/// The type. ★ **2026-10-21：18 → 24。** 和行高一起放大，否则字还是小小的贴在一条厚条上。
const TEXT_PX: f32 = 24.0;
const TEXT_MIN_PX: f32 = 15.0;

/// The title's own height and where it sits.
pub const TITLE_H: f32 = 36.0;
pub const TITLE_Y: f32 = 20.0;
/// The line at the foot of the sheet.
pub const FOOT_H: f32 = 28.0;
pub const FOOT_Y: f32 = 438.0;

/// How many rows the picker's window holds. Measured against the foot line rather than the bottom
/// of the screen, because the foot line is what the last row may not run under. **The key rows
/// do not use this** — seven rows spread over the whole page instead (see `pitch_for`).
pub const VISIBLE: usize = ((FOOT_Y - TOP + GAP) / PITCH) as usize;

/// ★ 2026-10-21：**行距跟着模式走。**
///
/// * **行列表**（七行）：把 `TOP..FOOT_Y` 整段平分，行与行之间留出很宽的气 —— 七行铺满一屏，
///   不是七行堆在顶上、下面空三分之二。这才是六六要的"别那么紧凑"。
/// * **目标清单**（十五项）：用紧凑的 `PITCH`，一屏装下九个，剩下的自己滚（`pick_top`）。
pub fn pitch_for(picking: bool, row_count: usize) -> f32 {
    if picking {
        PITCH
    } else {
        // The key rows (+ the rate row) spread from `TOP` to `FOOT_Y`. `row_count` is the number
        // of rows drawn (4 / 6 / 8 physical keys + the rate row), so a PlayStation's eight keys
        // get the same spread as a Famicom's four.
        (FOOT_Y - TOP) / row_count as f32
    }
}

// Kept honest by the compiler: a window of at least seven rows, and the picker does not run
// under the foot line. ★ **2026-10-21：下限 `13` → `7`** —— 13 是十三行 chord 表时代的数，
// 那张表已经不在这儿了；七行是这张表自己的长度。
const _: () = assert!(TOP + VISIBLE as f32 * PITCH - GAP <= FOOT_Y);
const _: () = assert!(VISIBLE >= 7);

/// The sheet's left edge. Centred for `ROW_W`'s own width rather than reusing `menu::MARGIN`
/// (which is cut for 660), so a row of 592 sits in the middle of the 720 screen.
pub const X: f32 = (crate::OUT_W as f32 - ROW_W) / 2.0;

/// Where a row sits, as its top left corner. `row_at`/`pitch` are handed in rather than read off a
/// constant, because **the two modes space their rows differently**: seven key rows spread over the
/// whole page, the target list packed tight enough for `VISIBLE` to show.
pub fn row_at(i: usize, pitch: f32) -> (f32, f32) {
    (X, TOP + i as f32 * pitch)
}

/// The sheet's title, centred across the page's width. Transparent rather than a plate: the
/// panel behind the sheet is already dark, and a plate would read as a second surface.
pub fn title_face(title: &str) -> UndoFace {
    let (w, h) = (ROW_W.round() as u32, TITLE_H.round() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let Some(font) = text::label_font() else {
        return UndoFace { rgba, w, h };
    };
    let layout = text::fit(font, title, ROW_W - 2.0 * PAD, 1, 28.0, TEXT_MIN_PX);
    // `coverage` centres its layout in the buffer it is given, which is what puts the title in
    // the middle of the page without a second measurement.
    ink(&mut rgba, w, h, &text::coverage(w, h, &layout), palette::chosen_ink());
    UndoFace { rgba, w, h }
}

/// One row: what the key is on the left, what it currently does on the right.
pub fn row_face(name: &str, value: &str) -> UndoFace {
    let (w, h) = (ROW_W.round() as u32, ROW_H.round() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];

    // The same three greys the settings rows take, in the same roles: a row is `housing` on the
    // panel with an `edge` rim, so the two sheets cannot read as different machines.
    rounded::plate(&mut rgba, w, h, RADIUS, RIM_PX, housing(), Some(edge()));
    crate::menu::mould(&mut rgba, w, h, RADIUS, RIM_PX);

    let Some(font) = text::label_font() else {
        return UndoFace { rgba, w, h };
    };
    // The value comes off first: it is what the name may not run into, so its width is the
    // budget the name is measured against.
    let val_layout = text::fit(font, value, KEY_W - 2.0 * PAD, 1, TEXT_PX, TEXT_MIN_PX);
    let val_w = crate::menu::line_w(font, &val_layout);
    let name_layout =
        text::fit(font, name, ROW_W - PAD - KEY_W - GAP, 1, TEXT_PX, TEXT_MIN_PX);

    stamp(&mut rgba, w, h, font, &name_layout, palette::ink(), PAD);
    stamp(&mut rgba, w, h, font, &val_layout, palette::idle_ink(), ROW_W - PAD - val_w);
    UndoFace { rgba, w, h }
}

/// The overlay a row in hand wears. One texture for the whole sheet rather than one per row.
pub fn highlight_face() -> UndoFace {
    let (w, h) = (ROW_W.round() as u32, ROW_H.round() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let e = edge();
    // A tint of the lightest value in the theme rather than the value itself: enough to say
    // "this one" without lifting the row off the page.
    let fill = [e[0], e[1], e[2], 0.18];
    rounded::plate(&mut rgba, w, h, RADIUS, RIM_PX + 0.5, fill, Some(e));
    UndoFace { rgba, w, h }
}

/// The line at the foot of the sheet: what the keys do while it is up.
pub fn foot_face(foot: &str) -> UndoFace {
    let (w, h) = (ROW_W.round() as u32, FOOT_H.round() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let Some(font) = text::label_font() else {
        return UndoFace { rgba, w, h };
    };
    let layout = text::fit(font, foot, ROW_W - 2.0 * PAD, 1, TEXT_PX, TEXT_MIN_PX);
    ink(&mut rgba, w, h, &text::coverage(w, h, &layout), palette::dim_ink());
    UndoFace { rgba, w, h }
}

/// One target row in the picker: what the key would do on the left, its file name on the right.
///
/// **One variant, and one only.** The row in hand wears the sheet's own `highlight_face` over
/// it rather than a second baked face: seventeen targets × two states would be thirty-four faces for a
/// list that is up for a second or two.
/// One target row in the picker: what the key would do on the left, its file name on the right.
///
/// `name` is the file key (`GAME_A` …) and `label` is the machine-specific button name to print
/// (see `keys::face_label`) — the same two columns a row has, and the reason the picker reads as
/// a list of the same sheet rather than a second screen.
pub fn target_face(name: &str, label: &str) -> UndoFace {
    let (w, h) = (ROW_W.round() as u32, ROW_H.round() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    rounded::plate(&mut rgba, w, h, RADIUS, RIM_PX, housing(), Some(edge()));
    crate::menu::mould(&mut rgba, w, h, RADIUS, RIM_PX);
    let Some(font) = text::label_font() else {
        return UndoFace { rgba, w, h };
    };
    // What it does on the left, its file name on the right — the same two columns a row has,
    // and the reason the picker reads as a list of the same sheet rather than a second screen.
    let key_layout = text::fit(font, name, KEY_W - 2.0 * PAD, 1, TEXT_PX, TEXT_MIN_PX);
    let key_w = crate::menu::line_w(font, &key_layout);
    let name_layout =
        text::fit(font, label, ROW_W - PAD - KEY_W - GAP, 1, TEXT_PX, TEXT_MIN_PX);
    stamp(&mut rgba, w, h, font, &name_layout, palette::ink(), PAD);
    stamp(
        &mut rgba,
        w,
        h,
        font,
        &key_layout,
        palette::idle_ink(),
        ROW_W - PAD - key_w,
    );
    UndoFace { rgba, w, h }
}

/// The rate row's right-hand cell: the number and its unit, so the row reads the way the two
/// speed rows on the settings screen do — a gear is a number with a unit beside it, and 「8」
/// alone is not a rate.
pub fn rate_face(hz: u8) -> String {
    format!("{} Hz", hz)
}

/// Where the picker's own window starts. `p` is the target in hand; the window is `VISIBLE` tall
/// and slides down once the list is longer than the page, the same way the settings list's
/// scroll does — a target list longer than `VISIBLE` is a list that has to move.
pub fn pick_top(p: usize, len: usize) -> usize {
    if len <= VISIBLE {
        return 0;
    }
    // Keep the target in hand one up from the bottom edge once the top can no longer hold it, so
    // walking down reads as the list moving under a fixed thumb.
    p.saturating_sub(VISIBLE - 2).min(len - VISIBLE)
}

/// The whole sheet: the title, the rows, the highlight, the picker and the foot.
///
/// The caller draws the panel behind it — a translucent veil and a plate over whatever was on
/// screen — so the sheet is the rows and nothing else. `highlight`, `foot` and the faces are
/// `None`-able for the reason `draw_settings`'s `face` is: a face the compositor did not build is
/// not drawn rather than drawn as a hole.
///
/// `chosen` is the row in hand (0..=6, where 6 is the rate row). `picking` is `Some(target)` when
/// the picker is open over a key row: the list replaces the rows, and the target in hand wears
/// the same highlight.
// Eight faces and two selections, all baked elsewhere and handed in: the sheet owns no state,
// so there is nothing to group them into that would not simply be the argument list again.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    title: Option<(TexId, u32, u32)>,
    rows: &[(TexId, u32, u32)],
    highlight: Option<(TexId, u32, u32)>,
    foot: Option<(TexId, u32, u32)>,
    chosen: Option<usize>,
    picking: Option<usize>,
    targets: &[(TexId, u32, u32)],
    out: &mut Vec<Draw>,
) {
    if let Some((tex, w, h)) = title {
        out.push(Draw::Tex {
            x: X,
            y: TITLE_Y,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
    }
    // The rows, with the picker's list over them when it is open. The rows below the one in hand
    // are covered rather than moved — a list opened *on* a row means the choices come out from
    // under it. ★ **2026-10-21：行距跟着模式走**（`pitch_for`）—— 七行摊满一屏，清单紧凑。
    let pitch = pitch_for(picking.is_some(), rows.len());
    if let Some(pick) = picking {
        let top = pick_top(pick, targets.len());
        for (vis, face) in targets.iter().enumerate().skip(top).take(VISIBLE) {
            let (x, y) = row_at(vis - top, pitch);
            out.push(Draw::Tex {
                x,
                y,
                w: face.1 as f32,
                h: face.2 as f32,
                tex: face.0,
                alpha: 1.0,
            });
            // The target in hand wears the sheet's own highlight — one texture for the whole
            // list rather than one per target twice.
            if vis == pick {
                if let Some((ht, hw, hh)) = highlight {
                    out.push(Draw::Tex {
                        x,
                        y,
                        w: hw as f32,
                        h: hh as f32,
                        tex: ht,
                        alpha: 1.0,
                    });
                }
            }
        }
    } else {
        for (i, &(tex, w, h)) in rows.iter().enumerate() {
            let (x, y) = row_at(i, pitch);
            out.push(Draw::Tex {
                x,
                y,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
            if Some(i) == chosen {
                if let Some((ht, hw, hh)) = highlight {
                    out.push(Draw::Tex {
                        x,
                        y,
                        w: hw as f32,
                        h: hh as f32,
                        tex: ht,
                        alpha: 1.0,
                    });
                }
            }
        }
    }
    if let Some((tex, w, h)) = foot {
        out.push(Draw::Tex {
            x: X,
            y: FOOT_Y,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
    }
}

/// One line of type, set into `dst` with its left edge at `x` and centred in the buffer's
/// height. A box is measured and the layout centred inside it, the same bargain
/// `settings::stamp` makes.
fn stamp(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    font: &fontdue::Font,
    layout: &text::Layout,
    colour: [u8; 3],
    x: f32,
) {
    if layout.lines.is_empty() {
        return;
    }
    let width = crate::menu::line_w(font, layout).ceil().max(1.0) as u32;
    let cov = text::coverage(width, dst_h, layout);
    let left = x.round() as i32;
    for y in 0..dst_h {
        for sx in 0..width {
            let a = cov[(y * width + sx) as usize] as u32;
            if a == 0 {
                continue;
            }
            let dx = left + sx as i32;
            if dx < 0 || dx as u32 >= dst_w {
                continue;
            }
            let at = ((y * dst_w + dx as u32) * 4) as usize;
            for c in 0..3 {
                let under = dst[at + c] as u32;
                dst[at + c] = ((colour[c] as u32 * a + under * (255 - a)) / 255) as u8;
            }
            dst[at + 3] = (a.max(dst[at + 3] as u32)) as u8;
        }
    }
}

/// Centred, no measuring: the coverage is already the size of its buffer.
fn ink(dst: &mut [u8], dst_w: u32, dst_h: u32, cov: &[u8], colour: [u8; 3]) {
    for (p, &cov_a) in cov[..(dst_w * dst_h) as usize].iter().enumerate() {
        let a = cov_a as u32;
        if a == 0 {
            continue;
        }
        let i = p * 4;
        for c in 0..3 {
            let under = dst[i + c] as u32;
            dst[i + c] = ((colour[c] as u32 * a + under * (255 - a)) / 255) as u8;
        }
        dst[i + 3] = (a.max(dst[i + 3] as u32)) as u8;
    }
}
