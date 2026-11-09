//! The thirteen looks, held to what the shelf asks of them.
//!
//! `contrast.rs` next door guards the case *against the backdrop*. This guards the palettes
//! themselves — the thirteen entries of `palette::LOOKS`, walked in both modes — because a look
//! is only four numbers typed in from outside this project (PXReader's `PALETTES[]`), and every
//! other colour the screens print, from the halo to the key cap to the case's own four plastics,
//! is *derived* from them. A derivation is exactly the kind of thing that reads fine as numbers
//! and leaves one entry out of thirteen illegible on a panel.
//!
//! What is asserted here is the part that is **this project's own rule**, not PXReader's taste:
//! the order of the four case surfaces, the way the rim picks a branch, the direction `dim` steps
//! in, and the fact that index zero is still the old device to the byte. Whether a given ramp's
//! ink is comfortable is the ramp's business and it arrives as given; the floors below are set
//! under the measured minimum of the twelve so a red line means the derivation moved, not that
//! somebody chose a soft colour.
//!
//! Measured (2026-10-15) against the shipped table — the numbers in the comments are these, not
//! estimates, and they are here so the next person knows the headroom:
//!
//! | pairing | tightest | look |
//! |---|---|---|
//! | ink vs its own ground      | 99  | RED |
//! | dim vs its own ground      | 45  | RED |
//! | ink on `opening`           | 22  | PINKBG ⚠️ see below |
//! | ink on `housing`           | 59  | RED |
//! | ink over the scrim         | 55  | RED |
//! | a cap vs its card          | 47  | PINKBG |
//!
//! ⚠️ **One pairing is faint and is not asserted**: type on a *lit* button is `opening`-coloured
//! on an `edge` fill (`menu::face`, `settings::face`). That is the selected row, and for the pale
//! rows of the light mode its separation is PINKBG 20 / SEPIA 45 / GB 50, against 146 for the
//! device's own palette. It is not a rule this file can hold — it is a question about the two
//! palest ramps, and the 2026-10-15 handoff carries it for six-six to decide.
//!
//! The two globals the palette lives in (`MODE`, `LOOK`) are process-wide, so every test takes
//! `GLOBALS` for its whole body and leaves the device where it found it.

use std::sync::{Mutex, MutexGuard};

use slot_store::Theme;
use slot_ui::palette::{
    chosen_ink, current, deepest, dim_ink, ground, idle_ink, ink, keycap, look_case, look_name,
    mode, next_look, panel, panel_ink, pill_fill, pill_ink, plate, plate_ink, ridge_ink, scrim,
    set_look,
    set_mode, Mode, LOOK_COUNT,
};
use slot_ui::{edge, housing, look, looks, opening, recess};

/// WCAG 2.x contrast ratio. The instrument this file reads palettes with, because luminance alone
/// ranks a saturated phosphor against black as unreadable when it is the brightest thing on the
/// screen — the mistake the 2026-10-15 review made and corrected.
fn wcag(a: [u8; 3], b: [u8; 3]) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let rel = |c: [u8; 3]| 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2]);
    let (la, lb) = (rel(a), rel(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// One lock for every test in this file: the palette is a pair of atomics, so two of these
/// running at once would read each other's mode. Poisoning is ignored — a test that failed inside
/// the lock must not take the other ten down with it, which is what a bare `unwrap()` here does.
static GLOBALS: Mutex<()> = Mutex::new(());

fn globals() -> MutexGuard<'static, ()> {
    GLOBALS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Puts the mode and the look back when it goes out of scope, so a test that fails halfway does
/// not leave the next one walking a row nobody asked for.
struct Pin {
    mode: Mode,
    look: usize,
}

impl Drop for Pin {
    fn drop(&mut self) {
        set_mode(self.mode);
        set_look(self.look);
    }
}

fn pin(m: Mode, l: usize) -> Pin {
    let p = Pin {
        mode: mode(),
        look: look(),
    };
    set_mode(m);
    set_look(l);
    p
}

fn lum(c: [u8; 3]) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

/// A `slot_chrome` surface, which is `[f32; 4]`, read back as the `[u8; 3]` it came from.
fn bytes(c: [f32; 4]) -> [u8; 3] {
    [
        (c[0] * 255.0).round() as u8,
        (c[1] * 255.0).round() as u8,
        (c[2] * 255.0).round() as u8,
    ]
}

fn rgb(c: [u8; 3]) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ]
}

