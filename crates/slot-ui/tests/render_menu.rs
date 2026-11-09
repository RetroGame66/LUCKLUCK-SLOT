//! Composes the menu screen exactly as `Phase::Menu` does — backdrop, label, the slots, the
//! foot line — and rasterises it to a PNG, so the layout can be looked at rather than asserted
//! about.
//!
//! Does nothing unless `SCRATCH_PNG` names an output file. `SCRATCH_WALL` puts a wallpaper
//! behind the label:
//!
//! `SCRATCH_PNG=/tmp/menu.png cargo test -p slot-ui --test render_menu -- --nocapture`
//!
//! The faces are the real ones — `menu::button` is the same function the binary bakes from — so
//! what comes out is what the device draws, at the device's own resolution, without a device.

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};
use slot_ui::menu;
use slot_ui::{
    draw_backdrop, draw_sticker, shortcut_hint_face, sticker_face, wallpaper_face,
};

/// ★ 2026-10-20：`draw_hint_bar` **在 `slot` 那个 crate 里，不在 `slot-ui`**（`app.rs` 的私有
///   函数），这个测试却从 `slot_ui` 引它，于是 `--all-targets` 一直编译不过。这里是它的同一份
///   实现（`app.rs` 的 `HINT_BAR_INK` / `HINT_GLOW_*` 五个常量一并抄来），为的是这张 PNG 跟真机
///   画的是同一条脚线 —— 而不是为了让测试自己能过就把那条底衬删掉。
const HINT_BAR_INK: [f32; 4] = [0.055, 0.05, 0.045, 1.0];
const HINT_GLOW_STEPS: u32 = 8;
const HINT_GLOW_H: f32 = 1.5;
const HINT_GLOW_A: f32 = 0.13;
const HINT_GLOW_INK: [f32; 3] = [0.86, 0.83, 0.77];

fn draw_hint_bar(top: f32, out: &mut Vec<slot_gfx::Draw>) {
    out.push(slot_gfx::Draw::Rect {
        x: 0.0,
        y: top,
        w: OUT_W as f32,
        h: (OUT_H as f32 - top).max(0.0),
        colour: HINT_BAR_INK,
    });
    for i in 0..HINT_GLOW_STEPS {
        let a = HINT_GLOW_A * (i as f32 + 1.0) / HINT_GLOW_STEPS as f32;
        let y = top - (HINT_GLOW_STEPS - i) as f32 * HINT_GLOW_H;
        out.push(slot_gfx::Draw::Rect {
            x: 0.0,
            y,
            w: OUT_W as f32,
            h: HINT_GLOW_H,
            colour: [HINT_GLOW_INK[0], HINT_GLOW_INK[1], HINT_GLOW_INK[2], a],
        });
    }
}

/// One texture the list can name, with the handle the list names it by.
struct Tex {
    id: TexId,
    rgba: Vec<u8>,
    w: u32,
    h: u32,
}

