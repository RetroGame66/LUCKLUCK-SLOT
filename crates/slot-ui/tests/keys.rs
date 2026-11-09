//! The operation keymap's tables, and the contract between them and the input layer.
//!
//! This file keeps the six physical keys, the fifteen targets and the turbo rate honest against
//! each other. The `SELECT` chord table is intentionally not here: it is not opened yet.

use std::collections::BTreeMap;

use slot_input::Btn;
use slot_ui::keys;

/// ★ **Six, and the six 六六 named** (2026-10-20): `A B X Y L1 R1`. The exclusions are not
/// arbitrary and a change here is a change to what the machine lets a player reach:
#[test]
fn the_physical_keys_are_the_six_a_thumb_can_reach() {
    assert_eq!(keys::PHYSICAL.len(), 6);
    for p in keys::PHYSICAL {
        assert!(
            matches!(p, Btn::A | Btn::B | Btn::X | Btn::Y | Btn::L1 | Btn::R1),
            "{p:?} is on the sheet"
        );
    }
    // The D-pad: a direction mapped onto a face button is a direction the player cannot walk.
    for out in [Btn::Up, Btn::Down, Btn::Left, Btn::Right] {
        assert!(!keys::PHYSICAL.contains(&out), "{out:?} is on the sheet");
    }
    // `Select` is reserved for the gesture layer; `Start` is the game's own pause.
    assert!(!keys::PHYSICAL.contains(&Btn::Select));
    assert!(!keys::PHYSICAL.contains(&Btn::Start));
    // `L2`/`R2` are rewind and fast forward, and the rest hand the machine away.
    for out in [Btn::L2, Btn::R2, Btn::Menu, Btn::VolUp, Btn::VolDown, Btn::Power, Btn::Lid] {
        assert!(!keys::PHYSICAL.contains(&out), "{out:?} is on the sheet");
    }
}

/// ★★ 2026-10-27：改键表的行数**按机种**不同，且不能回归。
///
/// * FC / NEOGEO / ARCADE —— 四行（`A B X Y`，无肩键行）。
/// * GBA / GB / GBC / SFC / MD / PCE —— 六行（`A B X Y L1 R1`）。
/// * PS1 —— 八行（多了 `L2 R2`，唯一带这两行的机种）。
///
/// 这条断言把「机种行数」钉死，避免 `physical_for` 被改回全局六行后，PS1 的 `L2/R2`
/// 行丢失、或 FC 的肩键行混入。
#[test]
fn the_physical_rows_follow_the_machine() {
    use slot_ui::System;
    for sys in [System::Fc, System::Neogeo, System::Arcade] {
        assert_eq!(keys::physical_for(sys).len(), 4, "{sys:?} should be 4 rows");
    }
    for sys in [
        System::Sfc,
        System::Md,
        System::Gba,
        System::Gb,
        System::Gbc,
        System::Pce,
    ] {
        assert_eq!(keys::physical_for(sys).len(), 6, "{sys:?} should be 6 rows");
    }
    assert_eq!(keys::physical_for(System::Ps1).len(), 8, "PS1 should be 8 rows");
    // PS1 八行必须含 L2/R2，且它们排在肩键之后。
    let ps1 = keys::physical_for(System::Ps1);
    assert_eq!(ps1[6], Btn::L2, "PS1 row 6 is L2");
    assert_eq!(ps1[7], Btn::R2, "PS1 row 7 is R2");
}

/// Every target's file key is its own, and every one is a spelling the card can carry.
#[test]
fn every_target_has_its_own_file_key() {
    for (i, t) in keys::TARGETS.iter().enumerate() {
        for other in &keys::TARGETS[i + 1..] {
            assert_ne!(t.name, other.name, "two targets filed under {}", t.name);
        }
        assert!(
            t.name
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'+' || b == b'_'),
            "{} is not a file key",
            t.name
        );
        assert!(!t.label.is_empty(), "{} has nothing to print", t.name);
    }
}

