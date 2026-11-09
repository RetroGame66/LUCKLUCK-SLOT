//! The operation keymap: which button (or buttons, or turbo) a game operation takes.
//!
//! This module is the one place that names any of it. `SlotState` keeps the table as plain
//! name-to-name strings — it does not know a `Btn` or a `ButtonMask`, and neither should the
//! file on the card, which is spelled the way `System/remap.txt` spells it. Turning a row into
//! the mask the core polls is the caller's job at the point of use, which is what makes a file
//! edited on a desktop degrade to the stock binding rather than to something half-wired.
//!
//! **The physical keys the sheet offers, per machine.** Most machines give `A`, `B`, `X`, `Y`,
//! `L1` and `R1`; a PlayStation adds `L2`/`R2` (eight rows); a Famicom / NeoGeo / arcade pad has
//! no shoulders, so four (`A`, `B`, `X`, `Y`). See [`physical_for`]. `Up`/`Down`/`Left`/`Right`
//! are out (a D-pad on a face button is a direction the player can no longer walk); `Start`/
//! `Select` are out (`Select` is the gesture layer's, `Start` the game's own pause); `MENU`/`Vol±`/
//! `Power`/`Lid` would hand the machine away.
//!
//! **`L2`/`R2` live on this sheet now — but only for PlayStation.** Until 2026-10-27 they were
//! the frontend's own transport only (hold to rewind / fast-forward, double-tap to latch), handled
//! in `slot_input::gesture` before the core saw them, and `game_target(L2)` was `None`. On a PS1
//! that cost the player two real buttons, so the keymap now carries `GAME_L2` / `GAME_R2`
//! (`bit12` / `bit13`, see `game_mask`) and `physical_for(System::Ps1)` adds the eighth and
//! ninth rows; `targets_for(System::Ps1)` is the only pool that offers them. Every other machine
//! still leaves `L2`/`R2` to the gesture layer: their stock binding is `NONE` and they are not in
//! that machine's candidate pool, so `remap.txt` on a non-PS1 card cannot re-point them.
//!
//! ★★ **The transport survives the change.** Bare `L2`/`R2` are still the game's on a PS1 *and*
//! `SELECT+L2` / `SELECT+R2` stay rewind / fast-forward: `Gestures::set_l2r2_to_game` makes the
//! bare press a game bit, while the `SELECT` chord arm in `gesture` runs first and keeps its own
//! meaning. The keymap row is what a *card line* can set, not what the bare press already does —
//! the two never collide because the bare press is the gesture layer's, and the row only matters
//! once a `remap.txt` line says otherwise.
//!
//! **What an operation can be is wider than the six.** Four kinds, in the order the capture
//! list walks them:
//!
//! 0. **Nothing.** The key reports nothing at all — the stock binding of `X` and `Y` on the
//!    five machines that have no such buttons, and the only way back to it everywhere.
//!
//! 1. **A game button.** `A`, `B`, `X`, `Y`, `L` or `R` — the four a GBA has plus the two bits
//!    libretro keeps for a Mega Drive's `C`/`X` and a PlayStation's `□`/`△` (see `game_mask`).
//!    The six physical keys do not exhaust them, so **the same game button may sit under more
//!    than one physical key** (`物理A = 游戏A` untouched, `物理X = 游戏A` beside it). The mask is a
//!    bitwise or and `down[]` is per *physical* button, so two keys on one button release each
//!    correctly.
//! 2. **A pair of game buttons.** `A+B`, `A+L`, `A+R`, `B+L`, `B+R`, `L+R` — one press, two
//!    bits. Written with a `+` in the file (`B=A+B`), a spelling `remap.txt` has never used,
//!    so a line in that older format still reads as the button it always was.
//! 3. **Turbo.** `TURBO_A`, `TURBO_B`, `TURBO_C`, `TURBO_D`, `TURBO_L`, `TURBO_R` — the
//!    physical key repeats the game button while held, at the rate the sheet's own foot row
//!    sets. Its own row kind rather than folded into the pairs: the mask the core polls is level
//!    state, so a repeating key has to be pulsed by something that knows the frame clock, and
//!    that is `Pad`'s job (see `slot::input::mask`), not the capture's. ★ **`TURBO_C`/`TURBO_D`
//!    (2026-11-01) are for FBNeo's four-button pads** and are only in the NeoGeo / arcade pool —
//!    a machine with a `C` and a `D` is the only one where naming them makes sense.
//!
//! **The chord table is not here.** The `SELECT` chords keep the gesture layer's own table, and
//! this module carries only the operation-key rows that the in-game keymap sheet edits.

use slot_input::Btn;

/// The spelling `System/slot.state` and `System/remap.txt` both use, one per `Btn`.
pub const BTN_NAME: [&str; 21] = [
    "UP", "DOWN", "LEFT", "RIGHT", "A", "B", "X", "Y", "L1", "R1", "L2", "R2", "START",
    "SELECT", "MENU", "VOLUP", "VOLDOWN", "POWER", "LID", "L3", "R3",
];

