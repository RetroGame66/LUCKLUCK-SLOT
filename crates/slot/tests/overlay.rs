//! The overlay rings: which machine owns which, and what its files are called.
//!
//! ★ 2026-10-22: the consoles join the ring — the Famicom and the Super Famicom have
//! their own art, their own reflection and their own scanline states, exactly as the Game
//! Boy and the Game Boy Color do. The slots are appended and never inserted, so a card
//! written by an older build still reads the Game Boy's and the Game Boy Color's picks
//! where it left them.

mod common;

use common::tmp_root_with_carts;
use slot::root::{OVERLAY_RINGS, overlay_files, overlay_indices, overlay_ring, write_overlay_indices};
use slot_store::System;

#[test]
fn every_machine_owns_a_ring_and_the_advance_owns_none() {
    // The mapping is the one place that knows a machine's slot and its file-name stem, so
    // it is pinned here rather than trusted.
    assert_eq!(overlay_ring(System::Gb), Some((0, "gb")));
    assert_eq!(overlay_ring(System::Gbc), Some((1, "gbc")));
    assert_eq!(overlay_ring(System::Sfc), Some((2, "sfc")));
    assert_eq!(overlay_ring(System::Fc), Some((3, "fc")));
    // NeoGeo keeps its own bezel; only the arcade machines (街机除 neogeo) stretch to
    // 640×480 and share the one bezel with PS1.
    assert_eq!(overlay_ring(System::Neogeo), Some((4, "neogeo")));
    assert_eq!(overlay_ring(System::Arcade), Some((5, "all_4-3")));
    // ★ 2026-10-25：MD 占第 6 格、PS1 占第 7 格，**追加在末尾**。MD 保持自己的遮罩；
    //   PS1 源帧 640×480，与街机共用 `Overlay/ALL_4-3*.png`。两者各自的素材若暂缺，
    //   `overlay_files` 返回空列表，旋转与扫描线停在"关"，行为为零；但槽位必须先占住。
    assert_eq!(overlay_ring(System::Md), Some((6, "md")));
    assert_eq!(overlay_ring(System::Ps1), Some((7, "all_4-3")));
    // The Advance fills the frame: no bezel to draw around it and nothing to reflect past.
    // ★ 2026-10-29（640 树）：GBA 在 640×480 面板上的画面矩形是 636×424，四周露出面板黑底
    //   —— 六六给了 640×480 的 GBA 遮罩（`Overlay/gba-00.png`，画窗 x=2,y=2），走 OVERLAY。
    //   **不带 `x` 尾缀**：边距只有 2px/28px，没有空间开反射层。
    //   GBA 占第 9 槽（0-based），`OVERLAY_RINGS` 推到 10。
    assert_eq!(overlay_ring(System::Gba), Some((9, "gba")));
}

#[test]
fn the_slots_are_the_ones_the_older_builds_wrote() {
    // The Game Boy's and the Game Boy Color's picks live in the first two slots, because
    // that is where the file has always put them. A console that appears later takes the
    // next number; it never shifts the others — the same rule as the card's own folders.
    let (gb, _) = overlay_ring(System::Gb).unwrap();
    let (gbc, _) = overlay_ring(System::Gbc).unwrap();
    assert_eq!((gb, gbc), (0, 1), "the two shipping machines keep their slots");
    // ★ 2026-10-25：4 → 8。`OVERLAY_RINGS` 是**数组长度**，不是有素材的机种数 —— GB/GBC/SFC/FC
    //   四格是 10-22 定的，NEOGEO/ARCADE 两格当时没有 ring 却也占着号（`overlay_ring` 对它们返回
    //   `None`，但下标数组照旧按 8 长），MD/PS1 又各占一格。缩到「恰好有素材的个数」就会越界。
    // ★ 2026-10-29（640 树）：GBA 占第 9 槽 ⇒ 数组长 10。
    assert_eq!(OVERLAY_RINGS, 10, "one slot per machine, whether or not the art exists yet");
}