/// The ten game buttons, the six pairs, the two NeoGeo chords and the six turbos — plus
/// "nothing", which is the only way back to `X` and `Y` as they came.
///
/// ★ **2026-10-25：15 → 17。** `GAME_X` / `GAME_Y` 是给 MD 的 `C`/`X` 和 PS1 的 `□`/`△` 开的两个
/// libretro 位（`JOYPAD_X`/`JOYPAD_Y`，见 `keys::game_mask`）。**组合对刻意不扩** —— 一台新机器
/// 往「游戏按键」那一块加自己的键，不往组合对里加排列。
///
/// ★ **2026-11-01：21 → 23。** `TURBO_C` / `TURBO_D` 给 FBNeo 的四键用（NeoGeo 的 C/D，
/// 位 = `JOYPAD_Y` / `JOYPAD_X`）。
///
/// ★ **2026-11-02：23 → 25。** `GAME_C` / `GAME_D` 加入 **NeoGeo/ARCADE 专用**（位 = bit1 / bit9，
/// 与 `TURBO_C`/`TURBO_D` 同位），新前端不再用 `GAME_X`/`GAME_Y` 当 NeoGeo 的 C/D。MD/SFC/PS1
/// 仍走 `GAME_X`/`GAME_Y`。
#[test]
fn the_target_list_covers_every_kind_the_sheet_promises() {
    assert_eq!(keys::TARGETS.len(), 25);
    let count = |k| {
        keys::TARGETS
            .iter()
            .filter(|t| std::mem::discriminant(&t.kind) == std::mem::discriminant(&k))
            .count()
    };
    assert_eq!(count(keys::TargetKind::None), 1);
    assert_eq!(count(keys::TargetKind::Game(Btn::A)), 10);
    assert_eq!(count(keys::TargetKind::Pair(Btn::A, Btn::B)), 6);
    assert_eq!(count(keys::TargetKind::Triple(Btn::B, Btn::A, Btn::Y)), 2);
    assert_eq!(count(keys::TargetKind::Turbo(Btn::A)), 6);
}

/// ★ 2026-11-01（六六）：**FBNeo 的四键连发。** 文件键 `TURBO_A`…`TURBO_D` 与同机的
/// `GAME_A`… 同位（bit8 / bit0 / bit1 / bit9），只在 NEOGEO / ARCADE 的候选池里出现 ——
/// 那是唯一一台真的有 C、D 两颗键的机器。
///
/// **屏幕名要跟着位走。** `TURBO_A` 是 bit8，在 NeoGeo 上那颗键印的是 `B`，所以它读作
/// 「连发 B」（＝ `TURBO_B` 自己的标签）。名字错了，玩家就会挑一个写着 A 的选项去连发 B ——
/// 这条测试钉的就是这个，且它断言的是**两张标签相等**而不是某个字面量，所以两种语言的包都过。
#[test]
fn the_four_turbo_rows_are_the_neo_geos_and_they_are_named_after_it() {
    use slot_ui::System;
    let label = |name: &str| keys::target_by_name(name).unwrap().label;
    // (文件键, 位, 屏幕上该印成谁的名字)
    let want = [
        ("TURBO_A", 1 << 8, "TURBO_B"),
        ("TURBO_B", 1 << 0, "TURBO_A"),
        ("TURBO_C", 1 << 1, "TURBO_C"),
        ("TURBO_D", 1 << 9, "TURBO_D"),
    ];
    for sys in [System::Neogeo, System::Arcade] {
        let pool = keys::targets_for(sys);
        for (name, mask, printed) in want {
            let t = pool
                .iter()
                .find(|t| t.name == name)
                .unwrap_or_else(|| panic!("{sys:?} cannot pick {name}"));
            assert_eq!(t.mask(), mask, "{sys:?} {name} is the wrong bit");
            assert_eq!(
                keys::face_label(sys, name),
                label(printed),
                "{sys:?} prints {name} under the wrong name"
            );
        }
    }
    // `C`/`D` 只在这一台上：别的机种没有这两颗键，池子里就不该有这两条。
    for sys in [
        System::Gba,
        System::Gb,
        System::Gbc,
        System::Fc,
        System::Sfc,
        System::Md,
        System::Pce,
        System::Ps1,
    ] {
        for name in ["TURBO_C", "TURBO_D"] {
            assert!(
                !keys::targets_for(sys).iter().any(|t| t.name == name),
                "{sys:?} should not offer {name}"
            );
        }
    }
}

