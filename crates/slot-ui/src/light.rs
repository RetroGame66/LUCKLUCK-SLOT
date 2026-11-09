//! 发光件：这台机器**亮着**的部分，与它被**印成**什么样无关。
//!
//! # 为什么要单独一个模块
//!
//! `palette` 管的是印刷 —— 纸、壳、字、卡带配色，七个 look × 两种模式，全部跟 look 走。
//! 这一份管的是发光 —— 电源灯、卡座受光边、通电渐隐线、扫描线。它们**不跟 look 走**：
//! 换一套粉色壳不会把 LED 换成粉色，因为发光的不是壳，是壳里的那颗灯珠。
//! 两者混在一个模块里，早晚会出现「某个 look 下 LED 变成塑料色」这种错误。
//!
//! # 唯一琥珀
//!
//! 整台机器只有**一种**发光颜色。这条是从工具箱那一版 UI 抄过来的结论，理由在那里也一样：
//! 几个等权的发光色会让整屏都在喊，而「主次分明」只有一个办法 —— 光只出现在真有状态的地方，
//! 并且只有一种颜色。
//!
//! # 每个光都要对应一个真实状态
//!
//! 深色底上堆霓虹是廉价感的头号来源，所以这里只做「一台通着电的设备」：
//!
//! | 光 | 它说的事 |
//! |---|---|
//! | `dot_glow` | 电源灯 —— 机器醒着（唯一的常态动效，缓慢呼吸） |
//! | `power_line` | 一条通着电的边 —— 面板的顶边 |
//! | `title_glow` | 这块地方亮着 —— 居中标题的背光 |
//! | `hairline` | 手里拿的是这一张（焦点） |
//! | `halo` | 一个正在发生的动作（插入 / 拔出） |
//! | `scanlines` | 这是一块屏，不是一张纸 |
//!
//! # 预渲染
//!
//! 每个生成器都返回一份 RGBA8，开机烘一次，之后只是一次 `Draw::Tex`。
//! 每帧重算渐变是把 GPU 该干的活搬回 CPU，而一帧只有 16.667 ms。

use slot_gfx::Draw;

/// 机器上唯一的那颗灯的颜色。
///
/// 255/176/0 是工具箱那一版定下来的琥珀：在近黑的底上它是暖的，但不像橙那样带警告意味；
/// 压到 8bit 之后仍然有 15 级以上的色阶，渐变不会断。
pub const AMBER: [u8; 3] = [255, 176, 0];

/// 同一颗灯**被按下去**时的颜色 —— 实底上用它，差一档亮度就够读作"按下"，不必换色相。
pub const AMBER_DK: [u8; 3] = [198, 132, 0];

/// 琥珀压在深墨底上时的影子色：实底按钮按下那一瞬的内侧，以及发光件正下方的接触阴影。
pub const AMBER_DEEP: [u8; 3] = [86, 62, 10];

/// 呼吸一个来回要多久。3400 ms 是"注意到但不会想看第二眼"的速度 ——
/// 快过 2 s 像在闪，慢过 5 s 会让人怀疑它是不是卡住了。
pub const BREATHE_MS: u32 = 3400;

/// 一个 `[u8;3]` 加一个 0..1 的透明度，变回 `Draw` 用的 `[f32;4]`。
pub const fn rgba(c: [u8; 3], a: f32) -> [f32; 4] {
    [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, a]
}

/// 发光件在**亮色界面**上的强度系数。
///
/// 这一条不违反"发光不跟 look 走"：**颜色**不跟，**强度**必须跟。
/// 光是被看见的，而白纸上没有光 —— 琥珀压在 0.9 的纸上只会变成一块脏黄，
/// 读不出"这里亮着"。所以亮色模式一律收到 0.42：线（焦点边、通电边）还读得出来，
/// 面（标题背光）退到只剩一层暖意。深色模式是 1.0，不做任何修饰。
///
/// **是系数不是开关**：整个关掉的话，亮色模式下焦点就重新变成"只有缩放没有指示"，
/// 那正是这一版要修的东西。
pub fn gain() -> f32 {
    match crate::palette::mode() {
        crate::palette::Mode::Light => 0.42,
        crate::palette::Mode::Dark => 1.0,
    }
}

