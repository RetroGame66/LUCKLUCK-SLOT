//! The two ways the frontend is printed, and the one switch between them.
//!
//! Everything on the shelf was written in a single set of colours and those colours were
//! constants: `letters::INK`, `hud::HUD_INK`, `plate::INK`, `clock::INK`, `battery::INK`, the
//! card's four case colours in `theme.txt`, and one black scrim over the wallpaper. Six copies
//! of one idea, each in the file that happened to need it, which is why a mode could not exist
//! and why this module now owns all six.
//!
//! # Why a global and not a parameter
//!
//! The colours are read in some forty places, from the shelf's own draw to a dozen face
//! builders that run on `FaceBuilder`'s thread while the main loop keeps drawing. Threading a
//! palette through all of them would mean a parameter on every one of those functions, on both
//! sides of a thread boundary, to express a value that has exactly one answer at any moment.
//! An atomic is the honest shape of that: the mode is a fact about the device, not an argument
//! to a function.
//!
//! # Why switching is not free
//!
//! Most of the type on this device is not drawn, it is *baked*: a string is rasterised once
//! into a texture with the ink burned into it. The ink therefore cannot change under a face
//! that already exists, and the binary has to re-bake when the mode moves — see
//! `Frontend::upload_faces`, which is the one place that does it. What does follow the mode for
//! free is everything drawn as a filled quad: the case itself, the scrim, the gauge and the
//! HUD's bar.

use std::sync::atomic::{AtomicU8, Ordering};

use slot_store::Theme;
// Re-exported rather than defined here: the mode is a line in the state file before it is a
// palette, and `slot-store` is what reads and writes that file.
pub use slot_store::Mode;

static MODE: AtomicU8 = AtomicU8::new(0);

pub fn mode() -> Mode {
    match MODE.load(Ordering::Relaxed) {
        1 => Mode::Light,
        _ => Mode::Dark,
    }
}

pub fn set_mode(m: Mode) {
    MODE.store(
        match m {
            Mode::Dark => 0,
            Mode::Light => 1,
        },
        Ordering::Relaxed,
    );
}

// ---------------------------------------------------------------------------------------
// The twelve looks, on top of the device's own palette
// ---------------------------------------------------------------------------------------

/// One arm of the machine's colouring.
///
/// The four colours are the whole of it and they come from outside this project: they are the
/// `bg / text / dim / bright` of the twelve ramps in **PXReader**'s `PALETTES[]`, six dark
/// (a backlit dot-matrix's phosphor) and six light (paper and old LCDs). Everything else the
/// frontend prints — the halo, the key cap, the case's own four plastics — is *derived* from
/// these four by the functions below rather than carried here, because a fifth field would be
/// a fifth thing to keep in step across thirteen entries by hand.
///
/// **Index 0 is the device** — the palette this build has always had — and it is not derived:
/// every accessor below returns the legacy constants unchanged for it, so a card that has
/// never touched the key is byte-for-byte the device it was before looks existed. The twelve
/// after it are the ramps.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Look {
    /// What the key cycles past, so the row a look is picked on can say which one it is.
    pub name: &'static str,
    /// The ground: the colour of the screen when there is no wallpaper, and the paper the
    /// whole device is printed on.
    pub bg: [u8; 3],
    /// The ink: every letter, title, hint and number.
    pub text: [u8; 3],
    /// One step back from the ink, for type that is present but is not what is being read.
    pub dim: [u8; 3],
    /// The highlight: the metal on the index strip, and the lit rim of the case.
    pub bright: [u8; 3],
}

/// How many looks a mode has: the device's own, then six.
pub const LOOK_COUNT: usize = 7;

/// The device's own palette, written as a look so the table has one shape. These four are read
/// back only by the tests and by nothing on the draw path — every accessor short-circuits to its
/// own legacy constant at index 0 — and they are here to say what the default *is*.
const DEVICE: Look = Look {
    name: "默认",
    bg: [0x00, 0x00, 0x00],
    text: [0xf5, 0xf2, 0xef],
    dim: [0xba, 0xb8, 0xb6],
    bright: [0xd8, 0xd4, 0xcc],
};

/// The same, the other way up. Its ground and ink are the light mode's; the two after them are
/// its own steps, so the table reads as the pair it is.
const DEVICE_LIGHT: Look = Look {
    name: "默认",
    bg: [0xe6, 0xe3, 0xdd],
    text: [0x1c, 0x1b, 0x1f],
    dim: [0x4c, 0x4b, 0x4d],
    bright: [0x6b, 0x64, 0x5b],
};

