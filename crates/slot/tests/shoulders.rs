//! The pad's two shoulders, which are the one channel it has that is not a keymap row.
//!
//! ★ 2026-10-25. `L2`/`R2` are rewind and fast forward on every machine, and `Pad` has never
//! reported a bit for either: `legacy_bit` gives them zero and the keymap's six keys do not
//! include them, so a GBA/GBC/MD core sees exactly the pads it always did. A PlayStation is the
//! exception — `L2`/`R2` are real buttons there — and `Pad::set_shoulders` is that exception.
//!
//! The pair matters more than either half: `Gestures::set_l2r2_to_game` decides **what a press
//! becomes** (an `Action` or the transport) and `Pad::set_shoulders` decides **what the core
//! sees**. Setting only one of them reads on the device as "the key does nothing at all" — the
//! action arrives but the mask has no bit, or the bit is there but nothing ever sets `down[]`.

use slot::input::Pad;
use slot_input::{Action, Btn};
use slot_retro::ButtonMask;

/// The whole point of the channel: on, a shoulder reports its libretro bit; off, it reports
/// nothing at all. The second half is what keeps the other six machines byte-identical.
#[test]
fn a_shoulder_reports_a_bit_only_when_the_machine_asks_for_it() {
    for (b, bit) in [(Btn::L2, ButtonMask::L2), (Btn::R2, ButtonMask::R2)] {
        let mut off = Pad::default();
        off.set_shoulders(false);
        off.apply(Action::GbaDown(b));
        assert_eq!(
            off.mask().0 & bit,
            0,
            "{b:?} reported a bit on a machine that never asked for it"
        );

        let mut on = Pad::default();
        on.set_shoulders(true);
        on.apply(Action::GbaDown(b));
        assert_eq!(
            on.mask().0 & bit,
            bit,
            "{b:?} did not reach the core on PS1"
        );

        // And the release takes it back — a mask that cannot clear a shoulder is a core that
        // thinks L2 is held for the rest of the cart.
        on.apply(Action::GbaUp(b));
        assert_eq!(on.mask().0 & bit, 0, "{b:?} stayed down after its release");
    }
}

/// The channel is per shoulder, not a pair: a game that wants `L2` and not `R2` gets exactly
/// that. Written as one press each rather than a both-then-none, because an arm that read the
/// wrong index would pass a test that only ever held them together.
#[test]
fn the_two_shoulders_are_independent() {
    let mut pad = Pad::default();
    pad.set_shoulders(true);
    pad.apply(Action::GbaDown(Btn::L2));
    assert_eq!(pad.mask().0 & ButtonMask::L2, ButtonMask::L2);
    assert_eq!(pad.mask().0 & ButtonMask::R2, 0);
}

/// `clear()` is what a menu or a switch-away uses to stop a game holding what was down when the
/// input left it. A shoulder has to be released by it too, or a PS1 game keeps L2 held across a
/// menu that was opened with the shoulder down.
#[test]
fn clearing_the_pad_lets_go_of_the_shoulders_too() {
    let mut pad = Pad::default();
    pad.set_shoulders(true);
    pad.apply(Action::GbaDown(Btn::L2));
    pad.apply(Action::GbaDown(Btn::R2));
    pad.clear();
    assert_eq!(pad.mask().0 & (ButtonMask::L2 | ButtonMask::R2), 0);
}

/// Turning the channel off mid-press releases the bits immediately, without needing an up edge
/// the core may never be sent: a cart can be pulled while a shoulder is down. The `down[]` state
/// is kept (it is which keys are held, not which bits they make — the same rule `set_remap`
/// follows), so turning it back on finds the key still held.
#[test]
fn switching_the_channel_off_drops_the_bits_at_once() {
    let mut pad = Pad::default();
    pad.set_shoulders(true);
    pad.apply(Action::GbaDown(Btn::L2));
    pad.set_shoulders(false);
    assert_eq!(pad.mask().0 & ButtonMask::L2, 0, "the bit outlived the machine");
    pad.set_shoulders(true);
    assert_eq!(
        pad.mask().0 & ButtonMask::L2,
        ButtonMask::L2,
        "the key was forgotten instead of the bit being withheld"
    );
}