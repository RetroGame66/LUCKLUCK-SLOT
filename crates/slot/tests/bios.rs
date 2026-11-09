mod common;

use std::path::PathBuf;

use common::core_lock;
use slot::core::open_core_for;
use slot_retro::{ButtonMask, LibretroCore};
use slot_store::Core;
use tempfile::tempdir;

fn vendored_core_paths() -> Vec<PathBuf> {
    common::vendored_core().into_iter().collect()
}

#[test]
fn a_missing_bios_folder_still_boots_a_core() {
    let _g = core_lock();
    let d = common::tmp_root_with_real_carts(&["Emerald"]);
    std::fs::remove_dir_all(d.path().join("BIOS")).ok();
    let mut core = open_core_for(d.path(), Core::Mgba, slot_store::System::Gba, &vendored_core_paths());
    core.load(&d.path().join("Games/Emerald.gba")).unwrap();
    core.run_frame(ButtonMask::default());
}

#[test]
fn an_empty_bios_folder_still_boots_a_core() {
    let _g = core_lock();
    let d = common::tmp_root_with_real_carts(&["Emerald"]);
    std::fs::create_dir_all(d.path().join("BIOS")).unwrap();
    let mut core = open_core_for(d.path(), Core::Mgba, slot_store::System::Gba, &vendored_core_paths());
    core.load(&d.path().join("Games/Emerald.gba")).unwrap();
    core.run_frame(ButtonMask::default());
}

#[test]
fn the_core_is_told_the_bios_folder_not_the_dylib_folder() {
    let _g = core_lock();
    let d = tempdir().unwrap();
    let bios = d.path().join("BIOS");
    let saves = d.path().join("Saves");
    std::fs::create_dir_all(&bios).unwrap();
    std::fs::create_dir_all(&saves).unwrap();
    let Some(dylib) = common::vendored_core() else {
        return;
    };
    let core = LibretroCore::open_with(&dylib, &bios, &saves, &[], false).unwrap();
    assert_eq!(core.reported_system_dir(), bios.to_string_lossy());
    assert_ne!(
        core.reported_system_dir(),
        dylib.parent().unwrap().to_string_lossy()
    );
}

/// The save directory is the other half of the same wiring, and pointing it at the dylib
/// would scatter `.sav` files next to the core instead of into the content root.
#[test]
fn the_core_is_told_the_saves_folder_too() {
    let _g = core_lock();
    let d = tempdir().unwrap();
    let bios = d.path().join("BIOS");
    let saves = d.path().join("Saves");
    std::fs::create_dir_all(&bios).unwrap();
    std::fs::create_dir_all(&saves).unwrap();
    let Some(dylib) = common::vendored_core() else {
        return;
    };
    let core = LibretroCore::open_with(&dylib, &bios, &saves, &[], false).unwrap();
    assert_eq!(core.reported_save_dir(), saves.to_string_lossy());
}

/// A card that has never held slot. has none of the six folders, and every write path
/// below assumes its own is already there.
#[test]
fn boot_creates_the_six_content_folders() {
    let d = tempdir().unwrap();
    let _ = slot::app::App::boot(d.path());
    for sub in ["BIOS", "Games", "Saves", "States", "System"] {
        assert!(d.path().join(sub).is_dir(), "{sub} was not created");
    }
}

/// ★★★ 2026-11-03（六六）：玩家反馈「Labels 改了很不方便」—— **`Labels` 回到开机建的目录表**
/// （2026-10-28 那次把它摘掉是因为"体系取消"，但取消之后玩家连放图的地方都没有了）。
/// `Cartridge` 仍是整卡图的家，`Art` 那个旧名字不许回来。
#[test]
fn the_content_root_builds_labels_alongside_the_art_directory() {
    assert!(
        slot::root::DIRS.contains(&"Labels"),
        "Labels 已恢复：玩家要有个地方放自己的封面"
    );
    assert!(slot::root::DIRS.contains(&"Cartridge"));
    assert!(
        !slot::root::DIRS.contains(&"Art"),
        "Art survived the rename"
    );
}
