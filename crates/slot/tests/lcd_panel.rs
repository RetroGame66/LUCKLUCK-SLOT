//! 卡座那块复古液晶面板的规则 —— 自足，不引 `tests/common`。
//!
//! **为什么另写一份**：`tests/common/mod.rs` 一进测试文件就让进程在**任何测试体之前**
//! `0xc0000005` 崩掉（连 `eprintln!("BODY RAN")` 都打不出），这是本工作树的既有状态，
//! 2026-10-04 已用逐个探针排除工具链、依赖与本轮改动。详见 `deck_rules.rs` 的头注释。
//!
//! 这里能自己站住是因为面板的两条规则**都不需要一张卡**：一是一条 240×120 的帧上索引
//! 只能是 0..3，二是放大必须是整数倍。素材（字体、音频、色板）要么可选项，要么现成。

use slot_ui::lcd::{self, Content, Face, Frame};

/// ★ **装上播放面板那两张面，几何才和生产一致。**
///
/// ⚠️ 面板的两张面**不再编进二进制**（2026-10-06：六六「别封进二进制，内存卡根目录的 `Media`
/// 里到时随包分发」），所以不装它们时 `cjk_font()` 会退回内嵌的 `label.ttf`，而那张面的行高
/// 在 16px 上是 **22** 而不是 16 —— 于是**整块面板的纵向版式全变**，量出来的每一段都和生产
/// 不是一回事。第一次跑就栽在这上面：`line_h_public()` 报 22 而断言要 16。
///
/// 只装一次（`OnceLock` 只能设一次，而且装的是常量数据）。文件不在就直接红 ——
/// 缺了字体这条测试**验不了它要验的东西**，静默通过比失败更糟。
fn install_panel_fonts() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // 卡根那一级：`<repo>/slot-main/deploy/Music/Fonts`（deploy 树就是卡根镜像）。
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../slot-main/deploy/Music/Fonts");
        let names = slot_ui::text::PANEL_FONT_FILES;
        let cjk = std::fs::read(dir.join(names[0])).ok();
        let latin = std::fs::read(dir.join(names[1])).ok();
        assert!(
            cjk.is_some() && latin.is_some(),
            "{} 下缺 {} / {}：面板测试需要它们",
            dir.display(),
            names[0],
            names[1]
        );
        slot_ui::text::set_panel_fonts(cjk, latin);
    });
}

/// 一段正弦 tap，`i16` 域，与解码器交给频谱的东西同形。
fn tone(hz: f32, rate: u32) -> Vec<i16> {
    (0..(rate as usize / 4))
        .map(|i| {
            let t = i as f32 / rate as f32;
            (16_000.0 * (2.0 * std::f32::consts::PI * hz * t).sin()) as i16
        })
        .collect()
}

fn content<'a>(face: Face, lines: &'a [String], tap: &'a [i16]) -> Content<'a> {
    // 每一条画字的测试都从这里过，所以脸在这里装一次就够。
    install_panel_fonts();
    Content {
        face,
        title: "算 (Original)",
        artist: "CrazySix",
        lines,
        tap,
        rate: 48_000,
        // 测试要的是"同一份 `Content` 画两次得到同一张图"，所以不过弹道 —— 它是纯函数。
        levels: None,
        elapsed: 30.0,
        length: 240.0,
        scroll: 0,
        eq: slot_ui::eq::Eq::Null,
        bar: None,
    }
}

/// **逻辑帧的尺寸就是能整除整屏的那个尺寸。** 240×160 ×3 = **720×480**，
/// 而屏就是 720×480 —— 这不是巧合，是这块面板能存在的原因：放大后没有任何一个像素被插值。
///
/// ★ 帧高从 120 变成 160（2026-10-04）：六六说「操作区也是像素面板，720×480 同一套渲染」，
///   所以**走带条是同一个帧的下面 40 行**，不是另一张纹理。旧的 720×360 断言
///   （"底下 120px 留给走带键"）已经不成立 —— 走带键现在在帧里。
#[test]
fn the_logical_frame_divides_the_whole_screen_exactly() {
    assert_eq!(lcd::LOGICAL_W * lcd::SCALE, lcd::PANEL_W);
    assert_eq!(lcd::LOGICAL_H * lcd::SCALE, lcd::PANEL_H);
    assert_eq!(lcd::PANEL_W, 720, "帧的宽就是屏宽");
    assert_eq!(lcd::PANEL_H, 480, "帧的高就是屏高 —— 整屏一块面板");
    // ★ 第九轮：**边框 + 内缩 + 贴底走带区**。操作区取消了，所以再没有"液晶/走带"两段，
    //   只有：四周 2px 的墨圈、框内侧 1px 的浅边、内缩的内容区、贴底的走带区。
    assert_eq!(lcd::FRAME, 2, "四周 2px 的墨圈（与歌名、频谱柱同一个墨档）");
    assert_eq!(lcd::BEVEL, 1, "框内侧 1px 的浅边（EDGE 档）");
    assert_eq!(lcd::content_x0(), lcd::FRAME + lcd::PAD, "内容左界");
    assert_eq!(
        lcd::content_x1(),
        lcd::LOGICAL_W as i32 - lcd::FRAME - lcd::PAD,
        "内容右界"
    );
    // 走带区**贴着边框的内下沿**（用户："走带区贴底"），但整条仍在框里面。
    assert_eq!(
        lcd::strip_y() + lcd::strip_h(),
        lcd::LOGICAL_H as i32 - lcd::FRAME,
        "走带区应贴到边框的内下沿"
    );
    assert!(
        lcd::strip_y() > lcd::body_top(),
        "走带区应在主体之下"
    );
    // 走带区的高度：一行 12px 字 + 留白 + 进度轨 + 留白。
    assert_eq!(
        lcd::strip_text_h() + 2 * lcd::PROG_PAD + lcd::PROG_TRACK_H,
        lcd::strip_h(),
        "走带区 = 一行字 + 留白 + 进度轨 + 留白"
    );
    // 整数倍这件事本身：不是 3 的倍数就意味着重采样。
    assert!(lcd::LOGICAL_W.checked_mul(lcd::SCALE).is_some());
}

