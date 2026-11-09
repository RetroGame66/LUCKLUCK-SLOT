//! A record of what the buttons actually send, written to the card.
//!
//! The code table in `evdev` was transcribed from one of these, and a board that disagrees
//! with it reads as a dead button rather than as an error. There is no console on the device
//! to print to, so tracing writes a file the card can be taken out to read.

use std::path::{Path, PathBuf};

use super::evdev::{
    abs_axis_name, code_to_btn, device_name, pick_devices, Ev, EV_ABS, EV_KEY, EV_SW, SW_LID,
};

/// Set to anything but `0` to write the trace. Off by default: it is a bring-up tool, and it
/// writes a line per button edge for as long as the device is on.
pub const TRACE_VAR: &str = "SLOT_TRACE_INPUT";

/// Beside the content folders rather than inside one, so it is the first thing seen when the
/// card is put back in a computer and nothing has to be fished out of `System`.
pub const TRACE_FILE: &str = "input-trace.log";

pub fn enabled() -> bool {
    std::env::var_os(TRACE_VAR).is_some_and(|v| v != "0")
}

/// Every event node, not only the ones opened. A button that produces nothing is either a
/// code the table does not know or a node that was never read, and the two need different
/// fixes, so the survey has to show the nodes that were passed over and why they looked
/// uninteresting.
pub fn survey(dev: &Path, sys: &Path) -> Vec<String> {
    // The trace wants everything: a stick wired as its own ABS-only node is "opened" here so
    // the player sees it was caught, not skipped and silent.
    let opened = pick_devices(dev, sys);
    let Ok(entries) = std::fs::read_dir(dev) else {
        return vec![format!("no {} to survey", dev.display())];
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
    nodes
        .iter()
        .map(|node| {
            let Some(file) = node.file_name() else {
                return format!("{} has no name", node.display());
            };
            let file = file.to_string_lossy();
            let state = if opened.contains(node) {
                "opened"
            } else {
                "skipped"
            };
            let cap = |which: &str| {
                std::fs::read_to_string(sys.join(&*file).join("device/capabilities").join(which))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| "-".to_string())
            };
            format!(
                "{file} {state} name={:?} key=[{}] sw=[{}] abs=[{}]",
                device_name(sys, node),
                cap("key"),
                cap("sw"),
                cap("abs")
            )
        })
        .collect()
}

/// One button edge. The code is printed in hex and decimal because the table is written in
/// hex and `evtest` and the kernel headers speak decimal.
pub fn event_line(node: &str, ms: u128, ev: Ev) -> String {
    let btn = match ev.kind {
        EV_KEY => code_to_btn(ev.code).map_or_else(|| "unmapped".to_string(), |b| format!("{b:?}")),
        EV_SW if ev.code == SW_LID => "Lid".to_string(),
        // An axis. Named by the axis code rather than by the direction, because which end of
        // it is pressed is the sign of the value on the same line — and a stick the table does
        // not know yet still has to show its number so a bring-up can see it.
        EV_ABS => abs_axis_name(ev.code).map(|n| n.to_string()).unwrap_or_else(|| format!("abs_{:#x}", ev.code)),
        _ => "ignored".to_string(),
    };
    format!(
        "{ms:>8}ms {node} type={:#04x} code={:#x}({}) value={} -> {btn}",
        ev.kind, ev.code, ev.code, ev.value
    )
}
