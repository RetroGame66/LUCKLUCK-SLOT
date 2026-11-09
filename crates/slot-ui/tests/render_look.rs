//! Renders the shelf and menu screens, once per look, to a PNG — so the twelve colourings can be
//! compared by eye rather than only by assertion.
//!
//! The faces are the real ones (`menu::button`, `sticker_face`, `cart_face`, `draw_top_band`), the
//! layout is the app's own numbers, and the compositing is the same source-over blend the sprite
//! pass does. What comes out is what the device draws, at the device's own 720×480, without a
//! device — which is the only way to look at a palette change when the handheld is not to hand.
//!
//! **Opt-in.** It does nothing unless `LOOK_PNG_DIR` names an output directory:
//!
//! ```text
//! LOOK_PNG_DIR=F:/…/_looks LOOK_ROWS=shelf \
//!   SLOT_TEST_PKG=slot-ui python _tools/run_host_test.py --test render_look -- --nocapture
//! ```
//!
//! `LOOK_ROWS` picks the screens (comma separated, default `shelf,menu`), and `LOOK_ONLY` walks
//! only the looks named there — which is what keeps a re-run cheap while tuning one colouring.
//!
//! ⚠️ **The card's own interface face has to be installed or the Chinese type is blank**: the
//! binary reads `System/fonts/Plix-Bold.otf` off the card and falls back to an embedded Latin
//! `label.ttf` that carries no hanzi at all. `LOOK_FONT` points at the card's file; without it the
//! Chinese labels come out empty and only the Latin ones (GREEN, AMBER, …) are drawn.

use std::path::{Path, PathBuf};

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};
use slot_power::{Battery, Charge};
use slot_store::{Cart, System};
use slot_ui::palette::{look_name, set_look, set_mode, Mode, LOOK_COUNT};
use slot_ui::{
    cart_face, cart_placeholder, clock_face, desktop_rgba, draw_backdrop,
    draw_edge_glow, draw_empty_slot, draw_status, draw_sticker_at, draw_top_band, icon_face,
    letters, menu, settings, shelf_title_face, shortcut_hint_face,
    shortcut_row_face, size_for, sticker_face, word_face, CartFace, CartSize, Icon, Letters, Printed,
    Shelf,
    STICKER_H, STICKER_W, UndoFace, HUD_ICON_PX,
};
use slot_store::{DOZE_MIN_CHOICES, FF_SPEED_CHOICES, REWIND_SPEED_CHOICES};

/// Both a `CartFace` and an `UndoFace` are `{rgba, w, h}`; this is what lets one texture table
/// hold either.
trait Face {
    fn parts(self) -> (Vec<u8>, u32, u32);
}

impl Face for CartFace {
    fn parts(self) -> (Vec<u8>, u32, u32) {
        (self.rgba, self.w, self.h)
    }
}

impl Face for UndoFace {
    fn parts(self) -> (Vec<u8>, u32, u32) {
        (self.rgba, self.w, self.h)
    }
}

/// One texture the list can name, with the handle it is named by.
struct Tex {
    id: TexId,
    rgba: Vec<u8>,
    w: u32,
    h: u32,
}

/// A screen as a draw list and the textures that list names.
struct Screen {
    list: Vec<Draw>,
    texes: Vec<Tex>,
}

impl Screen {
    fn new() -> Self {
        Screen {
            list: Vec::new(),
            texes: Vec::new(),
        }
    }

    /// Take a face into the table and hand back the handle it is addressed by. Nothing is
    /// uploaded, so the id is just the slot — the substitution `render_menu` makes too.
    fn mint(&mut self, f: impl Face) -> TexId {
        let (rgba, w, h) = f.parts();
        let id = TexId::from_raw(self.texes.len());
        self.texes.push(Tex { id, rgba, w, h });
        id
    }

    fn png(&self, path: &Path) {
        let (w, h) = (OUT_W as usize, OUT_H as usize);
        let mut px = vec![0u8; w * h * 4];
        // Opaque to begin with, which is what the panel starts as; the backdrop's own ground is
        // the first thing laid over it.
        for p in px.chunks_mut(4) {
            p.copy_from_slice(&[0, 0, 0, 255]);
        }

        for d in &self.list {
            match *d {
                Draw::Rect {
                    x,
                    y,
                    w: rw,
                    h: rh,
                    colour,
                } => {
                    for yy in (y.max(0.0) as usize)..((y + rh) as usize).min(h) {
                        for xx in (x.max(0.0) as usize)..((x + rw) as usize).min(w) {
                            let at = (yy * w + xx) * 4;
                            blend(&mut px[at..at + 4], colour);
                        }
                    }
                }
                Draw::Tex {
                    x,
                    y,
                    w: tw,
                    h: th,
                    tex,
                    alpha,
                } => {
                    let Some(t) = self.texes.iter().find(|t| t.id == tex) else {
                        continue;
                    };
                    if t.rgba.is_empty() || tw <= 0.0 || th <= 0.0 {
                        continue;
                    }
                    // ★ **按屏幕像素走，不要按 quad 像素走**（2026-10-23 23:5x 修）。
                    //   上一版是 `dx = x as usize + col`，而 Rust 里 **负数 f32 转 usize 会饱和到 0**
                    //   ⇒ 侧位卡的 quad 在 `x = -120` 时，前半张被当成 0 起点，**整张贴图被压进
                    //   0..240**，而不是「裁掉左边一半、只留右半张」。于是效果图上左卡的标签
                    //   从 37 画起（正确应从 120 之后才可见），右卡却裁得对 —— 左右不对称，
                    //   看着像 `pitch` 没生效。**几何一直是对的，错的是这个离屏合成器。**
                    //   现在反过来：屏幕像素 → quad 内归一化 → 贴图采样点，正负坐标都成立。
                    let x0 = (x.max(0.0) as usize).min(w);
                    let x1 = ((x + tw).max(0.0) as usize).min(w);
                    let y0 = (y.max(0.0) as usize).min(h);
                    let y1 = ((y + th).max(0.0) as usize).min(h);
                    for dy in y0..y1 {
                        let v = (dy as f32 + 0.5 - y) / th;
                        let sy = (v * t.h as f32) as usize;
                        if sy >= t.h as usize {
                            continue;
                        }
                        for dx in x0..x1 {
                            let u = (dx as f32 + 0.5 - x) / tw;
                            let sx = (u * t.w as f32) as usize;
                            if sx >= t.w as usize {
                                continue;
                            }
                            let s = (sy * t.w as usize + sx) * 4;
                            if s + 3 >= t.rgba.len() {
                                continue;
                            }
                            let a = t.rgba[s + 3] as f32 / 255.0 * alpha;
                            if a > 0.0 {
                                let at = (dy * w + dx) * 4;
                                let c = [
                                    t.rgba[s] as f32 / 255.0,
                                    t.rgba[s + 1] as f32 / 255.0,
                                    t.rgba[s + 2] as f32 / 255.0,
                                    a,
                                ];
                                blend(&mut px[at..at + 4], c);
                            }
                        }
                    }
                }
                // 转过的面。真渲染器里这走的是同一个 sprite program，只是 `u_turn` 给的是
                // (cos, sin)；这里按同一套公式反解采样点 —— 先把目标像素绕中心拧回去，再
                // **双线性采样**。最近邻在这儿会把标签上的字拧成锯齿，而字正是要看的。
                Draw::Turned {
                    x,
                    y,
                    w: tw,
                    h: th,
                    tex,
                    alpha,
                    turn,
                } => {
                    let Some(t) = self.texes.iter().find(|t| t.id == tex) else {
                        continue;
                    };
                    if t.rgba.is_empty() || tw <= 0.0 || th <= 0.0 {
                        continue;
                    }
                    let (cx, cy) = (x + tw / 2.0, y + th / 2.0);
                    let (co, si) = (turn.cos(), turn.sin());
                    let x0 = (x.max(0.0) as usize).min(w);
                    let x1 = ((x + tw).max(0.0) as usize).min(w);
                    let y0 = (y.max(0.0) as usize).min(h);
                    let y1 = ((y + th).max(0.0) as usize).min(h);
                    for dy in y0..y1 {
                        for dx in x0..x1 {
                            // 目标像素中心相对卡带中心，**按 -turn 拧回**，得到源图坐标。
                            let lx = dx as f32 + 0.5 - cx;
                            let ly = dy as f32 + 0.5 - cy;
                            let u = (lx * co + ly * si + tw / 2.0) / tw;
                            let v = (-lx * si + ly * co + th / 2.0) / th;
                            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                                continue;
                            }
                            let c = sample_bilinear(&t.rgba, t.w, t.h, u, v, alpha);
                            if c[3] > 0.0 {
                                let at = (dy * w + dx) * 4;
                                blend(&mut px[at..at + 4], c);
                            }
                        }
                    }
                }
                // 绕**竖轴**偏转的面（`Draw::Yaw`）。真渲染器里这走 `SPRITE_VERT` 的
                // `u_yaw` 分支；这里按**同一套公式反解**采样点：
                //
                //   正向：`out.x = cx + lx·cosθ / d`、`out.y = (y+h) + (ly − h/2) / d`，
                //         其中 `d = 1 + (lx / (w/2))·sinθ·YAW_DEPTH`。
                //   反解：`d` 只跟 `lx` 有关 ⇒ 先从 `out.x` 解出 `lx`，再由 `d` 得 `ly`。
                //
                // ⚠️ `YAW_DEPTH` 必须从 `slot_gfx` 取，**不能在这里写一个字面量** ——
                //    两边一旦不一致，离机图就是"看着对了但上机不是这个形状"。
                Draw::Yaw {
                    x,
                    y,
                    w: tw,
                    h: th,
                    tex,
                    alpha,
                    yaw,
                } => {
                    let Some(t) = self.texes.iter().find(|t| t.id == tex) else {
                        continue;
                    };
                    if t.rgba.is_empty() || tw <= 0.0 || th <= 0.0 {
                        continue;
                    }
                    let hw = tw / 2.0;
                    let (cx, base) = (x + hw, y + th);
                    // `-` 号与 `SPRITE_VERT` 一致：`yaw > 0` = 右边朝观察者。
                    let (co, si) = (yaw.cos(), -yaw.sin() * slot_gfx::YAW_DEPTH);
                    // 包围盒：四个角里最靠外的那些。近边最高，所以上界按它给。
                    let corner = |q: f32, py: f32| {
                        let d = 1.0 + q * si;
                        (cx + q * hw * co / d, base + py / d)
                    };
                    let (mut bx0, mut bx1, mut by0, mut by1) =
                        (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
                    for &q in &[-1.0f32, 1.0] {
                        for &py in &[-th, 0.0f32] {
                            let (px_, py_) = corner(q, py);
                            bx0 = bx0.min(px_);
                            bx1 = bx1.max(px_);
                            by0 = by0.min(py_);
                            by1 = by1.max(py_);
                        }
                    }
                    let x0 = (bx0.floor().max(0.0) as usize).min(w);
                    let x1 = ((bx1.ceil()).max(0.0) as usize).min(w);
                    let y0 = (by0.floor().max(0.0) as usize).min(h);
                    let y1 = ((by1.ceil()).max(0.0) as usize).min(h);
                    for dy in y0..y1 {
                        for dx in x0..x1 {
                            let vx = dx as f32 + 0.5 - cx;
                            let vy = dy as f32 + 0.5 - base;
                            // vx = q·hw·cos / (1 + q·si)  ⇒  q = vx / (hw·cos + vx·si)
                            let den = hw * co + vx * si;
                            if den <= 0.0 {
                                continue;
                            }
                            let q = vx / den;
                            if !(-1.0..=1.0).contains(&q) {
                                continue;
                            }
                            // 正向的 `out.y = base + (v − 1)·th / d` 反解：
                            //   v = 1 + vy·d / th。
                            let d = 1.0 + q * si;
                            let v = 1.0 + vy * d / th;
                            if !(0.0..1.0).contains(&v) {
                                continue;
                            }
                            let u = (q + 1.0) / 2.0;
                            let c = sample_bilinear(&t.rgba, t.w, t.h, u, v, alpha);
                            if c[3] > 0.0 {
                                let at = (dy * w + dx) * 4;
                                blend(&mut px[at..at + 4], c);
                            }
                        }
                    }
                }
                // Neither row rendered here emits `Game`, `Shot` or `Glow`: the first two are the
                // emulator's own passes and the third wants the overlay.
                _ => {}
            }
        }

        let f = std::fs::File::create(path).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&px).unwrap();
    }
}