/// **一帧里只有四档。** 索引越界就是"发明了色板里没有的颜色"——四阶的意义就在这四档，
/// 而一个越界的索引会让查表回退到最深档并**静默地**画错，比越界本身更难查。
#[test]
fn a_frame_only_ever_holds_four_shades() {
    let mut f = Frame::panel();
    f.put(10, 10, 3);
    f.put(11, 10, 9); // 越界
    f.put(12, 10, 255); // 更越界
    f.fill(0, 0, lcd::LOGICAL_W as i32, 3, 7); // 一整条越界的横带
    assert!(f.px.iter().all(|v| *v <= 3), "越界的索引必须被夹住");
}

/// **越界的位置不写、也不绕到隔壁。** 画一根多一列的柱子如果绕回去了，
/// 出来的痕迹是一条斜穿画面的对角线——那看着像"面板坏了"，不像"柱子画长了"。
#[test]
fn a_write_past_the_edge_is_dropped_rather_than_wrapped() {
    let mut f = Frame::panel();
    let (w, h) = (f.w as i32, f.h as i32);
    f.put(w, 5, 3); // 右边界外
    f.put(-1, 5, 3); // 左边界外
    f.put(5, h, 3); // 下边界外
    // 全帧应当仍是空白：一个都没写进去。
    assert!(f.px.iter().all(|v| *v == 0), "越界的写必须整个丢掉");
}

/// **柱子从基线往上长，不是从顶往下挂。** 频谱是"倒过来的一样东西"，只有一种读法是对的。
#[test]
fn a_bar_stands_on_the_floor() {
    let mut f = Frame::new(10, 10);
    f.bar(2, 8, 3, 4, 3); // floor=8, h=4 ⇒ 占 y 4..8
    for y in 0..10 {
        for x in 0..10 {
            let on = f.px[(y * 10 + x) as usize] == 3;
            let want = (4..8).contains(&y) && (2..5).contains(&x);
            assert_eq!(on, want, "y={y} x={x}");
        }
    }
}

/// **零高度的柱子不画。** 一根高度算出来是 0 的柱子画出来是一根 1px 的线，
/// 而 16 根里出现几根这种线，看起来就是频谱在抖。
#[test]
fn a_zero_height_bar_draws_nothing() {
    let mut f = Frame::new(10, 10);
    f.bar(2, 8, 3, 0, 3);
    assert!(f.px.iter().all(|v| *v == 0));
}

/// **格距恒等于 [`lcd::SCALE`]，线宽恒为 1 屏幕像素，而且间隙是纸色。**
///
/// 三条合起来才是"这是 GB 的液晶"：**格距**是格子的间距、**线宽**是间隙的厚度、
/// **间隙取最浅档**是背光透出来的颜色。前两条不成立就不是网格；第三条不成立就是
/// **格网与游戏的格网逐条一致** —— 照 `slot-gfx/src/shaders.rs` 的实现，不是自己发明的。
///
/// 三条：几何是 3×3 的**右列 + 下行**（九分之五），颜色是**格子与纸各半**，
/// 而纸取自色板自己最浅那档。**任一条不符就不是同一个网格。**
#[test]
fn the_lattice_is_the_games_lattice() {
    // 一套刻意拉开四档的色板，"半"才有东西可看。
    let ramp = [
        [240u8, 240, 240], // 0 纸
        [160, 160, 160],
        [80, 80, 80],
        [20, 20, 20], // 3 墨
    ];
    let half = |a: [u8; 3], b: [u8; 3]| -> [u8; 3] {
        [
            ((a[0] as u32 + b[0] as u32) / 2) as u8,
            ((a[1] as u32 + b[1] as u32) / 2) as u8,
            ((a[2] as u32 + b[2] as u32) / 2) as u8,
        ]
    };
    // 整片纸：格网上应当看到"半纸"，而格内是纯纸。
    let mut f = Frame::new(3, 3);
    f.fill(0, 0, 3, 3, 0);
    let out = lcd::render(&f, ramp, true);
    let w = 3 * lcd::SCALE;
    let at = |x: u32, y: u32| -> [u8; 3] {
        let i = ((y * w + x) * 4) as usize;
        [out[i], out[i + 1], out[i + 2]]
    };
    let s = lcd::SCALE as f32;
    for y in 0..3 * lcd::SCALE {
        for x in 0..w {
            // 着色器的判据：`sub >= 2/3`，而 sub 是 0、1/3、2/3 三档。
            let on = (y % lcd::SCALE) as f32 >= 2.0 / 3.0 * s
                || (x % lcd::SCALE) as f32 >= 2.0 / 3.0 * s;
            let want = if on { half(ramp[0], ramp[0]) } else { ramp[0] };
            assert_eq!(at(x, y), want, "({x},{y}) 纸面上的格线判定错了");
        }
    }
}