/// ★★ 2026-11-01（六六定的规则）：「**涉及核心键位排列与物理键位不符合的一律以核心键位为标准。**」
///
/// MD 是除 FBNeo 外唯一「同一位在我们表里叫 X、在机器上叫别的字母」的机种：bit8 是 MD 的 `C`、
/// bit10 是 `X`、bit11 是 `Z`。连发目标必须跟着**同一套字母**，否则玩家会看到「物理 L1 → 连发 L」
/// 而实际连发的是 MD 的 `X` 键 —— 这正是这条规则要杜绝的那种"物理键位说话"。
///
/// 断言的形状是「连发读什么 = 它连发的那颗游戏键的机器名」，两种语言的包都过。
#[test]
fn a_turbo_is_named_after_the_button_it_repeats_on_a_mega_drive() {
    use slot_ui::System;
    use slot_ui::lang;
    // (连发文件键, 同位的游戏键, 这一台上该印的连发名)
    let want = [
        ("TURBO_A", "GAME_A", lang::TARGET_TURBO_C),      // bit8  = MD C
        ("TURBO_B", "GAME_B", lang::TARGET_TURBO_B),      // bit0  = MD B
        ("TURBO_L", "GAME_L", lang::TARGET_TURBO_MD_X),   // bit10 = MD X
        ("TURBO_R", "GAME_R", lang::TARGET_TURBO_MD_Z),   // bit11 = MD Z
    ];
    let pool = keys::targets_for(System::Md);
    for (turbo, game, printed) in want {
        // ① 位必须与它连发的那颗游戏键同一位 —— 「GAME_C 对应 TURBO_C」的字面要求。
        let t = pool
            .iter()
            .find(|t| t.name == turbo)
            .unwrap_or_else(|| panic!("MD cannot pick {turbo}"));
        assert_eq!(
            t.mask(),
            keys::target_by_name(game).unwrap().mask(),
            "MD {turbo} does not pulse {game}"
        );
        // ② 屏幕名必须是那一台上那颗键的名字，不是我们表里的字母。
        assert_eq!(keys::face_label(System::Md, turbo), printed, "MD prints {turbo} wrong");
        // ③ 连发名的尾字 == 同键游戏名的尾字（「连发 C」↔「C」、「连发 X」↔「X」）。
        let face = keys::face_label(System::Md, game);
        assert!(
            printed.ends_with(face),
            "MD {turbo} prints {printed:?} but repeats {face:?}"
        );
    }
}

/// The bits are libretro's own, and `slot-ui` spells them out rather than importing the core.
/// These four are the only ones a target may name, so this is the test that keeps the hand-written
/// table honest against the type the core actually reads.
#[test]
fn the_game_bits_are_the_bits_the_core_reads() {
    let bits = |b: Btn| {
        keys::TARGETS
            .iter()
            .find(|t| matches!(t.kind, keys::TargetKind::Game(g) if g == b))
            .map(|t| t.mask())
            .unwrap_or_else(|| panic!("{b:?} has no game target"))
    };
    assert_eq!(bits(Btn::A), 1 << 8);
    assert_eq!(bits(Btn::B), 1 << 0);
    assert_eq!(bits(Btn::X), 1 << 9);
    assert_eq!(bits(Btn::Y), 1 << 1);
    assert_eq!(bits(Btn::L1), 1 << 10);
    assert_eq!(bits(Btn::R1), 1 << 11);
    assert_eq!(bits(Btn::L2), 1 << 12);
    assert_eq!(bits(Btn::R2), 1 << 13);
}

/// A pair is the two bits together, and a turbo is the one bit — the *pulse* is `Pad`'s business,
/// not the target's, so the two have to agree on the mask and differ only in kind.
#[test]
fn a_pair_is_both_bits_and_a_turbo_is_one() {
    for t in keys::TARGETS {
        match t.kind {
            keys::TargetKind::None => assert_eq!(t.mask(), 0, "{} is not silent", t.name),
            keys::TargetKind::Game(b) => assert_eq!(
                t.mask(),
                keys::game_target(b).unwrap().mask(),
                "{} disagrees with its own button",
                t.name
            ),
            keys::TargetKind::Pair(a, b) => assert_eq!(
                t.mask(),
                keys::game_target(a).unwrap().mask() | keys::game_target(b).unwrap().mask(),
                "{} is not both bits",
                t.name
            ),
            keys::TargetKind::Triple(a, b, c) => assert_eq!(
                t.mask(),
                keys::game_target(a).unwrap().mask()
                    | keys::game_target(b).unwrap().mask()
                    | keys::game_target(c).unwrap().mask(),
                "{} is not three bits",
                t.name
            ),
            keys::TargetKind::Turbo(b) => assert_eq!(
                t.mask(),
                keys::game_target(b).unwrap().mask(),
                "{} pulses the wrong button",
                t.name
            ),
        }
    }
}

