//! The tape: a compact cassette's shape, and the face drawn on it.
//!
//! # The proportion, and where 1.673 came from
//!
//! A compact cassette is 100.4 by 63.8 mm across its front face — the figure Philips licensed
//! into the format in 1965 — which is **1.574 to 1**. That number is real and it is what the
//! standard says, and it was what this file used to claim.
//!
//! **It is not the number the artwork is drawn at.** 六六's own cassette render (2026-10-03) was
//! measured at **1076 by 643 = 1.6734** — noticeably wider than 1.574, and deliberately so: a
//! tape whose window is the same proportion as the object has no room left for the panel that
//! wraps around it. **The artwork is the authority here**, so that is the number this file uses,
//! and the difference is stated rather than quietly averaged away. If a future render measures
//! differently, this constant and this comment change together.
//!
//! # Why 226 rather than 240: height, not width
//!
//! The shelf's row is built for an Advance cart, **240 by 135** (`CART_W`/`CART_H`), and a tape
//! that stood 240 wide would be 143 tall — taller than the cart, breaking the row's alignment.
//! So a tape is sized by its **height** against the cart's, and its width follows from its own
//! proportion: 135 x 1.6734 = **226**. It is 7 px narrower than the cart beside it, which is
//! correct: a cassette *is* a little narrower than an Advance cart, and letting the row breathe is
//! what makes a mixed row read as a row.
//!
//! This is the same argument the shelf already makes for the Game Boy family, which is 240 wide
//! and 276 tall — a different shape entirely, standing in the same row without asking the row to
//! know. A tape is the fourth such shape.
//!
//! **Nothing here is a `slot_store::System`.** A tape has no save folder, no core and no
//! `States/` path; what it has is a shape, and a shape is what the shelf and the face pipeline
//! actually want.

use crate::cart::{bare_face, built_face, CartFace, CartSize};
use crate::shell::{Finish, Shell};
use crate::silhouette::Shape;

/// A tape's size on the shelf.
///
/// **226 by 135**, which is 1.6734 — the artwork's own proportion (see the header) — at the
/// Advance cart's height. The pitch, the middle scale and the side scale are that row's too, so a
/// tape shelf and a cart shelf move the same way and the furniture around them does not have to
/// know which is up.
///
/// The label panel is **per-mille of the face, and it is the artwork that decides it**: a tape
/// whose art is the card's own says where its own label goes, in magenta, and
/// `tape_art::face` reads it from there. The box below is what the **built-in** tape falls back
/// to, and it is the artwork's band above the reel window — 4.7 to 1, not the Advance's 2.3,
/// because a cassette's title goes on the strip above its window and not on a portrait panel.
/// **226 by 135**, at a middle scale of **1.8** and a pitch of **430**.
///
/// The size is the same 226x135 the Advance cart's height gives (see the header), so the ratio is
/// untouched — only the two numbers beside it changed, and both are this row's own:
///
/// * **1.8, not 1.5.** At 1.5 a tape in the middle was 339 px across on a 720 panel, with 190 px
///   of empty case either side and the whole lower half of the screen unused. A tape is the
///   thing you are here to look at, and it is flat, so it can take a bigger scale than a cart
///   without looking stretched. 1.8 puts it at 406 px — still 157 px clear each side, so the row
///   still breathes.
/// * **pitch 400, not 360.** The pitch is how far apart neighbours stand, and at 360 both
///   neighbours sat **entirely on screen** (x 607..833 with a 720-wide panel) — the row showed
///   three tapes and a lot of case, and sliding further just moved the same three. At 400 a
///   neighbour shows 73 of its 226 px — **32%**, in the third-to-a-third band that reads as
///   "there are more of these" without looking like the row has run off the screen.
///   (390 gives 37%, 410 gives 28%; 400 sits in the middle of the 30–35 range asked for.)
/// * **drop 22, not 0.** At 1.8 the middle tape's top edge sat **6 px** under the letter ring
///   while 114 px of empty case sat below it, and the title — which is placed from the foot —
///   was left with a band it did not need. Dropping the row 22 px splits the difference: 28 px
///   above, 92 px below. **The title moves with it**, because `shelf_title_y` is derived from the
///   foot; that is the same relationship a portrait cart has, and it is why this is one number
///   rather than two edits.
///
/// The side scale stays 1.0: a neighbour is the same object seen further away, not a smaller one.
///
/// ⚠️ **`drop` 从 22 改成 −32（2026-10-23 23:5x）**，而且不是重新调观感：`foot_y` 的算法
/// 换成了「半屏 + 主卡半高」（主卡居中逆推），这一行算出来的底座从 **307.5 变成 361.5**。
/// 磁带**不是卡带行**，六六那条规矩不管它，所以这里把 drop 反推回差额、再加回原来那 22：
/// `361.5 − 32 = 329.5` ⇒ **脚线仍是 329.5，一个字没动**。
/// 要真按新规矩重排磁带，是另一件事，得他说。
///
/// ⚠️ 23:5x 第一版这里错写成 `-10`（注释却声称「脚线仍是 329.5」）—— 那是把"差 32"当成
/// "补 10"，脚线实际跑到 351.5、整行下移 22px。现在 -32 才对上。
pub const SIZE: CartSize = CartSize::new(226, 135, 400.0, 1.8, 1.0, -32.0, [62, 100, 938, 410]);

/// The colour a tape is moulded in when nothing has said otherwise.
///
/// Not any one cassette's plastic — a shell colour per album would be a table nobody could
/// finish — but the near-black every deck's own cassettes settled on, which is what makes a row
/// of them read as tapes rather than as cartridges with the wrong label on.
pub const SHELL: Shell = Shell {
    colour: [0x2a, 0x2a, 0x2e],
    finish: Finish::Solid,
};

/// A tape wearing `label`, cut to the cassette's outline.
///
/// `label` is `SIZE`'s own label box, face pixels, RGBA — the same shape `cart::cart_face`
/// hands the cart pipeline, so the two faces are built by the same code and cannot drift.
///
/// This is the **built-in** tape. A card that supplies `System/Carts/tape.png` gets that art
/// instead, through `tape_art::face`, which is what the deck calls; this stays as the fallback so
/// a card with no art is unchanged.
pub fn face(label: &[u8]) -> CartFace {
    built_face(Shape::Tape, &SHELL, SIZE, label)
}

/// The built-in face in a shell colour other than the default near-black, for `tape_art::face`'s
/// fallback. Same drawing, different plastic — the same relationship the cart path has between
/// `shell_face` and the art that replaces it.
///
/// Named apart from the `built_face` imported above because that one is `cart`'s four-argument
/// form; shadowing it would have made this file's own `face` call the wrong one.
pub(crate) fn face_in_shell(shell: &Shell, label: &[u8]) -> CartFace {
    built_face(Shape::Tape, shell, SIZE, label)
}

/// The cassette in black, for a tape drawn dimmed over a wallpaper. One silhouette for the whole
/// row, for the same reason the shelf keeps one for the carts.
pub fn shadow() -> CartFace {
    let (w, h) = (SIZE.face_w(), SIZE.face_h());
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for cover in crate::silhouette::silhouette_of(Shape::Tape, w, h) {
        rgba.extend_from_slice(&[0, 0, 0, *cover]);
    }
    CartFace { rgba, w, h }
}

/// A tape with nothing on it: the shell, its moulding, and the empty recess where the label would
/// sit. What the shelf draws for a tape whose own face has not been built yet.
pub fn placeholder() -> CartFace {
    bare_face(Shape::Tape, &SHELL, SIZE)
}