/// **透视上的卡**：近边远边**高度不同**的梯形。
///
/// ★★ 这是我前面几轮做错的那个地方，记下来：**横向按 `cosθ` 压窄做不出梯形** ——
///    那只会得到一个更窄的**矩形**（上下边平齐）。梯形只能来自**两个边的高度不同**，
///    而那要求**透视**：近的那条边离观察者近，所以更高。
///
/// 参数直接取自六六的示意图（他逐像素量出来就是这三个数）：
///   侧位卡宽 / 中间卡宽 = 0.62        （174 / 280）
///   近边高 / 中间卡高   = 0.733       （132 / 180）
///   远边高 / 中间卡高   = 0.528       （95  / 180）
/// 而且**上下边对称张开**、**侧位与中间共用同一条水平中轴**（示意图里中轴 225/226.5）。
///
/// 「近边」永远是**靠中间**的那条：左卡的右缘、右卡的左缘。
///
/// ⚠️ 真机实现：这需要**顶点着色器**给四个角不同的深度、让 `gl_Position.w` 做透视除法
/// （现在是 `vec4(.., 0.0, 1.0)`）。改动很小（多一个 vec4 的角深度），而且**卡面不需要重烤**
/// —— GPU 自己按透视校正插值采样。这里离屏是为了不改渲染层先看效果。
// 预览专用：一次离屏快照要的六个数（贴图、中心、尺寸、靠哪条近边、透明度），
// 每一个都是这一次快照自己的事实，收成结构体只是把同一串名字再写一遍。
#[allow(clippy::too_many_arguments)]
fn persp_card(
    texes: &mut Vec<Tex>,
    list: &mut Vec<Draw>,
    tex: TexId,
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    inner_left: bool,
    alpha: f32,
) {
    let Some(t) = texes.iter().find(|t| t.id == tex) else {
        return;
    };
    if t.rgba.is_empty() {
        return;
    }
    let (tw, th) = (t.w, t.h);
    let rgba = t.rgba.clone();

    let bx0 = (cx - w / 2.0 - 1.0).floor();
    let bw = (w + 2.0).ceil().max(1.0) as u32;
    // 近边最高，所以包围盒按近边高度给。
    let bh = (h * NEAR_K + 2.0).ceil().max(1.0) as u32;
    let by = (cy - bh as f32 / 2.0).floor();

    let mut buf = vec![0u8; (bw * bh * 4) as usize];
    for py in 0..bh {
        let dy = by + py as f32 + 0.5;
        for px_ in 0..bw {
            let dx = bx0 + px_ as f32 + 0.5;
            // t = 0 是**左**边沿，1 是右边沿（纹理横向坐标）。
            let t = (dx - (cx - w / 2.0)) / w;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            // u = 0 是**近边**、1 是远边。"近边靠中间" ⇒ 左卡近边在右、右卡近边在左。
            let u = if inner_left { t } else { 1.0 - t };
            // 半高在近/远之间线性过渡 —— 示意图的上边 −17 / 下边 +20 就是线性。
            // ★ 别漏 `* h`：`NEAR_K` 是**占卡高的比例**，漏乘就得到 0.37 **像素**的半高，
            //   整张卡被压成一条线（第一版就是这么消失的）。
            let hh = 0.5 * h * (NEAR_K * (1.0 - u) + FAR_K * u);
            let top = cy - hh;
            let v = (dy - top) / (2.0 * hh);
            if !(0.0..1.0).contains(&v) {
                continue;
            }
            let c = sample_bilinear(&rgba, tw, th, t, v, 1.0);
            if c[3] <= 0.0 {
                continue;
            }
            let at = ((py * bw + px_) * 4) as usize;
            buf[at] = (c[0] * 255.0) as u8;
            buf[at + 1] = (c[1] * 255.0) as u8;
            buf[at + 2] = (c[2] * 255.0) as u8;
            buf[at + 3] = (c[3] * 255.0) as u8;
        }
    }

    let id = TexId::from_raw(texes.len());
    texes.push(Tex { id, rgba: buf, w: bw, h: bh });
    list.push(Draw::Tex {
        x: bx0,
        y: by,
        w: bw as f32,
        h: bh as f32,
        tex: id,
        alpha,
    });
}

/// 侧位卡的梯形比例，**从六六示意图的实测值推出来的**（见 `persp_card` 的注释）。
///
/// 他的数字：中间 280×180；侧位宽 174、外缘高 95、内缘高 132。
/// 拆开看是**两件事**：
///   1. 侧位整体是中间的 **0.62 倍**（宽 174/280 = 0.62，平均高 113.5/180 = 0.63）
///      —— 这一条**已经由货架自己的 side scale 负责**（`size.side`），不能再乘一次。
///   2. 在这 0.62 之上，再上下各张 **±18.5px / 113.5 = ±16.3%** ⇒ 近 1.163、远 0.837，
///      比值 1.163/0.837 = **1.389**，与他的 132/95 完全一致。
///
/// ★ **我上一版把 0.62 又乘到了宽度上（缩了两次）** —— `WIDTH_K` 因此归 1.0。
const NEAR_K: f32 = 1.163;
const FAR_K: f32 = 0.837;
const WIDTH_K: f32 = 1.0;

/// 双线性采样，`u`/`v` 是 0..1 的归一化坐标。转过的面必须用它 —— 最近邻会把标签上的字
/// 拧成锯齿，而字正是要看的东西。
fn sample_bilinear(rgba: &[u8], tw: u32, th: u32, u: f32, v: f32, alpha: f32) -> [f32; 4] {
    let fx = (u * tw as f32 - 0.5).clamp(0.0, tw as f32 - 1.0);
    let fy = (v * th as f32 - 0.5).clamp(0.0, th as f32 - 1.0);
    let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(tw - 1), (y0 + 1).min(th - 1));
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let at = |x: u32, y: u32| ((y * tw + x) * 4) as usize;
    let mut out = [0.0f32; 4];
    for k in 0..4 {
        let a = rgba[at(x0, y0) + k] as f32;
        let b = rgba[at(x1, y0) + k] as f32;
        let c = rgba[at(x0, y1) + k] as f32;
        let d = rgba[at(x1, y1) + k] as f32;
        out[k] = (a * (1.0 - tx) + b * tx) * (1.0 - ty) + (c * (1.0 - tx) + d * tx) * ty;
    }
    out[3] = out[3] / 255.0 * alpha;
    [
        out[0] / 255.0,
        out[1] / 255.0,
        out[2] / 255.0,
        out[3],
    ]
}

/// Straight source-over on straight (non-premultiplied) alpha — what the sprite pass does.
fn blend(dst: &mut [u8], c: [f32; 4]) {
    let a = c[3].clamp(0.0, 1.0);
    for k in 0..3 {
        dst[k] = (c[k] * 255.0 * a + dst[k] as f32 * (1.0 - a)).round() as u8;
    }
    dst[3] = 255;
}

/// ★ 2026-10-23 23:3x: the machine is a parameter, not a constant. The portrait rule
/// (竖卡 `middle`/`side` 由**高**定量、三张全框零间距) can only be looked at on a Game Boy row, and
/// this used to be nailed to `Games/GBA/*.gba` — so every render of the shelf was the landscape
/// one and the portrait geometry had no picture at all, only assertions.
fn sample_carts(system: System) -> Vec<Cart> {
    let (dir, ext) = match system {
        System::Gb => ("GB", "gb"),
        System::Gbc => ("GBC", "gbc"),
        // ★ 2026-10-24：这三个机种的 `system()` 是从**目录名**读的（`scan::machine_of_path`），
        //   所以造的 rom 路径必须落在自己的目录里，否则 `cart_face` 会把它们当成 GBA。
        System::Sfc => ("SFC", "smc"),
        System::Fc => ("FC", "nes"),
        System::Neogeo => ("NEOGEO", "zip"),
        System::Arcade => ("ARCADE", "zip"),
        // ★ 2026-10-25：MD 与 PS1 的扩展名是**两份里各自特有**的那个 —— MD 用 `.md`
        //   （`.bin` 三家撞车，只认文件夹），PS1 用 `.cue`（光盘镜像的索引文件，`.bin`/`.iso`
        //   同样撞车）。这两个目录名就是 `scan::DIRS` 里的那两个，改一处要同时改另一处。
        System::Md => ("MD", "md"),
        System::Ps1 => ("PS1", "cue"),
        // ★ 2026-10-26：PCE 的 `.pce` 是三家里唯一能单独识别的（`.bin`/`.cue` 都撞车），
        //   目录名沿用 `scan::DIRS`。
        System::Pce => ("PCE", "pce"),
        System::Gba => ("GBA", "gba"),
    };
    ["口袋妖怪 绿宝石", "黄金太阳", "恶魔城 月之轮回", "火焰纹章", "银河战士 融合"]
        .iter()
        .map(|t| Cart {
            stem: t.to_string(),
            rom: PathBuf::from(format!("Games/{dir}/{t}.{ext}")),
            label: None,
            art: None,
            title: t.to_string(),
            code: String::new(),
            cgb: 0,
            initial: t.chars().next().unwrap_or('#'),
        })
        .collect()
}

/// The shelf screen: the case's bands, the row of carts, the letter index, the clock and the
/// battery — the same order `App::draw`'s `Phase::Shelf` arm lays them down.
fn shelf_screen(system: System) -> Screen {
    shelf_screen_idle(system, slot_ui::Idle::NONE)
}

/// ★ 2026-10-25：带**待机动效**的货架 —— 走的是真机同一条路
/// （`Shelf::draw_row_idle` + `palette` 的柔光），不是离机后处理。
fn shelf_screen_idle(system: System, idle: slot_ui::Idle) -> Screen {
    let mut s = Screen::new();
    paint_shelf_row(&mut s, system, idle);
    s
}

/// 把一整套货架（背景 + 卡带 + 桌面图 + 字母轮 + 状态条 + 标题）画进**已有的** `Screen`。
///
/// 抽出来只有一个用处：机种选择的动图要把两套货架的纹理装进同一张表 —— 帘子卷回去时
/// 露出的是新那一台，而它必须和老那一台共用同一个 `TexId` 空间，否则同一份 draw list
/// 里两套 id 会撞。静态出图那条路一个字没变（`shelf_screen_idle` 只是它的一个薄壳）。
fn paint_shelf_row(s: &mut Screen, system: System, idle: slot_ui::Idle) {
    let size = size_for(system);

    draw_backdrop(None, None, &mut s.list);
    // No `draw_empty_slot` here: with the desktop plate up, the real app skips the
    // programmatic bay (app.rs 6735-6741, the `show_desktop` branch) — the plate's own
    // slot wall is the slot. Drawn underneath, the old bay's thumb scoop showed through
    // the plate's open hole: last version's bar, ghosted into this one's screenshot.

    let carts = sample_carts(system);
    let faces: Vec<Option<TexId>> = carts.iter().map(|c| Some(s.mint(cart_face(c)))).collect();
        let placeholder = s.mint(cart_placeholder(size, system));
    let mut shelf = Shelf::new(carts);
    shelf.set_system(system);
        shelf.set_placeholder(placeholder);
    shelf.set_faces(faces);
    // The third of five, so both neighbours and the selected cart's own size are all on screen.
    shelf.index = 2;
    // ★ 2026-10-25：真机上这池柔光由 `app.rs` 的 `set_focus` 装进来，离机渲染器一直没装
    //   （静态图里看不到它）。出**待机动效**图时补上 —— 因为柔光呼吸是效果的一半；
    //   不带 idle 的静态图仍然不装，免得所有既有基线图都变一遍。
    if idle != slot_ui::Idle::NONE {
        let (pw, ph, buf) = slot_ui::pool(480, 20);
        let pool = s.mint(CartFace {
            rgba: buf,
            w: pw,
            h: ph,
        });
        shelf.set_focus(Some(pool));
    }
    let before = s.list.len();
    shelf.draw_row_idle(None, 0.0, 0.0, 1.0, idle, &mut s.list);
    if std::env::var("LOOK_RECTS").is_ok() {
        eprintln!(
            "shelf row ({system:?}): size w={} h={} pitch={} middle={} side={} foot={}",
            size.w, size.h, size.pitch, size.middle, size.side, size.foot_y()
        );
        for r in shelf.slot_rects(None, 0.0, 0.0) {
            eprintln!(
                "  cart {:>2} x={:8.2} y={:8.2} w={:7.2} h={:7.2} centre={:7.2}",
                r.cart, r.x, r.y, r.w, r.h, r.x + r.w / 2.0
            );
        }
        for d in &s.list[before..] {
            match d {
                Draw::Tex { x, y, w, h, .. } => {
                    eprintln!("  Tex  x={x:8.2} y={y:8.2} w={w:7.2} h={h:7.2}")
                }
                Draw::Rect { x, y, w, h, .. } => {
                    eprintln!("  Rect x={x:8.2} y={y:8.2} w={w:7.2} h={h:7.2}")
                }
                _ => {}
            }
        }
    }

    // ★ 2026-10-23（六六）：桌面图画在货架行之上（压卡带下缘），和 `App::draw` 的货架
    //   分支同一个次序。桌面图在时不画程序化的 top_band / edge_glow —— 它已自带顶条/底条
    //   的黑框与边缘光，两层叠在一起是双倍黑边（app.rs 6794 的同一个判据）。
    let desk = s.mint(CartFace {
        rgba: desktop_rgba(),
        w: OUT_W,
        h: OUT_H,
    });
    s.list.push(Draw::Tex {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        tex: desk,
        alpha: 1.0,
    });

    // The letter ring, every slot carrying a plausible count so no letter is faded out.
    let counts = [3usize; letters::N];
    let letter_faces: Vec<(TexId, u32, u32)> = letters::SLOTS
        .iter()
        .map(|ch| {
            let f = letters::letter_face(*ch);
            let (w, h) = (f.w, f.h);
            (s.mint(f), w, h)
        })
        .collect();
    let ridge = letters::ridge_face();
    let (rw, rh) = (ridge.w, ridge.h);
    let ridge = Some((s.mint(ridge), rw, rh));
    // ★ 2026-10-24：**胶囊要先铺**（真机 `app.rs` 是 `Draw::Tex` 在 `draw_strip` 之前）。
    //   渲染器原来漏了这一层，于是离机图上的字母直接落在底条上 —— 而真机是落在
    //   `deepest()` 那块药丸上。「亮色模式下字母轮不显示」这个 bug 在离机图里
    //   **根本看不见**（画面上字母清清楚楚），正是缺这一层造成的假绿。
    let pill = letters::pill_face();
    let (px, py, pw, ph) = letters::pill_quad();
    let pill_tex = s.mint(pill);
    s.list.push(Draw::Tex {
        x: px,
        y: py,
        w: pw,
        h: ph,
        tex: pill_tex,
        alpha: 1.0,
    });
    let mut ring = Letters::new();
    ring.snap_to('T');
    ring.draw_strip(&letter_faces, ridge, &counts, &mut s.list);

    // ★ 2026-10-23 23:4x：**标题的位置要照真机的 `App::shelf_title_y` 抄**。
    //   这里一直按 2026-10-22 之前的旧算法画在「卡带行与底条之间的缝」里 ——
    //   真机已改为**顶条居中**（`MOUTH_H / 2 - SHELF_TITLE_H / 2`），也就是名称背板里。
    //   因此旧值在竖卡上把标题压到了卡带下缘上（还撞上时钟与字母轮），而这张图就不是真机会出现的画面。
    //   两个数都是 `slot_ui` 的公开常量，不是重定义的数字。
    let title = shelf_title_face("口袋妖怪 绿宝石");
    let (tw, th) = (title.w, title.h);
    let tex = s.mint(title);
    s.list.push(Draw::Tex {
        x: (OUT_W as f32 - tw as f32) / 2.0,
        y: slot_ui::MOUTH_H / 2.0 - slot_ui::SHELF_TITLE_H as f32 / 2.0,
        w: tw as f32,
        h: th as f32,
        tex,
        alpha: 1.0,
    });

    status_row(s);
}

