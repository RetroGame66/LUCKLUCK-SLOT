//! 投币（ARCADE / NEOGEO）—— ★ 2026-10-27（六六）
//!
//! 六六定稿：「投币（动画同插光盘，不过**全部出屏**）后街机贴图**从插卡口升起**（洋红区发光，
//! 随主题色板）直到底部贴着屏幕底部即可，随后游戏加载完毕的时候 CRT 特效进游戏画面」；
//! 之后补了三条：**选中先往上提一下**、**旋转着加速下坠**、**箱体升起时屏幕震动**。
//!
//! 这份测试钉四件几何/时间：
//! 1. 代币**从货架位**出发，**先上提 `COIN_LIFT_PX`**，再一路加速到**整枚出屏**；
//! 2. 提起来之后**只往下**（没有回弹），转角**只增不减**且越转越快；
//! 3. 街机箱体升起前整块躲在屏下（顶边 = 480），升到位时**底边贴屏底**、左右居中；
//! 4. 箱体在代币走完之后才升，总长 = 提起 + 下落 + 升起。

use slot_gfx::{Draw, TexId};
use slot_ui::{
    coin_angle, coin_cy, coin_fall_s, coin_in_s, coin_total_s, draw_cabinet, CABINET_RISE_S,
    COIN_DIA, COIN_LIFT_PX, COIN_LIFT_S, OUT_H, OUT_W,
};

fn quad(d: &Draw) -> (f32, f32, f32, f32) {
    match *d {
        Draw::Tex { x, y, w, h, .. } | Draw::Rect { x, y, w, h, .. } => (x, y, w, h),
        _ => panic!("the cabinet is not a quad: {d:?}"),
    }
}

/// 起点在货架位；先**上提** `COIN_LIFT_PX`；终点整枚出屏。
#[test]
fn the_coin_lifts_first_and_then_leaves_the_panel() {
    let start = coin_cy(0.0);
    assert!(
        (start - OUT_H as f32 / 2.0).abs() < 0.5,
        "the coin starts at {start}, not on the shelf's centre line"
    );
    let top = coin_cy(COIN_LIFT_S);
    assert!(
        (top - (start - COIN_LIFT_PX)).abs() < 0.01,
        "the lift is {} px against a promised {COIN_LIFT_PX}",
        start - top
    );
    let end = coin_cy(coin_in_s());
    assert!((end - 640.0).abs() < 0.01, "the coin stops at {end}");
    let edge = end - COIN_DIA / 2.0;
    assert!(
        edge > OUT_H as f32,
        "the coin's top edge is still at {edge}, on a {OUT_H} panel"
    );
}

/// 提起来之后一路向下，不许回弹；而且越掉越快（加速，没有匀速段）。
#[test]
fn the_coin_falls_faster_and_faster() {
    let mut last = coin_cy(COIN_LIFT_S);
    // 先确认抬头那段确实是往上（从抬到位往回走，y 应该一路变大回到货架位）。
    let mut t = COIN_LIFT_S;
    let mut prev = coin_cy(COIN_LIFT_S);
    while t > 0.0 {
        t -= 0.02;
        let y = coin_cy(t.max(0.0));
        assert!(y >= prev - 0.001, "the lift dips at {t}: {y} < {prev}");
        prev = y;
    }
    assert!((prev - coin_cy(0.0)).abs() < 0.01, "the lift does not land on the shelf");
    // 再确认下落段单调、且每 0.1 s 走的距离越来越长。
    let mut step_prev = 0.0;
    t = COIN_LIFT_S;
    while t < coin_in_s() - 0.1 {
        let a = coin_cy(t);
        let b = coin_cy(t + 0.1);
        let step = b - a;
        assert!(step > 0.0, "the coin goes back up at {t}");
        assert!(
            step > step_prev,
            "the fall is not accelerating at {t}: {step} then {step_prev}"
        );
        step_prev = step;
        assert!(b >= last, "the coin buckles at {t}");
        last = b;
        t += 0.1;
    }
}

/// 侧着转起来：抬头不转，撒手之后转角只增、每 0.1 s 转得更多。
#[test]
fn the_coin_spins_up_as_it_falls() {
    assert_eq!(coin_angle(0.0), 0.0, "the coin spins before it is let go");
    assert_eq!(coin_angle(COIN_LIFT_S), 0.0);
    let mut prev = 0.0;
    let mut prev_step = 0.0;
    let mut t = COIN_LIFT_S;
    while t <= coin_in_s() {
        let a = coin_angle(t);
        assert!(a >= prev - 0.001, "the coin's spin reverses at {t}");
        let step = a - prev;
        assert!(
            step >= prev_step - 0.001,
            "the spin is not speeding up at {t}: {step} < {prev_step}"
        );
        prev = a;
        prev_step = step;
        t += 0.1;
    }
    assert!(prev > std::f32::consts::TAU, "less than one turn in the whole fall");
}

/// 箱体：升起前整块在屏下，升到位时底边贴屏底、左右居中、尺寸不变。
#[test]
fn the_cabinet_rises_out_of_the_slot_and_stops_on_the_bottom_edge() {
    for (w, h) in [(300u32, 440u32), (312u32, 437u32)] {
        let face = (TexId::from_raw(1), w, h);

        let mut down = Vec::new();
        draw_cabinet(&mut down, &face, None, 0.0, 0.0);
        let (_, y0, _, _) = quad(down.last().expect("nothing drawn at rise 0"));
        assert!(
            (y0 - OUT_H as f32).abs() < 0.01,
            "at rise 0 the cabinet's top is at {y0}, so {} px of it would show",
            OUT_H as f32 - y0
        );

        let mut up = Vec::new();
        draw_cabinet(&mut up, &face, None, 0.0, 1.0);
        let (x1, y1, w1, h1) = quad(up.last().expect("nothing drawn at rise 1"));
        assert!(
            (y1 + h1 - OUT_H as f32).abs() < 0.01,
            "the cabinet's feet are at {}, not on the {OUT_H} bottom edge",
            y1 + h1
        );
        assert!(
            (x1 + w1 / 2.0 - OUT_W as f32 / 2.0).abs() < 0.01,
            "the cabinet is not centred"
        );
        assert!((h1 - h as f32).abs() < 0.01, "the cabinet was scaled to {h1}");
    }
}

/// 箱体在代币**走完之后**才开始升，整段长度对得上。
#[test]
fn the_cabinet_waits_for_the_coin_to_leave() {
    assert!((coin_in_s() - (COIN_LIFT_S + coin_fall_s())).abs() < 0.001);
    assert!((coin_total_s() - (coin_in_s() + CABINET_RISE_S)).abs() < 0.001);
    assert!(coin_total_s() > coin_in_s(), "the cabinet would rise through the coin");
}
