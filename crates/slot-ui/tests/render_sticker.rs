//! Writes the label out, and a magnified crop for judging it up close.
//!
//! Does nothing unless `SCRATCH_PNG` names an output file. `SCRATCH_CROP` and `SCRATCH_BOX`
//! ask for the magnified crop:
//!
//! `SCRATCH_PNG=/tmp/sticker.png cargo test -p slot-ui --test render_sticker -- --nocapture`
//!
//! ★ 2026-10-29（六六）：牌子上的字全删了，这张面就是**图本身** —— 所以这个预览现在等于
//! 把 `assets/sticker-66mod.png` 原样写出来。留着它仍然有用：换图之后不必上机，
//! 先看一眼尺寸与透明边对不对。图不是本面板的尺寸时 `sticker_face()` 返回 `None`，
//! 这里照样写一张**透明的**同尺寸图，把"图没到"这件事画出来，而不是让测试直接失败。

use slot_ui::{sticker_face, STICKER_H, STICKER_W};

fn write(path: &str, px: &[u8], w: usize, h: usize) {
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(px).unwrap();
}

#[test]
fn render_sticker() {
    let Ok(out) = std::env::var("SCRATCH_PNG") else {
        return;
    };
    let (w, h) = (STICKER_W as usize, STICKER_H as usize);
    let face = sticker_face();
    let px = match &face {
        Some(f) => f.rgba.clone(),
        None => {
            eprintln!(
                "no artwork at {w}x{h} in the tree: writing an empty label so the size is still \
                 visible"
            );
            vec![0; w * h * 4]
        }
    };
    write(&out, &px, w, h);

    // A crop, scaled up, so one area can be judged without squinting.
    if let Ok(crop) = std::env::var("SCRATCH_CROP") {
        let n: Vec<usize> = std::env::var("SCRATCH_BOX")
            .unwrap_or_else(|_| "0,0,200,60".into())
            .split(',')
            .map(|v| v.parse().unwrap())
            .collect();
        let (cx, cy, cw, ch) = (n[0], n[1], n[2], n[3]);
        let z = 5usize;
        let mut zoom = vec![0u8; cw * z * ch * z * 4];
        for y in 0..ch * z {
            for x in 0..cw * z {
                let (sx, sy) = (cx + x / z, cy + y / z);
                if sx < w && sy < h {
                    let s = (sy * w + sx) * 4;
                    let d = (y * cw * z + x) * 4;
                    zoom[d..d + 4].copy_from_slice(&px[s..s + 4]);
                }
            }
        }
        write(&crop, &zoom, cw * z, ch * z);
        println!("wrote {crop}");
    }
    println!("wrote {out}");
}