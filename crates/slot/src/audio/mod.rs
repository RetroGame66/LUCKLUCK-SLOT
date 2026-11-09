mod alsa;
#[cfg(feature = "host")]
mod host;
mod ring;
mod sfx;
mod sink;
mod stub;
pub mod volume;

pub use alsa::AlsaSink;
#[cfg(feature = "host")]
pub use host::HostAudio;
pub use ring::{ring_capacity, Ring};
pub use sfx::Sfx;
pub use sink::{AudioError, AudioSink};
pub use stub::StubSink;

/// The GBA's own rate. slot plays nothing else, so the device is opened for it before there
/// is a core to ask, and a device that takes it needs no resampling at all.
pub const GBA_HZ: u32 = 32_768;

/// How hard the audio path trades latency for slack.
///
/// What the player feels in a rhythm game is the distance between the beat they *hear* and
/// the press the game *judges*. That is the depth of the ring plus whatever the device
/// buffers, so this is the one knob that moves it.
///
/// Chosen on the shelf and applied when the next cart is inserted: the device buffer is fixed
/// when the PCM is opened, so changing it means reopening the hardware, and on the shelf there
/// is no game to interrupt.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Profile {
    /// The shipped margins, sized against a slow card and a busy scheduler.
    #[default]
    Stable,
    /// The middle: noticeably tighter, still room for a late frame at either end.
    Balanced,
    /// For rhythm games. One video frame of queue plus the device's own floor, which is about
    /// as far as this pipeline can go before a late frame starts costing audio.
    Strict,
}

impl Profile {
    pub const ALL: [Profile; 3] = [Profile::Stable, Profile::Balanced, Profile::Strict];

    /// What `System/audio.txt` holds. Missing or unparsable means `Stable`, the shipped
    /// behaviour: a card edited on a PC must never be able to make the machine click by typo.
    pub fn parse(s: &str) -> Option<Profile> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stable" => Some(Profile::Stable),
            "balanced" => Some(Profile::Balanced),
            "strict" => Some(Profile::Strict),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Stable => "stable",
            Profile::Balanced => "balanced",
            Profile::Strict => "strict",
        }
    }

    // The on-screen label lives with the toast it is drawn from: see `Toast::AudioStable` in
    // `slot-ui`. Keeping it here too would be a second copy of the same three strings.

    pub fn next(self) -> Profile {
        match self {
            Profile::Stable => Profile::Balanced,
            Profile::Balanced => Profile::Strict,
            Profile::Strict => Profile::Stable,
        }
    }

    pub fn prev(self) -> Profile {
        match self {
            Profile::Stable => Profile::Strict,
            Profile::Balanced => Profile::Stable,
            Profile::Strict => Profile::Balanced,
        }
    }

    /// Where the DRC parks the ring, in video frames of audio. `Stable` reproduces the shipped
    /// steady state exactly (`capacity / 2`); the others are fixed counts, which is what makes
    /// them independent of how big the ring happens to be.
    pub fn target(self, capacity_frames: usize) -> usize {
        match self {
            Profile::Stable => capacity_frames / 2,
            Profile::Balanced => capacity_frames * 3 / 8,
            Profile::Strict => capacity_frames / 4,
        }
    }

    /// What ALSA is asked to buffer, in microseconds.
    pub fn latency_us(self) -> u32 {
        match self {
            Profile::Stable => 40_000,
            Profile::Balanced => 24_000,
            Profile::Strict => 16_000,
        }
    }

    /// Frames per device write. The device needs at least two periods, so this is what sets
    /// the floor under `latency_us`: at the GBA's rate 512 frames is 15.6 ms and 256 is 7.8 ms.
    pub fn period_frames(self) -> usize {
        match self {
            Profile::Stable => 512,
            Profile::Balanced => 256,
            Profile::Strict => 256,
        }
    }
}

/// The sink this build talks to: cpal on a desktop, ALSA on the device. A sink that fails to
/// open is not a failure to boot either way, since a ring nothing drains still lets the
/// emulator run.
#[cfg(feature = "host")]
pub fn open_sink() -> Box<dyn AudioSink> {
    Box::new(HostAudio::new())
}

#[cfg(not(feature = "host"))]
pub fn open_sink() -> Box<dyn AudioSink> {
    Box::new(AlsaSink::new())
}

/// The one output device, shared by everything that plays through it.
///
/// # Why this exists (2026-10-07, measured on the card)
///
/// The shelf opens a sink at boot and keeps it for the whole session (`Session::sink`) — a cart
/// clicks home while it is still on its way in, and that is exactly when there is no core to own
/// one. The deck used to open a **second** `AudioSink` of its own, and on the H700 that cannot
/// work: `plug:default` falls through to a raw `hw:` slave, which is **exclusive**. The second
/// `open` came back busy, and the decode loop's `if opened { ring.push_blocking(..) }` then
/// pushed nothing at all — so a track "played" with no device behind the ring, which is no
/// throttle whatsoever. Measured: 5.1 s of wall clock decoded **209.2 s** of audio, about forty
/// times speed, with the card silent. That is the user's "no sound, and it plays too fast".
///
/// So there is one device and one ring, and whoever needs it asks for it. `open` is re-entrant
/// (`AlsaSink::open` closes what it had first) and the ring outlives every open, so the deck
/// reopening it at the track's own rate is the ordinary path rather than a special case — and
/// the GBA worker, which reads `ring.sample_rate()` once when its core starts, sees whichever
/// rate was last taken rather than a ring nothing is draining.
pub struct Device {
    sink: std::sync::Mutex<Box<dyn AudioSink>>,
}

impl Device {
    pub fn new() -> Self {
        Device {
            sink: std::sync::Mutex::new(open_sink()),
        }
    }

    /// Open, or reopen, the device at `hz`. The caller takes the ring from `ring` and writes
    /// into it; nothing else about the device is its business.
    pub fn open(&self, hz: u32, profile: Profile) -> Result<(), AudioError> {
        self.with(|s| s.open(hz, profile))
    }

    /// The ring everything shares. Its rate is the device's, and it is 0 before the first open.
    pub fn ring(&self) -> std::sync::Arc<Ring> {
        self.with(|s| s.ring())
    }

    /// The guard is held for the call and no longer — **the deck must never hold it across its
    /// own loop**: the lock is what keeps one opener out of another's way, not what paces the
    /// writer.
    ///
    /// Not a plain `lock().unwrap()`: a poisoned lock would turn "an audio thread panicked" into
    /// "the next frame panics too", and a frontend that cannot draw is worse than one that
    /// cannot play.
    fn with<T>(&self, f: impl FnOnce(&mut Box<dyn AudioSink>) -> T) -> T {
        let mut guard = match self.sink.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&mut guard)
    }
}

impl Default for Device {
    fn default() -> Self {
        Device::new()
    }
}
