use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use slot_retro::{
    ButtonMask, Link, LinkChannel, RetroCore, Rumble, MAX_FRAME_H, MAX_FRAME_W, NETPACKET_RELIABLE,
};

use crate::audio::{Profile, Ring};
use crate::drc::{drc_ratio, drc_target};
use crate::frames::{FrameRef, Frames};
use crate::persist::Snapshot;
use crate::resample::Resampler;
use crate::rewind::{RewindThread, REWIND_BYTES};

/// Present is locked to the 60 Hz panel and the core is stepped once per present, so the
/// 0.456% the GBA runs slow lands entirely on audio rate control.
const PRESENT: Duration = Duration::from_nanos(16_666_667);

/// The panel cadence in Hz. Must track `PRESENT` (16.666667 ms ≃ 60 Hz). The worker presents
/// once per `PRESENT`; anything the core should run at a different rate is reconciled against
/// this, not against the core's own fps.
const PRESENT_HZ: f64 = 60.0;

/// ★ 2026-10-22: how far a core's reported fps may sit from the panel before we stop
/// video-locking it and switch to fractional stepping. GBA's 59.7275 Hz is 0.46% off — far
/// below this — so it keeps locking to the 60 Hz panel and lets audio absorb the sub-1% drift,
/// exactly as before. A 50 Hz PAL game is 16.7% off, far above this, so it steps at its true
/// rate instead of being forced 20% fast.
const PAL_LOCK_THRESHOLD: f64 = 0.02;

/// Core frames per present while fast forwarding. There is no ramp and no adaptive cap: the
/// core is stepped this many times and the deadline below absorbs whatever that costs.
///
/// Four, because an H700 cannot serve more. Eight was measured on hardware and pegged the
/// worker at the same 100.5% of one core that four does — a thread already at 100% does the
/// same work per second either way, so the extra steps bought no speed at all. What they cost
/// was presents: the deadline sleep was never reached, so the picture fell from 60 Hz to
/// around 30 at the same ~4x. A step count the hardware cannot serve does not run faster, it
/// runs choppier.
///
/// ★ **2026-10-10: this is now the default and the top of the list, not the only value.** The
/// player picks from `slot_store::FF_SPEED_CHOICES` and `Session::sync_speed` hands it over every
/// frame; what stands here is what a session that never got that far runs at, and it is the value
/// this build ran at before the setting existed — so the default is the old behaviour, not a
/// change of behaviour.
pub const FAST_STEPS: u32 = 4;

/// Snapshot every other frame, so rewinding at one pop per present runs back at 2x.
///
/// Not raiseable to 1 without a fight: measured on the H700 a snapshot is 9.2 ms of the
/// 16.67 ms frame — serialize 6.6, compress 2.6 — so every frame would spend most of the
/// budget before the core has run at all. The Mac does the same work in 0.33 ms, which is
/// why this has to be measured on the device and not the desk.
const SNAPSHOT_EVERY: u32 = 2;

/// Frames between traced pacing lines, about five seconds.
const TRACE_EVERY: u64 = 300;

/// A per-present cap on how many packets the worker will move from the transport into the
/// core's inbound queue. Real GBA serial hardware never comes close to this in a present's
/// worth of traffic; it exists for a peer that floods, so one present's worth of a flood
/// costs one present's worth of work — the transport's `try_recv` is a queue poll, not a
/// syscall, so this is cheap insurance rather than a real constraint on anything legitimate.
const MAX_LINK_PACKETS_PER_PRESENT: u32 = 256;

/// ★ 2026-10-20：慢放的分频 —— **每 `SLOW_DIV` 个 present 跑一帧核心**，所以 `2` 是半速。
///
/// 为什么不直接调低出帧率：屏幕的节拍是它自己的（`drc` 在管的那件事），把 60Hz 的屏改成 30Hz
/// 会让每一帧都变成两次刷新里的同一张图，那是闪，不是慢。分频只动核心那一侧，画面依旧每个
/// present 都在出，于是看到的是"画面更新变慢、游戏时间变慢"，而屏幕本身一点没变。
const SLOW_DIV: u32 = 2;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Speed {
    Paused,
    Normal,
    Fast,
    /// ★ 2026-10-20：慢放（L2 双击锁存）。**不是"每帧跑得更慢"** —— 核心只能整帧地跑，所以慢放
    ///   是**隔几个 present 才跑一帧**：画面照旧按屏幕的节拍出，游戏里的时间走一半。见下面的
    ///   `SLOW_DIV`。
    Slow,
}