/// `BTN_NAME` in buttons, in the same order. The two are kept together on purpose: `Btn` has no
/// name of its own, so this is the translation table and it must not drift from the order of the
/// variants — the same bargain `BTN_COUNT` makes over in `slot::input::mask`.
const BTN_BY_NAME: [slot_input::Btn; 21] = [
    slot_input::Btn::Up,
    slot_input::Btn::Down,
    slot_input::Btn::Left,
    slot_input::Btn::Right,
    slot_input::Btn::A,
    slot_input::Btn::B,
    slot_input::Btn::X,
    slot_input::Btn::Y,
    slot_input::Btn::L1,
    slot_input::Btn::R1,
    slot_input::Btn::L2,
    slot_input::Btn::R2,
    slot_input::Btn::Start,
    slot_input::Btn::Select,
    slot_input::Btn::Menu,
    slot_input::Btn::VolUp,
    slot_input::Btn::VolDown,
    slot_input::Btn::Power,
    slot_input::Btn::Lid,
    slot_input::Btn::L3,
    slot_input::Btn::R3,
];

/// What a button is called in the file and on the screen.
pub fn name(b: slot_input::Btn) -> &'static str {
    BTN_NAME[b as usize]
}

/// The inverse of `name`, case insensitive. `None` for a spelling nobody uses, which is what
/// makes a line in a hand-edited state file a skipped entry rather than a corrupt one.
pub fn btn(s: &str) -> Option<slot_input::Btn> {
    let s = s.trim().to_ascii_uppercase();
    BTN_NAME
        .iter()
        .position(|n| *n == s)
        .map(|i| BTN_BY_NAME[i])
}

/// The six physical keys a row can be **about** — the left column, in row order.
///
/// The order is the order the rows are drawn in, and also the order `Remap::from_keymap` fills
/// its table in, so an index means the same thing to both.
pub const PHYSICAL: [slot_input::Btn; 6] = [
    slot_input::Btn::A,
    slot_input::Btn::B,
    slot_input::Btn::X,
    slot_input::Btn::Y,
    slot_input::Btn::L1,
    slot_input::Btn::R1,
];

/// ★★ 2026-10-27：改键表**按机种的行数**不同。
///
/// * **FC / NEOGEO / ARCADE** — 四行：`A B X Y`（这两类手柄没有肩键行）。
/// * **GBA / GB / GBC / SFC / MD / PCE** — 六行：`A B X Y L1 R1`。
/// * **PS1** — **八行**：`A B X Y L1 R1 L2 R2`（唯一带 L2/R2 行的机种）。
///
/// `Remap` 与键位表都按这份行数走，所以索引在「机种内」一致、`remap.txt` 不会把
/// 一个没有的键写到卡上。
pub fn physical_for(system: slot_store::System) -> &'static [slot_input::Btn] {
    use slot_store::System as S;
    match system {
        S::Fc | S::Neogeo | S::Arcade => &[
            slot_input::Btn::A,
            slot_input::Btn::B,
            slot_input::Btn::X,
            slot_input::Btn::Y,
        ],
        S::Sfc | S::Md | S::Gba | S::Gb | S::Gbc | S::Pce => &[
            slot_input::Btn::A,
            slot_input::Btn::B,
            slot_input::Btn::X,
            slot_input::Btn::Y,
            slot_input::Btn::L1,
            slot_input::Btn::R1,
        ],
        S::Ps1 => &[
            slot_input::Btn::A,
            slot_input::Btn::B,
            slot_input::Btn::X,
            slot_input::Btn::Y,
            slot_input::Btn::L1,
            slot_input::Btn::R1,
            slot_input::Btn::L2,
            slot_input::Btn::R2,
        ],
    }
}

/// How the sheet names a row: the six physical keys, then the one rate row that every turbo
/// target reads.
pub fn row_name(i: usize) -> &'static str {
    match i {
        0..=5 => name(PHYSICAL[i]),
        _ => crate::lang::TURBO_RATE,
    }
}

/// What a physical key can be set **to** — the right column, and the whole of the capture.
///
/// Every entry is what one press of the physical key makes the game see. A game button, a pair
/// of them, or a turbo row: seventeen of the things — nothing, six game buttons, six pairs
/// (the `choose(2)` over four), four turbos. A physical key's own game button is the stock binding
/// and is also on here, which is what makes "put it back" a selection rather than a second
/// control.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Target {
    /// The file key under which a line is saved, and the spelling the sheet reads back. A
    /// fixed ASCII string rather than `label`, because `label` is translated and the file is
    /// not — the same bargain `Chord::name` makes one sheet over.
    pub name: &'static str,
    /// The screen, in the language the card is set to.
    pub label: &'static str,
    /// The game bits one press of it makes, and whether those bits are pulsed rather than
    /// held. `turbo` is `false` for every kind but the turbo rows.
    pub kind: TargetKind,
}

