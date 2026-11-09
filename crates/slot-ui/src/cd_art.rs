//! CD sleeve artwork the card supplies, in place of the built-in outline and moulding.
//!
//! **This is `tape_art.rs` and `cart_art.rs` again, for sleeves**, and like both it invents
//! nothing: the conventions, the magenta cover mark, the luma-percentile shell tint and the edge
//! bleed are copied rather than reinvented, because a sleeve is drawn by the same pipeline at a
//! different shape and there is no reason for the three to differ.
//!
//! # Where the art comes from
//!
//! `System/Carts/cd.png` — one file, beside the four the carts and the cassette already use.
//! Absent, the built-in outline and flat shell colour are drawn and nothing changes; present, the
//! card's own art **is** the sleeve, and the shell colour is applied to it as a multiplier rather
//! than painted through a mask.
//!
//! # The one convention that reads differently here
//!
//! * **Alpha is the shape.** A transparent pixel is not a sleeve.
//! * **Magenta is the cover.** `#ff00ff` marks where the album art goes, and the code cuts that
//!   region out, fills it with the colour just outside it, and pastes the cover in clipped to the
//!   same shape — so a rounded cover keeps its corners.
//!
//! **On a cassette that region is a strip and on a sleeve it is nearly the whole face**, and that
//! is the only real difference between the two paths. Both are "paste here"; one covers a third
//! of the object and the other covers all of it. The artist decides by drawing the mark, which
//! is the same contract as everywhere else: **the art says where its own cover goes, and nothing
//! has to be written down twice.**
//!
//! **The mark's tolerance is not restated here.** `find_label` is `tape_art`'s, so the numbers
//! that decide "is this pixel magenta" live in exactly one place; a second copy would be a
//! second thing to keep in step, and a card that found one would not find the other.

use crate::art;
use crate::cart::{CartFace, CartSize};
use crate::shell::Shell;

use super::tape_art::{bleed_edge, find_label, reference_luma, ring_colour};

// ★★ 2026-10-28（六六）：「4 你重定位到 Music/Covers，把带洋红区的光盘盒移过去」——
//   封套模板和封面现在住同一处（`Music/Covers/cd.png`），音乐那一摊的东西不再散在
//   `System/Carts/` 里。老卡上 `System/Carts/cd.png` 仍然认（`OLD_DIR`），一个人手上的卡
//   不该因为文件搬了家而突然没有盒面。
const DIR: &str = "Music/Covers";
const OLD_DIR: &str = "System/Carts";
/// 模板自己的文件名。`Covers::scan` 会跳过它 —— 否则一张叫 `cd` 的碟会拿盒面当封面。
pub const FILE: &str = "cd.png";

/// The sleeve's size, aliased so the code below reads in this module's own terms.
const SIZE: CartSize = crate::cd::SIZE;

/// The card's sleeve art: the picture, where its cover goes, and the two numbers the shell tint
/// and the empty recess need.
///
/// **The `Label` is `tape_art`'s, not a copy of it.** It became `pub(crate)` when this file
/// needed it, which is the honest form of "these two paths are one algorithm" — a second struct
/// with the same four numbers and a `mask` would be a second thing to keep in step.
pub struct CdArt {
    rgba: Vec<u8>,
    w: u32,
    h: u32,
    label: Option<super::tape_art::Label>,
    /// Brightness of the plastic, for the shell tint.
    luma: f32,
    /// The colour just outside the cover, so the area can be filled before the cover goes in.
    recess: [u8; 3],
}

static ART: std::sync::OnceLock<Option<CdArt>> = std::sync::OnceLock::new();

/// Called once at boot, before the face builder starts. `None` is a legitimate value and is
/// installed as such, so a card with no sleeve art is asked exactly once.
pub fn install(art: Option<CdArt>) {
    let _ = ART.set(art);
}

/// The card's sleeve art, if it supplied any.
pub fn art() -> Option<&'static CdArt> {
    ART.get()?.as_ref()
}

/// Read the card's sleeve art. A missing, unreadable or undersized file is simply not there: the
/// built-in sleeve is a perfectly good sleeve.
pub fn load(root: &std::path::Path) -> Option<CdArt> {
    let read = |dir: &str| art::decode_rgba(&root.join(dir).join(FILE));
    let (rgba, w, h) = read(DIR).or_else(|| read(OLD_DIR))?;
    if w < 8 || h < 8 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    let mut rgba = rgba;
    // The same cut-out halo the carts and the cassette get, for the same reason: a cut-out
    // carries the colour it was cut from in its semi-transparent pixels, which is a dark rim the
    // moment the art is painted light.
    bleed_edge(&mut rgba, w, h);
    let label = find_label(&rgba, w, h);
    let luma = reference_luma(&rgba, w, h, label.as_ref());
    let recess = label
        .as_ref()
        .map(|l| ring_colour(&rgba, w, h, l))
        .unwrap_or([80, 80, 80]);
    Some(CdArt {
        rgba,
        w,
        h,
        label,
        luma,
        recess,
    })
}

