use std::time::Duration;

use crate::{Battery, Charge, LedState, LidPolicy, Platform};

/// The lid policy and the panel it acts on, which have to be one object because the policy
/// takes no arguments. `depth` and `timeout` are constructor values: spec section 9 defers
/// both to hardware bring-up and neither forks the code.
///
/// `timeout` is the exception, and only because a card can now be asked for it: `new` takes the
/// value the frontend falls back on, and `set_timeout` replaces it with the one the user chose.
/// It stays a field rather than being read from the card here because this crate does not know
/// what a card is — the frontend hands the answer over as soon as it has read it, and again
/// every time the setting changes.
///
/// ★ **It is an `Option`, because the setting has a value that means "no timer at all"**: the
/// screen's 「仅息屏」 / "Screen Off Only" darkens the panel on a shut lid and never stops the
/// machine. That cannot be spelled `Duration::ZERO` — zero is the *shortest* wait, so a zero
/// timeout is already expired on the next frame and the lid would power the device off the
/// instant it shut, which is the exact opposite of what the choice says.
pub struct Power {
    platform: Box<dyn Platform>,
    /// What the panel goes back to on open. The platform has no getter, so remembering
    /// every step that passed through here is the only way to know.
    level: u8,
    closed: bool,
    timeout: Option<Duration>,
}

impl Power {
    pub fn new(platform: Box<dyn Platform>, timeout: Option<Duration>) -> Self {
        Power {
            platform,
            level: 0,
            closed: false,
            timeout,
        }
    }

    /// How long a shut lid waits before the machine stops, or `None` for a timer switched off.
    /// Takes effect on the next doze, so a card changed while the lid is already down keeps the
    /// timer it went down with — which is right: the setting is what the *next* walk-away waits
    /// for.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    /// A level set while the lid is shut is remembered rather than lit. The keyboard still
    /// reaches a dozing host, and a panel that comes on inside a closed clamshell is the
    /// one thing the doze exists to prevent.
    pub fn set_backlight(&mut self, step: u8) {
        self.level = step;
        if !self.closed {
            self.platform.set_backlight(step);
        }
    }

    pub fn battery(&self) -> Option<Battery> {
        self.platform.battery()
    }

    pub fn charge(&self) -> Charge {
        self.platform.charge()
    }

    pub fn set_led(&mut self, state: LedState) {
        self.platform.set_led(state)
    }

    /// Not gated on the lid the way the backlight is. Whatever stopped the motor for the
    /// doze has to be the thing that starts it again, or a cart resumes buzzing on wake.
    pub fn set_rumble(&mut self, strength: u16) {
        self.platform.set_rumble(strength);
    }

    /// Straight through: this is a cable coming back, which has nothing to do with the lid,
    /// the level or anything else this type arbitrates.
    pub fn relink_adb(&mut self) -> bool {
        self.platform.relink_adb()
    }

    pub fn poweroff(&mut self) -> ! {
        self.platform.poweroff()
    }

    pub fn restart(&mut self) -> ! {
        self.platform.restart()
    }

    pub fn now(&self) -> i64 {
        self.platform.now()
    }

    pub fn set_clock(&mut self, secs: i64) {
        self.platform.set_clock(secs);
    }
}

impl LidPolicy for Power {
    fn on_close(&mut self) {
        // A hall sensor bounces, and a second close would otherwise take the dark panel
        // for the level to restore.
        if self.closed {
            return;
        }
        self.closed = true;
        self.platform.set_backlight(0);
    }

    fn on_open(&mut self) {
        self.closed = false;
        self.platform.set_backlight(self.level);
    }

    /// `None` is a fit answer for "how long": the policy's own answer is that there is no
    /// timer, and a caller that wants a number has to say what it does without one (see
    /// `App::doze_expired`, which refuses to expire).
    fn timeout(&self) -> Option<Duration> {
        self.timeout
    }
}