/// What a `[f32; 4]` veil leaves of the picture behind it, over a mid grey — the two veils and the
/// one picture tone that is the worst case for either of them.
fn veiled(v: [f32; 4]) -> f32 {
    0.299 * (v[0] * 255.0 * v[3] + (1.0 - v[3]) * 128.0)
        + 0.587 * (v[1] * 255.0 * v[3] + (1.0 - v[3]) * 128.0)
        + 0.114 * (v[2] * 255.0 * v[3] + (1.0 - v[3]) * 128.0)
}

const DARK_NAMES: [&str; LOOK_COUNT] = ["默认", "GREEN", "AMBER", "RED", "VIOLET", "CYAN", "PINK"];
const LIGHT_NAMES: [&str; LOOK_COUNT] = ["默认", "GB", "PAPER", "LCDG", "LCDB", "SEPIA", "PINKBG"];

const ROWS: [(Mode, [&str; LOOK_COUNT]); 2] = [
    (Mode::Dark, DARK_NAMES),
    (Mode::Light, LIGHT_NAMES),
];

#[test]
fn the_table_is_two_rows_of_seven_with_the_device_at_the_front() {
    let _g = globals();
    let _pin = pin(Mode::Dark, 0);
    assert_eq!(looks().len(), 2, "the two modes are the two rows");
    for (m, names) in ROWS {
        set_mode(m);
        for (i, name) in names.iter().enumerate() {
            set_look(i);
            assert_eq!(look_name(i), *name, "{m:?} row entry {i} is misnamed");
            assert_eq!(look_name(i), current().name);
            assert!(!current().name.is_empty());
        }
        // Zero is the device in *both* rows, and it is the one name the two rows share.
        set_look(0);
        assert_eq!(current().name, "默认", "{m:?}: index zero is not the device");
    }
}

/// The whole point of index zero: a card that has never touched the key is the machine this build
/// has always been. Every accessor short-circuits there, so this is the regression net under the
/// twelve — and it is the test that would have caught a `look_case()` that forgot to ask.
#[test]
fn index_zero_is_byte_for_byte_the_device_it_was() {
    let _g = globals();

    let _pin = pin(Mode::Dark, 0);
    assert_eq!(ink(), [0xf5, 0xf2, 0xef]);
    assert_eq!(ground(), [0x00, 0x00, 0x00]);
    assert_eq!(panel_ink(), [0x1a, 0x19, 0x17]);
    assert_eq!(ridge_ink(), [0xd8, 0xd4, 0xcc]);
    assert_eq!(plate(), [0.0, 0.0, 0.0, 0.72]);
    assert_eq!(scrim(), [0.0, 0.0, 0.0, 0.62]);
    assert_eq!(keycap(), [0xb4, 0xb0, 0xa8]);
    // The case at zero is the card's `theme.txt`, which is `Theme::default` when the card carries
    // none — and `look_case()` must not have been consulted at all. This is the assertion that
    // makes "a look owns the case outright" safe to say.
    let t = Theme::default();
    assert_eq!(housing(), rgb(t.housing));
    assert_eq!(opening(), rgb(t.opening));
    assert_eq!(recess(), rgb(t.recess));
    assert_eq!(edge(), rgb(t.edge));

    set_mode(Mode::Light);
    assert_eq!(ink(), [0x1c, 0x1b, 0x1f]);
    assert_eq!(ground(), [0xe6, 0xe3, 0xdd]);
    assert_eq!(panel_ink(), [0xf4, 0xf1, 0xea]);
    assert_eq!(ridge_ink(), [0x6b, 0x64, 0x5b]);
    assert_eq!(scrim(), [1.0, 1.0, 1.0, 0.74]);
    let l = slot_ui::palette::LIGHT_CASE;
    assert_eq!(housing(), rgb(l.housing));
    assert_eq!(opening(), rgb(l.opening));
    assert_eq!(recess(), rgb(l.recess));
    assert_eq!(edge(), rgb(l.edge));
}