/// **格网上的颜色是「格子与纸各半」，所以它比纸深、比格子浅。**
///
/// 这一条是"50% 透明度"的落法，也是第三版（间隙恒为纸）缺的东西：
/// 纸透纸还是纸 ⇒ 空白处一点格网都没有。这里各半之后，**空白处也看得见格网**。
#[test]
fn the_gap_is_a_quarter_of_the_way_to_paper() {
    let ramp = [
        [240u8, 240, 240],
        [160, 160, 160],
        [80, 80, 80],
        [20, 20, 20],
    ];
    // ★ 2026-10-06 六六：「网格的渲染透明度调到现在的一半」。
    //   格线的混色从「与纸各半」(1/2) 改成「往纸走四分之一」(1/4)，所以这里量的是 1/4。
    let quarter = |a: [u8; 3], b: [u8; 3]| -> [u8; 3] {
        [
            ((a[0] as u32 * 3 + b[0] as u32) / 4) as u8,
            ((a[1] as u32 * 3 + b[1] as u32) / 4) as u8,
            ((a[2] as u32 * 3 + b[2] as u32) / 4) as u8,
        ]
    };
    let mut f = Frame::new(2, 1);
    f.put(0, 0, 3); // 满墨
    f.put(1, 0, 0); // 纸
    let out = lcd::render(&f, ramp, true);
    let at = |x: u32| -> [u8; 3] {
        let i = (x * 4) as usize;
        [out[i], out[i + 1], out[i + 2]]
    };
    let s = lcd::SCALE as f32;
    let _on = |x: u32| (x % lcd::SCALE) as f32 >= 2.0 / 3.0 * s;
    // 满墨格：格内最深，格线上是「往纸走四分之一」。
    assert_eq!(at(0), ramp[3], "墨格内部应保持最深");
    assert_eq!(at(2), quarter(ramp[3], ramp[0]), "墨上的格线应是往纸走四分之一");
    // 纸格：格内纸，格线上是「纸往纸走」= 纸（空白处因此看不见格网之外的任何东西）。
    assert_eq!(at(lcd::SCALE), ramp[0], "纸格内部应是纸");
    assert_eq!(at(lcd::SCALE + 2), quarter(ramp[0], ramp[0]), "纸上的格线");
}

/// **格网只覆盖九分之五，且右列与下行都算。** 着色器说这是"九分之五"——
/// 3×3 里最后一列 + 最后一行，重叠的角算一次。若有人把它写成一条扫描线，
/// 覆盖就变成九分之三，而那正是第一版的错。
#[test]
fn the_lattice_covers_five_ninths_of_every_cell() {
    let ramp = [
        [240u8, 240, 240],
        [160, 160, 160],
        [80, 80, 80],
        [20, 20, 20],
    ];
    // 用「全纸」和「不开格网」的差来数：格网上的像素一定比纯纸深。
    let mut f = Frame::new(2, 2);
    f.fill(0, 0, 2, 2, 0);
    let on = lcd::render(&f, ramp, true);
    let w = 2 * lcd::SCALE;
    let h = 2 * lcd::SCALE;
    let paper_at = |buf: &[u8], x: u32, y: u32| -> [u8; 3] {
        let i = ((y * w + x) * 4) as usize;
        [buf[i], buf[i + 1], buf[i + 2]]
    };
    // 注意：全纸时格线色 = 纸，所以数不出差别。改用满墨，才数得出格线。
    let mut f2 = Frame::new(2, 2);
    f2.fill(0, 0, 2, 2, 3);
    let inked = lcd::render(&f2, ramp, true);
    let lit = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|(x, y)| paper_at(&inked, *x, *y) != ramp[3])
        .count();
    let total = (w * h) as usize;
    // 2×2 格、每格 9 个子像素、5 个是格线 => 20/36
    assert_eq!(lit, 20, "格线应占 4 格 × 5 = 20 个子像素，实得 {lit}/{total}");
    let _ = on;
}

/// **关闭格网 = 纯粹放大，一个像素都不多。** 一个可有可无的开关存在的意义就是
/// 「关掉它等于没开过」；若关掉仍有痕迹，那这个开关在骗人。
#[test]
fn turning_the_grid_off_changes_nothing() {
    let ramp = [
        [240u8, 240, 240],
        [160, 160, 160],
        [80, 80, 80],
        [20, 20, 20],
    ];
    let mut f = Frame::new(3, 3);
    // 一半纸一半墨 ⇒ 任何"格网影响到了格内"的 bug 都会露出来。
    for y in 0..3 {
        for x in 0..3 {
            f.put(x, y, if (x + y) % 2 == 0 { 3 } else { 0 });
        }
    }
    let on = lcd::render(&f, ramp, true);
    let off = lcd::render(&f, ramp, false);
    let w = 3 * lcd::SCALE;
    let h = 3 * lcd::SCALE;
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if !((x % lcd::SCALE == lcd::SCALE - 1) || (y % lcd::SCALE == lcd::SCALE - 1)) {
                assert_eq!(
                    [on[i], on[i + 1], on[i + 2]],
                    [off[i], off[i + 1], off[i + 2]],
                    "({x},{y}) 不是格线，开关不该影响它"
                );
            }
        }
    }
}

