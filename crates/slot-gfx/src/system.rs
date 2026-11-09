//! One machine's picture geometry: how big a frame its core hands us, and where on the
//! panel that frame lands.
//!
//! This used to be two constants and a single `SCALE`, because there used to be one machine.
//! There is still one machine at a time, but there is more than one machine this panel can
//! hold: the Advance at 240x160 and Game Boy at 160x144 today, and the 256x224 / 320x240
//! frames of the consoles once those cores land. Every one of them wants the same rule, so
//! the rule is written once and the machine fills it in — see [`scale_for`].
//!
//! On this panel (720x480) the rule returns three for both machines that ship today, which is
//! exactly the constant this module used to hardcode: the Advance is 240x160 and 240 lands in
//! 720 exactly three times *and* 160 in 480 exactly three times, and Game Boy is 160x144 and
//! 160 lands four times across in the panel but only three times down it, so the height is
//! what pins it at three. That is why nothing on the panel moves when the constant becomes a
//! calculation — it is the same three, arrived at differently. See the 2026-10-21 measurement
//! for the pixel-for-pixel proof (0 differing pixels, 0.0/255 max delta).
//!
//! Game Boy is short of the panel in both directions, so it does not fill it: it gets a
//! 480x432 window centred in the frame. That window is what a bezel goes around.
//!
//! Game Boy and Game Boy Color get one row between them rather than one each: their frames
//! are the same 160x144, so they want the same texture and the same window, and a second copy
//! would only be a second place for the two to drift apart.

use crate::surface::{OUT_H, OUT_W};

/// The whole multiple of a source pixel that fits the panel, floored at one.
///
/// The Advance and Game Boy both answer three on 720x480 (see the module note); a frame this
/// panel could not hold even once is clamped to one rather than zero, so an undersized or
/// unexpected frame is drawn too large to crop rather than not drawn at all.
///
/// **The one number the geometry hangs on.** The picture rect, the aperture mask, the lattice
/// and the reflection's zone all key off it, so it is computed in one place and read
/// everywhere rather than being a constant each site trusts. `min` of the two axes is what
/// keeps the picture square: a scale that fit the width but overflowed the height would
/// stretch the picture, which is never what "bigger" is allowed to mean here.
pub fn scale_for(src_w: u32, src_h: u32) -> u32 {
    (OUT_W / src_w).min(OUT_H / src_h).max(1)
}

/// 每个源像素在面板上占多少**输出**像素，两轴各一个。`rect` 是该机种的画面矩形（不含
/// 电源收拢），`tex` 是这一帧真实纹理的尺寸。
///
/// **不是缩放常数。** 它从几何除出来，所以两个"帧尺寸会变"的机种自动正确：
///
/// * `Ps1` —— 320×240 的低分辨率游戏落在 640×480 的矩形里，这里是 `(2, 2)`；
///   640×480 的游戏落在同一个矩形里，这里是 `(1, 1)`。用 `System::scale()` 会一律给 1。
/// * `Arcade` —— 384×224 的 CPS1 板子拉伸到 640×480，这里是 `(1.667, 2.143)`，无理数，
///   这正是它唯一该用 pixellate 的地方。
///
/// 格子线宽（`GAME_FRAG` 的 `1 - 1/cell`）与遮罩锚法（[`mask_scale`]）都读它。
pub fn cell_size(rect: (f32, f32, f32, f32), tex_w: u32, tex_h: u32) -> (f32, f32) {
    let (_, _, rw, rh) = rect;
    if tex_w == 0 || tex_h == 0 {
        return (1.0, 1.0);
    }
    (rw / tex_w as f32, rh / tex_h as f32)
}

