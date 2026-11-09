//! The card's images, and the two formats a card is allowed to hold.
//!
//! **This file exists because of a bug that cost a debugging session.** `slot_media::Covers`
//! indexes `png / jpg / jpeg / webp`, so a card filed as `[2001]范特西/cover.jpg` resolved to
//! the right path, the file was there and readable — and the sleeve drew an **empty recess**,
//! because the decoder only ever spoke PNG. Nothing was logged, nothing looked wrong; the art
//! was found and then thrown away. Every case below is one way that can happen.
//!
//! **Each one asserts on a colour, not merely on `Some`.** "It decoded" and "it decoded into
//! the right pixels" are different claims, and the weaker one would pass on a build that spent
//! the whole budget on a format it half supports.

use slot_ui::{cd_cover, decode_rgba};

/// A 32 by 32 with four flat quadrants: red, green, blue, yellow. Flat and far apart, so a
/// wrong colour is unmistakable and a right one needs only a JPEG-sized tolerance.
const COLOUR: &[u8] = include_bytes!("fixtures/cover.jpg");
/// The same picture out of a progressive encoder. Store-bought and phone-saved art is as likely
/// to be progressive as baseline, and the two take different paths through the decoder.
const PROGRESSIVE: &[u8] = include_bytes!("fixtures/cover-progressive.jpg");
/// Greyscale: one component in, four out. The widening is the decoder's job, and it is the case
/// a cover taken off an old scan lands in.
const GREY: &[u8] = include_bytes!("fixtures/cover-gray.jpg");
/// A PNG **wearing a `.jpg` name**. The sniff has to be on content, or files that used to work
/// stop working the moment a second format is added — which is the trap this whole change sets.
const PNG_NAMED_JPG: &[u8] = include_bytes!("fixtures/png-named-jpg.jpg");

const RED: [u8; 4] = [220, 30, 30, 255];
const GREEN: [u8; 4] = [30, 200, 60, 255];
const BLUE: [u8; 4] = [40, 60, 220, 255];
const YELLOW: [u8; 4] = [235, 210, 40, 255];

/// Writes `bytes` under `name` and decodes it the way the shelf does — from a path, through the
/// public door, with no reach into the decoder's internals.
fn decode(bytes: &[u8], name: &str) -> (Vec<u8>, u32, u32) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).expect("write fixture");
    decode_rgba(&path).expect("decoded")
}

/// The pixel at `x, y`, as straight RGBA.
fn at(px: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    px[i..i + 4].try_into().expect("four channels")
}

/// One channel of one pixel, for a readable failure message.
fn near(px: &[u8], w: u32, x: u32, y: u32, want: [u8; 4], what: &str) {
    let got = at(px, w, x, y);
    for c in 0..4 {
        let d = got[c].abs_diff(want[c]);
        assert!(
            d <= 12,
            "{what}: channel {c} at {x},{y} is {} want {} (off by {d})",
            got[c],
            want[c]
        );
    }
}

/// The four quadrants, sampled well inside their own flat areas so a boundary pixel from the
/// JPEG's own smoothing cannot decide the test.
fn quadrants_are_right(px: &[u8], w: u32) {
    near(px, w, 8, 8, RED, "top left");
    near(px, w, 24, 8, GREEN, "top right");
    near(px, w, 8, 24, BLUE, "bottom left");
    near(px, w, 24, 24, YELLOW, "bottom right");
}

#[test]
fn a_jpeg_cover_decodes() {
    let (px, w, h) = decode(COLOUR, "cover.jpg");
    assert_eq!((w, h), (32, 32), "a jpeg keeps its own dimensions");
    quadrants_are_right(&px, w);
}

#[test]
fn a_progressive_jpeg_cover_decodes() {
    let (px, w, h) = decode(PROGRESSIVE, "cover.jpg");
    assert_eq!((w, h), (32, 32));
    quadrants_are_right(&px, w);
}

#[test]
fn a_greyscale_jpeg_cover_decodes() {
    let (px, w, h) = decode(GREY, "cover.jpg");
    assert_eq!((w, h), (32, 32));
    // One channel in, four out, and the format carries no alpha: an unwritten alpha channel
    // would paste the sleeve into the case as a hole.
    assert_eq!(at(&px, w, 8, 8)[3], 255, "a jpeg sleeve is opaque");
    // Grey, so the four quadrants differ only in brightness — which still leaves them unequal,
    // and that is all this asserts: the picture arrived, it was not a single flat fill.
    let a = at(&px, w, 8, 8);
    let b = at(&px, w, 24, 24);
    assert_ne!(a[0], b[0], "the quadrants came out as one flat colour");
}

/// The sniff is on the file's first bytes, never on its name. Without this, adding JPEG would
/// have quietly broken every card that named a PNG `.jpg`.
#[test]
fn a_png_named_jpg_still_decodes() {
    let (px, w, h) = decode(PNG_NAMED_JPG, "cover.jpg");
    assert_eq!((w, h), (32, 32));
    quadrants_are_right(&px, w);
}

/// What the sleeve actually asks for: a JPEG cover, through the public door, at the size the
/// card's own magenta mark says the area is.
#[test]
fn a_cover_scales_into_the_slot() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("cover.jpg");
    std::fs::write(&path, COLOUR).expect("write fixture");

    let (w, h) = (120, 96);
    let px = cd_cover(&path, w, h).expect("scaled cover");
    assert_eq!(px.len(), (w * h * 4) as usize, "exactly the slot, RGBA");
    // Cover, not fit: a 32 by 32 square into a 120 by 96 box fills the whole of it, and the
    // quadrants survive the box average well enough to still be told apart.
    let top = at(&px, w, w / 2, 4);
    let bottom = at(&px, w, w / 2, h - 5);
    assert_ne!(top, bottom, "the two halves of the cover did not survive scaling");
}

/// A slot with no room is not a decode. `cd_cover` is called with `(0, 0)` on a card whose art
/// carries no magenta, and it must answer before touching the file.
#[test]
fn a_zero_slot_is_no_cover() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("cover.jpg");
    std::fs::write(&path, COLOUR).expect("write fixture");
    assert!(cd_cover(&path, 0, 0).is_none());
}