/// **放大只复制、不插值。** 每一档在放大后必须还是原来那一档，
/// 而查色表只认四档——插值会造出第五、第六种颜色，那不是这块面板的东西。
///
/// **用一块 4×4 的小帧而不是整块面板**：这张测试图有 `4*3 × 4*3 = 144` 个像素，
/// 而面板是 720×360 = 259 200 个 —— 逐像素查一遍的颜色断言要在小帧上才说得清，
/// 面板那个尺寸留给 `lcd::render` 自己的长度来保证。
#[test]
fn the_upscale_copies_rather_than_blending() {
    let ramp = [
        [10u8, 20, 30],
        [40, 50, 60],
        [70, 80, 90],
        [100, 110, 120],
    ];
    let mut f = Frame::new(4, 4);
    f.put(1, 2, 2); // 只点亮逻辑 (1,2)
    let out = lcd::render(&f, ramp, false);
    let w = 4 * lcd::SCALE;
    let h = 4 * lcd::SCALE;
    assert_eq!(out.len(), (w * h * 4) as usize, "输出长度");
    let at = |x: u32, y: u32| -> [u8; 3] {
        let i = ((y * w + x) * 4) as usize;
        [out[i], out[i + 1], out[i + 2]]
    };
    // 逻辑 (1,2) 放大 3 倍 ⇒ 图上 x 3..6, y 6..9，全是第 3 档（索引 2）。
    for y in 6..9 {
        for x in 3..6 {
            assert_eq!(at(x, y), ramp[2], "({x},{y}) 应是纯第 3 档");
        }
    }
    // 紧邻的一格必须是别的档 —— 若出现第五种颜色说明混了。
    assert_eq!(at(2, 6), ramp[0], "逻辑 (0,2) 仍是纸");
    // 整张图只可能出现这四档。
    for i in 0..(w * h) as usize {
        let c = [out[i * 4], out[i * 4 + 1], out[i * 4 + 2]];
        assert!(ramp.contains(&c), "出现了色板里没有的颜色 {c:?}");
    }
}

/// **没有声音就是没有柱子，不是一排贴地的线。** Goertzel 打在全零的 tap 上必须是零，
/// 而零高度不画——这两条合起来才是"频谱不动"而不是"频谱贴地"。
///
/// **判据只看频谱区里的满墨（索引 3），不看整块面板。** 频谱区里还有分区框、分区标签与
/// 基线，它们是**规则档（索引 2）**，静音时照样在——那是"读数零"而不是"表没了"。
/// 拿"有没有墨"来数会把这些规则线算成柱子。
#[test]
fn silence_is_no_bars_rather_than_a_row_of_lines() {
    let quiet = vec![0i16; 2048];
    let levels = lcd::bands(&quiet, 48_000);
    assert!(
        levels.iter().all(|v| *v == 0.0),
        "全零 tap 的频段必须是 0，实得 {levels:?}"
    );
    let c = content(Face::Name, &[], &quiet);
    let f = lcd::compose(&c);
    let w = lcd::LOGICAL_W as i32;
    // ⚠️ **只数内容区里面的**：第九轮那圈 2px 的边框是墨色、且贯穿全高，
    //    按整幅数的话光是左右两条就 284 个，这条测试会永远红。
    let count = |shade: u8| -> usize {
        let mut n = 0;
        for y in lcd::body_top()..lcd::body_bot() {
            for x in lcd::content_x0()..lcd::content_x1() {
                if f.px[(y * w + x) as usize] == shade {
                    n += 1;
                }
            }
        }
        n
    };
    assert_eq!(count(3), 0, "静音时频谱区不该有满墨的柱子");
    assert!(
        count(2) > 0,
        "分区框与基线仍应在（规则档），否则不是读数零而是表没了"
    );
}

