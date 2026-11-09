//! The evdev wire format and the codes the device's buttons arrive as. No I/O beyond
//! reading the sysfs tree that says which node is which, so all of it is testable off
//! device.

use std::path::{Path, PathBuf};

use slot_input::{Btn, RawEvent};

/// `struct input_event`: a 64 bit timeval, then type, code and value. The kernel writes
/// these in the machine's own endianness; every target slot builds for is little endian, and
/// pinning that is what lets the decode be tested away from the device.
pub const EVENT_BYTES: usize = 24;

/// The frame marker the kernel ends every group of edges with.
pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_SW: u16 = 0x05;
pub const SW_LID: u16 = 0x00;

pub const EV_ABS: u16 = 0x03;
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;
/// The analog stick. A stick is its own pair of axes, distinct from the d-pad's hat axes:
/// `ABS_X/Y` for the first stick, `ABS_RX/RY` for a second. See `Stick`.
pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
/// Linux `ABS_Z`: 0x02. Not a menu axis on the H700 boards seen so far (the sticks report on
/// `ABS_RX/RY`), so it is left unnamed here rather than wired into `Stick`.
#[allow(dead_code)]
pub const ABS_Z: u16 = 0x02;
pub const ABS_RX: u16 = 0x03;
pub const ABS_RY: u16 = 0x04;
/// Linux `ABS_RZ`: 0x05 — the other half of the 0x02 pair, likewise not a menu axis here.
#[allow(dead_code)]
pub const ABS_RZ: u16 = 0x05;

/// The gamepad axes worth opening a node for. The d-pad is `ABS_HAT0X/Y` (see `Hat`); a stick
/// that a board wires as its own axes shows up as `ABS_X/Y` (or `ABS_RX/RY` for a second one).
/// Deliberately the d-pad + the first stick pair only: anything in the pressure / multitouch
/// range is left for `pick_devices` to pass over, so a stray node is not opened.
pub const WANTED_ABS: [u16; 14] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, // ABS_X Y Z RX RY RZ
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, // ABS_HAT0..3 X/Y
];

/// The spellings the trace prints for an axis code. `None` for one this table does not know,
/// which the trace then prints as `abs_<hex>` so a bring-up still sees the number.
pub fn abs_axis_name(code: u16) -> Option<&'static str> {
    Some(match code {
        0x00 => "abs_x",
        0x01 => "abs_y",
        0x02 => "abs_z",
        0x03 => "abs_rx",
        0x04 => "abs_ry",
        0x05 => "abs_rz",
        0x10 => "hat0x",
        0x11 => "hat0y",
        0x12 => "hat0x2",
        0x13 => "hat0y2",
        0x14 => "hat1x",
        0x15 => "hat1y",
        0x16 => "hat2x",
        0x17 => "hat2y",
        _ => return None,
    })
}

/// The d-pad, which is two axes on hat 0 rather than four keys. A zero releases whichever
/// direction the axis was last at, and only the axis knows which that was, so this is the one
/// part of the decode that has to remember anything.
#[derive(Default)]
pub struct Hat {
    x: i32,
    y: i32,
}

impl Hat {
    /// Up to two events. A thumb rolled around the pivot swings the axis end to end without
    /// stopping in the middle, and the direction being left has to be released or it stays
    /// down for as long as the device is on.
    pub fn feed(&mut self, ev: Ev) -> Vec<RawEvent> {
        let (held, ends) = match ev.code {
            ABS_HAT0X => (&mut self.x, [Btn::Left, Btn::Right]),
            ABS_HAT0Y => (&mut self.y, [Btn::Up, Btn::Down]),
            _ => return Vec::new(),
        };
        let (was, now) = (*held, ev.value.signum());
        if now == was {
            return Vec::new();
        }
        *held = now;
        let btn = |v: i32| ends[usize::from(v > 0)];
        let mut out = Vec::new();
        if was != 0 {
            out.push(RawEvent::Up(btn(was)));
        }
        if now != 0 {
            out.push(RawEvent::Down(btn(now)));
        }
        out
    }
}