/// What a target does to the core, and which of the four kinds it is.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TargetKind {
    /// Nothing at all. The key is dead to the game — which is what `X` and `Y` already were,
    /// and the only way back to that once one of them has been given something.
    None,
    /// One game button, held for as long as the key is.
    Game(slot_input::Btn),
    /// Two game buttons, held together.
    Pair(slot_input::Btn, slot_input::Btn),
    /// ★ 2026-10-27：three game buttons held together — the NeoGeo in-machine chords
    /// (`A+B+C` / `B+C+D`, see `lang::TARGET_ABC` / `TARGET_BCD`). One press, three bits; the
    /// mask is the same bitwise-or as a pair, just a third button in.
    Triple(slot_input::Btn, slot_input::Btn, slot_input::Btn),
    /// One game button, repeated while the key is held, at the foot row's rate.
    Turbo(slot_input::Btn),
}

impl Target {
    /// The game bits one press makes. Turbo and game buttons are the same mask here — the
    /// difference is *how* `Pad` reports them, not *which* bits.
    pub fn mask(&self) -> u16 {
        match self.kind {
            TargetKind::None => 0,
            TargetKind::Game(b) => game_mask(b),
            TargetKind::Pair(a, b) => game_mask(a) | game_mask(b),
            TargetKind::Triple(a, b, c) => game_mask(a) | game_mask(b) | game_mask(c),
            TargetKind::Turbo(b) => game_mask(b),
        }
    }
}

/// One `Btn` in libretro bits — the six game buttons a target may name, and nothing else.
/// A target that named `Start` would be a target the six-keys rule above has already refused,
/// so this is private rather than a second table to keep honest.
///
/// **Written by hand rather than read from `slot_retro`**: `slot-ui` has never heard of a core
/// (see `Cargo.toml`), and the bits are libretro's own fixed bit order — a table that
/// imported the whole `ButtonMask` type to name six of its sixteen bits would be a dependency
/// for nothing. The test compares these against `slot_retro::ButtonMask` rather than trusting it.
///
/// ★ **2026-10-25: `X` and `Y` were added for MD and PS1.** Until now this said "the four game
/// buttons a GBA has", and `X`/`Y` were not among them because an Advance has no such buttons —
/// their stock binding is `NONE` and that was the end of it. That reading capped the whole
/// frontend at a GBA pad:
///
/// * **A Mega Drive six-button pad is `A/B/C` + `X/Y/Z`**, and the machine's own `A/B/X/Y/L1/R1`
///   map straight onto libretro's `A/B/Y/X/L/R` — five of the six bits below are already the
///   Madō Pump's ABC and XYZ. `JOYPAD_X`/`JOYPAD_Y` are what was missing to make `C`/`X` reachable
///   at all when a player wants them off the shoulder keys.
/// * **A PlayStation pad is `△○×□` + `L1/L2/R1/R2`**: `○`/`×` are `JOYPAD_B`/`JOYPAD_A`,
///   `△`/`□` are `JOYPAD_Y`/`JOYPAD_X` — the two bits this table could not produce.
///
/// ⚠️ **`X` and `Y`'s *stock* binding stays `NONE` on a machine that has no such buttons.**
/// Adding the bit here only makes the target *offerable* (see `TARGETS`); what a key does when
/// the card says nothing is `stock_for`'s business, and leaving `X`/`Y` silent on an Advance is
/// what keeps a GBA game from reacting to a button it never had. A MD/PS1 player who wants
/// `C`/`△` gets it from the machine's own default — see `stock_for` (2026-10-26) — or by writing
/// the row, which is exactly what the keymap sheet is for.
fn game_mask(b: slot_input::Btn) -> u16 {
    match b {
        slot_input::Btn::A => 1 << 8,   // RETRO_DEVICE_ID_JOYPAD_A
        slot_input::Btn::B => 1 << 0,   // RETRO_DEVICE_ID_JOYPAD_B
        slot_input::Btn::X => 1 << 9,   // RETRO_DEVICE_ID_JOYPAD_X
        slot_input::Btn::Y => 1 << 1,   // RETRO_DEVICE_ID_JOYPAD_Y
        slot_input::Btn::L1 => 1 << 10, // RETRO_DEVICE_ID_JOYPAD_L
        slot_input::Btn::R1 => 1 << 11, // RETRO_DEVICE_ID_JOYPAD_R
        slot_input::Btn::L2 => 1 << 12, // RETRO_DEVICE_ID_JOYPAD_L2
        slot_input::Btn::R2 => 1 << 13, // RETRO_DEVICE_ID_JOYPAD_R2
        _ => 0,
    }
}

