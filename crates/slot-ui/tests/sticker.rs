//! The label fits on the panel it is drawn on.
//!
//! ★ 2026-10-29（六六）：这个文件原先测的是**牌子上印的那些字** —— 型号行读得像真的、
//! 认证行带直流符号、十行 credit 对得上 README、序列号和条形码说的是同一件事、没有电量时
//! 不画成 0%。六六把字和条形码全删了（两棵树都删），那些断言随之失去对象，
//! 所以这里只剩一条**跟美术无关、跟面板有关**的：牌子必须装得下。

/// The label has to fit on the panel, and **it has to fit with a margin**, not exactly.
///
/// This is the assertion that was missing when the plate was 660 wide on a 640 panel:
/// `draw_sticker_at` centres it with `(OUT_W − STICKER_W) / 2`, and that arithmetic yields
/// **−10** without complaining — the division is valid, the sprite simply hangs off both sides
/// and takes the rounded corners and the die-cut notch with it.
///
/// A width is allowed to be awkward, but it is never allowed to be wider than the screen it is
/// centred on. The margin is required rather than `<=` because a label flush to the edge reads
/// as a label that got cut, even when it technically stopped in time.
#[test]
fn the_label_fits_inside_the_panel_with_room_to_spare() {
    let (w, h) = (slot_ui::STICKER_W, slot_ui::STICKER_H);
    let (ow, oh) = (slot_gfx::OUT_W, slot_gfx::OUT_H);
    assert!(
        w < ow,
        "the label is {w} across and the panel is {ow}: centred, it starts at {} and is cut \
         on both sides",
        (ow as f32 - w as f32) / 2.0
    );
    assert!(h < oh, "the label is {h} tall and the panel is {oh}");
    // At least 8 px a side, so it reads as sitting on the panel rather than filling it.
    assert!(
        ow - w >= 16,
        "only {} px of panel is left over, {} a side, which draws the label as a cut-off thing",
        ow - w,
        (ow - w) / 2
    );
}

/// The picture is carried at exactly the size it is drawn at.
///
/// A plate decoded at one size and asked for at another is a plate drawn for a different panel.
/// `sticker_face` answers `None` in that case rather than resizing, so this guards the same
/// rule from the other end: the asset in the tree *is* the face, or there is no face.
#[test]
fn the_artwork_is_carried_at_the_face_size() {
    match slot_ui::sticker_face() {
        Some(face) => assert_eq!(
            (face.w, face.h),
            (slot_ui::STICKER_W, slot_ui::STICKER_H),
            "the decoded label is {}x{}, the face is {}x{}",
            face.w,
            face.h,
            slot_ui::STICKER_W,
            slot_ui::STICKER_H
        ),
        // No artwork yet is a legitimate state — see the module note in `sticker.rs`. It is not
        // a failure of this crate, so the assertion above is simply not made.
        None => {}
    }
}