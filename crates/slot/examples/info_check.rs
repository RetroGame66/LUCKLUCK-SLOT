//! 把信息页的**两页**按 1:1 渲出来看 —— 与播放面板**同一条路**：
//! `lcd::info_frame` / `lcd::info_keys_frame` 合成 1-bit 帧，`lcd::render` 用**面板那一套
//! 色板和格网**变成 720×480。
//!
//! 为什么必须另写一个（而不是拿真机抓帧）：这一屏曾经是"铺 `opening()` 底 + 逐行
//! `word_face`"，那是**货架/菜单那一套渲染**；现在它与面板共用零件，所以**能比的对象是
//! 面板自己** —— 同一个色板、同一个倍率、同一种字。而这些在宿主上全部是可计算的纯函数。
//!
//! 跑法（从工作区根）：
//!
//! ```text
//! export PATH="$(pwd)/toolchain:$PATH"
//! cargo run --release --manifest-path slot-main/Cargo.toml --example info_check -- <面板字体目录> <色板序号> <输出目录>
//! ```
//!
//! ⚠️ **面板字体必须自己装**（`Music/Fonts/` 那两张）。不装则 `text::cjk_font()` 退回内嵌
//!   `label.ttf`，它在 16px 的行高是 22 而不是 16 ⇒ 整块版式全变，量出来的东西没有意义。

use std::path::PathBuf;

