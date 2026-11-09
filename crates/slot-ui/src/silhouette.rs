use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use slot_store::System;

// ★★ 2026-10-24（六六）：「阴影还是最早期画的那个丑东西……**把那几个默认的 svg 都解除绑定
// 然后删掉**，目前已经是卡带贴图了也不怕透光了」。
//
// 所以这里**只剩磁带与 CD 两个 SVG**（音乐行那一对，跟卡带无关）。
// 卡带（GBA / GB / GBC）的轮廓不再由手绘 SVG 给出 —— 它**只从卡上那张贴图的 alpha 来**
// （`System/Carts/cart_gba.png` / `cart_gb.png` / `cart_gbc.png`）。
// 好处不只是"少一套要跟贴图对齐的图"：轮廓与贴图**永远是同一个形状**，
// 贴图换了轮廓自动跟着换，不存在"改了图忘了改剪影"。
//
// 卡上若连那张贴图都没有，掩膜退化成**一整块**（`vec![255; …]`，见 `masks`）——
// 一块平板的卡片，而不是一个凭空画出来的假轮廓。
const CART_TAPE_SVG: &str = include_str!("../assets/tape.svg");
const CART_CD_SVG: &str = include_str!("../assets/cd.svg");
const DETAIL_TAPE_SVG: &str = include_str!("../assets/tape_detail.svg");
const DETAIL_CD_SVG: &str = include_str!("../assets/cd_detail.svg");

/// What a face is cut to.
///
/// Three of these are machines and two are not, which is the whole reason it is a type of its own
/// rather than `slot_store::System` with a fourth arm: **a tape is not a machine and never will
/// be** — it has no save folder, no core, no `States/` path — but it does need a silhouette, a
/// depth map and a moulding, and those are exactly what this file makes.
///
/// The cassette and the CD sleeve are both here on purpose. **The sleeve is what the music row
/// draws now; the cassette is what it drew before** (2026-10-04), kept so the old look is one
/// line away rather than a `git checkout` away. Nothing in this file chooses between them —
/// which shape a row uses is the row's own `CartSize`, not a property of the format.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Shape {
    Gba,
    Gb,
    Gbc,
    /// A compact cassette. Not a cartridge, and no longer the music row's shape either.
    Tape,
    /// A CD sleeve: square, 1:1, and what the music row is made of as of 2026-10-04.
    Cd,
}

impl From<System> for Shape {
    fn from(system: System) -> Self {
        match system {
            System::Gba => Shape::Gba,
            System::Gb => Shape::Gb,
            System::Gbc => Shape::Gbc,
            System::Pce => Shape::Gba,
            // No console or arcade cartridge outline yet: borrow the Advance's box as a
            // placeholder shell until SFC/FC/NeoGeo silhouettes are drawn. The face is blank
            // regardless (no art).
            //
            // ★ 2026-10-24：**这张兜底现在只影响阴影与内建壳** —— 卡上放了
            // `cart_sfc.png` 之类的本机种贴图，正面就从那张图画（`cart_art::art_for`），
            // 新增机种时若只给了贴图、没画 SVG 轮廓，**保持这里借 Advance**，两个地方就都
            // 自动跟着贴图走，不必再改别的。
            //
            // ★ 2026-10-25：MD 与 PS1 也走这条路（两者都还没有贴图、也没有 SVG 轮廓）——
            // 借 Advance 的**盒子**（240×135，见 `cart::size_for`），与 SFC/FC 到手贴图之前
            // 完全一样。
            //
            // ⚠️ **已知缺口，记录在案、本轮不动**：借盒子必然意味着 `masks` 的缓存键只有
            // `Shape`，于是 `Shape::Gba` 这一格的内建掩膜是拿 **`cart_gba.png` 的 alpha** 建的 ——
            // 也就是说，一个**没有卡上贴图**的 SFC/FC/MD/PS1 卡，内建壳与压模的轮廓是 GBA 的。
            // 这不是本次引入的：SFC/FC 从 2026-10-24 起就是这样，只是那时它们已经有贴图，
            // 走的是 `cart_art::art_for` 的自带 alpha（`Art::face` 用自己的 alpha 裁），
            // 内建这条路没被真正走到。要修得让掩膜按**机种**（而非 Shape）缓存，牵扯
            // `cart.rs` 六处 `cart_mask`/`cart_depth`/`detail_mask` 的签名，属于另一次改动。
            System::Sfc
            | System::Fc
            | System::Neogeo
            | System::Arcade
            | System::Md
            | System::Ps1 => Shape::Gba,
        }
    }
}

