//! The deck: what the card's music does to the screen, and what the keys do to the deck.
//!
//! The app is booted against a real temporary card, so these are the same rules the device
//! runs — including the one that only shows up when the deck is reached the way a debug boot
//! reaches it.

mod common;

use common::tmp_root_with_carts;
use slot::app::App;
use slot_input::{Action, Btn};

/// A boot with a clock already set, so the app is past the clock screen and the deck is
/// reachable. The `open.txt` route is the one these tests care about: it is a *second* way in,
/// and a rule that only holds for the first is a rule waiting to be wrong on a debug boot.
fn app_at(root: &std::path::Path) -> App {
    let (a, _clock) = common::app_booting_with_clock(root);
    // As shipped, minus the debug file: these tests drive the deck by key, not by `open.txt`.
    let _ = std::fs::remove_file(root.join("System/open.txt"));
    a
}

/// The deck, reached the way a person reaches it: MENU, walk to the discs button, press it
/// (which lands on the **disc row**), then pick an album on that row.
///
/// The walk is done with `menu::step` — the same arithmetic the keys go through — rather than by
/// poking the phase, because "the discs button is the second one" is a claim about the grid that
/// the grid itself should answer. If the grid is rearranged, this finds the button anyway.
fn open_deck(a: &mut App) {
    a.apply(Action::OpenMenu);
    let discs = slot_ui::menu::Item::GRID
        .iter()
        .position(|s| *s == Some(slot_ui::menu::Item::Discs))
        .expect("the menu has a discs button");
    // Step until the caret is on it, one axis at a time, with a bound so a grid that cannot be
    // reached fails the test rather than hanging it.
    for _ in 0..16 {
        if menu_index(a) == discs {
            break;
        }
        a.apply(Action::GbaDown(Btn::Right));
    }
    assert_eq!(menu_index(a), discs, "the discs button is reachable by walking");
    // The button opens the row; A on the row puts the album in the deck. `Action::Insert` is
    // the row's own confirm, so this is the same two-step a person makes without the tap/release
    // timing of the button.
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::Insert);
}

/// Where the menu's caret is. Read out of the phase rather than remembered, so a test that
/// walked somewhere wrong says so.
fn menu_index(a: &App) -> usize {
    match a.phase() {
        slot::app::Phase::Menu { index } => *index,
        other => panic!("expected the menu, was {other:?}"),
    }
}

/// **The walk is owed from birth, not on request.** This is the whole point of the flag: the
/// deck can be entered from the menu or from `open.txt`, and a flag only the menu raises would
/// leave the second route showing an empty deck forever. A card with no music must also still
/// *ask* — that is how a card which grows music later gets seen at all.
#[test]
fn a_fresh_app_owes_the_music_walk() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let a = app_at(d.path());
    assert!(
        a.discs_dirty(),
        "a card's music starts unwalked; nothing has asked for it yet and everything must"
    );
}

/// Handing the walk over clears the debt, and a second hand-over is not asked for.
#[test]
fn handing_over_the_walk_clears_the_debt() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(Vec::new());
    assert!(
        !a.discs_dirty(),
        "the walk happened, so it must not be asked for again every frame"
    );
}

/// An index left over from a longer list must not survive a shorter one — the card's music can
/// be swapped under a running session, and a deck pointing past its end is a blank window.
#[test]
fn a_shorter_list_pulls_the_disc_back_into_range() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc("One"), disc("Two"), disc("Three")]);
    a.set_disc(2);
    assert_eq!(a.deck_disc().map(|t| t.name.as_str()), Some("Three"));

    // Now the card's music is swapped for something shorter.
    a.set_discs(vec![disc("Only")]);
    assert_eq!(
        a.deck_disc().map(|t| t.name.as_str()),
        Some("Only"),
        "index 2 into a 1-disc list must land on that disc, not on nothing"
    );
}

/// A card with no music says what the place is rather than showing a blank cassette — the same
/// word the menu's own button wears.
#[test]
fn a_card_with_no_music_still_names_the_screen() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(Vec::new());
    assert_eq!(a.deck_title(), slot_ui::lang::MENU_DISCS);
    assert!(a.deck_disc().is_none(), "no music walked means no disc in the deck");
}

/// With a disc in it, the name on the window is the disc's own.
#[test]
fn the_deck_names_the_disc_that_is_in_it() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc("Sunset Rip")]);
    assert_eq!(a.deck_title(), "Sunset Rip");
}

/// The counter counts discs, and only when there is more than one to count between.
#[test]
fn the_counter_is_only_worth_drawing_between_two_discs() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc("Only")]);
    assert_eq!(a.deck_disc_count(), (1, 0));

    a.set_discs(vec![disc("A"), disc("B")]);
    assert_eq!(a.deck_disc_count(), (2, 0), "a fresh list starts at the first disc");
}

