//! Print the card slot's own profile, straight out of `desktop_rgba()`.
//!
//! The Python model in `_tools/slot_profile.py` is a re-derivation, and a re-derivation that
//! drifts from the Rust is worse than no model at all — this prints the buffer the machine
//! actually gets, one row at a time, so the test's anchors can be read off instead of guessed.
//!
//! `cargo run --example slot_profile` on the host; `SLOT_PROFILE_ROW=<y>` narrows it to one row.
//!
//! `.` = clear (alpha 0), `#` = fully opaque (255), a digit `1..9` = a **partial** alpha
//! (`a * 9 / 255`), which is exactly how the arcs get smoothed — a row of `#` straight into `.`
//! means the edge is stair-stepping again.

use slot_ui::desktop_rgba;

fn alpha(buf: &[u8], x: usize, y: usize) -> u8 {
    buf[(y * 720 + x) * 4 + 3]
}

fn main() {
    let buf = desktop_rgba();
    // 把真缓冲整块落盘（`SLOT_PROFILE_DUMP=<path>`），给 `_tools/desktop_dump_png.py` 出图用 ——
    // 出图必须是**引擎自己的像素**，不能是 Python 复刻的模型。
    if let Ok(path) = std::env::var("SLOT_PROFILE_DUMP") {
        std::fs::write(&path, &buf).expect("dump the desktop buffer");
        println!("dumped {} bytes to {path}", buf.len());
    }
    println!("   y  left-wall float |  x 160..205  (. = clear, # = opaque, 1..9 = partial)");
    for y in 424..480 {
        // The leftmost clear pixel in the row: that is the hole's left wall on this row.
        let first = (160..206).find(|&x| alpha(&buf, x, y) == 0);
        let art: String = (160..206)
            .map(|x| match alpha(&buf, x, y) {
                0 => '.',
                255 => '#',
                a => char::from(b'0' + ((a as u16 * 9 / 255).clamp(1, 9) as u8)),
            })
            .collect();
        // 右壁：本行最右那个透明像素。
        let last = (160..640).filter(|&x| alpha(&buf, x, y) == 0).next_back();
        let n = first.map_or(0, |x| x as i64 - 180);
        let m = last.map_or(0, |x| x as i64 - 540);
        println!("{y:4}  L={first:?}({n:+}) R={last:?}({m:+}) |{art}|");
    }
}