#[test]
fn cycling_walks_the_whole_row_and_comes_back_to_the_device() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        let mut seen = vec![look_name(0)];
        for _ in 1..LOOK_COUNT {
            set_look(next_look());
            seen.push(look_name(look()));
        }
        assert_eq!(
            seen,
            names.to_vec(),
            "{m:?}: SELECT+R1 does not walk the row in order"
        );
        // The press after the last ramp is the way back to the machine's own colours, which is
        // what makes the key safe to press without knowing where it is.
        assert_eq!(next_look(), 0, "{m:?}: the row does not wrap");
    }
}

/// A state file edited on a desktop must not be able to put the device past the end of its row,
/// the same way it cannot put it into a third mode.
#[test]
fn a_state_file_cannot_walk_off_the_row() {
    let _g = globals();
    let _pin = pin(Mode::Dark, 0);
    set_look(200);
    assert_eq!(look(), LOOK_COUNT - 1);
    assert_eq!(look_name(200), look_name(LOOK_COUNT - 1));
    // And the derived case is still a case, rather than panicking on an index.
    let _ = look_case();
    assert_eq!(next_look(), 0);
}

/// The ramp's own four numbers. The floors are the measured minima of the twelve minus headroom,
/// so a failure means the *derivation* moved rather than that a ramp is soft.
#[test]
fn every_look_is_legible_on_its_own_ground() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        for (i, name) in names.iter().enumerate() {
            set_look(i);
            let gl = lum(current().bg);
            let tl = lum(current().text);
            let dl = lum(current().dim);

            // Measured min 99 (RED), whose phosphor is deliberately dim.
            assert!(
                (tl - gl).abs() > 60.0,
                "{m:?}/{name}: ink is only {:.0}/765 from its ground",
                (tl - gl).abs()
            );
            // Same side of the ground, or "dimmer" is the wrong way round and a description
            // reads brighter than the head above it.
            assert_eq!(
                dl > gl,
                tl > gl,
                "{m:?}/{name}: dim and ink are on opposite sides of the ground"
            );
            // Measured min 44 (RED).
            assert!(
                (dl - gl).abs() > 25.0,
                "{m:?}/{name}: dim is only {:.0}/765 from the ground — it is not there",
                (dl - gl).abs()
            );
            // The rule itself: nearer the ground than the ink is. A ramp is free to spend its dim
            // on desaturation rather than darkness — PINKBG does exactly that, and its dim is
            // *lighter* than its text while still sitting nearer the ground.
            assert!(
                (dl - gl).abs() < (tl - gl).abs(),
                "{m:?}/{name}: dim is further from the ground than the ink is"
            );
        }
    }
}

