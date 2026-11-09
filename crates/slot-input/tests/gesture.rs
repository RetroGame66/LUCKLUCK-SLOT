use slot_input::RawEvent::{Down, Up};
use slot_input::{
    Action::*, Btn, Btn::*, Gestures, RawEvent, A_HOLD_MS, MENU_HOLD_MS, POWER_HOLD_MS,
    SELECT_CHORD_MS, SELECT_TAP_MS, VOLUME_REPEAT_DELAY_MS, VOLUME_REPEAT_MS,
};

#[test]
fn select_alone_reaches_the_game_after_the_chord_window() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty()); // deferred, not swallowed
    assert!(g.tick(SELECT_CHORD_MS - 1).is_empty());
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
}

#[test]
fn select_chord_swallows_select_entirely() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(R1), 50), vec![SaveState]);
    assert!(g.tick(500).is_empty()); // SELECT never reaches the game
    assert!(g.feed(Up(R1), 60).is_empty());
    assert!(g.feed(Up(Select), 70).is_empty());
}

#[test]
fn select_chords_map_to_all_four_axes() {
    for (btn, want) in [
        (Btn::Up, BrightnessUp),
        (Btn::Down, BrightnessDown),
        (Right, BlueLightUp),
        (Left, BlueLightDown),
    ] {
        let mut g = Gestures::new();
        g.feed(RawEvent::Down(Select), 0);
        assert_eq!(g.feed(RawEvent::Down(btn), 10), vec![want]);
    }
}

/// ★ 2026-11-03（六六）：L3 直接呼出亮度调节、R3 直接呼出蓝光调节，无需按 SELECT。松开只消费。
#[test]
fn l3_r3_summon_brightness_and_blue_light() {
    for (btn, want) in [(Btn::L3, BrightnessUp), (Btn::R3, BlueLightUp)] {
        let mut g = Gestures::new();
        assert_eq!(g.feed(RawEvent::Down(btn), 0), vec![want]);
        assert!(g.feed(RawEvent::Up(btn), 10).is_empty());
    }
}

#[test]
fn select_and_volume_keys_switch_the_audio_profile() {
    for (btn, want) in [
        (Btn::VolUp, AudioProfileNext),
        (Btn::VolDown, AudioProfilePrev),
    ] {
        let mut g = Gestures::new();
        g.feed(RawEvent::Down(Select), 0);
        assert_eq!(g.feed(RawEvent::Down(btn), 10), vec![want]);
    }
}

/// SELECT+START prints the device the other way round.
///
/// START is also the shelf's core picker on its own, which is what makes this worth holding at
/// the gesture layer rather than only at the app's: the chord has to swallow the press, or the
/// picker would open under the mode change and the shelf would come back with its lid off.
/// ★ 2026-10-20：这条是**货架的主题切换**，游戏里它本来就不响应，所以不动。
#[test]
fn select_and_start_toggle_the_mode() {
    let mut g = Gestures::new();
    g.feed(RawEvent::Down(Select), 0);
    assert_eq!(g.feed(RawEvent::Down(Btn::Start), 10), vec![ModeToggle]);
    // The release of the chorded key is swallowed with the press: a bare START is a gesture the
    // shelf answers, and half of one arriving after the chord would be a second gesture.
    assert!(g.feed(RawEvent::Up(Btn::Start), 60).is_empty());
    assert!(g.feed(RawEvent::Up(Select), 70).is_empty());
}

/// And with no SELECT down, START is the picker's again — the chord does not take the button.
#[test]
fn start_alone_is_still_the_game_button() {
    let mut g = Gestures::new();
    assert_eq!(
        g.feed(RawEvent::Down(Btn::Start), 0),
        vec![GbaDown(Btn::Start)]
    );
    assert_eq!(
        g.feed(RawEvent::Up(Btn::Start), 30),
        vec![GbaUp(Btn::Start)]
    );
}

