use slot_gfx::{
    blit_rect, blit_rect_fit, fit_rect, fit_scale, scale_for, System, NEOGEO_SRC_W, OUT_H, OUT_W,
};

#[test]
fn integer_scale_never_fractional() {
    assert_eq!(fit_scale(1440, 960), 2);
    assert_eq!(fit_scale(1500, 1000), 2); // rounds down, never 2.08
    assert_eq!(fit_scale(700, 400), 1); // clamps to 1 below native
    assert_eq!(fit_scale(2160, 1440), 3);
}

#[test]
fn integer_scale_is_limited_by_the_tighter_axis() {
    assert_eq!(fit_scale(2160, 960), 2);
    assert_eq!(fit_scale(1440, 1440), 2);
}

// ★ 2026-10-29（本树）：这一组验的是**宿主窗口**把整屏合成图缩放上屏的矩形，
//   所以期望值全部跟着画布走。合成图在本树是 640×480（原 720×480），
//   下面的值都按 `min(win_w/640, win_h/480)` 现算，备注里留着算式。
#[test]
fn blit_rect_centres_the_scaled_output_in_the_window() {
    // 1440/640 = 2, 960/480 = 2 ⇒ 2× ⇒ 1280×960，x = (1440-1280)/2 = 80
    assert_eq!(fit_rect(1440, 960), (80, 0, 1280, 960));
    // 1500/640 = 2, 1000/480 = 2 ⇒ 2× ⇒ x = 110, y = 20
    assert_eq!(fit_rect(1500, 1000), (110, 20, 1280, 960));
    // 2160/640 = 3, 960/480 = 2 ⇒ 取小的 2× ⇒ x = (2160-1280)/2 = 440
    assert_eq!(fit_rect(2160, 960), (440, 0, 1280, 960));
}

#[test]
fn blit_rect_overflows_symmetrically_when_the_window_is_too_small() {
    // 700/640 = 1, 400/480 = 0 ⇒ 夹到 1× ⇒ 640×480，y 溢出 (400-480)/2 = -40
    assert_eq!(fit_rect(700, 400), (30, -40, 640, 480));
}

#[test]
fn a_panel_smaller_than_the_composite_fits_rather_than_crops() {
    let (x, y, w, h) = blit_rect_fit((640, 480), 0.0);
    assert!(w <= 640 && h <= 480, "{w}x{h} does not fit a 640x480 panel");
    // ★ 本树的合成图就是 640×480，**恰好 1:1**，所以这里比的是 4:3（原 3:2）。
    assert!(
        (w as f32 / h as f32 - 4.0 / 3.0).abs() < 0.01,
        "aspect was not preserved: {w}x{h} is {}",
        w as f32 / h as f32
    );
    assert!(x >= 0 && y >= 0);
}

#[test]
fn the_device_panel_takes_the_fit_and_a_desktop_window_does_not() {
    assert_eq!(blit_rect((640, 480), 0.0), blit_rect_fit((640, 480), 0.0));
    assert_eq!(blit_rect((1500, 1000), 0.0), fit_rect(1500, 1000));
}

#[test]
fn the_fit_is_centred_and_shakes_with_the_picture() {
    // 800×480 装 640×480 的合成图：800/640 = 1、480/480 = 1 ⇒ 1×，x = (800-640)/2 = 80
    let (x, y, w, h) = blit_rect_fit((800, 480), 0.0);
    assert_eq!((x, w), ((800 - w) / 2, 640));
    assert_eq!((y, h), (0, 480));
    assert!(blit_rect_fit((800, 480), 6.0).0 > x);
}

// --- the two arcade machines ---------------------------------------------------------------
//
// One FBNeo core, two `System` variants, and the whole of the difference between them is
// where the picture lands. NeoGeo is integer-scaled like the consoles; everything else is
// stretched into a fixed 4:3 rect. Both are 320x224 boards at source, so the two rects are
// *not* the same size even though the frame they draw is.

#[test]
fn neogeo_lands_at_integer_two_like_the_consoles() {
    assert_eq!(scale_for(320, 224), 2);
    let (x, y, w, h) = System::Neogeo.screen_base();
    // ★ 2026-10-28：**608，不是 640。** FBNeo 把 NeoGeo 的 320×224 每边裁 8px overscan
    //   交过来，所以 `NEOGEO_SRC_W` 是 304（见 `system.rs` 的实测记录：kof97.zip
    //   `frame N 304x224 pitch 608`）。2× ⇒ 608×448，比 MD 的 640×448 窄 32px。
    //   这条断言原来写 640，与 `system.rs` 自己的注释互相矛盾；而整条 `scale.rs`
    //   自 a5e5ea9 起就编译不过（上一行那个 `.into()`），所以没人发现。
    assert_eq!((w, h), (608.0, 448.0));
    assert_eq!(NEOGEO_SRC_W, 304);
    assert_eq!((x, y), ((OUT_W as f32 - 608.0) / 2.0, (OUT_H as f32 - 448.0) / 2.0));
}

#[test]
fn the_other_arcade_machines_stretch_into_640x480() {
    let (x, y, w, h) = System::Arcade.screen_base();
    assert_eq!((w, h), (640.0, 480.0), "arcade does not fill 4:3");
    assert_eq!((x, y), ((OUT_W as f32 - 640.0) / 2.0, 0.0));
    // 320x224 stretched to 640x480 is a 1.14x vertical stretch — the picture is *wider* than
    // its own aspect, which is the whole point: a cabinet monitor was 4:3 and the board was not.
    assert!(w / h < 1.5, "the arcade rect is not 4:3");
}

#[test]
fn only_the_arcade_machine_stretches() {
    assert!(System::Arcade.stretches());
    assert!(!System::Neogeo.stretches());
    assert!(!System::Sfc.stretches());
    assert!(!System::Gba.stretches());
}