/// The four plastics and the one order they have to keep: the recess is a step off the housing,
/// the opening is further in, and the edge is the lit line where the plastic is cut.
#[test]
fn every_look_case_keeps_the_four_surfaces_in_order() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        for (i, name) in names.iter().enumerate() {
            set_look(i);
            let t = look_case();
            assert!(
                lum(t.recess) < lum(t.housing),
                "{m:?}/{name}: the recess is not below the housing"
            );
            assert!(
                lum(t.opening) < lum(t.recess),
                "{m:?}/{name}: the opening is not inside the recess"
            );
            // Measured min +19 (PINKBG).
            assert!(
                lum(t.edge) > lum(t.housing) + 15.0,
                "{m:?}/{name}: the edge is not a lit rim ({:.0} vs {:.0})",
                lum(t.edge),
                lum(t.housing)
            );

            // The rim's two branches, pinned — this is the rule, and it is the same rule
            // `contrast.rs` states for the device's own case. A phosphor spends its `bright` on
            // the rim, because the highlight is the brightest thing it owns. A paper look's
            // `bright` is its *ink*, and an edge darker than the plastic it is the edge of is not
            // an edge, so those lighten the plastic instead.
            //
            // ⚠️ That second branch is honest flat alpha arithmetic and *not* colour-managed, so
            // on the three palest papers (PAPER, PINKBG and the light device itself, whose plastic
            // sits at 228–236) lightening by 1.35 clamps every channel and the rim comes out
            // white rather than as that paper's own highlight. Nothing else in the four fails —
            // the bands still order correctly — so this is asserted as the branch, and the clip is
            // written up as an observation for six-six rather than fixed here.
            let l = current();
            if m == Mode::Dark {
                for (i, name) in names.iter().enumerate() {
                    set_look(i);
                    assert_eq!(
                        look_case().edge,
                        current().bright,
                        "{m:?}/{name}: the phosphor's highlight is not on the rim"
                    );
                }
            } else {
                assert_ne!(
                    t.edge,
                    l.bright,
                    "{m:?}/{name}: a paper look put its ink on the rim"
                );
            }
        }
    }
}

/// **A look owns the case outright**, which is a rule stated in `slot_chrome::surface` and is not
/// the same claim as `look_case()` returning the right four numbers: it is that the drawn bands
/// *ask it*, rather than the card's `theme.txt` or the light case. Removing that `if` leaves every
/// other test in this file green — index zero is unaffected by it and `look_case()` is called
/// directly elsewhere — so this is the one that holds the composition the key depends on: the
/// twelve must not come out as a ramp's type on a black SP's plastic.
#[test]
fn a_look_paints_the_case_it_derives_rather_than_the_cards_theme() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        // The case a look must *not* be: the card's own in the dark, the built-in light one in
        // the light.
        let other = match m {
            Mode::Dark => Theme::default(),
            Mode::Light => slot_ui::palette::LIGHT_CASE,
        };
        set_mode(m);
        for (i, _) in names.iter().enumerate().skip(1) {
            set_look(i);
            let t = look_case();
            assert_eq!(housing(), rgb(t.housing), "{m:?} {}: housing", names[i]);
            assert_eq!(opening(), rgb(t.opening), "{m:?} {}: opening", names[i]);
            assert_eq!(recess(), rgb(t.recess), "{m:?} {}: recess", names[i]);
            assert_eq!(edge(), rgb(t.edge), "{m:?} {}: edge", names[i]);
            // And it is not the case a look must not be, or the look left half the machine as it
            // found it — the composition the key could not survive.
            assert_ne!(
                housing(),
                rgb(other.housing),
                "{m:?} {}: the housing is still the case's own",
                names[i]
            );
        }
    }
}

/// The one pairing on the machine that is a *choice* rather than a derivation: the label on a lit
/// face. It is `edge()`-backed, which is the brightest colour the case owns, so on a ramp whose
/// ink is a phosphor nearly as bright as the rim, "the palette's ink" is a label the same colour
/// as its own button — measured at **1.08:1** for GREEN. `chosen_ink` picks the ramp's own
/// colour that reads hardest instead, and this holds it to a floor rather than to a derivation.
///
/// 4.5:1 is the body-text bar and it is the right one here: this is a control's name, read at a
/// glance, and it is the row the user's finger is on. Measured worst of the twelve is PINKBG at
/// **5.16:1**; before this, the device's own palette was **2.13:1** light / **2.44:1** dark.
#[test]
fn the_label_on_a_lit_face_reads_hardest_it_can() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        for (i, _) in names.iter().enumerate().skip(1) {
            set_look(i);
            let face = look_case().edge;
            let ink = chosen_ink();
            let r = wcag(ink, face);
            assert!(
                r >= 4.5,
                "{m:?}/{}: the label on the lit face is only {r:.2}:1 — it is not readable",
                names[i]
            );
            // And it is one of the colours this palette already puts on the screen: the ramp's
            // three, or the case's own window. Nothing is invented off the table.
            let l = current();
            let allowed = [l.text, l.dim, l.bg, bytes(opening())];
            assert!(
                allowed.contains(&ink),
                "{m:?}/{}: the chosen label {ink:?} is not one of the palette's colours",
                names[i]
            );
            // The floor is not met by accident: whatever is picked must beat what the two
            // obvious answers would have given, or this is a worse label than the plain one.
            let best_of_four = [l.text, l.dim, l.bg, bytes(opening())]
                .iter()
                .map(|c| wcag(*c, face))
                .fold(0.0f32, f32::max);
            assert!(
                (r - best_of_four).abs() < 0.001,
                "{m:?}/{}: {r:.2}:1 is not the best the ramp offers ({best_of_four:.2}:1)",
                names[i]
            );
        }
    }
}