/// The ten game buttons, the six pairs, the two NeoGeo machines' own chords, the six turbos and
/// "nothing" — which is the only way back to `X` and `Y` as they came.
///
/// **Duplicate game buttons are the point, not a bug**: `物理A` keeps `游戏A` (its stock
/// binding, never written to the file) and `物理X` may take `游戏A` beside it — the mask is a
/// bitwise or and `Pad`'s `down[]` is per *physical* button, so two keys on one button release
/// each correctly.
///
/// ★ **2026-10-25: `GAME_X` and `GAME_Y` were added for MD and PS1** — see `game_mask` for why
/// those two bits arrived. **2026-11-02: `GAME_C` and `GAME_D` joined for NeoGeo/Arcade only**
/// (FBNeo's own C/D, bits 1/9) — the new frontend drops the old `GAME_X`/`GAME_Y` spelling on
/// that machine so a NeoGeo card reads `GAME_C`/`GAME_D` like the core does. The other four
/// machines keep `GAME_X`/`GAME_Y` (MD's Y/X, SFC's X/Y, PS1's △/□); the six pairs are
/// **deliberately not extended** — a pair list that grows with every machine's own button names
/// would turn the picker into a matrix of combinations nobody asked for. **A new machine adds
/// its own buttons to the game-button block, not to the pairs.**
pub const TARGETS: [Target; 25] = [
    // — nothing: `X`/`Y` on a two-button machine, and the only way back to it anywhere —
    Target { name: "NONE", label: crate::lang::TARGET_NONE, kind: TargetKind::None },
    // — game buttons —
    Target { name: "GAME_A", label: crate::lang::TARGET_GAME_A, kind: TargetKind::Game(slot_input::Btn::A) },
    Target { name: "GAME_B", label: crate::lang::TARGET_GAME_B, kind: TargetKind::Game(slot_input::Btn::B) },
    Target { name: "GAME_L", label: crate::lang::TARGET_GAME_L, kind: TargetKind::Game(slot_input::Btn::L1) },
    Target { name: "GAME_R", label: crate::lang::TARGET_GAME_R, kind: TargetKind::Game(slot_input::Btn::R1) },
    Target { name: "GAME_X", label: crate::lang::TARGET_GAME_X, kind: TargetKind::Game(slot_input::Btn::X) },
    Target { name: "GAME_Y", label: crate::lang::TARGET_GAME_Y, kind: TargetKind::Game(slot_input::Btn::Y) },
    // ★ 2026-11-02：NeoGeo/街机 的 C/D 文件键（位 = bit1 / bit9）。新前端，不再沿用 GAME_X/GAME_Y
    // 的兼容名：FBNeo 的 C 就是 C、D 就是 D，与同池的 TURBO_C/TURBO_D 同位、同名（「GAME_C 对应
    // TURBO_C」的字面落地）。别的机器仍用 GAME_X/GAME_Y（MD 的 Y/X、SFC 的 X/Y、PS1 的 △/□），
    // 因为它们核内就是那两个字母，与 FBNeo 的 C/D 不是同一组位。
    Target { name: "GAME_C", label: crate::lang::TARGET_GAME_C, kind: TargetKind::Game(slot_input::Btn::Y) },
    Target { name: "GAME_D", label: crate::lang::TARGET_GAME_D, kind: TargetKind::Game(slot_input::Btn::X) },
    // ★ 2026-10-27：PS1 的四颗肩键位（`L2`/`R2`，bit12/13）。只有 PS1 的候选池会出它们
    // （见 `targets_for`），别处这两颗肩键是传输（倒带/快进），不进这张表。
    Target { name: "GAME_L2", label: crate::lang::TARGET_GAME_L2, kind: TargetKind::Game(slot_input::Btn::L2) },
    Target { name: "GAME_R2", label: crate::lang::TARGET_GAME_R2, kind: TargetKind::Game(slot_input::Btn::R2) },
    // — pairs —
    Target { name: "A+B", label: crate::lang::TARGET_AB, kind: TargetKind::Pair(slot_input::Btn::A, slot_input::Btn::B) },
    Target { name: "A+L", label: crate::lang::TARGET_AL, kind: TargetKind::Pair(slot_input::Btn::A, slot_input::Btn::L1) },
    Target { name: "A+R", label: crate::lang::TARGET_AR, kind: TargetKind::Pair(slot_input::Btn::A, slot_input::Btn::R1) },
    Target { name: "B+L", label: crate::lang::TARGET_BL, kind: TargetKind::Pair(slot_input::Btn::B, slot_input::Btn::L1) },
    Target { name: "B+R", label: crate::lang::TARGET_BR, kind: TargetKind::Pair(slot_input::Btn::B, slot_input::Btn::R1) },
    Target { name: "L+R", label: crate::lang::TARGET_LR, kind: TargetKind::Pair(slot_input::Btn::L1, slot_input::Btn::R1) },
    // ★ 2026-10-27：NeoGeo/街机 的**机内三键和弦**（机内 `A+B+C` / `B+C+D`，FBNeo 官方和弦
    // 坐实：机内 A+B+C = bit0+bit8+bit1、B+C+D = bit8+bit1+bit9）。位用 libretro 位键拼出来
    // （A=B位、B=A位、C=Y位、D=X位，见 `face_label`），只有 NEOGEO/ARCADE 的候选池出这两条。
    Target {
        name: "A+B+C",
        label: crate::lang::TARGET_ABC,
        kind: TargetKind::Triple(slot_input::Btn::B, slot_input::Btn::A, slot_input::Btn::Y),
    },
    Target {
        name: "B+C+D",
        label: crate::lang::TARGET_BCD,
        kind: TargetKind::Triple(slot_input::Btn::A, slot_input::Btn::Y, slot_input::Btn::X),
    },
    // — turbo —
    Target { name: "TURBO_A", label: crate::lang::TARGET_TURBO_A, kind: TargetKind::Turbo(slot_input::Btn::A) },
    Target { name: "TURBO_B", label: crate::lang::TARGET_TURBO_B, kind: TargetKind::Turbo(slot_input::Btn::B) },
    Target { name: "TURBO_L", label: crate::lang::TARGET_TURBO_L, kind: TargetKind::Turbo(slot_input::Btn::L1) },
    Target { name: "TURBO_R", label: crate::lang::TARGET_TURBO_R, kind: TargetKind::Turbo(slot_input::Btn::R1) },
    // ★ 2026-11-01：**FBNeo 四键的 C/D。** 位 = `GAME_Y`(bit1) / `GAME_X`(bit9) —— 也就是
    // NeoGeo 手柄上的 C 和 D（见 `stock_target` 的 Neogeo 臂与 `face_label`）。只有
    // NEOGEO / ARCADE 的候选池出这两条（`targets_for`），别的机器没有第三、第四颗以外的键
    // 可以用它们命名。**追加在末尾而不是插进 turbo 段中间**：`stock_target` 用下标
    // 引用 17…20，插序会静默改掉 FC 的出厂连发键位。
    Target { name: "TURBO_C", label: crate::lang::TARGET_TURBO_C, kind: TargetKind::Turbo(slot_input::Btn::Y) },
    Target { name: "TURBO_D", label: crate::lang::TARGET_TURBO_D, kind: TargetKind::Turbo(slot_input::Btn::X) },
];