impl Shape {
    /// 这个形状对应的机种 —— 卡带的轮廓要靠它去卡上找贴图；磁带与 CD 没有机种。
    fn machine(self) -> Option<System> {
        match self {
            Shape::Gba => Some(System::Gba),
            Shape::Gb => Some(System::Gb),
            Shape::Gbc => Some(System::Gbc),
            Shape::Tape | Shape::Cd => None,
        }
    }
}

impl Shape {
    /// The shell's outline: the Advance cart's landscape shape, the Game Boy family's portrait
    /// one — carrying the top notch for class A and B (the grey and black carts) and not for
    /// class C (Game Boy Color only), which is the physical lockout — the cassette, and the
    /// sleeve.
    /// `None` = **这个形状没有手绘轮廓**，其掩膜只能从卡上的贴图取（见 [`masks`]）。
    /// 三种卡带都是 `None`：那几张 SVG 已按六六 2026-10-24 的要求删掉。
    fn outline(self) -> Option<&'static str> {
        match self {
            Shape::Gba | Shape::Gb | Shape::Gbc => None,
            Shape::Tape => Some(CART_TAPE_SVG),
            Shape::Cd => Some(CART_CD_SVG),
        }
    }

    /// The moulded detail, shaded into the shell rather than drawn in a colour of its own. The
    /// cart's is the grip ridge above the label and the thumb notch below it; the tape's is its
    /// spool window and the two hubs, which is what says "cassette" at a glance; the sleeve's is
    /// its spine, which is the only part of a jewel case that is not under the cover.
    /// `None` = 没有压模细节要另外画：卡带的细节**就长在贴图里**，贴图已经把它画好了。
    fn detail(self) -> Option<&'static str> {
        match self {
            Shape::Gba | Shape::Gb | Shape::Gbc => None,
            Shape::Tape => Some(DETAIL_TAPE_SVG),
            Shape::Cd => Some(DETAIL_CD_SVG),
        }
    }
}

/// Everything one shape's shell needs: shaped, depth-mapped, detailed. Rasterised once, at the
/// first face that asks for it, at that shape's own face size.
struct Masks {
    mask: Vec<u8>,
    depth: Vec<u8>,
    detail: Vec<u8>,
}

/// Cached per shape **and size**. The same shape is legitimately asked for at more than one
/// size — a cart's face is rasterised at twice its logical size, and the outline's own test
/// asks for the logical one — and a mask of the wrong length does not fail here: it builds a
/// face shorter than the one it claims to be, and the panic lands three functions later in
/// `recess_label`, on a card with a game on it, on every boot. So the size is part of the key.
///
/// The entries are leaked rather than dropped: there are as many as there are (shape, size)
/// pairs in a session, which is a handful, and one face is larger than its three masks.
/// What a mask is cached under: the shape **and** the size it was built for.
type MaskKey = (Shape, u32, u32);
/// The cache itself, keyed by `MaskKey`. Named so the static below reads as one thing rather
/// than as three layers of punctuation.
type MaskTable = HashMap<MaskKey, &'static Masks>;

static MASKS: OnceLock<Mutex<MaskTable>> = OnceLock::new();

fn masks(shape: Shape, w: u32, h: u32) -> &'static Masks {
    let table = MASKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut table = table.lock().unwrap_or_else(|e| e.into_inner());
    let entry = table.entry((shape, w, h)).or_insert_with(|| {
        // ★ 掩膜的两个来源，按"谁是真的形状"排序：
        //   1. 手绘 SVG —— 只剩下磁带与 CD；
        //   2. **卡上这个机种的贴图 alpha** —— 卡带的唯一来源。
        //   两样都没有（卡上连 `cart_gba.png` 都没放）才退成一整块平板：那是"没有形状信息"
        //   的诚实表现，而不是编一个轮廓出来。
        let mask = match shape.outline() {
            Some(svg) => rasterise_svg(svg, w, h).unwrap_or_else(|| vec![255; (w * h) as usize]),
            None => shape
                .machine()
                .and_then(crate::cart_art::alpha_for)
                .map(|alpha| resample_alpha(&alpha, w, h))
                .unwrap_or_else(|| vec![255; (w * h) as usize]),
        };
        let depth = depth_map(&mask, w as usize, h as usize);
        let detail = match shape.detail() {
            Some(svg) => rasterise_svg(svg, w, h).unwrap_or_else(|| vec![0; (w * h) as usize]),
            None => vec![0; (w * h) as usize],
        };
        Box::leak(Box::new(Masks { mask, depth, detail }))
    });
    entry
}

/// Coverage of the shape's outline, one byte per pixel, row major.
pub fn silhouette(system: System, w: u32, h: u32) -> &'static [u8] {
    &masks(Shape::from(system), w, h).mask
}