/// **The device's own label on a lit face takes the rule too** — the one accessor in `palette.rs`
/// that is deliberately *not* zero-regression, on six-six's call (2026-10-16).
///
/// It used to short-circuit to `opening` like every other accessor, which left it at **2.07:1 in
/// the light and 2.81:1 in the dark — the worst of all thirteen**, with the *chosen* button harder
/// to read than the unselected one beside it. Holding a regression rule over a defect that the
/// rule itself exposed is the wrong trade, so the device now takes the same rule as the twelve.
///
/// This test is the record of that decision: it asserts the improvement, and the bounds, so a
/// later "restore zero-regression here" cannot land without tripping it.
#[test]
fn the_devices_lit_label_is_readable_like_everyone_elses() {
    let _g = globals();
    let _pin = pin(Mode::Dark, 0);
    // Was (0x05, 0x05, 0x08) — `opening`. Now the ramp's own ink, which is what reads hardest on
    // the dark case's rim.
    assert_eq!(chosen_ink(), [0xf5, 0xf2, 0xef], "dark: not the ink the rule picks");
    let dark = wcag(chosen_ink(), bytes(edge()));
    set_mode(Mode::Light);
    // Was (0xb0, 0xa8, 0x9a) — `LIGHT_CASE.opening`. Now the light ink.
    assert_eq!(chosen_ink(), [0x1c, 0x1b, 0x1f], "light: not the ink the rule picks");
    let light = wcag(chosen_ink(), bytes(edge()));
    assert!(
        dark >= 4.5 && light >= 4.5,
        "the device's lit label is {dark:.2} dark / {light:.2} light, under the 4.5 floor"
    );
    // Measured 2026-10-16: 2.81 -> 6.50 dark, 2.07 -> 15.06 light **on the rendered screen**.
    // These two bounds are the *flat* colour pair and come out a shade higher (7.49 / 15.46),
    // because the face is drawn through `menu::mould`'s gradient — the label sits on plastic that
    // is a few per cent lighter at the top than `edge()` itself. Both numbers are above the floor;
    // the render's is the one the eye gets, and it is the stricter of the two.
    assert!(
        (7.0..8.0).contains(&dark) && (15.0..16.0).contains(&light),
        "the device's lit label moved further than the measured fix: {dark:.2}/{light:.2}"
    );
}