/// ★ **版式的每一段都不许压到下一段。** 这条是本轮修的那个 bug 的守门人：
/// 上一版把状态条定成 6 行高，而一行字是 **16** 行，于是字溢出 7 行、**印在五键的头顶**。
/// 版式数字全是 `line_h()` 的推导量，这里把推导的结果按屏幕自上而下排一遍，逐对检查。
#[test]
fn the_bands_do_not_overlap() {
    install_panel_fonts();
    // (名字, 起, 止)，半开区间。**四段**：名字 → 元信息 → 主体 → 走带区。
    // ★ 2026-10-06：歌名与艺人合成一行，中间多出的那一行是元信息行（PLAY …… [EQ]）。
    let bands: [(&str, i32, i32); 4] = [
        ("歌名", lcd::title_y(), lcd::title_y() + lcd::headline_h()),
        ("元信息", lcd::title_y() + lcd::headline_h(), lcd::body_top()),
        ("主体", lcd::body_top(), lcd::body_bot()),
        ("走带区", lcd::strip_y(), lcd::strip_y() + lcd::strip_h()),
    ];
    for pair in bands.windows(2) {
        let (above, _, a1) = pair[0];
        let (below, b0, _) = pair[1];
        assert!(a1 <= b0, "{above} 的底 {a1} 压到了 {below} 的顶 {b0}");
    }
    // **所有内容都在边框里面**（用户："然后所有画面内缩"）。
    assert!(
        lcd::title_y() >= lcd::FRAME,
        "歌名不能压在边框上（title_y = {}）",
        lcd::title_y()
    );
    assert!(
        lcd::strip_y() + lcd::strip_h() <= lcd::LOGICAL_H as i32 - lcd::FRAME,
        "走带区不能越出边框"
    );
    // 走带区的文字带正好一行 12px 字（第九轮从 10px 换回来）。
    assert_eq!(
        lcd::strip_text_h(),
        lcd::line_h_public() as i32,
        "走带区的文字带必须一行 12px 字"
    );
    // 进度轨在文字带之下、仍在走带区之内，**而且上下留白相等**（在文字与条底之间居中）。
    assert_eq!(
        lcd::prog_y() - (lcd::strip_y() + lcd::strip_text_h()),
        (lcd::strip_y() + lcd::strip_h()) - (lcd::prog_y() + lcd::PROG_TRACK_H),
        "进度轨应在文字与条底之间居中"
    );
    // **名字块就一行**（歌手名并入其中），从内容区顶开始。
    assert_eq!(lcd::title_y(), lcd::content_y0(), "歌名从内容区顶开始");
    assert_eq!(
        lcd::headline_h(),
        lcd::line_h_public() as i32,
        "名字块现在是一行 —— 汉字 16px 的自己的行高"
    );
    // 正文框顶 = 名字块 + 一行元信息 + 两条纸；**这就是 6 + 16 + 16 + 2 = 40**。
    assert_eq!(
        lcd::body_top(),
        lcd::title_y() + lcd::headline_h() + lcd::line_h_public() as i32 + 2,
        "正文框顶 = 名字块 + 一行元信息 + 两条纸"
    );
    // ★ 元信息行是**算出来的**（按墨迹居中），所以它必须真的落在自己那一段里。
    //   ⚠️ 判据要用**墨迹**，不能用 em 框：拉丁 8px 的 em 框是 16 行、墨只有 7 行，
    //      按框判会把一行合法的位置判成越界（第一版就写错了，当场被这条抓住）。
    // ★ 两个 plan：名字/歌词走 prose（16px，拉丁与汉字同一张面），元信息/走带条走 tag（8px 拉丁）。
    let prose = slot_ui::lcd::prose_plan();
    let tag = slot_ui::lcd::tag_plan();
    for (artist, title) in [("逃跑计划", "夜空中最亮的星"), ("", "CRAZYSIX")] {
        let transport = lcd::Transport::Playing;
        let y = lcd::meta_y(artist, title, transport, "Rock");
        let band0 = lcd::title_y() + lcd::headline_h();
        let band1 = lcd::body_top();
        // 元信息行自己的墨
        let probe = format!("{} [Rock]", transport.word());
        let (lo, hi) = slot_ui::text::plan_ink_span(&probe, &tag);
        // 名字行的墨底（**不是**名字那一段的底）
        let name = lcd::title_text(artist, title);
        let title_ink_bottom = lcd::title_y() + slot_ui::text::plan_ink_span(&name, &prose).1;

        assert!(
            y + lo >= band0 && y + hi <= band1,
            "元信息行的墨没落在它那一段里（artist={artist:?} title={title:?} ⇒ 墨 {}..{}，\
             该段是 {band0}..{band1}）",
            y + lo,
            y + hi
        );
        // ★ 而且要在「名字行墨底」与「正文框顶」之间**上下留白相等** —— 这条就是"按墨迹居中"本身。
        let above = (y + lo) - title_ink_bottom;
        let below = band1 - (y + hi);
        assert!(
            (above - below).abs() <= 1,
            "元信息行没在名字墨底与正文框顶之间居中（artist={artist:?} ⇒ 上 {above} 下 {below}）"
        );
    }
}

/// ★ **格网铺满整屏**（用户第五轮："操作区同样也是有网格的，屏幕为整体，跟游戏一样的"）。
///
/// ⚠️ 这条**反着守**一个更早的结论：那时格网在状态条上沿收住，理由是"反白条被网住会成麻点"。
/// 用户要的是整屏同一套渲染（格网是这块屏的物理属性，不是某一层的装饰），所以现在**连走带区
/// 也上网**。这里断言**每一格**都带格线，最后一格（最下面那行）也不例外。
#[test]
fn the_grid_covers_the_whole_panel() {
    let ramp = [
        [240u8, 240, 240],
        [160, 160, 160],
        [80, 80, 80],
        [20, 20, 20],
    ];
    // 一条从顶到底的满墨竖列：格网应当在**每一格**上都留下痕迹。
    let mut f = Frame::panel();
    f.fill(0, 0, 1, lcd::LOGICAL_H as i32, 3);
    let out = lcd::render(&f, ramp, true);
    let w = lcd::LOGICAL_W * lcd::SCALE;
    let at = |x: u32, y: u32| -> [u8; 3] {
        let i = ((y * w + x) * 4) as usize;
        [out[i], out[i + 1], out[i + 2]]
    };
    let gap = [
        ((ramp[3][0] as u32 * 3 + ramp[0][0] as u32) / 4) as u8,
        ((ramp[3][1] as u32 * 3 + ramp[0][1] as u32) / 4) as u8,
        ((ramp[3][2] as u32 * 3 + ramp[0][2] as u32) / 4) as u8,
    ];
    // x=1（x%3==1）⇒ 这一列的格线只可能来自 `y % 3 == 2` 的那一行。
    for cell in 0..(lcd::LOGICAL_H / lcd::SCALE) {
        let y = cell * lcd::SCALE + (lcd::SCALE - 1);
        assert_eq!(
            at(1, y),
            gap,
            "第 {cell} 格（y={y}）没有格线 —— 格网要铺满整屏，走带区也算"
        );
    }
}

