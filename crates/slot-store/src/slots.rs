//! The deliberate saves of one cart, in **numbered slots** rather than in a ring.
//!
//! This is the model the in-game menu asked for on 2026-10-20: `Auto` plus `01` to `99`, and
//! nothing is ever evicted behind the player's back. The ring it replaces held ten saves and
//! dropped the oldest when an eleventh came along, which is the right shape for "undo the last
//! few seconds" and the wrong shape for "this is where I keep my run" — a save the machine
//! threw away is not a slot the player chose.
//!
//! **The directory is the ring's**, so a card saved by an older build keeps its saves: same
//! `States/<core>/<system>/<stem>/`, and the same `resume.state` beside them. What changed is
//! only how the deliberate ones are *named* — `auto.state`, `01.state`, `02.state` — and one
//! pass at first use renames whatever the ring left behind into the first ten slots (oldest
//! first), so a card that was saved ten times comes up with `01` to `10` already occupied.
//!
//! ⚠️ **The slot's number is the file's name and the save's only identity.** There is no
//! manifest: the card is meant to be edited by hand, and a directory you can read is worth more
//! than a file that can fall out of step with it. The **time** therefore comes from the file's
//! own mtime, not from a name — which is also why a slot copied off another card keeps the time
//! it was copied, and a slot whose mtime the filesystem lost reads as "saved, time unknown"
//! rather than as empty.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::atomic::atomic_write;
use crate::core::Core;

/// How many numbered slots there are. `01` through `99`, and `Auto` on top of them.
pub const SLOT_MAX: u8 = 99;
/// The slots a menu walks: `Auto` first, then `01` to `99`.
pub const SLOT_COUNT: usize = SLOT_MAX as usize + 1;

const STATE_EXT: &str = "state";
const THUMB_EXT: &str = "png";

/// One slot: `Auto`, or one of the ninety-nine numbered ones.
///
/// `Auto` is first in every list because it is the one the machine writes to on its own —
/// an autosave is a thing you fall back on, not a thing you choose, and a menu that buried it
/// under ninety-nine numbers would be a menu that hid the only slot that is never empty.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Slot {
    Auto,
    Num(u8),
}

impl Slot {
    /// The name the file is written under: `auto`, `01`, `02`… Two digits, zero-filled, so
    /// a directory listing sorts the way the menu walks it.
    pub fn name(self) -> String {
        match self {
            Slot::Auto => "auto".to_string(),
            Slot::Num(n) => format!("{n:02}"),
        }
    }

    /// What the menu shows on the row. `Auto` keeps its capital: it is a name, not a number.
    pub fn label(self) -> String {
        match self {
            Slot::Auto => "Auto".to_string(),
            Slot::Num(n) => format!("{n:02}"),
        }
    }

    /// The slot at a position in the list `SLOT_COUNT` long. `0` is `Auto`, `1` is `01`.
    /// ⚠️ **Wrapping, not saturating**: a list walked with ← and → has no end to stop at, and
    /// the alternative is a cursor that sticks at 99 and needs a second rule to say why.
    pub fn at(index: usize) -> Slot {
        match index % SLOT_COUNT {
            0 => Slot::Auto,
            i => Slot::Num(i as u8),
        }
    }

    /// Where this one sits in that list — the inverse of `at`.
    pub fn index(self) -> usize {
        match self {
            Slot::Auto => 0,
            Slot::Num(n) => usize::from(n),
        }
    }

    /// From a directory entry's stem. Anything that is not a slot is `None`, which is what
    /// keeps `resume` and a file the player dropped in from being offered as a save.
    pub fn parse(stem: &str) -> Option<Slot> {
        if stem == "auto" {
            return Some(Slot::Auto);
        }
        if stem.len() != 2 || !stem.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let n: u8 = stem.parse().ok()?;
        (1..=SLOT_MAX).contains(&n).then_some(Slot::Num(n))
    }
}

/// A slot that holds a save. `secs` is when it was saved, off the state file's mtime.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotEntry {
    pub slot: Slot,
    pub state: PathBuf,
    pub thumb: PathBuf,
    /// `None` when the filesystem will not say — older cards, and cards written by a build
    /// that predates this module. The row says "saved" either way; only the time is missing.
    pub secs: Option<i64>,
}

/// The slots of one cart.
pub struct StateSlots {
    dir: PathBuf,
}