/// **A resting label steps back without vanishing** — the other half of the pairing above, and a
/// rule nobody asked for until 2026-10-16, when 六六 wanted the unselected keys duller than the
/// selected one ("选中键为亮色，未选中的为暗色").
///
/// The first cut was `dim_ink`, which is right on the device's own palette and **wrong on every
/// ramp**: `dim` is a step back from the ramp's *ground*, and a resting button is drawn on the
/// ramp's *plastic*. Measured on the rendered screen, RED fell to **1.30:1**, VIOLET 1.57, PINK
/// 1.76, SEPIA 1.91, AMBER 1.98 — 六六 saw it on the RED card and called it: "红色那张换过来就对了".
/// `idle_ink` picks the dullest of the palette's own colours that still clears 3:1 instead.
///
/// Three things are held here, because any one of them alone can be satisfied by a bad answer:
/// the floor (it reads), the ceiling (it is not brighter than the label it is stepping back from),
/// and the table (it is a colour this palette already draws — nothing invented off the ramp).
#[test]
fn a_resting_label_steps_back_but_stays_readable() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        for (i, _) in names.iter().enumerate().skip(1) {
            set_look(i);
            let face = bytes(housing());
            let label = idle_ink();
            let r = wcag(label, face);
            let charged = wcag(ink(), face);
            // Measured worst is PINKBG 3.03:1 — the one ramp whose ink and its plastic are both
            // pink, answered by the case's own window rather than by the ramp.
            assert!(
                r >= 2.9,
                "{m:?}/{}: a resting label fell to {r:.2}:1 — it does not read",
                names[i]
            );
            // ★ "退后"的字面承诺：**只要该 look 自己的墨色本身够读（同一把 4.0 的尺），
            //   就只许选它或比它更钝的**。唯一的例外正是 PINKBG —— 它的墨色在粉塑料上
            //   只有 1.91:1，那里"退后"让位给"能读"，规则越过 `ink()` 取 case 的窗色（3.03:1）。
            //   所以这条断言是有条件的：拿一条会在坏调色板上必然失败的规则去卡规则，是错的。
            if charged >= 4.0 {
                assert!(
                    r <= charged + 0.001,
                    "{m:?}/{}: the resting label ({r:.2}:1) out-shines the ramp's own ink ({charged:.2}:1)",
                    names[i]
                );
            }
            let l = current();
            let allowed = [l.text, l.dim, l.bg, bytes(edge()), bytes(opening())];
            assert!(
                allowed.contains(&label),
                "{m:?}/{}: the resting label {label:?} is not one of the palette's colours",
                names[i]
            );
        }
    }
}

/// **The device's own resting label is byte-for-byte what `dim_ink` gives** — the zero-regression
/// half of the rule, and the reason 六六's call could land without touching the shipped look: on
/// index 0 the palette's own step back *is* the dullest colour that clears the floor, so the new
/// rule agrees with the old derivation instead of replacing it.
#[test]
fn the_devices_resting_label_is_still_the_palettes_own_step_back() {
    let _g = globals();
    let _pin = pin(Mode::Dark, 0);
    assert_eq!(idle_ink(), dim_ink(), "dark: the rule left the device's own palette");
    assert_eq!(idle_ink(), [0xba, 0xb8, 0xb6], "dark: not the grey that shipped");
    set_mode(Mode::Light);
    assert_eq!(idle_ink(), dim_ink(), "light: the rule left the device's own palette");
    assert_eq!(idle_ink(), [0x4c, 0x4b, 0x4d], "light: not the grey that shipped");
}

/// The same label on the *unlit* rows, for the two screens that draw one: a row's name is `ink`
/// on `housing`, and its value is `dim_ink` on `housing`. Both are derived and both are asserted
/// here because the two screens (menu and settings) share these functions and must not drift.
#[test]
fn a_row_that_is_not_chosen_is_readable_too() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        for (i, _) in names.iter().enumerate().skip(1) {
            set_look(i);
            let t = look_case();
            let name = wcag(ink(), t.housing);
            // ⚠️ **Measured worst: PINKBG 1.91, GREEN 2.27.** The name is plain `ink` on
            // `housing`, and on a ramp whose plastic comes out close to its ink the label nearly
            // vanishes — the same eleven-odd looks that read at 4–8:1 have three that do not. Kept
            // as-is on 2026-10-16: the name is the row's own word, and on the device's palette it
            // is 12.99 / 9.26 — the ramps that fall short are the ones whose *ink* is the problem,
            // not this pairing, and moving it would move all twelve.
            assert!(
                name >= 1.85,
                "{m:?}/{}: a row's name fell to {name:.2}:1, below the worst measured (PINKBG 1.91)",
                names[i]
            );
            // ★ **The value was the machine's weakest pairing, and it is fixed (2026-10-16).**
            // It used to be the ramp's `dim` used raw, while the surface under it is `housing` —
            // the plastic *derived* from that ramp's ground — so on a ramp whose `dim` is a
            // mid-tone close to that plastic the two nearly coincide: **RED 1.25:1, AMBER 1.89,
            // GREEN 2.27**, i.e. on RED the value on a settings row was invisible. The review left
            // it for 六六 because the fix moves all twelve; his call ("主次分明就行") is that "the
            // second thing still has to be legible", so it now goes through `palette::idle_ink()` —
            // the same function, the same 4.0 floor and the same reasoning as a menu key that is
            // not in hand. One standard for "secondary" per screen, not two.
            let value = wcag(idle_ink(), t.housing);
            assert!(
                value >= 2.9,
                "{m:?}/{}: a row's value fell to {value:.2}:1, below the worst measured (PINKBG 3.03)",
                names[i]
            );
        }
    }
}