/// `System/remap.txt` names a *button*, not a target. The six the core has a bit for are the
/// only ones a line there can mean, and anything else has to be skipped rather than silenced.
///
/// ★ **2026-10-25：四个 → 六个。** `X`/`Y` 现在有 `GAME_X`/`GAME_Y` 了（MD 的 `C`/`X`、PS1 的
/// `□`/`△`），所以这条断言把它们从「不许被 remap 行命名」那一组里**移到**了这一组。
#[test]
fn only_the_six_game_buttons_are_targets_a_remap_file_can_name() {
    for b in [Btn::A, Btn::B, Btn::X, Btn::Y, Btn::L1, Btn::R1] {
        assert!(keys::game_target(b).is_some(), "{b:?} has no game target");
    }
    for b in [
        Btn::Up,
        Btn::Down,
        Btn::Left,
        Btn::Right,
        Btn::Start,
        Btn::Select,
        Btn::Menu,
        Btn::VolUp,
        Btn::VolDown,
        Btn::Power,
        Btn::Lid,
    ] {
        assert!(keys::game_target(b).is_none(), "{b:?} would be silenced by a remap line");
    }
    // ★★ 2026-10-27：L2/R2 现在可被 `remap.txt` 命名（PS1 的肩键是游戏键）。
    for b in [Btn::L2, Btn::R2] {
        assert!(keys::game_target(b).is_some(), "{b:?} should now be nameable");
    }
}

/// ★ **An untouched key is the game button that shares its name, and `X`/`Y` are silent.** The
/// second half is the whole reason the two halves are one function: a row whose line the file
/// lost has to read as the buttons the game has always had.
#[test]
fn the_stock_binding_is_the_button_that_shares_the_name_and_nothing_for_x_and_y() {
    assert_eq!(keys::stock(Btn::A).name, "GAME_A");
    assert_eq!(keys::stock(Btn::B).name, "GAME_B");
    assert_eq!(keys::stock(Btn::L1).name, "GAME_L");
    assert_eq!(keys::stock(Btn::R1).name, "GAME_R");
    assert_eq!(keys::stock(Btn::X).name, "NONE");
    assert_eq!(keys::stock(Btn::Y).name, "NONE");
}

/// ★★ 2026-10-26（六六）：**出厂键位是机种的，不是卡的。** 十台机器的手柄不是同一个 ——
/// 真正有四颗面键的那几台（SFC / NEOGEO / ARCADE / MD / PS1）`X`/`Y` 出厂就接上它们的
/// 第三、第四颗；FC 的 `X`/`Y` 是连发 A/B；PS1 多一对 `L2`/`R2`。只有两颗的
/// （GBA / GB / GBC / PCE）`X`/`Y` 保持静音。
///
/// ★★ 2026-10-27：改键表按机种重排后的完整出厂矩阵。顺序是 `(A, B, X, Y, L1, R1)`，每个机器一份。
#[test]
fn the_stock_binding_follows_the_machine() {
    use slot_ui::System;
    let cases: &[(System, &str, &str, &str, &str, &str, &str)] = &[
        (System::Gba, "GAME_A", "GAME_B", "NONE", "NONE", "GAME_L", "GAME_R"),
        (System::Gb, "GAME_A", "GAME_B", "NONE", "NONE", "GAME_L", "GAME_R"),
        (System::Gbc, "GAME_A", "GAME_B", "NONE", "NONE", "GAME_L", "GAME_R"),
        (System::Pce, "GAME_A", "GAME_B", "NONE", "NONE", "GAME_L", "GAME_R"),
        (System::Fc, "GAME_A", "GAME_B", "TURBO_A", "TURBO_B", "NONE", "NONE"),
        (System::Sfc, "GAME_A", "GAME_B", "GAME_X", "GAME_Y", "GAME_L", "GAME_R"),
        (System::Md, "GAME_A", "GAME_B", "GAME_X", "GAME_Y", "GAME_L", "GAME_R"),
        (System::Neogeo, "GAME_A", "GAME_B", "GAME_D", "GAME_C", "NONE", "NONE"),
        (System::Arcade, "GAME_A", "GAME_B", "GAME_D", "GAME_C", "NONE", "NONE"),
        // PS1：X→△(GAME_X) Y→□(GAME_Y)，且带 L2/R2。
        (System::Ps1, "GAME_A", "GAME_B", "GAME_X", "GAME_Y", "GAME_L", "GAME_R"),
    ];
    for (sys, a, b, x, y, l1, r1) in cases {
        assert_eq!(keys::stock_for(*sys, Btn::A).name, *a, "{sys:?} A");
        assert_eq!(keys::stock_for(*sys, Btn::B).name, *b, "{sys:?} B");
        assert_eq!(keys::stock_for(*sys, Btn::X).name, *x, "{sys:?} X");
        assert_eq!(keys::stock_for(*sys, Btn::Y).name, *y, "{sys:?} Y");
        assert_eq!(keys::stock_for(*sys, Btn::L1).name, *l1, "{sys:?} L1");
        assert_eq!(keys::stock_for(*sys, Btn::R1).name, *r1, "{sys:?} R1");
    }
}