/// The target a physical key is on when the card says nothing **on this machine**. The game
/// button that shares its name — except `X` and `Y`, where the answer is the machine's.
///
/// ★ **2026-10-25：`X`/`Y` 现在是「默认静音」，不再是「没有位」。** 自从 `game_mask` 给了它们
/// `JOYPAD_X`/`JOYPAD_Y`（MD 的 `C`/`X`、PS1 的 `□`/`△`），这个默认值就从**能力上限**变成了
/// **策略选择**。
///
/// ★★ **2026-10-26（六六）：这个策略选择是「各机种」的。** 之前它是全场一个答案（`NONE`），
/// 因为那时键位表只有一份；现在每台机种自己一份，于是默认也能分开了：
///
/// * **GBA / GB / GBC / FC / PCE** — `X`/`Y` 仍是 `NONE`。这五台没有这两颗键，给它们一个
///   会报位的默认就等于"游戏被一个它没有的键戳到"。
/// * **SFC / NEOGEO / ARCADE / MD / PS1** — `X`→`GAME_X`、`Y`→`GAME_Y`。这五台真的有四键，
///   而且 libretro 的 `JOYPAD_X`/`JOYPAD_Y` 就是它们的第三/四把：SFC 的 X/Y、
///   NEOGEO·FBNeo 的 D/C、MD 的 Y/C、PS1 的 `□`/`△`。默认空着就是让玩家
///   买了个四键手柄却只能按两个键。
///
/// 注意这不改物理 A/B 的默认（各自同名的游戏键）—— 那四台机种的 A/B 位置与
/// GBA 一致，没有要修正的东西；要交换 A/B 的人在键位表里自己改。
pub fn stock_for(system: slot_store::System, phys: slot_input::Btn) -> &'static Target {
    stock_target(system, phys)
}

/// The stock binding read as an Advance's — the answer before per-machine defaults existed, and
/// still the right one wherever a caller has no machine in hand (`Remap::identity`, and the
/// sheet's own "has this row been put back?").
pub fn stock(phys: slot_input::Btn) -> &'static Target {
    stock_for(slot_store::System::Gba, phys)
}

