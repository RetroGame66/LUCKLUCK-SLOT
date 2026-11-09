use std::collections::BTreeMap;

use slot_input::{Action, Btn, Millis};
use slot_retro::ButtonMask;
use slot_store::System;
use slot_ui::keys;

/// One past the last `Btn` discriminant. The physical-button state is held as a fixed array
/// indexed by `Btn as usize`, so this must match the number of variants in `slot_input::Btn`.
const BTN_COUNT: usize = 21;

/// The physical keys the operation keymap offers rows for, **per machine**, as `Btn as usize`.
/// **In that order** — the same order `keys::physical_for` walks them — so an index means the same
/// thing to the sheet and to the mask. Only these ever read a target off the card; the other
/// buttons keep the identity this file had before the keymap existed.
///
/// ★★ 2026-10-27：从固定六键改成按机种行数（PS1 八键、FC/NEO/ARCADE 四键）。
fn physical_for(system: System) -> &'static [usize] {
    use System::*;
    match system {
        Fc | Neogeo | Arcade => &[
            Btn::A as usize, Btn::B as usize, Btn::X as usize, Btn::Y as usize,
        ],
        Sfc | Md | Gba | Gb | Gbc | Pce => &[
            Btn::A as usize, Btn::B as usize, Btn::X as usize, Btn::Y as usize,
            Btn::L1 as usize, Btn::R1 as usize,
        ],
        Ps1 => &[
            Btn::A as usize, Btn::B as usize, Btn::X as usize, Btn::Y as usize,
            Btn::L1 as usize, Btn::R1 as usize, Btn::L2 as usize, Btn::R2 as usize,
        ],
    }
}

/// The button an index in the fixed arrays names, in `Btn`'s own discriminant order. **The
/// arrays are indexed by `Btn as usize`**, so this and that order must never drift — the same
/// bargain `BTN_COUNT` makes at the top of this file.
fn btn_at(i: usize) -> Btn {
    match i {
        0 => Btn::Up,
        1 => Btn::Down,
        2 => Btn::Left,
        3 => Btn::Right,
        4 => Btn::A,
        5 => Btn::B,
        6 => Btn::X,
        7 => Btn::Y,
        8 => Btn::L1,
        9 => Btn::R1,
        10 => Btn::L2,
        11 => Btn::R2,
        12 => Btn::Start,
        13 => Btn::Select,
        14 => Btn::Menu,
        15 => Btn::VolUp,
        16 => Btn::VolDown,
        17 => Btn::Power,
        18 => Btn::Lid,
        19 => Btn::L3,
        20 => Btn::R3,
        _ => Btn::Lid,
    }
}

/// The bits the legacy (non-keymap) buttons report to the core: the four the old `Remap` reached
/// with a bit, and zero for everything it did not. This is what `A = A`, `Start = Start`, …
/// meant before the keymap existed, and it is the value the thirteen untouched buttons keep.
fn legacy_bit(b: Btn) -> u16 {
    match b {
        Btn::Up => ButtonMask::UP,
        Btn::Down => ButtonMask::DOWN,
        Btn::Left => ButtonMask::LEFT,
        Btn::Right => ButtonMask::RIGHT,
        Btn::Start => ButtonMask::START,
        Btn::Select => ButtonMask::SELECT,
        _ => 0,
    }
}

/// What a physical key drives into the core, one of four kinds — the same four `keys::TargetKind`
/// spells. The mask layer only needs to know two things about a target: **which bits a press
/// makes** (`Target::mask` gives it, and pairs fold to the bitwise or the game already sees)
/// and **whether it is a turbo row** (pulsed rather than held). Those are the `bits` and
/// `pulse` arrays below.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Remap {
    /// The game bits one press of this physical key makes, or zero for a silent key.
    bits: [u16; BTN_COUNT],
    /// Which of the six are turbo rows, pulsing `bits[i]` at `turbo_hz` while held.
    pulse: [bool; BTN_COUNT],
    /// The rate a turbo key repeats at, in Hz, clamped to `keys::TURBO_HZ`.
    turbo_hz: u8,
}