/// **The twelve, in PXReader's own order and with its own numbers.** `PAL_GRPS` there is
/// `[[0,1,2,7,8,9], [3,4,5,6,10,11]]` into a flat table; here the groups are the two rows, which
/// is the same split said once instead of twice. The names are PXReader's, kept in Latin because
/// they are what a user comparing the two projects will have seen.
///
/// The two groups are what makes the cycling honest: dark mode walks the phosphors and light mode
/// walks the papers, so a look is *within* a mode rather than a fourteenth thing that overrides
/// it. The `mode` key still means what it always did.
const LOOKS: [[Look; LOOK_COUNT]; 2] = [
    // The dark row: six backlit phosphors, on a ground that is nearly off.
    [
        DEVICE,
        Look {
            name: "GREEN",
            bg: [10, 24, 12],
            text: [51, 255, 102],
            dim: [26, 128, 51],
            bright: [136, 255, 170],
        },
        Look {
            name: "AMBER",
            bg: [26, 12, 0],
            text: [255, 176, 0],
            dim: [128, 88, 0],
            bright: [255, 200, 77],
        },
        Look {
            name: "RED",
            bg: [26, 4, 4],
            text: [255, 51, 34],
            dim: [128, 26, 17],
            bright: [255, 110, 90],
        },
        Look {
            name: "VIOLET",
            bg: [18, 0, 30],
            text: [201, 166, 255],
            dim: [90, 60, 140],
            bright: [230, 210, 255],
        },
        Look {
            name: "CYAN",
            bg: [0, 20, 20],
            text: [64, 232, 216],
            dim: [24, 120, 112],
            bright: [150, 255, 245],
        },
        Look {
            name: "PINK",
            bg: [26, 0, 20],
            text: [255, 136, 216],
            dim: [128, 60, 110],
            bright: [255, 200, 240],
        },
    ],
    // The light row: paper and the LCDs of a calculator, on a ground that is the paper itself.
    [
        DEVICE_LIGHT,
        Look {
            name: "GB",
            bg: [155, 188, 15],
            text: [15, 56, 15],
            dim: [60, 100, 30],
            bright: [15, 56, 15],
        },
        Look {
            name: "PAPER",
            bg: [238, 238, 228],
            text: [34, 34, 30],
            dim: [102, 102, 92],
            bright: [0, 0, 0],
        },
        Look {
            name: "LCDG",
            bg: [192, 224, 192],
            text: [16, 48, 16],
            dim: [64, 112, 64],
            bright: [16, 48, 16],
        },
        Look {
            name: "LCDB",
            bg: [192, 216, 232],
            text: [16, 32, 48],
            dim: [64, 96, 128],
            bright: [16, 32, 48],
        },
        Look {
            name: "SEPIA",
            bg: [240, 228, 200],
            text: [74, 56, 32],
            dim: [150, 120, 80],
            bright: [40, 28, 16],
        },
        Look {
            name: "PINKBG",
            bg: [255, 229, 236],
            text: [255, 70, 130],
            dim: [214, 110, 148],
            bright: [230, 40, 105],
        },
    ],
];

/// Which arm of the current mode's row is in use. Zero is the device's own and is what every
/// card reads as until the key is pressed.
static LOOK: AtomicU8 = AtomicU8::new(0);

/// The index, held to the table: a state file edited on a desktop must not be able to put the
/// device past the end of its row, the same way it cannot put it into a third mode.
pub fn look() -> usize {
    (LOOK.load(Ordering::Relaxed) as usize).min(LOOK_COUNT - 1)
}

pub fn set_look(n: usize) {
    LOOK.store(n.min(LOOK_COUNT - 1) as u8, Ordering::Relaxed);
}

/// The next look in the current mode's row, wrapping. What SELECT+R1 does on the shelf, and the
/// whole of what it does: the two rows are walked independently and the mode key is what chooses
/// between them, so the same index means a phosphor in the dark and a paper in the light.
pub fn next_look() -> usize {
    (look() + 1) % LOOK_COUNT
}

/// The look in use, resolved against the mode. The one place the two axes are read together.
pub fn current() -> Look {
    LOOKS[match mode() {
        Mode::Dark => 0,
        Mode::Light => 1,
    }][look()]
}

/// What a look is called, for anything that has to say which one is up.
pub fn look_name(n: usize) -> &'static str {
    let row = match mode() {
        Mode::Dark => 0,
        Mode::Light => 1,
    };
    LOOKS[row][n.min(LOOK_COUNT - 1)].name
}

/// Whether the device is on one of the twelve rather than on its own palette. Every accessor
/// below asks this first, and that is what keeps index 0 byte-for-byte the old device.
fn on_a_ramp() -> bool {
    look() != 0
}

/// The twelve from the outside, for a test that has to walk them all. Nothing on the draw path
/// needs the whole table.
pub fn looks() -> [[Look; LOOK_COUNT]; 2] {
    LOOKS
}

fn scale(c: [u8; 3], k: f32) -> [u8; 3] {
    let f = |v: u8| (v as f32 * k).round().clamp(0.0, 255.0) as u8;
    [f(c[0]), f(c[1]), f(c[2])]
}

