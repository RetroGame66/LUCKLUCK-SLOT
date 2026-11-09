//! 随身听 × 合盖（2026-11-02 六六拍板两条）：① 放音乐时屏蔽超时关机 ② 唤醒回播放器。
//!
//! 全部用**真 MP3**：`finished`/`playing` 是解码线程自己写的标志，假文件（线程压根不
//! 起来）走不到那一步，测的就是"线程在走"这件事本身。素材在 `_scratch` 下现场切
//! （ffmpeg 不在则整份跳过，不依赖本机音乐库）。
//!
//! ⚠️ 宿主跑不了这个文件：`slot` 的测试二进制在本机 SIGSEGV（既有环境坑），
//! 与 `deck_finished.rs` 同路径，要在设备上跑 `bash _tools/_dev_test.sh deck_doze`。

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{app_booting_with_clock, panel, tmp_root_with_carts};
use slot::app::{App, Phase};
use slot::audio::Device;
use slot::music::Player;
use slot_input::{Action, Btn};

const SCRATCH: &str = "_scratch/2026-10-04-deckprobe";

/// 0.35 秒的真音频。短到测试等得起，长到解码线程真的跑过一轮。
fn tiny_mp3() -> Option<PathBuf> {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../{SCRATCH}/tiny.mp3"));
    p.is_file().then_some(p)
}

/// 30 秒的真音频。「在放不关机」要靠它：测试里等几秒的期间，曲子还在走，
/// `playing` 保持 true —— 用 0.35 秒的素材测这条，等完它就放完了。
fn long_mp3() -> Option<PathBuf> {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../{SCRATCH}/long.mp3"));
    p.is_file().then_some(p)
}

/// 一轨指向真音频的碟。`duration` 给 0.0：测试不依赖容器时长（`deck_seek` 的上界与
/// 本文件无关），而 `slot_media::Track::duration` 是扫描器的活。
fn track_of(path: PathBuf, name: String) -> slot_media::Track {
    slot_media::Track {
        stem: name.clone(),
        path,
        title: name.clone(),
        artist: "Nobody".to_string(),
        album: "Test".to_string(),
        track_number: None,
        duration: 0.0,
        initial: name.chars().next().unwrap_or('?'),
        tags: Vec::new(),
    }
}

fn disc_of(path: PathBuf, name: &str, tracks: usize) -> slot_media::Disc {
    slot_media::Disc {
        name: name.to_string(),
        stem: name.to_string(),
        tracks: (0..tracks).map(|i| track_of(path.clone(), format!("{name}{i}"))).collect(),
        duration: 0.0,
        initial: name.chars().next().unwrap_or('?'),
    }
}

/// 走 MENU → 碟片行 → 选碟，跟一个人按的一模一样（与 `tests/deck.rs` 的 `open_deck`
/// 同手法：`menu::step` 的算术，不直接写相位）。
fn open_deck(a: &mut App) {
    a.apply(Action::OpenMenu);
    let discs = slot_ui::menu::Item::GRID
        .iter()
        .position(|s| *s == Some(slot_ui::menu::Item::Discs))
        .expect("the menu has a discs button");
    for _ in 0..16 {
        if matches!(a.phase(), Phase::Menu { index } if *index == discs) {
            break;
        }
        a.apply(Action::GbaDown(Btn::Right));
    }
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::Insert);
}

/// 等解码线程把当前轨放完。宽限 6 秒对 0.35 秒的素材是极大的余量。
fn await_finished(a: &App) {
    let t0 = Instant::now();
    while !a.deck_finished() && t0.elapsed() < Duration::from_secs(6) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(a.deck_finished(), "the track should have finished (waited {:?})", t0.elapsed());
}

/// 已过时钟的 app，卡座可以走的 boot（`app_at` 的同款，`tests/deck.rs` 的私有版）。
fn app_at(root: &std::path::Path) -> App {
    let (a, _clock) = app_booting_with_clock(root);
    let _ = std::fs::remove_file(root.join("System/open.txt"));
    a
}

