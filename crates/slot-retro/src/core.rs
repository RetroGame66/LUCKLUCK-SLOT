use std::fmt;
use std::path::Path;

use crate::link::Link;
use crate::rumble::Rumble;

pub const GBA_W: u32 = 240;
pub const GBA_H: u32 = 160;

/// 帧缓冲的上界尺寸，单位像素。**它不是任何一台机器的帧** —— 它是这台掌机将来要跑的所有
/// 机种里最大的一帧，缓冲按它一次分配好，会话期间不再变（见 `libretro::VIDEO_BYTES` 与
/// `slot::emu` 的 `Frames::new`）。
///
/// 1024×576 装得下 PS1 的高分辨率模式（640×480）与 SFC 的宽屏（512×448），也给 NDS 双屏
/// 留了余量。**取上界而不是每帧按需分配**：分配一次 2.25 MiB 比每帧一次便宜得多，而三个
/// 缓冲共 6.75 MiB 在 1 GiB 的机器上不值得为它引入一个分配器。这一条取代了先前"缓冲就是
/// GBA 那一帧"的写法，那时 `video_refresh` 拿 `width.min(GBA_W)` 把更大的帧**静默裁掉**
/// （见 2026-10-21 的实测报告：FC/SFC 丢 6% 宽、PS1 丢 25%）。
pub const MAX_FRAME_W: u32 = 1024;
pub const MAX_FRAME_H: u32 = 576;

/// libretro `RETRO_DEVICE_ID_JOYPAD` bit order.
///
/// The names below are the standard order — B, Y, Select, Start, Up, Down, Left, Right, A, X,
/// L, R, L2, R2, L3, R3 — and every bit a machine's pad has must be stated here, because a bit
/// nothing can set is a button the game never sees pressed.
///
/// ★ **2026-10-25: Y, X, L2 and R2 were missing until MD and PS1 arrived.** The note here used
/// to read *"Y and X have no GBA equivalent, so bits 1 and 9 are never set"*, which was true of
/// the Advance and acted as a quiet cap on the whole frontend: a Mega Drive's six-button pad is
/// A/B/C + X/Y/Z (three of which land on X, Y and Z's page), and a PlayStation pad is
/// △○×□ + L1/L2/R1/R2. Neither could be reported at all while those bits had no constant.
///
/// ⚠️ **Declaring the bit is only half — `slot/src/input/mask.rs` has to be able to produce
/// it.** See `BTN_COUNT` / `PHYSICAL` there: those decide which physical buttons exist and
/// which logical bits they drive, and a bit declared here with nothing driving it reads on
/// screen exactly like a broken key.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct ButtonMask(pub u16);

impl ButtonMask {
    pub const B: u16 = 1 << 0;
    pub const Y: u16 = 1 << 1;
    pub const SELECT: u16 = 1 << 2;
    pub const START: u16 = 1 << 3;
    pub const UP: u16 = 1 << 4;
    pub const DOWN: u16 = 1 << 5;
    pub const LEFT: u16 = 1 << 6;
    pub const RIGHT: u16 = 1 << 7;
    pub const A: u16 = 1 << 8;
    pub const X: u16 = 1 << 9;
    pub const L: u16 = 1 << 10;
    pub const R: u16 = 1 << 11;
    pub const L2: u16 = 1 << 12;
    pub const R2: u16 = 1 << 13;
}