/// See [`stock_for`]. Named separately so the doc above can be about the *what* and this one
/// about the *which*, and so every caller reads from one table.
///
/// ★★ **2026-10-27：这张表现在是「按机种」的。** 六六定的出厂键位（见
/// `slot-键位按机种重排-规格-2026-10-27`）：
///
/// * **GBA / GB / GBC / PCE** — `A`/`B`/`L1`/`R1` 同名游戏键，`X`/`Y` 静音（这两台无四键）。
/// * **FC** — `A`/`B` 同名，`X`→连发 A、`Y`→连发 B（FC 手柄只有 A/B，连发是它唯一的额外键）。
/// * **SFC / MD** — `A`/`B`/`X`/`Y`/`L1`/`R1` 全接上。MD 的按钮名是 `C`/`X`/`Y`/`Z`，但 libretro
///   位与 SFC 完全一致（`C`=bit8 `A` 位、`Y`=bit9 `X` 位、`X`=bit10 `L` 位、`Z`=bit11 `R` 位），
///   所以**默认值一样，差别只在屏幕标签**（`face_label`）。
/// * **NEOGEO / ARCADE** — 四键 `A`/`B`/`X`/`Y`：但物理 `A`→`B` 位、物理 `B`→`A` 位、
///   物理 `X`→`D` 位（=bit9 `X` 位）、物理 `Y`→`C` 位（=bit1 `Y` 位）。即「A/B 交叉、X/Y 为 D/C」。
/// * **PS1** — 八键：`A`→○(bit8)、`B`→×(bit0)、`X`→△(bit9=GAME_X)、`Y`→□(bit1=GAME_Y)、
///   `L1`/`R1`/`L2`/`R2` 各就各位。**唯一带 L2/R2 行的机种。**
fn stock_target(system: slot_store::System, phys: slot_input::Btn) -> &'static Target {
    use slot_store::System as S;
    let t = |i: usize| &TARGETS[i];
    match (system, phys) {
        // ── PS1：八行，X→△（GAME_X）Y→□（GAME_Y），L2/R2 接上 ──
        (S::Ps1, Btn::A) => t(1),  // ○
        (S::Ps1, Btn::B) => t(2),  // ×
        (S::Ps1, Btn::X) => t(5),  // △ = GAME_X
        (S::Ps1, Btn::Y) => t(6),  // □ = GAME_Y
        (S::Ps1, Btn::L1) => t(3),
        (S::Ps1, Btn::R1) => t(4),
        (S::Ps1, Btn::L2) => t(9),
        (S::Ps1, Btn::R2) => t(10),
        // ── FC：A/B + 连发 X/Y，无肩键行 ──
        (S::Fc, Btn::A) => t(1),
        (S::Fc, Btn::B) => t(2),
        (S::Fc, Btn::X) => t(19), // TURBO_A
        (S::Fc, Btn::Y) => t(20), // TURBO_B
        // ── SFC / MD：六行全接上（标签由 face_label 区分）──
        (S::Sfc | S::Md, Btn::A) => t(1),
        (S::Sfc | S::Md, Btn::B) => t(2),
        (S::Sfc | S::Md, Btn::X) => t(5),
        (S::Sfc | S::Md, Btn::Y) => t(6),
        (S::Sfc | S::Md, Btn::L1) => t(3),
        (S::Sfc | S::Md, Btn::R1) => t(4),
        // ── NEOGEO / ARCADE：四键，A/B 交叉、X/Y 为 D/C ──
        (S::Neogeo | S::Arcade, Btn::A) => t(1), // → B 位
        (S::Neogeo | S::Arcade, Btn::B) => t(2), // → A 位
        (S::Neogeo | S::Arcade, Btn::X) => t(8), // → D 位 (GAME_D, bit9)
        (S::Neogeo | S::Arcade, Btn::Y) => t(7), // → C 位 (GAME_C, bit1)
        // ── GBA / GB / GBC / PCE：六行，X/Y 静音 ──
        (S::Gba | S::Gb | S::Gbc | S::Pce, Btn::A) => t(1),
        (S::Gba | S::Gb | S::Gbc | S::Pce, Btn::B) => t(2),
        (S::Gba | S::Gb | S::Gbc | S::Pce, Btn::L1) => t(3),
        (S::Gba | S::Gb | S::Gbc | S::Pce, Btn::R1) => t(4),
        // 其余一律静音（含两键机种的 X/Y/L1/R1、所有机的 L2/R2 无行）。
        _ => t(0),
    }
}

/// ★★ 2026-10-27：改键**候选池按机种裁剪**。
///
/// 默认是全池去掉 L2/R2（这两颗肩键只在 PS1 候选里）；按六六定的规格：
///
/// * **FC** — 只有 `A`/`B` 和连发，没有肩键、没有 `X`/`Y` 游戏键：候选 = 静音 + `A`/`B` + 连发 A/B + `A+B`。
/// * **NEOGEO / ARCADE** — `A`/`B`/`C`/`D` 四键 + 两条机内三键和弦（`A+B+C` / `B+C+D`）
///   + ★ 2026-11-01 的**四条连发**（`TURBO_A`…`TURBO_D`）。街机的射击连打、蓄力、逃命键
///   全在这四颗上，少一颗就是少一条命。
/// * **PS1** — 八颗（含 `L2`/`R2`），无组合对/连发。
/// * **其余** — 全池（NONE + 10 游戏键 + 6 组合对 + 4 连发），去掉 L2/R2。
///
/// 池子里的顺序**按文件键（也就是位）排，不按屏幕上的名字** —— NeoGeo 的屏幕名是
/// 「B, A, C, D / 连发 B, 连发 A, 连发 C, 连发 D」（见 `face_label`），把它按字母重排会让
/// 下标与 `TARGETS` 对不上，而这张表的根本排序规则就是位。
pub fn targets_for(system: slot_store::System) -> &'static [&'static Target] {
    use slot_store::System as S;
    const FC: [&'static Target; 6] = [
        &TARGETS[0], &TARGETS[1], &TARGETS[2], &TARGETS[19], &TARGETS[20], &TARGETS[11],
    ];
    const NEO: [&'static Target; 11] = [
        &TARGETS[0], &TARGETS[1], &TARGETS[2], &TARGETS[7], &TARGETS[8], &TARGETS[17],
        &TARGETS[18], &TARGETS[19], &TARGETS[20], &TARGETS[23], &TARGETS[24],
    ];
    const PS1: [&'static Target; 9] = [
        &TARGETS[0], &TARGETS[1], &TARGETS[2], &TARGETS[3], &TARGETS[4], &TARGETS[5], &TARGETS[6],
        &TARGETS[9], &TARGETS[10],
    ];
    const DEF: [&'static Target; 17] = [
        &TARGETS[0], &TARGETS[1], &TARGETS[2], &TARGETS[3], &TARGETS[4], &TARGETS[5], &TARGETS[6],
        &TARGETS[11], &TARGETS[12], &TARGETS[13], &TARGETS[14], &TARGETS[15], &TARGETS[16],
        &TARGETS[19], &TARGETS[20], &TARGETS[21], &TARGETS[22],
    ];
    match system {
        S::Fc => &FC,
        S::Neogeo | S::Arcade => &NEO,
        S::Ps1 => &PS1,
        _ => &DEF,
    }
}