/// 灯的亮度，0..1，按 `ms` 走一个 3400 ms 的正弦来回。///
/// **下限 0.55 而不是 0**：这是一颗**亮着**的灯在呼吸，不是一颗在闪的灯。
/// 熄到零就变成了告警，而告警要留给真的告警。
pub fn breathe(ms: u32) -> f32 {
    let t = (ms % BREATHE_MS) as f32 / BREATHE_MS as f32;
    let s = 0.5 - 0.5 * (std::f32::consts::TAU * t).cos();
    0.55 + 0.45 * s
}

// ── 生成器 ───────────────────────────────────────────────────────────────────
// 统一返回 `(w, h, rgba)`：缓冲区按行优先，长度一定是 `w * h * 4`。
// 尺寸随值一起返回而不是让调用方自己记，是因为像素尺寸是这些函数的**输入**，
// 而调用方要的往往是"把它铺到 x..x+w" —— 两个数分开存，早晚会对不上。

fn blank(w: u32, h: u32) -> Vec<u8> {
    vec![0; (w as usize) * (h as usize) * 4]
}

/// 写一个像素。`a` 是 0..1，越界直接忽略 —— 这些函数里所有坐标都由循环给出，
/// 越界只可能是尺寸算错，而尺寸算错时安静地不画比 panic 好：一块屏上少一个光点，
/// 比整台机器起不来轻得多。
fn put(buf: &mut [u8], w: u32, x: i32, y: i32, c: [u8; 3], a: f32) {
    if x < 0 || y < 0 || x >= w as i32 {
        return;
    }
    let i = ((y as usize) * (w as usize) + (x as usize)) * 4;
    if i + 3 >= buf.len() {
        return;
    }
    let a = a.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    buf[i] = c[0];
    buf[i + 1] = c[1];
    buf[i + 2] = c[2];
    buf[i + 3] = (a * 255.0).round() as u8;
}

/// 通电渐隐线：一条从左边最亮、向右衰减到零的横线，两像素高。
///
/// 这是整套灯效里唯一带"方向"的一个 —— 它读作电流从左往右走，所以它出现在
/// **面板的起始边**（左上角那一条），而不是随便什么地方。曲线是 `(1-t)^1.5`：
/// 直线衰减到一半就看不出在衰减了，1.5 次方在前面留得住亮度、后半段收得干净。
///
/// 高度固定 2：第一行全亮、第二行半亮，这样一条 1px 的线在 720 宽的屏上也有体积，
/// 而不至于粗到像一条边框。
pub fn power_line(w: u32) -> (u32, u32, Vec<u8>) {
    let h = 2u32;
    let mut buf = blank(w, h);
    let n = w.max(1) as f32;
    for x in 0..w as i32 {
        let t = x as f32 / (n - 1.0).max(1.0);
        let a = 0.86 * (1.0 - t).powf(1.5);
        put(&mut buf, w, x, 0, AMBER, a);
        put(&mut buf, w, x, 1, AMBER, a * 0.5);
    }
    (w, h, buf)
}

/// 点光源辉光：一个直径为 `d` 的圆，中心最亮，边缘收干净。
///
/// 幂次 1.8 与 2 的差别在**边缘**：平方衰减到半径处还剩一点尾巴，看起来像一个脏边；
/// 1.8 收得更快，边缘是干净的。中心不用纯白 —— 琥珀自己就够了，掺白会让它读成橙。
pub fn dot_glow(d: u32) -> (u32, u32, Vec<u8>) {
    let d = d.max(2);
    let mut buf = blank(d, d);
    let r = d as f32 / 2.0;
    let c = r - 0.5;
    for y in 0..d as i32 {
        for x in 0..d as i32 {
            let dx = x as f32 - c;
            let dy = y as f32 - c;
            let dist = (dx * dx + dy * dy).sqrt() / r;
            if dist >= 1.0 {
                continue;
            }
            let a = 0.62 * (1.0 - dist).powf(1.8);
            put(&mut buf, d, x, y, AMBER, a);
        }
    }
    (d, d, buf)
}

