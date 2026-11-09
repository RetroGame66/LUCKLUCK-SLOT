//! The CD case: a square sleeve, and the face drawn on it.
//!
//! **This replaces the cassette as the shape the music row is made of** (2026-10-04). The
//! cassette's own file, `tape.rs`, is still here and still builds — the project changed which
//! shape it ships, not which shapes it knows — so a card that wants the old look has a line
//! back to it.
//!
//! # Why the artwork is a square, and what that costs
//!
//! A CD sleeve is 142 mm square — **1:1, exactly**, not approximately. That is the easy half.
//! The other half is what the shape means to the rest of the row:
//!
//! * **There is no label slot.** The cassette pipeline is "cut a slot in the shell, paste the
//!   album art into it" — a cassette's title goes on the strip above its window, and that strip
//!   is a small part of the object. A sleeve's whole front *is* the cover, so the code that
//!   finds a magenta region and fills it (`tape_art::find_label`) has nothing to find here: the
//!   magenta **is** the cover area, marked so the code knows where the art goes. Both readings
//!   are the same convention — `#ff00ff` means "paste here" — and the difference is only how much
//!   of the face gets pasted.
//! * **It is much taller than the cassette.** 135 px against 260, in a row whose whole geometry
//!   is derived from a 135 px cart. That is what `drop` is for; see `SIZE`.
//!
//! # Why 260 rather than the panel's width
//!
//! The row's own numbers answer this, and the answer is not "as big as fits".
//!
//! * **Width: the neighbours must still show.** A sleeve at 260 px with a middle scale of 1.4
//!   puts the chosen one at 364 px across, and a **pitch of 364** — the middle sleeve's own
//!   width, which is the relationship the Advance row uses — leaves each neighbour showing 130
//!   of its 260, **50%**, the same share the Advance row leaves.
//! * **Height: the title owns the space below.** `App::shelf_title_y` derives the shelf's line
//!   of type from **this row's own foot** while the discs are up, so the title follows this row
//!   rather than the game row's. A 364 px sleeve stands from y 28 to y 392 and the title has the
//!   whole band under it.
//!
//! **This row has no bands, and that is what makes the height work.** The top band (clock,
//! letter ring, battery) and the bottom band (the bay, `DISC`, the favourites star) are the
//! cartridge case's furniture, and a sleeve has no case: a bay under a row of albums promises a
//! slot to push a disc into, which is not what happens (2026-10-07, 六六). Both are skipped on
//! this row — see the `Phase::Shelf` arm in `App::draw` and `Shelf::set_mouth`.

use crate::cart::{bare_face, built_face, CartFace, CartSize};
use crate::shell::{Finish, Shell};
use crate::silhouette::Shape;

/// A sleeve's size on the shelf.
///
/// **260 by 260**, at a middle scale of **1.4** and a pitch of **364** — see the module header
/// for where each of those comes from. The cover box is **per-mille of the face, and the
/// artwork decides it**: a card's own sleeve says where its cover goes, in magenta, and
/// `cd_art` reads it from there.
///
/// **`drop` is 54, and it is 54 because `CartSize::foot_y` lifts every row that is not the
/// Advance cart by 32 px.** The derived foot for a 260 px row is 370, the portrait branch makes
/// that 338, and 54 puts the foot at **392** — 28 px of margin above a 364 px sleeve and the
/// title's own band below it. This is the knob the `drop` field exists for; nudging `h` instead
/// would land the row in the portrait branch and throw it 32 px the *other* way.
///
/// The box below is what the **built-in** sleeve falls back to, and it is **not the whole
/// face**: a CD's cover is printed on the front of the plastic with a few millimetres of clear
/// showing at the edges, so the built-in box is inset by 5% on each side. A card whose own art
/// marks a full-bleed area overrides this — which is the point of the art deciding.
///
/// ⚠️ **`drop` 从 54 改成 −30（2026-10-23 23:5x）**：`foot_y` 换成「半屏 + 主卡半高」之后，
/// 这一行算出来的底座从 **338 变成 422**。碟片**不是卡带行**，所以反过来补差额：
/// `422 − 30 = 392` ⇒ 脚线仍是 392、封套仍是 364 高、上方仍是 28px 余量。
/// `App::shelf_title_y` 的碟片分支读的正是这个 foot，因此标题也跟着不动。
///
/// ⚠️ 23:5x 第一版这里错写成 `24`（注释却声称「脚线仍是 392」）—— 方向写反了，脚线实际
/// 跑到 446、整行下移 54px。现在 -30 才对上。
pub const SIZE: CartSize = CartSize::new(260, 260, 364.0, 1.4, 1.0, -30.0, [50, 50, 900, 900]);

/// The colour a sleeve is moulded in when nothing has said otherwise.
///
/// **Near-black, like the cassette's** — a jewel case is not one colour, and a row of them
/// reads as a row because they are the same object seen repeatedly. What the card's art says
/// about its own plastic wins over this; see `cd_art::face`.
pub const SHELL: Shell = Shell {
    colour: [0x24, 0x24, 0x28],
    finish: Finish::Solid,
};

/// A sleeve wearing `cover`, cut to the case's outline.
///
/// `cover` is `SIZE`'s own cover box, face pixels, RGBA — the same shape `cart::cart_face`
/// hands the cart pipeline, so the two faces are built by the same code and cannot drift.
///
/// This is the **built-in** sleeve. A card that supplies `System/Carts/cd.png` gets that art
/// instead, through `cd_art::face`; this stays as the fallback so a card with no art is
/// unchanged.
pub fn face(cover: &[u8]) -> CartFace {
    built_face(Shape::Cd, &SHELL, SIZE, cover)
}

/// The built-in face in a shell colour other than the default, for `cd_art::face`'s fallback.
/// Same drawing, different plastic — the same relationship the cart path has between
/// `shell_face` and the art that replaces it.
///
/// Named apart from the `built_face` imported above because that one is `cart`'s four-argument
/// form; shadowing it would have made this file's own `face` call the wrong one.
pub(crate) fn face_in_shell(shell: &Shell, cover: &[u8]) -> CartFace {
    built_face(Shape::Cd, shell, SIZE, cover)
}

/// The case in black, for a sleeve drawn dimmed over a wallpaper. One silhouette for the whole
/// row, for the same reason the shelf keeps one for the carts.
pub fn shadow() -> CartFace {
    let (w, h) = (SIZE.face_w(), SIZE.face_h());
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for cover in crate::silhouette::silhouette_of(Shape::Cd, w, h) {
        rgba.extend_from_slice(&[0, 0, 0, *cover]);
    }
    CartFace { rgba, w, h }
}

/// A sleeve with nothing on it: the shell, its moulding, and the empty recess where the cover
/// would sit. What the shelf draws for a sleeve whose own face has not been built yet.
pub fn placeholder() -> CartFace {
    bare_face(Shape::Cd, &SHELL, SIZE)
}
