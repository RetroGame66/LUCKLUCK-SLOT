//! Playing the card's music.
//!
//! The decoder is `slot-media`'s; this is what makes it audible. It is the same chain a game
//! takes — resample, volume, the ring, the sink — because that chain is already tuned for this
//! device and a second one would only be a second set of clicks to find.
//!
//! What is here so far is the audio path on its own, reachable as `slot --play <file> [secs]`.
//! Nothing is drawn: it exists to prove the device can decode and make a sound with no screen
//! in the way, which is the part a card's music cannot be tested for any other way.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI16, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::audio::{open_sink, volume, Device, Profile};
use crate::resample::Resampler;

/// How loud a track plays when nothing has said otherwise, on the sink's own 0-100 scale. The
/// same number the shelf boots with, so the first track is not a surprise.
pub const DEFAULT_VOLUME: u8 = 80;

/// Play `path` for at most `seconds`, then return.
///
/// A sink that will not open is **not** a reason to stop: the device's PCM is exclusive, so a
/// second `slot` started while the frontend is up is told the device is busy, and the decode is
/// the half of this that cannot be tested any other way. It decodes regardless and says which
/// half it got.
///
/// Everything it learns goes to stderr, because the device has no console and the interesting
/// numbers — the file's rate, the device's, the overrun count — are the whole reason to run it.
pub fn smoke(path: &Path, seconds: f32) -> Result<(), String> {
    let mut source = slot_media::Source::open(path)
        .ok_or_else(|| format!("would not open: {}", path.display()))?;
    let file_hz = source.rate();

    // Asked for the file's own rate, which is the whole point of opening here rather than at
    // the GBA's 32768: a 48 kHz track played at its own rate needs no resampling at all, and
    // the device's own `plug:` layer is what converts if the codec will not take it.
    let mut sink = open_sink();
    let opened = sink.open(file_hz, Profile::Stable).is_ok();
    let ring = sink.ring();
    // What the device actually opened at, which is what the resampler has to land on — the
    // rate asked for is a preference and the sink is allowed to say no.
    let device_hz = match ring.sample_rate() {
        0 => file_hz,
        r => r,
    };
    let mut resampler = Resampler::new(file_hz as f64, device_hz as f64);

    eprintln!(
        "slot: music: {} — file {file_hz} Hz, {} ch, sink {} (device {device_hz} Hz)",
        path.display(),
        source.channels(),
        if opened { "open" } else { "BUSY — decoding only" },
    );

    let deadline = Instant::now() + Duration::from_secs_f32(seconds);
    let mut frames = 0u64;
    let mut peak = 0u16;
    let mut out: Vec<i16> = Vec::new();
    while Instant::now() < deadline {
        let Some(block) = source.next() else {
            eprintln!("slot: music: end of file");
            break;
        };
        peak = block
            .iter()
            .fold(peak, |m, s| m.max(s.unsigned_abs()));
        if opened {
            resampler.process(block, &mut out);
            volume::apply(&mut out, DEFAULT_VOLUME);
            // A ring with no device behind it has no capacity, and `push_blocking` on one
            // returns immediately — which is what lets this loop keep decoding with no sink.
            ring.push_blocking(&out);
        }
        frames += (block.len() / 2) as u64;
    }

    let played = frames as f64 / file_hz as f64;
    eprintln!(
        "slot: music: {played:.1} s decoded, peak {peak}, overruns {}, underruns {}",
        ring.overruns(),
        ring.underruns()
    );
    if opened {
        // The sink's thread drains what is left of the ring. Dropping it this instant would
        // cut the tail off, which sounds exactly like a bug that is not there.
        std::thread::sleep(Duration::from_millis(400));
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// The deck: a track that plays while the rest of the program keeps drawing.
//
// `smoke` above is a straight line — decode, push, return — which is the whole of what a
// bring-up test needs and exactly the wrong shape for a music player. A track has to keep
// going while the shelf is on screen, so the decode moves onto its own thread and everything
// the frame wants to know about it is read through atomics rather than asked for.
// ══════════════════════════════════════════════════════════════════════════════

/// How many mono samples the visualiser can look back over. 2048 at 48 kHz is 43 ms, which
/// is the shortest window that still resolves the bottom band of a log spectrum (80 Hz needs
/// 12.5 ms, and Goertzel wants more than the bare minimum to be steady).
pub const TAP_LEN: usize = 2048;

/// What the frame reads about what is playing.
///
/// Every field is atomic because the decode thread writes them and the frame reads them, and
/// neither is ever allowed to wait for the other: a frame that blocked on the decoder would
/// hitch every time a packet was slow, and a decoder that blocked on the frame would click.
///
/// This is the same shape `emu.rs`'s `Shared` already uses, so the deck is not a second way
/// to do the same thing.
pub struct Deck {
    playing: AtomicBool,
    /// Frames handed to the sink since the track opened.
    position: AtomicU64,
    /// The file's own rate, read off the decoder. 0 until a track is open.
    rate: AtomicU32,
    /// The decoder has run out of track and the deck is parked on the last frame.
    ///
    /// **A separate fact from `playing`, and it has to be one.** The thread stops itself at the
    /// end rather than tearing down, so `playing` goes false for two reasons that mean opposite
    /// things to a deck: the user pressed pause, or the song ended. Only this says which. Without
    /// it an album plays its first track and then sits there mute, because "not playing" is all
    /// the frame can see.
    ///
    /// Cleared by `Play`, so pressing play at the end of a track replays it from the top —
    /// which is what a deck does, and is the only way back after a seek-less source runs out.
    finished: AtomicBool,
    /// 0..100, on the sink's own scale. Read by the loop before each block, so a volume
    /// change takes effect at the next packet rather than at the next reopen.
    volume: std::sync::atomic::AtomicU8,
    /// The visualiser's tap: the last `TAP_LEN` mono samples, written one per decoded frame.
    ///
    /// Plain atomics rather than a lock, and deliberately racy in the same way the reference
    /// player's is: the frame may read a tap the thread is halfway through rewriting, which
    /// shows up as one sample slightly out of place in a spectrum. A lock would cost more
    /// than the artefact is visible.
    tap: Box<[AtomicI16]>,
    tap_w: AtomicUsize,
    /// The equaliser preset, **one of 18** (the Winamp set VLC ships — see `slot_ui::eq`).
    ///
    /// Set by the frame the moment the user cycles it, read by the decode thread once per
    /// block and applied through a biquad cascade. `NULL` is the honest default: no EQ is a
    /// preset here, not a special case, so a deck that has never been touched pays nothing.
    /// Stored as a `u8` because the deck's fields are all atomic and the frame writes one
    /// without ever meeting the thread.
    eq_preset: std::sync::atomic::AtomicU8,
}

impl Default for Deck {
    fn default() -> Self {
        // `AtomicI16` is not `Clone`, so a `vec![..; N]` cannot build this; `resize_with`
        // takes a closure rather than `AtomicI16::new`, because `new` wants its value.
        let mut tap = Vec::with_capacity(TAP_LEN);
        tap.resize_with(TAP_LEN, || AtomicI16::new(0));
        Deck {
            playing: AtomicBool::new(false),
            position: AtomicU64::new(0),
            rate: AtomicU32::new(0),
            finished: AtomicBool::new(false),
            volume: std::sync::atomic::AtomicU8::new(DEFAULT_VOLUME),
            tap: tap.into_boxed_slice(),
            tap_w: AtomicUsize::new(0),
            eq_preset: std::sync::atomic::AtomicU8::new(0),
        }
    }
}

impl Deck {
    pub fn playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    /// Seconds played, or 0.0 before the first packet.
    pub fn elapsed(&self) -> f32 {
        let rate = self.rate.load(Ordering::Relaxed);
        if rate == 0 {
            return 0.0;
        }
        self.position.load(Ordering::Relaxed) as f32 / rate as f32
    }

    pub fn rate(&self) -> u32 {
        self.rate.load(Ordering::Relaxed)
    }

    /// The track has run out and the deck is parked on its last frame. False again once the
    /// transport is pressed.
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    pub fn set_volume(&self, v: u8) {
        self.volume.store(v.min(100), Ordering::Relaxed);
    }

    /// The level the decode thread is reading. **Nothing else can see it**, so this exists for
    /// the tests that prove a volume press reached the deck rather than only `App::state`.
    /// `DEFAULT_VOLUME` until something sets it.
    pub fn volume(&self) -> u8 {
        self.volume.load(Ordering::Relaxed)
    }

    /// What equaliser preset the deck is playing through. `NULL` (index 0) until set.
    pub fn eq_preset(&self) -> slot_ui::eq::Eq {
        slot_ui::eq::Eq::from_index(self.eq_preset.load(Ordering::Relaxed))
    }

    /// Change the equaliser preset. Cheap and lock-free: the thread picks it up on the next
    /// block, which is within a few milliseconds of the press.
    pub fn set_eq(&self, e: slot_ui::eq::Eq) {
        self.eq_preset.store(e as u8, Ordering::Relaxed);
    }

    /// The last `dst.len()` mono samples, oldest first. Returns how many were written, which
    /// is short only when `dst` is longer than the tap.
    pub fn read_tap(&self, dst: &mut [i16]) -> usize {
        let n = dst.len().min(TAP_LEN);
        let w = self.tap_w.load(Ordering::Relaxed);
        for (k, slot) in dst[..n].iter_mut().enumerate() {
            let i = (w + TAP_LEN - n + k) % TAP_LEN;
            *slot = self.tap[i].load(Ordering::Relaxed);
        }
        n
    }

    fn write_tap(&self, block: &[i16], channels: usize) {
        if channels < 1 {
            return;
        }
        for frame in block.chunks(channels) {
            let l = frame[0] as i32;
            let r = if channels == 1 { l } else { frame[1] as i32 };
            let mono = ((l + r) / 2).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let i = self.tap_w.fetch_add(1, Ordering::Relaxed) % TAP_LEN;
            self.tap[i].store(mono, Ordering::Relaxed);
        }
    }
}

enum Cmd {
    Play,
    Pause,
    Stop,
    /// 跳到 `f32` 秒。**只有这一条让解码位置真的动**，其余都是"放/停"。
    /// ★ 2026-10-06：卡座的 ←/→ 要"快退/快进"，而在此之前这里没有定位能力，
    ///   "快退"只能是重新 open 整个文件（= 回开头），那不叫快退。
    Seek(f32),
}

/// Play `path` on the deck for `seconds`, printing where it is and how loud it is.
///
/// A probe for the two things a smoke test cannot show: that the decode runs **while the
/// caller keeps going**, and that the visualiser's tap actually carries samples. Both are
/// invisible from `smoke`, which returns before any of it can be asked.
pub fn deck_probe(path: &Path, seconds: f32) -> Result<(), String> {
    use std::sync::Arc;

    let deck = Arc::new(Deck::default());
    let mut player = Player::new(Arc::clone(&deck));
    player.open(path.to_path_buf())?;

    // Pause and resume are the whole state machine, and the part a straight-line smoke test
    // can never reach. Run them here so a deck that only plays is caught here rather than in
    // the first button press.
    let mut window = [0i16; 512];
    let mut peak = 0u32;
    let mut taps = 0usize;

    player.play();
    let deadline = Instant::now() + Duration::from_secs_f32(seconds);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(250));
        let n = deck.read_tap(&mut window);
        if n == 0 {
            continue;
        }
        taps += 1;
        let p = window.iter().map(|s| s.unsigned_abs() as u32).max().unwrap_or(0);
        peak = peak.max(p);
        eprintln!(
            "slot: deck: {:6.2}s  peak {:5}  |{}|",
            deck.elapsed(),
            p,
            "#".repeat(((p as usize * 40) / i16::MAX as usize).min(40))
        );
        if !deck.playing() && taps > 4 {
            eprintln!("slot: deck: reached the end of the track");
            break;
        }
    }

    let before_pause = deck.elapsed();
    player.pause();
    std::thread::sleep(Duration::from_millis(600));
    let while_paused = deck.elapsed();
    player.play();
    std::thread::sleep(Duration::from_millis(600));
    let after_resume = deck.elapsed();

    let held = (while_paused - before_pause).abs() < 0.05;
    let moved = after_resume - while_paused > 0.2;
    eprintln!(
        "slot: deck: {} Hz  elapsed {:.2}s  tap reads {}  peak {}",
        deck.rate(),
        deck.elapsed(),
        taps,
        peak
    );
    eprintln!(
        "slot: deck: pause {:.2} -> {:.2} ({})   resume {:.2} -> {:.2} ({})",
        before_pause,
        while_paused,
        if held { "held ✓" } else { "✗ 还在走" },
        while_paused,
        after_resume,
        if moved { "moved ✓" } else { "✗ 没继续" }
    );
    if !held || !moved {
        return Err("pause/resume did not behave".to_string());
    }
    player.pause();
    Ok(())
}

