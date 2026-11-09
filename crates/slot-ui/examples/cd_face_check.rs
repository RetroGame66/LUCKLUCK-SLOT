//! 把 CD 封面那张脸**真的拼出来**，出成 PNG —— 走的是库里的真函数，不是模拟。
//!
//! 为什么要有这个：贴图路径的每一个约定（洋红标记、区外取 recess、遮罩填充、裁切粘贴、
//! luma 百分位着色）单看代码都对，但**拼错一处不会报错，只会画错** —— 一张没有封面的
//! sleeve 和「本来就没有封面」长得一模一样。上一版就栽在这上面：卡自己的 art 开机报了
//! `disc art: disc 340x203+label`，看着没问题，可整个 boot 从没画过它。
//!
//! 跑法（**从工作区根目录**，别在 `slot-main` 里跑 —— 见 `slot-main/rust-toolchain.toml`）：
//!
//! ```text
//! export PATH="$(pwd)/toolchain:$PATH"
//! cargo run --release --manifest-path slot-main/crates/slot-ui/examples/../../Cargo.toml \
//!     -p slot-ui --example cd_face_check
//! ```
//!
//! 需要两个素材（`--art` / `--cover`），缺了就只出内建 sleeve 并说明 ——
//! **内建那张也值得看**，因为它是卡上没有 art 时真正会画的东西。

use std::path::PathBuf;

fn arg(i: usize) -> Option<PathBuf> {
    std::env::args().nth(i).map(PathBuf::from)
}

/// 把 `CartFace` 写成 PNG。**只写不缩放**，出的是 `cd::SIZE` 的面尺寸 ——
/// 预览管线会 PNG→JPEG 再 4:2:0 二次采样，宽 >1080 还会缩小，所以看像素级的对错要开原图。
fn write_png(path: &std::path::Path, face: &slot_ui::CartFace) {
    let mut png = Vec::new();
    {
        // 最小 PNG 编码器：IHDR / IDAT(zlib store) / IEND。**不引依赖**，因为这只是
        // 一个开发期的自查程序，而 slot-ui 的 dev-dependencies 已经够多了。
        let w = face.w;
        let h = face.h;
        let mut raw = Vec::with_capacity((h * (1 + w * 4)) as usize);
        for y in 0..h {
            raw.push(0); // filter: none
            let row = &face.rgba[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
            raw.extend_from_slice(row);
        }
        // zlib stream with stored deflate blocks
        let mut z = vec![0x78, 0x01];
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

        let chunk = move |ty: &[u8; 4], body: &[u8], out: &mut Vec<u8>| {
            out.extend_from_slice(&(body.len() as u32).to_be_bytes());
            out.extend_from_slice(ty);
            out.extend_from_slice(body);
            let mut c = crc32(ty);
            c.update(body);
            out.extend_from_slice(&c.finish().to_be_bytes());
        };
        png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
        chunk(b"IHDR", &ihdr, &mut png);
        chunk(b"IDAT", &z, &mut png);
        chunk(b"IEND", &[], &mut png);
    }
    std::fs::write(path, png).expect("write png");
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = (a + *byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

struct Crc32(u32);
impl Crc32 {
    fn new() -> Self {
        Crc32(0xffff_ffff)
    }
    fn update(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= *b as u32;
            for _ in 0..8 {
                self.0 = if self.0 & 1 != 0 {
                    (self.0 >> 1) ^ 0xedb8_8320
                } else {
                    self.0 >> 1
                };
            }
        }
    }
    fn finish(self) -> u32 {
        self.0 ^ 0xffff_ffff
    }
}
fn crc32(ty: &[u8; 4]) -> Crc32 {
    let mut c = Crc32::new();
    c.update(ty);
    c
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cd_face_check.png"));

    let sz = slot_ui::cd::SIZE;
    println!(
        "cd::SIZE = {}x{} pitch={} middle={} side={} drop={}",
        sz.w, sz.h, sz.pitch, sz.middle, sz.side, sz.drop
    );
    println!(
        "face = {}x{}, label panel = {:?}",
        slot_ui::cd::SIZE.face_w(),
        slot_ui::cd::SIZE.face_h(),
        slot_ui::cd::SIZE.label_panel()
    );

    // ① 内建 sleeve：卡上没有 `System/Carts/cd.png` 时真正会画的那一张。
    let plain = slot_ui::cd::face(&[]);
    write_png(&out.with_file_name("cd_builtin.png"), &plain);
    println!(
        "built-in  -> cd_builtin.png  {}x{}",
        plain.w, plain.h
    );

    // ② 卡上有 art 的那一张。**装载只此一次**（`OnceLock`），所以这个 example 只能演一遍。
    //
    // `--art` 给的是**卡根**而不是 art 文件本身：`load` 自己去找 `System/Carts/cd.png`，
    // 这正是开机时 `App::boot` 走的那条路 —— 传文件路径进来会绕过它，等于验了另一条。
    let Some(root) = arg(2) else {
        println!("no card root given: stopping after the built-in sleeve");
        return;
    };
    let root = if root.is_dir() { root } else { root.clone() };
    match slot_ui::load_cd_art(&root) {
        None => println!(
            "!! {} 下没有可用的 cd art（要 System/Carts/cd.png）",
            root.display()
        ),
        Some(loaded) => {
            println!("cd art summary: {}", slot_ui::cd_art_summary_of(&loaded));
            slot_ui::install_cd_art(Some(loaded));
            let (cw, ch) = slot_ui::cd_cover_size();
            println!("cover area = {cw}x{ch}  (0x0 = 图上没有洋红标记)");
            let bytes = arg(3)
                .as_ref()
                .and_then(|p| slot_ui::cd_cover(p, cw, ch))
                .unwrap_or_default();
            println!("cover bytes = {}", bytes.len());
            let face = slot_ui::cd_face(&bytes, &slot_ui::cd::SHELL, None);
            write_png(&out.with_file_name("cd_from_art.png"), &face);
            println!("from art -> cd_from_art.png  {}x{}", face.w, face.h);
        }
    }
}
