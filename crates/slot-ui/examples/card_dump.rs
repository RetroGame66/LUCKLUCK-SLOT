//! Dump the shortcut card as one tall PNG, so the whole thing can be read on a desk instead of
//! paged on a 480-pixel screen. Same faces the device rasterises — `shortcut_row_face` — so what
//! this writes is what the card is, just unwound.
//!
//!     CARD_FONT=…/Plix-Bold.otf CARD_OUT=…/card.png \
//!         cargo run -p slot-ui --example card_dump
//!
//! The background is a flat dark grey rather than the card's usual photograph: the ink is light in
//! the dark mode and every line carries its own halo, which is exactly what a dark flat buys — a
//! readable picture of type that was designed to sit over something.

use std::io::Write;

fn main() {
    if let Ok(p) = std::env::var("CARD_FONT") {
        match std::fs::read(&p) {
            Ok(b) => {
                eprintln!("font: {p} ({} bytes)", b.len());
                slot_ui::text::set_font(b);
            }
            Err(e) => eprintln!("font: {p} unreadable: {e}"),
        }
    }
    let out = std::env::var("CARD_OUT").unwrap_or_else(|_| "card.png".into());

    // Every face first, because the canvas is one allocation and the rows' heights are the
    // faces' own — `ROW_H`/`HEAD_H` are the card's private business.
    let faces: Vec<_> = slot_ui::SHORTCUT_ROWS
        .iter()
        .map(|r| slot_ui::shortcut_row_face(*r))
        .collect();
    let pad = 26u32;
    let total: u32 = faces.iter().map(|f| f.h).sum::<u32>() + 2 * pad;
    let w = slot_ui::OUT_W;
    let mut canvas = vec![0u8; (w * total * 4) as usize];
    for px in canvas.chunks_mut(4) {
        px.copy_from_slice(&[0x1b, 0x1b, 0x20, 0xff]);
    }

    let row_x = (w - slot_ui::SHORTCUT_ROW_W) / 2;
    let mut y = pad;
    for f in &faces {
        for j in 0..f.h {
            for i in 0..f.w {
                let s = ((j * f.w + i) * 4) as usize;
                let a = f.rgba[s + 3] as u32;
                if a == 0 {
                    continue;
                }
                let d = (((y + j) * w + row_x + i) * 4) as usize;
                for c in 0..3 {
                    let src = f.rgba[s + c] as u32;
                    let dst = canvas[d + c] as u32;
                    canvas[d + c] = ((src * a + dst * (255 - a)) / 255) as u8;
                }
            }
        }
        y += f.h;
    }

    let file = std::fs::File::create(&out).expect("create");
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, total);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let mut wr = e.write_header().expect("header");
    wr.write_image_data(&canvas).expect("data");
    wr.finish().expect("finish");
    let mut b = std::io::BufWriter::new(std::io::stdout());
    writeln!(b, "{out} {w}x{total} rows={}", faces.len()).ok();
}