/// The case a look is cut from. Not the look's own ground: a shell the same colour as the screen
/// behind it stops being an object on it, and every ramp's ground is either nearly off or the
/// paper itself. So the plastic is the ground *taken towards neutral grey* — it keeps the look's
/// cast without being a second copy of the screen — and the three surfaces inside it step down
/// from there the way `theme.txt`'s do.
///
/// The lit edge is the one field that is not a step: a phosphor already has a highlight to spend
/// and it is the brightest thing in the look, so the rim is `bright` where that is lighter than
/// the plastic. Where it is not — every paper look, whose `bright` is its *ink* — the rim is the
/// plastic lightened, because an edge darker than the thing it is the edge of is not an edge.
pub fn look_case() -> Theme {
    let l = current();
    let plastic = mix(l.bg, [128, 128, 128], 0.35);
    let rim = if lum(l.bright) > lum(plastic) {
        l.bright
    } else {
        scale(plastic, 1.35)
    };
    Theme {
        housing: plastic,
        recess: scale(plastic, 0.80),
        opening: scale(plastic, 0.55),
        edge: rim,
    }
}

fn lum(c: [u8; 3]) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

// ---------------------------------------------------------------------------------------
// The case
// ---------------------------------------------------------------------------------------

/// The light case, in the same four roles the card's `theme.txt` addresses.
///
/// Not derived from the dark one by scaling, and that is the whole point of writing it out: a
/// pure inversion of `housing #242429` gives a near-black `opening`, so the letters would be
/// dark type on a black window — the one combination that is unreadable in either mode. The
/// four roles are what carry over, not the numbers: the housing is the outer plastic, the
/// recess is a step down from it, the opening is what you see *into*, and the edge is the lit
/// line where the plastic is cut. Each stays on the side of the housing that makes it read as
/// what it is, which is why the opening stays *darker* than the housing even though the
/// housing has gone light.
///
/// A warm grey rather than a white: the scrim over the wallpaper lands at about `#e8e5e0`, and
/// plastic the same tone as the ground behind it stops being an object on it.
///
/// `theme.txt` addresses the dark case and only the dark case: it is the file a card ships to
/// say what colour its machine is, and the light palette is the *same* machine seen the other
/// way up rather than a second theme to be overridden in four more names. `slot_chrome` chooses
/// between the two — it is the only thing that holds the card's theme — and this is only the
/// light half of that answer.
pub const LIGHT_CASE: Theme = Theme {
    housing: [0xcf, 0xc8, 0xbb],
    recess: [0xbf, 0xb8, 0xaa],
    opening: [0xb0, 0xa8, 0x9a],
    edge: [0xf6, 0xf3, 0xec],
};

// ---------------------------------------------------------------------------------------
// The type
// ---------------------------------------------------------------------------------------

/// The ink every letter, title, hint and number is set in.
pub fn ink() -> [u8; 3] {
    if on_a_ramp() {
        return current().text;
    }
    match mode() {
        Mode::Dark => [0xf5, 0xf2, 0xef],
        Mode::Light => [0x1c, 0x1b, 0x1f],
    }
}

pub fn ink_f() -> [f32; 4] {
    let c = ink();
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ]
}

/// The ink *on* an ink-coloured panel: a key cap's letter, which has to be the ground's colour
/// for the cap to read as a key in either mode. Dark mode is a light cap with a dark letter and
/// the light mode is a dark cap with a light one, so this is not "a darker ink" — it is the
/// opposite of `ink`, and the only value in here that is defined against another.
pub fn panel_ink() -> [u8; 3] {
    if on_a_ramp() {
        // The ground's colour, the same rule the two hard-coded arms keep: a cap's letter has
        // to be the opposite of the cap, whichever way round the palette is.
        return current().bg;
    }
    match mode() {
        Mode::Dark => [0x1a, 0x19, 0x17],
        Mode::Light => [0xf4, 0xf1, 0xea],
    }
}

/// The ink *on the lit face*: the label on the row the user is standing on.
///
/// The one colour in here that is **chosen rather than derived**, and the reason is that neither
/// of the two obvious answers survives the whole table. A lit face is `edge()`, which is the
/// brightest thing the case owns — so on the twelve ramps, whose ink is a phosphor and is
/// therefore nearly as bright as the rim, `ink` on it comes out about **1.1:1**, a label the same
/// colour as its button. `opening` — what the two hard-coded arms have always used, and what this
/// returns for them, so nothing about the device changes — is worse than that on the palette the
/// device is *born* with: **2.13:1 in the light, 2.44:1 in the dark**.
///
/// So the rule is: **take whichever of the colours this palette already puts on a case surface
/// reads hardest against the lit one.** Those are the ramp's `text`, `dim` and `bg`, plus the
/// case's own `opening` — nothing is invented, and one of them always wins. The worst of the
/// **thirteen** is PINKBG at 5.16:1; which one wins differs per look and that is the point: a
/// phosphor's lit face wants its *ground* on it (fourteen to one), a paper's wants its *ink*
/// (sixteen), and PINKBG wants the case's window — so no single one of them covers both rows.
///
/// # This one accessor is *not* zero-regression, on six-six's call (2026-10-16)
///
/// Every other accessor here short-circuits at index zero so a card that has never touched the
/// key is the byte-identical device. This one used to as well, and the measurement is why it no
/// longer does: the device's own label on a lit row was **2.07:1 in the light and 2.81:1 in the
/// dark — the worst of all thirteen**, with the *chosen* button harder to read than the unselected
/// one beside it. Holding a regression rule over a defect that the rule itself exposed is the
/// wrong trade, so the device's palette now takes the same rule as the twelve: **15.06:1 light /
/// 6.50:1 dark**, and `opening` — the colour it used to be set in — is still one of the candidates,
/// so a card whose theme happens to want it gets it.
///
/// Everything else about index zero is untouched; `tests/look.rs` holds the rest of the palette to
/// the byte.
pub fn chosen_ink() -> [u8; 3] {
    // The *drawn* surfaces, through the same accessors the faces read — **not** `look_case()`,
    // which at index zero is a case derived from the ramp table and is not the card's theme at all.
    // Asking it here would have silently ignored a card's own `theme.txt`.
    let face = bytes(crate::slot_chrome::edge());
    let window = bytes(crate::slot_chrome::opening());
    let l = current();
    // The case's own window first, so the candidate set is the same shape for every palette and
    // the device's historical answer is one of the things a ramp is allowed to choose.
    let mut best = window;
    let mut best_r = contrast(best, face);
    for c in [l.text, l.dim, l.bg] {
        let r = contrast(c, face);
        if r > best_r {
            best_r = r;
            best = c;
        }
    }
    best
}

