//! The insert: a lift, a fall, and where each machine's cart stops.
//!
//! ★★ 2026-10-24（六六）重新定了这件事，所以这份测试也跟着重写：
//!
//! * **卡座不再由 `SlotChrome` 画。** 插卡位现在是货架框桌面图自己挖出来的 360×44 洞口
//!   （`desktop::desktop_rgba`），由 `App::draw` 盖在卡带之上。旧的那条 254 宽程序化底条
//!   连同 `SEATED_Y` / `GB_INSET` / "卡在唇上"的 `catch_at` 一起解绑 —— `the_chrome_draws_
//!   no_slot_furniture` 钉这一条。
//! * **动画只有两段**：向上拉一小段（`PULL_UP`），然后撒手加速往下插（位移 `u²`）。
//! * **落位是「自身高度的百分之几在开口顶沿之下」**：GBA 1.00（上缘与插卡位平齐）、
//!   GB/GBC 0.50、FC/SFC 0.50（★ 2026-10-24 20:0x 六六把 40/20 一起改成 50）。
//!   只有这样 1.00 才正好等于"上缘齐平"。

use slot_store::{Cart, System};
use slot_ui::{
    draw_edge_glow, draw_empty_slot, edge, housing, icon_box, label_colour, label_text, opening,
    recess, size_for, Draw, Seat, Shelf, SlotChrome, ALERT_PX, CART_H, CART_W, MOUTH_H, OUT_H,
    OUT_W, PULL_SHARE, PULL_UP, SLOT_OPEN_W, SLOT_OPEN_Y, TOP_BAND_H,
};

/// The rom's extension is what decides the machine (`slot_store::scan::machine_of_path`), so
/// the shelf test's cart is built the way a card's would be.
fn cart() -> Cart {
    let stem = "Emerald";
    Cart {
        initial: slot_store::initial(&slot_store::clean_label(stem)),
        stem: stem.into(),
        rom: format!("Games/{stem}.gba").into(),
        label: None,
        art: None,
        code: String::new(),
        cgb: 0,
        title: "POKEMON EMER".into(),
    }
}

/// The chrome as `App::chrome` builds it, for one machine and one point in the travel.
/// `dim` stays at zero so the veil is not in the list: every quad found below is then the
/// cart, which is what these tests measure.
fn chrome_draw(system: System, seat: f32, screen: f32) -> Vec<Draw> {
    let mut out = Vec::new();
    SlotChrome {
        size: size_for(system),
        seat_kind: Seat::from(system),
        ink: label_colour(&label_text(&cart())),
        face: None,
        seat,
        alert: None,
        dim: 0.0,
        screen,
        game: screen > 0.0,
    }
    .draw(&mut out);
    out
}

fn chrome_of(system: System, seat: f32) -> Vec<Draw> {
    chrome_draw(system, seat, 0.0)
}