#[test]
fn the_whole_ring_survives_the_file() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_overlay_indices(d.path(), [1, 2, 3, 4, 5, 1, 6, 7, 8, 0]);
    assert_eq!(overlay_indices(d.path()), [1, 2, 3, 4, 5, 1, 6, 7, 8, 0]);
}

#[test]
fn a_card_from_an_older_build_still_reads() {
    // `System/overlay.txt` used to hold two numbers — the Game Boy's and the Game Boy
    // Color's. The consoles' slots read as zero, which is their default, and the two older
    // picks stay where they were.
    let d = tmp_root_with_carts(&["Emerald"]);
    let file = d.path().join("System/overlay.txt");
    std::fs::write(&file, "3 1").unwrap();
    assert_eq!(overlay_indices(d.path()), [3, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
    // And no file at all is the same answer.
    let _ = std::fs::remove_file(&file);
    assert_eq!(overlay_indices(d.path()), [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn the_consoles_files_are_theirs_own() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let dir = d.path().join("Overlay");
    std::fs::create_dir_all(&dir).unwrap();
    // Each console's own bezel, the Game Boy's two, and the one bezel shared by the
    // 640×480-stretched machines (arcade-except-neogeo and PS1).
    for name in [
        "GB-00x.png",
        "GBC-00x.png",
        "SFC-00x.png",
        "FC-00x.png",
        "NEOGEO-00x.png",
        "MD-00x.png",
        "ALL_4-3_00x.png",
        "all_4-3_01.png",
    ] {
        std::fs::write(dir.join(name), b"").unwrap();
    }
    // The Game Boy and Game Boy Color keep their own bezels.
    let gb = overlay_files(d.path(), "gb");
    assert_eq!(gb.len(), 1);
    assert_eq!(gb[0].path.file_name().unwrap(), "GB-00x.png");
    let gbc = overlay_files(d.path(), "gbc");
    assert_eq!(gbc.len(), 1);
    assert_eq!(gbc[0].path.file_name().unwrap(), "GBC-00x.png");
    // The integer-scaled consoles keep their own named bezels; they never read the shared one.
    for (stem, want) in [
        ("sfc", "SFC-00x.png"),
        ("fc", "FC-00x.png"),
        ("neogeo", "NEOGEO-00x.png"),
        ("md", "MD-00x.png"),
    ] {
        let got = overlay_files(d.path(), stem);
        assert_eq!(got.len(), 1, "{stem} keeps its own bezel");
        assert_eq!(got[0].path.file_name().unwrap(), want);
    }
    // Only the arcade machines (街机除 neogeo) and PS1 stretch to 640×480 and share the
    // SAME bezel. The trailing `x` marks it reflective. Both separators are accepted; the
    // user's file uses an underscore.
    let arcade = overlay_files(d.path(), "all_4-3");
    assert_eq!(arcade.len(), 2, "the reflective and the plain copy");
    // Sorted by (rank, reflect, name): `false < true`, so the plain copy leads and the
    // reflective `x` file follows.
    assert_eq!(arcade[0].path.file_name().unwrap(), "all_4-3_01.png");
    assert!(!arcade[0].reflect);
    assert_eq!(arcade[1].path.file_name().unwrap(), "ALL_4-3_00x.png");
    assert!(arcade[1].reflect, "the `x` file asks for the reflection");
    // PS1 resolves to the identical shared set.
    let ps1 = overlay_files(d.path(), "all_4-3");
    assert_eq!(ps1.len(), 2);
    assert_eq!(ps1[1].path.file_name().unwrap(), "ALL_4-3_00x.png");
}
#[test]
fn a_card_with_no_console_art_is_not_an_error() {
    // No art at all: the ring is empty and the caller sees nothing to draw, so the picture
    // comes up the way it always did. Not a failure, and not a reason to refuse the cart.
    let d = tmp_root_with_carts(&["Emerald"]);
    assert!(overlay_files(d.path(), "fc").is_empty());
    assert!(overlay_files(d.path(), "sfc").is_empty());
}
