//! 字体装载的**两段**成本：**从卡上读** vs **fontdue 解析**。
//!
//! 为什么要有这个：开机日志里 `font load` 只给一个总数，而"预渲染用过的字 / 后台回填"
//! 这类想法**只能省第三段（逐字形栅格化）**，省不了前两段 —— 而且第三段通常小得可以忽略。
//! 要判断一个想法值不值得做，得先知道总数里前两段各占多少。
//!
//! 跑法（**从工作区根**）：
//!
//! ```text
//! cargo run --manifest-path slot-main/Cargo.toml --target-dir slot-main/target-host \
//!   -p slot --example font_parse_cost -- <字体.ttf> [更多字体...]
//! ```

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: font_parse_cost <font.ttf> [more...]");
        std::process::exit(2);
    }
    println!(
        "{:<52} {:>10} {:>9} {:>10}",
        "file", "bytes", "read ms", "parse ms"
    );
    for p in paths {
        let t = std::time::Instant::now();
        let Ok(bytes) = std::fs::read(&p) else {
            println!("{p:<52} 读不了");
            continue;
        };
        let read = t.elapsed();
        let n = bytes.len();
        let t = std::time::Instant::now();
        // `set_font` 的内部就是 `Font::from_bytes`。`OnceLock::set` 从第二次起会失败，
        // 但**解析本身照跑** —— 而这正是要量的东西。
        slot_ui::text::set_font(bytes);
        let parse = t.elapsed();
        println!(
            "{p:<52} {n:>10} {:>9} {:>10}",
            read.as_millis(),
            parse.as_millis()
        );
    }
}
