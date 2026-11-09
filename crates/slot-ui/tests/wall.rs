//! 默认壁纸与它那圈呼吸光（2026-10-28 六六）。
//!
//! 三条要钉住的东西：
//! 1. 内置那张图能解、就是屏那么大（不然 `draw_backdrop` 会把一张错的图拉满屏）；
//! 2. 光**只落在网线上、中央还是黑的** —— 货架、LOADING 字、卡带全压在中央那块，
//!    光糊进去就是"背景把字洗了"；
//! 3. 画的次序：底色 → 光（有光时不压纱），光在最上面，且贴图往左上偏回来，
//!    网线和底图重合。

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};
use slot_ui::{
    default_wallpaper, draw_backdrop, wallpaper_glow, WALL_GLOW_PAD,
};

/// 一张贴图的 alpha 剖面：`(x, y, w, h, alpha)`。
fn last_tex(out: &[Draw]) -> Option<(f32, f32, f32, f32, f32)> {
    out.iter().rev().find_map(|d| match *d {
        Draw::Tex {
            x, y, w, h, alpha, ..
        } => Some((x, y, w, h, alpha)),
        _ => None,
    })
}

#[test]
fn the_default_wallpaper_is_the_panel() {
    let (rgba, w, h) = default_wallpaper().expect("默认壁纸解不出来");
    assert_eq!((*w, *h), (OUT_W, OUT_H), "默认壁纸不是屏那么大");
    assert_eq!(rgba.len(), (w * h * 4) as usize);
    // 中央必须留黑：货架、字、卡带全在那儿。
    let mid = (((h / 2) * w + w / 2) * 4) as usize;
    assert_eq!(
        &rgba[mid..mid + 3],
        &[0, 0, 0],
        "默认壁纸的中央不是黑的，界面压上去会看不清"
    );
}

#[test]
fn the_glow_traces_the_frame_and_leaves_the_middle_dark() {
    let (rgba, w, h) = default_wallpaper().unwrap();
    let (glow, gw, gh) = wallpaper_glow(rgba, *w, *h, [0xd8, 0xd4, 0xcc]).expect("烘不出光");

    // 柔光图比底图大一圈，画的时候往左上偏回来 —— 两个尺寸必须是同一个 `PAD`。
    assert_eq!((gw, gh), (w + 2 * WALL_GLOW_PAD, h + 2 * WALL_GLOW_PAD));

    let at = |x: u32, y: u32| -> u8 { glow[((y * gw + x) * 4 + 3) as usize] };
    // 中央（货架区）一个字节都不许亮 —— 实测这块 200×160 里本来就没有一条网线，
    // 所以"有光"只可能是糊进来了。
    let (cx, cy) = (gw / 2, gh / 2);
    let mut centre_max = 0u8;
    for y in cy - 80..cy + 80 {
        for x in cx - 100..cx + 100 {
            centre_max = centre_max.max(at(x, y));
        }
    }
    assert_eq!(centre_max, 0, "光糊进了中央，会洗掉背景上的字");

    // 网线上必须有光，否则这一层等于没画。
    let mut frame_max = 0u8;
    for y in 0..gh / 4 {
        for x in 0..gw / 4 {
            frame_max = frame_max.max(at(x, y));
        }
    }
    assert!(frame_max > 40, "四角网线上没有光（max alpha {frame_max}）");
    // 峰值 0.55 是「线在发光」的观感上限，**不是**压品红的手段（那件事归 `wallpaper_ink`）。
    // 谁再想靠抬高峰值去盖色，这条先红。
    assert!(frame_max <= 141, "光的峰值被抬到 {frame_max}，盖色不该靠峰值");
}

/// 满屏矩形的下标（底色是第一个，纱是第二个 —— 两个都长这样，只能数位置）。
fn full_rects(out: &[Draw]) -> Vec<usize> {
    out.iter()
        .enumerate()
        .filter_map(|(i, d)| match *d {
            Draw::Rect { w, h, .. } if w == OUT_W as f32 && h == OUT_H as f32 => Some(i),
            _ => None,
        })
        .collect()
}

/// 壁纸本体那张贴图的下标（满屏的那个）。
fn wall_tex_at(out: &[Draw]) -> usize {
    out.iter()
        .position(|d| matches!(*d, Draw::Tex { w, h, .. } if w == OUT_W as f32 && h == OUT_H as f32))
        .expect("壁纸本体没画")
}

#[test]
fn the_glow_is_drawn_last_and_lands_on_the_picture() {
    let mut out = Vec::new();
    draw_backdrop(
        Some(TexId::from_raw(0)),
        Some((TexId::from_raw(7), OUT_W + 2 * WALL_GLOW_PAD, OUT_H + 2 * WALL_GLOW_PAD, 0.8)),
        &mut out,
    );

    let (x, y, w, h, alpha) = last_tex(&out).expect("光没画出来");
    assert_eq!((x, y), (-(WALL_GLOW_PAD as f32), -(WALL_GLOW_PAD as f32)));
    assert_eq!(
        (w, h),
        (
            OUT_W as f32 + 2.0 * WALL_GLOW_PAD as f32,
            OUT_H as f32 + 2.0 * WALL_GLOW_PAD as f32
        )
    );
    assert!((alpha - 0.8).abs() < 1e-6, "呼吸值没传下来：{alpha}");

    // 暗态：默认壁纸不压纱 —— 光要是垫在纱下面，霓虹就被洗成灰粉，读不出"在发光"。
    if matches!(slot_ui::palette::mode(), slot_ui::palette::Mode::Dark) {
        let wall = wall_tex_at(&out);
        assert!(
            full_rects(&out).iter().all(|i| *i < wall),
            "默认壁纸在暗态压了纱"
        );
    }
}

#[test]
fn a_cards_own_wallpaper_keeps_its_veil() {
    let mut out = Vec::new();
    draw_backdrop(Some(TexId::from_raw(0)), None, &mut out);
    // 没有光 ⇒ 卡自己的图，照旧压纱（那张图没为这块屏画过）。
    let wall = wall_tex_at(&out);
    assert!(
        full_rects(&out).iter().any(|i| *i > wall),
        "卡自己的壁纸上没有纱"
    );
}