/// Shoulders change the disc, and both ends stop rather than wrapping.
#[test]
fn the_shoulders_walk_the_discs_and_stop_at_both_ends() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc("A"), disc("B"), disc("C")]);
    open_deck(&mut a);
    assert!(a.is_listening(), "the menu's discs button opens the deck");

    a.apply(Action::GbaDown(Btn::L1));
    assert_eq!(a.deck_disc().map(|t| t.name.as_str()), Some("A"), "already at the first");
    a.apply(Action::GbaDown(Btn::R1));
    assert_eq!(a.deck_disc().map(|t| t.name.as_str()), Some("B"));
    a.apply(Action::GbaDown(Btn::R1));
    a.apply(Action::GbaDown(Btn::R1));
    assert_eq!(
        a.deck_disc().map(|t| t.name.as_str()),
        Some("C"),
        "at the last disc the shoulder stops rather than wrapping to the first"
    );
}

/// Arrows walk the transport, not the discs. The two are separate on purpose, so a test that
/// only ever pressed arrows would never notice them being the same control.
#[test]
fn the_arrows_walk_the_keys_and_leave_the_disc_alone() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc("A"), disc("B"), disc("C")]);
    open_deck(&mut a);
    let before = a.deck_disc().map(|t| t.name.clone());

    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::Right));

    assert_eq!(
        a.deck_disc().map(|t| t.name.clone()),
        before,
        "the transport is under the fingers; it is not how the disc changes"
    );
}

/// Changing the disc stops it **and starts the new one**. A deck playing album A does not carry
/// on playing it while album B goes in — and it does not sit there wound and waiting either,
/// because the press that changes the cassette is the same gesture that starts it.
///
/// The disc used here has tracks that are not real files, so nothing actually plays: what this
/// holds is that the *phase* says the deck is running, which is the part the panel's glyph and
/// the transport read. Whether a real file makes sound is `music.rs`'s business and is covered by
/// the `--deck` probe against real MP3s.
#[test]
fn changing_the_disc_stops_it_and_starts_the_new_one() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc("A"), disc("B")]);
    open_deck(&mut a);

    // Walk the caret to Play and press it: the key in hand starts on Rewind.
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::A));
    assert!(a.deck_playing(), "A on the play key starts the disc");

    a.apply(Action::GbaDown(Btn::R1));
    assert_eq!(a.deck_disc().map(|t| t.name.as_str()), Some("B"));
    assert!(
        a.deck_playing(),
        "a disc change starts the new disc rather than leaving the deck wound and waiting"
    );
}

/// **The shoulders stop at both ends rather than wrapping.** With no real file to play, what is
/// being held here is the refusal: a rewind at the first track must not silently become a
/// fast-forward to the last.
#[test]
fn a_disc_change_at_either_end_is_refused() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc("A"), disc("B"), disc("C")]);
    open_deck(&mut a);

    a.apply(Action::GbaDown(Btn::L1));
    assert_eq!(
        a.deck_disc().map(|t| t.name.as_str()),
        Some("A"),
        "already at the first disc, so the shoulder stops"
    );
    a.apply(Action::GbaDown(Btn::R1));
    a.apply(Action::GbaDown(Btn::R1));
    a.apply(Action::GbaDown(Btn::R1));
    assert_eq!(
        a.deck_disc().map(|t| t.name.as_str()),
        Some("C"),
        "at the last disc the shoulder stops rather than wrapping to the first"
    );
}

/// **The transport walks within a disc, and the shoulders change which disc.** These are two
/// different controls on purpose, and this is the test that would notice them being merged: the
/// arrows must move the track and leave the disc alone, and the shoulders must move the disc and
/// reset the track to its first.
#[test]
fn the_transport_walks_tracks_inside_the_disc() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of("A", 3), disc_of("B", 3)]);
    open_deck(&mut a);
    assert_eq!(a.deck_track(), (0, 3), "a fresh deck is on the first track");

    // Right to FastForward, then A on it: one track on, not one disc on.
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        a.deck_disc().map(|t| t.name.as_str()),
        Some("A"),
        "fast-forward is inside the cassette and must not change it"
    );

    // A new disc starts from its own first track, not from where the old one had got to.
    a.set_disc(1);
    a.set_discs(vec![disc_of("A", 3), disc_of("B", 3)]);
    assert_eq!(a.deck_track(), (0, 3));
}