#[derive(Copy, Clone, Debug)]
pub struct AvInfo {
    pub fps: f64,
    pub sample_rate: f64,
    /// The frame the core is actually handing back, in pixels. Measured off the video callback
    /// rather than read from the geometry the core reports: it is the size the buffer really
    /// is, which is the only size anybody downstream can safely cut it at. Zero until the
    /// first frame arrives.
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub enum CoreError {
    Io(std::io::Error),
    Load(String),
    Unsupported(String),
    State(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Io(e) => write!(f, "io: {e}"),
            CoreError::Load(m) => write!(f, "load: {m}"),
            CoreError::Unsupported(m) => write!(f, "unsupported: {m}"),
            CoreError::State(m) => write!(f, "state: {m}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e)
    }
}

pub trait RetroCore: Send {
    fn load(&mut self, rom: &Path) -> Result<(), CoreError>;
    fn run_frame(&mut self, input: ButtonMask);
    /// ★ 2026-10-27：给某个手柄口喂一份按键位图 —— **0 = 本机，1..3 = 联机时的其他玩家**。
    ///
    /// 热点联机那套（每台各跑一份同一个 ROM、只交换按键）就靠它把远端按键塞进对应的口。
    /// 默认什么都不做：没有多口概念的测试替身、以及把四个口都当同一个人的核心，都不受影响。
    fn set_port_input(&mut self, _port: usize, _mask: ButtonMask) {}
    /// ★ 2026-10-21：**把核心拨回开机那一刻**（libretro 的 `retro_reset`）。CPU 回复位向量、
    /// 内存回到开机值、BIOS 重走一遍 —— 不重读 ROM、不重开会话、不动 SRAM 和存档文件。
    ///
    /// 这是游戏菜单「重开本局」唯一该做的事：先前那条路是拔卡再插回去，而插回去会先接上
    /// **拔卡那一刻的即时存档**，于是"重开"变回原样。默认实现什么都不做，只给没有核心的
    /// 测试替身用。
    fn reset(&mut self) {}
    /// `GBA_W * GBA_H * 4` bytes, little endian XRGB8888, so the byte order is B, G, R, unused.
    fn video_xrgb8888(&self) -> &[u8];
    fn take_audio(&mut self) -> Vec<i16>;
    fn serialize(&mut self) -> Result<Vec<u8>, CoreError>;
    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError>;
    fn save_ram(&self) -> Option<Vec<u8>>;
    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError>;
    fn av_info(&self) -> AvInfo;
    /// Where this core's rumble lands. A core that was never offered the interface, or one
    /// that turned it down, hands back a cell nothing ever writes.
    fn rumble(&self) -> Rumble {
        Rumble::default()
    }
    /// Where this core's serial traffic — link cable, wireless adapter — goes. Only gpSP ever
    /// answers this for real; a core that never registers libretro's netpacket interface
    /// hands back a handle nothing ever reads from or writes to, the same shape `rumble`
    /// above uses for a core with no motor.
    fn net(&self) -> Link {
        Link::default()
    }
    /// Begins a netpacket session on this core, if it carries one. `client_id` is libretro's
    /// own: 0 the host, 1 the joiner — the only two this product has (RFU supports four; see
    /// the plan for why this stays at two). A core with no serial traffic of its own — every
    /// core but gpSP — has nothing to start, so the default does nothing.
    fn start_link(&mut self, _client_id: u16) {}
    /// Once per frame: hand the core anything the transport put in its inbound queue since
    /// the last call, then let the core do its own polling. A core that never registered
    /// netpacket has nothing to drain, so the default is a no-op — only `LibretroCore` (gpSP)
    /// ever overrides this.
    fn pump_link(&mut self) {}
    /// Ends a netpacket session on this core: `start_link`'s counterpart, called once
    /// whatever was carrying the session's traffic is going away. libretro documents `stop`
    /// as OPTIONAL — unlike `start`, a spec-compliant core may leave it NULL — so a core that
    /// never offered one simply has nothing to hear this through, and the default does
    /// nothing.
    fn stop_link(&mut self) {}
    /// Apply a cheat code to the core. A no-op for cores or builds without cheat support — the
    /// only crate that ever overrides this is `LibretroCore`, and only for a core that actually
    /// exports `retro_cheat_set`. Called from the emulator thread only, like every other core
    /// entry point.
    fn set_cheat(&self, _index: u32, _enabled: bool, _code: &str) {}
    /// Clear the core's cheat list. No-op for cores/builds without cheat support. Called from the
    /// emulator thread only.
    fn cheat_reset(&self) {}
    /// Whether this core opens a `.zip` itself and wants the archive handed over whole.
    ///
    /// True for exactly one core, FBNeo: a zip is its native romset format and it reads the
    /// whole archive to find the chips inside it. Every other core is handed a path and expects
    /// a rom at it, so a zipped rom has to be unpacked by the frontend first — and unpacking
    /// FBNeo's would break it, because one file out of a romset is not a romset.
    ///
    /// Defaults to false, so a core that says nothing gets the unpacking, which is the safe
    /// direction: a core that wanted the archive and got the inner file fails loudly, while the
    /// reverse hands a core bytes it cannot read and it fails just as loudly — but only FBNeo
    /// is in the first position, and it says so.
    fn reads_zip_natively(&self) -> bool {
        false
    }
}