/// The same, for a caller that has a `Shape` rather than a machine — which is every caller that
/// can be handed a tape.
pub fn silhouette_of(shape: Shape, w: u32, h: u32) -> &'static [u8] {
    &masks(shape, w, h).mask
}

/// Every face of one shape is cut the same way, so the mask is rasterised once per shape and
/// multiplied into its faces. Rasterised at the face resolution so it stays crisp at the largest
/// size the shelf draws it.
pub(crate) fn cart_mask(shape: Shape, w: u32, h: u32) -> &'static [u8] {
    silhouette_of(shape, w, h)
}

/// How far inside the outline each pixel sits, in city block steps, saturating at 255. A
/// translucent shell fades from its edge inward and needs the distance, not the coverage.
pub(crate) fn cart_depth(shape: Shape, w: u32, h: u32) -> &'static [u8] {
    &masks(shape, w, h).depth
}

/// The moulded detail, shaded into the shell rather than drawn in a fixed colour, so it belongs
/// to whatever colour the face is.
pub(crate) fn detail_mask(shape: Shape, w: u32, h: u32) -> &'static [u8] {
    &masks(shape, w, h).detail
}

/// Two pass chamfer. Everything off the edge of the buffer counts as outside, so a pixel on the
/// top row is one step in rather than unreachable.
fn depth_map(mask: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut d: Vec<u8> = mask
        .iter()
        .map(|c| if *c > 127 { 255 } else { 0 })
        .collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if d[i] == 0 {
                continue;
            }
            let up = if y == 0 { 0 } else { d[i - w] };
            let left = if x == 0 { 0 } else { d[i - 1] };
            d[i] = d[i].min(up.saturating_add(1)).min(left.saturating_add(1));
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            if d[i] == 0 {
                continue;
            }
            let down = if y + 1 == h { 0 } else { d[i + w] };
            let right = if x + 1 == w { 0 } else { d[i + 1] };
            d[i] = d[i]
                .min(down.saturating_add(1))
                .min(right.saturating_add(1));
        }
    }
    d
}

/// 把一张贴图的 alpha 盒式重采样到 `w × h`，作为掩膜。
///
/// 与 `art::stretch` 同一种采样：卡带边缘的抗锯齿要留下来，否则掩膜边缘是一排硬台阶，
/// 贴图自身的柔边会被它切成锯齿。
fn resample_alpha(alpha: &crate::cart_art::Alpha, w: u32, h: u32) -> Vec<u8> {
    let (sw, sh) = (alpha.w, alpha.h);
    if sw == 0 || sh == 0 || w == 0 || h == 0 {
        return vec![255; (w * h) as usize];
    }
    let (kx, ky) = (sw as f32 / w as f32, sh as f32 / h as f32);
    let mut out = vec![0u8; (w * h) as usize];
    for y in 0..h {
        let y0 = y as f32 * ky;
        let ya = (y0.floor() as u32).min(sh - 1);
        let yb = ((y0 + ky).ceil() as u32).max(ya + 1).min(sh);
        for x in 0..w {
            let x0 = x as f32 * kx;
            let xa = (x0.floor() as u32).min(sw - 1);
            let xb = ((x0 + kx).ceil() as u32).max(xa + 1).min(sw);
            let mut sum = 0u32;
            let mut n = 0u32;
            for sy in ya..yb {
                for sx in xa..xb {
                    sum += alpha.bytes[(sy * sw + sx) as usize] as u32;
                    n += 1;
                }
            }
            out[(y * w + x) as usize] = if n == 0 { 0 } else { (sum / n) as u8 };
        }
    }
    out
}

fn rasterise_svg(svg: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let scale =
        resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    Some(pixmap.data().iter().skip(3).step_by(4).copied().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One shape asked for at two sizes gets two masks, each its own length. The cache used to
    /// be keyed on the shape alone, so the second size silently got the first size's mask — a
    /// face shorter than it claimed to be, and a panic in `recess_label` three calls later,
    /// which is where the cart tests found it. The order the two are asked in must not matter,
    /// so this asks for the larger first and the smaller second.
    #[test]
    fn one_shape_two_sizes_two_masks() {
        let large = silhouette(System::Gba, 480, 270);
        let small = silhouette(System::Gba, 240, 135);
        assert_eq!(large.len(), 480 * 270);
        assert_eq!(small.len(), 240 * 135);
        // The depth and the detail follow the size they were asked for, not the first one.
        assert_eq!(cart_depth(Shape::Gba, 240, 135).len(), 240 * 135);
        assert_eq!(detail_mask(Shape::Gba, 480, 270).len(), 480 * 270);
        // And the smaller one is the smaller shape, not a crop of the larger.
        assert_ne!(small.len(), large.len());
    }
}