/// ★ 2026-10-21：**同一条和弦，有卡的时候是游戏的。** 这是那句"只解开游戏中的关联"落到
/// 代码上的样子 —— 货架上 `SELECT+START` 仍然是主题（上面那条），游戏里它必须原封不动地
/// 到核心：START 按下/释放各一条 `GbaDown`/`GbaUp`，而 SELECT 只是被推迟，并不被吞掉，
/// 所以按住 SELECT 再按 START 在游戏看来就是"SELECT 和 START 都按了"。
///
/// 顺带钉住解绑的边界：`start_to_game` 只影响这一点，别的和弦一个不少。
#[test]
fn select_and_start_belong_to_the_game_while_a_cart_is_seated() {
    let mut g = Gestures::new();
    g.set_start_to_game(true);
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Start), 10), vec![GbaDown(Start)]);
    // The chord window still runs out, and SELECT goes to the core as well.
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert_eq!(g.feed(Up(Start), 200), vec![GbaUp(Start)]);
    assert_eq!(g.feed(Up(Select), 210), vec![GbaUp(Select)]);
    // Every other chord is untouched by the switch.
    let mut g = Gestures::new();
    g.set_start_to_game(true);
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(R1), 10), vec![SaveState]);
    assert_eq!(g.feed(Down(X), 20), vec![MaskCycle]);
}

#[test]
fn select_released_inside_the_window_still_reaches_the_game() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Up(Select), 40), vec![GbaDown(Select)]);
    assert_eq!(g.tick(40 + SELECT_TAP_MS), vec![GbaUp(Select)]);
    assert!(g.tick(1_000).is_empty());
}

/// A tap of MENU opens the about screen, on the release. The other two MENU gestures are
/// unchanged: a tap was the one press this button did not already mean something by.
#[test]
fn menu_single_tap_opens_the_about_screen() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Menu), 0).is_empty(), "acted on the press");
    assert_eq!(g.feed(Up(Menu), 100), vec![OpenMenu]);
    assert!(g.tick(451).is_empty(), "fired a second time on the timer");
}

/// The hold is an eject and nothing else **at the app's end** — but the gesture layer cannot
/// see the screen, so since 2026-10-06 one hold emits two actions, `Eject` + `MenuHold`, and
/// each screen takes the one it means (the shelf ejects, the tape deck leaves the player).
/// What it is still not is also a tap: letting go of one must not drop the about screen over
/// the shelf the cart just came back to.
#[test]
fn a_menu_hold_is_not_also_a_tap() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.tick(MENU_HOLD_MS), vec![Eject, MenuHold]);
    assert!(g.feed(Up(Menu), MENU_HOLD_MS + 50).is_empty());
}

#[test]
fn menu_double_tap_opens_polaroids_immediately() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), 100);
    assert_eq!(g.feed(Down(Menu), 300), vec![Polaroids]);
}

#[test]
fn menu_hold_ejects_at_the_hold_time_and_not_before() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert!(g.tick(MENU_HOLD_MS - 1).is_empty());
    assert_eq!(g.tick(MENU_HOLD_MS), vec![Eject, MenuHold]);
}

#[test]
fn menu_hold_released_early_ejects_nothing() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), MENU_HOLD_MS - 200);
    assert!(g.tick(3000).is_empty());
}

#[test]
fn r2_hold_is_momentary() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(R2), 0), vec![FfStart]);
    assert_eq!(g.feed(Up(R2), 900), vec![FfStop]);
}

#[test]
fn r2_double_tap_latches_and_single_press_clears() {
    let mut g = Gestures::new();
    g.feed(Down(R2), 0);
    g.feed(Up(R2), 50); // tap 1
    g.feed(Down(R2), 100);
    assert!(g.feed(Up(R2), 150).is_empty()); // latched, FF stays on
    g.feed(Down(R2), 5000);
    assert_eq!(g.feed(Up(R2), 5050), vec![FfStop]);
}

/// The flush hangs off the press, because a button being held may be cut by the PMIC before
/// there is any release to see. The lock waits for the release, so that a press on its way
/// to becoming a hold does not darken the panel on the way through.
#[test]
fn power_flushes_on_the_press_and_locks_on_the_release() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(Btn::Power), 0), vec![PowerPress]);
    assert_eq!(g.feed(Up(Btn::Power), 80), vec![PowerTap]);
}

