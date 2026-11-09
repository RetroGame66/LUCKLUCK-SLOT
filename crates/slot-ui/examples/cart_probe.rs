//! 卡带两个可疑点的取证 —— **纯诊断，不碰设备、不改视觉**。
//!
//!   ① 货架两侧的候选卡带为什么是半透明的？（有壁纸时最明显）
//!   ② 亮色模式下"特殊颜色卡带"为什么变成浅色卡带？
//!
//! 跑法（宿主，不需要设备）：
//!     _tools/cart_probe.sh
//! 或：
//!     cargo run --manifest-path slot-main/Cargo.toml --target-dir slot-main/target-hosttest \
//!         --target x86_64-pc-windows-gnu -p slot-ui --example cart_probe
//!
//! 每一个数都是从真函数算出来的（`cart_face` / `Art::face` / `themed_shell`
//! 的实际输出），不是抄来的常量。**场上的卡带素材**从 `deploy/System/Carts/` 读，和真机
//! 上卡里那份是同一批文件 —— 所以要先把素材装进进程（`install_cart_art`），否则量到的是
//! 「卡上没有素材」那条路，而玩家看到的是另一条。

use std::path::Path;

use slot_store::{Cart, System};
use slot_ui::{cart_face, cart_placeholder, size_for, SIDE_ALPHA};

/// 一张脸的"实心度"：把 alpha 当覆盖率累加，除以像素数。1.0 = 完全不透明。
fn coverage(rgba: &[u8]) -> f64 {
    let n = rgba.len() / 4;
    if n == 0 {
        return 0.0;
    }
    rgba.chunks_exact(4).map(|p| p[3] as f64 / 255.0).sum::<f64>() / n as f64
}

/// **壳色**：脸的最上面那一条横带，避开采不采到标签。
///
/// 为什么不是整张脸的平均色：一张脸的**大半面积是标签**（游戏自己的画），把标签平均进去
/// 量出来的既不是壳色也不是任何有意义的东西 —— 第一版的 Ruby 均色 `[137,119,67]` 就是
/// 这么来的，而那明显不是红色。壳色的正确取样是**标签面板之上**那一窄条，那一片一定是壳。
fn shell_band(rgba: &[u8], w: u32, h: u32) -> Option<[u8; 3]> {
    // 只取最上面 12% 的高、中间 60% 的宽（避开左右圆角）。
    let y1 = ((h as f32) * 0.12).max(1.0) as u32;
    let x0 = (w as f32 * 0.20) as u32;
    let x1 = (w as f32 * 0.80) as u32;
    let mut s = [0u64; 3];
    let mut n = 0u64;
    for y in 0..y1.min(h) {
        for x in x0..x1.min(w) {
            let i = ((y * w + x) * 4) as usize;
            if rgba[i + 3] < 250 {
                continue; // 透明或抗锯齿边，不算壳色
            }
            s[0] += rgba[i] as u64;
            s[1] += rgba[i + 1] as u64;
            s[2] += rgba[i + 2] as u64;
            n += 1;
        }
    }
    (n > 0).then(|| [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8])
}

fn dist(a: [u8; 3], b: [u8; 3]) -> f64 {
    let d: i32 = (0..3).map(|i| (a[i] as i32 - b[i] as i32).pow(2)).sum();
    (d as f64).sqrt()
}

fn cart(code: &str, title: &str) -> Cart {
    Cart {
        stem: title.to_string(),
        rom: std::path::PathBuf::from(format!("Games/GBA/{title}.gba")),
        label: None,
        art: None,
        title: title.to_string(),
        code: code.to_string(),
        cgb: 0,
        initial: title.chars().next().unwrap_or('#'),
    }
}

fn housing_bytes() -> [u8; 3] {
    let h = slot_ui::housing();
    [
        (h[0] * 255.0) as u8,
        (h[1] * 255.0) as u8,
        (h[2] * 255.0) as u8,
    ]
}