fn write_png(path: &std::path::Path, rgba: &[u8], w: u32, h: u32) {
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
        .unwrap_or_else(|| PathBuf::from("slot-main/deploy/Music/Fonts"));
    let ramp_at: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let out_dir = args
        .get(3)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("_scratch/shots_info"));

    // ── 装面板字体。⚠️ 不装就静默退回内嵌 `label.ttf`，版式全变（行高 22 而不是 16）。
    let mut installed = Vec::new();
    for name in ["unifont-16px.otf", "press-start-2p.ttf"] {
        match std::fs::read(font_dir.join(name)) {
            Ok(bytes) => {
                installed.push(format!("{} ({} B)", name, bytes.len()));
                if name.starts_with("chill") {
                    slot_ui::text::set_panel_fonts(Some(bytes), None);
                } else {
                    slot_ui::text::set_panel_fonts(None, Some(bytes));
                }
            }
            Err(e) => println!("字体缺失 {name}: {e}"),
        }
    }
    println!("面板字体: {installed:?}");
    // ⚠️ **不能用 `text::missing_in`** —— 它查的是**界面那张面**（`USER_FONT` +
    //   内嵌 `label.ttf`），而这一页用的是**面板那两张**（寒蝉 + PS2P）。
    //   拿 `missing_in` 问面板的字，答的是"Plix 有没有这个字"，答的不是这里要问的
    //   "面板这套 plan 画不画得出来"。**测量工具问错了对象，得到的答案就是错的**，
    //   而错法与"字体没装上"一模一样（整页没字）。
    //
    //   所以这里**真的去栅格化**：每个字单独走一遍 `plan_coverage`，数它有几格墨。
    //   **零格 = 画不出来**。
    for (tag, probe) in [
        ("prose", "像素随身听 曲目 12 突然好想你 翻页 ←→"),
        ("tag", "1/2L/R 翻页"),
    ] {
        let plan = if tag == "prose" {
            slot_ui::lcd::prose_plan()
        } else {
            slot_ui::lcd::tag_plan()
        };
        let blank: Vec<char> = probe
            .chars()
            .filter(|c| !c.is_whitespace())
            .filter(|c| {
                let l = slot_ui::lcd::layout(&c.to_string(), 999.0, 1, plan);
                let cov = slot_ui::text::plan_coverage(64, slot_ui::lcd::line_h_public(), &l, &plan);
                cov.iter().filter(|v| **v >= 128).count() == 0
            })
            .collect();
        println!("面板字体自检 [{tag}] {probe:?} 画不出来的字 = {blank:?}");
    }

    // ── 色板：**面板那 8 套里的第 `ramp_at` 套**，与屏上 Y 键轮换的是同一张表。
    let ramps = slot::palettes::deck_ramps();
    let (ramp_name, ramp) = ramps[ramp_at.min(ramps.len() - 1)];
    println!("色板 #{ramp_at} = {ramp_name}");

    // ── 几何自检：**这些数字是版式的全部约束**，打印出来是为了能在图之前先核对。
    println!(
        "几何: {}x{} 逻辑 → {}x{} 屏（×{}）｜line_h={}｜foot_y={}｜每页 {} 行详情",
        slot_ui::lcd::LOGICAL_W,
        slot_ui::lcd::LOGICAL_H,
        slot_ui::lcd::PANEL_W,
        slot_ui::lcd::PANEL_H,
        slot_ui::lcd::SCALE,
        slot_ui::lcd::line_h_public(),
        slot_ui::lcd::info_foot_y(),
        slot_ui::lcd::info_detail_rows(),
    );

    // ── 三种卡：满标签（挤）、无标签（松）、空卡（只有两行）。
    let keys = slot_ui::lang::DECK_KEY_GUIDE;
    println!("键位指南 {} 条 → 两栏每栏 {} 条", keys.len(), keys.len().div_ceil(2));
    let col_w = (slot_ui::lcd::content_x1() - slot_ui::lcd::content_x0()) / 2;
    let full_w = slot_ui::lcd::content_x1() - slot_ui::lcd::content_x0();
    println!(
        "间隔 = {} 逻辑像素；两栏 {}；整幅 {}",
        slot_ui::lcd::info_key_gap(),
        col_w,
        full_w
    );
    for (k, v) in keys {
        let kw = slot_ui::text::plan_line_width(k, &slot_ui::lcd::prose_plan());
        let vw = slot_ui::text::plan_line_width(v, &slot_ui::lcd::prose_plan());
        let w = kw + slot_ui::lcd::info_key_gap() as f32 + vw;
        println!(
            "  {k:>10} {v:<8} 键 {kw:>5.1} + 6 + 说明 {vw:>5.1} = {w:>5.1}（两栏 {col_w}／整幅 {full_w}）{}",
            if w > full_w as f32 {
                "  ★连整幅都放不下"
            } else if w > col_w as f32 {
                "  ★放不进两栏"
            } else {
                ""
            }
        );
    }
    // 底部那一行单独量：它是 tag_plan，而 `←` 在 PS2P 里有没有字形决定了它走哪张面。
    let foot = "1/2   L / R 翻页";
    println!(
        "页脚 {:?} = {:.1} 逻辑像素；plan_coverage 会不会丢字：{:?}",
        foot,
        slot_ui::text::plan_line_width(foot, &slot_ui::lcd::tag_plan()),
        slot_ui::text::missing_in(foot),
    );

    for (tag, mut info) in [
        ("full", sample("示例专辑", "12", "3/12  突然好想你", "五月天", "3:45")),
        ("plain", sample("示例专辑", "2", "1/2  第一首", "某人", "3:32")),
        // ★ **标签多到装不下**：这一档是"跨页流动"存在的理由 ——
        //   8 个标签项 + 曲名 + 歌手 + 时长 = 11 行，而一页只放 6 行。
        //   不流动的话后面 5 行**静默丢掉**，而屏上看不出少了东西。
        ("many", sample("示例专辑", "12", "3/12  突然好想你", "五月天", "3:45")),
        // 卡上没有音乐：只有抬头两行，详情全空 —— **与 `App::deck_info` 那条臂一样**。
        ("empty", sample(slot_ui::lang::MENU_DISCS, "0", "", "", "")),
    ] {
        if tag == "empty" {
            info.disc_line = slot_ui::lang::INFO_NO_DISC.to_string();
        }
        if tag == "many" {
            info.tags = [
                ("year", "1999"),
                ("genre", "Rock"),
                ("album", "示例专辑"),
                ("date", "1999-11-01"),
                ("track", "3"),
                ("disc", "1"),
                ("albumartist", "示例歌手"),
                ("isrc", "TPA199900001"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        }
        if tag == "full" {
            info.tags = vec![
                ("year".into(), "1998".into()),
                ("genre".into(), "Pop".into()),
                ("album".into(), "示例专辑".into()),
            ];
        }
        println!(
            "── [{tag}] 详情 {} 行 ⇒ {} 页（详情 + 键位）",
            info.rows(),
            info.pages()
        );
        for page in 0..info.pages() {
            let frame = info.frame(page);
            let rgba = slot_ui::lcd::render(&frame, ramp, true);
            let p = out_dir.join(format!("info-{tag}-p{page}.png"));
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            write_png(&p, &rgba, slot_ui::lcd::PANEL_W, slot_ui::lcd::PANEL_H);
            println!("  {tag} p{page} -> {}", p.display());
        }
    }
}

/// 一张样例碟。**手抄而不是造 `App`** —— 造 `App` 要一张卡 + 一次扫描，
/// 而这里要验的是"排得对不对"，手抄能把"数据对不对"和"排得不对"分开看。
fn sample(
    disc: &str,
    tracks: &str,
    track_line: &str,
    artist: &str,
    length: &str,
) -> slot::app::DeckInfo {
    slot::app::DeckInfo {
        disc: disc.to_string(),
        disc_line: format!("曲目 {tracks} · 48:20"),
        track_line: track_line.to_string(),
        artist: artist.to_string(),
        length_line: format!("时长 {length}"),
        tags: Vec::new(),
        keys: slot_ui::lang::DECK_KEY_GUIDE,
    }
}