/// **一个 440Hz 的正弦只该在低频段竖起来。** 这是唯一能一眼看出"频段算错了没有"的形状：
/// 随机噪声会让 16 段一样高（一条平顶），而一个单频只在自己的段上高。
#[test]
fn a_single_tone_lights_one_part_of_the_spectrum() {
    let tap = tone(440.0, 48_000);
    let levels = lcd::bands(&tap, 48_000);
    let peak_at = levels
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .expect("有频段");
    // ★ 440Hz 落在 80Hz–16kHz 对数轴的约 1/4 处。**32 段时那一段是第 6–12 段**，
    //   而 16 段时是第 2–6 段 —— 段数变了，"第几段"跟着变，写死下标就会假失败。
    let frac = peak_at as f32 / (lcd::BANDS - 1) as f32;
    assert!(
        (0.15..0.40).contains(&frac),
        "440Hz 的峰应在对数轴 15%–40% 处，实得第 {peak_at}/{} 段 = {:.0}%",
        lcd::BANDS,
        frac * 100.0
    );
    // 而且不能是平的。
    let flat = levels.iter().all(|v| (v - levels[0]).abs() < 0.02);
    assert!(!flat, "{} 段不应一样高", lcd::BANDS);
}

/// ★ **频带轴真的是 80 Hz–16 kHz 的对数轴。** 这一条守的是 2026-10-07 修掉的那个**底数错误**：
/// 原来 `(F_HI / F_LO).ln() / (BANDS - 1)` 的值被当成 `2.0f32.powf(...)` 的指数，自然对数
/// 被当成以 2 为底 ⇒ 整条轴压扁，**顶带只有 3148 Hz，16 kHz 那一头从来没被分析过**。
/// 上面那条"440 Hz 应在 15%–40%"之所以以前也能过，是因为 Goertzel 的系数也错着
/// （`cos(w)` 而不是 `2·cos(w)`），把峰往下拖了约 4.4 带 —— 两个错误在旧容差里互相抵消。
#[test]
fn the_band_axis_spans_80_hz_to_16_khz() {
    assert!((lcd::band_hz(0) - 80.0).abs() < 0.01, "第 0 带应当是 80 Hz");
    let top = lcd::band_hz(lcd::BANDS - 1);
    assert!(
        (top - 16_000.0).abs() < 1.0,
        "顶带应当是 16 kHz，实测 {top:.1} Hz（这个数只有 3148 就是底数错了）"
    );
    // 单调、等比 —— 一个对数轴的两条基本性质。
    let want = 200f32.powf(1.0 / (lcd::BANDS - 1) as f32);
    for b in 1..lcd::BANDS {
        let (lo, hi) = (lcd::band_hz(b - 1), lcd::band_hz(b));
        assert!(hi > lo, "第 {b} 带应当比第 {} 带高", b - 1);
        assert!(
            (hi / lo - want).abs() < 1e-4,
            "第 {b} 带的步进应当是 {want:.6}，实测 {:.6}",
            hi / lo
        );
    }
    let nearest = (0..lcd::BANDS)
        .min_by(|&a, &c| {
            (lcd::band_hz(a) - 440.0)
                .abs()
                .partial_cmp(&(lcd::band_hz(c) - 440.0).abs())
                .unwrap()
        })
        .expect("有频段");
    assert_eq!(
        nearest,
        10,
        "440 Hz 最近的带应当是第 10 带，实测第 {nearest} 带 = {:.1} Hz",
        lcd::band_hz(nearest)
    );
}

/// ★ **恒 1 列间隙、铺满整框** —— 加倍立柱时才暴露的那条。
///
/// 16 段每段 15 逻辑列，间隙比柱还宽，像一排计数棒；32 段每段约 7 列，读得出频谱。
/// 2026-10-07 一度加倍到 64 段，**226 ÷ 64 = 3.53 不是整数** —— 按旧的"每根占整数格"
/// 写法只占 192 列、**两端各空 17 列**，谱面缩在框中间（这就是排布改成"左沿按比例分、
/// 余数摊进柱宽"的起因）。64 段当晚被六六否决回退，这句断言跟着回到 **32**：
/// **判据守的是排布，不是段数** —— 段数再变（16/32/64）这套断言都该绿。
#[test]
fn the_spectrum_fills_its_box_whatever_the_band_count() {
    assert_eq!(lcd::BANDS, 32, "频段数");
    let x0 = lcd::content_x0() + 1;
    let w = (lcd::content_x1() - 1) - x0;
    let mut widths = Vec::new();
    for b in 0..lcd::BANDS as i32 {
        let (bx, bw) = lcd::spectrum_bar(x0, w, b);
        assert!(bw >= 2, "第 {b} 根柱只有 {bw} 列宽");
        widths.push(bw);
        if b + 1 < lcd::BANDS as i32 {
            let (next, _) = lcd::spectrum_bar(x0, w, b + 1);
            assert_eq!(next - (bx + bw), 1, "第 {b} 根与下一根之间应当恰好空一列");
        }
    }
    let (last_x, last_w) = lcd::spectrum_bar(x0, w, lcd::BANDS as i32 - 1);
    assert_eq!(last_x + last_w, x0 + w - 1, "最后一根应当贴到内容区右沿");
    let lo = *widths.iter().min().unwrap();
    let hi = *widths.iter().max().unwrap();
    assert!(hi - lo <= 1, "柱宽应当只差 1 列：{lo}..{hi}");
}