impl Speed {
    /// The one place the wire encoding is decided, so the worker's read of what it was told
    /// and the handle's read of what the worker saw cannot drift apart by having two matches
    /// that quietly stop agreeing.
    fn from_u8(v: u8) -> Speed {
        match v {
            0 => Speed::Paused,
            1 => Speed::Normal,
            2 => Speed::Fast,
            _ => Speed::Slow,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CoreState {
    Loading,
    Ready,
    Failed,
}

pub struct EmuHandle {
    frames: Arc<Frames>,
    shared: Arc<Shared>,
    cmds: Sender<Cmd>,
    join: Option<JoinHandle<()>>,
    rumble: Rumble,
    /// The core's end of its own serial traffic, cloned off the core exactly once — see
    /// `spawn` for why calling `RetroCore::net` a second time would not do, for a core (the
    /// mock) whose default hands back a fresh, unrelated queue every time it is asked.
    link: Link,
}

enum Cmd {
    Load(Vec<u8>),
    Save(Sender<Vec<u8>>),
    Sav(Sender<Option<Vec<u8>>>),
    Thumb(Sender<Option<Vec<u8>>>),
    /// Wires a transport to the core's serial traffic. `client_id` is libretro's own: 0 the
    /// host, 1 the joiner.
    BeginLink(u16, Box<dyn LinkChannel>),
    /// Drops the transport — which is what actually closes the wire, see `TcpLink`'s `Drop`
    /// — and marks the session no longer active.
    EndLink,
    /// (Re)apply the cheat list. Each entry is `(enabled, code)`; the index in the list is the
    /// slot mGBA builds its table from. Sent from the frontend, applied on the emulator thread
    /// like every other core call.
    ///
    /// ★ 2026-10-31：**`enabled` 必须是前端先过滤好的结果，核心不会替我们过滤。**
    /// mGBA 的 `retro_cheat_set` 里写着 `UNUSED(enabled)` —— 它把收到的每一条都无条件
    /// `mCheatAddLine` 进同一张表，`refresh` 之后全部生效。所以「逐条开关」这条能力
    /// 只存在于前端：`false` 的码必须在 `Session::push_cheats` 就被筛掉，不能交给核心。
    /// RetroArch 自己也是这么做的（只把 `enabled = true` 的条目交给 `core_set_cheat`）。
    SetCheats(Vec<(bool, String)>),
    /// ★ 2026-10-21：把核心拨回开机那一刻（`RetroCore::reset`）。游戏菜单「重开本局」走这条，
    /// 取代先前"拔卡再插回去"那套 —— 插回去会先接上拔卡那一刻的即时存档，于是重开变回原样。
    Reset,
}

struct Shared {
    input: AtomicU16,
    speed: AtomicU8,
    /// What the worker last read `speed` as, stored right after that read with `Release` so
    /// `EmuHandle::observed_speed` can tell a stopped core from a merely descheduled one — see
    /// its doc comment.
    observed: AtomicU8,
    state: AtomicU8,
    rewind: AtomicBool,
    /// How much rewind history is left, 0 to 100, for the HUD bar to draw.
    rewind_fill: AtomicU8,
    /// ★ 2026-10-10: how many core frames one present runs while fast forwarding. The card's
    /// gear (`slot_store::FF_SPEED_CHOICES`), read every pass rather than latched at spawn, so a
    /// settings screen that is open in another session cannot leave this stale — the session
    /// pushes it every frame, see `Session::sync_speed`.
    ff_steps: AtomicU8,
    /// ★ 2026-10-10: how far back one present walks while rewinding, **as a multiplier of the
    /// frame rate rather than as a count of pops** — the pops are this over `SNAPSHOT_EVERY`, and
    /// that conversion belongs on this side of the boundary, where the spacing is known.
    ///
    /// ⚠️ `SNAPSHOT_EVERY` divides every entry of `REWIND_SPEED_CHOICES`; a spacing that did not
    /// divide them would round a gear down to a slower one rather than fail, which is the quiet
    /// kind of wrong.
    rewind_gear: AtomicU8,
    /// ★ 2026-10-16: whether fast forward and rewind keep the core's audio. The card's switch
    /// (`slot_store::FF_SOUND_DEFAULT`), read every pass rather than latched at spawn, so a
    /// settings screen that is open in another session cannot leave this stale — the session
    /// pushes it every frame, see `Session::sync_speed`.
    ///
    /// **Starts `false`, which is what this build has always done**: the worker drops the core's
    /// audio while speeding unless told otherwise, and a session that never got a push has to
    /// behave exactly as one that did.
    ff_sound: AtomicBool,    stop: AtomicBool,
    /// 0 to 100. Read by the worker every batch, so a change lands within one frame.
    volume: AtomicU8,
    /// Frames this core has published. Counted rather than peeked because `Frames::latest`
    /// consumes: anything that asks the buffer a question steals a frame from the renderer.
    published: AtomicU64,
    /// Set once, at open, if the core refused the resume state it was handed. A core running
    /// with an `unserialize` it rejected is not resuming the player's session — it is
    /// wherever `load` left it, most often frame zero — so its own `serialize()` is not the
    /// player's progress and must not be allowed to overwrite the resume file that state was
    /// refused instead of replacing. See `EmuSnapshot::resume_trusted`.
    resume_refused: AtomicBool,
    /// The save-ram twin of `resume_refused`, and deliberately a separate flag rather than
    /// one shared bit: a core can accept one and refuse the other (save ram is a fixed-size
    /// cartridge byte count that can coincidentally match across two unrelated cores; a
    /// serialized machine state almost never does), so only the region actually refused may
    /// be withheld. See `EmuSnapshot::save_ram_trusted`.
    sav_refused: AtomicBool,
}

impl EmuHandle {
    /// The ring rather than the device: it was opened before this cart and it outlives it,
    /// so the slot can still make a noise with no core running.
    pub fn spawn(
        core: Box<dyn RetroCore>,
        rom: PathBuf,
        ring: Arc<Ring>,
        sav: Option<Vec<u8>>,
        resume: Option<Vec<u8>>,
        profile: Profile,
    ) -> Self {
        // Taken before the core goes to its thread, which is the last moment this side can
        // reach it. `net()` exactly once, for the same reason `rumble()` is: `RetroCore`'s
        // default hands back a fresh, disconnected queue on every call (there is nothing to
        // persist for a core with no serial traffic of its own), so calling it again inside
        // the worker to get "the same" link would not be the same link at all for the mock.
        let rumble = core.rumble();
        let link = core.net();
        // Sized at the upper bound of any machine's frame, not the GBA's: a bigger frame than
        // the GBA is what this handoff has to carry once the panel runs more than one machine,
        // and a buffer sized for 240x160 would drop the surplus. The cost is an allocation
        // rather than a copy — one buffer, doubled in the worst case — which is not worth a
        // per-frame resize. See `slot_retro::MAX_FRAME_W` for the reasoning.
        let frames = Frames::new((MAX_FRAME_W * MAX_FRAME_H * 4) as usize);
        let shared = Arc::new(Shared {
            input: AtomicU16::new(0),
            // Paused until told otherwise. A core spawned during the insert would
            // otherwise run a frame or two before the session's first `sync_speed` lands,
            // and those frames are the start of the bios boot animation.
            speed: AtomicU8::new(Speed::Paused as u8),
            // Matches `speed`'s own initial value: no iteration has run yet, so nothing has
            // been observed but the value it will start from.
            observed: AtomicU8::new(Speed::Paused as u8),
            state: AtomicU8::new(CoreState::Loading as u8),
            rewind: AtomicBool::new(false),
            rewind_fill: AtomicU8::new(0),
            // The gears start where this build has always run: one flat step count for fast
            // forward and one snapshot popped per present for rewind. A session pushes the card's
            // values within a frame; these are what a session that never got that far runs at.
            ff_steps: AtomicU8::new(FAST_STEPS as u8),
            rewind_gear: AtomicU8::new(2),
            // Silent while speeding, which is the shipped behaviour and the card's default — a
            // session pushes the card's value within a frame; this is what one that never got
            // that far runs at.
            ff_sound: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            volume: AtomicU8::new(100),
            published: AtomicU64::new(0),
            resume_refused: AtomicBool::new(false),
            sav_refused: AtomicBool::new(false),
        });
        let (tx, rx) = channel();
        let worker = Worker {
            frames: frames.clone(),
            shared: shared.clone(),
            cmds: rx,
            profile,
        };
        // A clone rather than the value itself: the worker needs its own handle to pump every
        // frame, and this side keeps one so `EmuHandle::net` can hand it out too.
        let worker_link = link.clone();
        let join = std::thread::Builder::new()
            .name("slot-emu".into())
            .spawn(move || worker.run(core, rom, ring, sav, resume, worker_link))
            .ok();
        if join.is_none() {
            shared
                .state
                .store(CoreState::Failed as u8, Ordering::Release);
        }
        EmuHandle {
            frames,
            shared,
            cmds: tx,
            join,
            rumble,
            link,
        }
    }

    /// The core's end of the motor, written from the emulator thread and read from the
    /// render one. Keeping the device write on this side is the whole reason it is a cell.
    pub fn rumble(&self) -> &Rumble {
        &self.rumble
    }

    /// The core's end of its own serial traffic, the same shape `rumble` above is. Exists for
    /// whoever ends up showing a link indicator, and is what a test pushes a packet onto or
    /// reads one off to prove the worker's own pump moved it — see `crates/slot/tests/emu.rs`.
    pub fn net(&self) -> &Link {
        &self.link
    }

    /// Wires a transport into the core's serial traffic, on the emulator thread — the only
    /// place a call into a libretro core is ever allowed to happen. `client_id` is libretro's
    /// own: 0 the host, 1 the joiner, the only two this product has.
    pub fn begin_link(&self, client_id: u16, transport: Box<dyn LinkChannel>) {
        let _ = self.cmds.send(Cmd::BeginLink(client_id, transport));
    }

    /// Drops the transport and marks the session no longer active. Safe to call whether or
    /// not one was ever begun — the peer vanishing and this end asking to stop are the same
    /// request as far as the worker is concerned.
    pub fn end_link(&self) {
        let _ = self.cmds.send(Cmd::EndLink);
    }

    /// (Re)apply the cheat list. Each entry is `(enabled, code)` — the index in the list is the
    /// slot the core's cheat table is built from, so re-sending the same list with a flipped
    /// `enabled` toggles that code without the core losing track of which is which. A core with
    /// no `retro_cheat_set` (gpSP) simply no-ops each call.
    pub fn set_cheats(&self, list: Vec<(bool, String)>) {
        let _ = self.cmds.send(Cmd::SetCheats(list));
    }

    /// ★ 2026-10-21：**重开本局** —— 让核心把自己的机器拨回开机那一刻（`retro_reset`）。
    ///
    /// 走命令队列而不是直接调核心，和这里每一条核心入口一样：核心只属于那个线程。它是一个
    /// 请求，不等回执 —— 复位是一帧之内的事，等它没有意义，而一个会因为核心忙而卡住前端的
    /// 菜单是不可接受的。
    ///
    /// 复位不动 ROM、SRAM、即时存档文件，也**不动倒带历史**：玩家按"重开"之后手里那条历史
    /// 仍然是重开前的，一条撤销就能回去 —— 这是有意的，重开不是"销毁证据"。
    pub fn reset(&self) {
        let _ = self.cmds.send(Cmd::Reset);
    }

    pub fn set_input(&self, mask: ButtonMask) {
        self.shared.input.store(mask.0, Ordering::Relaxed);
    }

    /// What the worker will read on its next pass. The far side of the one boundary a
    /// button crosses to become the game's, and the only place a test can ask whether a
    /// press a menu was using reached the core anyway.
    pub fn input(&self) -> ButtonMask {
        ButtonMask(self.shared.input.load(Ordering::Relaxed))
    }

    pub fn latest_frame(&self) -> Option<FrameRef> {
        self.frames.latest()
    }

    pub fn set_speed(&self, speed: Speed) {
        self.shared.speed.store(speed as u8, Ordering::Relaxed);
    }

    /// L2 is momentary and takes precedence over fast forward, so this is a separate axis
    /// from `Speed` rather than another value of it: releasing it returns to whatever the
    /// speed already was.
    /// Whether this core has produced anything yet. Never gate the game layer on the
    /// handle existing: it is built before its worker has run a single frame.
    pub fn has_published(&self) -> bool {
        self.shared.published.load(Ordering::Relaxed) > 0
    }

    pub fn frame_ready(&self) -> bool {
        self.frames.is_ready()
    }

    pub fn frames_taken(&self) -> u64 {
        self.frames.taken()
    }

    pub fn published_count(&self) -> u64 {
        self.shared.published.load(Ordering::Relaxed)
    }

    /// What the worker last read `speed` as, not what this side last told it to be — the gap
    /// between those two is exactly the race an eject has to close. `Acquire`, paired with the
    /// worker's `Release` store, means a caller who sees `Paused` here is also guaranteed to
    /// see every frame `publish` counted before that store: a fact about the last iteration
    /// the worker actually ran, not an inference from a count that merely has not moved yet.
    pub fn observed_speed(&self) -> Speed {
        Speed::from_u8(self.shared.observed.load(Ordering::Acquire))
    }

    pub fn set_volume(&self, level: u8) {
        self.shared.volume.store(level.min(100), Ordering::Relaxed);
    }

    pub fn set_rewinding(&self, on: bool) {
        self.shared.rewind.store(on, Ordering::Relaxed);
    }

    /// ★ 2026-10-10: how many core frames one present runs while fast forwarding — the card's
    /// gear (`slot_store::FF_SPEED_CHOICES`), handed over every frame by `Session::sync_speed`.
    ///
    /// **Clamped at one, not at zero.** The worker already treats "no steps" as a pause, and a
    /// zero arriving here would stop the core dead behind a picture the screen is still updating
    /// — a freeze with no key behind it. The card's own list never contains one; this is the edge
    /// being held here rather than trusted from three crates away.
    pub fn set_ff_steps(&self, steps: u8) {
        self.shared.ff_steps.store(steps.max(1), Ordering::Relaxed);
    }

    /// ★ 2026-10-10: how far back one present walks while rewinding, as a multiple of the
    /// snapshot spacing (`SNAPSHOT_EVERY`) — so `2` is the one-snapshot-per-present this build
    /// always did and `8` is four of them. Latched here rather than converted: this side is where
    /// the spacing lives.
    pub fn set_rewind_speed(&self, x: u8) {
        self.shared.rewind_gear.store(x, Ordering::Relaxed);
    }

    /// ★ 2026-10-16: whether fast forward and rewind keep playing the core's audio — the card's
    /// switch (`slot_store::FF_SOUND_DEFAULT`), handed over every frame by `Session::sync_speed`.
    ///
    /// Off is the shipped behaviour outright: the worker drops the core's audio while speeding.
    /// On is two different things per speed, and both are the honest ones available — a rewind
    /// plays the frame the walk landed on at its natural rate (one frame per present, nothing
    /// to compress), and a fast forward resamples its gear's worth of audio down into one
    /// present, which is what a tape does: the pitch rises with the speed. See `Worker::run`
    /// for the measurement that rules out keeping the pitch instead.
    pub fn set_ff_sound(&self, on: bool) {
        self.shared.ff_sound.store(on, Ordering::Relaxed);
    }

    pub fn rewind_fill(&self) -> u8 {
        self.shared.rewind_fill.load(Ordering::Relaxed)
    }

    pub fn state(&self) -> CoreState {
        match self.shared.state.load(Ordering::Acquire) {
            0 => CoreState::Loading,
            1 => CoreState::Ready,
            _ => CoreState::Failed,
        }
    }

    /// The state arrives on the receiver once the worker reaches a frame boundary. A dead
    /// worker closes the channel rather than leaving the caller waiting forever.
    pub fn request_state(&self) -> Receiver<Vec<u8>> {
        let (tx, rx) = channel();
        let _ = self.cmds.send(Cmd::Save(tx));
        rx
    }

    pub fn request_load(&self, state: Vec<u8>) {
        let _ = self.cmds.send(Cmd::Load(state));
    }

    pub fn snapshot(&self) -> EmuSnapshot {
        EmuSnapshot {
            cmds: self.cmds.clone(),
            shared: self.shared.clone(),
        }
    }
}

/// The flush paths need the core's bytes, not its thread or its frames. Cloning the
/// command sender is most of that; `shared` rides along too, because the two flags on it are
/// how a flush path learns a region it is about to ask for was never the player's to begin
/// with — see `resume_trusted`/`save_ram_trusted` below.
#[derive(Clone)]
pub struct EmuSnapshot {
    cmds: Sender<Cmd>,
    shared: Arc<Shared>,
}

impl Snapshot for EmuSnapshot {
    fn state(&self) -> Option<Vec<u8>> {
        let (tx, rx) = channel();
        self.cmds.send(Cmd::Save(tx)).ok()?;
        rx.recv().ok()
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        let (tx, rx) = channel();
        self.cmds.send(Cmd::Sav(tx)).ok()?;
        rx.recv().ok().flatten()
    }

    fn thumb(&self) -> Option<Vec<u8>> {
        let (tx, rx) = channel();
        self.cmds.send(Cmd::Thumb(tx)).ok()?;
        rx.recv().ok().flatten()
    }

    fn load(&self, state: Vec<u8>) {
        let _ = self.cmds.send(Cmd::Load(state));
    }

    /// `false` exactly when `Worker::run` handed this core a resume it went on to refuse.
    /// `state()` above still answers with whatever the core serializes regardless — a running
    /// core always has *some* state — so a flush path must check this before it is allowed to
    /// treat those bytes as the player's session and write them over the resume file.
    fn resume_trusted(&self) -> bool {
        !self.shared.resume_refused.load(Ordering::Acquire)
    }

    /// The save-ram twin of `resume_trusted`.
    fn save_ram_trusted(&self) -> bool {
        !self.shared.sav_refused.load(Ordering::Acquire)
    }
}

impl Drop for EmuHandle {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

struct Worker {
    frames: Arc<Frames>,
    shared: Arc<Shared>,
    cmds: Receiver<Cmd>,
    /// The audio profile this session was inserted with. Held here rather than passed through
    /// `run` because it is fixed for the life of the session — the shelf is where it changes,
    /// and a change is what reopens the sink and starts the next session with a new value.
    profile: Profile,
}

impl Worker {
    /// One present's worth of core audio, resampled to the device and pushed, paced by the DRC.
    ///
    /// The DRC ratio is re-read every call rather than held: it is proportional and only
    /// reaches full authority at twice the target, so a ring that has run ahead or behind is
    /// walked back over the next few presents rather than yanked. Normal play and a rewind
    /// with 「加速时声音」 on both land here — the rewind produces exactly one frame of audio
    /// per present, the same rate the device consumes, so there is nothing for the DRC to
    /// fight.
    fn push_audio(
        &self,
        audio: &[i16],
        out: &mut Vec<i16>,
        resampler: &mut Resampler,
        ring: &Ring,
    ) {
        let target = drc_target(ring.capacity_frames(), self.profile);
        let queued = ring.queued_frames();
        let ratio = drc_ratio(queued, target);
        self.push_at(audio, out, resampler, ring, ratio);
    }

    /// Resample `audio` at `ratio` and push it. The ratio is the resampler's own: above 1.0
    /// stretches a starved device, below 1.0 compresses — which is what a fast forward with
    /// 「加速时声音」 on asks for, one gear's worth of core audio squeezed into one present.
    fn push_at(
        &self,
        audio: &[i16],
        out: &mut Vec<i16>,
        resampler: &mut Resampler,
        ring: &Ring,
        ratio: f64,
    ) {
        resampler.set_ratio(ratio);
        resampler.process(audio, out);
        crate::audio::volume::apply(out, self.shared.volume.load(Ordering::Relaxed));
        ring.push_blocking(out);
    }

    fn run(
        self,
        mut core: Box<dyn RetroCore>,
        rom: PathBuf,
        ring: Arc<Ring>,
        sav: Option<Vec<u8>>,
        resume: Option<Vec<u8>>,
        link: Link,
    ) {
        // A PBP is a container the core cannot open, so it is unwrapped here before the load —
        // the same division of labour as RetroArch, where the frontend extracts it and
        // pcsx_rearmed only ever sees a plain image. The stem is unchanged, so the cart's own
        // `.bios` still resolves to the file it always did.
        let rom = crate::core::pbp_image(&rom).unwrap_or(rom);
        // ★ 2026-10-27：**多手柄口那一步的自测开关**。`SLOT_P1_MIRROR=1` 把本机按键同时喂给
        //   port 1 —— 一台机器上就能看见"第二个玩家"（FBNeo 双人模式里 P2 跟着 P1 动），
        //   不必等热点联机那套做出来。默认关闭，发布版一个字都不受影响。
        let mirror_p1 = std::env::var_os("SLOT_P1_MIRROR").is_some_and(|v| v != "0");
        if let Err(e) = core.load(&rom) {
            eprintln!("slot: {e}");
            self.shared
                .state
                .store(CoreState::Failed as u8, Ordering::Release);
            return;
        }
        // After the load: there is no save ram to copy into until the rom says how much of
        // it there is. A game with none at all is not a failure to boot.
        if let Some(sav) = sav {
            if let Err(e) = core.load_save_ram(&sav) {
                eprintln!("slot: save ram: {e}");
                // The core is about to run with its own idea of save ram rather than the
                // player's — most often a mock's or a mismatched core's own default — so
                // `save_ram()` from here on must never be allowed to overwrite the real file
                // that refusal left untouched. `EmuSnapshot::save_ram_trusted` is what a
                // flush path checks before it will.
                self.shared.sav_refused.store(true, Ordering::Release);
            }
        }
        // Before Ready, so the reveal shows where the cart left off rather than a frame of
        // the intro. A state the core will not take leaves the save ram loaded above, which
        // costs the player their position but not their progress.
        if let Some(resume) = resume {
            if let Err(e) = core.unserialize(&resume) {
                eprintln!("slot: resume: {e}");
                // Same reasoning as `sav_refused` above, for the resume half: a core running
                // from wherever `load` left it is not resuming anything, and its `serialize()`
                // must not be allowed to overwrite the resume file that was refused instead of
                // replacing.
                self.shared.resume_refused.store(true, Ordering::Release);
            }
        }
        let av = core.av_info();
        // A device that refused the GBA's rate reports its own, and a device that failed to
        // open reports zero, which the resampler reads as "no conversion to do".
        let device_hz = match ring.sample_rate() {
            0 => av.sample_rate,
            hz => hz as f64,
        };
        // ★ 2026-10-22: PAL/NTSC. The libretro core reports its *content's* true fps through
        // `av.fps` — fceumm, supafaust, genesis_plus_gx and pcsx_rearmed all do, so the region
        // (PAL 50 Hz vs NTSC ~60 Hz) is detected for us; we never parse the ROM. Anything within
        // `PAL_LOCK_THRESHOLD` of the panel stays video-locked (stepped once per present) and
        // lets the resampler absorb the tiny drift — GBA's 0.46% included, unchanged from
        // before. Anything further off (a 50 Hz PAL game) is stepped at its true rate via the
        // fractional accumulator in `normal_steps`, so it is not forced 20% fast onto the 60 Hz
        // panel.
        let fps = if av.fps > 0.0 { av.fps } else { PRESENT_HZ };
        let drift = (fps - PRESENT_HZ).abs() / PRESENT_HZ;
        let video_locked = drift < PAL_LOCK_THRESHOLD;
        // When locked, the core runs one frame per present but at `fps` != 60, so its audio is
        // produced at `sample_rate / (fps * PRESENT)` in wall time — the resampler's base rate
        // absorbs that sub-threshold surplus exactly as it did for the GBA before this change.
        // When not locked we step at the true `fps`, so wall production is just `sample_rate`.
        let core_hz = if video_locked {
            av.sample_rate / (fps * PRESENT.as_secs_f64())
        } else {
            av.sample_rate
        };
        let mut resampler = Resampler::new(core_hz, device_hz);
        ring.clear_faults();
        self.shared
            .state
            .store(CoreState::Ready as u8, Ordering::Release);

        let mut out = Vec::new();
        let mut muted_at = Speed::Normal;
        // ★ 2026-10-20：慢放的分频相位。**只在 `Speed::Slow` 的 pass 里走**（`steps` 那一臂），
        //   所以从暂停或快进退回慢放时它从 0 起算，不会出现"上一次剩下的半个周期"。
        let mut slow_tick: u32 = 0;
        // What the ring was last muted for. Kept beside `muted_at` because since 2026-10-16
        // the mute depends on two inputs — the speed and the sound switch — and a guard on one
        // of them alone would leave a ring muted (or heard) for a switch flipped since the last
        // change of speed.
        let mut sound_at = false;
        let rewind = RewindThread::spawn(REWIND_BYTES);
        let mut since_snapshot = 0;
        let mut deadline = Instant::now();
        let mut paced = 0u64;
        // ★ 2026-10-22: running credit for fractional stepping. Only spent when the core is
        //   not video-locked (a 50 Hz PAL game); `normal_steps` adds `fps / PRESENT_HZ` per
        //   present and runs `floor` of it, so the core averages its true rate on the 60 Hz
        //   panel. Idle for locked systems (GBA), where Normal steps exactly one frame.
        let mut frame_acc: f64 = 0.0;
        // `None` until a session begins. Held here rather than on `Shared`: the transport is
        // not `Sync`-shaped state a render-thread read would make sense of, only something
        // this loop drains and feeds once a frame.
        let mut transport: Option<Box<dyn LinkChannel>> = None;
        while !self.shared.stop.load(Ordering::Relaxed) {
            for cmd in self.cmds.try_iter() {
                match cmd {
                    Cmd::Save(reply) => match core.serialize() {
                        Ok(state) => {
                            let _ = reply.send(state);
                        }
                        Err(e) => eprintln!("slot: {e}"),
                    },
                    Cmd::Load(state) => {
                        if let Err(e) = core.unserialize(&state) {
                            eprintln!("slot: {e}");
                        }
                    }
                    Cmd::Sav(reply) => {
                        let _ = reply.send(core.save_ram());
                    }
                    Cmd::Thumb(reply) => {
                        let av = core.av_info();
                        let _ = reply.send(crate::thumb::png(
                            core.video_xrgb8888(),
                            av.width,
                            av.height,
                        ));
                    }
                    Cmd::BeginLink(client_id, t) => {
                        core.start_link(client_id);
                        // Set here as well as by `LibretroCore::start_link` itself: this is
                        // the thing that actually knows a transport is wired and about to be
                        // pumped, whatever the concrete core does or does not do with
                        // `client_id` — the mock, in particular, has no session of its own to
                        // start and would otherwise leave `is_active` false with real traffic
                        // already flowing through it.
                        link.set_active(true);
                        transport = Some(t);
                    }
                    Cmd::SetCheats(list) => {
                        // Clear first so a shortened list can't leave stale entries behind —
                        // which for mGBA is the *only* way a code ever comes off, since it
                        // ignores the `enabled` flag entirely. The list arriving here is
                        // already the live subset (see the variant's note); `enabled` is passed
                        // along anyway because a core that does honour it (RetroArch-style)
                        // would want it, and it costs nothing to say.
                        core.cheat_reset();
                        for (i, (enabled, code)) in list.into_iter().enumerate() {
                            core.set_cheat(i as u32, enabled, &code);
                        }
                    }
                    // ★ 2026-10-21：重开本局。核心自己的复位，ROM 早在 `spawn` 时读进
                    // 内存了，所以这一下只重走开机的第一步 —— 正是"回到 BIOS 出现那一刻"。
                    // 退带历史、SRAM、即时存档一概不碰。
                    Cmd::Reset => {
                        core.reset();
                        // The history behind a reset describes a machine that no longer exists;
                        // keeping it would let one undo pull the picture back to where the
                        // player was before they asked to start over.
                        rewind.clear();
                    }
                    Cmd::EndLink => {
                        // `Cmd::BeginLink`'s counterpart: tells the core the session is over,
                        // if it registered a `stop` to hear it through (`RetroCore::stop_link`
                        // — libretro documents `stop` as OPTIONAL, unlike `start`, so this is
                        // a no-op for a core that never offered one). Without this the core
                        // keeps believing a session is live and keeps producing packets
                        // nobody is left to carry.
                        core.stop_link();
                        // The drop is what actually closes the wire (see `TcpLink`'s `Drop`);
                        // this is just letting go of it.
                        transport = None;
                        // Cleared *before* the flag flips, not after: `Link::clear` empties
                        // both queues (a packet that arrived a moment before this command
                        // would otherwise sit here until the *next* session begins and gets
                        // fed to a core that never sent or asked for it), and `set_active`'s
                        // `Release` store only carries a happens-before guarantee for what
                        // ran on this thread *before* it. Clearing first is what lets a
                        // reader who observes `is_active() == false` (`Acquire`) also see the
                        // queues already empty, with no sleep needed to bridge the gap.
                        link.clear();
                        link.set_active(false);
                    }
                }
            }

            // Pumped every present regardless of speed or phase, not only while the core is
            // stepping frames: a trade partner reading the local device's power menu, or
            // sitting in the switcher, must not see the link go quiet just because this
            // device paused its own picture. Neither direction may block the frame —
            // `try_recv` already never does — so this is always safe to run.
            if let Some(t) = transport.as_mut() {
                drain_transport(t.as_mut(), &link, MAX_LINK_PACKETS_PER_PRESENT);
            }
            core.pump_link();
            // A `poll` can make the core send, so this catches anything it just queued. The
            // send that matters is the one after the frame runs, below.
            flush_outbound(&mut transport, &link);

            let speed = self.speed();
            let ff_sound = self.shared.ff_sound.load(Ordering::Relaxed);
            // Published before anything below acts on it, and with `Release`: a reader who
            // observes `Paused` from this store is thereby also guaranteed to see every frame
            // `publish` counted on an earlier pass, because that publish happened-before this
            // store in program order and `Release`/`Acquire` makes that ordering visible across
            // threads. `publish`'s own counter is `Relaxed` and leans on this pairing for it —
            // `Relaxed` here would leave that unordered, trading the scheduling race this exists
            // to close for a subtler visibility one.
            self.shared.observed.store(speed as u8, Ordering::Release);
            // ★ 2026-10-16: the mute now leans on the sound switch as well as the speed — a
            //   fast forward with 「加速时声音」 on is *meant* to be heard, so the ring must not
            //   be muted for it. Re-evaluated when either input changes, because the switch is
            //   pushed every frame (`Session::sync_speed`) and can flip without the speed
            //   changing under it.
            if speed != muted_at || ff_sound != sound_at {
                // Fast forward produces audio nobody asked to hear, so it is gated — unless
                // the card says otherwise, in which case the worker resamples the gear's worth
                // of it down into one present and pushes it like normal play's. A pause
                // produces none at all and `fill` pads a dry ring with silence, so there is
                // nothing to gate: what is left simply runs out. Muting on a pause silenced
                // the insert as well, which is mixed into this ring while the core is held
                // still and does not come from the core at all.
                //
                // ★ 2026-10-20：**慢放一律静音**，跟快进的默认一样。半速的音频只有两种做法，
                //   另一种是每两帧一次的音频硬塞进一条按满速铺的 ring，那是一秒两次的
                //   underrun —— 啪嗒声，而这条机器上的音频最怕的就是这个。
                ring.set_muted((speed == Speed::Fast && !ff_sound) || speed == Speed::Slow);
                // A held core feeds it nothing, so the device reading silence out of it is
                // the arrangement working rather than a starve worth reporting.
                ring.set_idle(speed == Speed::Paused);
                muted_at = speed;
                sound_at = ff_sound;
            }
            let input = ButtonMask(self.shared.input.load(Ordering::Relaxed));
            let rewinding = speed != Speed::Paused && self.shared.rewind.load(Ordering::Relaxed);
            let steps = match speed {
                Speed::Paused => 0,
                // ★ 2026-10-22: normal play. When the core is video-locked (near 60 Hz, e.g.
                //   GBA at 59.7275) we step exactly one frame per present, keeping the picture
                //   perfectly locked to the panel — unchanged from before. When it is not (a 50
                //   Hz PAL game) `normal_steps` feeds a fractional accumulator `fps /
                //   PRESENT_HZ` credits per present and runs `floor` of it: ~0.833 credit /
                //   present averages to 50 core frames per 60 presents, the game's true 50 Hz,
                //   at the cost of some presents holding the previous frame (3:2-style) — the
                //   unavoidable map of 50 onto 60. Fast/Slow below scale exactly as they did.
                Speed::Normal => normal_steps(&mut frame_acc, fps, PRESENT_HZ, video_locked, FAST_STEPS),
                // ★ 2026-10-20: the slow-motion divider. One core frame every `SLOW_DIV`
                //   presents, and the `SLOW_DIV - 1` presents in between run none — so the
                //   picture keeps coming out at the screen's own rate while the game's clock
                //   crawls. A present that runs no frame still publishes the last one, which
                //   is what makes this slow motion rather than a stutter.
                Speed::Slow => {
                    slow_tick = (slow_tick + 1) % SLOW_DIV;
                    u32::from(slow_tick == 0)
                }
                // ★ The card's gear, not a constant (2026-10-10). Read here rather than latched
                //   so the setting takes effect on the next hold of R2 with no place to push from.
                Speed::Fast => u32::from(self.shared.ff_steps.load(Ordering::Relaxed)).max(1),
            };
            if rewinding {
                // ★ **How far back one present travels, as a whole number of snapshots**
                //   (2026-10-10). The card offers 2x / 4x / 8x (`REWIND_SPEED_CHOICES`) and the
                //   ring holds one snapshot every `SNAPSHOT_EVERY` frames of core time, so the
                //   pops are the multiplier over the spacing: **one at 2x**, which is exactly what
                //   this arm did before the gear existed, and what a card that has never been to
                //   the settings screen still asks for.
                //
                //   Popping more than one is cheap rather than fast. Each pop is one delta decode
                //   and one XOR on the rewind thread — the expensive half of a snapshot,
                //   `serialize` and `compress`, is the capture side and is not paid here — and
                //   only the last state is handed to the core, so four pops still cost the single
                //   `run_frame` the first gear does. What it buys is the walk itself: the picture
                //   moves back 2k frames per present instead of 2.
                //
                //   ⚠️ A ring that runs out mid-walk **stops there rather than padding**: the
                //   oldest state it has is the end of the history, and that is where the picture
                //   goes. Skipping the leftover pops is also why the loop keeps the last state it
                //   got rather than the first.
                let pops = (self.shared.rewind_gear.load(Ordering::Relaxed) / SNAPSHOT_EVERY as u8)
                    .max(1);
                let mut state = None;
                for _ in 0..pops {
                    match rewind.pop() {
                        Some(s) => state = Some(s),
                        None => break,
                    }
                }
                if let Some(state) = state {
                    if let Err(e) = core.unserialize(&state) {
                        eprintln!("slot: rewind: {e}");
                    }
                    // A core is not obliged to repaint from a load, so the frame the user
                    // sees comes from running one. One pop walks back two frames and this
                    // runs one forward, so the picture travels back two frames per present:
                    // reverse at 2x, showing every other frame. `pops` of them make that
                    // 2k, which is the multiplier the settings screen prints.
                    //
                    // Empty input, never the live mask. The live one necessarily holds L2 —
                    // it is what is being held to rewind — so replaying with it re-simulated
                    // the frame under buttons that were not pressed at the time, and the
                    // state landed on when the trigger was released inherited the
                    // difference. Nothing was being replayed faithfully; it was being
                    // re-played.
                    core.run_frame(ButtonMask(0));
                    let av = core.av_info();
                    self.publish(core.video_xrgb8888(), av.width, av.height);
                }
                self.shared
                    .rewind_fill
                    .store(rewind.fill(), Ordering::Relaxed);
                // ★ 2026-10-16: 「加速时声音」 reaches here too, and the rewind's frame is the
                //   easy half of it. One `run_frame` per present is one frame of audio per
                //   present — exactly the rate the device consumes, so there is nothing to
                //   compress and nothing to queue. What is heard is the frame the walk landed
                //   on, played forward: true reversed playback would mean keeping the whole
                //   stream and running it backwards, and a game heard backwards is not a scrub.
                //
                //   Without the switch this stays what it was: taken off the core and dropped,
                //   reverse audio being noise, and the sink runs itself dry into silence.
                let audio = core.take_audio();
                if ff_sound {
                    self.push_audio(&audio, &mut out, &mut resampler, &ring);
                }
            } else if steps > 0 {
                // 自测开关（见 `mirror_p1`）：本机按键同时当 port 1 用。
                if mirror_p1 {
                    core.set_port_input(1, input);
                }
                for _ in 0..steps {
                    core.run_frame(input);
                }
                // Immediately, and this is the one that decides whether a link is playable.
                // The emulated serial hardware only executes inside `run_frame`, so every
                // packet a session actually produces is born here. Sending them from the top
                // of the loop instead means each one waits for the next present: a whole
                // frame, 16.7 ms, added to a wire measured at about 2 ms, in both directions
                // and on both devices. A GBA that asked a question and heard nothing for four
                // frames reports a communication error, which is what it should do.
                flush_outbound(&mut transport, &link);
                let av = core.av_info();
                self.publish(core.video_xrgb8888(), av.width, av.height);

                // Counted per present rather than per frame, so a fast forward pays the
                // same snapshot cost per present as normal play and simply records a
                // coarser trail: eight frames apart at `FAST_STEPS` rather than two.
                //
                // This used to sit inside the `Normal` arm below, which exists to gate the
                // audio, and was swept in with it. The effect was a hole: nothing recorded
                // while fast forwarding, so the newest state was whatever predated the
                // trigger and the first pop of a rewind swallowed the entire stretch in one
                // step instead of walking back through it.
                since_snapshot += 1;
                if since_snapshot >= SNAPSHOT_EVERY {
                    since_snapshot = 0;
                    // A core that will not serialize has already said so through the save
                    // path. Rewind is not the place to say it again at 30 Hz.
                    if let Ok(state) = core.serialize() {
                        rewind.push(state);
                        self.shared
                            .rewind_fill
                            .store(rewind.fill(), Ordering::Relaxed);
                    }
                }

                let audio = core.take_audio();
                if speed == Speed::Normal {
                    self.push_audio(&audio, &mut out, &mut resampler, &ring);
                    // Where occupancy actually sits against target is the one thing a
                    // crackle complaint needs and no test can watch on real hardware. The two
                    // numbers are re-read here rather than handed back by `push_audio`: this
                    // runs once every few hundred presents, and a return value carried for it
                    // would be paid on every present.
                    paced += 1;
                    if crate::session::trace() && paced.is_multiple_of(TRACE_EVERY) {
                        let (dropped, starved) = (ring.overruns(), ring.underruns());
                        let queued = ring.queued_frames();
                        let target = drc_target(ring.capacity_frames(), self.profile);
                        eprintln!(
                            "slot: audio: {queued}/{target} queued, {dropped} dropped, {starved} starved"
                        );
                    }
                } else if ff_sound {
                    // ★ 2026-10-16: 「加速时声音」 on, and this is a fast forward. The core just
                    //   produced `steps` frames of audio where the device consumes one, so they
                    //   are resampled down into one present's worth — **the tape's own
                    //   behaviour: the pitch rises with the gear and everything else survives**.
                    //
                    //   Why a resample and not time compression (2026-10-16, measured, then
                    //   thrown away): a WSOLA overlap-add compressor was built and swept across
                    //   window/hop/search parameters against full-scale squares and pure tones
                    //   at every gear. It does not work on this hardware's audio — at 2x a
                    //   440 Hz tone kept only 37% of its periodicity (a pure tone keeps 100%)
                    //   with its energy shoved into ±512 Hz sidebands, one of them a 72 Hz
                    //   warble; at 3x and 4x the periodicity was destroyed outright. The
                    //   structural reason: the overlap-add sums frames whose content is offset
                    //   by `HOP - HOP/ratio` samples, and that offset is not a whole number of
                    //   periods for anything the PSG plays, so the sum's phase rotates with the
                    //   output position. Keeping the spectrum exact needs a phase vocoder,
                    //   which needs an FFT this tree does not have; until then the honest
                    //   choices were chipmunk or silence, and a switch that is off by default
                    //   is where chipmunk belongs.
                    //
                    //   The ring sees exactly one present's worth per present, so the DRC has
                    //   nothing to correct and is stood down for the duration — the fixed
                    //   `1/steps` ratio is the whole of the pacing.
                    self.push_at(&audio, &mut out, &mut resampler, &ring, 1.0 / f64::from(steps));
                }
                // Without the switch the audio is dropped: it was taken off the core above,
                // and a core holding audio nobody will hear is a core that stops making it.
            }

            // The write above holds this thread whenever the device has no room, which is
            // the backstop. This is the pacing the rest of the time, and the only pacing at
            // all with no audio to pace against: paused, fast forwarding, rewinding.
            deadline += PRESENT;
            let now = Instant::now();
            match deadline.checked_duration_since(now) {
                Some(wait) => std::thread::sleep(wait),
                // Falling behind by more than a frame means a stall, not a slow frame.
                // Catching up would sprint through frames nobody sees.
                None => deadline = now,
            }
        }
        // The ring belongs to the session, so a cart that left while fast forwarding would
        // otherwise take every sound after it with it.
        ring.set_muted(false);
        ring.set_idle(false);
        let (dropped, starved) = (ring.overruns(), ring.underruns());
        if dropped > 0 || starved > 0 || crate::session::trace() {
            eprintln!("slot: audio: {dropped} samples dropped, {starved} starved");
        }
    }

    /// Publish the core's latest frame, carrying its own dimensions. The size is read from the
    /// core (`av_info`) rather than assumed from the active machine, because an arcade board
    /// hands over whatever frame it produces — a CPS1 board's 384x224 is not the FBNeo
    /// `System::Arcade`'s nominal size, and the upload has to be told the real one.
    fn publish(&self, video: &[u8], w: u32, h: u32) {
        let mut buf = self.frames.take_write();
        buf.clear();
        buf.extend_from_slice(video);
        self.frames.publish(buf, w, h);
        self.shared.published.fetch_add(1, Ordering::Relaxed);
    }

    fn speed(&self) -> Speed {
        Speed::from_u8(self.shared.speed.load(Ordering::Relaxed))
    }
}

/// Moves up to `cap` packets from `transport` into `link`'s inbound queue, in order, and
/// leaves the rest — however many there are — queued in the transport for the next call.
/// A free function, rather than inline in `Worker::run`'s loop, so the cap can be driven
/// directly against a fake transport in a test with no worker thread and no real timing
/// involved (`MAX_LINK_PACKETS_PER_PRESENT`'s own point is to bound work in one present,
/// which a test racing a real 60 Hz loop could never pin down deterministically).
/// Everything the core has queued for its peer, onto the wire.
///
/// The flag `netpacket_send` was called with never reaches this queue — only the bytes do —
/// so this asks every transport for reliable delivery. Safe for `TcpLink`, which is reliable
/// regardless of what is asked: TCP cannot honour "unreliable" any other way, and
/// `LinkChannel::send`'s own contract is to fall back to reliable when a flag cannot be
/// honoured.
fn flush_outbound(transport: &mut Option<Box<dyn LinkChannel>>, link: &Link) {
    let Some(t) = transport.as_deref_mut() else {
        return;
    };
    while let Some(packet) = link.take_outbound() {
        t.send(NETPACKET_RELIABLE, &packet);
    }
}

fn drain_transport(transport: &mut dyn LinkChannel, link: &Link, cap: u32) {
    for _ in 0..cap {
        let Some(packet) = transport.try_recv() else {
            break;
        };
        link.push_inbound(packet);
    }
}

/// ★ 2026-10-22: how many core frames one present of *normal* play should run, given the
/// core's true `fps` and the panel's `present_hz`.
///
/// Near-panel rates stay video-locked: one frame per present, smooth, with the tiny drift
/// left to the audio resampler. Far-off rates (a 50 Hz PAL game on a 60 Hz panel) cannot be
/// locked without running 20% fast, so they accumulate fractional credit — `fps /
/// present_hz` per present — and run `floor(credit)` frames, averaging the core's true rate.
/// `acc` is the running credit (persists across presents); `max` caps the frames spent in one
/// present so a delayed present cannot trigger a runaway catch-up burst.
///
/// A free function, not inline in the worker loop, so the stepping math is driven directly in
/// a test with no worker thread and no real timing — the same reason `drain_transport` is one.
fn normal_steps(acc: &mut f64, fps: f64, present_hz: f64, video_locked: bool, max: u32) -> u32 {
    if video_locked {
        1
    } else {
        *acc += fps / present_hz;
        let n = ((*acc).floor() as u32).min(max);
        *acc -= n as f64;
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slot_retro::LoopbackLink;

    /// I6: an unbounded drain here gives a flooding peer unbounded work in a single present.
    /// `LoopbackLink` holds everything sent to it in a plain queue, so filling it past the
    /// cap and draining once is enough to prove the cap actually holds — no thread, no
    /// timing, no worker loop needed.
    #[test]
    fn drain_transport_stops_at_the_cap_and_leaves_the_rest_queued() {
        let mut transport = LoopbackLink::default();
        for i in 0..10u8 {
            transport.send(0, &[i]);
        }
        let link = Link::default();

        drain_transport(&mut transport, &link, 4);

        let mut got = Vec::new();
        while let Some(p) = link.take_inbound() {
            got.push(p[0]);
        }
        assert_eq!(
            got,
            vec![0, 1, 2, 3],
            "the cap must stop the drain, not just slow it"
        );
        assert_eq!(
            transport.try_recv(),
            Some(vec![4]),
            "packets past the cap must stay queued in the transport, not be dropped"
        );
    }

    /// The ordinary case: a present's worth of traffic never comes close to the cap, so
    /// everything waiting moves in one call, same as an unbounded drain would.
    #[test]
    fn drain_transport_moves_everything_under_the_cap() {
        let mut transport = LoopbackLink::default();
        transport.send(0, b"one");
        transport.send(0, b"two");
        let link = Link::default();

        drain_transport(&mut transport, &link, MAX_LINK_PACKETS_PER_PRESENT);

        assert_eq!(link.take_inbound().as_deref(), Some(&b"one"[..]));
        assert_eq!(link.take_inbound().as_deref(), Some(&b"two"[..]));
        assert_eq!(link.take_inbound(), None);
    }

    /// ★ 2026-10-22: a video-locked system (GBA's 59.7275 Hz, or any fps within
    /// `PAL_LOCK_THRESHOLD` of 60) always steps exactly one frame per present — the picture
    /// stays locked to the panel and no fractional credit is ever spent. A near-60 fps must
    /// not drift into the PAL accumulator path.
    #[test]
    fn normal_steps_locks_near_panel_rates_to_one_per_present() {
        let mut acc = 0.0;
        for _ in 0..1000 {
            assert_eq!(
                normal_steps(&mut acc, 59.7275, PRESENT_HZ, true, FAST_STEPS),
                1,
                "locked rate must step one frame per present"
            );
        }
        assert_eq!(acc, 0.0, "locked rate must never accumulate credit");

        // Same for exactly 60 and a touch above (NTSC cores reporting 60.0988).
        let mut acc2 = 0.0;
        assert_eq!(normal_steps(&mut acc2, 60.0988, PRESENT_HZ, true, FAST_STEPS), 1);
        assert_eq!(acc2, 0.0);
    }

    /// ★ 2026-10-22: a 50 Hz PAL game must average its true rate on the 60 Hz panel, not run
    /// 20% fast. Over 60 presents the accumulator should yield 50 frames (5 holds), and over
    /// many presents the mean must converge to 50/60.
    #[test]
    fn normal_steps_pal_averages_true_fifty_hz() {
        let mut acc = 0.0;
        let mut frames = 0u64;
        const PRESENTS: u64 = 60;
        for _ in 0..PRESENTS {
            frames += normal_steps(&mut acc, 50.0, PRESENT_HZ, false, FAST_STEPS) as u64;
        }
        assert_eq!(frames, 50, "60 presents at 50 Hz must run exactly 50 frames");
        assert!(
            acc > 0.0 && acc < 1.0,
            "credit must remain fractional, not lost or wrapped"
        );

        // Long run: mean per present must be 50/60 to within a frame's worth.
        let mut acc2 = 0.0;
        let mut total = 0u64;
        const LONG: u64 = 60_000;
        for _ in 0..LONG {
            total += normal_steps(&mut acc2, 50.0, PRESENT_HZ, false, FAST_STEPS) as u64;
        }
        let mean = total as f64 / LONG as f64;
        assert!(
            (mean - 50.0 / 60.0).abs() < 1e-3,
            "long-run mean {mean} must be 50/60"
        );
    }

    /// ★ 2026-10-22: a badly delayed present cannot make `normal_steps` spend an unbounded
    /// burst — the `max` cap bounds the frames run in a single present.
    #[test]
    fn normal_steps_caps_a_catch_up_burst() {
        // Seed a huge credit; a single call must still return at most `max`.
        let mut acc = 1e9;
        assert_eq!(normal_steps(&mut acc, 50.0, PRESENT_HZ, false, FAST_STEPS), FAST_STEPS);
        assert!(acc > 0.0, "credit beyond the cap is preserved for later presents");
    }
}