/// The clock at one end of the top band and the battery at the other. The app draws these on the
/// band whether or not there is a cart in front of it, so every shelf render carries them.
fn status_row(s: &mut Screen) {
    let clock = clock_face("14:52");
    let cw = clock.w;
    let clock_p = Printed::new(s.mint(clock), cw);
    let pct = word_face("64%");
    let pw = pct.w;
    let pct_p = Printed::new(s.mint(pct), pw);
    let badge = icon_face(
        Icon::of_mode(slot_ui::palette::mode()),
        HUD_ICON_PX,
        slot_ui::palette::ink(),
    );
    let badge = Some(s.mint(badge));
    draw_status(
        Some(Battery {
            percent: 64,
            charge: Charge::Discharging,
        }),
        pct_p,
        None,
        clock_p,
        badge,
        &mut s.list,
    );
}

/// The menu screen: the sticker, the buttons, the pinned foot line — `Phase::Menu`'s order.
/// ★ 2026-10-26（六六）：**机种选择面板**（长按 Y 打开的那一块）。
///
/// **压在真货架上画，不是空背景上** —— 这块屏是一层浮层，它的纱盖住了多少、格子落在
/// 桌面图上方还是下方，只有在有背景的时候才看得出来。所以先拿 `shelf_screen` 那一条，
/// 再把面板叠上去（`mint` 只是接着往同一张表里追加，不冲突）。
///
/// 十张标识从卡上读（`LOOK_DEPLOY/System/Carts/SLOT_SYS_*.png`），与真机同一条路。
/// 文件名表在这里又列了一遍，而不是去问 `slot` 的 `root::sys_badge_file`：这个测试只依赖
/// `slot-ui`，不该为十个字符串把整个二进制拉进来。列错的后果是那一格空着 —— 图上立刻
/// 看得出来，所以它自己就是那份表的检查。
fn sys_picker_faces(
    s: &mut Screen,
) -> (slot_ui::sys_picker::Faces, Vec<slot_ui::sys_picker::Cell>) {
    use slot_ui::sys_picker;
    use slot_ui::{arrows_hint_face, hint_face, lang, menu_face_at};

    const FILES: [&str; sys_picker::COUNT] = [
        "SLOT_SYS_GBA.png",
        "SLOT_SYS_GB.png",
        "SLOT_SYS_GBC.png",
        "SLOT_SYS_SFC.png",
        "SLOT_SYS_FC.png",
        "SLOT_SYS_NEOGEO.png",
        "SLOT_SYS_ARCADE.png",
        "SLOT_SYS_MD.png",
        "SLOT_SYS_PS1.png",
        "SLOT_SYS_PCE.png",
    ];

    let icon_px = sys_picker::icon_px();
    let carts = std::env::var("LOOK_DEPLOY")
        .ok()
        .map(|d| PathBuf::from(d).join("System/Carts"));
    // 第 6 格（NEOGEO）演一次「这张卡上没有它的游戏」：十格里留一格暗的，才看得出
    // 暗态与亮态差多少 —— 否则"没卡"这一态永远没被画出来过。
    //
    // 每个机种烘两份（卡上那张白图 + 染成墨色的那份），和真机同一条路：白标识压在亮色
    // 主题的浅塑料格子上等于没有标识。图上两种主题各出一次，两份都被用到。
    let ink = slot_ui::palette::ink();
    let cells: Vec<sys_picker::Cell> = FILES
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let icon = carts
                .as_ref()
                .and_then(|d| slot_ui::art_stretch(&d.join(file), icon_px, icon_px))
                .map(|rgba| {
                    let dark = (
                        s.mint(CartFace {
                            rgba: rgba.clone(),
                            w: icon_px,
                            h: icon_px,
                        }),
                        icon_px,
                        icon_px,
                    );
                    let lit = slot_ui::sys_picker_ink_icon(&rgba, ink);
                    let light = (
                        s.mint(CartFace {
                            rgba: lit,
                            w: icon_px,
                            h: icon_px,
                        }),
                        icon_px,
                        icon_px,
                    );
                    (dark, light)
                });
            sys_picker::Cell {
                icon,
                empty: i == 5,
            }
        })
        .collect();

    let face = slot_ui::sys_picker_button(false);
    let (iw, ih) = (face.w, face.h);
    let idle = (s.mint(face), iw, ih);
    let face = slot_ui::sys_picker_button(true);
    let (hw, hh) = (face.w, face.h);
    let hot = (s.mint(face), hw, hh);
    let (gw, gh, gbuf) = slot_ui::sys_picker_glow();
    let glow = (
        s.mint(CartFace {
            rgba: gbuf,
            w: gw,
            h: gh,
        }),
        gw,
        gh,
    );
    let face = menu_face_at(lang::SYS_PICKER_TITLE, 36.0, 46, 18);
    let (tw, th) = (face.w, face.h);
    let title = (s.mint(face), tw, th);
    let foot: Vec<(TexId, u32)> = [
        hint_face("B", lang::CORE_CANCEL),
        arrows_hint_face(lang::SYS_PICKER_MOVE),
        hint_face("A", lang::CORE_CHOOSE),
    ]
    .into_iter()
    .map(|f| {
        let w = f.w;
        (s.mint(f), w)
    })
    .collect();
    let faces = sys_picker::Faces {
        idle,
        hot,
        glow,
        title: Some(title),
        foot,
    };
    (faces, cells)
}

/// 静态一张：**先落帘子，再浮十格**。
///
/// 顺序必须是这样 —— 这块屏不是"盖在货架上的浮层"，而是"帘子停下来时帘子上多了点东西"。
/// 上一版把它画成浮层，六六当场否掉了。`shutter_at(SWAP_AT_S)` 就是满闸那一帧：帘子盖满
/// 整屏、顶栏已经跟着走到屏外，正是真机上用户看到的那一帧。
fn sys_picker_screen() -> Screen {
    use slot_ui::sys_picker;

    let mut s = shelf_screen(System::Gbc);
    let (faces, cells) = sys_picker_faces(&mut s);
    slot_ui::draw_shutter(
        slot_ui::shutter_at(slot_ui::SHUTTER_SWAP_AT_S),
        None,
        None,
        &mut s.list,
    );
    let view = sys_picker::View {
        // 手里是 GBC（第 3 格）：既不在第一格也不在角上，光标落在哪一格一眼能验。
        // 四角那点几何由 `sys_picker` 的单元测试管（放大 120 之后光晕会不会出屏）。
        cursor: 2,
        prev: 2,
        moved: 1.0,
        alpha: 1.0,
        // 出图取呼吸的上限（`light::breathe` 在 0.55..1.0 之间来回）。静态图只能取一个值，
        // 取上限是因为要看的是"最亮那一档会不会糊"。
        breathe: 1.0,
        shake: 0.0,
        cells: &cells,
    };
    sys_picker::draw(&faces, &view, &mut s.list);
    s
}