#[test]
fn render_menu() {
    let Ok(out) = std::env::var("SCRATCH_PNG") else {
        return;
    };
    let paper = std::env::var("SCRATCH_WALL")
        .ok()
        .and_then(|p| wallpaper_face(std::path::Path::new(&p)));

    // The same list `App::draw` builds, in the same order.
    let mut list = Vec::new();
    let mut texes: Vec<Tex> = Vec::new();
    let mint = |texes: &mut Vec<Tex>, w: u32, h: u32, rgba: Vec<u8>| {
        let id = TexId::from_raw(texes.len());
        texes.push(Tex { id, rgba, w, h });
        id
    };

    if let Some(p) = paper.as_ref() {
        let t = mint(&mut texes, OUT_W, OUT_H, p.to_vec());
        draw_backdrop(Some(t), None, &mut list);
    } else {
        draw_backdrop(None, None, &mut list);
    }

    // ★ 2026-10-29（六六）：牌子上的字全删了，这张面不再带任何参数。
    if let Some(label) = sticker_face() {
        let t = mint(&mut texes, label.w, label.h, label.rgba.clone());
        draw_sticker(Some(t), &mut list);
    }

    // The grid, in `GRID` order. The caret is on the first slot, which is where MENU opens it.
    let empty = menu::empty();
    let empty = mint(&mut texes, empty.w, empty.h, empty.rgba);
    let caret = menu::first_slot();
    for (i, slot) in menu::Item::GRID.iter().enumerate() {
        let (x, y) = menu::slot_at(i);
        let tex = match slot {
            Some(item) => {
                let b = menu::button(*item);
                let f = if i == caret { b.chosen } else { b.idle };
                mint(&mut texes, f.w, f.h, f.rgba)
            }
            None => empty,
        };
        // ★ 2026-10-21: the width is the slot's own, not `BUTTON_W` — the player's button is
        // a full row wide, and drawing it at a half-row width would show the panel's own
        // ground through the right half of a button that was baked 660 pixels wide.
        list.push(Draw::Tex {
            x,
            y,
            w: menu::slot_w(i),
            h: menu::BUTTON_H,
            tex,
            alpha: 1.0,
        });
    }

    // The pinned foot line, on the band the app draws behind it.
    let hint = shortcut_hint_face(menu::HINT);
    let top = OUT_H as f32 - hint.h as f32;
    draw_hint_bar(top, &mut list);
    let t = mint(&mut texes, hint.w, hint.h, hint.rgba);
    list.push(Draw::Tex {
        x: ((OUT_W - hint.w) / 2) as f32,
        y: top,
        w: hint.w as f32,
        h: hint.h as f32,
        tex: t,
        alpha: 1.0,
    });

    let (w, h) = (OUT_W as usize, OUT_H as usize);
    let mut px = vec![0u8; w * h * 4];
    for p in px.chunks_mut(4) {
        p.copy_from_slice(&[0, 0, 0, 255]);
    }
    let blend = |dst: &mut [u8], c: [f32; 3], a: f32| {
        for k in 0..3 {
            dst[k] = (c[k] * 255.0 * a + dst[k] as f32 * (1.0 - a)) as u8;
        }
    };

    for d in &list {
        match *d {
            Draw::Rect {
                x,
                y,
                w: rw,
                h: rh,
                colour,
            } => {
                for yy in y as usize..((y + rh) as usize).min(h) {
                    for xx in x as usize..((x + rw) as usize).min(w) {
                        let at = (yy * w + xx) * 4;
                        blend(
                            &mut px[at..at + 4],
                            [colour[0], colour[1], colour[2]],
                            colour[3],
                        );
                    }
                }
            }
            Draw::Tex {
                x,
                y,
                w: tw,
                h: th,
                tex,
                alpha,
            } => {
                let Some(t) = texes.iter().find(|t| t.id == tex) else {
                    continue;
                };
                if t.rgba.is_empty() {
                    continue;
                }
                for row in 0..th as usize {
                    for col in 0..tw as usize {
                        let (sx, sy) = (
                            col * t.w as usize / tw.max(1.0) as usize,
                            row * t.h as usize / th.max(1.0) as usize,
                        );
                        let s = (sy * t.w as usize + sx) * 4;
                        let a = t.rgba[s + 3] as f32 / 255.0 * alpha;
                        let (dx, dy) = (x as usize + col, y as usize + row);
                        if a > 0.0 && dx < w && dy < h {
                            let at = (dy * w + dx) * 4;
                            let c = [
                                t.rgba[s] as f32 / 255.0,
                                t.rgba[s + 1] as f32 / 255.0,
                                t.rgba[s + 2] as f32 / 255.0,
                            ];
                            blend(&mut px[at..at + 4], c, a);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let f = std::fs::File::create(&out).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&px).unwrap();
    println!("wrote {out} ({} draws, {} textures)", list.len(), texes.len());
}