fn chrome(seat: f32) -> Vec<Draw> {
    chrome_of(System::Gba, seat)
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct Quad {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn quad(d: &Draw) -> Quad {
    match *d {
        Draw::Rect { x, y, w, h, .. }
        | Draw::Tex { x, y, w, h, .. }
        | Draw::Turned { x, y, w, h, .. }
        | Draw::Yaw { x, y, w, h, .. }
        | Draw::Glyph { x, y, w, h, .. }
        | Draw::Foil { x, y, w, h, .. } => Quad { x, y, w, h },
        // The pass owns its own rect. Fully on is what a list carrying one is asking for.
        Draw::Game | Draw::Glow | Draw::Shot { .. } => Quad {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
        },
    }
}

fn alpha(d: &Draw) -> f32 {
    match *d {
        Draw::Rect { colour, .. } => colour[3],
        Draw::Tex { alpha, .. }
        | Draw::Turned { alpha, .. }
        | Draw::Yaw { alpha, .. }
        | Draw::Glyph { alpha, .. }
        | Draw::Foil { alpha, .. } => alpha,
        Draw::Game | Draw::Glow | Draw::Shot { .. } => 1.0,
    }
}

fn opaque(d: &Draw) -> bool {
    alpha(d) >= 1.0
}

/// The cart is the **biggest** quad in the list that is not the full screen: `dim` is zero in
/// these tests, the slot's furniture is not drawn any more, and a face-less cart is a quad in
/// its label's ink.
///
/// ★★ 2026-10-27（六六：「插卡和拔卡时也加上光效」）：**插卡位溢光也在这张表里** ——
/// 底壁 360×2.5、两侧各 12 条 2×44，全都比屏窄。所以不能再拿"第一个比屏窄的 quad"当卡带
/// （那会拿到一条 2.5 px 的光）。按面积取最大：卡带 360×200 上下，光带最大 900 px²。
fn cart_at(out: &[Draw]) -> usize {
    out.iter()
        .enumerate()
        .filter(|(_, d)| quad(d).w < OUT_W as f32 - 0.5)
        .max_by(|a, b| {
            let (qa, qb) = (quad(a.1), quad(b.1));
            let area = |q: Quad| q.w * q.h;
            area(qa).partial_cmp(&area(qb)).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .expect("no cart sized quad in the list")
}

fn cart_quad(out: &[Draw]) -> Quad {
    quad(&out[cart_at(out)])
}

fn at(system: System, seat: f32) -> Quad {
    cart_quad(&chrome_of(system, seat))
}

fn tinted(d: &Draw, c: [f32; 4]) -> bool {
    match *d {
        Draw::Rect { colour, .. } => (0..3).all(|i| (colour[i] - c[i]).abs() < 0.001),
        _ => false,
    }
}

fn is_mouth(d: &Draw) -> bool {
    tinted(d, opening())
}

fn is_lip(d: &Draw) -> bool {
    tinted(d, edge())
}

fn is_game_layer(d: &Draw) -> bool {
    matches!(d, Draw::Game)
}

// ── The travel ──────────────────────────────────────────────────────────────────────────────

/// ★ 按下 A 的那一帧不许跳。起点必须是货架画它的那个矩形 —— 同一个 `foot_y`、同一个
/// `size.middle`，一个像素都不差。
#[test]
fn the_cart_starts_exactly_where_the_shelf_left_it() {
    let mut shelf = Vec::new();
    Shelf::new(vec![cart()]).draw_row(None, 0.0, 0.0, 1.0, &mut shelf);
    let on_shelf = cart_quad(&shelf);

    let start = at(System::Gba, 0.0);
    assert!(
        (on_shelf.x - start.x).abs() < 0.01 && (on_shelf.y - start.y).abs() < 0.01,
        "cart jumps from {on_shelf:?} to {start:?} on insert"
    );
    assert!(
        (on_shelf.w - start.w).abs() < 0.01 && (on_shelf.h - start.h).abs() < 0.01,
        "cart changes size on insert: {on_shelf:?} -> {start:?}"
    );
}

/// 第一段：**向上拉一小段**。
#[test]
fn the_cart_lifts_before_it_goes_in() {
    let rest = at(System::Gba, 0.0);
    let top = at(System::Gba, PULL_SHARE);
    assert!(
        (top.y - (rest.y - PULL_UP)).abs() < 0.01,
        "the lift is {} px against a promised {PULL_UP}",
        rest.y - top.y
    );
    // And it is a lift all the way up, not a dip through the shelf first.
    let mut last = rest.y;
    let mut t = 0.0;
    while t < PULL_SHARE {
        t += 0.02;
        let y = at(System::Gba, t).y;
        assert!(y <= last + 0.001, "the cart dips at the start of the lift: {y} > {last}");
        last = y;
    }
}

/// ★★ 2026-10-24 20:0x（六六）：「**GB/GBC 的放大后插入看起来不太流畅，像是突然变大了**」。
///
/// 量出来的根因是放大锚在上缘：竖版卡带要从 300 长到 414 高，而 `y` 只按抬头走 40px，
/// 于是**下缘 = y + h 在抬头那一段里往下窜 74px**，一路冲出 480 的屏幕底 —— 屏幕上看到的
/// 就是"卡带突然长大并且糊下去"。锚到**中心**之后，那条 74px 变成 17px。
///
/// ★★ 2026-10-27：这条测试原来断言"下缘每帧都不许下移"，在**中心锚**下是过严的 ——
/// 绕中心等比长高时，下缘必然跟着长：`bottom(e) = 390 + 17e`（e 是抬头的缓动 0→1，
/// 高的一侧长 `57e`、抬头抬 `40e`）。实测第一帧就超出 0.01 容差 0.033 px（这条一直是红的，
/// 宿主套件今天才真跑过一遍）。⇒ 改断言成它真正要守的两条：**下窜有界**（远小于上缘锚的
/// 74px）和**不出屏**。要"下缘一动不动"就得不绕中心，那是另一版观感，六六 10-24 否过。
#[test]
fn a_portrait_cart_grows_about_its_centre() {
    /// 中心锚下的位移上限：`390 + 17` 就是抬头走完时那 17px 的下窜，留 3px 余量。
    const CREEP_MAX: f32 = 20.0;
    for system in [System::Gb, System::Gbc] {
        let rest = at(system, 0.0);
        let rest_bottom = rest.y + rest.h;
        let mut t = 0.0;
        while t <= PULL_SHARE + 0.001 {
            t += 0.01;
            let d = at(system, t.min(PULL_SHARE));
            let bottom = d.y + d.h;
            assert!(
                bottom <= rest_bottom + CREEP_MAX,
                "{system:?} bottom dives {} px at {t}",
                bottom - rest_bottom
            );
            assert!(
                bottom <= OUT_H as f32 + 0.01,
                "{system:?} runs off the bottom of the panel at {t}: {bottom}"
            );
        }
        // And it does grow — a cart that arrived at the slot's width without growing would
        // not be the same object at all.
        assert!(
            (at(system, PULL_SHARE).w - SLOT_OPEN_W).abs() < 0.01,
            "{system:?} did not grow to the opening's width"
        );
    }
}

/// 第二段：**加速往下插**。位移按 `u²` 走，所以每一帧走的路必须比上一帧更长。
#[test]
fn the_fall_accelerates() {
    let step = |a: f32, b: f32| at(System::Gba, b).y - at(System::Gba, a).y;
    let first = step(PULL_SHARE + 0.02, PULL_SHARE + 0.12);
    let middle = step(PULL_SHARE + 0.45, PULL_SHARE + 0.55);
    let last = step(0.88, 0.98);
    assert!(first > 0.0, "the cart is not falling");
    assert!(middle > first, "the fall is not speeding up: {first} then {middle}");
    assert!(last > middle, "the fall is not speeding up: {middle} then {last}");
}

/// One way only, once the lift is over: a cart that went back up mid-fall reads as a bounce.
#[test]
fn the_fall_never_goes_back_up() {
    let mut last = at(System::Gba, PULL_SHARE).y;
    let mut t = PULL_SHARE;
    while t < 1.0 {
        t += 0.01;
        let y = at(System::Gba, t.min(1.0)).y;
        assert!(y >= last - 0.001, "the cart rises at {t}: {y} < {last}");
        last = y;
    }
}

/// The whole thing has to end on the frame it says it ends on, or the sound and the seat
/// disagree.
#[test]
fn the_travel_is_finished_at_one() {
    for system in [System::Gba, System::Gb, System::Gbc, System::Sfc, System::Fc] {
        let end = at(system, 1.0);
        assert!(
            end.y.is_finite() && end.w > 0.0,
            "{system:?} has no seat: {end:?}"
        );
        assert!(
            end.w <= SLOT_OPEN_W + 0.01,
            "{system:?} lands {} px wide in a {SLOT_OPEN_W} px slot",
            end.w
        );
    }
}

// ── Where each machine stops ───────────────────────────────────────────────────────────────

/// ★ 六六的口径：「**GBA 卡带贴图完全插入即卡带贴图上缘与插卡位平齐**」。
#[test]
fn an_advance_cart_lands_with_its_top_edge_level_with_the_opening() {
    let end = at(System::Gba, 1.0);
    assert!(
        (end.y - SLOT_OPEN_Y).abs() < 0.01,
        "the Advance cart's top edge lands at {} against an opening at {SLOT_OPEN_Y}",
        end.y
    );
    assert!(
        (end.w - SLOT_OPEN_W).abs() < 0.01,
        "it lands {} wide against a {SLOT_OPEN_W} px opening",
        end.w
    );
    assert!(
        (end.x - (OUT_W as f32 - end.w) / 2.0).abs() < 0.01,
        "it does not land centred"
    );
}

/// A GB/GBC cart is portrait, and it is **pulled up while it grows to the slot's 360 px** —
/// uniformly, because a portrait cart squeezed to a landscape width is not a cart any more.
#[test]
fn a_game_boy_cart_grows_to_the_slot_width_and_stops_half_in() {
    for system in [System::Gb, System::Gbc] {
        let rest = at(system, 0.0);
        let end = at(system, 1.0);
        assert!(
            (end.w - SLOT_OPEN_W).abs() < 0.01,
            "{system:?} lands {} wide against a {SLOT_OPEN_W} px opening",
            end.w
        );
        // Uniform: the aspect ratio is the cart's own at both ends of the travel.
        let want = rest.w / rest.h;
        assert!(
            (end.w / end.h - want).abs() < 0.001,
            "{system:?} is stretched: {} at rest, {} seated",
            want,
            end.w / end.h
        );
        // Half of its own height ends up below the opening's top edge.
        let depth = (end.y + end.h - SLOT_OPEN_Y) / end.h;
        assert!(
            (depth - 0.50).abs() < 0.001,
            "{system:?} is {:.1}% in, not 50%",
            depth * 100.0
        );
    }
}

/// ★「FC/SFC 的动画与 GBA 一致，但只插到一半」—— same travel, same width, half its own
/// height below the line.
#[test]
fn a_famicom_cart_stops_half_in() {
    for system in [System::Fc, System::Sfc] {
        let end = at(system, 1.0);
        let depth = (end.y + end.h - SLOT_OPEN_Y) / end.h;
        assert!(
            (depth - 0.50).abs() < 0.001,
            "{system:?} is {:.1}% in, not 50%",
            depth * 100.0
        );
        assert!(
            (end.w - SLOT_OPEN_W).abs() < 0.01,
            "{system:?} lands {} wide",
            end.w
        );
    }
}

/// By the time it is falling the cart is the slot's own width, whatever it started at.
#[test]
fn every_cart_is_the_slot_width_by_the_time_it_falls() {
    for system in [
        System::Gba,
        System::Gb,
        System::Gbc,
        System::Sfc,
        System::Fc,
        System::Neogeo,
        System::Arcade,
    ] {
        for t in [PULL_SHARE, 0.5, 0.75, 1.0] {
            let q = at(system, t);
            assert!(
                (q.w - SLOT_OPEN_W).abs() < 0.01,
                "{system:?} is {} wide at {t}",
                q.w
            );
        }
    }
}

/// A refused cart is pulled back out along the same curve, so an eject is the insert backwards.
#[test]
fn ejecting_reverses_the_travel() {
    let high = at(System::Gba, 1.0 - 0.85);
    let low = at(System::Gba, 1.0 - 0.15);
    assert!(low.y > high.y, "eject is not coming out");
}

// ── What the chrome must NOT draw ──────────────────────────────────────────────────────────

/// ★★ 2026-10-24：**旧动画的底条解绑了**。卡座是桌面图的洞，这里再画一遍屏幕上就有两个
/// 插卡位。这条是那个决定的钉子：一整个行程里，`SlotChrome` 不许推出任何塑料、开口或
/// 凹腔颜色的矩形。
#[test]
fn the_chrome_draws_no_slot_furniture() {
    for step in 0..=20 {
        let seat = step as f32 / 20.0;
        let out = chrome(seat);
        for (i, d) in out.iter().enumerate() {
            assert!(
                !(is_mouth(d) || is_lip(d) || tinted(d, housing()) || tinted(d, recess())),
                "at seat {seat}, piece {i} is slot furniture: {d:?}"
            );
        }
    }
}

/// A dark panel shows nothing. A game layer listed at zero power is a black rectangle over
/// the cart for the whole of its travel.
#[test]
fn a_dark_screen_lists_no_game_layer() {
    let mut out = Vec::new();
    out.extend(chrome(0.5));
    assert!(
        !out.iter().any(is_game_layer),
        "the picture is drawn with the screen off"
    );
}

/// The seated cart fades with the case: the frame is a lit picture now, so the cart has to
/// leave as one object with it rather than through it.
#[test]
fn the_seated_cart_leaves_with_the_case() {
    for t in [0.2_f32, 0.4, 0.6, 0.8] {
        let out = chrome_draw(System::Gba, 1.0, t);
        let a = alpha(&out[cart_at(&out)]);
        assert!(
            (a - (1.0 - t)).abs() < 0.001,
            "at {t} the cart is at {a}, not 1 - screen"
        );
        assert!(out.iter().any(is_game_layer), "no picture at {t}");
    }
}

/// The refusal symbol is drawn on the cart, so it has to fit on one.
#[test]
fn the_alert_fits_on_the_cart_face() {
    let (w, h) = icon_box(ALERT_PX);
    assert!(
        w < CART_W && h < CART_H,
        "a {w}x{h} alert on a {CART_W}x{CART_H} cart"
    );
    assert!(
        h * 4 > CART_H,
        "a {h} px alert on a {CART_H} px cart is a speck"
    );
}

// ── The programmatic slot, which is what the shelf falls back to ───────────────────────────

/// The slot is one object whether or not anything is going into it, and it is drawn by the
/// shelf (not by the chrome) — the desktop frame is the usual case, this is the fallback when
/// the frame's own texture could not be had.
#[test]
fn the_empty_slot_is_drawn_whole() {
    let mut out = Vec::new();
    draw_empty_slot(&mut out);
    assert!(
        out.iter().any(|d| tinted(d, recess())),
        "the empty slot has no recess, so it is a hole onto the backdrop"
    );
    let mouth = out
        .iter()
        .find_map(|d| is_mouth(d).then(|| quad(d)))
        .expect("no mouth drawn");
    assert!(
        mouth.y >= OUT_H as f32 - MOUTH_H - 1.0,
        "the mouth is not on the bottom edge"
    );
}

/// The GBA SP's thumb scoop: one broad arc across the *middle* of the near wall, deepest at
/// the centre, which is how you get hold of a cart to pull it out.
#[test]
fn the_slot_has_a_thumb_scoop_across_its_middle() {
    let mut out = Vec::new();
    draw_empty_slot(&mut out);
    let dark: Vec<_> = out.iter().filter(|d| is_mouth(d)).map(quad).collect();
    let slit = dark
        .iter()
        .copied()
        .max_by(|a, b| a.w.partial_cmp(&b.w).unwrap())
        .expect("no slot drawn");
    let below: Vec<_> = dark
        .iter()
        .copied()
        .filter(|q| q.y >= slit.y + slit.h)
        .collect();
    assert!(below.len() >= 8, "the scoop is not cut as a curve");

    let span_of = |pick: &dyn Fn(&Quad) -> bool| -> (f32, f32) {
        let l = below
            .iter()
            .filter(|q| pick(q))
            .map(|q| q.x)
            .fold(f32::INFINITY, f32::min);
        let r = below
            .iter()
            .filter(|q| pick(q))
            .map(|q| q.x + q.w)
            .fold(f32::NEG_INFINITY, f32::max);
        (l, r)
    };
    let (l, r) = span_of(&|_| true);
    let centre = OUT_W as f32 / 2.0;
    assert!(
        ((l + r) / 2.0 - centre).abs() < 1.0,
        "the scoop runs {l}..{r} and is not centred"
    );
    assert!(r - l < slit.w, "the scoop is as wide as the opening itself");
}

/// The slot sits in a bay stepped down from the shell.
#[test]
fn the_slot_sits_in_a_recessed_bay() {
    let mut out = Vec::new();
    draw_empty_slot(&mut out);
    let bay = out
        .iter()
        .find_map(|d| tinted(d, recess()).then(|| quad(d)))
        .expect("no bay drawn");
    let slit = out
        .iter()
        .filter(|d| is_mouth(d))
        .map(quad)
        .max_by(|a, b| a.w.partial_cmp(&b.w).unwrap())
        .expect("no slot drawn");
    assert!(bay.w > slit.w, "the bay is no wider than the opening in it");
    let step = |a: [f32; 4], b: [f32; 4]| (0..3).map(|i| a[i] - b[i]).sum::<f32>();
    assert!(
        step(housing(), recess()) > 0.0 && step(recess(), opening()) > 0.0,
        "the bay does not read as a step between the shell and the opening"
    );
}

/// The glow is light, and it has to behave like it: off both bands, on the picture side only,
/// and gone before it reaches anything.
#[test]
fn the_edge_glow_falls_off_from_both_bands() {
    let mut out = Vec::new();
    draw_edge_glow(&mut out);
    assert!(!out.is_empty(), "the glow drew nothing");
    for d in &out {
        assert!(tinted(d, edge()), "the glow is not the edge colour: {d:?}");
        assert!(alpha(d) < 1.0, "the glow is not translucent: {d:?}");
    }

    let bay = OUT_H as f32 - MOUTH_H;
    for d in &out {
        let q = quad(d);
        let clears_top = q.y >= TOP_BAND_H - 0.01;
        let clears_bay = q.y + q.h <= bay + 0.01;
        assert!(
            clears_top || clears_bay,
            "a band of the glow sits on the case: {q:?}"
        );
    }

    let mid = OUT_H as f32 / 2.0;
    let down: Vec<f32> = out.iter().filter(|d| quad(d).y < mid).map(alpha).collect();
    let up: Vec<f32> = out.iter().filter(|d| quad(d).y >= mid).map(alpha).collect();
    assert!(
        down.len() > 1 && up.len() > 1,
        "a glow has no falloff: {} down, {} up",
        down.len(),
        up.len()
    );
    for half in [&down, &up] {
        for pair in half.windows(2) {
            assert!(
                pair[1] < pair[0],
                "the glow does not dim as it goes: {half:?}"
            );
        }
    }
    assert_eq!(down.len(), up.len(), "the two glows are not the same shape");
    assert!(
        down[0] < 0.3,
        "the glow is too strong to be light: {}",
        down[0]
    );
    assert!(
        out.len() < 40,
        "the glow costs too many quads: {}",
        out.len()
    );
}

/// Nothing in the chrome is allowed to be a *second* set of slot pieces, so the only opaque
/// thing in the list is the cart itself.
#[test]
fn the_only_opaque_piece_is_the_cart() {
    for step in 0..=10 {
        let out = chrome(step as f32 / 10.0);
        let cart = cart_at(&out);
        assert!(
            out.iter().enumerate().all(|(i, d)| i == cart || !opaque(d)),
            "at seat {} something else is opaque: {:?}",
            step as f32 / 10.0,
            out.iter().map(|d| (quad(d), alpha(d))).collect::<Vec<_>>()
        );
    }
}

/// ★★ 2026-10-27（六六）：「**插卡和拔卡时也加上光效（跟 PS1 一样）**」+
/// 「**各机种的插卡位光效都跟主题色板**」。
///
/// 三条一起钉：货架上一个像素都不画（`seat == 0`）、抬头那一段也不画（光从行程的后 62%
/// 才起）、而且**颜色必须是 `palette::ridge_ink()`** —— 不是原来写死的冷蓝白。
#[test]
fn the_slot_lights_up_with_the_travel_and_wears_the_palette() {
    let glow = |seat: f32| -> Vec<[f32; 4]> {
        chrome(seat)
            .iter()
            .filter_map(|d| match *d {
                Draw::Rect { colour, .. } if colour[3] > 0.0 && colour[3] < 0.9 => Some(colour),
                _ => None,
            })
            .collect()
    };
    assert!(glow(0.0).is_empty(), "the slot is lit on the shelf");
    assert!(glow(0.2).is_empty(), "the slot lights up during the lift");

    let peak = |v: &[[f32; 4]]| v.iter().fold(0.0f32, |m, c| m.max(c[3]));
    let mid = glow(0.6);
    let home = glow(1.0);
    assert!(!mid.is_empty(), "no light in the slot with the cart on its way in");
    assert!(!home.is_empty(), "no light in the slot with the cart home");
    assert!(
        peak(&mid) < peak(&home),
        "the light does not rise with the travel: {} then {}",
        peak(&mid),
        peak(&home)
    );

    let c = slot_ui::palette::ridge_ink();
    let want = [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
    ];
    for col in &home {
        assert!(
            (0..3).all(|i| (col[i] - want[i]).abs() < 0.001),
            "the glow is not the palette's ink: {col:?} vs {want:?}"
        );
    }
}
