//! The deck: the screen a disc plays on.
//!
//! A cassette deck's front is three things — the disc behind its window, the name of what is
//! playing, and the keys — and this is the same three on the furniture the shelf already uses.
//! The name goes in the shelf's top window. The keys go on the shelf's slot band, which is where
//! a deck's transport is anyway: at the front, under the disc. What is left between them is
//! where the disc goes.
//!
//! **Rewind and fast-forward, not "previous" and "next".** A deck's keys say what a deck's keys
//! say, and a disc that is running is the one thing that cannot be told to run — so play and
//! pause are one key with two glyphs.

use crate::cart::CartFace;
use crate::draw::OUT_W;
use crate::slot_chrome::{edge, housing, MOUTH_H};
use crate::Icon;

/// The disc, drawn at its own face size.
///
/// A disc's face is 480 by 304 at `cart::FACE_SCALE`, so this is 1:1 — no scaling, which is what
/// keeps the spool window's hairlines and the label's corners exactly where they were rasterised.
pub const FACE_W: f32 = 480.0;
pub const FACE_H: f32 = 304.0;
pub const FACE_X: f32 = (OUT_W as f32 - FACE_W) / 2.0;
/// Below the top band and above the keys, nearer the top: a deck's window sits high, and the
/// band at the bottom belongs to the transport.
pub const FACE_Y: f32 = 96.0;

/// A transport key.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Key {
    Rewind,
    Play,
    FastForward,
}

impl Key {
    pub const ALL: [Key; 3] = [Key::Rewind, Key::Play, Key::FastForward];

    pub fn index(self) -> usize {
        self as usize
    }

    /// Which glyph this key is wearing. Play is pause while the disc runs.
    pub fn icon(self, playing: bool) -> Icon {
        match self {
            Key::Rewind => Icon::Rewind,
            Key::Play => {
                if playing {
                    Icon::Pause
                } else {
                    Icon::Play
                }
            }
            Key::FastForward => Icon::FastForward,
        }
    }
}

/// One key's own size.
pub const KEY_W: f32 = 88.0;
pub const KEY_H: f32 = 46.0;
/// Between the keys. Wide enough that a thumb lands on one.
pub const KEY_GAP: f32 = 14.0;
/// How round, and how thick the rim is. Smaller than a menu button's, because these are keys
/// rather than destinations.
pub const KEY_RADIUS: f32 = 10.0;
const KEY_RIM_PX: f32 = 1.5;

/// Where the key row starts, and where each key sits: centred on the panel, on the bottom band.
pub fn key_x(i: usize) -> f32 {
    let span = Key::ALL.len() as f32 * KEY_W + (Key::ALL.len() - 1) as f32 * KEY_GAP;
    (OUT_W as f32 - span) / 2.0 + i as f32 * (KEY_W + KEY_GAP)
}

/// The top of the band the keys are set on — the shelf's slot band, at the same height.
pub fn key_y() -> f32 {
    crate::draw::OUT_H as f32 - MOUTH_H + (MOUTH_H - KEY_H) / 2.0
}

/// A key's plate: the shell's grey with the lightest edge around it, rounded.
///
/// Baked rather than drawn as a rect for the same reason the menu's buttons are — a rounded
/// corner is not a rectangle — and one face serves all three keys, because a key is a key.
pub fn key_face() -> CartFace {
    let (w, h) = (KEY_W.round() as u32, KEY_H.round() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    crate::rounded::plate(&mut rgba, w, h, KEY_RADIUS, KEY_RIM_PX, housing(), Some(edge()));
    CartFace { rgba, w, h }
}

/// The size a key's glyph is drawn at. The faces themselves are the icons uploaded with the rest
/// of the fixed furniture, indexed by `Icon::index` — a glyph baked per key would be the same
/// glyph twice.
pub const GLYPH_PX: f32 = 26.0;
