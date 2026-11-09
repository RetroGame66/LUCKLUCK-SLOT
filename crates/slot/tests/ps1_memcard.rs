//! The options a core must be told **before `retro_init`**, and the one that cost a player
//! their PlayStation saves.
//!
//! ★★ 2026-11-01. The report was "the 640 build will not save" from a player. The bug was not
//! about resolution at all — it had been there since PS1 landed on 2026-10-25, identically on
//! both trees (`core.rs` was byte for byte the same on the 720 and 640 sides).
//!
//! What happened, read off `pcsx_rearmed`'s own source rather than guessed:
//!
//! * `retro_get_memory_data(RETRO_MEMORY_SAVE_RAM)` answers `Mcd1Data` **only** while
//!   `memcard_type[0] == MEMCARDTYPE_LIBRETRO`, and `NULL` otherwise
//!   (`libretro.c:2231`/`2244`). No pointer, no `save_ram()` bytes, no `.sav` on the card.
//! * `memcard_type` is an enum whose zero value is `MEMCARDTYPE_NONE` (`libretro.c:133-138`),
//!   so it starts as "no memory card" and only `load_memcards()` ever changes it.
//! * `load_memcards()` asks the frontend for `pcsx_rearmed_memcard1` and **`continue`s on no
//!   answer** (`libretro.c:3783-3790`) — the core's own "Libretro (Default)" in its option
//!   table is never consulted.
//! * SLOT's `environment()` answers `GET_VARIABLE` out of `h.options`, which only holds keys
//!   something called `set_option` for. It does not implement `SET_CORE_OPTIONS` either, so
//!   the advertised default has no path to the core at all.
//!
//! So the entry below is not a preference — it is what makes a PlayStation memory card exist.
//! The tests are on the list itself, because a list is what a missing entry breaks.

/// The bug, stated as the one assertion that would have caught it.
#[test]
fn ps1_is_told_to_run_a_libretro_memory_card() {
    let opts = slot::core::initial_core_options(slot_store::Core::Pcsx);
    let value = opts
        .iter()
        .find(|(k, _)| *k == "pcsx_rearmed_memcard1")
        .map(|(_, v)| *v);
    assert_eq!(
        value,
        Some("libretro"),
        "without this, pcsx hands back NULL for RETRO_MEMORY_SAVE_RAM and no PS1 cart can save"
    );
}

/// The frame-flip option this list already carried, kept next to it so a future edit that
/// rewrites the PS1 arm cannot silently drop either one.
#[test]
fn ps1_keeps_its_single_field_frame_flip() {
    let opts = slot::core::initial_core_options(slot_store::Core::Pcsx);
    assert!(
        opts.iter()
            .any(|(k, v)| *k == "pcsx_rearmed_alt_flip" && *v == "early"),
        "the interlace fix must survive whatever else the PS1 arm grows"
    );
}

/// Card 2 is deliberately *not* set: an unanswered slot lands on the same `MEMCARDTYPE_NONE`
/// an explicit `none` would, so a second line would buy nothing and could only drift.
#[test]
fn ps1_is_not_given_a_second_memory_card() {
    let opts = slot::core::initial_core_options(slot_store::Core::Pcsx);
    assert!(
        !opts.iter().any(|(k, _)| *k == "pcsx_rearmed_memcard2"),
        "slot 2 defaults to none on its own; naming it here would be a second source of truth"
    );
}

/// `libretro` is the token the core compares against (`strcmp(var.value, "libretro")`), not
/// the label its menu shows. A label here would fail the comparison and land on `none`.
#[test]
fn the_memory_card_mode_is_the_token_not_the_label() {
    let opts = slot::core::initial_core_options(slot_store::Core::Pcsx);
    for (k, v) in opts {
        assert!(
            !v.contains(' ') && !v.contains('('),
            "{k} was handed a menu label ({v:?}); cores compare against the token"
        );
    }
}

/// The other machines' entries are untouched by this fix — the regression test states the
/// list's shape so an edit to one arm cannot quietly rewrite another.
#[test]
fn the_other_cores_keep_their_own_startup_options() {
    use slot_store::Core;
    // mGBA's two are read during `retro_init`; a missing one brings back the Super Game Boy
    // border and a colourised DMG palette.
    let mgba = slot::core::initial_core_options(Core::Mgba);
    assert!(mgba.iter().any(|(k, v)| *k == "mgba_sgb_borders" && *v == "OFF"));
    assert!(mgba
        .iter()
        .any(|(k, v)| *k == "mgba_gb_colors" && *v == "Grayscale"));

    // fceumm's overscan crop is what puts FC's frame exactly on `FC_SRC_H`.
    let nes = slot::core::initial_core_options(Core::Nes);
    assert!(nes.iter().any(|(k, v)| *k == "fceumm_overscan_v_top" && *v == "8"));
    assert!(nes
        .iter()
        .any(|(k, v)| *k == "fceumm_overscan_v_bottom" && *v == "8"));

    // SFC's pixel format is why a SFC cart shows a picture rather than a black screen.
    let snes = slot::core::initial_core_options(Core::Snes);
    assert!(snes
        .iter()
        .any(|(k, v)| *k == "supafaust_pixel_format" && *v == "xrgb8888"));

    // gpSP has nothing to seed before init; its serial mode is set in `apply_core_options`.
    assert!(slot::core::initial_core_options(Core::Gpsp).is_empty());
}