/// The hold arms while the button is still down and the release commits, so the shutdown
/// screen is on the panel for as long as the user holds rather than flashing past.
#[test]
fn power_held_past_the_threshold_raises_the_menu_and_the_release_does_nothing() {
    let mut g = Gestures::new();
    g.feed(Down(Btn::Power), 0);
    assert!(g.tick(POWER_HOLD_MS - 1).is_empty());
    assert_eq!(g.tick(POWER_HOLD_MS), vec![PowerHold]);
    assert!(g.tick(4000).is_empty(), "the hold fires once, not per tick");
    assert_eq!(g.feed(Up(Btn::Power), 4500), vec![PowerOff]);
}

/// And a release that never reached the threshold is a lock, never a shutdown.
#[test]
fn a_press_just_short_of_the_threshold_is_a_lock() {
    let mut g = Gestures::new();
    g.feed(Down(Btn::Power), 0);
    assert!(g.tick(POWER_HOLD_MS - 1).is_empty());
    assert_eq!(g.feed(Up(Btn::Power), POWER_HOLD_MS - 1), vec![PowerTap]);
}

#[test]
fn rewind_beats_latched_fast_forward() {
    let mut g = Gestures::new();
    g.feed(Down(R2), 0);
    g.feed(Up(R2), 50);
    g.feed(Down(R2), 100);
    g.feed(Up(R2), 150); // latched
    assert_eq!(g.feed(Down(L2), 200), vec![FfStop, RewindStart]);
}

/// 120 ms was not enough time to land the second key of a chord, so SELECT reached the game
/// and opened a menu mid press. A held SELECT can wait much longer: the only reason to ever
/// give up on the chord is a game that wants SELECT held down.
#[test]
fn a_held_select_waits_much_longer_than_it_used_to() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert!(g.tick(400).is_empty(), "gave up on the chord at 400 ms");
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
}

#[test]
fn a_chord_landing_late_is_still_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.tick(400);
    assert_eq!(g.feed(Down(R1), 400), vec![SaveState]);
    assert!(
        g.tick(5_000).is_empty(),
        "SELECT leaked to the game after a late chord"
    );
}

/// The other half: releasing SELECT settles the question, so a tap should cost the game no
/// latency at all rather than waiting out a window that can no longer produce a chord.
#[test]
fn a_released_select_reaches_the_game_immediately() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Up(Select), 80), vec![GbaDown(Select)]);
}

/// Down and up in one batch net out to nothing: the mask is set and cleared before the core
/// ever reads it, so the press is invisible to the game.
#[test]
fn a_select_tap_is_held_long_enough_for_the_core_to_see_it() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    let on_release = g.feed(Up(Select), 80);
    assert_eq!(on_release, vec![GbaDown(Select)]);
    assert!(
        !on_release.contains(&GbaUp(Select)),
        "press and release in the same frame"
    );
    assert!(
        g.tick(100).is_empty(),
        "released before the core could poll it"
    );
    assert_eq!(g.tick(80 + SELECT_TAP_MS), vec![GbaUp(Select)]);
}

#[test]
fn both_volume_keys_together_mute_once() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    let out = g.feed(Down(VolDown), 80);
    assert!(out.contains(&MuteToggle), "the pair did not mute");
    assert!(
        g.tick(400).is_empty(),
        "it kept firing while both were held"
    );
}

#[test]
fn volume_keys_far_apart_are_not_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    let out = g.feed(Down(VolDown), 400);
    assert!(!out.contains(&MuteToggle), "two separate presses muted");
}

/// The app rolls the chord's own two presses back, so it has to see them before it sees the
/// chord. Reversed, the second press would move the level after the mute remembered it.
#[test]
fn the_chord_arrives_behind_the_press_that_completed_it() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    assert_eq!(g.feed(Down(VolDown), 80), vec![VolumeDown, MuteToggle]);
}

/// Unmuting is the same gesture, so a pair that never rearmed would be a one way trip.
#[test]
fn releasing_both_rearms_the_chord() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    g.feed(Down(VolDown), 80);
    g.feed(Up(VolUp), 200);
    g.feed(Up(VolDown), 220);
    g.feed(Down(VolUp), 1_000);
    assert!(g.feed(Down(VolDown), 1_050).contains(&MuteToggle));
}

