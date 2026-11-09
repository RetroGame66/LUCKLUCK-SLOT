//! Tape artwork the card supplies, in place of the built-in silhouette and moulding.
//!
//! **This is `cart_art.rs` again, for tapes.** Nothing here is invented: the conventions, the
//! magenta label mark, the luma-percentile shell tint and the edge bleed are all copied from the
//! cart path, because a tape is drawn by the same pipeline at a different size and there is no
//! reason for the two to differ. Read `cart_art.rs` first; this file is the short version.
//!
//! # Where the tape's art comes from
//!
//! `System/Carts/tape.png` — one file, beside the three the carts already use. Absent, the
//! built-in outline and flat shell colour are drawn and nothing changes; present, the card's own
//! art **is** the tape, and the shell colour is applied to it as a multiplier rather than painted
//! through a mask. So a card that ships one clear-shell tape and one red one gets both from the
//! same code by shipping two files, exactly as a card ships three cart files for three machines.
//!
//! # The two conventions, unchanged
//!
//! * **Alpha is the shape.** A transparent pixel is not tape. The rounded corners, the side
//!   notches and the bottom plate's outline are all cut this way.
//! * **Magenta is the label.** `#ff00ff` marks where the album art goes. The code cuts that region
//!   out, fills it with the colour just outside it, and pastes the label in clipped to the same
//!   shape — so a rounded label keeps its corners.
//!
//! The art says where its own label goes; nothing has to be written down twice.
//!
//! # Why the art has to be prepared, and how
//!
//! The card's art is a **photograph-grade render** (六六's own, 2026-10-03), not a drawing. Two
//! things follow, and both are the artist's to decide rather than the code's:
//!
//! * **The label slot must be marked in magenta**, because the code finds the slot that way and
//!   has no other way to know. A render that leaves the slot white gives a tape whose album art
//!   has nowhere to go, silently.
//! * **The reel window is part of the art.** The window and the two reels are *in* the picture,
//!   which is what makes this path the right one: a cassette's most characteristic feature is that
//!   you can see the tape through its face, and that cannot be faked with a mask over a flat
//!   colour. It also means the winding cannot be animated — see `winding` below.
//!
//! # The art's resolution, and the sticker's — measured off the card, not assumed
//!
//! Two assets, two rules, and they are **not** the same rule. Measured on 2026-10-03 from the
//! files the card already ships (`System/Carts/`, `Labels_logo/`):
//!
//! ```text
//! cart_gba.png    360x202  =  240x135 logical x 1.5   the centre cart's real size
//! Labels_logo/    394x171  =  the label box at 1:1     not scaled up
//! board_gba.png   744x418  =  drawn at 1.55x into the core picker's panel — a different rule
//! ```
//!
//! **So: a tape's art is 339x202 — its logical 226x135 at the centre scale of 1.5 — and its
//! sticker is 396x84 at 1:1.** The art is pixel-for-pixel the centre cart, so nothing interpolates
//! where the eye looks hardest; the neighbours draw at 226x135 and are a downsample, which is the
//! trade `cart_gba.png` has made all along.
//!
//! Worth writing down because the obvious move is wrong: `board.svg` says *"sharp only at its
//! own size"*, and applying that to a tape gives 904x540 — the **face** doubled, i.e. the board's
//! rule on the wrong asset. The face is 452x270 (`SIZE` x `FACE_SCALE` 2) and nothing is ever
//! drawn at the face's full size, so paying for it buys nothing.
//!
//! `bleed_edge` and the alpha ramp both want the art at a size where a pixel is a pixel, and
//! 339x202 is comfortably past that; going further is disk and decode time for no visible gain.
//!
//! # The winding, and why this path gives it up
//!
//! The reels in the art are a still frame. A real cassette's two hubs counter-rotate and trade
//! radius as the tape moves across (area conservation, `r_left² + r_right² = hub² + full²`), and
//! on 六六's own drawing that is a 21% radius swing against a real cassette's 56% — small enough
//! that it does not carry a screen on its own. Animating it would mean the art being a shell plus
//! a separately-drawn mechanism, which is a different asset and a different pipeline. **So: the
//! tape is a picture of a cassette, and that is the trade for having it look like one.**