fn main() {
    // ── 先把卡上的素材装上，跑的就是玩家那条路 ──
    //
    // 不装的话 `art_for()` 恒为 `None`，量的全是内建壳那条路 —— 而玩家卡上有
    // `System/Carts/cart_gba.png`，走的是另一条。第一版就是这么量错的。
    let art_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "deploy".to_string());
    let root = Path::new(&art_dir);
    if root.join("System/Carts/cart_gba.png").is_file() {
        slot_ui::install_cart_art(slot_ui::load_cart_art(root));
        println!("已装载卡上素材（{}）\n", root.join("System/Carts").display());
    } else {
        println!(
            "⚠️ {} 下没有 System/Carts/cart_gba.png —— 这次量的**只有内建壳**那条路\n",
            root.display()
        );
    }
    println!("卡上素材: {}\n", slot_ui::cart_art_summary());

    // ── ① 侧位半透明 ────────────────────────────────────────────────────────────
    println!("================= ① 侧位为什么半透明 =================");
    println!("SIDE_ALPHA = {SIDE_ALPHA}   ← 两侧候选的不透明度（0.70 → 0.90 → 1.0，见下）");
    println!();
    println!("alpha = (1 + (SIDE_ALPHA - 1) * t) * (1 - recede)     [shelf.rs:949]");
    for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let a = 1.0 + (SIDE_ALPHA - 1.0) * t;
        let tag = if t == 0.0 { "  ← 正中（选中那张）" } else if t == 1.0 { "  ← 紧邻的两张" } else { "" };
        println!("    t={t:<5}  alpha={a:.3}{tag}");
    }
    println!();
    // ★ 2026-10-25（六六：「GBC 的阴影还没拿掉，还是兜底的那个代码画的 svg」）——
    //   这一段原来量的是"垫在侧位卡下面的那层黑"（`cart_shadow`）。那块黑影**已经整条删掉了**：
    //   它的形状只能从贴图自己的 alpha 推，而贴图几乎铺满画布（GBC 只有 2.6% 透明），
    //   推出来就是一块方角矩形，比卡带的圆角大一圈。
    // ★★ 2026-10-28（六六第二次：「换了壁纸才发现又犯了一个老问题：货架上的卡带是半透明的」）：
    //   删影子时那句"贴图不透明所以透出来的是它自己的暗面"只对了下半句 —— **合成**的结果取决于
    //   底下是什么：底是黑的时候 0.7 就是压暗，底是亮壁纸的时候 0.7 就只是"看得见壁纸"。
    //   所以把 0.7 抬到 0.90，漏出来的降到 10% 以内（再过一层纱，实际 2~4%）。
    // ★★★ 2026-11-05（六六第三次：「把两侧卡带改不透明」）：0.90 → 1.0。1.0 时绘制 alpha 不再
    //   透任何壁纸 —— 侧位若还读作半透明，就只剩贴图自身 alpha < 255 这一条路（如
    //   66SLOT_CART_GBC.png 有 21% 像素 alpha 在 234~253，叠乘 1.0 也不变），那是素材的事，
    //   不是这里的数能治的。
    println!("侧位卡 = 一张 alpha 被乘成 {SIDE_ALPHA} 的**贴图**，底下没有垫任何东西。");
    if SIDE_ALPHA < 1.0 {
        println!("  ⇒ 漏出来的壁纸 = 1 - {SIDE_ALPHA}；黑底时看起来是「压暗」，亮壁纸时才是「半透明」。");
    } else {
        println!("  ⇒ 绘制 alpha 已到 1.0，壁纸一点不透。侧位再显半透明就只能是贴图自身 alpha < 255。");
    }
    println!();

    // ── ② 亮色模式下特殊颜色卡带 ────────────────────────────────────────────────
    println!("================= ② 亮色下特殊色为什么没了 =================");
    println!("两条路都把 GBA 卡的壳色换成 housing：");
    println!("  内建路 `cart_face` → `themed_shell`：Light ⇒ colour = housing()");
    println!("  素材路 `Art::face` → Light 且 light=Some(flat) ⇒ 整块按 luma 重涂成 housing");
    println!();

    let specials = [
        ("AXV", "Pokemon Ruby", "红"),
        ("AXP", "Pokemon Sapphire", "蓝"),
        ("BPE", "Pokemon Emerald", "绿"),
        ("BPR", "Pokemon FireRed", "橙红"),
        ("BPG", "Pokemon LeafGreen", "黄绿"),
        ("ZZZZ", "Some Homebrew", "默认灰"),
    ];

    println!("【GBA 卡】按**壳色条带**取样（脸最上面那条，避开标签）");
    println!(
        "{:<6} {:<18} {:<8} {:>18} {:>18} {:>10}",
        "code", "title", "设计", "Dark 壳色", "Light 壳色", "Light→housing"
    );
    println!("{}", "-".repeat(86));

    let mut gba_rows = Vec::new();
    for (code, title, name) in specials {
        let c = cart(code, title);
        let (dark, light) = {
            slot_ui::palette::set_mode(slot_ui::palette::Mode::Dark);
            let fd = cart_face(&c);
            let d = shell_band(&fd.rgba, fd.w, fd.h);
            slot_ui::palette::set_mode(slot_ui::palette::Mode::Light);
            let fl = cart_face(&c);
            let l = shell_band(&fl.rgba, fl.w, fl.h);
            slot_ui::palette::set_mode(slot_ui::palette::Mode::Dark);
            (d, l)
        };
        let hb_light = {
            slot_ui::palette::set_mode(slot_ui::palette::Mode::Light);
            let h = housing_bytes();
            slot_ui::palette::set_mode(slot_ui::palette::Mode::Dark);
            h
        };
        let dl = light.map(|l| dist(l, hb_light));
        println!(
            "{:<6} {:<18} {:<8} {:>18} {:>18} {:>10}",
            code,
            title,
            name,
            format!("{:?}", dark.unwrap_or([0, 0, 0])),
            format!("{:?}", light.unwrap_or([0, 0, 0])),
            dl.map(|v| format!("{v:.0}")).unwrap_or("-".into())
        );
        gba_rows.push((code, name, dark.unwrap_or([0, 0, 0]), light.unwrap_or([0, 0, 0])));
    }
    println!();

    // GB / GBC 是对照组：`light = None` ⇒ 亮色下**保留原色**
    println!("【对照组：GB / GBC，light=None】");
    println!("{:<6} {:<18} {:>18} {:>18}", "sys", "title", "Dark 壳色", "Light 壳色");
    println!("{}", "-".repeat(64));
    for (sys, title) in [(System::Gb, "Pokemon Red"), (System::Gbc, "Pokemon Crystal")] {
        let ext = if sys == System::Gb { "gb" } else { "gbc" };
        let c = Cart {
            stem: title.to_string(),
            rom: std::path::PathBuf::from(format!("Games/{title}.{ext}")),
            label: None,
            art: None,
            title: title.to_string(),
            code: "BYTE".to_string(),
            cgb: 0,
            initial: 'P',
        };
        let (d, l) = {
            slot_ui::palette::set_mode(slot_ui::palette::Mode::Dark);
            let fd = cart_face(&c);
            let d = shell_band(&fd.rgba, fd.w, fd.h);
            slot_ui::palette::set_mode(slot_ui::palette::Mode::Light);
            let fl = cart_face(&c);
            let l = shell_band(&fl.rgba, fl.w, fl.h);
            slot_ui::palette::set_mode(slot_ui::palette::Mode::Dark);
            (d, l)
        };
        println!(
            "{:<6} {:<18} {:>18} {:>18}",
            format!("{sys:?}"),
            title,
            format!("{:?}", d.unwrap_or([0, 0, 0])),
            format!("{:?}", l.unwrap_or([0, 0, 0]))
        );
    }
    println!();
    let hb = {
        slot_ui::palette::set_mode(slot_ui::palette::Mode::Light);
        let h = housing_bytes();
        slot_ui::palette::set_mode(slot_ui::palette::Mode::Dark);
        h
    };
    println!(
        "LIGHT_CASE.housing() = {hb:?}   ← 亮色下 GBA 卡全被涂成它（GB/GBC 不涂）"
    );
    println!();
    println!("色距汇总（GBA 卡，越小 = 越被涂平）：");
    for (code, name, d, l) in &gba_rows {
        println!(
            "  {code:<6} {name:<8}  Dark 距 housing {:6.0}   →   Light 距 housing {:6.0}",
            dist(*d, hb),
            dist(*l, hb)
        );
    }
    println!();
    println!("⚠️ 关键：**壳色条带**在 Dark 下是各自的设计色（Ruby 红 / Sapphire 蓝 …），");
    println!("   在 Light 下全部收敛到 housing 附近 —— 这就是\"特殊颜色卡带变成浅色卡带\"。");
    println!("   GB/GBC 那张上写 `light = None`（cart.rs:579），所以它们**不受影响**。");
    println!();

    // ── 附：占位空壳 ──
    println!("================= 附：跳字母滑过的空壳 =================");
    for (mode, name) in [
        (slot_ui::palette::Mode::Dark, "Dark"),
        (slot_ui::palette::Mode::Light, "Light"),
    ] {
        slot_ui::palette::set_mode(mode);
        let p = cart_placeholder(size_for(System::Gba), System::Gba);
        println!(
            "  {name:<6} 壳色条带 {:?}   实心度 {:.3}",
            shell_band(&p.rgba, p.w, p.h).unwrap_or([0, 0, 0]),
            coverage(&p.rgba)
        );
    }
    slot_ui::palette::set_mode(slot_ui::palette::Mode::Dark);
    println!();

    println!("================= 结论 =================");
    println!("① 侧位半透明：alpha 是设计值 0.7（有意为之，还带随距衰减）。");
    println!("   真缺陷是**垫底的黑在不该省的时候被省了** —— 条件写的是");
    println!("   `art_for(system).is_none()`，而侧位的半透明与卡有没有自带素材**毫无关系**。");
    println!("   卡上有素材（发布包就有）⇒ 不画阴影 ⇒ 0.7 直接盖壁纸 ⇒ 半透明。");
    println!("② 亮色特殊色：两条路都主动把 GBA 壳色换成 housing；GB/GBC 由于 `light=None` 幸免。");
    println!("   这是刻意的设计（让卡带看起来属于同一台浅色设备），代价是丢掉了特殊色。");
}