/// A held volume key ramps. One step per press would mean tapping a dozen times to cross the
/// range, and the press already records when it went down for exactly this.
#[test]
fn a_held_volume_key_repeats() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(VolUp), 0), vec![VolumeUp]);
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS - 1).is_empty(),
        "it repeated before the ramp was due"
    );
    assert_eq!(g.tick(VOLUME_REPEAT_DELAY_MS), vec![VolumeUp]);
    assert!(g.tick(VOLUME_REPEAT_DELAY_MS + 1).is_empty());
    assert_eq!(
        g.tick(VOLUME_REPEAT_DELAY_MS + VOLUME_REPEAT_MS),
        vec![VolumeUp]
    );
}

#[test]
fn a_released_volume_key_stops_repeating() {
    let mut g = Gestures::new();
    g.feed(Down(VolDown), 0);
    assert_eq!(g.tick(VOLUME_REPEAT_DELAY_MS), vec![VolumeDown]);
    assert!(g.feed(Up(VolDown), VOLUME_REPEAT_DELAY_MS + 10).is_empty());
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS * 4).is_empty(),
        "a key nobody is holding kept ramping"
    );
}

/// Both keys held is the mute chord, not two levels moving at once. The ramp would fight the
/// mute it just fired and leave the level somewhere nobody asked for.
#[test]
fn the_mute_chord_does_not_ramp() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    assert!(g.feed(Down(VolDown), 50).contains(&MuteToggle));
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS * 3).is_empty(),
        "the mute chord ramped the volume while it was held"
    );
}

/// A second press after the ramp starts from the top again, rather than inheriting the pace
/// of the press before it.
#[test]
fn each_press_starts_its_own_ramp() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    g.tick(VOLUME_REPEAT_DELAY_MS);
    g.feed(Up(VolUp), VOLUME_REPEAT_DELAY_MS + 5);
    assert_eq!(g.feed(Down(VolUp), 5_000), vec![VolumeUp]);
    assert!(
        g.tick(5_000 + VOLUME_REPEAT_DELAY_MS - 1).is_empty(),
        "the new press repeated early"
    );
    assert_eq!(g.tick(5_000 + VOLUME_REPEAT_DELAY_MS), vec![VolumeUp]);
}

/// X on its own is the game's X. A chord key is only a chord while SELECT is down.
#[test]
fn x_without_select_is_still_the_games_x() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(X), 0), vec![GbaDown(Btn::X)]);
    assert_eq!(g.feed(Up(X), 40), vec![GbaUp(Btn::X)]);
}

/// SELECT+MENU, which is what opens the in-game menu. One gesture, not two: it fires on the
/// MENU press and the release says nothing at all.
#[test]
fn select_and_menu_open_the_in_game_menu() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Menu), 50), vec![GameMenu]);
    assert!(
        g.feed(Up(Menu), 150).is_empty(),
        "the release landed as a second gesture on top of the menu"
    );
    assert!(
        g.feed(Up(Select), 200).is_empty(),
        "SELECT reached the game behind the chord that consumed it"
    );
}

/// The chorded press must not also arm the eject hold, or reading the menu with the buttons
/// still down would eject the cart out from under it.
#[test]
fn a_chorded_menu_never_arms_the_eject_hold() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Menu), 50), vec![GameMenu]);
    assert!(
        g.tick(50 + MENU_HOLD_MS + 1).is_empty(),
        "holding the menu open ejected the cart"
    );
}

/// The whole test is the ordering inside `menu_down`. Behind the double tap check, a
/// SELECT+MENU that follows a recent MENU tap opens the switcher instead of the menu.
#[test]
fn a_chord_after_a_recent_menu_tap_is_still_the_menu() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Up(Menu), 100), vec![OpenMenu]);
    g.feed(Down(Select), 150);
    assert_eq!(
        g.feed(Down(Menu), 200),
        vec![GameMenu],
        "a recent tap turned the chord into the switcher"
    );
}

/// The difference between a chord and a trap. SELECT commits to being the game's after
/// `SELECT_CHORD_MS`, and MENU under a SELECT the game already has is an ordinary MENU.
#[test]
fn menu_under_a_select_the_game_already_has_is_not_the_menu() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert!(
        g.feed(Down(Menu), 700).is_empty(),
        "a SELECT the game already owns still chorded"
    );
    assert_eq!(g.feed(Up(Menu), 800), vec![OpenMenu]);
}