/// Full-scale of an analog stick axis, **measured**, not assumed. The kernel default for a
/// joystick is signed 16-bit centred on 0 (±32767), but the H700 board here (RG34XXSP) reports
/// **±4096 centred on 0** — see the stick-probe log of 2026-11-02. With the old 32767 the press
/// threshold sat at 10922, above the stick's own maximum, so the stick could never move the
/// menus. A board reporting a different range sets these from its own dump.
const STICK_RANGE: i32 = 4096;
/// A press registers past one third of full-scale; it releases only once back inside one sixth,
/// so a stick that rests a little off centre does not chatter the menu.
const STICK_EDGE: i32 = STICK_RANGE / 3;
const STICK_DEAD: i32 = STICK_RANGE / 6;
const STICK_CENTER: i32 = 0;

/// Re-maps a device's raw `ABS_*` axis code into the layout `Stick` expects: `ABS_X/Y` is the
/// first stick (left) and `ABS_RX/RY` the second (right). `Stick` is written for that layout and
/// must not change, so a board that wires its sticks elsewhere is translated here instead.
///
/// Most H700 boards report the sticks as `ABS_X/Y` + `ABS_RX/RY` (see `Stick`) and use
/// `IDENTITY`. The RG34XXSP (see the `stick-probe` log of 2026-11-03) reports the sticks as
/// `ABS_Z/RX/RY/RZ` with **no `ABS_X/Y` at all** — and not in the usual `Z/RZ` + `RX/RY` pairing:
/// its left stick is `Z`(horizontal) + `RX`(vertical) and its right stick is `RY`(horizontal) +
/// `RZ`(vertical). Without the remap those four axes land on `ABS_X|RX → horizontal` and
/// `ABS_Y|RY → vertical` the wrong way round, and `Z/RZ` are dropped entirely — which is exactly
/// the "left/right dead, up/down swapped" report from that board. The remap below puts each axis
/// back where `Stick` looks for it. The directional polarity is untouched (the device already
/// signs left/right and up/down the same way `Stick` does), so only the code is rewritten.
pub struct AxisMap(pub [u16; 6]);
impl AxisMap {
    /// No translation: `X→X, Y→Y, Z→Z, RX→RX, RY→RY, RZ→RZ`.
    pub const IDENTITY: AxisMap = AxisMap([0, 1, 2, 3, 4, 5]);
    /// RG34XXSP: `Z→X`(left horizontal), `RX→Y`(left vertical), `RY→RX`(right horizontal),
    /// `RZ→RY`(right vertical). Codes outside `0..=5` pass through unchanged.
    pub const RG34XXSP: AxisMap = AxisMap([0, 1, 0, 1, 3, 4]);
    /// Translate a raw axis code. Codes outside the table range are returned as-is so the d-pad
    /// hat axes (`ABS_HAT0X/Y`) and anything else are never touched.
    pub fn map(&self, code: u16) -> u16 {
        if (code as usize) < self.0.len() {
            self.0[code as usize]
        } else {
            code
        }
    }
}

/// An analog stick. The d-pad is a hat (digital, two axes on hat 0); a stick is its own pair of
/// axes (`ABS_X/Y`, or `ABS_RX/RY` for a second one) whose value is a continuous magnitude. It
/// is mapped onto the same four `Btn` directions as the d-pad so a H700 board's stick drives the
/// menus without per-model code, but because the value is analog the decode has to remember which
/// direction is held and release it when the axis returns to centre.
#[derive(Default)]
pub struct Stick {
    /// Held sign per axis: `[-1, 0, +1]` for `{x, y}`. `0` means nothing is down on that axis.
    held: [i8; 2],
}

