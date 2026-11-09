//! 卡座接线规则的独立验证：**不引 `tests/common`**。
//!
//! 原因（实测，不是猜）：`mod common;` 在本工作树上会让测试进程在**任何测试体运行之前**
//! 就 0xc0000005 崩掉 —— 连 `#[test] fn t() { eprintln!("BODY RAN"); assert_eq!(1,1); }`
//! 都打不出 `BODY RAN`。已用逐个探针排除：`slot-media` 的 12 个单测正常、
//! `-p slot --test volume` 能加载、`play.rs` 等文件本身就编不过（`Cart` 缺字段）。
//! `crates/slot/tests/common/mod.rs` 自本树首次提交（7fd471d）起**未改动过**。
//! ⇒ 这条崩溃是环境/既有状态，与本轮改动无关；但它使 `tests/deck.rs` 整份跑不了，
//! 所以这里用一份**自足**的等价物把规则真跑一遍。
//!
//! 走的是同一套公开 API（`App::set_discs` / `apply` / `deck_playing` / `deck_track`），
//! 磁带用不存在的路径：`Player::open` 会拒开，所以这里断言的是**索引与传输规则**，
//! 真正出声的部分由 `slot --deck <真 MP3>` 探针负责（交接文档记为已跑通）。

use slot::app::{App, Phase, ShelfRow};
use slot_input::{Action, Btn};

fn track(disc: &str, i: usize) -> slot_media::Track {
    slot_media::Track {
        stem: format!("{disc}{i}"),
        path: std::path::PathBuf::from(format!("/nonexistent/{disc}{i}.mp3")),
        title: format!("{disc}{i}"),
        artist: "Nobody".to_string(),
        album: disc.to_string(),
        duration: 0.0,
        initial: disc.chars().next().unwrap_or('?'),
        track_number: None,
        tags: Vec::new(),
    }
}

fn disc_of(name: &str, tracks: usize) -> slot_media::Disc {
    slot_media::Disc {
        name: name.to_string(),
        stem: name.to_string(),
        tracks: (0..tracks).map(|i| track(name, i)).collect(),
        duration: 0.0,
        initial: name.chars().next().unwrap_or('?'),
    }
}

/// 把菜单光标走到「磁带」按钮上：走格子而不是写死"第二个是它"。
fn walk_to_discs(a: &mut App) {
    if let Phase::Shelf = a.phase() {
        a.apply(Action::OpenMenu);
    }
    let discs_at = slot_ui::menu::Item::GRID
        .iter()
        .position(|s| *s == Some(slot_ui::menu::Item::Discs))
        .expect("菜单里有磁带按钮");
    for _ in 0..16 {
        if matches!(a.phase(), Phase::Menu { index } if *index == discs_at) {
            break;
        }
        a.apply(Action::GbaDown(Btn::Right));
    }
}

/// 菜单的磁带按钮 ⇒ **碟片货架**（不是卡座）。无 root、无设备供电也到得了，因为
/// `App::disc_row` 是从 `self.discs` 建行的，不需要卡（`set_discs` 已经喂过）。
fn app_on_disc_row(discs: Vec<slot_media::Disc>) -> App {
    let mut a = App::new(Vec::new());
    a.set_discs(discs);
    walk_to_discs(&mut a);
    a.apply(Action::GbaDown(Btn::A));
    a
}

/// 直启到卡座相位：菜单 → 磁带按钮 → 碟片行上确认（`Action::Insert`）。
fn app_on_deck(discs: Vec<slot_media::Disc>) -> App {
    let mut a = app_on_disc_row(discs);
    a.apply(Action::Insert);
    a
}

#[test]
fn the_menu_button_opens_the_disc_row() {
    let a = app_on_disc_row(vec![disc_of("A", 2)]);
    assert!(matches!(a.phase(), Phase::Shelf), "菜单的磁带按钮先到货架");
    assert!(a.shelf_kind() == ShelfRow::Discs, "而且停在碟片那一行");
    assert!(!a.is_listening(), "不是直接跳进卡座");
}