/// The chord clears both of MENU's own windows, so the press after it has to start its own.
///
/// It has to begin with a real tap, or the half of that which matters is invisible: the tap
/// leaves a double tap window open, the chord lands inside it, and a chord that does not
/// close that window leaves the very next MENU tap opening the switcher instead of the about
/// screen — a press whose meaning depends on a chord two presses ago.
#[test]
fn the_menu_button_still_works_after_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Up(Menu), 100), vec![OpenMenu]);
    g.feed(Down(Select), 150);
    assert_eq!(g.feed(Down(Menu), 200), vec![GameMenu]);
    assert!(g.feed(Up(Menu), 250).is_empty());
    assert!(g.feed(Up(Select), 260).is_empty());
    assert!(
        g.feed(Down(Menu), 400).is_empty(),
        "the tap before the chord was still standing as half of a double tap"
    );
    assert_eq!(
        g.feed(Up(Menu), 450),
        vec![OpenMenu],
        "the chord left the menu button dead"
    );
}

/// SELECT+Y 是一条**只有松手才落定**的和弦：按下不消费，松手发一次 `ColorCycle`。
///
/// 长按曾经开色板浏览器（2026-10-26 六六取消，浏览器改从游戏菜单进），
/// 所以现在的规则很朴素：**按住多久都只算一次 `ColorCycle`，中途不再发任何动作**。
/// 这一整块就是"没有中途"这个不变式的证据：无论 hold 多久、tick 多少次、
/// SELECT 谁先放，输出序列都是恰好一次 `ColorCycle`，且这条按下不泄漏给游戏。

#[test]
fn a_select_y_tap_walks_the_palette_page() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert!(g.feed(Down(Y), 10).is_empty());
    assert_eq!(
        g.feed(Up(Y), 60),
        vec![ColorCycle],
        "松手发一次 walk，因为按下没消费（这一按仍可能长成别的和弦）"
    );
    assert!(g.feed(Up(Select), 70).is_empty());
}

/// 按住很久也不开任何面板：`tick` 一路空输出，只到 Y 松手那一下才发 `ColorCycle`。
/// 这也是"没有 hold 中途"的直接断言 —— 若某一刻 `tick` 里又冒出来动作，就是它回归了。
#[test]
fn a_select_y_hold_also_would_only_walk_once() {
    let mut g = Gestures::new();
    let mut out = Vec::new();
    out.extend(g.feed(Down(Select), 0));
    out.extend(g.feed(Down(Y), 10));
    // 按住一路 tick 上千毫秒，中间不应有 `ColorCycle`，更不应有任何面板级动作。
    for t in 60..2000 {
        out.extend(g.tick(t));
    }
    assert!(out.is_empty(), "按住期间漏出了动作：{out:?}");
    out.extend(g.feed(Up(Y), 2100));
    out.extend(g.feed(Up(Select), 2110));
    assert_eq!(out, vec![ColorCycle], "整段按住只应换来松手那一次 walk：{out:?}");
}

#[test]
fn a_select_released_under_a_held_y_owes_nothing() {
    // SELECT 先松，Y 还在按：松手序列仍应恰好是一次 `ColorCycle`。
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.feed(Down(Y), 10);
    assert!(g.tick(1000).is_empty(), "按住期间就发了动作");
    let mut out = g.feed(Up(Select), 1100);
    out.extend(g.feed(Up(Y), 1500));
    assert_eq!(out, vec![ColorCycle], "SELECT 先松仍应只换一次 walk：{out:?}");
}

/// And with no SELECT down, Y is the game's again — the chord does not take the button.
#[test]
fn y_without_select_is_still_the_games_y() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(Y), 0), vec![GbaDown(Y)]);
    assert_eq!(g.feed(Up(Y), 40), vec![GbaUp(Y)]);
    // ...and holding it is not a hold, because there is no chord to be the hold half of.
    assert!(g.tick(3000).is_empty());
    assert_eq!(g.feed(Down(Y), 2000), vec![GbaDown(Y)]);
    assert_eq!(g.feed(Up(Y), 4000), vec![GbaUp(Y)]);
}