/// 矩形外扩光晕。`spread` 是向外扩多少像素，`radius` 是矩形本身的圆角。
///
/// 只在**有动作正在发生**的时候用（插卡、拔卡），静态界面上一个都不用 ——
/// 一块屏上同时亮着三个光晕，它们就不再表示任何事。
pub fn halo(w: u32, h: u32, spread: u32, radius: u32, alpha: f32) -> (u32, u32, Vec<u8>) {
    let ow = w + spread * 2;
    let oh = h + spread * 2;
    let mut buf = blank(ow, oh);
    // 从外往里叠：每一圈都比外一圈更亮一点，叠完就是一条平滑的衰减带。
    for i in (1..=spread).rev() {
        let t = 1.0 - i as f32 / spread as f32;
        let a = alpha * t.powf(1.7);
        // 这一圈是"离矩形 i 像素"的壳：画四条边就够了，中间那块下面的圈已经填过。
        let (x0, y0) = (spread as i32 - i as i32, spread as i32 - i as i32);
        let (x1, y1) = (x0 + w as i32 + i as i32 * 2, y0 + h as i32 + i as i32 * 2);
        let rr = radius as i32 + i as i32;
        for y in y0..y1 {
            for x in x0..x1 {
                // 圆角：四个角上按到圆心距离裁掉。
                let cx = if x < x0 + rr {
                    x0 + rr
                } else if x >= x1 - rr {
                    x1 - rr - 1
                } else {
                    x
                };
                let cy = if y < y0 + rr {
                    y0 + rr
                } else if y >= y1 - rr {
                    y1 - rr - 1
                } else {
                    y
                };
                let dx = (x - cx) as f32;
                let dy = (y - cy) as f32;
                if dx * dx + dy * dy > (rr * rr) as f32 {
                    continue;
                }
                // 只描这一圈的外缘，里圈留给更亮的下一轮。
                let on_edge = x == x0 || y == y0 || x == x1 - 1 || y == y1 - 1;
                if !on_edge {
                    continue;
                }
                put(&mut buf, ow, x, y, AMBER, a);
            }
        }
    }
    (ow, oh, buf)
}

/// 背光带：标题**后面**那一层，中间最亮、向两端对称收掉。
///
/// 和 `power_line` 是一对相反的用法：那条讲"电从左往右走"，只适合左对齐的标题；
/// 这条讲"这块地方被照亮了"，适合**居中**的标题 —— 货架的名字就是居中的，
/// 一条右渐隐的线压在一个居中标题下面，会读成排版错了而不是灯。
///
/// 高度给 48（比标题面高 8）：上下各留 4 px 的衰减，字才像是**浮在光里**而不是坐在一条色带上。
pub fn title_glow(w: u32, h: u32) -> (u32, u32, Vec<u8>) {
    let mut buf = blank(w, h);
    let n = w.max(1) as f32;
    for y in 0..h as i32 {
        // 竖直方向：中间一条，上下各让 4px 淡出。
        let vy = (y as f32 + 0.5) / h as f32;
        let vfade = (1.0 - (2.0 * vy - 1.0).abs()).powf(0.55);
        for x in 0..w as i32 {
            let t = (x as f32 + 0.5) / n;
            // 水平方向：峰值在正中，二次衰减到两端为 0。
            let hfade = (1.0 - (2.0 * t - 1.0).abs()).powf(2.0);
            let a = 0.20 * hfade * vfade;
            put(&mut buf, w, x, y, AMBER, a);
        }
    }
    (w, h, buf)
}

