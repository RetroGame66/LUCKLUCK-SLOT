//! 打印货架行每一张卡的几何，用来看 `slot_rects` 到底把卡放在哪。
//! 
//! 2026-10-23 23:5x 定下的规矩是「主卡居中、横版侧卡露一半、竖版侧卡站在中点」，
//! 但效果图上量出来的标签宽度和规则对不上（主卡 305 而不是 360）。与其从 PNG 反推，
//! 不如直接问代码 —— 这个 example 就是那个提问。
//! 
//! cargo run --example shelf_rects -- gba
//! cargo run --example shelf_rects -- gb

use slot_store::{Cart, System};

fn main() {
    let system = match std::env::args().nth(1).as_deref() {
        Some("gb") => System::Gb,
        Some("gbc") => System::Gbc,
        _ => System::Gba,
    };
    let titles = [
        "口袋妖怪 绿宝石",
        "黄金太阳",
        "恶魔城 月之轮回",
        "火焰纹章",
        "银河战士 融合",
    ];
    let carts: Vec<Cart> = titles
        .iter()
        .map(|t| Cart {
            stem: t.to_string(),
            rom: std::path::PathBuf::from(format!("Games/X/{t}.gba")),
            label: None,
            art: None,
            title: t.to_string(),
            code: String::new(),
            cgb: 0,
            initial: t.chars().next().unwrap_or('#'),
        })
        .collect();

    let mut shelf = slot_ui::Shelf::new(carts);
    shelf.set_system(system);
    shelf.index = 2;
    // `new` 把 scroll 留在 0；`index` 只是目标。要「停稳」的画面得先让弹簧落位，
    // 这正是 render_look 的 harness 可能漏掉的一步。
    println!("index={} scroll={}", shelf.index, shelf.scroll);

    let size = shelf.size();
    println!(
        "size: w={} h={} pitch={} middle={} side={} drop={} foot_y={}",
        size.w,
        size.h,
        size.pitch,
        size.middle,
        size.side,
        size.drop,
        size.foot_y()
    );

    // 绘制列表本身 —— `slot_rects` 是布局，`draw_row` 才是落笔。两者若有分歧，
    // 光看布局数字会得出「代码是对的」，而图上量出来的却是另一回事。
    let mut list = Vec::new();
    shelf.draw_row(None, 0.0, 0.0, 1.0, &mut list);
    for d in &list {
        match d {
            slot_gfx::Draw::Tex { x, y, w, h, alpha, .. } => println!(
                "  Tex  x={x:8.2} y={y:8.2} w={w:7.2} h={h:7.2} alpha={alpha:.2}"
            ),
            slot_gfx::Draw::Rect { x, y, w, h, .. } => println!(
                "  Rect x={x:8.2} y={y:8.2} w={w:7.2} h={h:7.2}"
            ),
            _ => {}
        }
    }

    for r in shelf.slot_rects(None, 0.0, 0.0) {
        println!(
            "cart {:>2}: x={:8.2} y={:8.2} w={:7.2} h={:7.2} centre={:7.2} selected={} alpha={:.2}",
            r.cart,
            r.x,
            r.y,
            r.w,
            r.h,
            r.x + r.w / 2.0,
            r.selected,
            r.alpha
        );
    }
}