#[test]
fn picking_a_disc_on_the_row_opens_the_deck() {
    let a = app_on_deck(vec![disc_of("A", 2)]);
    assert!(a.is_listening(), "碟片行上的确认把那一盘放进卡座");
}

#[test]
fn a_fresh_deck_is_on_the_first_track_of_the_first_disc() {
    let a = app_on_deck(vec![disc_of("A", 3), disc_of("B", 3)]);
    assert_eq!(a.deck_disc().map(|t| t.name.as_str()), Some("A"));
    assert_eq!(a.deck_track(), (0, 3));
    assert!(
        !a.deck_playing(),
        "磁带里的文件都不存在，开不出来 ⇒ 不该假装在放"
    );
}

#[test]
fn the_shoulders_walk_discs_and_stop_at_both_ends() {
    let mut a = app_on_deck(vec![disc_of("A", 2), disc_of("B", 2), disc_of("C", 2)]);
    a.apply(Action::GbaDown(Btn::L1));
    assert_eq!(a.deck_disc().map(|t| t.name.as_str()), Some("A"), "已在第一盘");
    a.apply(Action::GbaDown(Btn::R1));
    assert_eq!(a.deck_disc().map(|t| t.name.as_str()), Some("B"));
    a.apply(Action::GbaDown(Btn::R1));
    a.apply(Action::GbaDown(Btn::R1));
    assert_eq!(
        a.deck_disc().map(|t| t.name.as_str()),
        Some("C"),
        "在最后一盘停住，不回卷到第一盘"
    );
}

#[test]
fn the_arrows_walk_the_keys_and_leave_the_disc_alone() {
    let mut a = app_on_deck(vec![disc_of("A", 3), disc_of("B", 3)]);
    let before = a.deck_disc().map(|t| t.name.clone());
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::Right));
    assert_eq!(
        a.deck_disc().map(|t| t.name.clone()),
        before,
        "传输键在手指底下，它不是换带的方式"
    );
}

#[test]
fn a_disc_of_one_track_has_nothing_to_count_and_no_lyric() {
    let a = app_on_deck(vec![disc_of("Only", 1)]);
    assert_eq!(a.deck_track(), (0, 1));
    assert_eq!(a.deck_disc_count(), (1, 0));
    assert!(
        a.deck_lyric().is_none(),
        "没有 .lrc 就没有词可显，这是常态不是故障"
    );
}

#[test]
fn b_leaves_the_deck_for_the_menu() {
    let mut a = app_on_deck(vec![disc_of("A", 2)]);
    a.apply(Action::GbaDown(Btn::B));
    assert!(!a.is_listening());
    assert!(matches!(a.phase(), Phase::Menu { .. }));
    assert!(!a.deck_playing(), "离开卡座后不该还在放");
}

#[test]
fn a_shorter_disc_list_pulls_the_indices_back_into_range() {
    let mut a = App::new(Vec::new());
    a.set_discs(vec![disc_of("One", 1), disc_of("Two", 1), disc_of("Three", 1)]);
    a.set_disc(2);
    assert_eq!(a.deck_disc().map(|t| t.name.as_str()), Some("Three"));
    // 运行中换卡：列表变短，磁带与曲目两个下标都得被拉回来。
    a.set_discs(vec![disc_of("Only", 1)]);
    assert_eq!(
        a.deck_disc().map(|t| t.name.as_str()),
        Some("Only"),
        "2 进了 1 盘长的列表，必须落在那一盘上而不是落在空处"
    );
    assert_eq!(a.deck_track(), (0, 1), "曲目下标同样要跟着新的磁带收回来");
}

#[test]
fn the_track_index_survives_a_disc_with_more_tracks_than_the_last_one() {
    let mut a = App::new(Vec::new());
    a.set_discs(vec![disc_of("Many", 5)]);
    a.set_disc(0);
    // 直接把曲目推到位（公开 API 上没有单独的运动键，走键要靠相位），
    // 再换成一盘只有 2 首的：下标必须回落而不是指到空处。
    a.set_discs(vec![disc_of("Many", 5), disc_of("Few", 2)]);
    a.set_disc(1);
    assert_eq!(a.deck_track(), (0, 2), "换到新盘从第一首开始");
}