impl Stick {
    /// Up to two events, like `Hat`. Hysteresis between `STICK_EDGE` and `STICK_DEAD` stops a
    /// stick resting just off centre from chattering, and rolling to the other side releases the
    /// old direction before it presses the new one.
    pub fn feed(&mut self, ev: Ev) -> Vec<RawEvent> {
        let axis = match ev.code {
            ABS_X | ABS_RX => 0,
            ABS_Y | ABS_RY => 1,
            _ => return Vec::new(),
        };
        let raw = ev.value - STICK_CENTER;
        let mag = raw.unsigned_abs();
        let sign = raw.signum();
        let dirs = [[Btn::Left, Btn::Right], [Btn::Up, Btn::Down]];
        let held = &mut self.held[axis];
        let mut out = Vec::new();
        // Release when the axis falls back into the dead zone, or swings to the other side.
        if mag <= STICK_DEAD as u32 || sign != i32::from(*held) {
            if *held != 0 {
                out.push(RawEvent::Up(dirs[axis][usize::from(*held > 0)]));
                *held = 0;
            }
        }
        // Press once the magnitude clears the edge and it is not already held that way.
        if mag >= STICK_EDGE as u32 && sign != 0 && sign != i32::from(*held) {
            out.push(RawEvent::Down(dirs[axis][usize::from(sign > 0)]));
            *held = sign as i8;
        }
        out
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Ev {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

pub fn decode(bytes: &[u8]) -> Option<Ev> {
    let packet: &[u8; EVENT_BYTES] = bytes.get(..EVENT_BYTES)?.try_into().ok()?;
    Some(Ev {
        kind: u16::from_le_bytes([packet[16], packet[17]]),
        code: u16::from_le_bytes([packet[18], packet[19]]),
        value: i32::from_le_bytes([packet[20], packet[21], packet[22], packet[23]]),
    })
}

pub fn to_raw(ev: Ev) -> Option<RawEvent> {
    let btn = match ev.kind {
        EV_KEY => code_to_btn(ev.code)?,
        // The hinge is a switch, and its "code" collides with a key the pad may also send.
        EV_SW if ev.code == SW_LID => Btn::Lid,
        _ => return None,
    };
    match ev.value {
        0 => Some(RawEvent::Up(btn)),
        1 => Some(RawEvent::Down(btn)),
        // 2 is autorepeat: presses with no release, which would re-arm every hold and double
        // tap window in the gesture layer.
        _ => None,
    }
}

/// Read off the device with `SLOT_TRACE_INPUT`, pressing the buttons in a known order. The
/// board does not use the kernel's names for its own layout — `0x137` is `BTN_TR` in the
/// headers and START on the case — so these are transcribed rather than reasoned about. The
/// directions are absent because they are not keys here at all; see `Hat`.
pub fn code_to_btn(code: u16) -> Option<Btn> {
    Some(match code {
        0x130 => Btn::A,
        0x131 => Btn::B,
        0x132 => Btn::Y,
        0x133 => Btn::X,
        0x134 => Btn::L1,
        0x135 => Btn::R1,
        0x136 => Btn::Select,
        0x137 => Btn::Start,
        0x138 => Btn::Menu,
        0x13a => Btn::L2,
        0x13b => Btn::R2,
        115 => Btn::VolUp,
        114 => Btn::VolDown,
        116 => Btn::Power,
        // ★★ 2026-11-03（六六 + 玩家探针）：**这块板的摇杆按下不用标准的 `BTN_THUMBL/THUMBR`。**
        //   玩家日志 `stick-probe (2).log` 实测：左右摇杆各按住 2 秒，发出的是 **0x139** 与
        //   **0x13c**，整份日志里没有 0x13d/0x13e。而 RG SP 的 pad 能力位图
        //   （`{1,114,115,304–312,314,315,354}`）里连 313/316 都没有 ⇒ 这两个码本来就只属于
        //   带摇杆的机器，正是"多出来的键"。别被 `BTN_TR2`/`BTN_MODE` 这些内核名带到别处去：
        //   这块板的内核命名与物理按键无关（`0x137`=BTN_TR 实为 START）。
        //   ⚠️ 只认 313/316，**不要顺手把 0x13d/0x13e 也补上**。312(MENU) 会伴随一个 354(0x162)
        //   一起发（见本函数末尾那条注释），说明这张板会为一次按压发两个码；若它也把摇杆按下
        //   重复成两个码，两条映射就会让一次按压步进两档。
        0x139 => Btn::L3,
        0x13c => Btn::R3,
        // 0x162 arrives immediately behind 0x138 on one press of MENU. Mapping it too would
        // make every menu press a double tap, which is the gesture the state switcher opens
        // on, so it is deliberately nothing.
        _ => return None,
    })
}

/// Every key code the frontend has a use for. A node reporting none of them is not this
/// device's button pad, whatever it is called. The power key earns its place even though it is
/// the only thing its own node reports: left out, that whole node is passed over and the
/// button is dead however well it is mapped.
const WANTED: [u16; 15] = [
    0x130, 0x131, 0x132, 0x133, 0x134, 0x135, 0x136, 0x137, 0x138, 0x13a, 0x13b, 0x162, 115, 114,
    116,
];

/// The capability bitmask parse lives with the motor that also needs it, so the awkward part
/// — that the words are printed most significant first, and an offset only comes out right
/// counted from the end — has one definition and one set of tests.
pub use slot_power::has_bit;

/// The event nodes worth opening, in name order. Picked by what each one reports it can
/// send, never by position: `event0` is the power key on one boot and the pad on the next.
///
/// A node advertising a gamepad axis (see `WANTED_ABS`) is opened in normal operation, not only
/// while tracing: the frontend navigates the menus with the stick, so a stick wired as its own
/// ABS-only node must be read every boot. `WANTED_ABS` limits this to the gamepad axes, so a
/// touchscreen or accelerometer is still passed over.
pub fn pick_devices(dev: &Path, sys: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dev) else {
        return Vec::new();
    };
    let mut nodes: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("event"))
        })
        .collect();
    nodes.sort();
    nodes.retain(|node| {
        let Some(name) = node.file_name() else {
            return false;
        };
        wanted_node(&sys.join(name).join("device"))
    });
    nodes
}