impl Remap {
    /// Every physical button reports as itself, and the keymap keys are on their stock
    /// binding for `system`: `A`/`B`/`L1`/`R1` the game button that shares their name, `X`/`Y`
    /// silent on a two-button machine, crossed on a NeoGeo, the full eight on a PlayStation.
    /// **This is the whole of "the card never opened the keymap sheet"** — a `from_keymap` on an
    /// empty table reads back exactly this, which is what makes "put it back" a selection
    /// rather than a second control.
    pub fn identity(system: System) -> Self {
        Self::from_keymap(system, &BTreeMap::new(), keys::TURBO_HZ_DEFAULT)
    }

    /// Build the table from what the card carries: one target name per physical key, the rate on
    /// the foot row. A key with no line falls back to its stock binding **for `system`** (see
    /// `keys::stock_for`), and a rate not on the list clamps to the default — so a hand-edited
    /// card that has been made nonsense degrades to the buttons the game always had, never to
    /// something half-wired.
    pub fn from_keymap(system: System, map: &BTreeMap<String, String>, turbo_hz: u8) -> Self {
        let phys = physical_for(system);
        let mut bits = [0u16; BTN_COUNT];
        let mut pulse = [false; BTN_COUNT];
        // The buttons not on the keymap keep their identity from before the keymap existed.
        for (i, b) in bits.iter_mut().enumerate() {
            if phys.contains(&i) {
                continue;
            }
            let legacy = legacy_bit(btn_at(i));
            if legacy != 0 {
                *b = legacy;
            }
        }
        // And the keymap keys read their target off the card, falling back to this machine's stock.
        for &i in phys {
            let b = btn_at(i);
            let target = map
                .get(keys::name(b))
                .and_then(|v| keys::target_by_name(v))
                .unwrap_or_else(|| keys::stock_for(system, b));
            bits[i] = target.mask();
            pulse[i] = matches!(target.kind, keys::TargetKind::Turbo(_));
        }
        Self {
            bits,
            pulse,
            turbo_hz: keys::turbo_hz(turbo_hz),
        }
    }
}

impl Default for Remap {
    fn default() -> Self {
        Remap::identity(System::Gba)
    }
}

/// The buttons the gesture layer let through to the game, held as a libretro mask. It is
/// level state, not edges, because that is what the core is polled for every frame.
pub struct Pad {
    /// Per-physical-button down state. Recomputed into the mask each frame (see `mask`) so that
    /// two physical keys mapped onto the same game button both release correctly.
    down: [bool; BTN_COUNT],
    remap: Remap,
    /// The turbo rows' own clock. `phase` accumulates the time a turbo key has been held, and
    /// `on` is whether its bit is currently part of the mask — on for half the period, off for
    /// half, which is what "repeats at the rate" means. Held per physical key so a turbo `A` and
    /// a held `B` under two keys each step on their own clock.
    phase: [u32; BTN_COUNT],
    on: [bool; BTN_COUNT],
    /// The wall clock `step` counts against, so a gap (a doze, a switcher) cannot land a turbo
    /// mid-phase and hold it there.
    last: Millis,
    /// ★★ 2026-10-25：**`L2`/`R2` 报不报给核心。** 这两根肩键不在这张 remap 表的六键里
    /// （`legacy_bit` 给它们 0），所以它们从来没有位；PS1 需要它们是**游戏按键**，于是这里
    /// 开一条**独立于 remap 表**的通道：为真时，`down[L2]`/`down[R2]` 各自多报
    /// `ButtonMask::L2`/`R2`。为假时一个位都不多 —— GBA/GB/GBC/SFC/FC/MD 的掩码逐字节不变。
    ///
    /// **为什么不塞进 `remap.bits`**：那张表是卡上 `remap.txt` 与键位表读的东西，把位放进
    /// 去就等于"这两根肩键随时可以被重映射"，而它们只有在一局 PS1 里才是游戏的。开关比
    /// 表项更能说清这件事。
    shoulders: bool,
}

impl Pad {
    pub fn new(remap: Remap) -> Self {
        Pad {
            down: [false; BTN_COUNT],
            remap,
            phase: [0; BTN_COUNT],
            on: [false; BTN_COUNT],
            last: 0,
            shoulders: false,
        }
    }

