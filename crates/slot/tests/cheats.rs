mod common;

use std::fs;

use slot::root::cheats;
use tempfile::TempDir;

/// Drop a `.cht` beside a cart, the way a player would on a PC.
fn card_with_cht(dir: &TempDir, stem: &str, body: &str) {
    let cheats = dir.path().join("System").join("Cheats");
    fs::create_dir_all(&cheats).unwrap();
    fs::write(cheats.join(format!("{stem}.cht")), body).unwrap();
}

fn card() -> TempDir {
    common::tmp_root_with_carts(&["Emerald"])
}

/// The shape `libretro-database` actually ships: `cheats` and then three keys per entry.
#[test]
fn reads_the_retroarch_format_verbatim() {
    let d = card();
    card_with_cht(
        &d,
        "Emerald",
        "cheats = 2\n\
         \n\
         cheat0_desc = \"Enable Code (Must Be On)\"\n\
         cheat0_code = \"000084F5+000A+100193C0+0007\"\n\
         cheat0_enable = true\n\
         \n\
         cheat1_desc = \"Infinite Health\"\n\
         cheat1_code = \"3200E924+0096\"\n\
         cheat1_enable = false\n",
    );
    let list = cheats(d.path(), "Emerald");
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].desc, "Enable Code (Must Be On)");
    assert_eq!(list[0].code, "000084F5+000A+100193C0+0007");
    assert!(list[0].enabled, "the file said true");
    assert_eq!(list[1].desc, "Infinite Health");
    assert_eq!(list[1].code, "3200E924+0096");
    assert!(!list[1].enabled, "the file said false");
}

/// The `cheats = N` count is RetroArch's own requirement, not this reader's: a hand-written
/// file with three lines and no count is exactly what a player will make on a PC.
#[test]
fn a_file_without_the_count_line_still_reads() {
    let d = card();
    card_with_cht(
        &d,
        "Emerald",
        "cheat0_desc = \"Walk Through Walls\"\n\
         cheat0_code = \"3200E924+0096\"\n",
    );
    let list = cheats(d.path(), "Emerald");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].desc, "Walk Through Walls");
    assert!(!list[0].enabled, "no enable key means off, as in RetroArch");
}

/// Only these two spellings switch a code on. `True` and `on` are `false` to RetroArch, so they
/// must be `false` here — a card that looks enabled but is not would be the worst outcome.
#[test]
fn enable_is_the_two_exact_spellings() {
    let d = card();
    card_with_cht(
        &d,
        "Emerald",
        "cheat0_code = \"AAAAAAAA+0001\"\n\
         cheat0_enable = true\n\
         cheat1_code = \"AAAAAAAA+0002\"\n\
         cheat1_enable = 1\n\
         cheat2_code = \"AAAAAAAA+0003\"\n\
         cheat2_enable = True\n\
         cheat3_code = \"AAAAAAAA+0004\"\n\
         cheat3_enable = yes\n",
    );
    let list = cheats(d.path(), "Emerald");
    let flags: Vec<bool> = list.iter().map(|c| c.enabled).collect();
    assert_eq!(flags, vec![true, true, false, false]);
}

/// A quoted value is taken whole — a `#` in a description is text, not the start of a comment.
#[test]
fn a_hash_inside_quotes_is_text() {
    let d = card();
    card_with_cht(
        &d,
        "Emerald",
        "cheat0_desc = \"Stage 1 # shortcut\"\n\
         cheat0_code = \"AAAAAAAA+0001\"\n",
    );
    let list = cheats(d.path(), "Emerald");
    assert_eq!(list[0].desc, "Stage 1 # shortcut");
}

/// A bare (unquoted) value runs to the end of the line. RetroArch's own reader would stop it at
/// the first space; copying that would truncate a space-separated code into half a code, and
/// mGBA treats a space between groups exactly as it treats `+`.
#[test]
fn a_bare_value_keeps_its_spaces() {
    let d = card();
    card_with_cht(
        &d,
        "Emerald",
        "cheat0_desc = Infinite Health\n\
         cheat0_code = 3200E924 0096\n",
    );
    let list = cheats(d.path(), "Emerald");
    assert_eq!(list[0].desc, "Infinite Health");
    assert_eq!(list[0].code, "3200E924 0096");
}