/// The chord leaves nothing behind for the next press to find.
///
/// The menu button needed exactly this test and for the same reason: the deferred press is the
/// one place a chord keeps state across two edges, and state that is not cleared on the way out
/// changes the meaning of a press that comes much later.
#[test]
fn the_y_button_still_works_after_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.feed(Down(Y), 10);
    // 按住一阵（模拟长按），但期间不产生任何动作。
    g.tick(1000);
    g.feed(Up(Y), 1050);
    g.feed(Up(Select), 1060);
    assert_eq!(
        g.feed(Down(Y), 5000),
        vec![GbaDown(Y)],
        "the bare Y after the chord was not the game's"
    );
    assert_eq!(g.feed(Up(Y), 5040), vec![GbaUp(Y)]);
}

// ── PS1: the shoulders belong to the game ────────────────────────────────────────────────────
//
// ★ 2026-10-25. `L2`/`R2` are rewind and fast forward on every machine — except a PlayStation,
// where they are real pad buttons. `set_l2r2_to_game` moves a **bare** press to the core and
// leaves `SELECT+L2/R2` as the transport, and these two tests are the whole of that contract:
// one that the trade happens, one that it does not quietly eat the transport.

/// On PS1 a bare press is the game's, in both directions — and neither one may leave fast
/// forward or rewind running, because that press was never the transport's.
#[test]
fn a_bare_shoulder_is_the_games_when_the_machine_asks_for_it() {
    for b in [L2, R2] {
        let mut g = Gestures::new();
        g.set_l2r2_to_game(true);
        assert_eq!(
            g.feed(Down(b), 0),
            vec![GbaDown(b)],
            "a bare {b:?} on PS1 was not handed to the game"
        );
        // No transport leaked out of it: the tick has nothing to say, and the release is the
        // core's up edge rather than an `FfStop`/`RewindStop` for something that never started.
        assert!(g.tick(5_000).is_empty(), "{b:?} started a transport");
        assert!(!g.ff_latched(), "{b:?} latched fast forward");
        assert_eq!(g.feed(Up(b), 5000), vec![GbaUp(b)]);
    }
}

/// The other half of the trade: **`SELECT` still owns the shoulders, on PS1 too.** The chord
/// window is not a tap — a SELECT held far past it is still "SELECT is down", and the shoulder
/// pressed under it must stay the transport, or a PlayStation loses rewind altogether.
#[test]
fn select_and_a_shoulder_is_still_the_transport_on_ps1() {
    // L2 → rewind, R2 → fast forward, one case each.
    for (b, start, stop) in [(L2, RewindStart, RewindStop), (R2, FfStart, FfStop)] {
        let mut g = Gestures::new();
        g.set_l2r2_to_game(true);
        g.feed(Down(Select), 0);
        assert_eq!(
            g.feed(Down(b), 50),
            vec![start],
            "SELECT+{b:?} stopped being the transport on PS1"
        );
        assert_eq!(g.feed(Up(b), 100), vec![stop], "the transport did not stop");
        // And the release of the shoulder must not reach the core either — it was a chord.
        assert!(g.feed(Up(Select), 150).is_empty());
    }
}

/// Past the chord window the chord is gone but SELECT is not: `Select::Delivered` means the
/// game has SELECT, and a shoulder pressed then is still a two-key gesture. This is the case
/// the first draft got wrong — the transport check was reading the chord window alone.
#[test]
fn a_shoulder_after_the_chord_window_is_still_the_transport_on_ps1() {
    let mut g = Gestures::new();
    g.set_l2r2_to_game(true);
    g.feed(Down(Select), 0);
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert_eq!(
        g.feed(Down(R2), SELECT_CHORD_MS + 1_000),
        vec![FfStart],
        "a long-held SELECT gave the shoulder away to the game"
    );
}

/// Without the flag nothing changes at all — this is the GBA/GB/GBC/SFC/FC/MD path, and it is
/// the one every machine but a PlayStation has always taken.
#[test]
fn a_bare_shoulder_is_the_transport_when_the_machine_does_not_ask() {
    for (b, start, stop) in [(L2, RewindStart, RewindStop), (R2, FfStart, FfStop)] {
        let mut g = Gestures::new();
        assert_eq!(g.feed(Down(b), 0), vec![start]);
        assert_eq!(g.feed(Up(b), 50), vec![stop]);
    }
}