/// The rim a **lit** face wears — `chosen_ink`'s counterpart, on the edge instead of on the label.
///
/// Every face is drawn with a 1.5 px rim in `edge`, and the lit face is *filled* with `edge`, so on
/// the one row the eye has to find first the rim and the fill were the same colour: measured
/// **1.00:1**, i.e. no outline at all. The row is still *findable* without it — measured against
/// its neighbours the lit fill steps **1.49:1** on the light look and **1.78:1** on the dark, which
/// is why the selected row does read as selected in the renders. But a card that is only a lighter
/// rectangle is a highlight, not a key: nothing states where it ends. That matters most on the light
/// look, where the fill sits at **1.02:1** against the paper behind it — the fill alone contributes
/// no edge whatsoever against the sheet, so the card's own boundary has to come from the rim.
///
/// So the lit rim is `edge` pulled 45% toward the palette's own **ink**, the colour guaranteed to
/// sit at the far end of whatever surface it is drawn on. One rule, no branch, and it resolves in
/// both directions: on the light look the rim goes *darker* — (148,146,144) on a (246,243,236) face,
/// **2.80:1**, a moulded seam — and on the dark look *lighter*, (153,151,155) on (77,77,87),
/// **2.88:1**, the same lit edge seen from the other side.
///
/// 45% and not 30%: the rim is load-bearing here rather than decorative. At 30% it measures only
/// 1.91 / 2.08 — a soft tint that survives being next to the fill but stops reading as a seam. At
/// 45% the key gets an actual moulded edge while staying far below the label's own contrast, so it
/// never competes with the text it frames.
pub fn chosen_rim() -> [f32; 4] {
    let face = bytes(crate::slot_chrome::edge());
    // `ink()` and not `current().text`: the faces ink themselves through `ink()`, and on the device's
    // own two looks that accessor is the same answer — but on a ramp it is the field the card
    // actually set, which is the one the rim has to step away from.
    let ink = ink();
    let mix = |k: usize| -> f32 {
        (face[k] as f32 + (ink[k] as f32 - face[k] as f32) * 0.45) / 255.0
    };
    [mix(0), mix(1), mix(2), 1.0]
}

/// The ink on a face that is **not** the one in hand — `chosen_ink`'s mirror.
///
/// A resting button's label is the second thing on the screen and should step back; but "step
/// back" has to mean *duller and still readable*, not *dimmer than the thing it is drawn on*.
/// `dim_ink` is the palette's own step back and is exactly right against the case's own theme
/// (measured **7.80:1** dark / **5.16:1** light on `housing`). On a ramp it is not: `dim` is a
/// step back from the ramp's **ground**, while a resting button is drawn on the ramp's
/// **plastic**, which is a different colour altogether. Measured against it, RED is **1.30:1**,
/// VIOLET 1.57, PINK 1.76, SEPIA 1.91, AMBER 1.98, GB 2.13 — the label is invisible, which is
/// what 六六 saw on the RED card on 2026-10-16 and called "the red one needs it the other way".
///
/// So the rule picks, **among the colours this palette already draws on the case**, the dullest
/// one that still clears the floor. The candidates are the same five roles `chosen_ink` draws
/// from — the ramp's three plus **both** of the case's own other surfaces, the lit line and the
/// window — because a resting button sits on `housing` and everything else the case owns is fair
/// game for the label on it. That last one is not decoration: on PINKBG the ramp's ink and its
/// plastic are both pink and nothing on the ramp reads at all (**1.91:1** for the best of them),
/// while the case's window comes out a dull rose and reads at **3.03:1** — the difference between
/// a blank key and a legible one.
///
/// **4.0:1 and not 4.5:1**, deliberately: this is a 30 px bold control name on a button that is
/// not the one in hand, and the quarter-ratio it gives up is exactly the hierarchy the states are
/// meant to show. The lit face keeps the 4.5 bar — that one is being read.
///
/// Measured on the rendered screen, worst of the fourteen is PINKBG 3.03:1 flat / 2.51 rendered;
/// the device's own palette lands on `dim` exactly as `dim_ink` would (7.80 dark / 5.16 light
/// flat, 8.01 / 4.30 rendered) — one byte of the shipped look moves.
pub fn idle_ink() -> [u8; 3] {
    let face = bytes(crate::slot_chrome::housing());
    let l = current();
    let mut dullest: Option<([u8; 3], f32)> = None;
    let mut hardest = (l.text, contrast(l.text, face));
    for c in [
        l.text,
        l.dim,
        l.bg,
        bytes(crate::slot_chrome::edge()),
        bytes(crate::slot_chrome::opening()),
    ] {
        let r = contrast(c, face);
        if r > hardest.1 {
            hardest = (c, r);
        }
        if r >= RESTING_FLOOR && dullest.is_none_or(|(_, d)| r < d) {
            dullest = Some((c, r));
        }
    }
    dullest.unwrap_or(hardest).0
}