/// The deck's thread and the only way to talk to it.
pub struct Player {
    send: Option<Sender<Cmd>>,
    thread: Option<JoinHandle<()>>,
    deck: std::sync::Arc<Deck>,
    /// The shelf's output device, once something has handed one over.
    ///
    /// **`None` is a legitimate value**: `smoke` and `deck_probe` drive the deck with no
    /// frontend around them, and the tests do too. They decode and drop, which is what they
    /// always did; see `run`.
    device: Option<std::sync::Arc<Device>>,
}

impl Player {
    pub fn new(deck: std::sync::Arc<Deck>) -> Self {
        Player {
            send: None,
            thread: None,
            deck,
            device: None,
        }
    }

    /// Play through `device` from now on. Set once, at boot, by whoever holds the shelf's sink.
    ///
    /// A track already open keeps the device it opened with: `open` reads this at the moment it
    /// spawns the thread, and the two are never changed underneath a running track.
    pub fn attach_audio(&mut self, device: std::sync::Arc<Device>) {
        self.device = Some(device);
    }

    pub fn deck(&self) -> &std::sync::Arc<Deck> {
        &self.deck
    }

    /// A track is open, so `play` and `pause` have something to act on.
    ///
    /// **The difference between "paused" and "nothing loaded".** The deck has to tell them apart:
    /// coming back to a deck that was paused resumes, and pressing play on a deck with nothing
    /// loaded has to open something first. Both look like "not playing" from the outside.
    pub fn is_open(&self) -> bool {
        self.send.is_some()
    }

