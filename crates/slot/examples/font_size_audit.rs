//! 量一遍：**这块面板的字体在哪些字号下，阈值化之后还是"硬像素"**。
//!
//! 为什么要有这个：面板的字是 1bit 的（`>=128` 就是墨，否则是纸），所以**非设计尺寸的字号
//! 会先被抗锯齿、再被阈值切成毛边**——那看着像"字体本身笔触脏"，其实是字号传错了。
//! 一直以来只有一句口口相传的结论（"12 和 24 干净"），但**没人量过 6/8/10 到底行不行**，
//! 而"次要信息用小一号的字"这种需求恰恰卡在这里。
//!
//! 判据：把一行字渲进覆盖率缓冲，数**半调像素**（0 < 覆盖 < 255）占所有落墨像素的比例。
//! 0.0% ⇒ 每一笔都落在整数像素上，阈值化不掉任何信息；比例高 ⇒ 一渲染就是灰边。
//!
//! 参数可以是**一个目录**（取里面第一份 .otf/.ttf）或**一个字体文件**。顺带报缺字。
//!
//! 跑法（**从工作区根**）：
//!
//! ```text
//! cargo run --manifest-path slot-main/Cargo.toml --target-dir slot-main/target-host \
//!   -p slot --example font_size_audit -- 字体
//! cargo run ... --example font_size_audit -- _scratch/ark10/ark-pixel-10px-proportional-zh_hans.otf
//! ```

use std::path::PathBuf;

/// 取字体：文件直接用；目录取里面第一份 `.otf`/`.ttf`（按名字排序，所以给个明确的文件名最稳）。
fn load_font(arg: &str) -> Option<PathBuf> {
    let p = PathBuf::from(arg);
    let mut cands: Vec<PathBuf> = Vec::new();
    if p.is_file() {
        cands.push(p.clone());
    } else if let Ok(rd) = std::fs::read_dir(&p) {
        let mut v: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|q| {
                matches!(
                    q.extension().and_then(|s| s.to_str()),
                    Some("otf") | Some("ttf")
                )
            })
            .collect();
        v.sort();
        cands = v;
    }
    for c in cands {
        if let Ok(bytes) = std::fs::read(&c) {
            slot_ui::text::set_font(bytes);
            return Some(c);
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = args.get(1).cloned().unwrap_or_else(|| "字体".to_string());
    match load_font(&arg) {
        Some(p) => println!("font: {}\n", p.display()),
        None => println!("font: 内嵌 fallback（{arg} 里没有可用的 .otf/.ttf）\n"),
    }

    // 一行里既有汉字也有拉丁和数字，三种笔画的落地情况都能看到。
    const SAMPLE: &str = "正在播放 算 (Original) 0:37 / 4:00 TRACK 03/12";
    let missing = slot_ui::text::missing_in(SAMPLE);
    if missing.is_empty() {
        println!("缺字：无（样本里的字都能画）");
    } else {
        println!(
            "缺字：{} 个 —— {}",
            missing.len(),
            missing.iter().collect::<String>()
        );
    }
    println!();

    println!("{:>6}  {:>6}  {:>8}  {:>7}  判定", "px", "落墨", "半调", "半调占比");
    let Some(font) = slot_ui::text::label_font() else {
        println!("没有字体，无法量");
        return;
    };
    // 行高：版式里每一行占多少逻辑行都由它推出来，所以先报出来。
    // ★ **面板的 CJK 面是 16px，所以 16 这一档必须看** —— 换一张 CJK 面若 `line_h` 不是 16，
    //   整套纵向版式（`body_top` / `strip_y` / …）都会跟着动，那是"换字体"最容易被忽略的代价。
    for px in [10.0f32, 12.0, 16.0, 24.0] {
        if let Some(m) = font.horizontal_line_metrics(px) {
            println!(
                "line_h @{px:.0}px = {:.0}（new_line_size {:.0} - line_gap {:.0}）",
                m.new_line_size - m.line_gap,
                m.new_line_size,
                m.line_gap
            );
        }
    }
    println!();
    // ★ **8..=48 逐像素扫一遍，不是挑几个数。**
    //
    // 原先是给 12px 那套面手挑的（6/7/8/9/10/11/12/13/14/16/18/20/24/30/36），而那个表
    // **漏掉了 32 和 48** —— 对一张 16px 网格的字体（寒蝉），干净尺寸是 16/32/48，
    // 于是"32 干不干净"这个最关键的问题**根本没被问过**。而它同时也会骗人：
    // `px <= 11` 走的是内嵌的 10px 面（见 `text::face_for`），所以 6–11 那几行
    // **对每一张字体都是同一组数** —— 那是内嵌面在报自己的数，不是被测字体。
    // 全扫一遍，这两件事都看得见。
    for px in (8..=48).map(|p| p as f32) {
        let l = slot_ui::text::fit(font, SAMPLE, 4096.0, 1, px, px);
        let h = (px * 2.0).ceil() as u32;
        let w = (px * SAMPLE.chars().count() as f32).ceil() as u32 + 8;
        let cov = slot_ui::text::coverage(w, h, &l);
        let ink = cov.iter().filter(|v| **v > 0).count();
        let half = cov.iter().filter(|v| **v > 0 && **v < 255).count();
        let pct = if ink == 0 {
            0.0
        } else {
            half as f32 / ink as f32 * 100.0
        };
        let verdict = if ink == 0 {
            "没画出东西"
        } else if pct < 0.5 {
            "★ 硬像素"
        } else if pct < 8.0 {
            "略有灰边"
        } else {
            "毛边"
        };
        // 内嵌 10px 面在代答（`face_for`：px <= 11 走小号面）—— 标出来，免得把它的数
        // 当成被测字体的数。
        let who = if px <= 11.0 { "（内嵌 10px 面）" } else { "" };
        println!("{px:>6.1}  {ink:>6}  {half:>8}  {pct:>6.1}%  {verdict}{who}");
    }
    println!(
        "\n（比例 = 半调像素 / 落墨像素。阈值化时半调像素按 >=128 二选一，\n \
         所以这个比例就是「有多少笔画会糊掉」。）"
    );
}