/// 脚下的柔光池：货架上**被手里那张卡挡住的那一小块光**。
///
/// # 为什么不是一圈框
///
/// 第一版给选中卡带加的是一圈 1px 琥珀发丝边，六六当场否了：
/// **「卡带贴图不是死板的矩形，它是有轮廓的，好不容易做的透明底被你一框又是一个矩形。」**
/// 说得对 —— 卡带的贴图是按 GBA 卡的真实形状裁的透明 PNG，一个矩形框等于把那道轮廓重新
/// 宣布成矩形，之前做的形状白做了。
///
/// 池子不碰卡带本身：它画在卡带**脚下**，比卡带宽一截、比卡带矮得多，所以它读作
/// 「这块货架被它挡住了光」，而不是「有个框把它框住了」。轮廓一根线都没盖。
///
/// # 形状
///
/// 横向收得比竖向狠（1.6 对 0.7）：竖向平一点才像摊在地上的一滩光，
/// 竖向也收得狠就会变成一个椭圆墨点。峰值 0.22 —— 再亮就开始抢卡带。
pub fn pool(w: u32, h: u32) -> (u32, u32, Vec<u8>) {
    let mut buf = blank(w, h);
    let n = w.max(1) as f32;
    let m = h.max(1) as f32;
    for y in 0..h as i32 {
        let u = (y as f32 + 0.5) / m;
        let vfade = (1.0 - (2.0 * u - 1.0).abs()).powf(0.7);
        for x in 0..w as i32 {
            let t = (x as f32 + 0.5) / n;
            let hfade = (1.0 - (2.0 * t - 1.0).abs()).powf(1.6);
            put(&mut buf, w, x, y, AMBER, 0.22 * hfade * vfade);
        }
    }
    (w, h, buf)
}

/// CRT 扫描线：每隔 `step` 行一条极淡的亮线。///
/// 只铺在**屏幕**上（播放器面板、游戏画面之外的 UI 面），绝不铺满整块面板 ——
/// 铺满就等于给整台机器罩了一层纱，那正是"廉价复古"最典型的做法。
/// 默认 alpha 0.035 / step 3：在这个量级上它只在**大面积纯色**里看得见，
/// 而这恰好是它该起作用的地方。
pub fn scanlines(w: u32, h: u32, alpha: f32, step: u32) -> (u32, u32, Vec<u8>) {
    let step = step.max(2);
    let mut buf = blank(w, h);
    let mut y = 0u32;
    while y < h {
        for x in 0..w as i32 {
            put(&mut buf, w, x, y as i32, [255, 255, 255], alpha);
        }
        y += step;
    }
    (w, h, buf)
}

// ── 画 ───────────────────────────────────────────────────────────────────────

/// 一圈 1px 的框，四条 `Draw::Rect` 推进去。
///
/// **为什么不做成贴图**：1px 的线做贴图要处理缩放的半像素对齐，而四条矩形在
/// 整数坐标上永远是干净的 1px。省一次烘焙，也省一次上传。
///
/// `colour` 带 alpha，所以同一个函数既是发光的（`hairline`）也是不发的（面板描边）。
pub fn outline(out: &mut Vec<Draw>, x: f32, y: f32, w: f32, h: f32, colour: [f32; 4]) {
    if w <= 0.0 || h <= 0.0 || colour[3] <= 0.0 {
        return;
    }
    let (x, y, w, h) = (x.round(), y.round(), w.round(), h.round());
    // 上下两条通宽，左右两条让开上下已经占掉的那一像素 —— 四角只画一次。
    out.push(Draw::Rect {
        x,
        y,
        w,
        h: 1.0,
        colour,
    });
    out.push(Draw::Rect {
        x,
        y: y + h - 1.0,
        w,
        h: 1.0,
        colour,
    });
    if h > 2.0 {
        out.push(Draw::Rect {
            x,
            y: y + 1.0,
            w: 1.0,
            h: h - 2.0,
            colour,
        });
        out.push(Draw::Rect {
            x: x + w - 1.0,
            y: y + 1.0,
            w: 1.0,
            h: h - 2.0,
            colour,
        });
    }
}