use std::path::Path;
use std::sync::OnceLock;

use crate::art;
use crate::cart::{CartFace, CartSize};
use crate::shell::Shell;
use crate::tape::SIZE;

/// Where the card keeps it, beside the carts' own three.
const DIR: &str = "System/Carts";
const FILE: &str = "tape.png";

/// The magenta the label slot is marked with, and the tolerance. Copied from `cart_art.rs`
/// unchanged, and for the same reason: the art comes back through a paint program, and `#ff00ff`
/// with green held well below the other two is what every magenta meant as a marker looks like
/// and no shell colour does.
const MARK_R: u8 = 200;
const MARK_G: u8 = 60;
const MARK_B: u8 = 200;

/// 标记向外多长几格 —— **它自己那圈抗锯齿也是标记**。见 `find_label`。
///
/// 为什么非有不可：这几张图都是画图软件出来的，洋红区的边界是 **1~2 格洋红与背景混出来的**，
/// `(248,87,246)`（接浅壳）、`(85,34,89)`（接深壳）都过不了 `is_mark`。它们既不被填
/// recess、也不被贴纸盖住，而卡不给 `light` 时就照原样画 ⇒ **亮色主题下就是
/// 贴纸周围一圈洋红**（六六 2026-10-07：「DISC 货架我切回亮色模式发现封面区还有点洋红边」）。
/// 深色主题看不见，因为那边这一圈是用 `shell.colour` 重涂的。
///
/// **2 是量出来的，不是拍的**：卡上 `cd.png` 是 400×370，贴到 520×520 的面上是
/// **放大**，art 里 1 格混色到屏上是 2 行；只长 1 格 ⇒ 面里还剩 **1403** 个洋红像素，
/// 长 2 格 ⇒ **0**（实测，`.workbuddy/memory/MEMORY-排错与工具.md` §25）。坐标框同步外扩，
/// 否则贴纸会被截掉这一圈。
const MARK_GROW: u32 = 2;

/// How bright the plastic is taken to be, as a percentile of its own pixels. The 90th, not the
/// median — see `cart_art.rs`: the median scaled half the art up and the shell's brightest
/// highlight blew out to white.
const REF_PERCENTILE: f32 = 0.90;

/// The label slot, in the art's own pixels. `mask` is the slot's coverage rather than its
/// rectangle, so a rounded slot keeps its corners when the label is pasted into it.
///
/// **`pub(crate)` in every field, not just the struct**: the sleeve path (`cd_art`) reads the
/// same four numbers off the same shape, because it is the same algorithm pointed at a different
/// file. Accessors would be the tidier way to say that, and would also be four more functions to
/// keep in step with the struct.
pub(crate) struct Label {
    pub(crate) x0: u32,
    pub(crate) y0: u32,
    pub(crate) x1: u32,
    pub(crate) y1: u32,
    pub(crate) mask: Vec<u8>,
}

pub struct TapeArt {
    rgba: Vec<u8>,
    w: u32,
    h: u32,
    label: Option<Label>,
    /// Brightness of the plastic, for the shell tint.
    luma: f32,
    /// The colour just outside the slot, so the slot can be filled before the label goes in.
    recess: [u8; 3],
}

static ART: OnceLock<Option<TapeArt>> = OnceLock::new();

/// Called once at boot, before the face builder starts. `None` is a legitimate value and is
/// installed as such, so a card with no tape art is asked exactly once.
pub fn install(art: Option<TapeArt>) {
    let _ = ART.set(art);
}

/// The card's tape art, if it supplied any.
pub fn art() -> Option<&'static TapeArt> {
    ART.get()?.as_ref()
}