/// The type over the case, in the two pairings the menu and the settings screen draw for a row
/// that is *not* the one in hand: its letters in the window (`opening`), and the head of a row on
/// the housing.
#[test]
fn the_type_reads_in_the_window_and_on_the_housing_of_every_look() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        for (i, name) in names.iter().enumerate() {
            set_look(i);
            let t = look_case();
            let i_l = lum(ink());
            // Measured min 22, at PINKBG — and PINKBG is the one ramp where the window this
            // project derives is nearly the ink itself. It is named here rather than papered
            // over: the floor is below it so the test holds the *derivation*, and the number is
            // in the table at the top of the file for six-six to look at on the device.
            let window = (i_l - lum(t.opening)).abs();
            assert!(
                window > 15.0,
                "{m:?}/{name}: ink on the window is only {window:.0}/765"
            );
            // Measured min 59 (RED).
            let h = (i_l - lum(t.housing)).abs();
            assert!(
                h > 45.0,
                "{m:?}/{name}: ink on the housing is only {h:.0}/765"
            );
        }
    }
}

/// The type over a *picture*: the shelf can carry a wallpaper, and both veils are the look's own
/// ground rather than black or white. Mid grey behind is the worst case — the tone least like any
/// veil on either side of the scale.
#[test]
fn the_type_reads_over_a_wallpaper_in_every_look() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        for (i, name) in names.iter().enumerate() {
            set_look(i);
            let i_l = lum(ink());
            for (veil, what) in [(scrim(), "the scrim"), (plate(), "the HUD's plate")] {
                let g = veiled(veil);
                // Measured min 55 (RED).
                assert!(
                    (i_l - g).abs() > 45.0,
                    "{m:?}/{name}: ink over {what} is only {:.0}/765",
                    (i_l - g).abs()
                );
            }
        }
    }
}

/// The shortcut card, where the caps are filled squares on the card's own panel and the letter in
/// a cap is the panel's colour by rule rather than by choice.
#[test]
fn a_key_cap_separates_from_the_card_it_sits_on() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        for (i, name) in names.iter().enumerate() {
            set_look(i);
            let card = lum(bytes(panel()));
            let cap = lum(keycap());
            let letter = lum(panel_ink());
            // Measured min 47 (PINKBG).
            assert!(
                (card - cap).abs() > 25.0,
                "{m:?}/{name}: a cap is only {:.0}/765 from the card",
                (card - cap).abs()
            );
            assert!(
                (cap - letter).abs() > 25.0,
                "{m:?}/{name}: a cap's letter is only {:.0}/765 from the cap",
                (cap - letter).abs()
            );
        }
    }
}

/// The one place both rows are read together: a look is *within* a mode, so the same index means a
/// phosphor in the dark and a paper in the light. If the two rows ever collapsed into one, the
/// mode key would have nothing left to do and the same six names would be up in both.
#[test]
fn the_same_index_is_a_different_colour_in_each_mode() {
    let _g = globals();
    let _pin = pin(Mode::Dark, 0);
    for i in 1..LOOK_COUNT {
        set_mode(Mode::Dark);
        set_look(i);
        let dark = (current().bg, current().text);
        set_mode(Mode::Light);
        set_look(i);
        let light = (current().bg, current().text);
        assert_ne!(dark, light, "index {i} is the same colouring in both modes");
    }
}