    pub fn with_remap(remap: Remap) -> Self {
        Self::new(remap)
    }

    /// Whether a bare `L2`/`R2` is the game's to see. See the field: `Session` sets this every
    /// frame from the machine in the slot, and only a PlayStation asks for it.
    pub fn set_shoulders(&mut self, on: bool) {
        self.shoulders = on;
    }

    /// The table the pad is currently remapping the six keys through, for a caller that wants to
    /// see what it is on before deciding to replace it.
    pub fn remap(&self) -> Remap {
        self.remap
    }

    /// Swap in a new table — the live path a saved keymap takes to reach the core without a
    /// reboot. The `down` state is kept: a remap is which bits a held key makes, not which keys
    /// are held, and replacing the two in the same breath would drop a press the player is
    /// still making.
    pub fn set_remap(&mut self, remap: Remap) {
        self.remap = remap;
        // A key that is no longer a turbo row is not pulsed any more.
        for i in 0..BTN_COUNT {
            if !remap.pulse[i] {
                self.on[i] = false;
                self.phase[i] = 0;
            }
        }
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::GbaDown(b) => {
                self.down[b as usize] = true;
                // A turbo key that lands is on immediately — the first press is real, and
                // `step` is what starts counting from here.
                if self.remap.pulse[b as usize] {
                    self.on[b as usize] = true;
                    self.phase[b as usize] = 0;
                }
            }
            Action::GbaUp(b) => {
                self.down[b as usize] = false;
                self.on[b as usize] = false;
                self.phase[b as usize] = 0;
            }
            _ => {}
        }
    }

    /// Walk the six keys' turbo clocks forward by the frame that just passed, flipping each held
    /// turbo between its on and off half. **Called every frame whether or not anything was
    /// pressed**, the same way the gesture windows are: a turbo row is a thing that runs on the
    /// frame clock, not on the edge clock, so it has to tick on a press-less frame or the
    /// on/off would freeze at whatever it was at the last press.
    pub fn step(&mut self, now: Millis) {
        let dt = now.saturating_sub(self.last);
        self.last = now;
        // The rate a held key alternates at, in whole frames of `1/turbo_hz` each. A period the
        // device clock cannot resolve clamps to the half the shortest one offers rather than
        // stalling the pulse.
        let hz = self.remap.turbo_hz.max(1) as u32;
        let half_ms = (1000 / hz / 2).max(1);
        for i in 0..BTN_COUNT {
            if self.down[i] && self.remap.pulse[i] {
                self.phase[i] = self.phase[i].saturating_add(dt as u32);
                if self.phase[i] >= half_ms {
                    self.phase[i] = 0;
                    self.on[i] = !self.on[i];
                }
            }
        }
    }

    /// The mask the core is polled with this frame: every held physical key contributes its bits,
    /// a turbo row contributing only on the frames its clock has it lit.
    pub fn mask(&self) -> ButtonMask {
        let mut m = 0u16;
        for i in 0..BTN_COUNT {
            if !self.down[i] {
                continue;
            }
            if self.remap.pulse[i] {
                if self.on[i] {
                    m |= self.remap.bits[i];
                }
            } else {
                m |= self.remap.bits[i];
            }
        }
        // The two shoulders, on their own channel — see `shoulders`. Zero when the machine
        // does not want them, so a GBA/GBC/MD mask is exactly the bits it was before.
        if self.shoulders {
            if self.down[Btn::L2 as usize] {
                m |= ButtonMask::L2;
            }
            if self.down[Btn::R2 as usize] {
                m |= ButtonMask::R2;
            }
        }
        ButtonMask(m)
    }

    /// Buttons pressed for the switcher are not the game's. Without this the game resumes
    /// holding whatever was down when the switcher took the input.
    pub fn clear(&mut self) {
        self.down = [false; BTN_COUNT];
        self.on = [false; BTN_COUNT];
        self.phase = [0; BTN_COUNT];
    }
}

impl Default for Pad {
    fn default() -> Self {
        Pad::new(Remap::identity(System::Gba))
    }
}