/// Read the card's tape art. A missing, unreadable or undersized file is simply not there: the
/// built-in tape is a perfectly good tape.
pub fn load(root: &Path) -> Option<TapeArt> {
    let (rgba, w, h) = art::decode_rgba(&root.join(DIR).join(FILE))?;
    if w < 8 || h < 8 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    let mut rgba = rgba;
    // The same cut-out halo the carts get, for the same reason: a cut-out carries the colour it
    // was cut from in its semi-transparent pixels, which is a dark rim the moment the art is
    // painted light. Three 3x3 passes inward, colour only, alpha untouched.
    bleed_edge(&mut rgba, w, h);
    let label = find_label(&rgba, w, h);
    let luma = reference_luma(&rgba, w, h, label.as_ref());
    let recess = label
        .as_ref()
        .map(|l| ring_colour(&rgba, w, h, l))
        .unwrap_or([80, 80, 80]);
    Some(TapeArt {
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
        Some(a) => match &a.label {
            Some(_) => format!("tape {}x{}+label", a.w, a.h),
            None => format!("tape {}x{} (no label mark)", a.w, a.h),
        },
    }
}

fn is_mark(p: &[u8]) -> bool {
    p[3] > 0 && p[0] > MARK_R && p[1] < MARK_G && p[2] > MARK_B
}

/// The magenta region, as one rectangle plus its coverage. Copied from `cart_art.rs`; a slot is
/// one region on a tape, and anything more elaborate would be a second way to say the same thing.
pub(crate) fn find_label(rgba: &[u8], w: u32, h: u32) -> Option<Label> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if is_mark(&rgba[((y * w + x) * 4) as usize..][..4]) {
                any = true;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if !any {
        return None;
    }
    let mut mask = vec![0u8; (w * h) as usize];
    for y in y0..=y1 {
        for x in x0..=x1 {
            if is_mark(&rgba[((y * w + x) * 4) as usize..][..4]) {
                mask[(y * w + x) as usize] = 255;
            }
        }
    }
    // ★ 标记连同它自己那圈抗锯齿一起算 —— 见 `MARK_GROW`。**读快照不读自己**：
    // 一边长一边读会让刚长出去的一圈再长一圈，那就不是"两格"而是"一直长到碰边"。
    let base = mask.clone();
    let r = MARK_GROW as i32;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if base[i] != 0 {
                continue;
            }
            'ring: for dy in -r..=r {
                for dx in -r..=r {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    if base[(ny as u32 * w + nx as u32) as usize] != 0 {
                        mask[i] = 255;
                        break 'ring;
                    }
                }
            }
        }
    }
    Some(Label {
        x0: x0.saturating_sub(MARK_GROW),
        y0: y0.saturating_sub(MARK_GROW),
        x1: (x1 + MARK_GROW).min(w - 1),
        y1: (y1 + MARK_GROW).min(h - 1),
        mask,
    })
}

pub(crate) fn reference_luma(rgba: &[u8], w: u32, h: u32, label: Option<&Label>) -> f32 {
    let mut lums = Vec::new();
    for y in (0..h).step_by(3) {
        for x in (0..w).step_by(3) {
            let inside = label.is_some_and(|l| x >= l.x0 && x <= l.x1 && y >= l.y0 && y <= l.y1);
            let p = &rgba[((y * w + x) * 4) as usize..][..4];
            if p[3] > 200 && !inside {
                lums.push(0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32);
            }
        }
    }
    if lums.is_empty() {
        return 128.0;
    }
    lums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let i = ((lums.len() - 1) as f32 * REF_PERCENTILE) as usize;
    lums[i].max(1.0)
}

pub(crate) fn ring_colour(rgba: &[u8], w: u32, h: u32, l: &Label) -> [u8; 3] {
    let mut samples: Vec<[u8; 3]> = Vec::new();
    let mut take = |x: u32, y: u32| {
        if x < w && y < h {
            let p = &rgba[((y * w + x) * 4) as usize..][..4];
            if p[3] > 0 {
                samples.push([p[0], p[1], p[2]]);
            }
        }
    };
    for x in l.x0..=l.x1 {
        take(x, l.y0.saturating_sub(3));
        take(x, (l.y1 + 3).min(h - 1));
    }
    for y in l.y0..=l.y1 {
        take(l.x0.saturating_sub(3), y);
        take((l.x1 + 3).min(w - 1), y);
    }
    if samples.is_empty() {
        return [80, 80, 80];
    }
    let mut out = [0u8; 3];
    for c in 0..3 {
        let mut v: Vec<u8> = samples.iter().map(|s| s[c]).collect();
        v.sort_unstable();
        out[c] = v[v.len() / 2];
    }
    out
}