/// 面板遮罩的平铺次数，即 `texture2D(u_mask, v_uv * scale)` 里的 `scale`（表是 3×3，
/// 一个重复 = 三格）。
///
/// ★★ 2026-10-28：**两轴各自决定锚源还是锚面板**，规则是"这个轴的缩放是不是整数倍"。
///
/// * **整数倍 → 锚源像素**（`scale = tex`）。一个源像素 = 一张表 = 三色相，源像素在屏上
///   占整 2 或整 3 个像素，块内取同一批格子。**与旧版数值完全相同**，所以现有 720 机器
///   上 GBA/GB/GBC/FC/SFC/MD/NEOGEO/PS1 的画面逐像素不变。
/// * **非整数倍 → 锚面板像素**（`scale = rect/3`）。每 3 个屏像素一个周期，格推进率恒为
///   1，拍频归零。源像素在屏上宽窄不一，若仍锚源，格宽就跟着源像素抖。
///
/// 为什么不能一律锚面板：2× 时两者**不等价**。锚源是每源像素取第 {0, 2} 格（跳过 1），
/// 锚面板是 0/1/2 均匀轮转 —— 格子边界从块间挪到了块内，720 机器上所有 2× 主机的画面
/// 都会变。已在 `_tools/_mask_anchor_sim.py` 逐像素枚举证实（9 个场景全符合预期）。
pub fn mask_scale(rect: (f32, f32, f32, f32), tex_w: u32, tex_h: u32) -> (f32, f32) {
    let (cx, cy) = cell_size(rect, tex_w, tex_h);
    let (_, _, rw, rh) = rect;
    // 1e-3：`53/20` 这类有理数在 f32 下算不出精确整数，但"是整数倍"这件事差得很远
    // （2.0 与 2.65 之间没有歧义），所以这个容差只筛浮点噪声，不筛真判断。
    let whole = |c: f32| (c - c.round()).abs() < 1e-3;
    let x = if whole(cx) { tex_w as f32 } else { rw / 3.0 };
    let y = if whole(cy) { tex_h as f32 } else { rh / 3.0 };
    (x, y)
}

/// The GBA's own frame. The pair this module used to be.
pub const GBA_SRC_W: u32 = 240;
pub const GBA_SRC_H: u32 = 160;

/// Game Boy's frame, and Game Boy Color's.
pub const GB_SRC_W: u32 = 160;
pub const GB_SRC_H: u32 = 144;

/// Super Famicom's frame. Lands on the 720x480 panel at integer 2x (512x448).
pub const SFC_SRC_W: u32 = 256;
pub const SFC_SRC_H: u32 = 224;

/// Famicom / NES frame. The *visible* picture is 256x224: the core (fceumm) crops the 8px of
/// overscan top and bottom, which is exactly what `Core::Nes`'s startup options ask for — see
/// `crates/slot/src/core.rs`. Lands on the 720x480 panel at integer 2x (512x448); the 32px the
/// crop leaves becomes symmetric letterbox rather than the overscan junk.
pub const FC_SRC_W: u32 = 256;
pub const FC_SRC_H: u32 = 224;

/// NeoGeo frame as FBNeo actually hands it over: **304x224**, not 320. The core crops 8px of
/// overscan off each side of the real 320x224, so every FBNeo/NeoGeo game is 304 wide. Verified
/// on-device with kof97.zip: `frame N 304x224 pitch 608` from the first frame to the last. It
/// lands on the 720x480 panel at integer 2x (608x448), 16px narrower than MD's 640x448.
pub const NEOGEO_SRC_W: u32 = 304;
pub const NEOGEO_SRC_H: u32 = 224;

/// Other arcade games (CPS1/CPS2/CPS3). The source frame varies per game, so the
/// geometry is not pinned to one size; the picture is stretched to fill a 640x480
/// 4:3 rectangle rather than integer-scaled.
pub const ARCADE_DRAW_W: u32 = 640;
pub const ARCADE_DRAW_H: u32 = 480;

/// ★ 2026-10-25: Mega Drive's frame, **320x224**, which is the same pair NeoGeo uses and for
/// the same reason — `scale_for` gives 2, so the picture lands at 640x448 and 32px of the panel
/// is letterbox. Three would need 960 and 672, and this panel is 720x480.
///
/// ⚠️ **224 is produced, not assumed.** A Mega Drive can also run in a 192-line mode (a lot of
/// PAL titles and a few NTSC ones do), and `genesis_plus_gx` hands over whatever the game is
/// really drawing. Nothing here needs to know: `FrameRef` carries the frame's own `w`/`h`
/// (`slot/src/frames.rs`), and `GamePass::upload` rebuilds the texture when the size changes
/// (`pipeline.rs`). So a 192-line game arrives as a 320x192 frame and is centred in the same
/// window rather than being stretched to 224 — the geometry below is the *nominal* one the
/// screen rect and the bezel are laid out for, and a shorter frame simply sits inside it.
///
/// The one thing `224` does buy is `genesis_plus_gx_overscan = disabled` in
/// `crates/slot/src/core.rs`: without that the core pads top and bottom and every frame is 240
/// rows, which would letterbox differently from what this rect says.
pub const MD_SRC_W: u32 = 320;
pub const MD_SRC_H: u32 = 224;