/// A trailing `#` on an unquoted value is still a comment.
#[test]
fn a_trailing_comment_on_a_bare_value_is_dropped() {
    let d = card();
    card_with_cht(
        &d,
        "Emerald",
        "cheat0_code = 3200E924+0096 # health\n",
    );
    assert_eq!(cheats(d.path(), "Emerald")[0].code, "3200E924+0096");
}

/// Comments, blank lines and keys this build does not know are all skipped, not errors.
#[test]
fn comments_and_unknown_keys_are_skipped() {
    let d = card();
    card_with_cht(
        &d,
        "Emerald",
        "# a comment above everything\n\
         \n\
         cheat0_desc = \"Keep\"\n\
         cheat0_code = \"AAAAAAAA+0001\"\n\
         cheat0_cheat_type = 1\n\
         cheat0_handler = 3\n\
         cheat0_rumble_port = 0\n\
         some_other_key = 42\n",
    );
    let list = cheats(d.path(), "Emerald");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].desc, "Keep");
}

/// A missing code means there is nothing to apply, so the entry is not a row at all.
#[test]
fn an_entry_with_no_code_is_not_a_row() {
    let d = card();
    card_with_cht(
        &d,
        "Emerald",
        "cheat0_desc = \"Only a label\"\n\
         cheat1_desc = \"Real\"\n\
         cheat1_code = \"AAAAAAAA+0001\"\n",
    );
    let list = cheats(d.path(), "Emerald");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].desc, "Real");
}

/// Entries come out by their number, not by the order their lines happened to land in.
#[test]
fn entries_keep_their_numbered_order() {
    let d = card();
    card_with_cht(
        &d,
        "Emerald",
        "cheat2_desc = \"Third\"\n\
         cheat2_code = \"AAAAAAAA+0003\"\n\
         cheat0_desc = \"First\"\n\
         cheat0_code = \"AAAAAAAA+0001\"\n\
         cheat1_desc = \"Second\"\n\
         cheat1_code = \"AAAAAAAA+0002\"\n",
    );
    let list = cheats(d.path(), "Emerald");
    let descs: Vec<&str> = list.iter().map(|c| c.desc.as_str()).collect();
    assert_eq!(descs, vec!["First", "Second", "Third"]);
}

/// The old `.txt` format is gone: a card that still carries one gets no cheats rather than a
/// half-read list from a format nothing parses any more.
#[test]
fn the_old_txt_format_is_no_longer_read() {
    let d = card();
    let cheats_dir = d.path().join("System").join("Cheats");
    fs::create_dir_all(&cheats_dir).unwrap();
    fs::write(
        cheats_dir.join("Emerald.txt"),
        "8A1B1234+1C2D5678 # 无限生命\n",
    )
    .unwrap();
    assert!(cheats(d.path(), "Emerald").is_empty());
}

/// No file, no cheats — the table then shows its NO CHEATS panel instead of erroring.
#[test]
fn a_cart_without_a_cht_has_no_cheats() {
    let d = card();
    assert!(cheats(d.path(), "Emerald").is_empty());
}

/// A `.cht` RetroArch itself wrote carries keys this build has no use for. The file still reads,
/// and the keys do not leak into a row's label or code.
#[test]
fn a_full_retroarch_file_reads_without_its_extra_keys() {
    let d = card();
    card_with_cht(
        &d,
        "Emerald",
        "cheats = 1\n\
         \n\
         cheat0_desc = \"Infinite Money\"\n\
         cheat0_code = \"82025BC4+967A\"\n\
         cheat0_enable = false\n\
         cheat0_cheat_type = 0\n\
         cheat0_handler = 0\n\
         cheat0_address = 0x25BC4\n\
         cheat0_address_bit_position = 0\n\
         cheat0_big_endian = false\n\
         cheat0_memory_search_size = 3\n\
         cheat0_repeat_count = 1\n\
         cheat0_repeat_add_to_value = 0\n\
         cheat0_repeat_add_to_address = 1\n\
         cheat0_rumble_port = 0\n\
         cheat0_rumble_primary_strength = 0\n\
         cheat0_rumble_primary_duration = 0\n\
         cheat0_rumble_secondary_strength = 0\n\
         cheat0_rumble_secondary_duration = 0\n\
         cheat0_rumble_type = 0\n\
         cheat0_rumble_value = 0\n\
         cheat0_value = 0\n",
    );
    let list = cheats(d.path(), "Emerald");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].desc, "Infinite Money");
    assert_eq!(list[0].code, "82025BC4+967A");
    assert!(!list[0].enabled);
}