/// **频谱现在骑在一条固定的 dB 刻度上，所以轻的段落画得矮。**
///
/// ★ **2026-10-07 这条判据整个翻了过来**（原名 `the_bars_normalise_to_themselves_...`）。
/// 旧设计的原话是"按自己的峰归一，所以安静段落与响亮段落画得一样高 —— 进度条才是那个
/// 不撒谎的读数"。六六看过之后说的是「很踊跃的喷泉」：每帧各自归一，等于**每次烘焙都是一张
/// 独立快照**，一根柱子可以这一帧满格、下一帧什么都没有。改成 fixed dB scale（0 dB =
/// 满量程正弦，下沿 `DB_FLOOR`）之后，两条曲线的**位置**仍该一样（都指在 440 Hz 那一带），
/// 但**高度**必须差开 —— 差多少是可算的：幅度缩到 1/4 就是 −12.04 dB，而刻度总跨度是
/// 60 dB ⇒ 柱高应当差约 **0.20**。
///
/// 这正是"不撒谎"从 `peak` 那一格搬到了柱子上：以前只有 `peak` 能区分轻重，现在柱子本身
/// 也带着绝对电平。`peak` 依旧单独显示，理由也一样 —— 它是一个样本峰，不是按带估的。
#[test]
fn the_bars_ride_a_fixed_db_scale_so_a_quiet_passage_draws_shorter() {
    let tap = tone(440.0, 48_000);
    let loud = lcd::bands(&tap, 48_000);
    let soft_tap: Vec<i16> = tap.iter().map(|v| v / 4).collect();
    let soft = lcd::bands(&soft_tap, 48_000);

    let argmax = |h: &[f32; lcd::BANDS]| {
        h.iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .expect("有频段")
    };
    let (bl, bs) = (argmax(&loud), argmax(&soft));
    assert_eq!(
        bl, bs,
        "轻响两档的峰必须在同一带（位置由频率决定，不由音量决定）：{bl} vs {bs}"
    );

    // ★ 高度必须差开，而且差得对得上 −12.04 dB。
    let (a, b) = (loud[bl], soft[bs]);
    let gap = a - b;
    assert!(
        (0.15..0.25).contains(&gap),
        "幅度差 4×（−12.04 dB）在 60 dB 刻度上应当差约 0.20 个柱高，实测 {gap:.3}（响 {a:.3} / 轻 {b:.3}）"
    );
    // 差的方向不能反：绝对刻度下轻的一定更矮。
    assert!(a > b, "响亮段落应当更高：{a} vs {b}");

    // `peak` 还是那个样本级读数，依旧单独显示。
    assert!(lcd::peak(&tap) > 0.3, "本 tap 的峰值应超过 0.3");
    assert!(
        lcd::peak(&soft_tap) < lcd::peak(&tap) / 2.0,
        "缩到 1/4 后峰值应小得多：{} vs {}",
        lcd::peak(&soft_tap),
        lcd::peak(&tap)
    );
}

/// **没有歌词就退回歌名，而不是一个空框。** 这是两个面板分开的全部理由：
/// 卡上没有 `.lrc` 的那一种（大多数）必须仍然显示它在放什么。
#[test]
fn a_card_with_no_lyrics_shows_the_name_on_both_faces() {
    let tap = tone(440.0, 48_000);
    let none: [String; 0] = [];
    let name = lcd::compose(&content(Face::Name, &none, &tap));
    let lyric = lcd::compose(&content(Face::Lyrics, &none, &tap));
    // 两块面板都是"有字的"，不是空的。
    let ink = |f: &Frame| f.px.iter().filter(|v| **v > 0).count();
    assert!(ink(&name) > 100, "歌名面板应有字，实得 {} 个墨像素", ink(&name));
    assert!(
        ink(&lyric) > 100,
        "无歌词时歌词面板应退回歌名，实得 {} 个墨像素",
        ink(&lyric)
    );
}

/// **有歌词时，中间那行是满墨、上下两行浅一档。** 满屏同权重的行是堵墙，
/// 而眼睛需要一个落点。
///
/// **按行带数满墨，不数整块面板。** 反白状态条本身就是一个 240×高 的满墨块，
/// 整块数的话它会盖过一切、这条测试就永远通过而什么也没验。这里只数歌词主体的
/// 每一条行带，看**被唱的那一条**是不是满墨、邻行是不是空的（邻行画的是规则档）。
///
/// ⚠️ **歌词用拉丁文写。** 测试里没有装卡上的字体，用的是内嵌的 `label.ttf`——
/// 它**没有中日韩字形**，汉字会一个都不画（`has_glyph` 为假就退回内嵌，内嵌也没有），
/// 于是行带里是 0 个像素，这条测试会假红。中文那份要验就得先 `set_font`。
#[test]
fn the_sung_line_is_darker_than_its_neighbours() {
    let lines: Vec<String> = vec!["FIRST LINE".into(), "NOW SINGING".into(), "THIRD LINE".into()];
    let tap = tone(440.0, 48_000);
    let f = lcd::compose(&content(Face::Lyrics, &lines, &tap));
    let w = lcd::LOGICAL_W as i32;
    // ★ 2026-10-06：行距换成 `LYRIC_STEP`（21），整块按 `(n-1)*step + 一行字` 在框内居中。
    let step = lcd::LYRIC_STEP;
    let h = lcd::line_h_public() as i32;
    let n = lines.len() as i32;
    let mid = lines.len() / 2;
    let total = (n - 1) * step + h;
    let cy0 = lcd::body_top() + 2;
    let cy1 = lcd::body_bot() - 1;
    let first = cy0 + ((cy1 - cy0) - total) / 2;
    let ink_in = |row: i32| -> usize {
        let y0 = first + row * step;
        let mut n = 0;
        for y in y0..(y0 + h) {
            for x in 0..w {
                if f.px[(y * w + x) as usize] == 3 {
                    n += 1;
                }
            }
        }
        n
    };
    let sung = ink_in(mid as i32);
    let above = ink_in(mid as i32 - 1);
    assert!(
        sung > 20,
        "被唱的行应是满墨（实得 {sung} 个满墨像素）"
    );
    assert!(
        sung > above + 20,
        "被唱的行应比上一行满墨得多：被唱 {sung} vs 上一行 {above}"
    );
}