pub(crate) fn bleed_edge(rgba: &mut [u8], w: u32, h: u32) {
    for _ in 0..3 {
        let src = rgba.to_vec();
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let a = src[i + 3];
                if a == 0 || a == 255 {
                    continue;
                }
                let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                            continue;
                        }
                        let j = ((ny as u32 * w + nx as u32) * 4) as usize;
                        if src[j + 3] == 255 {
                            r += src[j] as u32;
                            g += src[j + 1] as u32;
                            b += src[j + 2] as u32;
                            n += 1;
                        }
                    }
                }
                if n == 0 {
                    // A pixel with no opaque neighbour keeps the colour it already had.
                    continue;
                }
                rgba[i] = (r / n) as u8;
                rgba[i + 1] = (g / n) as u8;
                rgba[i + 2] = (b / n) as u8;
            }
        }
    }
}

impl TapeArt {
    /// The label slot as a share of the art, for a caller aligning something to it.
    pub fn label_share(&self) -> Option<(f32, f32, f32, f32)> {
        let l = self.label.as_ref()?;
        Some((
            l.x0 as f32 / self.w as f32,
            l.y0 as f32 / self.h as f32,
            (l.x1 + 1) as f32 / self.w as f32,
            (l.y1 + 1) as f32 / self.h as f32,
        ))
    }

    /// Read a tape's sticker: `path` is the file, `w` by `h` the size the slot needs. `None` when
/// there is no sticker, which is not a failure — a tape with a blank label is a tape.
pub fn read_sticker(path: &std::path::Path, w: u32, h: u32) -> Option<Vec<u8>> {
    if w == 0 || h == 0 {
        return None;
    }
    crate::art::cover(path, w, h)
}

/// The label slot as a share of the face — the tape's answer to `cart::label_share`, so the
/// favourites star can sit on the same corner of the same sticker it does on a cart.
///
/// Named `share` rather than `label_share` because `Art::label_share` is the method on the
/// art; the free function is the same question asked of the installed art, and two things in
/// one module cannot share a name.
pub fn share() -> Option<(f32, f32, f32, f32)> {
    art()?.label_share()
}

/// The label slot in face pixels, so the caller can mint a label of the right size. The
    /// target face is the tape's own, which is why this cannot be a constant.
    pub fn label_size(&self, size: CartSize) -> Option<(u32, u32)> {
        let l = self.label.as_ref()?;
        let w = ((l.x1 - l.x0 + 1) as f32 * size.face_w() as f32 / self.w as f32).round() as u32;
        let h = ((l.y1 - l.y0 + 1) as f32 * size.face_h() as f32 / self.h as f32).round() as u32;
        (w > 0 && h > 0).then_some((w, h))
    }