/// ★★ 2026-10-27：改键表里**游戏键的屏幕名字按机种翻译**。
///
/// ★★ **2026-11-01（六六定的规则）：「涉及核心键位排列与物理键位不符合的一律以核心键位为标准。」**
/// 这条规则是本函数存在的全部理由，也是它唯一的判据：屏幕上印的字必须是**核心自己报的按键
/// 排列**，不是掌机上印的那几个字母。所以 FBNeo 的 bit8 读 `B`（不是掌机的 `A`）、MD 的 bit8
/// 读 `C`、PS1 的 bit8 读 `Circle`。**连发目标照同一条规则走** —— `TURBO_A` 在 NeoGeo 上读
/// 「连发 B」、在 MD 上读「连发 C」，因为它连发的就是那颗键。`GAME_C 对应 TURBO_C` 也是这条
/// 规则的推论：文件键名是 libretro 位（稳定、可写进卡），屏幕名是核心的（准确、玩家认得出）。
///
/// 文件键（`GAME_A`…）是 libretro 位，稳定不变；但同一颗位在不同机种上印的字不同：
///
/// * **MD**：`C`/`X`/`Y`/`Z`（位仍是 `A`/`B`/`X`/`Y`/`L`/`R`）。
/// * **NEOGEO / ARCADE**：`A`/`B`/`C`/`D`（位 = `B`/`A`/`Y`/`X`）。★ 2026-11-01：**四条连发
///   也跟着换名字** —— `TURBO_A` 是 bit8，这一台上那颗键印的是 `B`，所以它读作「连发 B」。
///   不换的话玩家会挑一个写着 `A` 的选项，然后看见 `B` 键自己在连打。
/// * **PS1**：`Circle`/`Cross`/`Triangle`/`Square`（位 = `A`/`B`/`X`/`Y`；标签面无 `○△□` 字形，
///   用国际按键名）。
///
/// 不在上表的机种/键，退回该 `Target` 自身的 `label`（即「Game A」那种）。
pub fn face_label(system: slot_store::System, name: &str) -> &str {
    use slot_store::System as S;
    match (system, name) {
        (S::Md, "GAME_A") => "C",
        (S::Md, "GAME_B") => "B",
        (S::Md, "GAME_X") => "Y",
        (S::Md, "GAME_Y") => "A",
        (S::Md, "GAME_L") => "X",
        (S::Md, "GAME_R") => "Z",
        // ★ 2026-11-01：MD 的四条连发（DEF 池里有 `TURBO_A/B/L/R`）。位对过去是
        // bit8=C / bit0=B / bit10=X / bit11=Z —— 与上面 `GAME_*` 那六行同一张表，
        // 所以玩家在两个栏里看到的是同一套字母。
        (S::Md, "TURBO_A") => crate::lang::TARGET_TURBO_C,
        (S::Md, "TURBO_B") => crate::lang::TARGET_TURBO_B,
        (S::Md, "TURBO_L") => crate::lang::TARGET_TURBO_MD_X,
        (S::Md, "TURBO_R") => crate::lang::TARGET_TURBO_MD_Z,
        (S::Neogeo | S::Arcade, "GAME_A") => "B",
        (S::Neogeo | S::Arcade, "GAME_B") => "A",
        // ★ 2026-11-02：NeoGeo 的 C/D 现在就是文件键 GAME_C/GAME_D（与 TURBO_C/TURBO_D 同位），
        // 没了 GAME_X/GAME_Y 的兼容名，退回 label 即「C」/「D」，与核心排列一致。
        // ★ 2026-11-01：连发与它真正连发的那颗键同名牌（`TURBO_A`→「连发 B」）。`C`/`D` 两条
        // 是恒等映射，写出来是为了让这四行并排可读，也免得以后只改了一半的文案。
        (S::Neogeo | S::Arcade, "TURBO_A") => crate::lang::TARGET_TURBO_B,
        (S::Neogeo | S::Arcade, "TURBO_B") => crate::lang::TARGET_TURBO_A,
        (S::Neogeo | S::Arcade, "TURBO_C") => crate::lang::TARGET_TURBO_C,
        (S::Neogeo | S::Arcade, "TURBO_D") => crate::lang::TARGET_TURBO_D,
        (S::Ps1, "GAME_A") => crate::lang::TARGET_PS1_CIRCLE,
        (S::Ps1, "GAME_B") => crate::lang::TARGET_PS1_CROSS,
        (S::Ps1, "GAME_X") => crate::lang::TARGET_PS1_TRIANGLE,
        (S::Ps1, "GAME_Y") => crate::lang::TARGET_PS1_SQUARE,
        (S::Ps1, "GAME_L") => crate::lang::TARGET_PS1_L1,
        (S::Ps1, "GAME_R") => crate::lang::TARGET_PS1_R1,
        (S::Ps1, "GAME_L2") => crate::lang::TARGET_PS1_L2,
        (S::Ps1, "GAME_R2") => crate::lang::TARGET_PS1_R2,
        _ => target_by_name(name).map(|t| t.label).unwrap_or(name),
    }
}