/// The floor a *resting* label has to clear — a shade under what a lit one is held to (4.5), and
/// that shade **is** the "step back" the eye is meant to read.
///
/// **4.0 and not 3.0, and the reason is `mould`.** This is asked about `housing()`, but the label
/// is drawn on a *rendered* button, and `menu::mould` takes ~11% off a light face and adds ~7% to
/// a dark one. So a candidate that clears 3.0 here measures **2.49:1 on the light ramps** — which
/// is what the first cut of this rule did on PAPER and SEPIA, and it is too dull to read. Four
/// leaves the render at 3.3–4.2 light and 4.2–4.6 dark: still the duller of the two labels, still
/// legible. Measured worst after the change is PINKBG at 2.51, and that one has no answer —
/// its ink and its plastic are both pink.
const RESTING_FLOOR: f32 = 4.0;

/// ★ 2026-10-24（六六）：「**部分主题配色游戏名看不清**」—— 顶栏上那行游戏名。
///
/// 名字原先一律用 `ink()`（= ramp 的 `text`），画在顶栏的 `opening()` 上。暗色主题下
/// `opening` 近黑、`ink` 近白，13+ :1，从来没出过问题。**亮色主题不是这样**：`opening`
/// 是 `plastic × 0.55` 的一道中灰，而 ramp 的 `text` 也是深色 —— 深压中灰。
///
/// 实测（WCAG，门槛大字 3.0）：
///
/// | look | 顶栏底 `opening` | 名字 `ink` | 比值 |
/// |---|---|---|---|
/// | 默认 | (107,106,103) | (28,27,31) | **3.17** |
/// | PAPER | (110,110,106) | (34,34,30) | **3.12** |
/// | LCDB | (94,102,108) | (16,32,48) | 2.83 |
/// | LCDG | (94,105,94) | (16,48,16) | 2.52 |
/// | SEPIA | (111,106,96) | (74,56,32) | 2.08 |
/// | GB | (80,92,30) | (15,56,15) | **1.81** |
/// | PINKBG | (116,107,109) | (255,70,130) | **1.59** |
///
/// 五套低于门槛，PINKBG 那套粉字压灰条基本是一行色块。所以名字不再固定读 `ink()`，
/// 改成**在这张面上取读得出来的那个**：
///
/// 1. `ink()` 本来就达标 ⇒ 原样用它（**设备两套主题一个字节都不动**，3.17 / 13+ 都够）；
/// 2. 不达标 ⇒ 在色板自己画在这台机器上的几个色里取对比最高的；
/// 3. 还是不够 ⇒ 把那个色朝"远离这张面"的方向推到刚好达标（最多推到纯白/纯黑）。
///
/// 第 3 步是"至少读得出"的兜底，不是在配色板：GB 那套最亮的候选是它的 `bg`
/// (155,188,15)，压在 (80,92,30) 上只有 2.6，必须再提一点才够 3.0。
pub fn plate_ink() -> [u8; 3] {
    ink_on_bytes(bytes(crate::slot_chrome::opening()), PLATE_FLOOR)
}

/// ★ 2026-10-24（六六）：「**字母轮不显示**」—— 真的不是"不显示"，是**字和胶囊一个色**。
///
/// 胶囊填充按 2026-10-24 的定稿取 `deepest()`（`ground` 与 `ink` 谁更暗用谁）：
/// 深色主题 ⇒ `ground`（黑），亮色主题 ⇒ **`ink`（墨）**。而字母画在 `ink()` 上。
/// 于是亮色主题下：胶囊 = ink、字母 = ink ⇒ **实测 1.00:1**，整条字母轮只剩胶囊那条深色
/// 药丸，字母全没了（暗色主题是 18.83:1，所以问题只在亮色出现）。
///
/// 规则与 `plate_ink` 同一条，只是面换成 `deepest()`、门槛更高（字母小一号 18px）：
/// 取在这块药丸上读得出来的那个色 —— 亮色主题自然落到 `bg`（纸色）上。
pub fn pill_ink() -> [u8; 3] {
    ink_on_bytes(pill_fill(), PILL_FLOOR)
}

