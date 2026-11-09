//! The battery reading: the percent, and the bolt while a cable is in.
//!
//! There used to be a capsule here — a 33 × 16.5 picture of a cell with a fill bar that tracked
//! the percent, and the number printed *under* it. Two objects stacked into a forty-one and a
//! half pixel cluster inside a fifty-eight pixel band, which read as crowded. What is left is the
//! reading itself: the percent at the clock's own size, right-anchored at the band's margin, with
//! the bolt beside it while charging.

use slot_gfx::{Draw, TexId};
use slot_power::{Battery, Charge};

use crate::plate::STATUS_H;
use crate::status::Printed;

/// The bolt's raster size, not its drawn one: `icon_face` cuts the glyph to its ink and the drawn
/// box is `BOLT_W` square, so this only has to be big enough for the glyph to be cut from.
pub const BOLT_PX: f32 = 27.0;

/// The bolt's drawn box, and the gap between it and the number.
pub const BOLT_W: f32 = 21.0;
pub const BOLT_H: f32 = 21.0;
const BOLT_GAP: f32 = 5.0;

/// The cluster is one band of type now: the same one the clock is set in.
///
/// Kept as a function rather than folded into `status`'s own constant so that exactly one place
/// decides how tall the row is, and both ends read it — the two ends of a status band drifting
/// apart is the failure this shape prevents.
pub fn cluster_h() -> f32 {
    STATUS_H as f32
}

/// The percent, right-anchored, with the bolt to its left whenever a cable is in.
///
/// `right` is the band's own margin, and the anchor is the number's **right** edge: the reading
/// grows leftward, so a percent that gains a digit moves nothing else and the margin holds.
///
/// The bolt is placed off the number's own width rather than from a reserved slot. There is no
/// longer a fixed-width capsule to reserve beside, and nothing needs reserving: the bolt appears
/// in the space the number leaves, in the gap, and the row does not move when it does.
///
/// Nothing is drawn until the percent's face has landed. With no capsule, a bolt placed against a
/// zero width would sit out at the margin and then jump left the moment the type arrived.
/// The percent, right-anchored, with the bolt to its left whenever a cable is in.
///
/// `right` is the band's own margin, and the anchor is the number's **right** edge: the reading
/// grows leftward, so a percent that gains a digit moves nothing else and the margin holds.
///
/// `scale` shrinks the whole cluster (number + bolt) to a fraction of its nominal `STATUS_H`
/// size, centred in the band — used for the top-right battery at 60% (2026-10-23 六六).
/// 竖放的电池图标（24×24 的框里画），★ 2026-10-24（六六）：「电量百分比改成竖放的电池
/// 图标，移至名称栏右边 30px 区域，按 24px 居中」。
///
/// 百分比读数整条拿掉：一格格的液面比数字更像"电量"，而且 30px 宽的角标里放三位数字
/// 必然要缩到看不清。外壳用四条 2px 的矩形拼（`Draw::Rect` 没有描边），液面从底部往上长。
pub fn draw_battery_icon(
    x: f32,
    y: f32,
    size: f32,
    battery: Option<Battery>,
    bolt: Option<TexId>,
    out: &mut Vec<Draw>,
) {
    let Some(b) = battery else {
        return;
    };
    let ink = crate::palette::ink_f();
    let k = size / 24.0;
    let mut rect = |rx: f32, ry: f32, rw: f32, rh: f32| {
        out.push(Draw::Rect {
            x: x + rx * k,
            y: y + ry * k,
            w: rw * k,
            h: rh * k,
            colour: ink,
        });
    };
    rect(8.0, 0.0, 8.0, 3.0); // 正极帽
    rect(5.0, 3.0, 14.0, 2.0); // 壳：上盖
    rect(5.0, 22.0, 14.0, 2.0); // 壳：下底
    rect(5.0, 5.0, 2.0, 17.0); // 壳：左壁
    rect(17.0, 5.0, 2.0, 17.0); // 壳：右壁
    // 液面：从底部往上长，占内腔（10×17）的比例。
    let level = (b.percent.min(100) as f32) / 100.0;
    let filled = 17.0 * level;
    if filled > 0.0 {
        rect(7.0, 5.0 + (17.0 - filled), 10.0, filled);
    }
    // 充电中的闪电，压在电池身上（12px）。
    if let (Charge::Charging, Some(tex)) = (b.charge, bolt) {
        let (bw, bh) = (12.0 * k, 12.0 * k);
        out.push(Draw::Tex {
            x: x + (size - bw) / 2.0,
            y: y + (size - bh) / 2.0,
            w: bw,
            h: bh,
            tex,
            alpha: 1.0,
        });
    }
}

pub fn draw_gauge(
    right: f32,
    top: f32,
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    out: &mut Vec<Draw>,
    scale: f32,
) {
    // No gauge is no reading, rather than an empty one: a device with no battery node has nothing
    // to say, and a bare "0%" would be a lie rather than an absence.
    let Some(b) = battery else {
        return;
    };
    if percent.w == 0 {
        return;
    }

    let h = STATUS_H as f32 * scale;
    let w = percent.w as f32 * scale;
    let x = right - w;
    let y = top + (STATUS_H as f32 - h) / 2.0;
    out.push(match percent.face {
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

    if let (Charge::Charging, Some(tex)) = (b.charge, bolt) {
        let bw = BOLT_W * scale;
        let bh = BOLT_H * scale;
        out.push(Draw::Tex {
            x: x - BOLT_GAP * scale - bw,
            y: y + (h - bh) / 2.0,
            w: bw,
            h: bh,
            tex,
            alpha: 1.0,
        });
    }
}
