//! Bringing the private link network up and down, by shelling out to the card's
//! own `System/slotlink.sh`.
//!
//! Not on `Platform` despite that being the seam for talking to the machine: `App` owns its
//! `Power` outright, and a link session needs this from a worker thread. See the plan.

#[cfg(any(feature = "device", test))]
use std::path::{Path, PathBuf};

/// Which end of a link session this device is. The host brings the access point up and
/// waits; the joiner associates to it and connects out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRole {
    Host,
    Join,
}

impl LinkRole {
    /// The subcommand `slotlink.sh link` expects.
    pub fn arg(self) -> &'static str {
        match self {
            LinkRole::Host => "host",
            LinkRole::Join => "join",
        }
    }
}

/// The card's bring-up script, under `root`. It lives on the card rather than in the dock
/// on purpose: AGS-102 ships `ags-net` and upstream BaseOS ships nothing, so the script
/// is the one form of this that runs on both without a fork of this file.
#[cfg(any(feature = "device", test))]
fn script_path(root: &Path) -> PathBuf {
    root.join("System/slotlink.sh")
}

/// Blocking, and slow enough to matter — one to five seconds in the measurements, up to
/// the script's `AGS_LINK_WAIT_S` before it gives up. The caller owns a thread the UI is
/// not waiting on.
#[cfg(feature = "device")]
pub fn up(role: LinkRole) -> std::io::Result<()> {
    let status = link(role.arg()).status()?;
    if status.success() {
        return Ok(());
    }
    Err(std::io::Error::other(format!(
        "slotlink.sh link {} failed: {status}",
        role.arg()
    )))
}

/// Best effort and infallible on purpose: this runs on the failure path of every step above
/// it, and a teardown that can fail is a teardown callers skip.
#[cfg(feature = "device")]
pub fn down() {
    let _ = link("down").status();
}

/// Run through `/bin/sh` so the exec bit on the card does not matter, the same way the
/// dock-agnostic network script is run. `SLOT_ROOT` is the escape hatch for a card that is
/// not mounted where both docks mount it; `/mnt/sdcard` is where both do.
#[cfg(feature = "device")]
fn link(verb: &str) -> std::process::Command {
    let root = std::env::var_os("SLOT_ROOT").unwrap_or_else(|| "/mnt/sdcard".into());
    let mut cmd = std::process::Command::new("/bin/sh");
    cmd.arg(script_path(Path::new(&root)))
        .arg("link")
        .arg(verb);
    cmd
}

/// No radio to bring up anywhere but the device, and nothing to shell out to. A host build
/// running two copies of slot over loopback is a real way to drive the screen, so this
/// succeeds rather than refusing.
#[cfg(not(feature = "device"))]
pub fn up(_role: LinkRole) -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(feature = "device"))]
pub fn down() {}

#[cfg(test)]
mod tests {
    use super::*;

    /// A device told to join that hosts instead brings up an access point nobody joins, and
    /// the failure it produces is "nobody arrived" — which reads as the other player's fault.
    #[test]
    fn each_role_asks_for_its_own_subcommand() {
        assert_eq!(LinkRole::Host.arg(), "host");
        assert_eq!(LinkRole::Join.arg(), "join");
    }

    /// The script's home is the one thing that must not drift: it is on the card, under
    /// `System/`, next to the binary that shells out to it. A card missing it fails with
    /// "would not start" rather than silently bringing up nothing.
    #[test]
    fn the_script_lives_on_the_card_under_system() {
        assert_eq!(
            script_path(Path::new("/mnt/sdcard")),
            Path::new("/mnt/sdcard/System/slotlink.sh")
        );
    }
}
