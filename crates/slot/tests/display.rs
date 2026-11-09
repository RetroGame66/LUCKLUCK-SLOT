//! What a card's `System/display.txt` is allowed to ask for.
//!
//! The file keeps one `key value` line per machine (`mask`, `scanline`, then
//! `gba`/`gb`/`gbc`/`sfc`/`fc`/`neogeo`/`arcade`/`md`/`ps1`/`pce`),
//! plus a legacy single-line integer form from earlier builds. It is the only thing that survives
//! a reboot, so every preset the interface can reach has to come back as itself.

mod common;

use common::tmp_root_with_carts;
use slot::root::MASK_STATES;
use slot::root::{display_modes, write_display_modes};

#[test]
fn the_whole_ring_survives_the_file() {
    // Every preset the Advance's ring holds, written and read back. The clamp that reads the file
    // was left on the old count of five when the scanline was added, so the two newest presets
    // were written, shown, persisted — and then silently read back as LCD3x 100% on the next boot.
    // Nothing else in the tree could have caught it: the file was always the right file, and the
    // number in it was always the right number.
    let d = tmp_root_with_carts(&["Emerald"]);
    for mask in 0..MASK_STATES as u8 {
        write_display_modes(d.path(), mask, [3, 7, 2, 0, 0, 5, 1, 4, 6, 0], 0);
        let (read, cc, scanline) = display_modes(d.path());
        assert_eq!(read, mask, "preset {mask} did not survive the file");
        assert_eq!(cc, [3, 7, 2, 0, 0, 5, 1, 4, 6, 0], "and neither did the ten machine slots");
        assert_eq!(scanline, 0, "scanline should round-trip its zero");
    }
    // Past the end clamps to the last preset rather than wrapping, for a file edited on a PC.
    write_display_modes(d.path(), 99, [0; 10], 99);
    let (mask, _cc, scanline) = display_modes(d.path());
    assert_eq!(mask as usize, MASK_STATES - 1);
    assert_eq!(scanline, 2, "an out-of-range scanline clamps to the top preset");
}

#[test]
fn one_line_per_machine_round_trips() {
    // The current keyed format: each machine keeps its own pick independently, so changing one
    // does not move another.
    let d = tmp_root_with_carts(&["Emerald"]);
    write_display_modes(d.path(), 2, [1, 5, 3, 0, 4, 6, 2, 7, 8, 0], 1);
    assert_eq!(
        display_modes(d.path()),
        (2, [1, 5, 3, 0, 4, 6, 2, 7, 8, 0], 1)
    );
    // A partial keyed file leaves the unwritten machines at their default 0.
    std::fs::write(d.path().join("System/display.txt"), "mask 1\ngba 6\ngb 2").unwrap();
    assert_eq!(display_modes(d.path()), (1, [6, 2, 0, 0, 0, 0, 0, 0, 0, 0], 0));
}

#[test]
fn the_scanline_field_round_trips_and_clamps() {
    // The three presets (off / half / full), one line per slot, plus the clamp for a PC-edited
    // card that names a fourth.
    let d = tmp_root_with_carts(&["Emerald"]);
    for scanline in 0..=2u8 {
        write_display_modes(d.path(), 0, [0; 10], scanline);
        assert_eq!(display_modes(d.path()).2, scanline);
    }
    write_display_modes(d.path(), 0, [0; 10], 7);
    assert_eq!(display_modes(d.path()).2, 2, "a hand-written 999 clamps to full");
}

#[test]
fn a_missing_or_older_file_still_reads() {
    // A card from an earlier build (single line of integers), one edited to a single grade for
    // everything, and one with nothing at all. None is an error.
    let d = tmp_root_with_carts(&["Emerald"]);
    let file = d.path().join("System/display.txt");
    let _ = std::fs::remove_file(&file);
    assert_eq!(display_modes(d.path()), (2, [0; 10], 0), "the built-in default");

    // Legacy four-integer file: `mask cc_gba cc_gb cc_gbc` from v2026.10.02.
    std::fs::write(&file, "2 0 0 0").unwrap();
    assert_eq!(display_modes(d.path()), (2, [0; 10], 0), "legacy positional, default");

    // A lone `mask cc` meant one grade for every machine.
    std::fs::write(&file, "1 4").unwrap();
    assert_eq!(display_modes(d.path()), (1, [4; 10], 0), "one colour, every machine");

    std::fs::write(&file, "not a number").unwrap();
    assert_eq!(display_modes(d.path()), (2, [0; 10], 0), "and it does not panic on nonsense");
}
