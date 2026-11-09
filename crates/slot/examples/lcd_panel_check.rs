//! 把卡座那块**复古液晶**真的拼出来出 PNG —— 走的是库里的真函数。
//!
//! 为什么要有这个：这块面板的每一步（1bit 阈值、逻辑帧上遮罩、整数倍放大、四阶上色）
//! **单看代码都对，但每一步都能画错而不报错**。而"画错了"在这里长得特别像"画对了"：
//! 字号写成 16 而不是 12，阈值化之后是一团毛边，看着像字体本身笔触脏；
//! 遮罩加在放大之后而不是之前，是一层细网纹，看着像贴了张纹理。
//!
//! 所以这块面板和 CD 封面一样用**渲染出来看**的方式验，而不是靠读代码。
//!
//! 跑法（**从工作区根目录**，别在 `slot-main` 里跑）：
//!
//! ```text
//! export PATH="$(pwd)/toolchain:$PATH"
//! cargo run --release -p slot-ui --example lcd_panel_check -- <字体目录> <输出目录>
//! ```
//!
//! 字体目录要给 `System/fonts/` 里的 otf（Ark Pixel 12px），没有就用内嵌的 fallback。

use std::path::{Path, PathBuf};

/// 一个不引依赖的最小 PNG 写出器。**只写不缩放**，出的是面板的真实像素 ——
/// 预览管线会 PNG→JPEG 再 4:2:0 二次采样，宽 >1080 还会缩小，所以验像素级必须看原图。
fn write_png(path: &Path, rgba: &[u8], w: u32, h: u32) {
    let mut png = Vec::new();
    {
        let mut raw = Vec::with_capacity((h * (1 + w * 4)) as usize);
        for y in 0..h {
            raw.push(0); // filter: none
            raw.extend_from_slice(&rgba[(y * w * 4) as usize..((y + 1) * w * 4) as usize]);
        }
        // zlib stream with stored deflate blocks
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
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
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

/// 一段像样的音频 tap，**用真的正弦波而不是随机数**。
///
/// 频谱是这块面板唯一会"什么都不画"的地方：Goertzel 打在 80Hz–16kHz 的对数频段上，
/// 随机白噪会每个频段都差不多高（一条平顶），正弦波会只在它自己的那段竖起来 ——
/// **而这正是能一眼看出"频段算错了没有"的样子**。
fn tone_tap(hz: f32, secs: f32, rate: u32) -> Vec<i16> {
    let n = (rate as f32 * secs) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / rate as f32;
            (16_000.0 * (2.0 * std::f32::consts::PI * hz * t).sin()) as i16
        })
        .collect()
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
        .unwrap_or_else(|| PathBuf::from("_scratch/shots_mock"));

    // ★ 2026-10-06：播放面板的两张面**不再编进二进制**，改从 **卡根的 `Music/Fonts/`** 读
    //   （六六：字体就住在**内存卡根目录的 `Music`**，跟原本游戏的 Games 和 Labels、System 同级，和 `Music/Covers`、`Music/Lyrics` 同属播放器那一摊）。
    //   这里按"真卡先找、再找工作区里的 deploy 树"的顺序找 —— 与 `root::panel_fonts` 同一条路，
    //   只是宿主上多试两个相对位置。
    {
        let dirs = [
            font_dir.join("../../Music/Fonts"),        // 真卡：<card>/System/fonts → <card>/Music/Fonts
            PathBuf::from("slot-main/deploy/Music/Fonts"), // 从工作区根跑
            PathBuf::from("deploy/Music/Fonts"),       // 从 slot-main 跑
        ];
        let names = slot_ui::text::PANEL_FONT_FILES;
        for d in dirs {
            let (cjk, latin) = (
                std::fs::read(d.join(names[0])).ok(),
                std::fs::read(d.join(names[1])).ok(),
            );
            if cjk.is_some() || latin.is_some() {
                println!(
                    "panel fonts: {} (cjk {}, latin {})",
                    d.display(),
                    cjk.is_some(),
                    latin.is_some()
                );
                slot_ui::text::set_panel_fonts(cjk, latin);
                break;
            }
        }
    }
    // 卡上自己的字体优先，其次内嵌的 fallback —— **与开机那条路是同一条**。
    let mut installed = false;
    for name in ["ark-pixel-12px-proportional-zh_hans.otf", "ArkPixel12px.ttf"] {
        let p = font_dir.join(name);
        if let Ok(bytes) = std::fs::read(&p) {
            slot_ui::text::set_font(bytes);
            println!("font: {}", p.display());
            installed = true;
            break;
        }
    }
    if !installed {
        println!("font: 内嵌 fallback（无 {:?}/ArkPixel12px.ttf）", font_dir);
    }
    println!(
        "line_h = {} px   panel = {}x{} (逻辑 {}x{} ×{})",
        slot_ui::lcd::line_h_public(),
        slot_ui::lcd::PANEL_W,
        slot_ui::lcd::PANEL_H,
        slot_ui::lcd::LOGICAL_W,
        slot_ui::lcd::LOGICAL_H,
        slot_ui::lcd::SCALE
    );

    // 从 PixelShift 里挑几套差别大的：灰、绿、琥珀、红。**用的是 slot 里的真表**，
    // 不是抄几个数进来 —— 抄进来的数会和表漂移。
    let ramps = slot::palettes::pixelshift();
    // ★ 多换几套色板看：格网是「格子与纸各半」，所以**纸越亮格子越淡**、
    // 纸越暗格子越深 —— 不同色板下格网的可见度差别很大。
    let picks: [&str; 8] = [
        "PS01", "PS10", "PS17", "PS23", "PS33", "PS37", "PS44", "PS02",
    ];
    let lyric: Vec<String> = ["街的灯がにじむ頃", "君の声を探してる", "風が運ぶ遠い記憶"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let tap = tone_tap(440.0, 0.05, 48_000);

    // ★ 走带条上真正会变的只有两处：**传输态**（播放键的图标）与**模式**（模式键的图标）。
    //   三种传输态各配一个模式，一次把两种图标的变化都照到。
    let states: [(slot_ui::lcd::Transport, slot_ui::lcd::Mode, &str); 4] = [
        (slot_ui::lcd::Transport::Playing, slot_ui::lcd::Mode::Repeat, "playing"),
        (slot_ui::lcd::Transport::Paused, slot_ui::lcd::Mode::One, "paused"),
        (slot_ui::lcd::Transport::Stopped, slot_ui::lcd::Mode::Shuffle, "stopped"),
        (slot_ui::lcd::Transport::Playing, slot_ui::lcd::Mode::Once, "once"),
    ];
    fn content<'a>(
        face: slot_ui::lcd::Face,
        tr: slot_ui::lcd::Transport,
        mode: slot_ui::lcd::Mode,
        lines: &'a [String],
        tap: &'a [i16],
        scroll: i32,
    ) -> slot_ui::lcd::Content<'a> {
        slot_ui::lcd::Content {
            face,
            title: "算 (Original)",
            artist: "CrazySix",
            lines,
            tap,
            rate: 48_000,
            // 一次性的版面渲染：没有上一帧，所以不过弹道（`lcd::Content::levels`）。
            levels: None,
            elapsed: 37.0,
            length: 240.0,
            scroll,
            eq: slot_ui::eq::Eq::Null,
            bar: Some(slot_ui::lcd::Bar {
                transport: tr,
                mode,
                elapsed: 37.0,
                length: 240.0,
            }),
        }
    }

    for (tr, mode, tag) in states {
        for (face, face_tag) in [
            (slot_ui::lcd::Face::Name, "name"),
            (slot_ui::lcd::Face::Lyrics, "lyric"),
        ] {
            let c = content(face, tr, mode, &lyric, &tap, 0);
            let p = out_dir.join(format!("bar-{tag}-{face_tag}.png"));
            let Some((name, ramp)) = ramps.iter().find(|(n, _)| n.starts_with("PS10")) else {
                continue;
            };
            let rgba = slot_ui::lcd::panel(&c, *ramp, true);
            write_png(&p, &rgba, slot_ui::lcd::PANEL_W, slot_ui::lcd::PANEL_H);
            println!("{name} / {tag} / {face_tag} -> {}", p.display());
        }
    }
    // 六套色板各出一张（播放态），看格网与走带条在不同色板下的样子
    for key in picks {
        let Some((name, ramp)) = ramps.iter().find(|(n, _)| n.starts_with(key)) else {
            println!("{key}: 表里没有，跳过");
            continue;
        };
        let c = content(
            slot_ui::lcd::Face::Name,
            slot_ui::lcd::Transport::Playing,
            slot_ui::lcd::Mode::Repeat,
            &lyric,
            &tap,
            0,
        );
        let rgba = slot_ui::lcd::panel(&c, *ramp, true);
        let p = out_dir.join(format!("bar-{key}.png"));
        write_png(&p, &rgba, slot_ui::lcd::PANEL_W, slot_ui::lcd::PANEL_H);
        println!("{name} -> {}", p.display());
    }

    // ★ 超长标题 ⇒ 滚动。**滚动偏移是外面算好传进来的**（`marquee_scroll`），
    //   所以这里照它走三档，看的是"同一个名字滚起来是不是连贯"。
    let Some((_, ramp)) = ramps.iter().find(|(n, _)| n.starts_with("PS10")) else {
        return;
    };
    let long_title = "雨上がりの商店街で君を待っている (Original Mix)";
    let long_w = slot_ui::lcd::title_width_px(long_title);
    let col_w = slot_ui::lcd::title_col_w();
    println!("long title: {long_w}px 在 {col_w}px 栏里 ⇒ 需要滚动");
    // ★ 版式自检：把按墨迹居中用到的三个量打出来，肉眼核对偏没偏。
    {
        let prose = slot_ui::lcd::prose_plan();
        let tag = slot_ui::lcd::tag_plan();
        let name = slot_ui::lcd::title_text("CrazySix", "算 (Original)");
        println!("[版式] title_text = {name:?}");
        println!("[版式] 名字行墨迹跨度 = {:?}", slot_ui::text::plan_ink_span(&name, &prose));
        println!(
            "[版式] 元信息探针墨迹跨度 = {:?}",
            slot_ui::text::plan_ink_span("PLAY [FLAT]", &tag)
        );
        println!("[版式] 走带条 0:37 墨迹跨度 = {:?}", slot_ui::text::plan_ink_span("0:37", &tag));
        println!(
            "[版式] title_y={} headline_h={} body_top={} strip_y={} prog_y={} line_h={}",
            slot_ui::lcd::title_y(),
            slot_ui::lcd::headline_h(),
            slot_ui::lcd::body_top(),
            slot_ui::lcd::strip_y(),
            slot_ui::lcd::prog_y(),
            slot_ui::lcd::line_h_public()
        );
        println!(
            "[版式] meta_y = {}",
            slot_ui::lcd::meta_y(
                "CrazySix",
                "算 (Original)",
                slot_ui::lcd::Transport::Playing,
                "NULL"
            )
        );
    }
    for step in [0, 60, 140] {
        let mut c = content(
            slot_ui::lcd::Face::Name,
            slot_ui::lcd::Transport::Playing,
            slot_ui::lcd::Mode::Repeat,
            &lyric,
            &tap,
            step,
        );
        c.title = long_title;
        let rgba = slot_ui::lcd::panel(&c, *ramp, true);
        let p = out_dir.join(format!("bar-scroll-{step:03}.png"));
        write_png(&p, &rgba, slot_ui::lcd::PANEL_W, slot_ui::lcd::PANEL_H);
        println!("scroll {step} -> {}", p.display());
    }

    // ★ **四个播放模式各出一张** —— 第九轮起模式在走带区正中显示成 `[英文词]`。
    println!();
    println!("四个播放模式的符号:");
    for (i, mode) in slot_ui::lcd::Mode::ALL.iter().enumerate() {
        let c = content(
            slot_ui::lcd::Face::Name,
            slot_ui::lcd::Transport::Playing,
            *mode,
            &lyric,
            &tap,
            0,
        );
        let rgba = slot_ui::lcd::panel(&c, *ramp, true);
        let p = out_dir.join(format!("bar-mode-{i}.png"));
        write_png(&p, &rgba, slot_ui::lcd::PANEL_W, slot_ui::lcd::PANEL_H);
        println!("  {mode:?} -> {}", p.display());
    }

    // ★ **同一个逻辑帧、只换放大倍率** —— 回答"基数/倍率怎么配才更细腻"。
    //   720×480 的整数倍只有这几档：720 = 240×3 = 360×2 = 180×4 = 144×5。
    //   倍率小 ⇒ 每个逻辑像素占的屏幕方块小 ⇒ 颗粒细；但**要填满屏就得把逻辑帧做大**，
    //   而版式是按逻辑像素写的 ⇒ 换基数等于重排版式，不是改一个常数。
    //   这里出的是"同一版式、不同倍率"的 1:1 对照（所以 ×2 的那张只有 480×320）。
    let c = content(
        slot_ui::lcd::Face::Name,
        slot_ui::lcd::Transport::Playing,
        slot_ui::lcd::Mode::Repeat,
        &lyric,
        &tap,
        0,
    );
    println!();
    println!("倍率对照（同一逻辑帧 240×160，字号 12px 逻辑）:");
    for scale in [2u32, 3, 4] {
        let rgba = slot_ui::lcd::panel_scaled(&c, *ramp, true, scale);
        let (w, h) = (slot_ui::lcd::LOGICAL_W * scale, slot_ui::lcd::LOGICAL_H * scale);
        let p = out_dir.join(format!("bar-scale{scale}.png"));
        write_png(&p, &rgba, w, h);
        println!(
            "  ×{scale} → {w}×{h}（填满屏需逻辑帧 {}×{}）  格距 {scale}px，12px 汉字 = {} 屏幕像素  -> {}",
            slot_ui::lcd::PANEL_W / scale,
            slot_ui::lcd::PANEL_H / scale,
            scale * 12,
            p.display()
        );
    }

    // ★ **每一套色板各出一张，并把"这套色板还能不能装下这块面板"打出来。**
    //
    //   为什么要这张表：面板是**四阶**的（纸 / EDGE / 规则 / 墨），它靠这四档说话 ——
    //   分区框是规则、唱到的那行是墨、其余行退一档、格网踩在纸与 EDGE 之间。
    //   **色板里的重复档就是丢内容的档**：`PS02/PS13/PS14/PS35` 在核心里只有两个颜色，
    //   展成四阶是 `纸,纸,墨,墨` ⇒ EDGE = 纸、规则 = 墨，
    //   于是**格网看不见、框变成实心黑条、没唱到的那几行和唱到的一样深**。
    //   六六 2026-10-06：「特别某些双色色板会缺内容」—— 这条就是把它量出来。
    println!();
    println!("色板一览（档位数不足 4 的会丢内容）:");
    let ramps = slot::palettes::pixelshift();
    let mut distinct_hist = std::collections::BTreeMap::new();
    for (i, (name, ramp)) in ramps.iter().enumerate() {
        let c = content(
            slot_ui::lcd::Face::Lyrics,
            slot_ui::lcd::Transport::Playing,
            slot_ui::lcd::Mode::Repeat,
            &lyric,
            &tap,
            0,
        );
        let rgba = slot_ui::lcd::panel(&c, *ramp, true);
        let p = out_dir.join(format!("ramp-{i:02}.png"));
        write_png(&p, &rgba, slot_ui::lcd::PANEL_W, slot_ui::lcd::PANEL_H);

        let mut seen: Vec<[u8; 3]> = Vec::new();
        for s in ramp.iter() {
            if !seen.contains(s) {
                seen.push(*s);
            }
        }
        *distinct_hist.entry(seen.len()).or_insert(0usize) += 1;
        let flag = if seen.len() >= 4 {
            ""
        } else {
            "   ← 档位不足，丢内容"
        };
        println!(
            "  [{i:02}] {name:<28} 档位 {}  纸{:?} edge{:?} 规{:?} 墨{:?}{flag}",
            seen.len(),
            ramp[0],
            ramp[1],
            ramp[2],
            ramp[3]
        );
    }
    println!("  档位数分布: {distinct_hist:?}");

    // ★ **面板轮换走的就是全量**（`deck_ramps()` = `pixelshift()`）。打出来核对顺序。
    println!();
    println!("面板轮换的色板（全量，顺序即轮换顺序）:");
    for (i, (name, _)) in slot::palettes::deck_ramps().iter().enumerate() {
        println!("  {i}. {name}");
    }
}