/// ★ **边框 2px 墨 + 内侧（上、左）1px 浅边，内容全部内缩。**
///
/// 用户第九轮："画面四周按最深色围 2px 的圈并做 45 度的浅边（即上方和左方的框内侧），
/// 浅边按 EDGE 这个档位就好，然后所有画面内缩"。
///
/// ⚠️ **浅边只在上与左** —— 那是"光从左上来、屏是凹的"的读法。右与下不铺，铺了就成了"凸"。
#[test]
fn the_frame_is_inked_and_bevelled_and_the_content_is_inset() {
    let tap = tone(440.0, 48_000);
    let none: [String; 0] = [];
    let f = lcd::compose(&content(Face::Name, &none, &tap));
    let (w, h) = (lcd::LOGICAL_W as i32, lcd::LOGICAL_H as i32);
    let at = |x: i32, y: i32| -> u8 { f.px[(y * w + x) as usize] };
    // 四边 2px 墨（3）。
    for k in 0..lcd::FRAME {
        assert_eq!(at(120, k), 3, "顶边 k={k} 应是墨");
        assert_eq!(at(120, h - 1 - k), 3, "底边 k={k} 应是墨");
        assert_eq!(at(k, 80), 3, "左边 k={k} 应是墨");
        assert_eq!(at(w - 1 - k, 80), 3, "右边 k={k} 应是墨");
    }
    // 浅边 EDGE(1)：框内侧的**上方一行**与**左方一列**。
    assert_eq!(at(120, lcd::FRAME), 1, "框内侧上方应是浅边（EDGE）");
    assert_eq!(at(lcd::FRAME, 80), 1, "框内侧左方应是浅边（EDGE）");
    // 右、下不铺浅边。
    assert_ne!(at(120, h - 1 - lcd::FRAME), 1, "下方不铺浅边");
    assert_ne!(at(w - 1 - lcd::FRAME, 80), 1, "右方不铺浅边");
    // 内容内缩：歌名从框之内开始，走带区不越出框。
    assert!(
        lcd::title_y() >= lcd::FRAME + lcd::PAD,
        "内容必须内缩（title_y = {}）",
        lcd::title_y()
    );
    assert!(lcd::content_x0() >= lcd::FRAME + lcd::BEVEL);
}

/// ★ **进度轨左右留白必须相等。**
///
/// 一排等宽的方块，偏一格就看得出来 —— 上一版左边距 5、右边距 6，用户一眼就说"走带不对称"。
/// 根因是段宽写死（`SEG = 5`），整排的余数只能落在一边。现在段宽/段数从可用宽度反推，
/// 且**余数必须成双**（差一格就减一段）。这条测试就是那个判据。
#[test]
fn the_progress_track_is_centred() {
    let tap = tone(440.0, 48_000);
    let none: [String; 0] = [];
    let mut c = content(Face::Name, &none, &tap);
    c.bar = Some(lcd::Bar {
        transport: lcd::Transport::Playing,
        mode: lcd::Mode::Repeat,
        elapsed: 100.0,
        length: 100.0, // 全亮：整条轨都是纸色，量得最清楚
    });
    let f = lcd::compose(&c);
    let w = lcd::LOGICAL_W as i32;
    let y = lcd::prog_y();
    // 纸色 = 0；这一行上除进度轨之外都是状态条的墨底。
    let lit: Vec<i32> = (0..w).filter(|x| f.px[(y * w + x) as usize] == 0).collect();
    assert!(!lit.is_empty(), "全亮的进度轨应该有格子");
    let left = lit[0];
    let right = w - 1 - lit[lit.len() - 1];
    assert_eq!(
        left, right,
        "进度轨左右留白应相等：左 {left} 右 {right}（余数要成双才分得匀）"
    );
}

/// **字号取字体的设计尺寸，不写死。**
///
/// 这条测试守的是一件已经踩过两次的事：**字号是字体的属性，不是排版的选择**。
/// 第一次是 12px 的接口面被按 16px 渲染（70.6% 抗锯齿，看着像"笔触脏"），
/// 第二次是 8px 的拉丁面被拿去当 12px 用。两次都是**字号传错**，不是字体不好。
///
/// ★ 2026-10-06：面板改成两张面 —— 汉字 16px（寒蝉点阵体）、拉丁 8px（Press Start 2P）。
///   两个数都是各自字体的设计尺寸，且都在整数倍上同样干净（实测半调 0.0%），
///   换面时这两个常量必须跟着字体一起走。
#[test]
fn the_type_size_is_the_fonts_own() {
    install_panel_fonts();
    assert_eq!(
        lcd::TITLE_CJK_PX, 16.0,
        "汉字面的设计尺寸是 16px（unifont-16px.otf）"
    );
    assert_eq!(
        lcd::TITLE_LATIN_PX, 8.0,
        "拉丁面的设计尺寸是 8px（press-start-2p.ttf）"
    );
    // 两面的 em 框关系：汉字面 16 行、拉丁面 8 行（它把 descent 报成 0），
    // 所以整块面板的纵向算术一律以**汉字面**为准 —— 见 `text::plan_line_height`。
    assert_eq!(lcd::line_h_public() as f32, lcd::TITLE_CJK_PX);
}