/// 一圈 1px 的琥珀发丝边。
///
/// `a` 是想要的不透明度。焦点用 0.55 —— 满亮度的框会把卡带本身压下去，
/// 而这里要表达的是"手里拿着"，不是"选中了一个控件"。
pub fn hairline(out: &mut Vec<Draw>, x: f32, y: f32, w: f32, h: f32, a: f32) {
    outline(out, x, y, w, h, rgba(AMBER, a * gain()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(buf: &[u8], w: u32, x: u32, y: u32) -> u8 {
        buf[((y * w + x) * 4 + 3) as usize]
    }

    #[test]
    fn power_line_fades_right() {
        let (w, h, buf) = power_line(64);
        assert_eq!((w, h), (64, 2));
        assert_eq!(buf.len(), 64 * 2 * 4);
        // 左端最亮，右端熄灭，中间单调。
        assert!(alpha_at(&buf, w, 0, 0) > 200);
        assert_eq!(alpha_at(&buf, w, 63, 0), 0);
        let mut prev = 255u8;
        for x in 0..w {
            let a = alpha_at(&buf, w, x, 0);
            assert!(a <= prev, "第 {x} 列比左边亮");
            prev = a;
        }
        // 第二行是半亮，这就是它的"体积"。
        for x in [0u32, 10, 30] {
            let (top, bottom) = (alpha_at(&buf, w, x, 0), alpha_at(&buf, w, x, 1));
            assert!(bottom <= top / 2 + 1 && bottom * 2 <= top * 2);
        }
    }

    #[test]
    fn dot_glow_is_round_and_centred() {
        let (d, _, buf) = dot_glow(33);
        let mid = d / 2;
        let centre = alpha_at(&buf, d, mid, mid);
        assert!(centre > 100, "中心不亮：{centre}");
        // 四角在圆外，必须是全零。
        for (x, y) in [(0, 0), (d - 1, 0), (0, d - 1), (d - 1, d - 1)] {
            assert_eq!(alpha_at(&buf, d, x, y), 0, "角 ({x},{y}) 没裁掉");
        }
        // 向外单调变暗。
        let mut prev = centre;
        for r in 1..=mid {
            let a = alpha_at(&buf, d, mid + r, mid);
            assert!(a <= prev);
            prev = a;
        }
    }

    #[test]
    fn halo_is_clear_in_the_middle() {
        let (w, h, buf) = halo(40, 20, 12, 4, 0.8);
        assert_eq!((w, h), (64, 44));
        // 矩形本体那一片**不能**被光晕盖住 —— 它盖住的是按钮的填充。
        for y in 14..30 {
            for x in 14..50 {
                assert_eq!(alpha_at(&buf, w, x, y), 0, "({x},{y}) 盖到了本体");
            }
        }
        // 边框正外侧必须亮着：`spread=12` 的第一圈画在偏移 11 那一条上。
        assert!(alpha_at(&buf, w, 11, 22) > 0);
    }

    #[test]
    fn scanlines_are_every_other_row() {
        let (w, _, buf) = scanlines(8, 9, 0.2, 3);
        for y in 0..9u32 {
            let a = alpha_at(&buf, w, 0, y);
            if y % 3 == 0 {
                assert!(a > 0, "第 {y} 行该有线");
            } else {
                assert_eq!(a, 0, "第 {y} 行不该有线");
            }
        }
    }

    #[test]
    fn breathe_never_goes_out() {
        // 一整圈都不许低于下限：这是"亮着的灯在呼吸"，不是"灯在闪"。
        let mut lo = 1.0f32;
        let mut hi = 0.0f32;
        for ms in (0..BREATHE_MS).step_by(7) {
            let b = breathe(ms);
            lo = lo.min(b);
            hi = hi.max(b);
        }
        assert!(lo >= 0.549, "呼吸压到了 {lo}");
        assert!(hi <= 1.0);
        assert!(hi - lo > 0.35, "起伏太小，看不出来：{lo}..{hi}");
        // 周期：一个呼吸长度之后回到原处。
        assert!((breathe(0) - breathe(BREATHE_MS)).abs() < 1e-6);
    }

    #[test]
    fn hairline_draws_four_edges_without_double_corners() {
        let mut out = Vec::new();
        hairline(&mut out, 10.0, 10.0, 100.0, 50.0, 0.55);
        assert_eq!(out.len(), 4);
        // 四条边的面积加起来不能超过整圈周长 —— 角上重复画会让角更亮。
        let total: f32 = out
            .iter()
            .map(|d| match d {
                Draw::Rect { w, h, .. } => w * h,
                _ => 0.0,
            })
            .sum();
        let perimeter = 2.0 * 100.0 + 2.0 * 50.0 - 4.0;
        assert!((total - perimeter).abs() < 0.01, "围长 {total} != {perimeter}");
    }
}