/// ★ 六六定的第二条：「回播放器」—— lid 合在随身听上，开盖回到随身听界面。
#[test]
fn a_lid_closed_over_a_running_deck_wakes_back_to_the_deck() {
    let Some(path) = tiny_mp3() else {
        eprintln!("skip: 没有 tiny.mp3");
        return;
    };
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of(path.clone(), "T", 1)]);
    open_deck(&mut a);
    assert!(a.is_listening(), "the deck is open");

    // 走带键走到 Play 按下去（键排焦点从 Rewind 起，按一次 Right 到 Play）。
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::A));
    assert!(a.deck_playing(), "A on the play key starts the disc");

    a.apply(Action::LidClose);
    assert!(matches!(a.phase(), Phase::Doze { deck: Some(_), .. }), "the doze carries the deck");
    a.apply(Action::LidOpen);

    let Phase::Listening { playing, disc, .. } = a.phase() else {
        panic!("waking from a deck doze must land back in the deck");
    };
    assert_eq!(*disc, 0, "same disc as before the lid");
    assert!(*playing, "the transport was running when the lid closed");
    assert!(a.deck_playing(), "the phase says what was asked, not what the thread has done yet");
}

/// ★ 六六定的第一条：「放音乐时屏蔽超时关机」。
///
/// 在放 ⇒ `doze_expired` 返回 false ⇒ 到点了也不 `begin_power_off`。判据是解码线程的
/// 实时 `playing`，不是相位快照：快照会永远挂着 true（歌曲自然放完时），那就等于
/// 永不关机，正是超时机制要防的东西。
#[test]
fn a_lid_closed_over_a_running_deck_never_times_out_while_it_plays() {
    let Some(path) = long_mp3() else {
        eprintln!("skip: 没有 long.mp3");
        return;
    };
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of(path, "L", 1)]);
    open_deck(&mut a);
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::A));

    // 2 秒超时。在放的 lid 合上 4 秒：超时早过了，机器不该停。
    a.set_power(panel(d.path(), Duration::from_secs(2)).0);
    a.apply(Action::LidClose);
    for _ in 0..240 {
        a.update(1.0 / 60.0);
    }
    assert!(!a.powering_off(), "4 s of lid over a playing deck must not power off");
}

/// 音乐**还在走**（不只是相位说在放）：合盖 1 秒后走带位置往前走了。
/// 这条钉住「合盖不暂停」—— 暂停是"走开"，合盖不是。
#[test]
fn a_lid_closed_over_a_running_deck_keeps_the_music_moving() {
    let Some(path) = long_mp3() else {
        eprintln!("skip: 没有 long.mp3");
        return;
    };
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of(path, "L", 1)]);
    open_deck(&mut a);
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::A));

    a.apply(Action::LidClose);
    std::thread::sleep(Duration::from_millis(500));
    let (elapsed, _) = a.deck_position();
    assert!(
        elapsed > 0.3,
        "half a second of lid should have a half second of track behind it, not {elapsed:.2} s"
    );
}

/// 停止/暂停时合盖：没有声音就没有"后台播放"这回事，超时照常生效。
/// （这条是"屏蔽"的边界 —— 不测它，第一条就成了"永不关机"。）
#[test]
fn a_lid_closed_over_a_stopped_deck_still_times_out() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of(PathBuf::from("/nonexistent/x0.mp3"), "S", 1)]);
    open_deck(&mut a);
    // open_disc_deck 载入并**停住**（第一版就是自己开始放，六六否了的那种）。

    a.set_power(panel(d.path(), Duration::from_secs(2)).0);
    a.apply(Action::LidClose);
    assert!(
        matches!(a.phase(), Phase::Doze { deck: Some((_, _, false)), .. }),
        "a stopped deck sleeps with the lid, but the doze still remembers it"
    );
    for _ in 0..240 {
        a.update(1.0 / 60.0);
    }
    assert!(a.powering_off(), "4 s of lid over a silent deck is the ordinary timeout");
}