/// ★ 2026-10-26（六六）：「我在外面，你做示意图我看看」——
/// **整条时序的动图**，只在 `SYSPICK_OUT` 指了目录时跑。
///
/// ```text
/// SYSPICK_OUT=F:/…/_syspick-anim LOOK_FONT=…/Plix-Bold.otf LOOK_DEPLOY=…/deploy \
///   SLOT_TEST_PKG=slot-ui python _tools/run_host_test.py --test render_look -- --nocapture
/// ```
///
/// 走的是真东西：真帘子（`draw_shutter` + `shutter_at`）、真十格（`sys_picker::draw`）、
/// 真两套货架（`paint_shelf_row`）。**唯一自拟的是时间轴**：真机上"满闸"停多久取决于人，
/// 动图里给 0.2 s 一个观察窗口。
///
/// 六段，30 fps：
///
/// | 段 | 秒 | 内容 |
/// |---|---|---|
/// | 落下 | 0.00–1.00 | 帘子从头顶下来盖满（顶栏跟着走，CHANGE 亮着） |
/// | 浮现 | 1.00–1.16 | 十格淡入 |
/// | 停 | 1.16–1.46 | 让眼睛看清十格 |
/// | 走格 | 1.46–1.82 | 光标 GBC → SFC → FC（右移两格） |
/// | 停 | 1.82–2.12 | 指着 FC |
/// | 确认 | — | A 按下：货架在帘子后面换成 FC |
/// | 回卷 | 2.12–2.72 | 帘子卷起，露出的是**新货架** |
/// | 停 | 2.72–3.39 | 卷完的画面 |
///
/// 不做 B 取消那一路：它在画面上少的是"货架变了"这一步，而这一步恰好是这张图要说的事。
#[test]
fn render_syspick_anim() {
    use slot_ui::sys_picker;

    let Ok(dir) = std::env::var("SYSPICK_OUT") else {
        eprintln!("SYSPICK_OUT not set: no sys-picker animation rendered");
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();

    if let Ok(p) = std::env::var("LOOK_FONT") {
        if let Ok(b) = std::fs::read(&p) {
            slot_ui::text::set_font(b);
        }
    }
    if let Ok(p) = std::env::var("LOOK_DEPLOY") {
        slot_ui::install_cart_art(slot_ui::load_cart_art(Path::new(&p)));
    }

    // 一张表、两套货架：卷起时要露出的那台和原来那台得共用一个 `TexId` 空间。
    //
    // **从 GBC 换到 GBA**：一个是竖版小卡、一个是横版大卡，卷起来一眼能看出"货架真的换了"；
    // 换两个形状相近的机种（SFC/FC 都是横版）反而看不出来。
    let mut base = Screen::new();
    paint_shelf_row(&mut base, System::Gbc, slot_ui::Idle::NONE);
    let old_len = base.list.len();
    paint_shelf_row(&mut base, System::Gba, slot_ui::Idle::NONE);
    let row_old: Vec<Draw> = base.list[..old_len].to_vec();
    let row_new: Vec<Draw> = base.list[old_len..].to_vec();
    let (faces, cells) = sys_picker_faces(&mut base);

    // CHANGE 的字与它的柔光：和前端同一套（卡上 `slot_change.png` + 洋红换主题亮色）。
    let bright = slot_ui::ridge_ink();
    let carts = std::env::var("LOOK_DEPLOY")
        .ok()
        .map(|d| PathBuf::from(d).join("System/Carts"));
    let change = carts
        .as_ref()
        .and_then(|d| slot_ui::decode_rgba(&d.join("slot_change.png")))
        .map(|(mut rgba, w, h)| {
            slot_ui::fill_logo_highlight(&mut rgba, w, h, bright);
            let glow = slot_ui::glow_face(&rgba, w, h, bright).map(|(g, gw, gh)| {
                (
                    base.mint(CartFace {
                        rgba: g,
                        w: gw,
                        h: gh,
                    }),
                    gw,
                    gh,
                )
            });
            let glyph = (base.mint(CartFace { rgba, w, h }), w, h);
            (Some(glyph), glow)
        })
        .unwrap_or((None, None));

    // 帧的时间轴，单位秒。`at` 给帘子，`lift` 是"已经按了 A 之后过了多久"。
    const DT: f32 = 1.0 / 30.0;
    const DOWN_S: f32 = slot_ui::SHUTTER_SWAP_AT_S;
    const UP_S: f32 = slot_ui::SHUTTER_UP_S;
    const REVEAL_S: f32 = 0.16;
    const MOVE_S: f32 = 0.18;
    // 各段起点。
    let t_reveal = DOWN_S;
    let t_walk = t_reveal + REVEAL_S + 0.30;
    let t_confirm = t_walk + 2.0 * MOVE_S + 0.30;
    let t_end = t_confirm + UP_S + 0.67;

    // 光标：GBC(2) → GB(1) → GBA(0)，左移两格，一格 180 ms。每项是
    // `(新格, 旧格, 这一刻开始走)`。
    let walk: [(usize, usize, f32); 3] = [
        (2, 2, 0.0),
        (1, 2, t_walk),
        (0, 1, t_walk + MOVE_S),
    ];

    let mut shell = Screen {
        list: Vec::new(),
        texes: std::mem::take(&mut base.texes),
    };
    let mut n = 0;
    let mut t = 0.0f32;
    while t < t_end {
        // 背景：确认之后换成 FC 那一行（但它被帘子盖着，只有卷起时才露出来）。
        let mut list = if t >= t_confirm {
            row_new.clone()
        } else {
            row_old.clone()
        };
        // 帘子。
        let shutter_t = if t >= t_confirm {
            // 从"满闸 + HOLD"那一刻接着走，和 `App::shutter` 同一个换算。
            slot_ui::SHUTTER_TOTAL_S - UP_S + (t - t_confirm).min(UP_S)
        } else {
            t.min(DOWN_S)
        };
        // 卷完之后帘子**整个不画** —— 真机上 `shelf_switch` 变 `None`，顶栏交回货架自己画。
        if t < t_confirm + UP_S {
            slot_ui::draw_shutter(
                slot_ui::shutter_at(shutter_t),
                change.0,
                change.1,
                &mut list,
            );
        }
        // 十格：浮现之后一直在，`alpha` 只由浮现那 160 ms 决定。
        let alpha = ((t - t_reveal) / REVEAL_S).clamp(0.0, 1.0);
        let (cursor, prev, since) = walk
            .iter()
            .rev()
            .find(|(_, _, at)| t >= *at - 1e-4)
            .map(|(c, p, at)| (*c, *p, *at))
            .unwrap_or((2, 2, 0.0));
        let view = sys_picker::View {
            cursor,
            prev,
            moved: ((t - since) / MOVE_S).clamp(0.0, 1.0),
            alpha,
            // 呼吸按 3400 ms 一个来回走，跟真机同一根曲线（`light::breathe`）。
            breathe: slot_ui::light_breathe((t * 1000.0) as u32),
            shake: 0.0,
            cells: &cells,
        };
        // 按 A 的那一刻十格就收了 —— 真机上 `settle_system_switch` 第一行是 `sys_picker = None`，
        // 帘子正盖着，用户看不见它消失；卷起来露出的直接是新货架。
        if t < t_confirm {
            sys_picker::draw(&faces, &view, &mut list);
        }

        shell.list = list;
        let path = dir.join(format!("f{n:04}.png"));
        shell.png(&path);
        n += 1;
        t += DT;
    }
    eprintln!("{n} frames in {}", dir.display());

}

fn menu_screen() -> Screen {
    let mut s = Screen::new();
    draw_backdrop(None, None, &mut s.list);

    // ★ 2026-10-29（六六）：牌子上的字全删了，这张面不再带任何参数。
    //   图没换成本面板的尺寸时 `sticker_face()` 是 `None` —— 出图照样跑，只是没有牌子。
    let sticker = sticker_face().map(|label| s.mint(label));
    draw_sticker_at(sticker, menu::PAD, &mut s.list);

    let empty = s.mint(menu::empty());
    let caret = menu::first_slot();
    for (i, item) in menu::Item::GRID.iter().enumerate() {
        let (x, y) = menu::slot_at(i);
        let tex = match item {
            Some(item) => {
                let b = menu::button(*item);
                let f = if i == caret { b.chosen } else { b.idle };
                s.mint(f)
            }
            None => empty,
        };
        // ★ 2026-10-21: the slot's own width — the player's button spans the row.
        s.list.push(Draw::Tex {
            x,
            y,
            w: menu::slot_w(i),
            h: menu::BUTTON_H,
            tex,
            alpha: 1.0,
        });
    }

    let hint = shortcut_hint_face(menu::HINT);
    let (hw, hh) = (hint.w, hint.h);
    let tex = s.mint(hint);
    draw_hint_bar(OUT_H as f32 - hh as f32, &mut s.list);
    s.list.push(Draw::Tex {
        x: (OUT_W as f32 - hw as f32) / 2.0,
        y: OUT_H as f32 - hh as f32,
        w: hw as f32,
        h: hh as f32,
        tex,
        alpha: 1.0,
    });

    s
}

/// The settings list: the **ten** rows in `Row::ALL` order, in a double column, the chosen one
/// lit, under the same foot bar. `Phase::Settings`'s order — no sticker, which is the whole
/// reason this screen is not the menu. Ten rows fill five grid rows and the window holds five,
/// so every one of them fits and nothing scrolls on this screen today.
fn settings_screen() -> Screen {
    use slot_ui::lang;
    let mut s = Screen::new();
    draw_backdrop(None, None, &mut s.list);

    let entries: [(settings::Row, String); 10] = [
        // ★ 2026-10-28 第二轮（六六）：日期与时间去掉「校准」描述项 —— 和刷新一样值空，
        //   `value_face` 只画居中的名字，`A` 开门（`opens`）。
        (settings::Row::DateTime, String::new()),
        // ★ 2026-10-28 六六：「选项去掉仅留描述名」—— 刷新这一行没有"当前值"了，`A` 就是做；
        //   值位给空串，`value_face` 就只画居中的名字（值空是这屏唯一一种"没值"的画法）。
        (settings::Row::Refresh, String::new()),
        (settings::Row::Sound, lang::switch_word(false).to_string()),
        (settings::Row::Sleep, lang::minutes(DOZE_MIN_CHOICES[2])),
        (settings::Row::FastForward, lang::speed(FF_SPEED_CHOICES[2])),
        (settings::Row::Rewind, lang::speed(REWIND_SPEED_CHOICES[1])),
        (settings::Row::InsertResume, lang::switch_word(true).to_string()),
        (settings::Row::EjectSave, lang::switch_word(false).to_string()),
        // ★ 2026-10-28：开机画面退成普通列表（`A` 一步一换、换到就写），不再是那套"左右浏览、
        //   A 替换"的走法。一张图名代表卡上 `Bootlogos/` 里那一张。
        (settings::Row::Boot, "66-mods".to_string()),
        (settings::Row::Wifi, lang::SETTINGS_WIFI_UNSET.to_string()),
    ];
    // ★ 2026-10-28：**刷新行**在手里 —— 它是这屏唯一"值空、名字居中、`A` 执行"的行，也是
    //   唯一穿 `HINT_ACT`（「上下左右选择 A 执行 B 返回」）底行的行，画它一张能同时验这两件
    //   新事。十行五格、双列，窗口五格，全收无滚动；`scroll_for` 为 0、`visible` 全保留。
    // ★ 2026-10-28 第三轮：第三行与第一行对调后，刷新行升到第一行右列（index 1）。
    let chosen = 1usize;
    let scroll = settings::scroll_for(chosen);
    for (i, (row, value)) in entries.iter().enumerate() {
        if !settings::visible(i, scroll) {
            continue;
        }
        let f = settings::value_face(*row, value);
        let face = if i == chosen { f.chosen } else { f.idle };
        let (w, h) = (face.w, face.h);
        let tex = s.mint(face);
        let (x, y) = settings::row_at(i, scroll);
        s.list.push(Draw::Tex {
            x,
            y,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
    }

    // ★ 2026-10-17: the line the row in hand actually asks for, not a fixed one — the refresh row
    // is the first on this screen whose foot line is neither "left and right to change" nor
    // "A to open", so a hard-coded line would render a picture of a screen that cannot exist.
    // ★ 2026-10-28: `A` is the only key that does anything but move the caret now, so the lines
    // collapsed to three (change / open / act); the row in hand here is the act one.
    let hint = shortcut_hint_face(settings::Row::ALL[chosen].hint());
    let (hw, hh) = (hint.w, hint.h);
    let tex = s.mint(hint);
    draw_hint_bar(OUT_H as f32 - hh as f32, &mut s.list);
    s.list.push(Draw::Tex {
        x: (OUT_W as f32 - hw as f32) / 2.0,
        y: OUT_H as f32 - hh as f32,
        w: hw as f32,
        h: hh as f32,
        tex,
        alpha: 1.0,
    });

    s
}

/// The key card: the first rows of `shortcuts::ROWS`, with a section head, then the pinned foot
/// line. `Phase::Shortcuts` at scroll 0, which is what a user sees on opening it.
fn card_screen() -> Screen {
    let mut s = Screen::new();
    draw_backdrop(None, None, &mut s.list);

    let hint = shortcut_hint_face(slot_ui::SHORTCUT_HINT);
    let hint_h = hint.h as f32;
    let top = 26.0;
    let mut y = top;
    // The card is 52 rows tall and 480 px shows about six, so draw until the band starts.
    for row in slot_ui::SHORTCUT_ROWS.iter() {
        if y > OUT_H as f32 - hint_h {
            break;
        }
        let face = shortcut_row_face(*row);
        let (w, h) = (face.w, face.h);
        let tex = s.mint(face);
        s.list.push(Draw::Tex {
            x: (OUT_W as f32 - w as f32) / 2.0,
            y,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
        y += h as f32;
    }

    let (hw, hh) = (hint.w, hint.h);
    let tex = s.mint(hint);
    draw_hint_bar(OUT_H as f32 - hh as f32, &mut s.list);
    s.list.push(Draw::Tex {
        x: (OUT_W as f32 - hw as f32) / 2.0,
        y: OUT_H as f32 - hh as f32,
        w: hw as f32,
        h: hh as f32,
        tex,
        alpha: 1.0,
    });

    s
}

/// The desktop plate on its own, nothing over it.
///
/// ★ 2026-10-23 23:5x: the shelf renders cannot answer a question about the plate's own
/// geometry — a 360-wide main cart sits exactly over the 360-wide slot, so the slot's mouth,
/// its walls and its two round corners are all hidden by the thing the slot is for. This row
/// draws the plate and the status row only, which is what the frame's shape has to be read from.
fn desktop_screen() -> Screen {
    let mut s = Screen::new();
    draw_backdrop(None, None, &mut s.list);
    let desk = s.mint(CartFace {
        rgba: desktop_rgba(),
        w: OUT_W,
        h: OUT_H,
    });
    s.list.push(Draw::Tex {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        tex: desk,
        alpha: 1.0,
    });
    status_row(&mut s);
    s
}

/// The shelf with no games on the card. A card can be empty — a fresh one, or one whose `Games/`
/// has not been filled yet — and what the screen does then is the state a design most often
/// forgets.
fn shelf_empty_screen() -> Screen {
    let mut s = Screen::new();
    draw_backdrop(None, None, &mut s.list);
    draw_empty_slot(&mut s.list);
    draw_top_band(&mut s.list);
    draw_edge_glow(&mut s.list);
    status_row(&mut s);
    s
}

/// The sticker on its own, at the size it is drawn. It is the artwork itself now — no type is
/// set on it — so this screen exists to check the picture and its transparent edge, not glyphs.
fn sticker_screen() -> Screen {
    let mut s = Screen::new();
    draw_backdrop(None, None, &mut s.list);
    let Some(label) = sticker_face() else {
        // No artwork at this panel's size: an empty sheet, and say so rather than panic.
        eprintln!("sticker_screen: no artwork at {}x{}", STICKER_W, STICKER_H);
        return s;
    };
    let (w, h) = (label.w, label.h);
    let tex = s.mint(label);
    // At 3× so the 8.5 px body type is legible when the sheet is viewed, and centred like a
    // magnifier would be — nearest-neighbour, so what is shown is the pixels themselves.
    let (dw, dh) = (w as f32 * 1.0, h as f32 * 1.0);
    s.list.push(Draw::Tex {
        x: (OUT_W as f32 - dw) / 2.0,
        y: (OUT_H as f32 - dh) / 2.0,
        w: dw,
        h: dh,
        tex,
        alpha: 1.0,
    });
    s
}

/// The bar the pinned foot line is set on. `app.rs`'s own is `pub(crate)` there, so it is
/// restated here with the three constants it draws with.
fn draw_hint_bar(top: f32, out: &mut Vec<Draw>) {
    out.push(Draw::Rect {
        x: 0.0,
        y: top,
        w: OUT_W as f32,
        h: (OUT_H as f32 - top).max(0.0),
        colour: [0.055, 0.05, 0.045, 1.0],
    });
    for i in 0..8u32 {
        let a = 0.13 * (i as f32 + 1.0) / 8.0;
        let y = top - (8 - i) as f32 * 1.5;
        out.push(Draw::Rect {
            x: 0.0,
            y,
            w: OUT_W as f32,
            h: 1.5,
            colour: [0.86, 0.83, 0.77, a],
        });
    }
}

#[test]
fn render_look() {
    let Ok(dir) = std::env::var("LOOK_PNG_DIR") else {
        eprintln!("LOOK_PNG_DIR not set: nothing rendered");
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();

    // The card's own interface face. Without it the Chinese type is blank — see the header.
    if let Ok(p) = std::env::var("LOOK_FONT") {
        match std::fs::read(&p) {
            Ok(b) => {
                eprintln!("font: {p} ({} bytes)", b.len());
                slot_ui::text::set_font(b);
            }
            Err(e) => eprintln!("font: {p} unreadable: {e}"),
        }
    } else {
        eprintln!("LOOK_FONT not set: hanzi will be blank");
    }

    // 卡上的卡带素材，走与 motion 那条路同一个函数。
    // ★ 不装的话，货架画的是内建轮廓，不是卡上那套 System/Carts/*.png —— 后者才是真机上看到的面。
    if let Ok(p) = std::env::var("LOOK_DEPLOY") {
        slot_ui::install_cart_art(slot_ui::load_cart_art(Path::new(&p)));
        eprintln!("cart art: {p}");
    } else {
        eprintln!("LOOK_DEPLOY not set: 内建轮廓，不是卡上素材");
    }

    let rows: Vec<String> = std::env::var("LOOK_ROWS")
        .unwrap_or_else(|_| "shelf,menu".into())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let only: Option<Vec<usize>> = std::env::var("LOOK_ONLY")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect());

    let mut written = 0;
    for mode in [Mode::Dark, Mode::Light] {
        set_mode(mode);
        let m = match mode {
            Mode::Dark => "dark",
            Mode::Light => "light",
        };
        for i in 0..LOOK_COUNT {
            if only.as_ref().is_some_and(|o| !o.contains(&i)) {
                continue;
            }
            set_look(i);
            for row in &rows {
                let screen = match row.as_str() {
                    "shelf" => shelf_screen(System::Gba),
                    // 竖版（GB/GBC）货架：300 高中、200 高侧、零间距、三张全框。
                    "portrait" => shelf_screen(System::Gb),
                    // ★ 2026-10-25：GBC 单开一行。它和 GB 共用竖版几何，但**壳与阴影**是另一套
                    //   （`shape_of` 会按卡带头把一部分卡判成共通壳），不出图就只能靠猜。
                    "gbc" => shelf_screen(System::Gbc),
                    // ★ 2026-10-24：SFC / FC / NeoGeo 的卡带盒子各有一套高度（165 / 157 /
                    //   135），原来只能看 GBA 与 GB 两行，这三个机种从来没出过图 ——
                    //   「各机种的卡带背部跟随着兜底的卡带剪影」正是在这些行上才看得见。
                    "sfc" => shelf_screen(System::Sfc),
                    "fc" => shelf_screen(System::Fc),
                    "neogeo" => shelf_screen(System::Neogeo),
                    // ★ 2026-10-25：MD / PS1 各开一行。两者现在都借横版占位盒（`cart::size_for`
                    //   里和 NEOGEO/ARCADE 同一组），所以出图主要是为了**看 GEOMETRY 在不在**
                    //   —— 尤其 PS1 是 640×480 源、1× 缩放、720×480 满屏，和别的机种不是一个取景。
                    "md" => shelf_screen(System::Md),
                    "ps1" => shelf_screen(System::Ps1),
                    "menu" => menu_screen(),
                    // ★ 2026-10-26：长按 Y 打开的机种面板（压在真货架上）。
                    "syspick" => sys_picker_screen(),
                    "settings" => settings_screen(),
                    "card" => card_screen(),
                    "empty" => shelf_empty_screen(),
                    "desk" => desktop_screen(),
                    "sticker" => sticker_screen(),
                    other => {
                        eprintln!("unknown row {other}");
                        continue;
                    }
                };
                let file = dir.join(format!("{m}-{i:02}-{}-{row}.png", look_name(i)));
                screen.png(&file);
                println!("wrote {}", file.display());
                written += 1;
            }
        }
    }
    eprintln!("{written} images in {}", dir.display());
    // Back where a fresh process would start, so nothing after this sees a moved palette.
    set_look(0);
    set_mode(Mode::Dark);
}

// =============================================================================================
// 卡带动效预览（2026-10-17）—— 1 / 3 / 5 三条候选动作，逐帧出图
// =============================================================================================
//
// **只在 `MOTION_OUT` 指了目录时跑**，平时跑测试什么都不做：
//
// ```text
// MOTION_OUT=F:/…/_motion MOTION_WHAT=1,3,5 MOTION_LABELS=F:/…/Labels_logo \
// MOTION_DEPLOY=F:/…/slot-main/deploy MOTION_FONT=F:/…/Plix-Bold.otf \
//   SLOT_TEST_PKG=slot-ui python _tools/run_host_test.py --test render_look -- --nocapture
// ```
//
// 全是真东西：真卡面（`cart_face`）、真行几何（`Shelf::slot_rects`）、真合成、真 720×480。
// **唯一新的**是被评估的那条公式本身 —— 所以这些帧能拿来拍板，不是示意图。
//
// 三条动作都从同一个底座出发：`slot_rects` 已经算好每张卡带这一帧在哪、多大、多透，动作只
// 是在这几个数之后再叠一层。**没有任何一帧重画卡带** —— 面是开机烤好的（真机 26–68 ms/张，
// 见 `.workbuddy/memory/2026-09-13.md`），60 fps 一帧只有 16.7 ms，所以能动的只有位置、
// 大小、透明度，这条线在预览里和真做的完全一致。

/// 源帧率。出图按 60 fps 存，GIF 再降到 30 —— 动作 1 只有约 0.3 s，降帧会看不出回弹。
const MOTION_FPS: f32 = 60.0;
const MOTION_DT: f32 = 1.0 / MOTION_FPS;
const TAU: f32 = std::f32::consts::TAU;

/// 弹簧，照 `Shelf::update` 那一行抄的：`a = -2ζω·v - ω²·x`。ζ=1 时与真实现逐位相同，
/// **ζ<1 就是被评估的那一处改动**（动作 3）。
struct Spring {
    scroll: f32,
    vel: f32,
}

impl Spring {
    fn step(&mut self, dt: f32, target: f32, zeta: f32) {
        const OMEGA: f32 = 16.0;
        let accel = -2.0 * zeta * OMEGA * self.vel - OMEGA * OMEGA * (self.scroll - target);
        self.vel += accel * dt;
        self.scroll += self.vel * dt;
    }
}

/// 程序生成的一张光晕贴图（径向衰减，暖白）。动作 5 真做的话这里要定成一张卡上的资源；
/// 预览只求"位置和强度对不对"，所以现场生成。
fn glow_face(w: u32, h: u32) -> CartFace {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for yy in 0..h {
        for xx in 0..w {
            let dx = (xx as f32 - cx) / cx;
            let dy = (yy as f32 - cy) / cy;
            let d = (dx * dx + dy * dy).sqrt().min(1.0);
            let a = (1.0 - d).powf(2.2);
            let at = ((yy * w + xx) * 4) as usize;
            rgba[at] = 255;
            rgba[at + 1] = 236;
            rgba[at + 2] = 206;
            rgba[at + 3] = (a * 255.0) as u8;
        }
    }
    CartFace { rgba, w, h }
}

/// 接地影：一张压扁的径向渐变（黑，中心最浓）。
///
/// **不是整卡剪影往下挪** —— 那是投影，读作「物体浮在面上」，正好和"接地"相反。
/// 硬物站在面上只有一条线索：贴着接触边缘的一团暗。
fn contact_face(w: u32, h: u32) -> CartFace {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for yy in 0..h {
        for xx in 0..w {
            let dx = (xx as f32 - cx) / cx;
            let dy = (yy as f32 - cy) / cy;
            let d = (dx * dx + dy * dy).sqrt().min(1.0);
            let a = (1.0 - d).powf(1.6);
            let at = ((yy * w + xx) * 4) as usize;
            rgba[at + 3] = (a * 255.0) as u8;
        }
    }
    CartFace { rgba, w, h }
}

/// 整屏货架，纹理只 mint 一次，逐帧只重放绘制列表 —— 这样一张图 ~600k 像素的合成不必
/// 每帧重烤卡面。
/// 一帧要施在行上的效果。全零 = 与 `Shelf::draw_row` 完全一致。
///
/// 这几个参数就是「自然 / 不自然」的全部旋钮，每一个都对应一条物理依据（见 `render_cart_motion`
/// 顶上的注释）：**形变**（`squash`）是给有弹性的东西的，**接地影**（`contact`）才是硬物重量的
/// 正确表达；`sink` 是硬物被放下时那 2 px；`lag_tau` 是每张卡各带一点惯性。
#[derive(Clone, Copy, Default)]
struct Fx {
    squash: Option<(f32, Option<usize>)>,
    /// 中间那张的接地影强度，0 = 不画（现状）。
    contact: f32,
    /// 整体下沉，像素（正 = 向下），硬物落地的那一下。
    sink: f32,
    /// 每张卡位置的低通时间常数（秒）。中心那张按中心度递减到 0，所以它永远准。
    lag_tau: f32,
}

struct Scene {
    s: Screen,
    shelf: Shelf,
    faces: Vec<Option<TexId>>,
    glow: Option<TexId>,
    contact: TexId,
    /// 每张卡带的名字，开局全烤好（真机也是一张张烤，所以这里不能每帧现烤）。
    titles: Vec<(TexId, u32, u32)>,
    ring: Letters,
    ring_faces: Vec<(TexId, u32, u32)>,
    ridge: Option<(TexId, u32, u32)>,
    counts: [usize; letters::N],
    clock: (TexId, u32),
    pct: (TexId, u32),
    badge: Option<TexId>,
    /// 每张卡上一帧画在哪 —— 位置滞后的状态。按 `carts` 下标，不是按槽位。
        lag: Vec<Option<[f32; 4]>>,
    /// 每张卡这一帧要不要转过一点（弧度），按 `carts` 下标。0 = 不转。
    turns: Vec<f32>,
    /// 每张卡额外下沉多少像素（斜向排列用），按 `carts` 下标。`None` = 不斜。
    slant: Option<Vec<f32>>,
    /// 每张卡绕竖直轴偏转多少，按 `carts` 下标（**景深透视的模拟**，见 `yaw_draw`）。
    yaw: Vec<f32>,
    /// 每张卡整体缩放（"离观察者多远"），按 `carts` 下标。1.0 = 原大小。
    zoom: Vec<f32>,
    /// 每张卡额外的横向位移（把行从"等距直线"挪到"圆弧上"），按 `carts` 下标。
    shift: Vec<f32>,
    /// 卡上的壁纸。**判「接地」「半透明」这类效果必须在壁纸上**：黑底上黑影子等于没画，
    /// 而当初玩家报的半透明问题本来也是"有壁纸的情况下"才看得见。
    wall: Option<TexId>,
    /// 侧位 / 中位的宽，用来看某张卡离中间有多近（`slot_rects` 不给 offset）。
    width: (f32, f32),
}

impl Scene {
    fn new(carts: Vec<Cart>, with_glow: bool, wall: Option<Vec<u8>>) -> Scene {
        let size = size_for(System::Gba);
        let system = System::Gba;
        let mut s = Screen::new();

        let faces: Vec<Option<TexId>> = carts.iter().map(|c| Some(s.mint(cart_face(c)))).collect();
                let glow = with_glow.then(|| s.mint(glow_face(620, 150)));
        let contact = s.mint(contact_face(360, 64));
        let wall = wall.map(|rgba| {
            s.mint(CartFace {
                rgba,
                w: OUT_W,
                h: OUT_H,
            })
        });

        // 标题跟着选中那张走，所以七张全烤：真机是选中时现烤一张，预览里没有那个预算，
        // 一次烤完再按帧挑。图里标题与中间那张对不上，是最容易被当成错的东西。
        let titles: Vec<(TexId, u32, u32)> = carts
            .iter()
            .map(|c| {
                let t = shelf_title_face(&c.title);
                let (w, h) = (t.w, t.h);
                (s.mint(t), w, h)
            })
            .collect();

        let counts = [3usize; letters::N];
        let ring_faces: Vec<(TexId, u32, u32)> = letters::SLOTS
            .iter()
            .map(|ch| {
                let f = letters::letter_face(*ch);
                let (w, h) = (f.w, f.h);
                (s.mint(f), w, h)
            })
            .collect();
        let ridge = letters::ridge_face();
        let (rw, rh) = (ridge.w, ridge.h);
        let ridge = Some((s.mint(ridge), rw, rh));
        let mut ring = Letters::new();
        ring.snap_to('T');

        let cf = clock_face("14:52");
        let cw = cf.w;
        let clock = (s.mint(cf), cw);
        let pf = word_face("64%");
        let pw = pf.w;
        let pct = (s.mint(pf), pw);
        let badge = Some(s.mint(icon_face(
            Icon::of_mode(slot_ui::palette::mode()),
            HUD_ICON_PX,
            slot_ui::palette::ink(),
        )));

        let mut shelf = Shelf::new(carts);
        shelf.set_system(system);
                shelf.set_faces(faces.clone());
        shelf.index = 3;
        shelf.scroll = 3.0;

        let n_carts = shelf.carts.len();
        Scene {
            s,
            shelf,
            faces,
            glow,
            contact,
            titles,
            ring,
            ring_faces,
            ridge,
            counts,
            clock,
            pct,
            badge,
            lag: vec![None; n_carts],
            turns: vec![0.0; n_carts],
            slant: None,
            yaw: vec![0.0; n_carts],
            zoom: vec![1.0; n_carts],
            shift: vec![0.0; n_carts],
            wall,
            width: (size.w as f32 * size.side, size.w as f32 * size.middle),
        }
    }

    /// 一帧。行的画法与 `Shelf::draw_row` 的非占位分支同构 —— 阴影（半透明时才画，与 10-17 的
    /// 修复一致）+ 面。`fx` 那几项作用在 rect 上，**不动纹理**：卡带面是开机烤好的，一帧都
    /// 不能重烤，能改的只有位置、大小、透明度。
    fn draw(&mut self, fx: Fx, glow: Option<(f32, f32)>) {
        self.s.list.clear();
        draw_backdrop(self.wall, None, &mut self.s.list);
        draw_empty_slot(&mut self.s.list);

        let foot = size_for(System::Gba).foot_y() + fx.sink;
        if let (Some(tex), Some((alpha, scale))) = (self.glow, glow) {
            let (w, h) = (620.0 * scale, 150.0 * scale);
            self.s.list.push(Draw::Tex {
                x: OUT_W as f32 / 2.0 - w / 2.0,
                y: foot + 6.0 - h / 2.0,
                w,
                h,
                tex,
                alpha,
            });
        }

        let (side_w, middle_w) = self.width;
        for r in self.shelf.slot_rects(None, 0.0, 0.0) {
            let sx = self.shift.get(r.cart).copied().unwrap_or(0.0);
            let extra = self
                .slant
                .as_ref()
                .and_then(|m| m.get(r.cart).copied())
                .unwrap_or(0.0);
            let (mut x, mut y, mut w, mut h) = (r.x + sx, r.y + fx.sink + extra, r.w, r.h);
            let alpha = r.alpha;
            let cw = ((r.w - side_w) / (middle_w - side_w)).clamp(0.0, 1.0);

            // ① 形变（旧的演示用；硬塑料上这是错的，见 render_cart_motion 的注释）。
            if let Some((k0, Some(sel))) = fx.squash {
                if sel == r.cart {
                    let k = k0.clamp(-0.15, 0.15);
                    let (nw, nh) = (w * (1.0 + k * 0.6), h * (1.0 - k));
                    x += (w - nw) / 2.0;
                    y = (y + h) - nh;
                    w = nw;
                    h = nh;
                }
            }

            // ② 每张卡各带一点惯性：中心那张 tau=0（它必须准），越往外越软。真实的一排
            //    物件不会同时起步、同时停住，这一项就是"被拖着走"的那点重量。
            if fx.lag_tau > 0.0 {
                let tau = fx.lag_tau * (1.0 - cw);
                if tau > 0.0 {
                    let a = 1.0 - (-MOTION_DT / tau).exp();
                    let prev = self.lag.get(r.cart).copied().flatten();
                    let now = [x, y, w, h];
                    let out = match prev {
                        Some(p) => [
                            p[0] + (now[0] - p[0]) * a,
                            p[1] + (now[1] - p[1]) * a,
                            p[2] + (now[2] - p[2]) * a,
                            p[3] + (now[3] - p[3]) * a,
                        ],
                        None => now,
                    };
                    if let Some(slot) = self.lag.get_mut(r.cart) {
                        *slot = Some(out);
                    }
                    x = out[0];
                    y = out[1];
                    w = out[2];
                    h = out[3];
                }
            }

            // ③ 接地影：物体与货架接触的那块暗。**不是整卡剪影再放大**，而是同一张剪影
            //    缩一点、往下压一点 —— 露出来的只有底缘和两侧，读作压在面上。
            if fx.contact > 0.0 && cw > 0.55 {
                let k = fx.contact * ((cw - 0.55) / 0.45).clamp(0.0, 1.0);
                // 贴住底缘的一团暗：宽 0.92 张卡、高 0.16 张卡，中心压在底边上方一点点，
                // 所以露出来的只有底缘那半圈。
                let gw = w * 0.94;
                let gh = h * 0.22;
                self.s.list.push(Draw::Tex {
                    x: x + (w - gw) / 2.0,
                    // 中心正好压在底边上：一半在卡带里、一半露在下面的壁上，
                    // 浓的那条紧贴边缘 —— 这才是"接触"。
                    y: y + h - gh * 0.5,
                    w: gw,
                    h: gh,
                    tex: self.contact,
                    alpha: k.clamp(0.0, 1.0),
                });
            }

            // ★ **影子和卡面必须走同一套变换。** 原来影子在透视绘制之前按正矩形画，
            //   于是卡带成了梯形、影子还是矩形 —— 六六看到的"阴影没跟着卡带的朝向"。
            //   做法：先画剪影，再画面（面盖在上面，与货架同构）。
            let z = self.zoom.get(r.cart).copied().unwrap_or(1.0);
            if z != 1.0 {
                let foot = y + h;
                let cx0 = x + w / 2.0;
                w *= z;
                h *= z;
                x = cx0 - w / 2.0;
                y = foot - h;
            }
            let yaw = self.yaw.get(r.cart).copied().unwrap_or(0.0);
            let dx = (x + w / 2.0) - OUT_W as f32 / 2.0;
            // 「近边靠中间」：左卡的近边在**右**、右卡的近边在**左** ⇒ 右卡 inner_left = true。
            let inner_left = dx >= 0.0;

            // ★ 2026-10-25：这里原本先铺一块"卡带剪影涂黑"再画卡面。真机（`shelf.rs`）
            //   已经不再画影子了（六六：「GBC 的阴影还没拿掉」），预览跟着删 ——
            //   预览要是还留着，就会一直给你一张比真机多一层黑的图。
            if yaw.abs() > 0.001 {
                if let Some(Some(tex)) = self.faces.get(r.cart) {
                    // 侧位：**按六六示意图的实测比例**做梯形（宽 0.62、近边 0.733、远边 0.528）。
                    let pw = w * WIDTH_K / 1.0_f32.max(1.0);
                    let ph = h;
                    let _ = pw;
                    persp_card(
                        &mut self.s.texes,
                        &mut self.s.list,
                        *tex,
                        x + w / 2.0,
                        y + h / 2.0,
                        w * WIDTH_K,
                        ph,
                        inner_left,
                        alpha,
                    );
                    continue;
                }
            }
            if let Some(Some(tex)) = self.faces.get(r.cart) {
                let turn = self.turns.get(r.cart).copied().unwrap_or(0.0);
                if turn == 0.0 {
                    self.s.list.push(Draw::Tex {
                        x,
                        y,
                        w,
                        h,
                        tex: *tex,
                        alpha,
                    });
                } else {
                    // 转角越大越要留边，否则四角会被裁 —— 与核心选择器里那颗芯片同一套做法
                    // （`grown(body, TURN_PAD * zoom)`）。留边是按**画面像素**给的，不是按面像素。
                    let pad = slot_ui::TURN_PAD as f32 * (w / 240.0).max(1.0);
                    self.s.list.push(Draw::Turned {
                        x: x - pad,
                        y: y - pad,
                        w: w + pad * 2.0,
                        h: h + pad * 2.0,
                        tex: *tex,
                        alpha,
                        turn,
                    });
                }
            }
        }

        draw_top_band(&mut self.s.list);
        draw_edge_glow(&mut self.s.list);
        let (ring_faces, ridge, counts) = (&self.ring_faces, self.ridge, &self.counts);
        self.ring.draw_strip(ring_faces, ridge, counts, &mut self.s.list);

        let sel = self.shelf.cart_at_offset(0).unwrap_or(0);
        let (tex, tw, th) = self.titles[sel];
        let case = OUT_H as f32 - 58.0;
        self.s.list.push(Draw::Tex {
            x: (OUT_W as f32 - tw as f32) / 2.0,
            y: (foot + case) / 2.0 - th as f32 / 2.0,
            w: tw as f32,
            h: th as f32,
            tex,
            alpha: 1.0,
        });

        draw_status(
            Some(Battery {
                percent: 64,
                charge: Charge::Discharging,
            }),
            Printed::new(self.pct.0, self.pct.1),
            None,
            Printed::new(self.clock.0, self.clock.1),
            self.badge,
            &mut self.s.list,
        );
    }

    fn frame_with(
        &mut self,
        dir: &Path,
        tag: &str,
        i: usize,
        fx: Fx,
        glow: Option<(f32, f32)>,
    ) {
        self.draw(fx, glow);
        self.s.png(&dir.join(format!("{tag}_{i:04}.png")));
    }
}

/// 七个真封面的卡带。认得的优先，缺了就从目录里补 —— 一张真实封面的卡带比五个色块更能
/// 判断"这张在动没有"。
fn motion_carts(labels: &Path) -> Vec<Cart> {
    const PREFER: [&str; 9] = [
        "宝可梦Gen3-红宝石",
        "塞尔达传说-缩小帽",
        "星之卡比-镜之大迷宫",
        "恶魔城月轮MOD版V2",
        "世界传说-召唤者的血统",
        "光明之魂1",
        "地球冒险3",
        "公主同盟",
        "召唤之夜-铸剑物语1",
    ];
    let on_disk: Vec<String> = std::fs::read_dir(labels)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.to_lowercase().ends_with(".png"))
        .collect();
    let mut names: Vec<String> = PREFER
        .iter()
        .filter(|t| on_disk.iter().any(|n| n == &format!("{t}.png")))
        .map(|t| (*t).to_string())
        .collect();
    for n in &on_disk {
        if names.len() >= 7 {
            break;
        }
        let stem = n.trim_end_matches(".png").to_string();
        if !names.contains(&stem) {
            names.push(stem);
        }
    }
    names.truncate(7);
    names
        .iter()
        .map(|t| Cart {
            stem: t.clone(),
            rom: PathBuf::from(format!("Games/GBA/{t}.gba")),
            label: Some(labels.join(format!("{t}.png"))),
            art: None,
            title: t.clone(),
            code: String::new(),
            cgb: 0,
            initial: t.chars().next().unwrap_or('#'),
        })
        .collect()
}

/// 三条动作共同的开场：一步都不走的一行，先把 scroll 摆到 index 上。
fn settled(carts: Vec<Cart>, with_glow: bool, wall: Option<Vec<u8>>) -> Scene {
    let mut sc = Scene::new(carts, with_glow, wall);
    sc.shelf.index = 3;
    sc.shelf.scroll = 3.0;
    sc
}

/// 把行收窄：卡小、格距密，这样一条弧上能同时看到五六张 —— **判断"是不是一个轮子"
/// 需要看到整条弧**，只露三张（1 张 + 左右半张）是看不出来的。
/// 这是**预览专用**：真机上要不要这么密是另一件事。
// 收窄预览是留给"要不要这么密"这个问题的：现在没有测试用它，但删掉它就得在下一次
// 看整条弧的时候重新写一遍。
#[allow(dead_code)]
fn narrow(mut sc: Scene, pitch: f32, middle: f32, side: f32, w: u32, h: u32) -> Scene {
    sc.shelf.set_size(CartSize::new(w, h, pitch, middle, side, 0.0, [90, 228, 910, 863]));
    sc.width = (w as f32 * side, w as f32 * middle);
    sc
}

#[test]
fn render_cart_motion() {
    let Ok(dir) = std::env::var("MOTION_OUT") else {
        eprintln!("MOTION_OUT not set: no motion frames rendered");
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();

    if let Ok(p) = std::env::var("MOTION_FONT") {
        if let Ok(b) = std::fs::read(&p) {
            slot_ui::text::set_font(b);
        }
    }
    if let Ok(p) = std::env::var("MOTION_DEPLOY") {
        slot_ui::install_cart_art(slot_ui::load_cart_art(Path::new(&p)));
    }
    // 壁纸：给了就用，没给就是黑底（黑底上接地影看不出来，别在这上面下判断）。
    let wall = std::env::var("MOTION_WALL")
        .ok()
        .and_then(|p| slot_ui::wallpaper_face(Path::new(&p)));
    if wall.is_none() {
        eprintln!("MOTION_WALL not set: 黑底，接地影与半透明都看不出来");
    }
    let labels = PathBuf::from(std::env::var("MOTION_LABELS").unwrap_or_default());
    let carts = motion_carts(&labels);
    if carts.len() < 5 {
        eprintln!("only {} labels found in {}: the row will be thin", carts.len(), labels.display());
    }
    let which: Vec<u8> = std::env::var("MOTION_WHAT")
        .unwrap_or_else(|_| "1,3,5".into())
        .split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect();

    // ---- 动作 1：落位压一下 ------------------------------------------------------------------
    // 0.35 s 静止 → 按一下右键 → 0.55 s 落位 → 0.5 s 静止，共 1.4 s。
    // 压扁曲线：4% 起压，`exp(-t/0.05)` 衰减，周期 75 ms ⇒ 压→拉→停，约 0.25 s 走完。
    if which.contains(&1) {
        let mut sc = settled(carts.clone(), false, wall.clone());
        let mut spring = Spring { scroll: 3.0, vel: 0.0 };
        let (total, press) = (84usize, 21usize);
        let mut since: Option<f32> = None;
        let mut peak = 0.0f32;
        for f in 0..total {
            if f == press {
                sc.shelf.index += 1;
            }
            sc.shelf.scroll = spring.scroll;
            let target = sc.shelf.scroll_target();
            spring.step(MOTION_DT, target, 1.0);
            sc.shelf.scroll = spring.scroll;
            // ★ 起压的时刻是**到位**那一刻，不是按键那一刻。弹簧从按下要 ~150 ms 才把新卡带
            //   送到中间，压扁曲线若跟着按键走，衰减完时卡带还没到 —— 压到的是正滑走的那张。
            //   判据用"离目标不到 0.15 张"（约 54 px，肉眼看就是已经到了），与走哪条曲线无关。
            if since.is_none() && f > press && (spring.scroll - target).abs() < 0.15 {
                since = Some(0.0);
            }
            let sel = sc.shelf.cart_at_offset(0);
            let squash = since.map(|t| {
                // 6% 起压，90 ms 衰减、周期 130 ms ⇒ 一压 → 一回弹 → 停，约 0.3 s 收干净。
                let k = 0.06 * (-t / 0.09).exp() * (TAU * t / 0.13).cos();
                (k, sel)
            });
            if let Some((k, _)) = squash {
                peak = peak.max(k.abs());
            }
            if let Some(t) = since.as_mut() {
                *t += MOTION_DT;
            }
            sc.frame_with(&dir, "m1", f, Fx { squash, ..Default::default() }, None);
        }
        // 5% 的高度压在 202.5 px 的中位卡面上是几像素，报出来，省得看图猜。
        let h = size_for(System::Gba).h as f32 * 1.5;
        eprintln!("m1: squash peak {peak:.3} = {:.1} px on a {h:.0} px face", peak * h);
    }

    // ---- 动作 3：快翻时轻微过冲 --------------------------------------------------------------
    // 三下连按（110 ms 一下，与 `REPEAT_MS` 一致）再撒手。ζ 随速度降：静止时仍是临界阻尼，
    // 所以跑完不会一直晃；速度到 4 张/秒时 ζ 落到 0.65。
    if which.contains(&3) {
        // 三档：不砍（现状）、砍到 ζ≈0.65（过冲 3.6% ≈ 13 px）、砍到 ζ≈0.50（14% ≈ 51 px）。
        // ★ 复算出来的反直觉处：砍 0.15（ζ 只到 0.85）过冲是 **0 px** —— 速度一落，阻尼就回到
        //   临界，根本越不过去。所以"轻微过冲"这个说法在 0.85 上不成立，要看得见就得砍到 0.5。
        for (tag, zeta_k) in [("m3a", 1.0f32), ("m3b", 0.35f32), ("m3c", 0.50f32)] {
            let mut sc = settled(carts.clone(), false, wall.clone());
            let mut spring = Spring { scroll: 3.0, vel: 0.0 };
            let total = 96usize;
            let presses = [21usize, 28, 35];
            let mut over = 0.0f32;
            for f in 0..total {
                if presses.contains(&f) {
                    sc.shelf.index = (sc.shelf.index + 1) % sc.shelf.carts.len();
                }
                sc.shelf.scroll = spring.scroll;
                let target = sc.shelf.scroll_target();
                let zeta = 1.0 - zeta_k * (spring.vel.abs() / 4.0).min(1.0);
                spring.step(MOTION_DT, target, zeta);
                sc.shelf.scroll = spring.scroll;
                if f > 40 {
                    over = over.max((spring.scroll - target).abs());
                }
                sc.frame_with(&dir, tag, f, Fx::default(), None);
            }
            eprintln!(
                "{}: zeta cut {zeta_k:.2}, peak overshoot {:.1} px (pitch 360)",
                tag,
                over * 360.0
            );
        }
    }

    // ---- 动作 5：选中下方光晕呼吸 ------------------------------------------------------------
    // 2.4 s 一个周期，α 0.10 ↔ 0.18，光斑 620×150 微微涨 2%。卡带一步不动 —— 这正是它被
    // 单列出来的原因：动的只有光。
    if which.contains(&5) {
        let mut sc = settled(carts.clone(), true, wall.clone());
        let total = 144usize; // 2.4 s
        for f in 0..total {
            let t = f as f32 * MOTION_DT;
            let s = (TAU * t / 2.4).sin();
            sc.frame_with(&dir, "m5", f, Fx::default(), Some((0.14 + 0.04 * s, 1.0 + 0.02 * s)));
        }
        eprintln!("m5: {} frames, alpha 0.10..0.18", total);
    }

    // ---- 6：单步落位 —— 现状 vs「接地」（对比 A）-----------------------------------------------
    //
    // **形变撤掉了。** 卡带是硬塑料：plastic–steel 的恢复系数 0.45–0.55，而手放下去的速度
    // 只有零点几 m/s，速度越低越不弹 ⇒ 它根本不压缩、也不回弹。squash&stretch 是给有弹性的
    // 东西的（迪士尼那套是给角色的），压在 ABS 壳上是把材质说错了。
    //
    // 硬物的「重量」只有一条正确线索：**它和货架的接触**。所以这一版只加两样 ——
    // 常驻的接地影（影子缩短、压深），和落位时 2 px 的下沉（整体位移，不是体积形变）。
    if which.contains(&6) {
        for (tag, ground) in [("c1a", 0.0f32), ("c1b", 1.0f32)] {
            let mut sc = settled(carts.clone(), false, wall.clone());
            let mut spring = Spring { scroll: 3.0, vel: 0.0 };
            let (total, press) = (84usize, 21usize);
            let mut since: Option<f32> = None;
            for f in 0..total {
                if f == press {
                    sc.shelf.index += 1;
                }
                sc.shelf.scroll = spring.scroll;
                let target = sc.shelf.scroll_target();
                spring.step(MOTION_DT, target, 1.0);
                sc.shelf.scroll = spring.scroll;
                if since.is_none() && f > press && (spring.scroll - target).abs() < 0.15 {
                    since = Some(0.0);
                }
                // 落位脉冲：正弦半波，0.20 s 走完，不出负值。
                // ★ 只走**一个**半波：`sin` 自带 0.4 s 周期，不截断的话它会在 0.4–0.6 s
                //   再来一次，变成"放下之后还在呼吸"——第一次跑就是这么错的。
                let pulse = since
                    .map(|t| {
                        if t >= 0.20 {
                            0.0
                        } else {
                            (std::f32::consts::PI * t / 0.20).sin()
                        }
                    })
                    .unwrap_or(0.0);
                let fx = Fx {
                    contact: ground * (0.50 + 0.20 * pulse),
                    sink: ground * 2.0 * pulse,
                    // 两侧卡带滞后 22 ms：真实的一排物件不会同时起步、同时停住。
                    lag_tau: ground * 0.022,
                    ..Default::default()
                };
                if let Some(t) = since.as_mut() {
                    *t += MOTION_DT;
                }
                sc.frame_with(&dir, tag, f, fx, None);
            }
        }
        eprintln!("c1: 单步 84 帧 ×2（现状 / 接地影+2px）");
    }

    // ---- 7：按住连发 —— 现状的脉动 vs 连续斜坡 + 磁吸（对比 B）--------------------------------
    //
    // 实测（`_tools` 里的数）：现状按住时 scroll 速度在 **6.4–11.4 格/秒**之间脉动，峰谷差
    // 5.0 —— 目标是一格一格阶梯跳的，二阶系统追阶梯必然交替加速/减速。这是**原有**的问题，
    // 不是这一轮加的。
    //
    // 改法不是"发明"：iOS / Android 的滚动按住都是**连续斜坡**，松手才减速。再补一条这个物件
    // 自己的物理 —— 货架有卡位，所以松手后把 scroll **磁吸**到最近的一格（临界阻尼 ω=20）。
    if which.contains(&7) {
        // 12 → 33 帧 = 0.35 s，走 3 格；松手时刻故意不落在整格上（≈ +0.15 格），这样才能
        // 看到「咔哒」那一下磁吸 —— 真的松手本来就落在任意位置。
        const HOLD: (usize, usize) = (12, 33);
        const SPEED: f32 = 9.0;
        for (tag, ramp) in [("c2a", false), ("c2b", true)] {
            let n = carts.len();
            let mut sc = settled(carts.clone(), false, wall.clone());
            let mut spring = Spring { scroll: 3.0, vel: 0.0 };
            let mut ramp_scroll = 3.0f32;
            let total = 90usize;
            let steps = [12usize, 19, 26];
            let mut released: Option<f32> = None;
            for f in 0..total {
                if ramp {
                    if f < HOLD.0 {
                        sc.shelf.scroll = 3.0;
                        sc.shelf.index = 3;
                    } else if f < HOLD.1 {
                        // 推力是连续的，所以速度从 0 包络到恒速（50 ms），之后线性 ——
                        // 斜坡输入，二阶系统跟斜坡是**匀速**的，不会交替加减速。
                        let since = (f - HOLD.0) as f32 * MOTION_DT;
                        let env = 1.0 - (-since / 0.05).exp();
                        ramp_scroll += SPEED * env * MOTION_DT;
                        sc.shelf.scroll = ramp_scroll;
                        sc.shelf.index = ramp_scroll.round() as usize % n;
                    } else {
                        if released.is_none() {
                            released = Some(ramp_scroll);
                            // 松手 = 推力消失：速度归零，只剩卡位把它吸住。
                            spring.scroll = ramp_scroll;
                            spring.vel = 0.0;
                        }
                        let snap = released.unwrap_or(3.0).round();
                        let d = spring.scroll - snap;
                        spring.vel += (-2.0 * 20.0 * spring.vel - 20.0 * 20.0 * d) * MOTION_DT;
                        spring.scroll += spring.vel * MOTION_DT;
                        sc.shelf.scroll = spring.scroll;
                        sc.shelf.index = spring.scroll.round() as usize % n;
                    }
                } else {
                    if steps.contains(&f) {
                        sc.shelf.index = (sc.shelf.index + 1) % n;
                    }
                    sc.shelf.scroll = spring.scroll;
                    let target = sc.shelf.scroll_target();
                    spring.step(MOTION_DT, target, 1.0);
                    sc.shelf.scroll = spring.scroll;
                }
                let fx = Fx {
                    contact: 0.50,
                    lag_tau: 0.022,
                    ..Default::default()
                };
                sc.frame_with(&dir, tag, f, fx, None);
            }
        }
        eprintln!("c2: 连发 90 帧 ×2（阶梯弹簧 / 斜坡+磁吸）");
    }

    // ---- 8：斜向多米诺 —— 三种方案并排（对比 C）------------------------------------------------
    //
    // 先算清一件事：**屏上其实只看得到三张卡**（1 张选中 + 左右各半张）。720 宽、pitch 360，
    // 侧位 240 宽 ⇒ 侧位只露 50%。所以"一排骨牌"最多是 3 张，主题是「斜向 + 选中回正」。
    //
    // 顶点着色器是纯 2D（`gl_Position` 的 z 恒 0、w 恒 1），但真渲染器**支持旋转**
    // （`Draw::Turned` 走的是同一个 sprite program 的 `u_turn`），所以 (3) 不需要动渲染层。
    //
    //   (1) 行整体斜：沿一条斜线排布，离中心每 1 格下沉 DROP px
    //   (2) 单张骨牌斜：每张各转 ±ANGLE，选中那张转到 0
    //   (3) 两样都来
    if which.contains(&8) {
        // 斜线的斜率：每 1 px 横向偏移升/降多少。tan(6°) ≈ 0.105 ⇒ 侧位中心距中心 360 px，
        // 两端高差约 ±38 px。
        const SLOPE: f32 = 0.105;
        // 单张卡跟着斜线躺下的角度。**同一张卡不需要"倾斜"这个独立参数** —— 骨牌躺在斜线上，
        // 它的角度就等于斜线的角度，这是它读起来像"一排骨牌"的原因。

        for (tag, slant, tilt) in [
            ("d1", true, false),
            ("d2", false, true),
            ("d3", true, true),
        ] {
            let mut sc = settled(carts.clone(), false, wall.clone());
            // 36 帧：静止 0.3 s → 按一下 → 回正 → 再停。**要出动画**，单帧看不出
            // "斜着躺 → 滑到中间 → 立正"这个过程。
            let mut spring = Spring { scroll: 3.0, vel: 0.0 };
            for f in 0..36usize {
                if f == 18 {
                    sc.shelf.index += 1;
                }
                sc.shelf.scroll = spring.scroll;
                let target = sc.shelf.scroll_target();
                spring.step(MOTION_DT, target, 1.0);
                sc.shelf.scroll = spring.scroll;
                let rects = sc.shelf.slot_rects(None, 0.0, 0.0);
                // ★ 斜线必须用**带符号**的横向偏移来算。按"离中心几格"算的话左右两侧下沉量
                //   相同，那只是整排一起下沉，不是斜线 —— 第一次就是这么错的。
                let mut down = vec![0.0f32; sc.turns.len()];
                let mut turns = vec![0.0f32; sc.turns.len()];
                // ★★ 这两个量**只按几何算，完全不看"谁是选中下标"**。
                //   `dx` 是这张卡中心离屏中的横向距离，`pitch`(360) 一格 ⇒ `slots` 就是
                //   "离中间几格"，连续、带符号。滑动过程中它自然在 0 附近连续变化，
                //   所以"滑到中间就自动立正"是**算出来的**，不需要任何判断。
                //
                //   之前两版都错在这里：一版拿"下标是否等于选中"当判据（滑动中屏幕上居中的
                //   未必是那个下标），一版把 `near=(w-side)/(mid-side)` 乘上去，结果把中间的
                //   那张乘成了 1（正好反了），出来是"中间歪、两边正"。
                for r in &rects {
                    let dx = (r.x + r.w / 2.0) - OUT_W as f32 / 2.0;
                    let slots = dx / 360.0;
                    if slant {
                        // 一条直线穿过中间那张：左升右降，限幅 ±48 px。
                        down[r.cart] = (slots * 38.0).clamp(-48.0, 48.0);
                    }
                    if tilt {
                        // ★ **同号**：骨牌躺在同一条线上的话，每张的角度是一样的（都等于这条
                        //   线的角度）。按 `sign(dx)` 取号会把整排变成浅 V 字。
                        turns[r.cart] = (slots * 0.105).atan().clamp(-0.108, 0.108);
                    }
                }
                // 选中的那张：斜线与角度都归零 —— 这就是"回正"。
                sc.slant = slant.then_some(down);
                sc.turns = turns;
                sc.frame_with(
                    &dir,
                    tag,
                    f,
                    Fx {
                        contact: 0.50,
                        ..Default::default()
                    },
                    None,
                );
            }
        }
        eprintln!("d1/d2/d3: 斜率 {SLOPE} (≈6.2°) · 两端高差 ≈ ±38px");
    }

    // ---- 9：景深透视（对比 D）------------------------------------------------------------------
    //
    // 「绕竖直轴偏转」的货架：侧位卡各偏一点，中间那张回正 —— 这就是常见的 coverflow。
    // 真实实现要动**顶点着色器**：现在 `gl_Position` 的 z 恒 0、w 恒 1（纯 2D 正交），
    // 要加一个 vec4 的四角深度、做透视除法。同一个 sprite program，改动不大但确实要改。
    //
    // 这里用逐列绘制先把**效果**给你看（一列一个 quad 是预览专用，设备上不成立）。
    if which.contains(&9) {
        // **投影对比**：这是唯一能把"平面 / 转角透视 / 真 Z 透视"分开的办法。
        // 一律：中间 0° 正对、两侧正立，行收窄到能同时看到 5 张，按一下方向键看轮子转。
        //
        //   r1 = 平面：只有卡带自身的旋转（真渲染层能做到的）
        //   r2 = 转角透视：水平位置沿 R·sinθ + 横向按 cosθ 压窄（仍然是平面重采样）
        //   r3 = 真 Z 透视：给四个角不同的深度让 w 做除法（＝你字母轮那套，也是 A 档）
        const DEG: f32 = 38.0;
        // ★★ **半径必须由格距和每格弧角推出来**，不能自己给一个数 ——
        //    这正是字母轮的做法（`DRUM_R = PITCH / FACET`）。我第一版硬写了 520，
        //    与格距不自洽，圆弧上的卡被推到屏幕外切掉。
        //    `R·Δθ = pitch` ⇒ 相邻两张在弧上的间距正好等于格距，前端那一格钉在原地。
        const PITCH: f32 = 250.0;
        const TH: f32 = DEG * std::f32::consts::PI / 180.0;
        const R: f32 = PITCH / TH;
        const D: f32 = 1500.0;
        for (tag, mode) in [("r1", 0u8), ("r2", 1u8), ("r3", 2u8)] {
            // ★ **用真实货架几何**（pitch 360 / middle 1.5 / side 1.0），不要 `narrow`：
            //   真实几何里侧位 240 / 中间 360 = **0.667**，本来就接近六六图里的 0.62；
            //   我上一版拿收窄的侧位比例去比，量出 1.16，是**预览参数错了**，不是透视错了。
            let mut sc = settled(carts.clone(), false, wall.clone());
            let mut spring = Spring { scroll: 3.0, vel: 0.0 };
            for f in 0..36usize {
                if f == 18 {
                    sc.shelf.index += 1;
                }
                sc.shelf.scroll = spring.scroll;
                let target = sc.shelf.scroll_target();
                spring.step(MOTION_DT, target, 1.0);
                sc.shelf.scroll = spring.scroll;

                let rects = sc.shelf.slot_rects(None, 0.0, 0.0);
                let mut yaw = vec![0.0f32; sc.turns.len()];
                let mut zoom = vec![1.0f32; sc.turns.len()];
                let mut want_x = vec![0.0f32; sc.turns.len()];
                for r in &rects {
                    // 离中间几格（连续、带符号）。轮子的一格 = `pitch`。
                    let slots = ((r.x + r.w / 2.0) - OUT_W as f32 / 2.0) / sc.shelf.size().pitch;
                    let k = slots.clamp(-3.0, 3.0);
                    let th = k * DEG.to_radians();
                    match mode {
                        // 平面：只转，不动位置
                        0 => {
                            yaw[r.cart] = th.clamp(-0.6, 0.6);
                        }
                        // 转角透视：位置沿弧，横向压窄（横向压窄由 `yaw` 的 cos 负责）
                        1 => {
                            let xr = R * th.sin();
                            want_x[r.cart] = xr - k * sc.shelf.size().pitch;
                            yaw[r.cart] = th;
                        }
                        // 真 Z 透视：同样沿弧，但**远处的卡更小**
                        _ => {
                            let xr = R * th.sin();
                            want_x[r.cart] = xr - k * sc.shelf.size().pitch;
                            yaw[r.cart] = th;
                            let z = R * (1.0 - th.cos());
                            zoom[r.cart] = D / (D + z);
                        }
                    }
                }
                sc.yaw = yaw;
                sc.zoom = zoom;
                // `want_x` 折进 slant 之外的横向位移：这里借 `turns` 之外再加一层不方便，
                // 直接把偏移写进一个专用的 shift（见 Scene::shift）。
                sc.shift = want_x;
                sc.frame_with(
                    &dir,
                    tag,
                    f,
                    Fx {
                        contact: 0.40,
                        ..Default::default()
                    },
                    None,
                );
            }
        }
        eprintln!("r1/r2/r3: 平面 / 转角透视 / 真Z透视 · pitch {PITCH} 一格 {DEG}° ⇒ R {R:.0}");
    }

    eprintln!("motion frames in {}", dir.display());
}

// 待机动效 · 逐帧预演（2026-10-24 六六：「都做 gif 我挑一下吧，用 gba 为例子」；
//                               2026-10-25「有稍微晃动或者摇摆之类的吗」）
// ═══════════════════════════════════════════════════════════════════════════════════════════

/// 一帧要施在中间那张卡上的待机动效。
///
/// 全部是**几何与 alpha** 的改动，不碰光栅化 —— 将来真机实现走同一条路
/// （`app.rs` 每帧重建的 `Draw` 列表里就是这些 quad，转角走 `Draw::Turned`）。
#[derive(Copy, Clone, PartialEq, Eq)]
enum Idle {
    // ── 平移 / 缩放一组 ──────────────────────────────────────────────────────────────
    /// 呼吸缩放：以卡带**脚线中点**为锚点等比缩放。
    Squash,
    /// 微浮动：整卡在 0..-2px 之间升降。
    Float,
    /// 柔光同步：脚下那池柔光按 `light::breathe` 的节奏亮暗。
    Glow,
    /// 高光扫过：一条斜向亮带横掠卡面，用卡带自己的 alpha 剪出来。
    Shine,
    /// 组合：呼吸 + 浮动 + 柔光同相。
    Combo,

    // ── 摆动 / 晃动一组（2026-10-25 六六：「有稍微晃动或者摇摆之类的吗」）────────────
    /// 左右轻摆：绕卡带**中心**转，像一张立在桌上的卡片被轻轻拨了一下。
    Sway,
    /// 挂摆：绕**脚线中点**转，像牌匾吊在钉子上被风吹。角比 `Sway` 大一点，
    /// 但上缘的位移不比它多 —— 因为支点在脚下，越往上摆幅越大，那正是"挂着"的样子。
    Hang,
    /// 被碰一下：快速摆两下再收住（指数衰减的旋转），静置久了来一次。
    Tap,
    /// 弹性挤压：宽高**非等比**交替（宽 +0.6% 时高 −0.6%），Socket 那种 squash & stretch。
    Jelly,
}

impl Idle {
    fn from_env(s: &str) -> Option<Idle> {
        match s {
            "squash" => Some(Idle::Squash),
            "float" => Some(Idle::Float),
            "glow" => Some(Idle::Glow),
            "shine" => Some(Idle::Shine),
            "combo" => Some(Idle::Combo),
            "sway" => Some(Idle::Sway),
            "hang" => Some(Idle::Hang),
            "tap" => Some(Idle::Tap),
            "jelly" => Some(Idle::Jelly),
            _ => None,
        }
    }

    /// 0..1 的平滑相位，两端都在 0，所以首尾帧能接上（循环不跳）。
    fn s(phase: f32) -> f32 {
        0.5 - 0.5 * (std::f32::consts::TAU * phase).cos()
    }

    /// 这一相位上的**非等比**缩放 `(sx, sy)`。
    fn scale(self, phase: f32) -> (f32, f32) {
        let s = Self::s(phase);
        match self {
            Idle::Squash => (1.0 + 0.008 * s, 1.0 + 0.008 * s),
            Idle::Combo => (1.0 + 0.006 * s, 1.0 + 0.006 * s),
            // 弹性：一个循环里压两次，宽度涨的时候高度收 —— 体积感守恒，才像塑料不像气球。
            Idle::Jelly => {
                let k = (std::f32::consts::TAU * 2.0 * phase).sin() * 0.006;
                (1.0 + k, 1.0 - k)
            }
            _ => (1.0, 1.0),
        }
    }

    /// 这一相位上的竖直偏移（像素，负 = 向上）。
    fn lift(self, phase: f32) -> f32 {
        let s = Self::s(phase);
        match self {
            Idle::Float => -2.0 * s,
            Idle::Combo => -1.4 * s,
            _ => 0.0,
        }
    }

    /// 这一相位上的转角（弧度）。正 = 顺时针（y 向下，见 `Draw::Turned` 的注释）。
    ///
    /// 幅度都在 1° 上下：**"稍微"**是六六的原话。1° 在 360 px 宽的卡上，
    /// 边缘位移约 3 px —— 看得出来，但不会读成"它在转"。
    fn turn(self, phase: f32) -> f32 {
        let deg = std::f32::consts::PI / 180.0;
        match self {
            Idle::Sway => 0.9 * deg * (std::f32::consts::TAU * phase).sin(),
            Idle::Hang => 1.3 * deg * (std::f32::consts::TAU * phase).sin(),
            // 被碰一下：2.6 个来回的摆动，外面套一层 `s(phase)` 让**两端都归零**。
            //
            // ⚠️ 第一版用的是 `e^{-4·phase}`：首帧就是最大摆幅，循环回接时会「啪」地跳回去
            //    （GIF 上尤其明显）。包络取 `s()` 之后是「起身 → 晃两下 → 收住」，首尾都在 0。
            Idle::Tap => {
                let env = Self::s(phase);
                1.6 * deg * env * (std::f32::consts::TAU * 2.6 * phase).sin()
            }
            _ => 0.0,
        }
    }

    /// 支点在脚线上而不是中心（挂摆需要）。其余绕自己的中心转。
    fn pivots_at_foot(self) -> bool {
        matches!(self, Idle::Hang)
    }

    /// 脚下柔光池的强度倍数。
    fn pool_gain(self, phase: f32) -> f32 {
        let ms = (phase * slot_ui::BREATHE_MS as f32) as u32;
        match self {
            Idle::Glow => slot_ui::light_breathe(ms),
            Idle::Combo => 0.5 + 0.5 * slot_ui::light_breathe(ms),
            _ => 1.0,
        }
    }
}

/// 一条用卡带自身 alpha 剪出来的高光带，尺寸与卡面贴图一致。
fn shine_face(phase: f32) -> Option<CartFace> {
    let alpha = slot_ui::cart_alpha_for(System::Gba)?;
    let size = size_for(System::Gba);
    let (w, h) = (size.face_w(), size.face_h());
    let mask = alpha.resized(w, h);
    let centre = -0.35 + 1.7 * phase;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let v = y as f32 / h as f32;
        for x in 0..w {
            let u = x as f32 / w as f32;
            let d = (u + 0.35 * v - centre).abs();
            let band = (1.0 - (d / 0.17).min(1.0)).powi(2) * 0.16;
            let a = band * mask[(y * w + x) as usize] as f32 / 255.0;
            let i = ((y * w + x) * 4) as usize;
            rgba[i] = 255;
            rgba[i + 1] = 255;
            rgba[i + 2] = 255;
            rgba[i + 3] = (a * 255.0).round() as u8;
        }
    }
    Some(CartFace { rgba, w, h })
}

/// 找到中间那张卡的贴图，以及它所在的 quad。
///
/// 判据是几何而不是顺序：720 宽的屏上，**中心在 360、宽在 300..420** 的那一张就是主卡
/// （侧卡 240 宽、中心在 0 / 720；桌面图 720 宽；柔光池更宽但中心同样在 360 —— 所以卡宽度）。
fn middle_cart(list: &[Draw]) -> Option<(usize, f32, f32, f32, f32)> {
    list.iter().enumerate().find_map(|(i, d)| match *d {
        Draw::Tex { x, y, w, h, .. } | Draw::Turned { x, y, w, h, .. } => {
            let centred = (x + w / 2.0 - OUT_W as f32 / 2.0).abs() < 1.5;
            (centred && (300.0..420.0).contains(&w)).then_some((i, x, y, w, h))
        }
        _ => None,
    })
}

fn cart_tex_of(d: &Draw) -> TexId {
    match *d {
        Draw::Tex { tex, .. } | Draw::Turned { tex, .. } => tex,
        _ => unreachable!(),
    }
}

/// `LOOK_ANIM=gif LOOK_ANIM_KIND=sway LOOK_ANIM_DIR=<dir>` ⇒ 出 24 帧 PNG。
#[test]
fn anim_gif_frames() {
    if std::env::var("LOOK_ANIM").is_err() {
        return;
    }
    let kind = std::env::var("LOOK_ANIM_KIND").unwrap_or_else(|_| "combo".into());
    // `swayglow` 不是这里的 `Idle` 变体：它走**真机路径**（`slot_ui::Idle::at` + `draw_row_idle`），
    // 下面那个分支自己处理，这里只是别让它被当成未知 kind 拒掉。
    let Some(idle) = Idle::from_env(&kind)
        .or_else(|| (kind == "swayglow").then_some(Idle::Sway))
    else {
        panic!("unknown LOOK_ANIM_KIND: {kind}");
    };
    let frames: usize = std::env::var("LOOK_ANIM_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);
    let dir = PathBuf::from(std::env::var("LOOK_ANIM_DIR").expect("LOOK_ANIM_DIR"));
    std::fs::create_dir_all(&dir).unwrap();

    // ★ 卡上素材要**自己装**（`render_looks` 那条测试装的那份不会自动继承）。
    //   不装就只有内建路径 —— 而卡带 SVG 已删，那会画出"一块平板"。
    if let Ok(p) = std::env::var("LOOK_DEPLOY") {
        slot_ui::install_cart_art(slot_ui::load_cart_art(Path::new(&p)));
    }

    for f in 0..frames {
        let phase = f as f32 / frames as f32;

        // ★★ 2026-10-25（六六选定）：「1 sway，幅度再大一点点，叠加柔光」——
        //    `swayglow` **不走后处理**，直接调真机的 `Idle::at()` + `draw_row_idle`，
        //    所以这张 GIF 就是上机后看到的那一帧，一个参数都不差。
        if kind == "swayglow" {
            let now = (phase * slot_ui::BREATHE_MS as f32) as u32;
            let s = shelf_screen_idle(System::Gba, slot_ui::Idle::at(now));
            s.png(&dir.join(format!("{kind}-{f:03}.png")));
            continue;
        }

        let mut s = shelf_screen(System::Gba);
        let Some((idx, x, y, w, h)) = middle_cart(&s.list) else {
            panic!("没有找到中间那张卡（{kind}）");
        };

        // 脚下的柔光池：真机由 `shelf.set_focus` 提供，离机渲染器一直没装，这里补上。
        let pool_tex = {
            let (pw, ph, buf) = slot_ui::pool(480, 20);
            s.mint(CartFace {
                rgba: buf,
                w: pw,
                h: ph,
            })
        };
        s.list.insert(
            idx,
            Draw::Tex {
                x: x - (w * 1.25 - w) / 2.0,
                y: y + h - 10.0,
                w: w * 1.25,
                h: 20.0,
                tex: pool_tex,
                alpha: slot_ui::light_gain() * idle.pool_gain(phase),
            },
        );

        // 卡带自己：先算缩放后的尺寸，再按"支点"决定转角把它的中心挪到哪。
        let i = idx + 1;
        let tex = cart_tex_of(&s.list[i]);
        let (sx, sy) = idle.scale(phase);
        let lift = idle.lift(phase);
        let turn = idle.turn(phase);
        let (nw, nh) = (w * sx, h * sy);
        let foot = y + h + lift;
        let cx = x + w / 2.0;
        let centre = if turn == 0.0 {
            (cx, foot - nh / 2.0)
        } else if idle.pivots_at_foot() {
            // 绕脚线中点转：中心相对支点的向量 (0, -nh/2) 转过去。
            let (vx, vy) = (0.0, -nh / 2.0);
            let (c, sn) = (turn.cos(), turn.sin());
            (cx + vx * c - vy * sn, foot + vx * sn + vy * c)
        } else {
            (cx, foot - nh / 2.0)
        };
        s.list[i] = if turn == 0.0 {
            Draw::Tex {
                x: centre.0 - nw / 2.0,
                y: centre.1 - nh / 2.0,
                w: nw,
                h: nh,
                tex,
                alpha: 1.0,
            }
        } else {
            Draw::Turned {
                x: centre.0 - nw / 2.0,
                y: centre.1 - nh / 2.0,
                w: nw,
                h: nh,
                tex,
                alpha: 1.0,
                turn,
            }
        };

        // 高光带压在卡带之上、桌面图之下。
        if idle == Idle::Shine {
            if let Some(face) = shine_face(phase) {
                let tex = s.mint(face);
                let (qx, qy, qw, qh) = match s.list[i] {
                    Draw::Tex { x, y, w, h, .. } => (x, y, w, h),
                    _ => unreachable!(),
                };
                s.list.insert(
                    i + 1,
                    Draw::Tex {
                        x: qx,
                        y: qy,
                        w: qw,
                        h: qh,
                        tex,
                        alpha: 1.0,
                    },
                );
            }
        }

        s.png(&dir.join(format!("{kind}-{f:03}.png")));
    }
    eprintln!("wrote {frames} frames of {kind} into {}", dir.display());
}