/// No two of the twelve are the same colouring: a row you cannot tell apart is a row you cannot
/// aim at, and the two rows being distinct is what keeps the mode key meaningful.
#[test]
fn the_twelve_are_distinct() {
    let _g = globals();
    let mut seen: Vec<([u8; 3], [u8; 3])> = Vec::new();
    for (m, _) in ROWS {
        let _pin = pin(m, 0);
        let row = match m {
            Mode::Dark => 0,
            Mode::Light => 1,
        };
        for i in 1..LOOK_COUNT {
            let pair = (looks()[row][i].bg, looks()[row][i].text);
            assert!(
                !seen.contains(&pair),
                "{m:?} entry {i} repeats the colouring of another look"
            );
            seen.push(pair);
        }
    }
    assert_eq!(seen.len(), 12);
}
/// ★ 2026-10-24（六六）：「**部分主题配色游戏名看不清**」＋「**字母轮不显示**」。
///
/// 两条都是"字与它压着的那张面同色"，而且都只在亮色模式出现 —— 所以这里把**两个新的
/// 判色访问器**在十三套 look、两种模式下逐个量一遍，门槛就是它们在文档里写的那两条。
///
/// 为什么必须有这条测试：这两个 bug 都不是"某套色选得软"，是**深色主题下根本量不出来**。
/// 胶囊在深色下是 `ground`（黑）、字母是 `ink`（近白），18.83:1 —— 换成亮色才变成
/// `deepest()` 落到 `ink` 上，1.00:1，一个字不剩。只量暗组的话，这条线永远是绿的。
///
/// 顶栏：`plate_ink()` 压 `opening()`（`desktop.rs` 里 Tab 的填充就是这个色）。
/// 字母轮：`pill_ink()` 压 `deepest()`（`letters::pill_face` 的填充）。
#[test]
fn the_title_and_the_letter_index_read_on_the_surfaces_they_sit_on() {
    let _g = globals();
    for (m, names) in ROWS {
        let _pin = pin(m, 0);
        for (i, name) in names.iter().enumerate() {
            set_look(i);
            // 门槛与 palette 里那两个常量一致：顶栏 26px 大字 3.5，字母 4.5。
            let t = wcag(plate_ink(), bytes(opening()));
            assert!(
                t >= 3.5,
                "{m:?}/{name}: 顶栏游戏名只有 {t:.2}:1（要 3.5）"
            );
            let p = wcag(pill_ink(), pill_fill());
            assert!(
                p >= 4.5,
                "{m:?}/{name}: 字母轮上的字母只有 {p:.2}:1（要 4.5）—— 就是「字母轮不显示」"
            );
        }
    }
}

/// 上面那条会掩盖一个方向：把 `pill_ink` 写成"永远返回纸色"也能过，但暗色主题下字母轮
/// 就反过来变成浅底浅字。所以这条钉住**暗色主题（那时从来没坏过）在字母轮上没被改动**。
///
/// ⚠️ **亮色主题不在这条里，而且不能在这条里**：设备自己的亮色主题原本也是同一个 bug
/// （胶囊 `deepest()` 落到 `ink`、字母也画 `ink`，1.00:1），所以那里的字色**必须**变。
/// 拿"零回归"去要求它就是把这个 bug 钉死在这儿。
#[test]
fn the_devices_letter_index_is_byte_for_byte_what_it_was() {
    let _g = globals();
    let _pin = pin(Mode::Dark, 0);
    assert_eq!(
        pill_ink(),
        ink(),
        "Dark: 设备原色下字母轮的字色必须还是 `ink()`，不能跟着新规则走"
    );
    assert_eq!(
        pill_fill(),
        deepest(),
        "Dark: 设备原色下胶囊仍然是 `deepest()`（黑），不该被压暗"
    );
}