/// ★ 后台曲终续播：合盖时专辑也要一曲接一曲（`deck_advance` 认 Doze 里的随身听）。
/// 没有这条，「后台播放」就是只有第一首。
#[test]
fn a_track_that_finishes_behind_the_lid_advances_and_wakes_into_the_next() {
    let Some(path) = tiny_mp3() else {
        eprintln!("skip: 没有 tiny.mp3");
        return;
    };
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of(path, "T", 2)]);
    open_deck(&mut a);
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(a.deck_track(), (0, 2));

    a.apply(Action::LidClose);
    // 第一首放完，解码线程自己 park 在末帧（`finished`），帧循环接力切轨。
    await_finished(&a);
    for _ in 0..5 {
        a.update(1.0 / 60.0);
    }
    assert_eq!(
        a.deck_track(),
        (1, 2),
        "the frame loop advances the album behind the lid, not only on screen"
    );

    a.apply(Action::LidOpen);
    let Phase::Listening { playing, disc, .. } = a.phase() else {
        panic!("waking from a deck doze must land back in the deck");
    };
    assert_eq!(*disc, 0, "still the same disc");
    assert!(*playing, "the second track is running when the lid opens");
}

/// 反向边界：**暂停**时合盖不续播。deck_advance 认的是"doze 里存着且当时在放"。
#[test]
fn a_paused_deck_does_not_advance_behind_the_lid() {
    let Some(path) = tiny_mp3() else {
        eprintln!("skip: 没有 tiny.mp3");
        return;
    };
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of(path, "T", 2)]);
    open_deck(&mut a);
    // 停在的 deck：不按 Play。

    a.apply(Action::LidClose);
    std::thread::sleep(Duration::from_millis(600)); // 够真音频放完一轮
    for _ in 0..5 {
        a.update(1.0 / 60.0);
    }
    assert_eq!(
        a.deck_track(),
        (0, 2),
        "a paused deck behind a lid must not start the next track on its own"
    );
}

/// ★ 2026-11-02 顺手修的 OR bug：暂停后屏上不能再显示"在放"。
///
/// `sync_phase_playing` 原写 `p || playing`（相位旧值取或）—— 暂停后相位存着的 true
/// 让图标继续显示播放三角。这条用假音频就够（transport 是用户意图，线程起不跑无所谓）。
#[test]
fn pressing_pause_on_a_running_deck_shows_the_paused_glyph() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_at(d.path());
    a.set_discs(vec![disc_of(PathBuf::from("/nonexistent/x0.mp3"), "P", 1)]);
    open_deck(&mut a);
    a.apply(Action::GbaDown(Btn::Right));
    a.apply(Action::GbaDown(Btn::A));
    assert!(a.deck_playing(), "A on the play key starts the disc");

    a.apply(Action::GbaDown(Btn::A));
    assert!(
        !a.deck_playing(),
        "the second A is pause: the panel must not still claim the disc is running"
    );
}

/// 合盖盖在**游戏**上不受影响：deck 是 None，wake 照旧回游戏（回归护栏）。
#[test]
fn a_lid_closed_over_a_game_does_not_invent_a_deck() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = common::app_playing_in(d.path(), "Emerald");
    a.apply(Action::LidClose);
    assert!(
        matches!(a.phase(), Phase::Doze { cart: Some(_), deck: None }),
        "a game doze carries the cart and nothing else"
    );
    a.apply(Action::LidOpen);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

/// 解码线程的挂法与 `tests/deck_finished.rs` 一致：真实 Player + 共享 Device 的
/// `attach_audio`（无 ALSA 的宿主上 `open` 失败、run 走 decode-only 分支，标志照常设置）。
#[test]
fn the_player_thread_writes_its_own_flags_without_a_device() {
    let Some(path) = tiny_mp3() else {
        eprintln!("skip: 没有 tiny.mp3");
        return;
    };
    let deck = Arc::new(slot::music::Deck::default());
    let mut player = Player::new(Arc::clone(&deck));
    player.attach_audio(Arc::new(Device::new()));
    player.open(path).expect("open");
    player.play();
    let t0 = Instant::now();
    while !deck.finished() && t0.elapsed() < Duration::from_secs(6) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(deck.finished(), "the decoder parks itself at the end of a track");
    assert!(!deck.playing(), "finished is not running");
}
