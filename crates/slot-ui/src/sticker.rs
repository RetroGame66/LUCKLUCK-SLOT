//! The about screen's label: the back label of a Game Boy Advance SP, **as a drawing and
//! nothing else**.
//!
//! # 2026-10-29（六六）：上面印的字全部去掉
//!
//! 这块牌子原先是一张底图 + 六组由代码印上去的字：型号行、认证行（含手绘的直流符号）、
//! 十行 credit、产地行、序列号与脏标记、版权与地址，外加一枚 Code 39 条形码和一枚
//! ANBERNIC 锁标。**六六 2026-10-29 定：这些 66SLOT 一个都用不上，条形码一并去掉，两棵树都去。**
//!
//! 理由不是"简化"，是**归位**：牌子上的内容是美术，美术该由画的人决定画成什么样。
//! 代码硬印一套字，等于每一版都得跟着改字号、改行长、改对位表 —— 那套对位表本身就是
//! 一串只能靠肉眼核的数字（`MARGIN`/`HEAD_PX`/`PANEL_FX`…）。改成纯图之后，
//! 想在上面写什么、写多大、放哪儿，全在 `sticker-66mod.png` 里，这里一个字都不管。
//!
//! ⇒ 因此本文件只剩两件事：**把图解码成一张面**，和**把它居中画出去**。
//! `StickerFields`（电量/序列号/脏标记）随之消失 —— 牌子不再显示任何"这台机器当前状态"。
//! 电量另有顶栏那枚竖放电池与 `battery.rs` 的读数，序列号在 `Build` 里仍然存在，
//! 只是不再往牌子上印。
//!
//! # 为什么面还是这块面
//!
//! 牌子是**被看的东西**，不是被排版的屏幕：它按自己的尺寸居中，不拉伸去填屏 ——
//! `draw_sticker_at` 不铺底、不缩边，画多大就是多大，落在什么颜色上由调用方决定。

use slot_gfx::{Draw, TexId};

use crate::art::decode_bytes;
use crate::plate::UndoFace;

/// The label itself — **the artwork, whole**. Carried in the binary at this face's exact size.
///
/// ★★ 2026-10-29（640 树）：**660×228 → 608×210。** 660 是 720 那块屏的尺度
/// （左右各余 30），到 640 上 `(640 − 660) / 2 = −10`，两侧各切掉 10 px：圆角和左上
/// 那个切角都在边上，切掉的正是这块牌子"是块牌子"的地方。
///
/// 608×210 与 660×228 **等比**（`608/660 = 0.9212`、`210/228 = 0.9211`，两者都落在
/// ppi 比 `229/247 = 0.927` 上），所以缩完在这台机器上看着**跟原来一样大**
/// —— 缩的是像素数，不是观感。居中后左右各余 `(640 − 608) / 2 = 16`。
///
/// ⚠️ 图没到之前这里是**透明底**：`decode_label` 拿不到这个尺寸就返回 `None`，
/// 于是牌子整块不画（而不是画一张被拉伸的旧图，也不是画一张空白的黑块）。
/// 六六重画好的图按 608×210 放进 `assets/` 就自动出现，代码不用再动。
pub const STICKER_W: u32 = 608;
pub const STICKER_H: u32 = 210;

/// The label's pixels, at this face's size.
///
/// Decoded once and cached: it never changes for the life of the binary, and re-inflating a
/// 608×210 PNG on the frame loop would be the one costly thing about an otherwise static
/// picture.
const STICKER_PNG: &[u8] = include_bytes!("../assets/sticker-66mod.png");

/// The whole label, ready to upload.
///
/// **A size mismatch is `None`, not a resize.** A plate carried at one size and asked for at
/// another is a plate drawn for a different panel; stretching it would hide that, and hiding it
/// is how the 720 art ended up on a 640 screen with its corners cut off and nothing complaining.
/// `None` makes the absence visible instead — see `draw_sticker_at`.
pub fn sticker_face() -> Option<UndoFace> {
    let (px, w, h) = decode_bytes(STICKER_PNG)?;
    if (w, h) != (STICKER_W, STICKER_H) {
        return None;
    }
    Some(UndoFace {
        rgba: px,
        w: STICKER_W,
        h: STICKER_H,
    })
}

/// Centred on screen, at its own size. The label is an object being looked at rather than a
/// screen being laid out, so it is not stretched to fit — and it paints no ground of its own,
/// so the caller decides what it sits on.
pub fn draw_sticker(face: Option<TexId>, out: &mut Vec<Draw>) {
    draw_sticker_at(face, (slot_gfx::OUT_H as f32 - STICKER_H as f32) / 2.0, out);
}

/// The same label at a height the caller has chosen, which is what a screen that scrolls needs:
/// the card under it moves, so the label has to be able to leave with it rather than being the
/// one thing pinned to the middle of the panel.
pub fn draw_sticker_at(face: Option<TexId>, y: f32, out: &mut Vec<Draw>) {
    let Some(tex) = face else {
        return;
    };
    out.push(Draw::Tex {
        x: (slot_gfx::OUT_W as f32 - STICKER_W as f32) / 2.0,
        y,
        w: STICKER_W as f32,
        h: STICKER_H as f32,
        tex,
        alpha: 1.0,
    });
}