/// What the card has, for the startup log.
pub fn summary() -> String {
    match art() {
        None => "none".to_string(),
        Some(a) => a.summary(),
    }
}

/// The same line, for art that has been read but not installed.
///
/// **Separate from `summary` because the two are asked at different moments**: `summary` answers
/// for the installed art at boot, and this one lets a caller that has just loaded a file say what
/// it got before deciding to install it. The wording is identical on purpose — two formats for one
/// fact is one more thing to keep in step.
pub fn summary_of(a: &CdArt) -> String {
    a.summary()
}

/// The cover area as a share of the face — the sleeve's answer to `cart::label_share`, so the
/// favourites star can sit on the same corner of the same cover it does on a cart.
pub fn share() -> Option<(f32, f32, f32, f32)> {
    art()?.label_share()
}

/// The cover area in face pixels — the size a cover for this card's sleeve has to be, and the
/// size a cover is read at. `None` when the card's art carries no magenta, which is the answer
/// rather than a zero: a card with no marked area has nothing to paste into.
pub fn slot_size() -> (u32, u32) {
    art().and_then(|a| a.label_size(SIZE)).unwrap_or((0, 0))
}

/// Read a sleeve's cover: `path` is the file, `w` by `h` the size the area needs. `None` when
/// there is no cover, which is not a failure — a sleeve with a blank face is a sleeve.
pub fn read_cover(path: &std::path::Path, w: u32, h: u32) -> Option<Vec<u8>> {
    if w == 0 || h == 0 {
        return None;
    }
    art::cover(path, w, h)
}

impl CdArt {
    /// The startup line for this art. See the free `summary_of` for why it is a method too.
    pub fn summary(&self) -> String {
        match &self.label {
            Some(_) => format!("cd {}x{}+cover", self.w, self.h),
            None => format!("cd {}x{} (no cover mark)", self.w, self.h),
        }
    }

    /// The cover area as a share of the face.
    pub fn label_share(&self) -> Option<(f32, f32, f32, f32)> {
        let l = self.label.as_ref()?;
        let (w, h) = (self.w as f32, self.h as f32);
        Some((
            l.x0 as f32 / w,
            l.y0 as f32 / h,
            (l.x1 - l.x0) as f32 / w,
            (l.y1 - l.y0) as f32 / h,
        ))
    }

    /// The cover area in face pixels against a size.
    pub fn label_size(&self, size: CartSize) -> Option<(u32, u32)> {
        let (w, h) = (size.face_w(), size.face_h());
        let l = self.label.as_ref()?;
        let sx = self.w as f32 / w as f32;
        let sy = self.h as f32 / h as f32;
        Some((
            (((l.x1 - l.x0) as f32 / sx).round() as u32).max(1),
            (((l.y1 - l.y0) as f32 / sy).round() as u32).max(1),
        ))
    }