    /// Open `path` and leave it paused. Any track already open is stopped first: one thread
    /// per track, and the sink is one device.
    pub fn open(&mut self, path: PathBuf) -> Result<(), String> {
        self.shutdown();
        // **Cleared here, on the caller's thread, not left to the new one.** The flag is what the
        // frame polls to decide a track has run out, and a thread that has been spawned but has
        // not run yet would leave the old track's `true` on screen for a frame — long enough for
        // the frame to read it as "this one ended too" and skip a track. Nothing here can fail
        // after this point, so the deck is truthfully "not finished" from the moment it is open.
        self.deck.finished.store(false, Ordering::Relaxed);
        // Open it here rather than on the thread so a file that will not decode is reported
        // to the caller that named it, instead of turning into a silent thread that exits.
        let probe = slot_media::Source::open(&path)
            .ok_or_else(|| format!("would not open: {}", path.display()))?;
        drop(probe);
        let (send, recv) = mpsc::channel();
        let deck = std::sync::Arc::clone(&self.deck);
        let device = self.device.clone();
        let handle = thread::Builder::new()
            .name("slot-deck".into())
            .spawn(move || run(path, deck, recv, device))
            .map_err(|e| format!("deck thread: {e}"))?;
        self.send = Some(send);
        self.thread = Some(handle);
        Ok(())
    }