/// Which target a physical key is currently on **on this machine**. A key with no line is on
/// its stock binding — see [`stock_for`] for what that is per machine — so the fallback and the
/// default are one thing, and a hand-edited file that has been made nonsense reads as the
/// buttons the game always had.
pub fn current_for(
    system: slot_store::System,
    phys: slot_input::Btn,
    map: &std::collections::BTreeMap<String, String>,
) -> &'static Target {
    let own = name(phys);
    match map.get(own) {
        Some(v) => target_by_name(v).unwrap_or_else(|| stock_target(system, phys)),
        None => stock_target(system, phys),
    }
}

/// [`current_for`] read as an Advance's. Kept for the callers that have no machine in hand.
pub fn current(
    phys: slot_input::Btn,
    map: &std::collections::BTreeMap<String, String>,
) -> &'static Target {
    current_for(slot_store::System::Gba, phys, map)
}

/// The target that is **exactly one game button and nothing else**, for a file that names a
/// button rather than a target. `System/remap.txt` is such a file: it spells `A=B`, and has
/// done since long before pairs and turbos existed.
///
/// `None` for a button the core has no bit for — `START`, `SELECT`, the D-pad, `X`, `Y` and the
/// machine keys. **Skipped rather than turned into silence**: a `remap.txt` line saying
/// `START=START` is a line nobody meant anything by, and reading it as "report nothing" would
/// be a table that broke a button a file had every right to name.
pub fn game_target(b: slot_input::Btn) -> Option<&'static Target> {
    TARGETS
        .iter()
        .find(|t| matches!(t.kind, TargetKind::Game(g) if g == b))
}

/// One target, by its file name. `None` for a spelling nobody uses — a skipped line, not a
/// corrupt file.
pub fn target_by_name(s: &str) -> Option<&'static Target> {
    let s = s.trim().to_ascii_uppercase();
    TARGETS.iter().find(|t| t.name == s)
}

/// The two columns of the operation keymap screen, in row order: the physical key, and what it
/// currently does **on this machine**. The row count is `physical_for(system).len()`.
pub fn rows_for(
    system: slot_store::System,
    map: &std::collections::BTreeMap<String, String>,
) -> Vec<(slot_input::Btn, &'static Target)> {
    physical_for(system)
        .iter()
        .map(|&p| (p, current_for(system, p, map)))
        .collect()
}

/// [`rows_for`] read as an Advance's.
pub fn rows(
    map: &std::collections::BTreeMap<String, String>,
) -> Vec<(slot_input::Btn, &'static Target)> {
    rows_for(slot_store::System::Gba, map)
}

// ── The turbo rate ──────────────────────────────────────────────────────────────────────────

/// The rates a turbo repeats at, in Hz, in the order the foot row's `←`/`→` walks them.
///
/// **Read out of `slot-store` rather than written here**, the way `DOZE_MIN_CHOICES` and the two
/// speed lists are: the choices *are* the setting, so the card's own file and the sheet have to
/// be reading one list. `slot-ui` depends on `slot-store`; the reverse is not true and must not
/// become true.
pub use slot_store::TURBO_HZ_CHOICES as TURBO_HZ;

/// The rate a card that has never been to this screen repeats at — the middle of the list.
pub use slot_store::TURBO_HZ_DEFAULT;

/// The turbo rate, as the card has it. A rate not on the list is the default, the same
/// clamp-and-fallback every other list on the settings screen makes.
pub fn turbo_hz(hz: u8) -> u8 {
    if TURBO_HZ.contains(&hz) {
        hz
    } else {
        TURBO_HZ_DEFAULT
    }
}
