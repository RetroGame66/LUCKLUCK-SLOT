//! `finished` 这个标志的时序：**用真 MP3**，因为要验的就是"解码线程自己停了"这件事，
//! 而假文件走不到那一步（`open` 会当场拒开，线程压根没起来）。
//!
//! 素材用 `_scratch` 下临时切出来的 0.35 秒静音 MP3（`ffmpeg` 若不在则整份跳过），
//! 不依赖 `F:/音乐相关` —— 那是本机素材，不该让测试在别的机器上红。

use slot::music::{Deck, Player};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 一段真的能解码的短音频，长度短到测试等得起。
fn tiny_mp3() -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../_scratch/2026-10-04-deckprobe/tiny.mp3");
    p.is_file().then_some(p)
}

#[test]
fn a_track_reports_itself_finished_and_play_clears_it() {
    let Some(path) = tiny_mp3() else {
        eprintln!("skip: 没有 tiny.mp3");
        return;
    };
    let deck = Arc::new(Deck::default());
    let mut player = Player::new(Arc::clone(&deck));
    player.open(path).expect("open");
    assert!(
        !deck.finished(),
        "刚 open 的轨不算播完 —— open 已在调用线程上清了标志，不靠新线程起跑"
    );

    player.play();
    // 等它真的走完。宽限 6 秒对 0.35 秒的音频是极大的余量。
    let t0 = Instant::now();
    while !deck.finished() && t0.elapsed() < Duration::from_secs(6) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        deck.finished(),
        "解码器跑完应把 finished 置真（等过 {:?}）",
        t0.elapsed()
    );
    assert!(
        !deck.playing(),
        "播完之后传输自己停住，不是还在放"
    );
    assert!(
        deck.elapsed() > 0.0,
        "停下的位置是这首的长度，不是 0"
    );

    // 按播放 = 从头再来。**这是没有 seek 时唯一的回头路**，也是标志必须被清掉的原因。
    player.play();
    assert!(
        !deck.finished(),
        "Play 必须清 finished，否则卡座会以为这一首也完了"
    );
}

/// `open` 会先停旧的线程再开新的，所以换轨之后**立刻**（不等新线程跑起来）读标志，
/// 拿到的必须是"没播完"。这一条是 `deck_advance` 不跳轨的前提。
#[test]
fn opening_another_track_clears_finished_before_the_thread_runs() {
    let Some(path) = tiny_mp3() else {
        eprintln!("skip: 没有 tiny.mp3");
        return;
    };
    let deck = Arc::new(Deck::default());
    let mut player = Player::new(Arc::clone(&deck));
    player.open(path.clone()).expect("first open");
    player.play();
    let t0 = Instant::now();
    while !deck.finished() && t0.elapsed() < Duration::from_secs(6) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(deck.finished(), "先让它真的播完");

    player.open(path).expect("reopen");
    assert!(
        !deck.finished(),
        "open 在调用线程上清标志：新线程还没起跑，这一帧读到的必须是干净的"
    );
}