    /// The face the row draws, from the card's art when there is any and the built-in sleeve when
    /// there is not.
    ///
    /// **Line for line `tape_art::Art::face`**, because the two differ only in which file the
    /// picture came from and how big the shape is — the cut-out, the masked recess fill, the
    /// luma-percentile tint and the clipped paste are one algorithm written once per shape, and
    /// a second version of it is a second set of artefacts to find. What a sleeve changes is
    /// only that the marked region is most of the face rather than a strip of it.
    pub fn face(
        &self,
        shell: &Shell,
        cover: &[u8],
        cover_size: (u32, u32),
        face_size: (u32, u32),
        light: Option<[u8; 3]>,
    ) -> CartFace {
        let (cover_w, cover_h) = cover_size;
        let (face_w, face_h) = face_size;
        let mut rgba = vec![0u8; (face_w * face_h * 4) as usize];
        let sx = self.w as f32 / face_w as f32;
        let sy = self.h as f32 / face_h as f32;
        let l = self.label.as_ref();
        let (lx, ly) = match l {
            Some(l) => (
                (l.x0 as f32 / sx).round() as i64,
                (l.y0 as f32 / sy).round() as i64,
            ),
            None => (0, 0),
        };
        for y in 0..face_h {
            let sy0 = (y as f32 * sy) as u32;
            for x in 0..face_w {
                let sx0 = (x as f32 * sx) as u32;
                let p = &self.rgba[((sy0.min(self.h - 1) * self.w + sx0.min(self.w - 1)) * 4)
                    as usize..][..4];
                if p[3] == 0 {
                    continue;
                }
                // The cover goes on the magenta and **only** on the magenta, so a mark the artist
                // left off — the spine, the shell — keeps the sleeve's own art and shows through.
                // The mask is what makes a cut-out work; the rectangle only says where to look.
                let mx = l.map(|l| l.x1 - l.x0 + 1).unwrap_or(0) as f32;
                let my = l.map(|l| l.y1 - l.y0 + 1).unwrap_or(0) as f32;
                let marked = match l {
                    Some(l) if cover_w > 0 && cover_h > 0 && (x as i64) >= lx
                        && (x as i64) < lx + cover_w as i64
                        && (y as i64) >= ly
                        && (y as i64) < ly + cover_h as i64 =>
                    {
                        let ax = l.x0 as f32 + (x as i64 - lx) as f32 * mx / cover_w as f32;
                        let ay = l.y0 as f32 + (y as i64 - ly) as f32 * my / cover_h as f32;
                        l.mask[((ay.min(self.h as f32 - 1.0)) as u32 * self.w
                            + ax.min(self.w as f32 - 1.0) as u32) as usize]
                            != 0
                    }
                    _ => false,
                };
                let src = if marked {
                    // The recess the cover sits in: the ring just outside the marked area at
                    // 78%, and **masked as well as filled**. A rectangle fill would paint over
                    // everything inside the bounding box — the spine included — and a cover with
                    // a cut-out corner would show that flat paint instead of the sleeve.
                    [
                        (self.recess[0] as f32 * 0.78) as u8,
                        (self.recess[1] as f32 * 0.78) as u8,
                        (self.recess[2] as f32 * 0.78) as u8,
                        255,
                    ]
                } else {
                    let luma = 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
                    let (base, k) = match crate::palette::mode() {
                        crate::palette::Mode::Light => match light {
                            None => ([p[0], p[1], p[2]], 1.0),
                            Some(flat) => (flat, (luma / self.luma).clamp(0.0, 1.6)),
                        },
                        crate::palette::Mode::Dark => {
                            (shell.colour, (luma / self.luma).clamp(0.0, 1.6))
                        }
                    };
                    [
                        (base[0] as f32 * k).min(255.0) as u8,
                        (base[1] as f32 * k).min(255.0) as u8,
                        (base[2] as f32 * k).min(255.0) as u8,
                        p[3],
                    ]
                };
                let d = ((y * face_w + x) * 4) as usize;
                rgba[d..d + 4].copy_from_slice(&src);
            }
        }
        // The cover itself, clipped to whatever shape the card drew its area with.
        if let (Some(l), true) = (l, cover_w > 0 && cover_h > 0) {
            let mx = l.x1 - l.x0 + 1;
            let my = l.y1 - l.y0 + 1;
            for y in 0..cover_h {
                for x in 0..cover_w {
                    let ax = l.x0 + (x as f32 * mx as f32 / cover_w as f32) as u32;
                    let ay = l.y0 + (y as f32 * my as f32 / cover_h as f32) as u32;
                    if l.mask[(ay.min(self.h - 1) * self.w + ax.min(self.w - 1)) as usize] == 0 {
                        continue;
                    }
                    let s = ((y * cover_w + x) * 4) as usize;
                    let a = cover[s + 3] as u32;
                    if a == 0 {
                        continue;
                    }
                    let d = ((((y as i64 + ly).max(0) as u32) * face_w
                        + (x as i64 + lx).max(0) as u32)
                        * 4) as usize;
                    if d + 4 > rgba.len() {
                        continue;
                    }
                    for c in 0..3 {
                        let dst = rgba[d + c] as u32;
                        let src = cover[s + c] as u32;
                        rgba[d + c] = ((src * a + dst * (255 - a) + 127) / 255) as u8;
                    }
                }
            }
        }
        CartFace {
            rgba,
            w: face_w,
            h: face_h,
        }
    }
}

/// The face the row draws, from the card's art when there is any and the built-in sleeve when
/// there is not. See `tape_art::face` for why an empty cover must not be a zero-width one over
/// a real slot: the row bakes its furniture before any sleeve is chosen, so "no cover yet" is
/// the normal state and it panicked on the device once already.
pub fn face(cover: &[u8], shell: &Shell, light: Option<[u8; 3]>) -> CartFace {
    let have_cover = cover.len() >= 4;
    match art() {
        Some(a) => {
            let (cw, ch) = if have_cover {
                a.label_size(SIZE).unwrap_or((0, 0))
            } else {
                (0, 0)
            };
            a.face(
                shell,
                cover,
                (cw, ch),
                (SIZE.face_w(), SIZE.face_h()),
                light,
            )
        }
        None => crate::cd::face_in_shell(shell, cover),
    }
}
