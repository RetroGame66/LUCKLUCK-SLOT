//! 把菜单按钮的**面**按 1:1 渲出来看 —— 排版（居中、行宽、质感）改一处就得看一处。
//!
//! 为什么另写一个：菜单那页的背景是**壁纸**，从 `/dev/fb0` 抓下来是一堆背景花纹盖着按钮，
//! 量"标签正不正"要在噪声里猜。而 `menu::face()` 是**纯函数**（给标签、给选中态，还一张 RGBA），
//! 所以这里直接调它，看到的就是显卡上那几十行代码画出来的原图，没有合成器、没有壁纸。
//!
//! 跑法（从工作区根）：
//!
//! ```text
//! export PATH="$(pwd)/toolchain:$PATH"
//! cargo run --release --manifest-path slot-main/Cargo.toml --example menu_check -- <字体目录> <输出目录>
//! ```

use std::path::{Path, PathBuf};

fn write_png(path: &Path, rgba: &[u8], w: u32, h: u32) {
    let mut png = Vec::new();
    {
        let mut raw = Vec::with_capacity((h * (1 + w * 4)) as usize);
        for y in 0..h {
            raw.push(0);
            raw.extend_from_slice(&rgba[(y * w * 4) as usize..((y + 1) * w * 4) as usize]);
        }
        let mut z = vec![0x78u8, 0x01];
        let mut i = 0;
        while i < raw.len() {
            let n = (raw.len() - i).min(65535);
            let last = if i + n >= raw.len() { 1u8 } else { 0 };
            z.push(last);
            z.extend_from_slice(&(n as u16).to_le_bytes());
            z.extend_from_slice(&(!(n as u16)).to_le_bytes());
            z.extend_from_slice(&raw[i..i + n]);
            i += n;
        }
        let (mut a, mut b) = (1u32, 0u32);
        for byte in &raw {
            a = (a + *byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        z.extend_from_slice(&adler32(&raw).to_be_bytes());

        let chunk = |ty: &[u8; 4], body: &[u8], out: &mut Vec<u8>| {
            out.extend_from_slice(&(body.len() as u32).to_be_bytes());
            out.extend_from_slice(ty);
            out.extend_from_slice(body);
            let mut c = Crc32::new();
            c.update(ty);
            c.update(body);
            out.extend_from_slice(&c.finish().to_be_bytes());
        };
        png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(b"IHDR", &ihdr, &mut png);
        chunk(b"IDAT", &z, &mut png);
        chunk(b"IEND", &[], &mut png);
    }
    let _ = std::fs::write(path, png);
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = (a + *byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

struct Crc32 {
    v: u32,
}

impl Crc32 {
    fn new() -> Self {
        Crc32 { v: 0xffff_ffff }
    }
    fn update(&mut self, data: &[u8]) {
        for byte in data {
            self.v ^= *byte as u32;
            for _ in 0..8 {
                let mask = (self.v & 1).wrapping_neg();
                self.v = (self.v >> 1) ^ (0xedb8_8320 & mask);
            }
        }
    }
    fn finish(self) -> u32 {
        self.v ^ 0xffff_ffff
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let font_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let out_dir = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("_scratch/shots_menu"));

    // ★ 2026-10-16（六六问"选中键与未选中键的文字色"）：**同一张面在十四档配色下不是一个颜色**。
    //   `palette::look()` 一非零，选中那档的字就走 `chosen_ink()` 的"挑对比度最高"，
    //   而设备自带那档两者恰好同色 —— 所以量色必须指明档位，否则量出来的是"没差别"。
    //   `SLOT_MODE=light|dark`（默认 dark）· `SLOT_LOOK=0..6`（默认 0 = 设备自带）。
    let light = std::env::var("SLOT_MODE").as_deref() == Ok("light");
    let look: usize = std::env::var("SLOT_LOOK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mode = if light {
        slot_ui::palette::Mode::Light
    } else {
        slot_ui::palette::Mode::Dark
    };
    slot_ui::palette::set_mode(mode);
    slot_ui::palette::set_look(look);
    let out_dir = out_dir.join(format!("{}-look{look}", if light { "light" } else { "dark" }));

    // ⚠️ **名字要跟卡上真在用的那张对上**（`System/fonts/` 里 `font_file` 排序取第一个 ttf/otf/ttc）。
    //   设备上那张是 **Plix-Bold.otf**；第一版这里只列了 Ark Pixel，于是喂 Plix 时静默没装字体，
    //   量出来是"无字"——**看起来像 Plix 不能用，其实是压根没装上**。
    for name in [
        "Plix-Bold.otf",
        "ark-pixel-12px-proportional-zh_hans.otf",
        "ArkPixel12px.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(font_dir.join(name)) {
            slot_ui::text::set_font(bytes);
            println!("font: {}", font_dir.join(name).display());
            break;
        }
    }

    // ★ 装上字体之后直接问它："这几个字你画得出来吗"（`missing_in` 走的就是绘制那条路的判据）。
    //   第一版这里只量像素、没问过这个问题，于是"宿主全是空、设备有字"这个矛盾一直找不到落点。
    for probe in ["像素随身听", "全键位一览", "PLAY 0:37"] {
        println!("字体自检 {probe:?} 画不出来的字 = {:?}", slot_ui::text::missing_in(probe));
    }
    println!(
        "菜单几何: MARGIN={} GUTTER={} BUTTON_W={} BUTTON_H={} TOP={} bottom={}  (屏 {}x{})",
        slot_ui::menu::MARGIN,
        slot_ui::menu::GUTTER,
        slot_ui::menu::BUTTON_W,
        slot_ui::menu::BUTTON_H,
        slot_ui::menu::TOP,
        slot_ui::menu::bottom(),
        slot_gfx::OUT_W,
        slot_gfx::OUT_H,
    );
    // 贴纸是 660 宽、`draw_sticker_at` 里居中 ⇒ x 30..690。格子的左右边要跟它对上。
    let left = slot_ui::menu::MARGIN;
    let right = slot_gfx::OUT_W as f32 - slot_ui::menu::MARGIN;
    println!("格子横向: {left}..{right}（贴纸是 30..690）");
    // ★★ 第二行曾经被底条吃掉 12px（`BUTTON_H` 70 → 64 之前）：底条**不透明且后画**，
    //   所以底边一进去就没了，看着像"面板被切"而不是"版式高了一点"。
    //   这一行就是那条判据的宿主复算 —— `<= 0` 才是对的。
    let grid_bottom = slot_ui::menu::bottom();
    let bar_top = slot_gfx::OUT_H as f32 - slot_ui::menu::HINT_BAR_H;
    println!(
        "格子脚={grid_bottom}  底条上沿={bar_top}  余量={}（>=0 才不算被底条切）",
        bar_top - grid_bottom
    );

    for (tag, item) in [
        ("shortcuts", slot_ui::menu::Item::Shortcuts),
        ("discs", slot_ui::menu::Item::Discs),
    ] {
        let b = slot_ui::menu::button(item);
        for (state, face) in [("idle", b.idle), ("chosen", b.chosen)] {
            let p = out_dir.join(format!("btn-{tag}-{state}.png"));
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            write_png(&p, &face.rgba, face.w, face.h);
            println!("{tag}/{state} {}x{} -> {}", face.w, face.h, p.display());
        }
    }
    // 空格子（凹槽），一并看一眼
    let e = slot_ui::menu::empty();
    let p = out_dir.join("btn-empty.png");
    let _ = std::fs::create_dir_all(&out_dir);
    write_png(&p, &e.rgba, e.w, e.h);
    println!("empty {}x{} -> {}", e.w, e.h, p.display());
}