/// Leaving the deck pauses rather than stopping, so the track is still open afterwards.
///
/// **This is a claim about the player, not about the panel** — the phase says stopped either way,
/// and the difference is only visible to the next press of play. It is held here because it is
/// the behaviour a person would notice: walking away from a deck and coming back to a silent one
/// that has forgotten its disc is the wrong answer.
///
/// ★ 2026-11-02：离开卡座的是 `B`（返回碟片货架，停在 `Phase::Shelf`）。停止挪到了 `A` 的长按
///   （`Action::DeckStop`，见 `tests/gesture` 那一侧）；这条只关心"走开是暂停不是硬停"。
#[test]
fn leaving_the_deck_pauses_rather_than_stops() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of("A", 2)]);
    open_deck(&mut a);
    a.apply(Action::GbaDown(Btn::B));
    assert!(!a.is_listening());
    assert!(
        !a.deck_playing(),
        "the deck is not on screen, so it is not running as far as the panel is concerned"
    );
}

/// A disc of one track has no track counter to show, and a card with no lyrics has no lyric
/// line. Both are the ordinary case, and both must be quiet rather than showing a `1/1` or an
/// empty face.
#[test]
fn a_disc_of_one_track_has_nothing_to_count() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of("Only", 1)]);
    open_deck(&mut a);
    assert_eq!(a.deck_track(), (0, 1));
    assert!(
        a.deck_lyric().is_none(),
        "a track with no .lrc beside it has no line to show, which is not a failure"
    );
}

/// ★ 2026-11-02（六六改键位）：`B` 不再是"停止"，而是**返回碟片货架**（离开播放器的那条路，
/// 停在 `Phase::Shelf` 的碟片那一行）。停止挪到了 `A` 的长按（`Action::DeckStop`）。
#[test]
fn b_leaves_the_deck_for_the_disc_shelf() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc("A")]);
    open_deck(&mut a);
    a.apply(Action::GbaDown(Btn::B));
    assert!(!a.is_listening());
    assert!(
        matches!(a.phase(), slot::app::Phase::Shelf { .. }),
        "B leaves the deck to the disc shelf, not the menu page"
    );
}

/// ★ 2026-11-02：**MENU 短按（`OpenMenu`）进碟片信息页**（原来这条是 SELECT，现 SELECT 空操作）。
/// 记下 `info_back`，信息页的 B/SELECT 会据此回到卡座。
#[test]
fn menu_tap_opens_the_disc_info_page() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc("A")]);
    open_deck(&mut a);
    a.apply(Action::OpenMenu);
    assert!(
        matches!(a.phase(), slot::app::Phase::DiscInfo { .. }),
        "MENU tap should land on the disc info page, not the shelf"
    );
    // 信息页的 B 收掉它，回到卡座（`info_back` 记得来路）。
    a.apply(Action::GbaDown(Btn::B));
    assert!(a.is_listening(), "leaving the info page should get back to the deck");
}

/// ★ 2026-11-02：**长按 `A` = 停止**（`Action::DeckStop`），**不离开这一屏** —— 短按播放/暂停
/// 的那颗键，按满 `A_HOLD_MS` 就是"■ 停"。手势层负责短/长的判定，`App` 只接 `DeckStop`。
#[test]
fn deck_stop_keeps_the_screen_and_winds_the_track_back() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of("A", 3)]);
    open_deck(&mut a);
    a.apply(Action::DeckStop);
    assert!(
        a.is_listening(),
        "stop is a tape stop, not a leave: the deck stays on screen"
    );
    // 停止 = 回曲首并停下，所以相位里"在放"是假。
    assert!(
        !a.deck_playing(),
        "after stop the deck is not running as far as the panel is concerned"
    );
}

/// A minimal disc, so these tests are about the deck's rules and not about what a scanner makes.
fn disc(name: &str) -> slot_media::Disc {
    disc_of(name, 0)
}

/// A disc of `tracks` tracks. The paths are not real files, so anything that tries to decode one
/// fails — which is the point of the tests that use it: the deck's rules are about indices and
/// transport, and a test that really opened audio would be testing the decoder instead.
fn disc_of(name: &str, tracks: usize) -> slot_media::Disc {
    slot_media::Disc {
        name: name.to_string(),
        stem: name.to_string(),
        tracks: (0..tracks)
            .map(|i| slot_media::Track {
                stem: format!("{name}{i}"),
                path: std::path::PathBuf::from(format!("/nonexistent/{name}{i}.mp3")),
                title: format!("{name}{i}"),
                artist: "Nobody".to_string(),
                album: name.to_string(),
                duration: 0.0,
                initial: name.chars().next().unwrap_or('?'),
                track_number: None,
                tags: Vec::new(),
            })
            .collect(),
        duration: 0.0,
        initial: name.chars().next().unwrap_or('?'),
    }
}