fn wanted_node(device: &Path) -> bool {
    let cap = |file: &str| std::fs::read_to_string(device.join("capabilities").join(file));
    let keys = cap("key").unwrap_or_default();
    let lid = cap("sw").is_ok_and(|sw| has_bit(&sw, SW_LID));
    // Open a node that advertises a gamepad axis: the stick (or a second one as `ABS_RX/RY`) is
    // read in normal operation so the menus can be driven with it. `WANTED_ABS` keeps this to the
    // gamepad axes, so a touchscreen or accelerometer node is still passed over.
    let abs = cap("abs").is_ok_and(|a| WANTED_ABS.iter().any(|bit| has_bit(&a, *bit)));
    lid || abs || WANTED.iter().any(|bit| has_bit(&keys, *bit))
}

/// What sysfs calls the node, for the one line the frontend prints about what it opened.
pub fn device_name(sys: &Path, node: &Path) -> String {
    node.file_name()
        .and_then(|n| std::fs::read_to_string(sys.join(n).join("device/name")).ok())
        .map(|n| n.trim().to_string())
        .unwrap_or_else(|| node.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slot_input::{Btn, RawEvent};

    /// Feed a sequence of raw axis values into a stick, as `read_node` would after remapping.
    fn feed(stick: &mut Stick, code: u16, values: &[i32]) -> Vec<RawEvent> {
        let mut out = Vec::new();
        for &v in values {
            out.extend(stick.feed(Ev {
                kind: EV_ABS,
                code,
                value: v,
            }));
        }
        out
    }

    #[test]
    fn axis_map_identity_passes_standard_codes_through() {
        assert_eq!(AxisMap::IDENTITY.map(0), 0);
        assert_eq!(AxisMap::IDENTITY.map(1), 1);
        assert_eq!(AxisMap::IDENTITY.map(2), 2);
        assert_eq!(AxisMap::IDENTITY.map(3), 3);
        assert_eq!(AxisMap::IDENTITY.map(4), 4);
        assert_eq!(AxisMap::IDENTITY.map(5), 5);
        // Out-of-range codes (e.g. the d-pad hat axes) pass through untouched.
        assert_eq!(AxisMap::IDENTITY.map(0x10), 0x10);
    }

    #[test]
    fn axis_map_rg34xxsp_rearranges_stick_axes() {
        // Z(2)->X(0) left horizontal, RX(3)->Y(1) left vertical,
        // RY(4)->RX(3) right horizontal, RZ(5)->RY(4) right vertical.
        assert_eq!(AxisMap::RG34XXSP.map(2), 0);
        assert_eq!(AxisMap::RG34XXSP.map(3), 1);
        assert_eq!(AxisMap::RG34XXSP.map(4), 3);
        assert_eq!(AxisMap::RG34XXSP.map(5), 4);
        // X/Y themselves are unchanged, so a node that also reports them is unaffected.
        assert_eq!(AxisMap::RG34XXSP.map(0), 0);
        assert_eq!(AxisMap::RG34XXSP.map(1), 1);
    }

    /// Mirrors the 2026-11-03 stick-probe: on RG34XXSP the raw axes are Z/RX/RY/RZ. After the
    /// RG34XXSP remap each must drive the correct menu direction, and none must be dead. This is
    /// the inverse of the bug report ("left/right dead, up/down swapped").
    #[test]
    fn rg34xxsp_raw_axes_drive_correct_directions() {
        let map = &AxisMap::RG34XXSP;
        let mut left = Stick::default();
        let mut right = Stick::default();
        // Left stick horizontal = raw Z(2) -> X. Push right (+4096) => Right.
        let r = feed(&mut left, map.map(2), &[0, 4096]);
        assert!(r.contains(&RawEvent::Down(Btn::Right)), "left-h push right");
        // Left stick vertical = raw RX(3) -> Y. Push down (+4096) => Down.
        let d = feed(&mut left, map.map(3), &[0, 4096]);
        assert!(d.contains(&RawEvent::Down(Btn::Down)), "left-v push down");
        // Right stick horizontal = raw RY(4) -> RX. Push right (+4096) => Right.
        let rr = feed(&mut right, map.map(4), &[0, 4096]);
        assert!(rr.contains(&RawEvent::Down(Btn::Right)), "right-h push right");
        // Right stick vertical = raw RZ(5) -> RY. Push down (+4096) => Down.
        let rd = feed(&mut right, map.map(5), &[0, 4096]);
        assert!(rd.contains(&RawEvent::Down(Btn::Down)), "right-v push down");
        // Releasing returns to centre: the held direction is released.
        let back = feed(&mut left, map.map(2), &[0]);
        assert!(back.iter().any(|e| *e == RawEvent::Up(Btn::Right)));
    }

    /// On a conventional board (identity map) the sticks are X/Y + RX/RY; Z/RZ are not stick
    /// axes and must stay inert so a stray throttle/rudder axis never moves the menus.
    #[test]
    fn standard_board_z_rz_stay_inert() {
        let mut s = Stick::default();
        assert!(feed(&mut s, 0, &[0, 4096]).contains(&RawEvent::Down(Btn::Right))); // X
        assert!(feed(&mut s, 1, &[0, 4096]).contains(&RawEvent::Down(Btn::Down))); // Y
        assert!(feed(&mut s, 3, &[0, 4096]).contains(&RawEvent::Down(Btn::Right))); // RX
        assert!(feed(&mut s, 4, &[0, 4096]).contains(&RawEvent::Down(Btn::Down))); // RY
        // Z(2)/RZ(5) are not recognized by Stick under the identity map: no direction events.
        assert!(feed(&mut s, 2, &[0, 4096]).is_empty());
        assert!(feed(&mut s, 5, &[0, 4096]).is_empty());
    }
}