impl StateSlots {
    /// The same directory `StateRing::new` uses, for the same reason: states are core private,
    /// because a serialized machine from one emulator cannot be loaded by another.
    pub fn new(root: &Path, core: Core, system: crate::System, stem: &str) -> Self {
        StateSlots {
            dir: root
                .join("States")
                .join(core.as_str())
                .join(system.dir_name())
                .join(stem),
        }
    }

    /// The card that has not been sorted into machine folders: `States/<core>/<stem>`. Read
    /// after the sorted directory came back empty, exactly as the ring's legacy path is.
    pub fn legacy(root: &Path, core: Core, stem: &str) -> Self {
        StateSlots {
            dir: root.join("States").join(core.as_str()).join(stem),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Write one slot. Whatever was in it is replaced outright: a slot is a place you put a
    /// save, and a machine that asked "are you sure" over a numbered slot would be asking
    /// about a decision the player made by walking to it.
    ///
    /// Thumb first, same as the ring did — an entry is listed by its state file, so a cut
    /// between the two writes leaves an unlisted orphan rather than a picture with no save.
    pub fn write(&self, slot: Slot, state: &[u8], thumb_png: &[u8]) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        atomic_write(&self.path(slot, THUMB_EXT), thumb_png)?;
        atomic_write(&self.path(slot, STATE_EXT), state)
    }

    /// Both halves of one slot, or `None` when it is empty. A thumbnail that never landed
    /// reads as empty rather than as a failure — the state is the part worth keeping.
    pub fn read(&self, slot: Slot) -> std::io::Result<Option<(Vec<u8>, Vec<u8>)>> {
        let state = match std::fs::read(self.path(slot, STATE_EXT)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let thumb = std::fs::read(self.path(slot, THUMB_EXT)).unwrap_or_default();
        Ok(Some((state, thumb)))
    }

    pub fn remove(&self, slot: Slot) -> std::io::Result<()> {
        match std::fs::remove_file(self.path(slot, STATE_EXT)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        }
        let _ = std::fs::remove_file(self.path(slot, THUMB_EXT));
        Ok(())
    }

    /// Which slots hold something, in menu order. The ring's leftovers are folded in first
    /// (see `adopt_ring`), so a card saved by an older build lists its saves on the first
    /// look rather than after a migration the player has to know to run.
    pub fn list(&self) -> std::io::Result<Vec<SlotEntry>> {
        self.adopt_ring()?;
        let dir = match std::fs::read_dir(&self.dir) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        for entry in dir {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some(STATE_EXT) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(slot) = Slot::parse(stem) else {
                continue;
            };
            out.push(SlotEntry {
                thumb: self.path(slot, THUMB_EXT),
                secs: mtime_secs(&path),
                state: path,
                slot,
            });
        }
        out.sort_by_key(|e| e.slot.index());
        Ok(out)
    }

    /// `true` when the slot holds a save. Cheaper than `read`, because a menu asking "is
    /// there a picture for this one" must not be loading the state to find out.
    pub fn filled(&self, slot: Slot) -> bool {
        self.path(slot, STATE_EXT).exists()
    }

    /// ★ The one pass that moves the ring's saves into the slots. Oldest save → `01`, newest
    /// → the highest number it reaches, capped at `99`.
    ///
    /// **Renames, and only within the same directory**, so a card yanked mid-pass has some
    /// saves under old names and some under new ones and nothing lost either way — the next
    /// call finishes the job. `Auto` is never a destination: the ring's saves are all things
    /// the player pressed a key for, and putting one of them in the slot the machine writes
    /// on its own would make an autosave look like a choice.
    ///
    /// Safe to call every time `list` runs: once there is nothing shaped like a stamp left,
    /// it walks the directory and renames nothing.
    fn adopt_ring(&self) -> std::io::Result<()> {
        let dir = match std::fs::read_dir(&self.dir) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        let mut stamps: Vec<String> = Vec::new();
        for entry in dir {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some(STATE_EXT) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            // The ring's own shape test, reused rather than re-described: `resume.state`
            // fails it, and so does anything the player dropped in.
            if crate::stamp::parse_stamp(stem).is_some() {
                stamps.push(stem.to_string());
            }
        }
        if stamps.is_empty() {
            return Ok(());
        }
        // Fixed width, most significant first: lexicographic is chronological, and oldest
        // first is what puts the oldest save in `01`.
        stamps.sort();
        for (i, stamp) in stamps.iter().enumerate() {
            let n = i + 1;
            if n > SLOT_MAX as usize {
                break;
            }
            let to = Slot::Num(n as u8);
            // A slot already holding a save keeps it: the ring's file is the older of the
            // two by definition, and a migration must not overwrite what a player has put
            // in a slot since.
            if self.filled(to) {
                let _ = std::fs::remove_file(self.dir.join(format!("{stamp}.{STATE_EXT}")));
                let _ = std::fs::remove_file(self.dir.join(format!("{stamp}.{THUMB_EXT}")));
                continue;
            }
            let _ = std::fs::rename(
                self.dir.join(format!("{stamp}.{STATE_EXT}")),
                self.path(to, STATE_EXT),
            );
            let _ = std::fs::rename(
                self.dir.join(format!("{stamp}.{THUMB_EXT}")),
                self.path(to, THUMB_EXT),
            );
        }
        Ok(())
    }

    fn path(&self, slot: Slot, ext: &str) -> PathBuf {
        self.dir.join(format!("{}.{ext}", slot.name()))
    }
}

/// Seconds since the epoch, off a file's mtime. `None` when the filesystem will not say,
/// which on this machine means an SD card that lost it — the save is still loadable, so the
/// row must not read as empty.
fn mtime_secs(path: &Path) -> Option<i64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        assert_eq!(Slot::Auto.name(), "auto");
        assert_eq!(Slot::Num(7).name(), "07");
        assert_eq!(Slot::parse("auto"), Some(Slot::Auto));
        assert_eq!(Slot::parse("07"), Some(Slot::Num(7)));
        assert_eq!(Slot::parse("7"), None);
        assert_eq!(Slot::parse("00"), None);
        assert_eq!(Slot::parse("100"), None);
        assert_eq!(Slot::parse("resume"), None);
    }