// ── 卡座上 `A` 的短按 / 长按（★ 2026-11-02 六六） ─────────────────────────────────────────
//
// `set_a_to_deck` 是手势层第一次看见"现在这块屏是不是播放器"。为假（游戏 / 货架 / 菜单，
// 也就是默认）时裸 `A` 就是游戏的，按下松手各一拍，一行不改；为真时**按下压住不发**，
// 松手判成短按（补发一次 `GbaDown(A)`，app 读作播放/暂停），或按满 `A_HOLD_MS` 判成长按
// （发 `DeckStop`，app 读作停止），长按的松手就不再欠那个短按。

/// The default reading: not the deck, so a bare `A` is the game's — down and up, no deferral.
#[test]
fn a_bare_a_is_the_games_when_not_on_the_deck() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(A), 0), vec![GbaDown(A)]);
    assert_eq!(g.feed(Up(A), 40), vec![GbaUp(A)]);
    assert!(
        g.tick(3_000).is_empty(),
        "no deck, no A hold, so nothing is owed on the timer"
    );
}

/// On the deck a quick `A` says nothing on the press — it is held for the short/long call
/// — and the release is what owes the play/pause down.
#[test]
fn a_tap_of_a_on_the_deck_defers_the_down_to_the_release() {
    let mut g = Gestures::new();
    g.set_a_to_deck(true);
    assert!(
        g.feed(Down(A), 0).is_empty(),
        "acted on the press; the call is not made yet"
    );
    assert!(g.tick(A_HOLD_MS - 1).is_empty(), "fired the stop early");
    assert_eq!(
        g.feed(Up(A), 400),
        vec![GbaDown(A)],
        "the tap is the play/pause, delivered on the release"
    );
    assert!(g.tick(500).is_empty(), "owed nothing after a tap that paid off");
}

/// Held past the threshold the deck stops rather than pauses: the stop fires at the hold,
/// on the timer, and the release of a hold does not also deliver the tap.
#[test]
fn a_hold_of_a_on_the_deck_is_the_stop() {
    let mut g = Gestures::new();
    g.set_a_to_deck(true);
    assert!(g.feed(Down(A), 0).is_empty());
    assert!(g.tick(A_HOLD_MS - 1).is_empty(), "stopped a millisecond early");
    assert_eq!(g.tick(A_HOLD_MS), vec![DeckStop], "the threshold is the stop");
    assert!(
        g.tick(A_HOLD_MS + 500).is_empty(),
        "the stop fires once, not per tick"
    );
    assert_eq!(
        g.feed(Up(A), A_HOLD_MS + 500),
        Vec::new(),
        "a hold that already stopped is not also a play/pause on the release"
    );
}

/// The hold and the tap share one state machine, so a tap after a hold still taps: whatever
/// the hold left behind must not make the next press into a stop.
#[test]
fn a_tap_after_a_hold_on_the_deck_still_taps() {
    let mut g = Gestures::new();
    g.set_a_to_deck(true);
    // A hold, cleared.
    g.feed(Down(A), 0);
    assert_eq!(g.tick(A_HOLD_MS), vec![DeckStop]);
    assert!(g.feed(Up(A), A_HOLD_MS + 100).is_empty());
    // And a tap on the next press.
    assert!(g.feed(Down(A), 2_000).is_empty());
    assert_eq!(
        g.feed(Up(A), 2_500),
        vec![GbaDown(A)],
        "the hold's state leaked into the next tap"
    );
}

/// The deck flag is a per-frame fact: the same key is the game's the frame the deck is gone,
/// so leaving the deck mid-press hands the press back to the core rather than deferring it.
#[test]
fn taking_a_off_the_deck_makes_a_the_games_again() {
    let mut g = Gestures::new();
    g.set_a_to_deck(true);
    assert!(g.feed(Down(A), 0).is_empty());
    // The deck left the screen before the release: the press is the game's now.
    g.set_a_to_deck(false);
    assert_eq!(g.feed(Up(A), 400), vec![GbaDown(A)]);
}
