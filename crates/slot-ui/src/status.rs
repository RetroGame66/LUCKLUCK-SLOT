use slot_gfx::{Draw, TexId, OUT_H};
use slot_power::Battery;

use crate::battery::draw_battery_icon;
use crate::plate::{plate_battery_x, plate_mark_y, STATUS_H, PLATE_MARK};
use crate::slot_chrome::MOUTH_H;

/// The bottom band's top edge. The clock still prints on the **bottom** band (2026-10-23 六六：
/// 移进黑条内、不再贴最左边框), while the battery moved to the top-right corner.
const BAND_Y: f32 = OUT_H as f32 - MOUTH_H;

/// Where the clock stands in the bottom band: its own top edge, centred in the band's height.
const STATUS_Y: f32 = BAND_Y + (MOUTH_H - STATUS_H as f32) / 2.0;

/// ★ 2026-10-24（六六）：「时间移至**左下角区域居中**」——
/// 底条的左段是「左边框到插卡位洞的左沿」这一块（0..`slot_chrome::SLOT_X0`），时钟在
/// 这块里水平居中，而不是贴着一个固定 x 左对齐。
///
/// ★★ 2026-10-29（640 树）：右界跟着洞的左沿走：720 上 180，本树 **140**。
/// 这一段窄了 40px，时钟仍然居中，只是可用的居中空间跟着少了一点。
const CLOCK_ZONE: (f32, f32) = (0.0, 140.0);

/// The top band's top edge, and the right margin the battery anchors to — the battery reads in
/// the top-right corner, shrunk to 60% (2026-10-23 六六).
// ★ 2026-10-24（六六）：电量从"右上角百分比"改成"名称栏右段 30px 里的竖电池图标"，
//   所以这两个锚点退役（位置改由 `plate::plate_battery_x/plate_mark_y` 给）。

/// A line of type and the width it rasterised to. The width cannot be recovered from a
/// `TexId`, and only the compositor can mint one, so the space is held from the width alone
/// while the face is still on its way.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Printed {
    pub face: Option<TexId>,
    pub w: u32,
}

impl Printed {
    pub fn new(face: TexId, w: u32) -> Self {
        Printed {
            face: Some(face),
            w,
        }
    }
}

/// The status line, reprinted after the 2026-10-23（六六）rework: the clock stays on the
/// **bottom** band but moves in off the left frame (`CLOCK_X`), and the battery moved to the
/// **top**-right corner at 60% (`TOP_Y` / `TOP_MARGIN`). The theme dark/light badge and the
/// machine badge are both gone.
pub fn draw_status(
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    clock: Printed,
    _mode: Option<TexId>,
    out: &mut Vec<Draw>,
) {
    // ★ 2026-10-23（六六）：时钟留在底条黑条内（CLOCK_X，解决「偏左」）；电量挪到顶条右上角并
    //   缩到 60%；主题暗/亮指示图标（_mode）与机种指示均移除。
    // 时钟在左段居中：宽度从面本身来，所以居中不会随字号/位数漂。
    let clock_w = if clock.w == 0 { 0.0 } else { clock.w as f32 };
    let clock_x = (CLOCK_ZONE.0 + CLOCK_ZONE.1) / 2.0 - clock_w / 2.0;
    draw_printed(clock_x, STATUS_Y, clock, STATUS_H, out);
    // 电量：名称栏右段（30px 区）里 24px 的竖电池图标，不印数字。
    let _ = percent;
    draw_battery_icon(
        plate_battery_x(),
        plate_mark_y(),
        PLATE_MARK,
        battery,
        bolt,
        out,
    );
}

/// A line of type at an arbitrary `y`. The placeholder is what holds the space while the
/// face is still on its way, so a row does not reflow the moment type arrives.
///
/// `h` is stated rather than assumed: `Draw::Tex` scales the face to the quad it is given, so a
/// caller that draws a `STATUS_H`-tall face into a `HINT_H` quad squashes it instead of leaving
/// it alone. There are two such callers now — the status row and the polaroids plate — and they
/// agree because both pass the same number, not because they both happen to use a default.
pub(crate) fn draw_printed(x: f32, y: f32, p: Printed, h: u32, out: &mut Vec<Draw>) {
    if p.w == 0 {
        return;
    }
    let (w, h) = (p.w as f32, h as f32);
    out.push(match p.face {
        Some(tex) => Draw::Tex {
            x,
            y,
            w,
            h,
            tex,
            alpha: 1.0,
        },
        None => Draw::Rect {
            x,
            y,
            w,
            h,
            colour: [1.0, 1.0, 1.0, 0.08],
        },
    });
}