/// 一张什么都没写的卡在 PS1 上要读出 PS1 的默认，在 GBA 上要读出 GBA 的默认 —— 文件里丢了
/// 那行不该是「退回 Advance 的答案」。
#[test]
fn a_lost_line_falls_back_to_its_own_machine() {
    use slot_ui::System;
    let empty = BTreeMap::new();
    // ★★ 2026-10-27：PS1 的 X 出厂是 △(GAME_X)，不是 GBA 的 NONE。
    assert_eq!(keys::current_for(System::Ps1, Btn::X, &empty).name, "GAME_X");
    assert_eq!(keys::current_for(System::Gba, Btn::X, &empty).name, "NONE");
    // 显式写过的那行在任何机种上都赢。
    let mut map = BTreeMap::new();
    map.insert("X".to_string(), "GAME_A".to_string());
    assert_eq!(keys::current_for(System::Ps1, Btn::X, &map).name, "GAME_A");
}

/// ★ **Two physical keys on one game button is the point, not a bug** — 六六:「允许重复，如物理
/// A 键无改动仍是游戏按键 A，可设物理 X = 游戏 A」. The mask is a bitwise or and `Pad`'s `down[]`
/// is per *physical* button, so both release correctly; what this test holds down is that the
/// sheet does not rewrite the second one to make room.
#[test]
fn the_same_game_button_may_sit_under_two_physical_keys() {
    let mut map = BTreeMap::new();
    map.insert("X".to_string(), "GAME_A".to_string());
    let rows = keys::rows(&map);
    let a = rows.iter().find(|(p, _)| *p == Btn::A).unwrap().1;
    let x = rows.iter().find(|(p, _)| *p == Btn::X).unwrap().1;
    assert_eq!(a.name, "GAME_A");
    assert_eq!(x.name, "GAME_A", "X could not take the button A already had");
}

/// A card edited on a desktop is a card slot still has to boot. What it cannot read it skips,
/// and skipping must land on the stock binding rather than on something half-wired.
#[test]
fn a_line_naming_something_impossible_falls_back_to_the_stock_binding() {
    for bad in ["NINTENDO", "", "  ", "none", "game_a"] {
        let mut map = BTreeMap::new();
        map.insert("X".to_string(), bad.to_string());
        let rows = keys::rows(&map);
        let x = rows.iter().find(|(p, _)| *p == Btn::X).unwrap().1;
        // `game_a` is the one that *does* read — a hand-edited file in lower case is still the
        // user's, so `target_by_name` folds the case and this is the only one that lands.
        let want = if bad == "game_a" { "GAME_A" } else { "NONE" };
        assert_eq!(x.name, want, "X took {bad:?}");
    }
}

/// The spelling on the card is the spelling `System/remap.txt` uses — one table, so a line
/// written by the sheet is a line the upstream file would have understood.
#[test]
fn every_button_round_trips_through_its_name() {
    for n in keys::BTN_NAME {
        let b = keys::btn(n).unwrap_or_else(|| panic!("{n} does not read back"));
        assert_eq!(keys::name(b), n);
    }
    // And case is not the user's problem: a hand-edited file in lower case still reads.
    assert_eq!(keys::btn(" select "), Some(Btn::Select));
}

// ── The turbo rate ──────────────────────────────────────────────────────────────────────────

/// The rate is **one gear for every turbo row**, so it lives on the card beside the tables and
/// the list is `slot-store`'s — the crate that owns the file. `slot-ui` reads it from there,
/// which is what keeps the sheet and the card from offering different gears.
#[test]
fn the_turbo_list_is_the_one_the_card_is_written_from() {
    assert_eq!(keys::TURBO_HZ, slot_store::TURBO_HZ_CHOICES);
    assert_eq!(keys::TURBO_HZ_DEFAULT, slot_store::TURBO_HZ_DEFAULT);
    assert!(keys::TURBO_HZ.contains(&keys::TURBO_HZ_DEFAULT));
}

/// A rate the machine cannot run is the default, not a rate it tries and fails at — the same
/// clamp every other list on the settings screen makes.
#[test]
fn a_rate_off_the_list_reads_as_the_default() {
    for hz in [0, 1, 3, 99, 200, 255] {
        assert_eq!(keys::turbo_hz(hz), keys::TURBO_HZ_DEFAULT, "{hz} was taken as a rate");
    }
    for hz in keys::TURBO_HZ {
        assert_eq!(keys::turbo_hz(hz), hz);
    }
}