/// 胶囊**填什么色** —— `deepest()`，但当它连纯白都托不住字时按需压暗。
///
/// ★ 2026-10-24：PINKBG 那套的 `deepest()` 落在 `text` = **粉红 (255,70,130)** 上，
/// 它按"通道和"是最深的，按亮度却不是 —— 纯白压上去只有 **3.25:1**，
/// 怎么挑字色都到不了 4.5。这不是字色的问题，是**那块底太亮**。
///
/// 所以药丸在必要时往黑里压，压到刚好托得住 4.5 为止（PINKBG 约压到 85%，还是那支粉，
/// 只是深了一档 —— 药丸本来就是"嵌进去的凹槽"，压暗它不改变它是凹槽这件事）。
/// 其余十二套的 `deepest()` 本来就够暗（GB 13.2、PAPER 15.8、SEPIA 11.0），
/// **一个字节都不动**。
pub fn pill_fill() -> [u8; 3] {
    let base = deepest();
    if contrast([255, 255, 255], base) >= PILL_FLOOR {
        return base;
    }
    // 二分混向黑，取刚好把纯白托上 PILL_FLOOR 的那一档。
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..12 {
        let mid = (lo + hi) * 0.5;
        let c: [u8; 3] = std::array::from_fn(|i| {
            (base[i] as f32 * (1.0 - mid)).round() as u8
        });
        if contrast([255, 255, 255], c) >= PILL_FLOOR {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    std::array::from_fn(|i| (base[i] as f32 * (1.0 - hi)).round() as u8)
}

/// `plate_ink` 的门槛：26px 游戏名，WCAG 大字 AA 是 3.0，这里留 0.5 的余量。
const PLATE_FLOOR: f32 = 3.5;

/// `pill_ink` 的门槛：字母里最小那档 18px，按正文的 4.5 走。
const PILL_FLOOR: f32 = 4.5;

/// 在一张**已知的**面上，取一个读得出来的字色。
///
/// 三步，按 [`plate_ink`] 文档里那三条：原 ink 达标就原样返回（零回归），否则在色板自己
/// 的色里挑最好的，再不够就往远离这张面的一端推到达标。
///
/// `face` 是**已经画出来的那个颜色**（逐字节从 `slot_chrome` 取），不是再推导一遍 ——
/// 与 `chosen_ink` / `idle_ink` 同一条纪律：拿推导值判色会静默改掉每张卡的外观。
fn ink_on_bytes(face: [u8; 3], floor: f32) -> [u8; 3] {
    let own = ink();
    let own_r = contrast(own, face);
    if own_r >= floor {
        return own;
    }
    let l = current();
    let mut best = own;
    let mut best_r = own_r;
    for c in [l.text, l.dim, l.bg, l.bright] {
        let r = contrast(c, face);
        if r > best_r {
            best_r = r;
            best = c;
        }
    }
    if best_r >= floor {
        return best;
    }
    // 最后一步：把它朝"远离这张面"的那一端推，二分到刚好过线。
    // 方向由亮度决定：比面亮的往白推，比面暗的往黑推 —— 这样推出去的颜色仍在这张面的
    // 反方向，不会推着推着从"亮字"变成"略亮的灰"。
    let to_white = rel_lum(best) >= rel_lum(face);
    let far = if to_white { [255, 255, 255] } else { [0, 0, 0] };
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..12 {
        let mid = (lo + hi) * 0.5;
        let c: [u8; 3] = std::array::from_fn(|i| {
            (best[i] as f32 + (far[i] as f32 - best[i] as f32) * mid).round() as u8
        });
        if contrast(c, face) >= floor {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    std::array::from_fn(|i| {
        (best[i] as f32 + (far[i] as f32 - best[i] as f32) * hi).round() as u8
    })
}

/// A `slot_chrome` surface, which is `[f32; 4]`, read back as the `[u8; 3]` it was made from.
fn bytes(c: [f32; 4]) -> [u8; 3] {
    [
        (c[0] * 255.0).round() as u8,
        (c[1] * 255.0).round() as u8,
        (c[2] * 255.0).round() as u8,
    ]
}

/// WCAG 2.x contrast ratio between two opaque colours. Relative luminance, `(L1+.05)/(L2+.05)`.
///
/// Here rather than in the test because the choice above needs to *rank colours*, and ranking by
/// raw luminance puts red at 0.09 against black and blue at 0.07 — which would say a phosphor's
/// label is unreadable when it is the brightest thing on the screen.
pub(crate) fn contrast(a: [u8; 3], b: [u8; 3]) -> f32 {
    let (la, lb) = (rel_lum(a), rel_lum(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn rel_lum(c: [u8; 3]) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// The one pixel halo behind a glyph that sits on a picture with nothing under it — the HUD's
/// icons and the toast's own outline. It exists to separate type from whatever is behind it, so
/// it is the ground's colour and it flips with everything else.
pub fn halo() -> [u8; 3] {
    if on_a_ramp() {
        // The ground, as here: the halo is what the glyph is read *against*.
        return current().bg;
    }
    match mode() {
        Mode::Dark => [0x08, 0x08, 0x0a],
        Mode::Light => [0xf2, 0xef, 0xe8],
    }
}

/// The veil the HUD's bar and its toasts are read against.
///
/// Not the scrim's colour and not the housing's either, though in the light it is close to the
/// latter. Its job is to separate type from a moving picture, so it has to be a colour no
/// game frame is: black in the dark, the case's own light plastic in the light, both at an
/// alpha that leaves the picture visible underneath rather than erasing it.
pub fn plate() -> [f32; 4] {
    if on_a_ramp() {
        // A solid version of the look's ground at the veil's own alpha, and the same one both
        // ways up: a ramp is chosen for how its background reads, so the veil has to be *that*
        // colour rather than the case's.
        let g = current().bg;
        return [
            g[0] as f32 / 255.0,
            g[1] as f32 / 255.0,
            g[2] as f32 / 255.0,
            0.72,
        ];
    }
    match mode() {
        Mode::Dark => [0.0, 0.0, 0.0, 0.72],
        Mode::Light => [0.81, 0.78, 0.73, 0.82],
    }
}

/// An opaque panel that type is set on, as opposed to `plate`, which is a translucent veil over
/// a picture. The clock picker is a screen of its own rather than something laid over one, so
/// what is behind it must not show through.
pub fn panel() -> [f32; 4] {
    if on_a_ramp() {
        let g = current().bg;
        return [
            g[0] as f32 / 255.0,
            g[1] as f32 / 255.0,
            g[2] as f32 / 255.0,
            1.0,
        ];
    }
    match mode() {
        Mode::Dark => [0.06, 0.06, 0.07, 1.0],
        Mode::Light => [0.88, 0.86, 0.82, 1.0],
    }
}

/// A key cap on the shortcut card, where the caps are drawn as filled squares rather than as
/// the light plates a hint's cap is.
///
/// Mid grey in the dark and darker in the light, which looks backwards until you see what it
/// is drawn on: the card is a light panel in both modes, so the cap has to be *darker* than the
/// card in both. It is the same rule as `panel_ink` read one level out — a cap separates from
/// what it sits on, whichever way round the rest of the screen is.
pub fn keycap() -> [u8; 3] {
    if on_a_ramp() {
        // The card the caps sit on is the look's paper, so the cap steps back from it the way
        // the two hard-coded arms step back from theirs.
        return mix(current().bg, current().text, 0.45);
    }
    match mode() {
        Mode::Dark => [0xb4, 0xb0, 0xa8],
        Mode::Light => [0x8a, 0x85, 0x7c],
    }
}

/// The flat the screen is when the card carries no wallpaper. The wallpaper-facing twin of
/// this is `scrim`, and the two are the same colour seen two ways: black and paper.
pub fn ground() -> [u8; 3] {
    if on_a_ramp() {
        return current().bg;
    }
    match mode() {
        Mode::Dark => [0x00, 0x00, 0x00],
        Mode::Light => [0xe6, 0xe3, 0xdd],
    }
}

/// ★ 2026-10-24（六六）：「胶囊按**最深色**」——主题里最深的那个色：地面上取 `ground`，
/// 墨色上取 `ink`，谁更暗用谁（深色主题 → 黑；亮色主题 → 墨）。
pub fn deepest() -> [u8; 3] {
    let (g, i) = (ground(), ink());
    let lum = |c: [u8; 3]| c[0] as u32 + c[1] as u32 + c[2] as u32;
    if lum(g) <= lum(i) {
        g
    } else {
        i
    }
}

/// Type that is present but is not the thing being read: a shortcut's description, one step
/// back from the head above it.
///
/// Defined as a step towards the ground rather than as a fraction of the ink, and that is the
/// only reason it is a function at all: "dimmer" is towards the ground in both modes, but the
/// ground is black in one and paper in the other, so a fixed fraction of the ink would make the
/// description *brighter* than the head in the light mode and the hierarchy would invert with
/// the palette.
pub fn dim_ink() -> [u8; 3] {
    if on_a_ramp() {
        // The ramp carries its own dim — that is what the fourth field was for — so a look does
        // not have to have one derived for it. It is also the one value a paper look cannot
        // derive honestly: `mix(ink, ground)` on a look whose ink already *is* its paper's
        // darkest would come out as a second ink rather than as a step back from one.
        return current().dim;
    }
    mix(ink(), ground(), 0.24)
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let mut out = [0u8; 3];
    for i in 0..3 {
        out[i] = (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    }
    out
}

/// The metal between two letters on the index strip, and the tooth on the end of it.
///
/// The one colour in here that is not type and not a surface: it is a highlight standing on the
/// window, so it goes the same way as the light rather than the same way as the ink. Light metal
/// on a dark window in the dark mode, dark metal on a pale one in the light — the same ridge,
/// seen against whatever the window has become.
pub fn ridge_ink() -> [u8; 3] {
    if on_a_ramp() {
        // The highlight, as here: the ridge stands on the window the ledger's own colour is,
        // and the ramp's `bright` is what it has to stand out against.
        return current().bright;
    }
    match mode() {
        Mode::Dark => [0xd8, 0xd4, 0xcc],
        Mode::Light => [0x6b, 0x64, 0x5b],
    }
}

/// The veil over the wallpaper: what the picture is taken back out by, and in which direction.
///
/// Black in the dark and white in the light, and the light one is stronger. Not symmetric, and
/// it is worth saying why: black at 0.62 leaves a photograph that is still a photograph, which
/// is what a dark ground needs to stay one. White at 0.62 leaves a pale wash with the picture's
/// own contrast showing through as grey blotches, which is exactly what `tests/contrast.rs`
/// exists to keep out of the ground the case is printed on.
pub fn scrim() -> [f32; 4] {
    if on_a_ramp() {
        // **The look's ground, not black or white**, and that is the difference between a
        // wallpaper wearing the ramp and the ramp being painted over. The two hard-coded arms
        // take the picture back out towards the ground's own end of the scale; a look has a
        // ground of its own, so the veil is that colour at the stronger of the two alphas.
        let g = current().bg;
        return [
            g[0] as f32 / 255.0,
            g[1] as f32 / 255.0,
            g[2] as f32 / 255.0,
            0.74,
        ];
    }
    match mode() {
        Mode::Dark => [0.0, 0.0, 0.0, 0.62],
        Mode::Light => [1.0, 1.0, 1.0, 0.74],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every test in the crate shares one mode, so a test that moves it has to put it back or
    /// the next one to run sees a light palette it never asked for.
    struct Restore(Mode);

    impl Drop for Restore {
        fn drop(&mut self) {
            set_mode(self.0);
        }
    }

    fn pinned(m: Mode) -> Restore {
        let was = mode();
        set_mode(m);
        Restore(was)
    }

    #[test]
    fn the_ink_inverts_and_the_panel_ink_is_its_opposite() {
        let _pin = pinned(Mode::Dark);
        assert_eq!(ink(), [0xf5, 0xf2, 0xef]);
        assert_eq!(panel_ink(), [0x1a, 0x19, 0x17]);

        set_mode(Mode::Light);
        assert_eq!(ink(), [0x1c, 0x1b, 0x1f]);
        assert_eq!(panel_ink(), [0xf4, 0xf1, 0xea]);

        // The one property that has to hold in both: a cap's letter is never the same colour
        // as the cap, or the key is a blank square.
        for m in [Mode::Dark, Mode::Light] {
            set_mode(m);
            assert_ne!(ink(), panel_ink(), "{m:?} draws a letter on its own ink");
        }
    }

    #[test]
    fn the_ground_is_always_on_the_other_side_of_the_type() {
        // The scrim is the ground the case is printed on, and the ink has to be legible on it
        // in both modes. Stated as one number — how far apart they are — because "readable" is
        // not otherwise a thing a test can hold, and a palette edit is exactly the change that
        // would quietly break it.
        //
        // The wallpaper behind the scrim is taken as mid grey, which is the worst case for
        // both modes: black at 0.62 leaves a light picture dark and white at 0.74 leaves a dark
        // picture light, so a mid grey is the ground least like the scrim's own colour.
        for m in [Mode::Dark, Mode::Light] {
            let _pin = pinned(m);
            let s = scrim();
            let ground = s[0] * 255.0 * s[3] + (1.0 - s[3]) * 128.0;
            let ink_lum = lum(ink());
            assert!(
                (ink_lum - ground).abs() > 60.0,
                "{m:?}: ink {ink_lum:.0} on a ground of {ground:.0} is too close to read"
            );
        }
    }

    #[test]
    fn the_light_case_keeps_the_roles_the_dark_one_has() {
        // The housing is the plastic, the recess is a step *down* from it and the opening is
        // further in still; the edge is the lit line and so is lighter than all of them. Which
        // way round the housing itself sits is the mode's business — that is the whole point of
        // the light palette — but the order of the three surfaces is not.
        for (name, t) in [("dark", Theme::default()), ("light", LIGHT_CASE)] {
            assert!(
                lum(t.recess) < lum(t.housing),
                "{name}: the recess is not below the housing"
            );
            assert!(
                lum(t.opening) < lum(t.recess),
                "{name}: the opening is not inside the recess"
            );
            assert!(
                lum(t.edge) > lum(t.housing),
                "{name}: the lit edge is not the lightest of the four"
            );
        }
    }

    #[test]
    fn the_type_is_legible_on_the_case_in_both_modes() {
        // The letters sit in the band's window, which is `opening`, and the clock and the gauge
        // sit on the housing. Both pairs have to separate, in both modes, or a mode is a
        // palette that only looks right in the mock.
        for m in [Mode::Dark, Mode::Light] {
            let _pin = pinned(m);
            let t = match m {
                Mode::Dark => Theme::default(),
                Mode::Light => LIGHT_CASE,
            };
            for (where_, surface) in [("the window", t.opening), ("the housing", t.housing)] {
                let gap = (lum(ink()) - lum(surface)).abs();
                assert!(gap > 90.0, "{m:?}: ink on {where_} is only {gap:.0} apart");
            }
        }
    }

    fn lum(c: [u8; 3]) -> f32 {
        0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
    }
}