/// ★ 2026-10-25: PlayStation's frame, **640x480**. This is the one machine whose source size is
/// the output size: `scale_for(640, 480)` is `min(720/640, 480/480) = 1`, so the picture lands
/// 1:1 across the full width and full height of the panel with no letterbox and no integer
/// blow-up. A 1x picture on a 3.5" 720x480 panel is exactly the panel's own pixel grid, which is
/// the best this hardware can do for a 3D console anyway — 2x would need 1280x960.
///
/// ⚠️ **Not every PS1 mode is 640x480.** `pcsx_rearmed` sends 320x240 for the many games that
/// render in 15-bit low-res, and 512x240 / 640x480 for the hi-res ones; `FrameRef` carries the
/// real size, so those arrive as their own frames and are centred rather than scaled up to
/// this rect. Same arrangement as Mega Drive: the constants are the layout's frame of
/// reference, not a promise about what any one game sends.
pub const PS1_SRC_W: u32 = 640;
pub const PS1_SRC_H: u32 = 480;

/// PC Engine / TurboGrafx-16 frame. 256x224, the same numbers as the Super Famicom, so it
/// lands on the 720x480 panel at integer 2x (512x448) and reads as one family of picture
/// with FC and SFC.
///
/// `pce_fast` hands over **240** rows out of the box (`pce_fast_initial_scanline` defaults
/// to 3, `pce_fast_last_scanline` to 242 — both measured out of the `.so`), which is the
/// raster with its overscan still on. `Core::PceFast`'s startup options crop 8 off each end
/// for exactly the reason `Core::Nes`'s do: the visible picture is 224 tall, and a frame
/// that is 240 tall against a geometry that says 224 is a picture scaled by two different
/// numbers at once.
pub const PCE_SRC_W: u32 = 256;
pub const PCE_SRC_H: u32 = 224;

/// Which machine's picture is on the panel.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum System {
    Gba,
    /// Game Boy *and* Game Boy Color: one row, because one frame size.
    Gb,
    /// Super Famicom / SNES: one row of its own frame size.
    Sfc,
    /// Famicom / NES: one row of its own frame size.
    Fc,
    /// NeoGeo: 320x224, integer 2x on the panel (640x448).
    Neogeo,
    /// Other arcade (CPS1/CPS2/CPS3): stretched to a 640x480 4:3 rect, not integer-scaled.
    Arcade,
    /// Mega Drive / Genesis: 320x224, integer 2x on the panel (640x448) — the same numbers as
    /// NeoGeo, which is why the two read as one family of picture on this screen.
    Md,
    /// PlayStation: 640x480, so `scale_for` gives 1 and the picture is the panel's own size.
    Ps1,
    /// PC Engine / TurboGrafx-16: 256x224, integer 2x on the panel (512x448).
    Pce,
}

impl System {
    /// The frame the core hands us, in pixels. This is also how many times the mask tiles
    /// across the window: one RGB triad per source pixel, exactly — which is why a
    /// different source size needs no change to the mask's own table, only to its tiling
    /// count (the shader reads `u_src` for both).
    pub fn src_w(self) -> u32 {
        match self {
            System::Gba => GBA_SRC_W,
            System::Gb => GB_SRC_W,
            System::Sfc => SFC_SRC_W,
            System::Fc => FC_SRC_W,
            System::Neogeo => NEOGEO_SRC_W,
            System::Arcade => ARCADE_DRAW_W,
            System::Md => MD_SRC_W,
            System::Ps1 => PS1_SRC_W,
            System::Pce => PCE_SRC_W,
        }
    }

    pub fn src_h(self) -> u32 {
        match self {
            System::Gba => GBA_SRC_H,
            System::Gb => GB_SRC_H,
            System::Sfc => SFC_SRC_H,
            System::Fc => FC_SRC_H,
            System::Neogeo => NEOGEO_SRC_H,
            System::Arcade => ARCADE_DRAW_H,
            System::Md => MD_SRC_H,
            System::Ps1 => PS1_SRC_H,
            System::Pce => PCE_SRC_H,
        }
    }