    pub fn play(&self) {
        self.send(Cmd::Play);
    }

    pub fn pause(&self) {
        self.send(Cmd::Pause);
    }

    /// 跳到 `sec` 秒。**没有轨可跳时什么也不做**（连播放态都不碰）。
    ///
    /// ⚠️ **这是"发出去就走"的**：跳在解码线程上做，而这里已经把命令排进队列就返回了，
    ///   所以紧接着读一次 `elapsed()` 可能还是跳之前的值 —— 下一帧就对。
    ///   面板每帧重烘，一帧的差看不见，而同步等待会给音频线程加一个握手。
    pub fn seek(&self, sec: f32) {
        if self.is_open() {
            self.send(Cmd::Seek(sec));
        }
    }

    fn send(&self, cmd: Cmd) {
        if let Some(s) = &self.send {
            let _ = s.send(cmd);
        }
    }

    fn shutdown(&mut self) {
        if let Some(s) = self.send.take() {
            let _ = s.send(Cmd::Stop);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// How long the thread sleeps when it has nothing to do. Short enough that a press answered
/// from the frame feels immediate, long enough that a paused deck costs nothing.
const IDLE_MS: u64 = 10;

fn run(
    path: PathBuf,
    deck: std::sync::Arc<Deck>,
    recv: Receiver<Cmd>,
    device: Option<std::sync::Arc<Device>>,
) {
    let Some(mut source) = slot_media::Source::open(&path) else {
        eprintln!("slot: deck: would not open: {}", path.display());
        return;
    };
    let file_hz = source.rate();
    let channels = source.channels().max(1);
    deck.rate.store(file_hz, Ordering::Relaxed);
    deck.position.store(0, Ordering::Relaxed);
    // A track that is only just opening has not finished. Set here rather than left to the
    // `Play` that follows, because `open` alone already replaced the deck's contents.
    deck.finished.store(false, Ordering::Relaxed);

    // ── The device: **the one the shelf already opened, not a new one** ─────────────────────
    //
    // ★ 2026-10-07 真机实锤：这里从前是 `open_sink()` 新开一台，而设备的分从设备是**独占**的
    //   （`plug:default` 落到裸 `hw:`）—— `Session::boot` 开机就把机器占住了，所以这里必然
    //   `open` 失败。失败之后 `if opened { ring.push_blocking(..) }` 一个字节都不推，解码于是
    //   **没有任何节流**：5.1 秒墙钟解出 209.2 秒音频（40 倍速），声卡上一个字节都没有。
    //   共用 `Session` 的那一台之后，"没声音 + 速度快"一起消失。
    let output = match &device {
        Some(d) => match d.open(file_hz, Profile::Stable) {
            Ok(()) => Some(d.ring()),
            // Still not a reason to stop: the decode is the half that keeps the screen alive,
            // and a card with no sound is better than a screen frozen on one frame.
            Err(e) => {
                eprintln!("slot: deck: audio unavailable ({e}) — decoding only");
                None
            }
        },
        None => None,
    };
    let _opened = output.is_some();
    // The ring reports the rate the device actually took — the rate asked for is a preference.
    // With no device the **file's own rate** stands in, which the resampler reads as "nothing to
    // convert", because the alternative (zero) is a division.
    let device_hz = match output.as_ref().map_or(0, |r| r.sample_rate()) {
        0 => file_hz,
        r => r,
    };
    let mut resampler = Resampler::new(file_hz as f64, device_hz as f64);
    // The EQ: **one cascade per channel, built once per stream** (see `slot_ui::eq::Stage` —
    // rebuilding it per block would restart the filter's tail and click). It is retuned once
    // per block from the deck's preset, which is a no-op when the preset has not changed.
    let mut eq_stage = slot_ui::eq::Stage::new(channels);
    let mut out: Vec<i16> = Vec::new();
    let mut playing = false;
    let mut frames: u64 = 0;

    loop {
        // Commands first, and drained: a pause while a packet is decoding is answered as
        // soon as the packet is done, which is a few milliseconds and well inside the time a
        // frame takes to notice.
        let mut quit = false;
        loop {
            match recv.try_recv() {
                Ok(Cmd::Play) => {
                    playing = true;
                    // Pressing play at the end of a track means "again from the top" — but the
                    // source has no seek, so what it really means is "the deck stopped being
                    // finished". The frame reads this to tell the two apart.
                    deck.finished.store(false, Ordering::Relaxed);
                }
                Ok(Cmd::Pause) => playing = false,
                Ok(Cmd::Stop) => quit = true,
                Ok(Cmd::Seek(sec)) => {
                    // 真跳：定位成功后**把已放过的帧数一起改写**，否则 `deck.position`
                    // 还停在跳之前，走带条会先往回跳一格再往前。
                    if source.seek_to(sec) {
                        frames = (sec.max(0.0) * file_hz as f32) as u64;
                        deck.position.store(frames, Ordering::Relaxed);
                    }
                }
                Err(_) => break,
            }
        }
        deck.playing.store(playing, Ordering::Relaxed);
        if quit {
            return;
        }
        if !playing {
            thread::sleep(Duration::from_millis(IDLE_MS));
            continue;
        }

        let Some(block) = source.next() else {
            // End of the track. Not an error, and not a reason to tear the thread down: the
            // deck stays parked on the last frame so the screen keeps showing the track
            // rather than emptying itself the moment a song finishes.
            playing = false;
            deck.playing.store(false, Ordering::Relaxed);
            deck.finished.store(true, Ordering::Relaxed);
            thread::sleep(Duration::from_millis(IDLE_MS));
            continue;
        };

        // Taken before the push: the tap is what the *decode* produced, so it stays live even
        // when there is no device behind the ring to play it.
        deck.write_tap(block, channels);
        resampler.process(block, &mut out);
        // The EQ sits **between resample and volume**: it shapes the spectrum, and the volume
        // knob after it is just a volume knob.
        //
        // ⚠️ **It is *not* what keeps an over-boosted preset from clipping** — an earlier version
        // of this comment said so and it was wrong (`.workbuddy/memory/MEMORY-排错与工具.md` §22):
        // the old stage squared the signal off *inside* `apply`, before volume ever saw it, so
        // turning the volume down only made the already-clipped square wave smaller. The stage now
        // does that job itself — it leaves each preset its own headroom and ends in a soft knee
        // (`slot_ui::eq::Stage`). (`NULL` is a no-op inside `apply`, so an untuned deck pays
        // nothing here.)
        eq_stage.set(deck.eq_preset(), device_hz);
        eq_stage.apply(&mut out, channels);
        volume::apply(&mut out, deck.volume.load(Ordering::Relaxed));
        if let Some(ring) = &output {
            ring.push_blocking(&out);
        }
        frames += (block.len() / channels) as u64;
        deck.position.store(frames, Ordering::Relaxed);
    }
}