    #[test]
    fn index_wraps_the_way_a_menu_walks_it() {
        assert_eq!(Slot::at(0), Slot::Auto);
        assert_eq!(Slot::at(1), Slot::Num(1));
        assert_eq!(Slot::at(99), Slot::Num(99));
        assert_eq!(Slot::at(100), Slot::Auto);
        assert_eq!(Slot::Num(12).index(), 12);
    }

    #[test]
    fn a_slot_is_written_read_and_emptied() {
        let dir = std::env::temp_dir().join(format!("slot-slots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let slots = StateSlots { dir: dir.clone() };
        assert!(!slots.filled(Slot::Num(3)));
        assert!(slots.read(Slot::Num(3)).unwrap().is_none());
        slots
            .write(Slot::Num(3), b"state", b"png")
            .expect("write");
        assert!(slots.filled(Slot::Num(3)));
        assert_eq!(slots.read(Slot::Num(3)).unwrap().unwrap().0, b"state");
        slots.remove(Slot::Num(3)).expect("remove");
        assert!(!slots.filled(Slot::Num(3)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The ring's ten saves become `01` to `10`, oldest first, and `Auto` is left alone.
    #[test]
    fn the_ring_is_adopted_oldest_first() {
        let dir = std::env::temp_dir().join(format!("slot-adopt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Three ring saves, written oldest first (the stamp sorts chronologically).
        for stamp in [
            "2026-10-01_10-00-00",
            "2026-10-02_10-00-00",
            "2026-10-03_10-00-00",
        ] {
            std::fs::write(dir.join(format!("{stamp}.state")), b"s").unwrap();
            std::fs::write(dir.join(format!("{stamp}.png")), b"p").unwrap();
        }
        // And one the machine wrote on its own, which is nobody's to move.
        std::fs::write(dir.join("resume.state"), b"r").unwrap();

        let slots = StateSlots { dir: dir.clone() };
        let listed = slots.list().expect("list");
        let got: Vec<String> = listed.iter().map(|e| e.slot.name()).collect();
        assert_eq!(got, vec!["01", "02", "03"]);
        assert!(!slots.filled(Slot::Auto));
        // The newest of the three landed in `03`, so `03` holds the newest one's bytes.
        assert!(dir.join("03.state").exists());
        assert!(dir.join("resume.state").exists());
        // A second call renames nothing and does not move the goalposts.
        assert_eq!(
            slots.list().unwrap().iter().map(|e| e.slot.name()).collect::<Vec<_>>(),
            got
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
