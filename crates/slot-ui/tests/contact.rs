//! 触点那一下（`draw_contact_flash`）—— ★ 2026-10-27（六六）：「光效 1 按你说的做」。
//!
//! 为什么要它：声音一直是两段（滑轨 + 触点），画面却只在**行程**上做事 —— 光随卡带往下插
//! 变亮，然后被卡带盖住，卡带**贴死**那一帧什么都没有。这条光就是"簧片接上"的那一瞬。
//!
//! 三条判据，每一条都对应一个会真的画错的方式：
//! 1. 货架上（`flash == 0`）**一个像素都不画**；
//! 2. 光只能落在**洞口外沿** —— 洞里落位后 100% 被卡带占着（落位宽 = 开口宽），画在洞里
//!    等于没画；而且**不许盖上洞口**（那是"光从缝里挤出来"，不是"洞被点亮"）；
//! 3. **上沿不发光** —— 10-26 六六否过朝货架发的那一面，和 `draw_slot_walls` 同一条。

use slot_gfx::Draw;
use slot_ui::{draw_contact_flash, SLOT_OPEN_H, SLOT_OPEN_W, SLOT_OPEN_X, SLOT_OPEN_Y};

fn quad(d: &Draw) -> (f32, f32, f32, f32, f32) {
    match *d {
        Draw::Rect { x, y, w, h, colour } => (x, y, w, h, colour[3]),
        other => panic!("the contact flash is bands of flat colour: {other:?}"),
    }
}

const EPS: f32 = 0.01;

#[test]
fn nothing_lights_up_on_the_shelf() {
    let mut out = Vec::new();
    draw_contact_flash(&mut out, 0.0);
    assert!(out.is_empty(), "the shelf lit up: {out:?}");
}

#[test]
fn the_light_sits_outside_the_mouth_and_leaves_the_top_dark() {
    let mut out = Vec::new();
    draw_contact_flash(&mut out, 1.0);
    assert_eq!(out.len(), 6, "two rings of three bands each, no more");
    let (right_edge, bottom_edge) = (SLOT_OPEN_X + SLOT_OPEN_W, SLOT_OPEN_Y + SLOT_OPEN_H);
    for d in &out {
        let (x, y, w, h, _) = quad(d);
        let left = x + w <= SLOT_OPEN_X + EPS;
        let right = x >= right_edge - EPS;
        let below = y >= bottom_edge - EPS;
        assert!(
            left || right || below,
            "a band covers the opening itself: {x},{y} {w}x{h}"
        );
        // 上沿朝货架的一侧不发光：没有任何一条带整条落在洞口上方。
        assert!(
            !(y + h <= SLOT_OPEN_Y + EPS),
            "a band lights the shelf side of the mouth: {x},{y} {w}x{h}"
        );
    }
}

/// 亮起来和收下去是同一条曲线：触点那一帧最亮，之后每一步都不许更亮。
#[test]
fn the_flash_only_ever_fades() {
    let brightest = |f: f32| {
        let mut out = Vec::new();
        draw_contact_flash(&mut out, f);
        out.iter().map(|d| quad(d).4).fold(0.0, f32::max)
    };
    let full = brightest(1.0);
    assert!(full > 0.5, "the touch-down frame is only {full} bright");
    let mut last = full;
    for step in 1..=16 {
        let a = brightest(1.0 - step as f32 / 16.0);
        assert!(a <= last + 1e-6, "the flash rises again at {step}");
        last = a;
    }
    assert_eq!(brightest(0.0), 0.0);
}
