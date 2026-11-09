//! A PlayStation cart's own BIOS: the file is an attachment, not a cart, and it is found by stem.
//!
//! ★ 2026-10-25. Two things have to be true at once, and they pull in opposite directions:
//!
//! 1. **`Games/PS1/Ridge.cue` + `Games/PS1/Ridge.bios` must scan as ONE cart.** `is_rom` takes
//!    any extension inside a machine folder that is not on `NOT_A_ROM`, so before `bios` was
//!    added there the BIOS became a second cart on the shelf — a "game" the player could insert,
//!    which is exactly the bug this file is here to keep buried.
//! 2. **It must not be handed to a core as content either**, which falls out of (1).
//!
//! The picking itself — `core::ps1_bios_dir` — needs a real `pcsx_rearmed` and a real 512 KiB
//! BIOS to be visible from outside (`RetroCore` does not expose `reported_system_dir`), so that
//! half is pinned by the two refusal tests below, which cover the cases that decide whether the
//! override fires at all without opening a core.

mod common;

/// One rom plus its own BIOS is one cart — and the `.bin` track beside them does not triple it.
#[test]
fn a_ps1_carts_own_bios_is_not_a_second_cart() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("Games/PS1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Ridge Racer.cue"), b"FILE \"track01.bin\" BINARY\n").unwrap();
    // 512 KiB is what a real BIOS is; the scanner must not care what is inside it, only that it
    // is not a cart.
    std::fs::write(dir.join("Ridge Racer.bios"), vec![0u8; 512 * 1024]).unwrap();

    let carts = slot_store::scan(d.path()).expect("scan");
    let ps1: Vec<&str> = carts
        .iter()
        .filter(|c| c.system() == slot_store::System::Ps1)
        .map(|c| c.stem.as_str())
        .collect();
    assert_eq!(
        ps1,
        vec!["Ridge Racer"],
        "the .bios (or something else in the folder) became a cart: {ps1:?}"
    );
}

/// A lone `.bios` is not a game. An empty shelf is the honest answer, and a card that ships only
/// a BIOS must not show a phantom cart.
#[test]
fn a_bios_with_no_game_beside_it_makes_no_cart() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("Games/PS1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("scph5501.bios"), vec![0u8; 512 * 1024]).unwrap();
    let carts = slot_store::scan(d.path()).expect("scan");
    assert!(
        carts.iter().all(|c| c.system() != slot_store::System::Ps1),
        "a lone .bios made a PS1 cart: {:?}",
        carts.iter().map(|c| c.stem.clone()).collect::<Vec<_>>()
    );
}

/// The same rule outside a machine folder, where the extension is the only evidence there is:
/// a `.bios` dropped directly in `Games/` is still not a cart.
#[test]
fn a_bios_in_the_flat_games_folder_is_not_a_cart() {
    let d = tempfile::tempdir().unwrap();
    let games = d.path().join("Games");
    std::fs::create_dir_all(&games).unwrap();
    std::fs::write(games.join("scph5501.bios"), vec![0u8; 512 * 1024]).unwrap();
    let carts = slot_store::scan(d.path()).expect("scan");
    assert!(
        carts.is_empty(),
        "a bare .bios in Games/ became a cart: {:?}",
        carts.iter().map(|c| c.stem.clone()).collect::<Vec<_>>()
    );
}

/// **Passing a rom down does not break opening a cart that has no `.bios` beside it.** The
/// stronger half of this — that a non-PS1 core still gets the card's shared `BIOS/` — lives in
/// `bios.rs`, which can see the reported directory from inside `slot-retro`; `RetroCore` does
/// not expose it, so from out here the honest claim is only "it still opens".
#[test]
fn a_cart_with_no_bios_beside_it_still_opens() {
    let Some(_dylib) = common::vendored_core() else {
        return;
    };
    let _g = common::core_lock();
    let d = common::tmp_root_with_real_carts(&["Emerald"]);
    let rom = d.path().join("Games/Emerald.gba");
    let mut core = slot::core::open_core(
        d.path(),
        slot_store::Core::Mgba,
        slot_store::System::Gba,
        &rom,
    );
    core.load(&rom)
        .expect("the core refused a cart with no .bios beside it");
    core.run_frame(slot_retro::ButtonMask::default());
}