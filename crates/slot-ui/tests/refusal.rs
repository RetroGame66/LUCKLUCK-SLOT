use slot_ui::Refusal;

/// Peak displacement over a window, so the assertion does not depend on where in the cycle
/// a given millisecond lands.
fn peak(r: &Refusal, from: u64, to: u64) -> f32 {
    (from..to).map(|t| r.offset(t).abs()).fold(0.0, f32::max)
}

#[test]
fn the_shake_decays_and_ends() {
    let r = Refusal::started(1_000);
    let early = peak(&r, 1_000, 1_100);
    let late = peak(&r, 1_200, 1_300);
    assert!(early > 0.0, "nothing moved");
    assert!(
        late < early,
        "the shake is not decaying: {early} then {late}"
    );
    assert_eq!(r.offset(1_400), 0.0, "the shake never ends");
    assert!(!r.active(1_400), "the refusal outlives its own shake");
}

/// It shakes both ways. A plate that only ever slides right is a plate that has moved.
#[test]
fn the_shake_crosses_where_it_started() {
    let r = Refusal::started(0);
    let left = (0..300).map(|t| r.offset(t)).fold(f32::INFINITY, f32::min);
    let right = (0..300)
        .map(|t| r.offset(t))
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(left < 0.0 && right > 0.0, "{left} to {right}");
}

/// ★ 2026-10-27（六六）：「拒绝时让光瞬灭一下」。
///
/// 拒绝 = 没通电，所以给它的是一条**断电曲线**，不是再加一层光：起手全灭、90 ms 内回来、
/// 比抖动（300 ms）先结束 —— 一次拒绝只能有一条"视觉句子"收尾。
#[test]
fn the_standby_light_cuts_out_and_comes_back() {
    let r = Refusal::started(1_000);
    assert_eq!(r.dip(1_000), 1.0, "the light does not go out on the frame it refused");
    assert!(r.dip(1_045) < 0.6, "the light is still half on at 45 ms");
    assert_eq!(r.dip(1_100), 0.0, "the light never comes back");
    // 一路变小，不许回弹（回弹会读成"闪两下"）。
    let mut last = r.dip(1_000);
    for t in 1_001..1_100 {
        let d = r.dip(t);
        assert!(d <= last + 1e-6, "the dip rises again at {t}: {d} > {last}");
        last = d;
    }
    // 断电先结束，抖动还在 —— 顺序反了就是"先抖完再断电"，读起来像两件事。
    assert!(r.active(1_100), "the cut ends after the shake instead of before it");
}