    /// This machine's scale on the panel, straight off [`scale_for`]. `Arcade` is not
    /// integer-scaled at all, so the answer it gives here is nominal and nothing reads it —
    /// `screen_base` special-cases it below.
    pub fn scale(self) -> u32 {
        scale_for(self.src_w(), self.src_h())
    }

    /// Whether this machine stretches to fill its rect rather than scaling by a whole
    /// multiple. Only the arcade games do: their source frames vary per game, and a
    /// fractional scale on a varying source is exactly what integer scale refuses.
    pub fn stretches(self) -> bool {
        matches!(self, System::Arcade)
    }

    /// The rect the picture fills with the screen fully up, in offscreen pixels.
    ///
    /// Centred, which is all it takes: the GBA's own window comes out as the whole frame
    /// (240 lands in 720 three times and 160 in 480 three times, so the margins are zero),
    /// and Game Boy's comes out as 480x432 sat in the middle. One formula, and the machine
    /// that fills the panel is the case that falls out of it rather than a special case.
    ///
    /// **`Arcade` is the one exception**, and it is a different rule rather than a fudge: its
    /// source frame varies per game (CPS1/CPS2 are 384x224, some boards 256x224, and so on),
    /// so an integer multiple would give a different picture size for every game and jump as
    /// the shelf moves between them. It gets a fixed 640x480 rectangle instead — 4:3, the
    /// shape the original cabinets' monitors were — and the picture is stretched to fill it.
    pub fn screen_base(self) -> (f32, f32, f32, f32) {
        if self == System::Arcade {
            let w = ARCADE_DRAW_W as f32;
            let h = ARCADE_DRAW_H as f32;
            return ((OUT_W as f32 - w) / 2.0, (OUT_H as f32 - h) / 2.0, w, h);
        }
        let s = self.scale() as f32;
        let w = self.src_w() as f32 * s;
        let h = self.src_h() as f32 * s;
        ((OUT_W as f32 - w) / 2.0, (OUT_H as f32 - h) / 2.0, w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of turning the constant into a calculation: on this panel it must still
    /// answer the three it used to be hardcoded to, for both machines that ship. If this ever
    /// changes, every pixel of the game layer on RG SP / RG34XX / RG34XXSP changes with it.
    #[test]
    fn the_panel_still_answers_three_for_both_shipping_machines() {
        assert_eq!(scale_for(GBA_SRC_W, GBA_SRC_H), 3);
        assert_eq!(scale_for(GB_SRC_W, GB_SRC_H), 3);
    }

    /// Game Boy is pinned by its height — it has room for four across but only three down, and
    /// the smaller of the two is what keeps the picture square rather than stretched.
    #[test]
    fn the_short_axis_is_what_pins_the_scale() {
        // 160 fits four times across 720, but 144 only fits three times down 480.
        assert_eq!(OUT_W / GB_SRC_W, 4);
        assert_eq!(OUT_H / GB_SRC_H, 3);
        assert_eq!(scale_for(GB_SRC_W, GB_SRC_H), 3);
    }

    /// A frame this panel cannot hold even once scales to one rather than zero, so it is drawn
    /// too large to crop rather than not drawn at all.
    #[test]
    fn a_frame_larger_than_the_panel_is_floored_at_one() {
        assert_eq!(scale_for(1024, 768), 1);
    }

    #[test]
    fn each_machines_rect_is_its_own_frame_times_the_scale() {
        let (gx, gy, gw, gh) = System::Gba.screen_base();
        assert_eq!((gx, gy), (0.0, 0.0));
        assert_eq!((gw, gh), (720.0, 480.0));
        let (_, _, w, h) = System::Gb.screen_base();
        assert_eq!((w, h), (480.0, 432.0));
    }

    /// The two consoles land at integer 2x on this panel — that is the whole reason they are
    /// the first new machines after the handhelds: 256 fits 720 and 480 two or three times
    /// over, so the picture is sharp and centred with no fractional scale.
    #[test]
    fn the_consoles_also_answer_integer_two() {
        assert_eq!(scale_for(SFC_SRC_W, SFC_SRC_H), 2);
        assert_eq!(scale_for(FC_SRC_W, FC_SRC_H), 2);
        let (_, _, w, h) = System::Sfc.screen_base();
        assert_eq!((w, h), (512.0, 448.0));
        let (_, _, w2, h2) = System::Fc.screen_base();
        assert_eq!((w2, h2), (512.0, 448.0));
    }

    /// ★ 2026-10-25: the two machines added with MD and PS1, and the two are deliberately
    /// *different* shapes of answer on this panel:
    ///
    /// * **Mega Drive is 320x224**, integer 2x, 640x448 on a 720x480 panel, letterboxed 16px
    ///   top and bottom. *Not* NeoGeo's geometry: FBNeo crops the NeoGeo frame to 304x224 (8px of
    ///   overscan off each side), so `NEOGEO_SRC_W` is 304 while `MD_SRC_W` stays 320 — two
    ///   224-line 4:3-ish pictures that simply are not the same width.
    /// * **PlayStation is the panel's height, not its width** — a 640x480 source scales by 1
    ///   (`min(720/640, 480/480)` = `min(1, 1)`), so the picture rect is **640x480, centred with
    ///   40 px of letterbox either side and none top or bottom**. It is the one machine whose
    ///   integer scale *is* 1 — worth pinning because every other machine here is 2x, so a
    ///   `scale_for` that ever started returning 0 for "does not fit" would silently push this
    ///   machine's picture to 0x0 while leaving the rest untouched.
    #[test]
    fn md_and_ps1_geometry_on_this_panel() {
        assert_eq!((MD_SRC_W, MD_SRC_H), (320, 224));
        assert_eq!((NEOGEO_SRC_W, NEOGEO_SRC_H), (304, 224));
        let (_, _, nw, nh) = System::Neogeo.screen_base();
        assert_eq!((nw, nh), (608.0, 448.0));
        assert_eq!(scale_for(MD_SRC_W, MD_SRC_H), 2);
        let (_, _, mw, mh) = System::Md.screen_base();
        assert_eq!((mw, mh), (640.0, 448.0));

        // 640 across 720 leaves 80 px to share, and 480 down 480 leaves none: x = 40, y = 0.
        // The short axis is one, not zero — that is the whole of what makes this machine work.
        assert_eq!(scale_for(PS1_SRC_W, PS1_SRC_H), 1);
        let (px, py, pw, ph) = System::Ps1.screen_base();
        assert_eq!((px, py), (40.0, 0.0));
        assert_eq!((pw, ph), (640.0, 480.0));
    }

    /// Neither new machine stretches: MD is integer-scaled like the consoles, PS1 happens to
    /// land 1:1. `stretches` stays `Arcade`'s alone — the one whose source frame genuinely
    /// varies per game.
    #[test]
    fn only_arcade_stretches() {
        for s in [System::Gba, System::Gb, System::Sfc, System::Fc, System::Neogeo,
                  System::Md, System::Ps1] {
            assert!(!s.stretches(), "{s:?} must not stretch");
        }
        assert!(System::Arcade.stretches());
    }

    /// ★ 2026-10-28：遮罩锚法**条件化** —— 整数倍锚源像素（与旧版逐像素相同），
    /// 非整数倍锚面板像素（消拍频）。
    ///
    /// 这个测试就是零回归的门禁：720 面板上**除 Arcade 外每一条**都必须落在"锚源"
    /// 那一支，且 `scale` 与旧版 `tex` 逐字相等。
    #[test]
    fn a_whole_multiple_anchors_the_mask_to_source_pixels() {
        // (机种, 在 720x480 上的画面矩形, 这一帧真实纹理尺寸)
        let cases = [
            (System::Gba, (240, 160)),
            (System::Gb, (160, 144)),
            (System::Sfc, (256, 224)),
            (System::Fc, (256, 224)),
            (System::Neogeo, (304, 224)),
            (System::Md, (320, 224)),
            (System::Ps1, (640, 480)),
            // PS1 的低分辨率帧：矩形仍是 640x480，纹理是 320x240 ⇒ cell 2x
            (System::Ps1, (320, 240)),
        ];
        for (system, (tw, th)) in cases {
            let rect = system.screen_base();
            let g = mask_scale(rect, tw, th);
            assert_eq!(
                g,
                (tw as f32, th as f32),
                "{system:?} at {tw}x{th} must keep the source-anchored mask"
            );
        }
    }

    /// 非整数倍必须改锚面板：`rect/3`，每 3 个屏像素一个周期。
    #[test]
    fn a_fractional_multiple_anchors_the_mask_to_the_panel() {
        // GBA 在 640x480 面板上实测的矩形：636x424 = 240x160 × 53/20
        let rect = (2.0, 28.0, 636.0, 424.0);
        let g = mask_scale(rect, 240, 160);
        assert_eq!(g, (212.0, 424.0 / 3.0));
        // 212 = 636/3 ⇒ 每屏像素推进 3*212/636 = 1.0 格，与源宽 240 无关
        assert_eq!(g.0, 636.0 / 3.0);

        // Arcade 在 720 面板上也是非整数（384x224 拉伸到 640x480），同样锚面板
        let rect = System::Arcade.screen_base();
        let g = mask_scale(rect, 384, 224);
        assert_eq!(g, (640.0 / 3.0, 480.0 / 3.0));
    }

    /// ★★ **逐像素门禁**：为什么 `mask_scale` 的条件不能去掉。
    ///
    /// 把条件删掉、一律返回 `rect/3`，在 **3× 下逐像素毫无差别**（`rect/3 == tex`），
    /// 所以两台掌机（GBA/GB/GBC）一点看不出来 —— 但 **2× 下格子边界会从块间挪进块内**，
    /// 720 机器上 FC/SFC/MD/NEOGEO/PS1 的画面全都跟着变。这个改动会一路绿着通过
    /// 所有"3× 没变"的检查，只有逐像素比格编号序列能拦住它。
    ///
    /// 格编号 = `floor(fract(u * scale) * 3)`，`u` 是屏像素中心的归一化位置。
    #[test]
    fn source_and_panel_anchoring_agree_at_three_times_and_differ_at_two() {
        let cells = |rw: f32, scale: f32| -> Vec<i32> {
            (0..rw as i32)
                .map(|ox| {
                    let u = (ox as f32 + 0.5) / rw * scale;
                    let f = u - u.floor();
                    ((f * 3.0).floor() as i32).rem_euclid(3)
                })
                .collect()
        };

        // 3×：`rect/3` 与 `tex` 是同一条序列 ⇒ "一律锚面板"在 720 掌机上察觉不到。
        assert_eq!(cells(720.0, 240.0), cells(720.0, 720.0 / 3.0));

        // 2×：不同。锚源是 0,2,0,2…（每源像素跳过中间那格），锚面板是 0,1,2,0,1,2…
        let src = cells(640.0, 320.0);
        let pnl = cells(640.0, 640.0 / 3.0);
        assert_ne!(src, pnl, "2x 下一律锚面板会改变 720 机器上主机的画面");
        assert_eq!(&src[..4], &[0, 2, 0, 2], "锚源：每源像素取 {{0,2}}");
        assert_eq!(&pnl[..4], &[0, 1, 2, 0], "锚面板：按屏像素均匀轮转");
    }

    /// `cell_size` 是几何量，不是缩放常数：两个帧尺寸会变的机种必须给出不同的值。
    #[test]
    fn cell_size_reads_the_frame_not_the_machine() {
        // PS1 同一个矩形，两种真实帧：hi-res 1:1，lo-res 2x。`System::scale()` 都会给 1，
        // 所以这里不能用它。
        let rect = System::Ps1.screen_base();
        assert_eq!(cell_size(rect, 640, 480), (1.0, 1.0));
        assert_eq!(cell_size(rect, 320, 240), (2.0, 2.0));

        // Arcade：384x224 拉伸到 640x480，两轴不同 ⇒ 这才是真正无理的那个
        let rect = System::Arcade.screen_base();
        let (cx, cy) = cell_size(rect, 384, 224);
        assert!((cx - 640.0 / 384.0).abs() < 1e-6);
        assert!((cy - 480.0 / 224.0).abs() < 1e-6);
        assert!((cx - cy).abs() > 0.4, "arcade stretches unevenly on the two axes");
    }
}