    /// The face the deck draws: this art in the tape's shell colour, with `label` pasted into the
    /// slot the art marks out.
    ///
    /// The colour is a **multiplier on the art's own brightness** — `shell * luma/luma` for the
    /// shell's own value at the reference brightness — which is the same arithmetic the cart path
    /// and the built-in path both use, and the reason a clear-shell art keeps its highlights when
    /// the shell colour changes. Tinting rather than recolouring: the art's own shading is the
    /// light on the object, and replacing the colour would throw that away.
    ///
    /// `light` is what the tape becomes on the light theme: `Some(grey)` to repaint it flat,
    /// `None` to use the art's own colour. Same rule as the carts, and for the same reason — a
    /// clear-shell tape already looks right on a light ground and a black one does not.
    pub fn face(
        &self,
        shell: &Shell,
        label: &[u8],
        label_size: (u32, u32),
        face_size: (u32, u32),
        light: Option<[u8; 3]>,
    ) -> CartFace {
        let (label_w, label_h) = label_size;
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
                // The sticker goes on the magenta and **only** on the magenta, so a mark the
                // artist left off — the reel window, a screw, the shell — keeps the tape's own
                // art and shows through the sticker. The mask is what makes a cut-out work; the
                // rectangle only says where to look.
                let mx = l.map(|l| l.x1 - l.x0 + 1).unwrap_or(0) as f32;
                let my = l.map(|l| l.y1 - l.y0 + 1).unwrap_or(0) as f32;
                let marked = match l {
                    Some(l) if label_w > 0 && label_h > 0 && (x as i64) >= lx
                        && (x as i64) < lx + label_w as i64
                        && (y as i64) >= ly
                        && (y as i64) < ly + label_h as i64 =>
                    {
                        // Which pixel of the art this face pixel came from, and is it marked?
                        let ax = l.x0 as f32 + (x as i64 - lx) as f32 * mx / label_w as f32;
                        let ay = l.y0 as f32 + (y as i64 - ly) as f32 * my / label_h as f32;
                        l.mask[((ay.min(self.h as f32 - 1.0)) as u32 * self.w
                            + ax.min(self.w as f32 - 1.0) as u32) as usize]
                            != 0
                    }
                    _ => false,
                };
                let src = if marked {
                    // The recess the label sits in: the ring just outside the slot at 78%, so a
                    // rounded slot's corners — which were magenta and have nothing behind them
                    // either — read as the well the sticker is stuck into. `cart_art.rs`'s rule
                    // unchanged; what is new here is that the **fill is masked too**.
                    //
                    // That is the whole cut-out fix. A rectangle fill paints over everything
                    // inside the slot's bounding box, the reel window included, and then a
                    // sticker with a hole in it shows that flat paint instead of the tape. With
                    // the fill masked, the unmarked window is never touched by either.
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
                        crate::palette::Mode::Dark => (shell.colour, (luma / self.luma).clamp(0.0, 1.6)),
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
        // The label itself, clipped to whatever shape the card drew its slot with.
        if let (Some(l), true) = (l, label_w > 0 && label_h > 0) {
            let mx = l.x1 - l.x0 + 1;
            let my = l.y1 - l.y0 + 1;
            for y in 0..label_h {
                for x in 0..label_w {
                    let ax = l.x0 + (x as f32 * mx as f32 / label_w as f32) as u32;
                    let ay = l.y0 + (y as f32 * my as f32 / label_h as f32) as u32;
                    if l.mask[(ay.min(self.h - 1) * self.w + ax.min(self.w - 1)) as usize] == 0 {
                        continue;
                    }
                    let s = ((y * label_w + x) * 4) as usize;
                    let a = label[s + 3] as u32;
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
                        let src = label[s + c] as u32;
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

/// Read a tape's sticker: `path` is the file, `w` by `h` the size the slot needs. `None` when
/// there is no sticker, which is not a failure — a tape with a blank label is a tape.
pub fn read_sticker(path: &std::path::Path, w: u32, h: u32) -> Option<Vec<u8>> {
    if w == 0 || h == 0 {
        return None;
    }
    crate::art::cover(path, w, h)
}

/// The label slot as a share of the face — the tape's answer to `cart::label_share`, so the
/// favourites star can sit on the same corner of the same sticker it does on a cart.
///
/// Named `share` rather than `label_share` because `Art::label_share` is the method on the
/// art; the free function is the same question asked of the installed art, and two things in
/// one module cannot share a name.
pub fn share() -> Option<(f32, f32, f32, f32)> {
    art()?.label_share()
}

/// The label slot in face pixels — the size a sticker for this card's tape has to be, and the
/// size a sticker is read at. `None` when the card's art carries no magenta, which is the answer
/// rather than a zero: a card with no slot has nothing to paste into.
pub fn slot_size() -> (u32, u32) {
    art().and_then(|a| a.label_size(SIZE)).unwrap_or((0, 0))
}

/// The face the deck draws, from the card's art when there is any and the built-in tape when there
/// is not. **The one function the tape side calls**, so the fallback lives in one place and a card
/// with no art behaves exactly as it did before this file existed.
pub fn face(label: &[u8], shell: &Shell, light: Option<[u8; 3]>) -> CartFace {
    // **An empty sticker must be an empty sticker, not a zero-width one over a real slot.**
    // `label_size` is the slot's size, and passing it with no label bytes behind panics on the
    // first pixel: the paste loop indexes `label` for its alpha and there is nothing there. The
    // deck bakes its furniture before any tape is chosen, so "no sticker yet" is the normal
    // state, not an edge case — and it panicked on the device rather than in a check, because
    // nothing in the host test suite builds a face.
    let have_label = label.len() >= 4;
    match art() {
        Some(a) => {
            // No label mark in the art means no label, and a label sized for the built-in panel
            // would be pasted over the picture's own middle. An unlabelled tape gets an unlabelled
            // face, which is the truthful answer rather than a wrong-sized paste.
            let (lw, lh) = if have_label {
                a.label_size(SIZE).unwrap_or((0, 0))
            } else {
                (0, 0)
            };
            a.face(
                shell,
                label,
                (lw, lh),
                (SIZE.face_w(), SIZE.face_h()),
                light,
            )
        }
        None => crate::tape::face_in_shell(shell, label),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// A `w×h` sheet: a magenta block of half-width `m`, **wrapped in one pixel of magenta
    /// blended halfway into the shell** — `(248,87,246)`, which is the value measured on the
    /// card's own `cd.png` where the mark meets the pale shell. That pixel fails `is_mark`.
    fn art_with_a_blended_edge(w: u32, h: u32, m: u32) -> Vec<u8> {
        let mut px = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let d = [x, y, w - 1 - x, h - 1 - y].into_iter().min().unwrap();
                let c: [u8; 4] = if d > m {
                    [255, 0, 255, 255]
                } else if d == m {
                    [248, 87, 246, 255]
                } else {
                    [40, 40, 44, 255]
                };
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 4].copy_from_slice(&c);
            }
        }
        px
    }

    /// ★ 2026-10-07 六六：「DISC 货架我切回亮色模式发现封面区还有点洋红边」。
    ///
    /// That ring is **not** the sticker failing to fill its slot. It is the one pixel of blend a
    /// paint program leaves along the mark, and it fails `is_mark` — so it is neither filled with
    /// the recess nor pasted over, and a card that passes no `light` colour draws it **as the art
    /// itself**. The dark theme never showed it because there that pixel comes out in
    /// `shell.colour`, which is exactly why it went unnoticed.
    ///
    /// The guard is the rule — "a mark includes its own anti-aliased edge" — and not a pixel
    /// count: it says the blend is inside the mask and that the mask stops where `MARK_GROW` says.
    #[test]
    fn the_mark_swallows_its_own_anti_aliased_edge() {
        let (w, h, m) = (40u32, 40u32, 10u32);
        let px = art_with_a_blended_edge(w, h, m);
        let l = find_label(&px, w, h).expect("a magenta block has to be found");

        // The strict block is `m + 1 ..= w - 2 - m`; the box has to have grown by `MARK_GROW` on
        // every side or the expanded mask would be cut off again by the paste.
        let (lo, hi) = (m + 1, w - 2 - m);
        assert_eq!((l.x0, l.y0), (lo - MARK_GROW, lo - MARK_GROW), "box did not grow");
        assert_eq!((l.x1, l.y1), (hi + MARK_GROW, hi + MARK_GROW), "box did not grow");

        let at = |x: u32, y: u32| l.mask[(y * w + x) as usize];
        assert_ne!(at(m, 20), 0, "the blended ring was left out of the mark");
        assert_ne!(at(lo - MARK_GROW, 20), 0, "the last grown pixel was left out");
        assert_eq!(at(lo - MARK_GROW - 1, 20), 0, "the grow ran past MARK_GROW");
        assert_eq!(at(0, 0), 0, "the shell in the corner was eaten");
    }
}
