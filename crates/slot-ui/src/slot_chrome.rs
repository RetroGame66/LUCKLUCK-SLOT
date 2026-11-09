use std::sync::OnceLock;

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};
use slot_store::{System, Theme};

use crate::cart::{CartSize, CART_W};
use crate::desktop::{SLOT_OPEN_H, SLOT_OPEN_W, SLOT_OPEN_X, SLOT_OPEN_Y};
use crate::icon::icon_box;
use crate::palette;
use crate::shelf::foot_y;

/// Big enough to read as a symbol on a 240 px cart rather than as a mark on its label.
pub const ALERT_PX: f32 = 44.0;

/// The bottom of the device. Deep enough to read as the cart bay it is: a 40px line under a
/// 135px cart was a detail the cart towered over, and the insert only touched it for the
/// last seventh of the travel.
pub const MOUTH_H: f32 = 58.0;
const BAND_Y: f32 = OUT_H as f32 - MOUTH_H;

/// The opening. It is the one piece of the slot that sits *behind* the cart: it is the
/// inside of the machine, so a cart on its way through fills it. Everything else is plastic
/// and draws in front, which is what cuts the cart off.
pub const MOUTH_W: f32 = CART_W as f32 + 14.0;
const SLIT_H: f32 = 9.0;
const MOUTH_X: f32 = (OUT_W as f32 - MOUTH_W) / 2.0;
const SLIT_Y: f32 = BAY_Y + 5.0;

/// The lit front edge of the slot: the line the cart is cut off at.
pub const LIP_H: f32 = 2.0;

/// The bay the slot sits in, stepped down from the outer shell and wider than the opening.
const BAY_W: f32 = MOUTH_W + 18.0;
const BAY_X: f32 = (OUT_W as f32 - BAY_W) / 2.0;
const BAY_Y: f32 = BAND_Y + LIP_H;
/// The thumb scoop: one broad arc across the middle of the bay's near wall, which is how you
/// get hold of a cart to pull it out. Not two notches at the ends, which is what was here
/// before and is not what an SP has. It runs nearly the whole opening.
const SCOOP_W: f32 = MOUTH_W * 0.88;
const SCOOP_D: f32 = RECESS_H - (SCOOP_Y - BAY_Y);
const SCOOP_Y: f32 = SLIT_Y + SLIT_H;

/// How deep you can see into the slot, and so how much of a seated cart shows in it. Set just
/// past the cart's own label inset, so a seated cart shows its moulded grip and the top edge
/// of its label through the thumb scoop, and nothing readable.
const RECESS_H: f32 = 42.0;
/// The lit edge of the plastic where it is cut away for the scoop.
const RIM_W: f32 = 2.0;
const CX: f32 = OUT_W as f32 / 2.0;
/// Flatness of the arc through the middle. An ellipse bottoms out in a curve where the real
/// scoop runs almost level and then turns up hard at the ends.
const SCOOP_FLAT: f32 = 4.0;

/// ★ 2026-10-22（六六）：机种 logo 高光区在卡座背层的位置。
///
/// 卡座内侧洞（`draw_slot_inside`：开口/凹腔/缝/拇指弧）画完后、卡带之前画 logo ——
/// 空槽时 logo 可见，卡带一插就盖住它（插卡动画进行中，卡带顶端还没沉到底，logo 露出）。
/// 水平居中、垂直在底条（bay）的上段。**上机实测再微调**。
pub const LOGO_BOX: (f32, f32, f32, f32) = (CX - 70.0, BAND_Y + 2.0, 140.0, 28.0);

/// ★ 2026-10-22（六六）：logo 高光区的洋红标记色，与 `cart_art::is_mark` 同判据。
///
/// 素材 PNG 里洋红 = "这块要被前端填主题色"。`fill_logo_highlight` 在上传时把洋红换成
/// 当前主题的 `bright` 色 —— 换主题时 `rebake()` 重新上传，高光色跟着走。
const LOGO_MARK_R: u8 = 200;
const LOGO_MARK_G: u8 = 60;
const LOGO_MARK_B: u8 = 200;

/// 把 logo RGBA 里的洋红高光区填成 `colour`，原地操作。
///
/// 返回替换了多少像素 —— 0 说明素材没有洋红标记（没有高光区），前端可以据此决定
/// 是否还要传这个 logo（没标记的 logo 就是一张普通贴图，不发光）。
pub fn fill_logo_highlight(rgba: &mut [u8], w: u32, h: u32, colour: [u8; 3]) -> usize {
    let mut n = 0;
    let len = (w as usize * h as usize) * 4;
    if rgba.len() < len {
        return 0;
    }
    for i in 0..(w as usize * h as usize) {
        let p = i * 4;
        if rgba[p + 3] > 0
            && rgba[p] > LOGO_MARK_R
            && rgba[p + 1] < LOGO_MARK_G
            && rgba[p + 2] > LOGO_MARK_B
        {
            rgba[p] = colour[0];
            rgba[p + 1] = colour[1];
            rgba[p + 2] = colour[2];
            n += 1;
        }
    }
    n
}

/// 同一套判据（`LOGO_MARK_*`）的**只留洋红**版本：非洋红像素的 alpha 全部归零。
///
/// ★ 2026-10-27（六六）：街机箱体 `SLOT_LOAD_*.png` 的洋红区要**发光**，而
/// `glow_face_spread` 是拿**整张图的 alpha** 去糊的 —— 直接喂箱体，光会跟着箱体外形走一圈，
/// 不是"屏幕/招牌亮起来"。所以先切一张只有洋红的遮罩，再拿它烘光；本体那一步照旧走
/// `fill_logo_highlight` 换成本主题亮色。两条用同一个判据，不许另写一套。
pub fn mark_only(rgba: &[u8], w: u32, h: u32) -> (Vec<u8>, usize) {
    let len = (w as usize * h as usize) * 4;
    let mut out = vec![0u8; len];
    if rgba.len() < len {
        return (out, 0);
    }
    let mut n = 0;
    for i in 0..(w as usize * h as usize) {
        let p = i * 4;
        if rgba[p + 3] > 0
            && rgba[p] > LOGO_MARK_R
            && rgba[p + 1] < LOGO_MARK_G
            && rgba[p + 2] > LOGO_MARK_B
        {
            out[p] = rgba[p];
            out[p + 1] = rgba[p + 1];
            out[p + 2] = rgba[p + 2];
            out[p + 3] = rgba[p + 3];
            n += 1;
        }
    }
    (out, n)
}

/// The palette, from `System/theme.txt` if the card carries one. Set once at boot and read
/// every frame after, because the slot is drawn from four screens and threading a theme
/// through all of them buys nothing: it cannot change while the device is on.
static THEME: OnceLock<Theme> = OnceLock::new();

/// Once, at boot. A second call is ignored rather than fought over: the card is read once and
/// there is no screen that changes this.
pub fn set_theme(theme: Theme) {
    let _ = THEME.set(theme);
}

pub fn theme() -> &'static Theme {
    THEME.get_or_init(Theme::default)
}

fn rgb(c: [u8; 3]) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ]
}

/// One surface of the case, in whichever palette the device is in.
///
/// Three answers, and the order they are asked in is the whole of the rule. **A look owns the
/// case outright**: it is a colouring of the whole machine, plastics included, and painting the
/// card's own `theme.txt` over the top of it would be the one composition the key could not
/// survive — half a Game Boy and half a black SP. So a look is asked first and wins. Otherwise
/// the card's `theme.txt` is read once into `THEME` and is the *dark* case; the light case is
/// the built-in one. Every accessor below goes through here, so there is exactly one place
/// that knows a mode exists and no way for one band of the case to be left behind when it
/// changes — which is the failure a per-band `if` would produce eventually.
fn surface(pick: fn(&Theme) -> [u8; 3]) -> [f32; 4] {
    if palette::look() != 0 {
        return rgb(pick(&palette::look_case()));
    }
    match palette::mode() {
        palette::Mode::Dark => rgb(pick(theme())),
        palette::Mode::Light => rgb(pick(&palette::LIGHT_CASE)),
    }
}

/// The case. Every band has to clear its neighbour, which `slot-ui/tests/contrast.rs` holds
/// for both palettes. A theme is the card's own business past that.
pub fn housing() -> [f32; 4] {
    surface(|t| t.housing)
}

pub fn opening() -> [f32; 4] {
    surface(|t| t.opening)
}

pub fn edge() -> [f32; 4] {
    surface(|t| t.edge)
}

/// 拟真高光的**对比度带**：上下限都取自默认深色壳自己的那一档。
///
/// 默认主题 `0x24` 壳 / `0x4d` 边 = **1.86:1**，而这正是六六说"恰好"的那个手感。
/// 问题出在新增的主题色板上：一个 look 拥有整台机器的配色（见 `surface` 的注释），
/// 它的 `edge` 可以是鲜亮的绿、粉、橙 —— 对比度冲到 3:1 以上，那三条线
/// （顶条字母轮上缘、插卡位上唇、卡座底缘的溢光）就开始**像画上去的边框**，
/// 不像塑料受光的那一道。所以这里把 `edge` 的色相留下，把它的明度夹回默认那一档：
///
/// * 太亮 → 往壳里退，退到刚好压回 `RIM_HI`；
/// * 太闷 → 往白里提，提到刚好够 `RIM_LO`；
/// * 落在带内 → 一个字都不改，原样用。
///
/// **色相不动、只动明度**，所以一个 GBC 绿壳的机器仍然有一条绿的高光线，
/// 只是那条线的明暗跟灰壳的机器是同一档。这是"拟真"和"主题"两件事的分工。
const RIM_HI: f32 = 1.86;
const RIM_LO: f32 = 1.25;

/// 三个拟真高光的落点。都走 `rim()`，不走 `edge()`：
/// 顶条字母轮上缘、插卡位上唇、卡座底缘的溢光。
pub fn rim() -> [f32; 4] {
    let housing = bytes(housing());
    let e = bytes(edge());
    let now = palette::contrast(e, housing);
    let c = if now > RIM_HI {
        // 太亮：往壳里退。
        bisect(e, housing, housing, RIM_HI)
    } else if now < RIM_LO {
        // 太闷：往白里提。
        bisect(e, housing, [255, 255, 255], RIM_LO)
    } else {
        e
    };
    rgb(c)
}

/// `[f32;4]` 读回 `[u8;3]`。本文件自己有一份而不是调 `palette` 的，是因为那边那份是
/// 私有的，而这里只用到一次。
fn bytes(c: [f32; 4]) -> [u8; 3] {
    [
        (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[2].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn mix3(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t).round() as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t).round() as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t).round() as u8,
    ]
}

/// 在 `from`（= look 的 edge）和 `to`（= 壳色，或白）之间找一个混合比，
/// 使混合色对壳色的对比度正好落在 `want`。
///
/// `now` 是调用方已经算出来的 `contrast(from, housing)`，`want` 一定在
/// `contrast(from, housing)` 和 `contrast(to, housing)` 之间 —— 调用方分了两条路，
/// 所以二分一定收敛，不需要判"无解"。
fn bisect(from: [u8; 3], housing: [u8; 3], to: [u8; 3], want: f32) -> [u8; 3] {
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..20 {
        let t = (lo + hi) * 0.5;
        if palette::contrast(mix3(from, to, t), housing) < want {
            lo = t;
        } else {
            hi = t;
        }
    }
    mix3(from, to, (lo + hi) * 0.5)
}

/// The floor of the bay: a step down from the shell, not a second opening. Subtle on purpose,
/// since it is a moulding line rather than something to read.
pub fn recess() -> [f32; 4] {
    surface(|t| t.recess)
}

/// How far the cart rises before the push, in pixels.
///
/// ★ 2026-10-24（六六）：「横版卡带的插卡动画改为**向上拉一小段**然后**加速往下插**」。
/// 这一下不是为了好看：卡带从货架正中一路下坠，没有它只是一块图往下滑；先提起来再撒手，
/// 读起来才是"插"。
///
/// ★★ 2026-10-24 20:0x（六六）：「**GBA/FC/SFC 没有往上提再加速插入的观感**」。
/// 26 px 在 480 的面板上只有 5%，落在 291 px 的下坠里是 9% —— 数字上存在、眼睛看不见。
/// 抬到 **40 px**（8.3% 面板 / 14% 行程），并把抬头那一段**末段留一拍不动**
/// （`LIFT_FILL`），"提一下"才成为一个动作而不是下坠的起手。
pub const PULL_UP: f32 = 40.0;

/// The lift's share of the travel. Everything after it is the fall.
///
/// 0.38 是权衡出来的：`INSERT_S` 是 0.73 s、其中 `SEATED_AT` 之前只有 0.45 s 在动，
/// 抬得太久就剩不下足够的时间给"加速"—— 抬 0.38 之后落还有 0.28 s，60 fps 下 17 帧，
/// 位移按 `u²` 走，最后几帧才有速度感。
pub const PULL_SHARE: f32 = 0.38;

/// How much of the lift is spent moving. The rest of it is a still beat at the top.
///
/// 0.84：抬头用掉这一段的 84%，剩下 16%（约 1.5 帧）停在顶上。速度曲线
/// （`ease` = smootherstep）两端本来就是零速度，所以那一拍不靠曲线也停得住 ——
/// 它买的是"停稳了再撒手"的读感：没有这一拍，抬头和下坠是一条连续的弧，眼睛只会读成
/// "往下插了一下"。
const LIFT_FILL: f32 = 0.84;

/// Where a cart of each kind comes to rest, as the share of **its own height** that ends up
/// below the top edge of the opening.
///
/// ★ 2026-10-24（六六）钉的三个数：
/// - **GBA 完全插入** = 卡带贴图的上缘与插卡位平齐 ⇒ **1.00**（这是唯一能让 1.00 正好等于
///   "上缘齐平"的定义，所以比例一律按卡带自己的高度算）；
/// - **GB/GBC 插入 50%**；
/// - **FC/SFC 也是 50%**（★ 2026-10-24 20:0x 六六改口：原来 FC 只插 20%，SFC 借 FC 的 20%，
///   现在两个一起到 50% —— 「GB/GBC 和 FC/SFC 都插入 50% 吧」）。
const GB_DEPTH: f32 = 0.50;
const FIRM_DEPTH: f32 = 0.50;

/// How deep the slot takes a thing, and what it does on the way down.
///
/// Two ways rather than one, because the recess is deep enough for an Advance cart and not for a
/// Game Boy one: a Game Boy cart stands half out of an Advance slot on a real machine, with its
/// label still readable, and the drawing has always said so.
///
/// A shape of its own rather than a `System` asked at the point of use, so the chrome never has
/// to know what a machine is. That is what lets something which is not a cartridge go into this
/// slot at all.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Seat {
    /// All the way home: the cart's **top edge** comes level with the top edge of the opening,
    /// so the whole of it is below the line and only what shows through the hole is left.
    Swallowed,
    /// Part of the way in, by the share of the cart's own height that ends up below that line.
    Partly(f32),
}

impl Seat {
    /// The share of the cart's own height that ends up below the top edge of the opening.
    pub fn depth(self) -> f32 {
        match self {
            Seat::Swallowed => 1.0,
            Seat::Partly(d) => d.clamp(0.0, 1.0),
        }
    }
}

impl From<System> for Seat {
    fn from(system: System) -> Self {
        match system {
            System::Gba => Seat::Swallowed,
            System::Gb | System::Gbc => Seat::Partly(GB_DEPTH),
            // 六六的口径是「动画与 GBA 一致，但只插一半」。SFC / NeoGeo / 街机共用同一个横版
            // 盒子，还没有各自的落位数字，先跟着 FC 走 —— 要给谁另一档，改的就是这一行。
            // ★ 2026-10-25：MD / PS1 也在这里，理由相同 —— 两者都没有自己的落位数字，
            //   而且都还在用那个横版占位盒（`cart::size_for`）。**PS1 尤其只是占位**：
            //   真实的 PS1 是 CD 盒，本不该进这个卡槽（`Seat` 说的是"卡带的洞"），
            //   真做对了以后它要么有自己的形状，要么根本不走 `SlotChrome`。
            System::Sfc
            | System::Fc
            | System::Neogeo
            | System::Arcade
            | System::Md
            | System::Ps1
            | System::Pce => Seat::Partly(FIRM_DEPTH),
        }
    }
}

pub struct SlotChrome {
    /// The shape going into the slot, and its size: an Advance cart is landscape and a Game Boy
    /// cart portrait, and the two are not the same object going into the same slot. Carried
    /// rather than derived from the cart, so this path never has to ask what a cart is.
    pub size: CartSize,
    /// How far down this slot takes it.
    pub seat_kind: Seat,
    /// The colour the stand-in wears before its face lands — the same ink the row gives the
    /// cart, so the thing sliding in does not change colour on the way down.
    pub ink: [u8; 3],
    pub face: Option<TexId>,
    /// 0.0 standing where the shelf left it; 1.0 at the depth `seat_kind` gives it, which is
    /// the top edge of the opening for an Advance cart and a share of its own height for the
    /// rest. The travel between them lifts first and then falls — see `draw`.
    pub seat: f32,
    /// The refusal symbol and how far into its fade it is. A cart that will not seat says so
    /// this way: it is right there and can carry a mark, where a jitter beside the slot reads
    /// as a rendering fault. `None` once the binary has nothing to say, or before it has
    /// uploaded the glyph.
    pub alert: Option<(TexId, f32)>,
    /// Alpha of the black veil over the layer behind: the shelf on the way in, the live
    /// game on the way out.
    pub dim: f32,
    /// How far up the screen behind the slot is, 0.0 dark and 1.0 fully on. The housing is
    /// solid over a dark screen and gone over a lit one, so the picture is never left with
    /// a black bar across the bottom of it.
    pub screen: f32,
    /// Whether there is a picture to show at all. False while the core is still loading,
    /// where the game texture still holds whatever the last cart left in it.
    pub game: bool,
}

impl SlotChrome {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        let seat = self.seat.clamp(0.0, 1.0);
        let dim = self.dim.clamp(0.0, 1.0);
        if dim > 0.0 {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                colour: [0.0, 0.0, 0.0, dim],
            });
        }

        let chrome = 1.0 - self.screen.clamp(0.0, 1.0);

        // ★★ 2026-10-24：**卡座不再由这里画**。插卡位现在是货架框桌面图自己挖出来的那个洞
        //   （`desktop::desktop_rgba` 的 360×44 开口），`App::draw` 把整张桌面图盖在卡带之上，
        //   卡带于是从洞口里沉下去。这里再画一遍旧底条，屏幕上就有两个插卡位。

        // ── The travel ──────────────────────────────────────────────────────────────────────
        //
        // ★★ 2026-10-24（六六）：**动画改绑到桌面图自己的插卡位**。旧的底条 —— `BAND_Y` 那一条
        //   254×58 的塑料、`SEATED_Y`、`GB_INSET`、以及"卡在唇上"的 `catch_at` —— 一律不再
        //   参与插卡。原因是他真机上看到的那条：横版卡带已收束为 **360 宽**，而插卡位开口
        //   也正好是 360（`desktop::SLOT_OPEN_W`）。旧动画是照 240 宽的卡带和 254 宽的洞写
        //   的，两套几何一对不上，卡带就既插不准、又压不住。
        //
        //   现在只有两段：
        //   ① **向上拉一小段**（`PULL_UP`）；GB/GBC 在这一段里把宽度长到 360（高度随之等比
        //      到 414，不是压扁 —— 竖版卡带拉宽只能是等比放大）；
        //   ② **撒手往下插**，位移按 `u²`：从静止开始加速，落到触点是它最快的一刻。
        //
        //   起点必须是货架画它的那个矩形，一个像素都不许差 —— 差一点，按下 A 的那一帧就跳。
        let size = self.size;
        // 落位宽度 = 插卡位宽度。横版 240 → 360（×1.5），竖版 240 → 360（×1.5，高 276→414）。
        let s1 = SLOT_OPEN_W / size.w as f32;
        let h1 = size.h as f32 * s1;
        // 落位高度：下缘在开口顶沿之下 `depth` 个自身高度处。
        // `depth` 1.0 ⇒ 下缘 = 顶沿 + 全高 ⇒ **上缘正好与开口顶沿平齐**（GBA 的"完全插入"）。
        let y1 = SLOT_OPEN_Y + self.seat_kind.depth() * h1 - h1;
        // 货架上那个矩形：中间卡按 `size.middle` 放大，脚踩 `foot_y`（与 `Shelf::slot_rects`
        // 同一个函数，所以两边不可能对不上）。
        let s0 = size.middle;
        let h0 = size.h as f32 * s0;
        // ★★ 2026-10-24 20:0x（六六）：「**GB/GBC 的放大后插入看起来不太流畅，像是突然变大了**」。
        //
        //   根因是锚点：卡带从 `s0` 长到 `s1` 时，`y` 只按 `foot_y - h0 - PULL_UP*u` 走，
        //   **高度却在长**，于是下缘 = y + h 变成 `foot_y - PULL_UP*u + (h(u) - h0)`。
        //   竖版卡带 300 → 414 高，下缘因此在抬头那一段里往下窜 **114 - 40 = 74 px**，
        //   一路冲过 480 的屏幕底 —— 眼睛里看到的就是"卡带突然长大并且糊下去"，
        //   而不是"被拎近了"。
        //
        //   改成**绕中心放大**：中心 `c` 按抬头量上移、高度在中心两侧同时长，下缘只跟着走
        //   `PULL_UP/2`，上缘上移 `(h1-h0)/2 + PULL_UP/2`。四个方向一起长，才是"凑近镜头"。
        //   GBA/FC/SFC 的 `s0 == s1`（货架中间那盘已经是 1.5），这一改对它们零影响。
        let c0 = foot_y(size) - h0 / 2.0;
        // 抬头到底时的样子：全尺寸、中心在 `c0 - PULL_UP`。两段共用这一个点，交接无台阶。
        let top_hold = c0 - PULL_UP - h1 / 2.0;
        let (cart_scale, y) = if seat < PULL_SHARE {
            // 抬头：位移与放大走同一条 `ease`，并在 `LIFT_FILL` 处就到顶 —— 剩下的是一拍静止。
            let u = ((seat / PULL_SHARE) / LIFT_FILL).clamp(0.0, 1.0);
            let e = ease(u);
            let s = s0 + (s1 - s0) * e;
            let c = c0 - PULL_UP * e;
            (s, c - size.h as f32 * s / 2.0)
        } else {
            let u = ((seat - PULL_SHARE) / (1.0 - PULL_SHARE)).clamp(0.0, 1.0);
            let drop = u * u;
            (s1, top_hold + (y1 - top_hold) * drop)
        };
        let cart_w = size.w as f32 * cart_scale;
        let cart_h = size.h as f32 * cart_scale;
        let x = (OUT_W as f32 - cart_w) / 2.0;
        // The cart fades with the case rather than through it. A seated cart is really in the
        // slot and has to be drawn, so the whole device face has to leave as one object as the
        // picture takes over. Held at full while the screen is off, which is all of the travel.
        let cart_alpha = if seat >= 1.0 { chrome } else { 1.0 };
        // ★★ 2026-10-27（六六）：「插卡和拔卡时也加上光效（跟 PS1 一样）」。
        //   画在卡带**之前** —— 光是洞里的，卡带压下来就把它盖住；顺序反过来就成了
        //   一块贴在卡带正面的发光纸片。货架上（`seat == 0`）它自己一个像素都不画。
        draw_cart_glow(out, seat);
        out.push(match self.face {
            Some(tex) => Draw::Tex {
                x,
                y,
                w: cart_w,
                h: cart_h,
                tex,
                alpha: cart_alpha,
            },
            None => {
                let c = self.ink;
                Draw::Rect {
                    x,
                    y,
                    w: cart_w,
                    h: cart_h,
                    colour: [
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0,
                        cart_alpha,
                    ],
                }
            }
        });

        // On the cart, so it goes behind the mouth with it: the alert leaves the way the
        // cart does rather than hanging in the opening after it.
        if let Some((tex, alpha)) = self.alert {
            let (w, h) = icon_box(ALERT_PX);
            let (w, h) = (w as f32, h as f32);
            out.push(Draw::Tex {
                x: x + (cart_w - w) / 2.0,
                y: y + (cart_h - h) / 2.0,
                w,
                h,
                tex,
                alpha,
            });
        }

        // After the cart and before the housing: the panel is the front surface of the
        // device, so a cart already in the slot is behind the picture the moment it lights.
        if self.game && self.screen > 0.0 {
            out.push(Draw::Game);
        }

        // ★★ 2026-10-24：遮挡卡带的那块塑料也归桌面图（见上）。这里一个像素都不画 ——
        //   旧的 `draw_slot_front` 会拿 254 宽的底条去压 360 宽的卡带。
    }
}

fn band(x: f32, y: f32, w: f32, h: f32, c: [f32; 4], alpha: f32) -> Draw {
    Draw::Rect {
        x,
        y,
        w,
        h,
        colour: [c[0], c[1], c[2], c[3] * alpha],
    }
}

/// 往白里提一档。只动亮度、不动色相 —— 暗色壳的琥珀边提起来还是琥珀，亮色壳的深边
/// 提起来是一道浅灰的高光，两边都读得成"光落在棱上"。
fn lighten(c: [f32; 4], k: f32) -> [f32; 4] {
    [
        c[0] + (1.0 - c[0]) * k,
        c[1] + (1.0 - c[1]) * k,
        c[2] + (1.0 - c[2]) * k,
        c[3],
    ]
}

/// ★★ 2026-10-21：**一条棱的受光，不是一根高亮胶带。**
///
/// 六六看完真机：「字母轮上方和插卡位共三处高亮色线条……看看怎么处理好」。这三处
/// （顶条字母轮上缘、插卡位上唇、卡座底缘的溢光）原本各是一条**等宽实色** `rim()` 的
/// 带子 —— 在 720×480 上它读起来像贴了一条发光胶带，不像塑料边缘接住光。
///
/// ★★ **第二版再改一次色**（六六：「把这里的颜色改为顶条和底条底色，然后加亮一点点」）：
/// 提亮**不再以 `rim()`／`edge()` 为底**，而以**顶条/底条自己的 `housing()`** 为底。
/// 那才是这块塑料的本色 —— 边缘受光不过是同一块塑料被照亮一点，色相不该换。
/// 拿主题的描边色当底，无论提多亮都还在说"这里有一条跟你壳色不同的线"。
///
/// 结构不变，仍是四层（棱心最亮、往两侧散），只是每一层都从 `housing` 提亮出来：
///
///   * **棱心**：`h` px，提 22% —— 「加亮一点点」，多一档就不是塑料而是金属了；
///   * **往塑料里**：两层，提 10% / 4%；
///   * **往开口里**：一层，提 15%（屏幕那边也接到一点）。
///
/// `into` 是塑料在哪一侧：`+1` 在下、`-1` 在上。三处各不一样 —— 顶条的塑料在线上方，
/// 卡座的塑料在唇下，拇指弧的塑料在弧下。
fn lit_edge(out: &mut Vec<Draw>, x: f32, y: f32, w: f32, h: f32, into: f32, alpha: f32) {
    const LIFT: f32 = 0.22;
    let base = housing();
    out.push(band(x, y, w, h, lighten(base, LIFT), alpha * 0.96));
    let s = if into >= 0.0 { 1.0 } else { -1.0 };
    out.push(band(x, y + s * h, w, 1.0, lighten(base, LIFT * 0.45), alpha * 0.55));
    out.push(band(x, y + s * (h + 1.0), w, 1.0, lighten(base, LIFT * 0.18), alpha * 0.22));
    out.push(band(x, y - s, w, 1.0, lighten(base, LIFT * 0.68), alpha * 0.32));
}

/// Everything you can see *into*: the bay floor, the opening, and the thumb scoop. All of it
/// draws behind the cart, because all of it is a hole. The scoop especially: it is a
/// cut-away in the near plastic, and the whole point of it is that you can see and grip the
/// cart through it. Painted in front, it was a dark arc lying on top of the cart.
fn draw_slot_back(alpha: f32, out: &mut Vec<Draw>) {
    // The top bar of the slot goes back here with the hole it spans. It is the front edge of
    // the case and a cart really does pass behind it, but at two pixels over a 240 px cart
    // all it does is draw a line across the label, and the cart reads as sliding behind a bar
    // rather than into an opening.
    out.push(band(BAY_X, BAND_Y, BAY_W, LIP_H, housing(), alpha));
    // ★ 2026-10-21：唇边不再是等宽实色，见 `lit_edge`。塑料在唇的下方。
    lit_edge(out, MOUTH_X, BAND_Y, MOUTH_W, LIP_H, 1.0, alpha);
    out.push(band(BAY_X, BAY_Y, BAY_W, RECESS_H, recess(), alpha));
    out.push(band(MOUTH_X, SLIT_Y, MOUTH_W, SLIT_H, opening(), alpha));
    for_each_scoop_span(|x, w, depth| {
        out.push(band(x, SCOOP_Y, w, depth, opening(), alpha));
    });
}

/// The plastic. Pieces around the hole, never over it: this is the only thing that occludes
/// the cart, and what it leaves uncovered is exactly the shape of the recess.
fn draw_slot_front(alpha: f32, out: &mut Vec<Draw>) {
    let w = OUT_W as f32;
    let right = BAY_X + BAY_W;
    let floor = SCOOP_Y + SCOOP_D + RIM_W;
    out.push(band(0.0, BAND_Y, BAY_X, MOUTH_H, housing(), alpha));
    out.push(band(right, BAND_Y, w - right, MOUTH_H, housing(), alpha));

    // Beside the arc, where the bay is wider than the scoop.
    let near = CX - SCOOP_W / 2.0;
    out.push(band(
        BAY_X,
        SCOOP_Y,
        near - BAY_X,
        floor - SCOOP_Y,
        housing(),
        alpha,
    ));
    let far = CX + SCOOP_W / 2.0;
    out.push(band(
        far,
        SCOOP_Y,
        right - far,
        floor - SCOOP_Y,
        housing(),
        alpha,
    ));

    // The arc itself: the plastic under the cut, then its lit edge over the top of it. The
    // highlight goes last so nothing is painted over it.
    for_each_scoop_span(|x, w, depth| {
        let top = SCOOP_Y + depth + RIM_W;
        out.push(band(x, top, w, floor - top, housing(), alpha));
    });
    out.push(band(0.0, floor, w, OUT_H as f32 - floor, housing(), alpha));
    for_each_scoop_span(|x, w, depth| {
        // ★ 2026-10-21：拇指弧的受光边，同 `lit_edge`。塑料在弧线下方。
        lit_edge(out, x, SCOOP_Y + depth, w, RIM_W, 1.0, alpha);
    });
}

/// The arc, walked by column and merged into spans of equal depth, so the hole and the
/// plastic beside it cannot drift apart.
///
/// Walking it by depth instead is what made it jagged: the trough is deliberately flat, so
/// half the arc's width shares its last pixel or two of depth, and the bottom of the curve
/// came out as one step a hundred pixels wide.
fn for_each_scoop_span(mut span: impl FnMut(f32, f32, f32)) {
    let hw = SCOOP_W / 2.0;
    let depth = |x: f32| SCOOP_D * (1.0 - (x.abs() / hw).powf(SCOOP_FLAT)).max(0.0);
    let mut x = -hw;
    while x < hw {
        let d = depth(x + 0.5).round();
        let start = x;
        while x < hw && depth(x + 0.5).round() == d {
            x += 1.0;
        }
        span(CX + start, x - start, d);
    }
}

// ---------------------------------------------------------------------------------------
// The top band: the same bay the other way up, with the letter strip in its window
// ---------------------------------------------------------------------------------------

/// The band the letter strip lives in, at the top of the screen. The same depth as the cart
/// bay, and the same width down to the pixel: what frames the letters is the machine's own
/// opening rather than a second piece of furniture invented for them, which is the whole of
/// why the strip reads as part of the device.
pub const TOP_BAND_H: f32 = MOUTH_H;

/// The window the letters show through: the bay's opening, mirrored. Its width is the
/// opening's and it stands on the same centre, so the strip above and the row below cannot
/// disagree about where the middle of the screen is.
pub const TOP_WIN_X: f32 = MOUTH_X;
pub const TOP_WIN_W: f32 = MOUTH_W;
pub const TOP_WIN_Y: f32 = 12.0;
pub const TOP_WIN_H: f32 = 36.0;

/// The well around it — the bay's recess, read the other way up — and so the depth the band
/// has over once the window has taken its share.
///
/// The well is not `RECESS_H`: the bay's opening is as deep as a thumb, because a hand has to
/// reach a cart's face through it, and this one is as deep as a line of letters. The window
/// sits in the well the way the opening sits in the recess, four pixels in from it.
const TOP_WELL_Y: f32 = 8.0;
const TOP_WELL_H: f32 = 44.0;

/// How far the light off a band reaches into the screen, how finely the falloff is cut, and how
/// bright it is where it starts.
///
/// The draw list is filled quads and nothing else — no gradient, no blur — so a glow is a stack
/// of one-pixel bands whose alpha steps down. Ten is fine enough that the steps do not read as
/// bands, which is the only thing that would give the trick away.
///
/// Faint on purpose, and it is worth saying why: this is the sheen of light spilling off a lit
/// edge onto the picture behind it, not a second edge. At any real strength it stops reading as
/// light and starts reading as a border someone drew on.
const GLOW_H: f32 = 10.0;
const GLOW_STEPS: usize = 10;
const GLOW_PEAK: f32 = 0.16;

/// The light that spills off one edge of the case onto the screen behind it.
///
/// `edge_y` is the boundary itself and `down` is which way the screen is from it: the band at
/// the top of the case throws its light downward, the bay at the bottom throws its upward, and
/// both are the same falloff read in opposite directions.
///
/// The falloff is squared rather than straight, because a linear ramp reads as a deliberate
/// gradient while a squared one reads as light: bright where it leaves the plastic, and gone
/// before the eye has followed it anywhere.
fn glow(out: &mut Vec<Draw>, edge_y: f32, down: bool) {
    let step = GLOW_H / GLOW_STEPS as f32;
    for i in 0..GLOW_STEPS {
        // One at the edge, zero at the end of the reach, and dimmer than that in between.
        let t = 1.0 - (i as f32 + 0.5) / GLOW_STEPS as f32;
        let alpha = GLOW_PEAK * t * t;
        if alpha <= 0.0 {
            continue;
        }
        // Stepped away from the edge, never onto it: the band's own lit edge is drawn already
        // and a glow across it would only wash it out.
        let y = if down {
            edge_y + i as f32 * step
        } else {
            edge_y - (i as f32 + 1.0) * step
        };
        out.push(band(0.0, y, OUT_W as f32, step, rim(), alpha));
    }
}

/// The light off both bands of the case: down from the top of the screen, up from the bay at
/// the bottom.
///
/// Both, and in one call, because they are one thing seen twice — the case is the same object
/// top and bottom, and the two boundaries are where it meets the picture. Drawn as the last
/// piece of the case's own furniture so a cart on its way into the slot is lit by it rather
/// than drawing over it.
pub fn draw_edge_glow(out: &mut Vec<Draw>) {
    glow(out, TOP_BAND_H, true);
    glow(out, BAND_Y, false);
}

/// ★ 2026-10-22（六六）：把机种 logo 画进卡座背层（卡带之下、内侧洞之上）。
///
/// 空槽时 logo 可见（高光区随主题亮着）；卡带一插就盖住它。
/// `tex` 由前端在上传时用 `fill_logo_highlight` 把洋红填成当前主题的 `bright` 色，
/// 所以这里只管画、不管色。
pub fn draw_slot_logo(tex: (slot_gfx::TexId, u32, u32), out: &mut Vec<Draw>) {
    let (bx, by, bw, bh) = LOGO_BOX;
    let (t, sw, sh) = tex;
    if sw == 0 || sh == 0 {
        return;
    }
    // Contain, not stretch: a logo that arrived a different shape sits inside the box rather
    // than being pulled to it.
    let k = (bw / sw as f32).min(bh / sh as f32);
    let (w, h) = (sw as f32 * k, sh as f32 * k);
    out.push(Draw::Tex {
        x: bx + (bw - w) / 2.0,
        y: by + (bh - h) / 2.0,
        w,
        h,
        tex: t,
        alpha: 1.0,
    });
}

/// The band at the top of the screen, drawn from the bay's numbers rather than from a second
/// set worked out for it.
///
/// Laid out as the bay is, read from the bottom up: the housing, the well stepped into it, the
/// opening inside that, and the lit edge of the cut at the top of the window — `RIM_W`.
///
/// **No lip, and that is a change from the bay rather than an omission.** The bay's own lower
/// boundary carries a lit edge — `LIP_H`, the line a cart is cut off at as it goes in — and the
/// band was first drawn with the same line under its window, on the argument that it was the
/// same moulding seen the other way up. It is not: the band is a lid, nothing passes through it,
/// and a second line two pixels above the band's edge read as a rule drawn under the letters
/// rather than as an edge of the case. The bay keeps its lip because a cart really is cut off
/// there.
///
/// One difference, and it is the reason this is a band and not a slot: nothing goes through
/// it. There is no depth behind the window and no shadow in it, because the letters are printed
/// on the inside of the opening and the band is a lid, so the whole of it is drawn flat.
pub fn draw_top_band(out: &mut Vec<Draw>) {
    out.push(band(0.0, 0.0, OUT_W as f32, TOP_BAND_H, housing(), 1.0));
    out.push(band(BAY_X, TOP_WELL_Y, BAY_W, TOP_WELL_H, recess(), 1.0));
    out.push(band(
        TOP_WIN_X,
        TOP_WIN_Y,
        TOP_WIN_W,
        TOP_WIN_H,
        opening(),
        1.0,
    ));
    // The lit edge last, so nothing is painted over the highlight — the order the bay uses.
    // ★ 2026-10-21：这条是六六说的"字母轮上方"那道 —— 同样是 `lit_edge`，塑料在它的上方。
    lit_edge(out, TOP_WIN_X, TOP_WIN_Y - RIM_W, TOP_WIN_W, RIM_W, -1.0, 1.0);
}

/// The slot with nothing going into it. The shelf shows it so the cart you pick has a
/// visible place to go, and so the bottom of the screen is the same object on every screen
/// rather than appearing only during the animation.
///
/// Drawn with the **inside** first and the **shell** last, so a cart that comes between the
/// two calls — the shelf's row of carts — slips *into* the opening rather than sitting in
/// front of a painted bar. Use `draw_slot_inside` / `draw_slot_shell` directly when a cart
/// has to stand in between; keep this whole-slot form only for a slot that holds nothing.
pub fn draw_empty_slot(out: &mut Vec<Draw>) {
    // Both halves. The recess is a hole in the front pieces, so a slot drawn from the front
    // alone is a hole onto the backdrop rather than an opening in a device.
    draw_slot_back(1.0, out);
    draw_slot_front(1.0, out);
}

/// The half of the slot that sits **behind** a cart: the opening, the recess, and the thumb
/// scoop — the machine's inside. A cart going into the slot is cut off by the shell, not by
/// this, so this half goes down *before* the cart the way `SlotChrome::draw` does it.
pub fn draw_slot_inside(out: &mut Vec<Draw>) {
    draw_slot_back(1.0, out);
}

/// The half of the slot that sits **in front of** a cart: the cut-away plastic, the pieces
/// around the hole that actually occlude it. This is the one thing a cart disappears behind,
/// so it goes down *after* the cart, and it is the bottom half of the machine's frame
/// (the top band is the other half, drawn by `draw_top_band`).
pub fn draw_slot_shell(out: &mut Vec<Draw>) {
    draw_slot_front(1.0, out);
}

/// Smootherstep. Zero velocity at both ends, so the two halves of the travel meet the catch
/// without a step in speed.
pub fn ease(u: f32) -> f32 {
    u * u * u * (u * (u * 6.0 - 15.0) + 10.0)
}

// ============================================================================
// PS1 光盘插入动画
// ============================================================================

/// 光盘直径（默认 300px）。
pub const DISC_DIA_DEFAULT: f32 = 300.0;

/// 光盘的**货架静止位置**（屏幕居中，选中卡片双向居中点 = `OUT_H/2 = 240`）。
/// ★ 2026-10-26（六六）：插入时光盘**从货架当前位置往下掉**（不从屏幕外飞入），
/// 弹出第三步也**弹回这里** —— 一进一出共享同一点。
pub const DISC_START_Y: f32 = OUT_H as f32 / 2.0;

/// 光盘插槽上沿 Y 坐标（与插卡位平齐）。
pub const DISC_SLOT_Y: f32 = 430.0;

// ── 吸入式（slot-loading）机制（★ 2026-10-26 六六定稿）─────────────────────
//
// 用户原话："舍弃托盘改为吸入式吧，选择游戏 → 光盘直接下落 → 光盘下缘碰到插卡位
// 底部时匀速下沉（像是被吸进去）直到光盘上缘与插卡位平齐 → 开始旋转并加速。"
//
// 参考机制：**slot-loading CD-ROM**（笔记本内置光驱那种"贴条式"吸入）。光盘不是
// 放上去、托盘推进去，而是**光盘本身被光驱的进给机构"吸进去"**，从插槽上沿消失。
//
// 三段动画：
//   1. **下落**（fall）：`θ = ½ a t²`，a = DISC_FALL_A。
//      结束时光盘底沿 = slot_bottom（`DISC_SLOT_Y + MOUTH_H`），
//      末速 `v_c = √(2 · a · d1)`，`d1 = 135 px`。
//   2. **吸进去**（suck-in）：**匀速** `v_c` 向下。
//      光盘从"底沿 = slot_bottom"沉到"上沿 = slot_top"，
//      即 cy 从 `slot_bottom - dia/2` 走到 `slot_top - dia/2`，**总位移 dia/2 = 150 px**。
//      这段时长 `d2 / v_c = 150 / v_c`，是"看起来在吃进去"的那一段。
//   3. **主轴加速 / 匀速 / 减速**：到 `core_at` 触发停止。
//
// 主轴转速：真实 CD 500-1000 RPM 在 300 px 盘上会糊成一片，取**视觉档 3 r/s**
// （≈180 RPM，比真实慢 3-5 倍）。
pub const DISC_DROP_S: f32 = 0.50; // 兼容旧名（app.rs 用）：不再代表独立阶段
pub const DISC_SPIN_S: f32 = DISC_SPIN_UP_S; // 兼容旧名：`DISC_SPIN_S ≡ DISC_SPIN_UP_S`

/// 下坠末的 cy（下缘贴 480 = 屏幕底）；吸入末的 cy（上缘贴 430，**沉进插卡位洞里**）。
pub const DISC_FALL_END_CY: f32 = 330.0;
pub const DISC_SETTLE_CY: f32 = 580.0;

/// 弹出（拍 3）的目标 cy：**屏幕居中**（货架选中时的位置）。
///
/// 货架主卡带是「双向居中」画的（`cart.rs::foot_y`：`foot = OUT_H/2 + h*middle/2`），
/// 所以选中卡片的中心就落在 `OUT_H/2 = 240`。光盘弹出后**弹回这里**（dia 300 时占 90..390，
/// 整盘在屏内），而不是飞出屏外 —— 见 `draw_disc_eject` 拍 3。
/// ★ 2026-10-26（六六）：与插入起点 `DISC_START_Y` 是同一点（240），一进一出共享货架静止位。
pub const DISC_EJECT_LAND_CY: f32 = DISC_START_Y;

/// 阶段 1（下落）的重力加速度 + 预推导的时长/速度（const 里不能调 `sqrt`，故预写死）。
///
/// 几何（★ 2026-10-26 六六改：光盘**从货架静止位 240** 往下掉，不从屏外飞入）：
///   d1 = DISC_FALL_END_CY - DISC_START_Y = 330 - 240 = 90 px
///   fall_s   = √(2·90/800)  = √0.225 ≈ 0.474 s
///   suck_v   = √(2·800·90)  = √144000 = 379.5 ≈ 380 px/s
///   d2 = DISC_SETTLE_CY - DISC_FALL_END_CY = 580 - 330 = 250 px
///   suck_in_s = 250 / 380 ≈ 0.658 s
///   spin_start = 0.474 + 0.658 ≈ 1.132 s
pub const DISC_FALL_A: f32 = 800.0;
pub const DISC_FALL_S: f32 = 0.474;
pub const DISC_SUCK_V: f32 = 380.0;
pub const DISC_SUCK_IN_S: f32 = 0.658;

/// 主轴起转的时刻（= 吸进去结束、光盘沉进洞里的时刻）。
pub const DISC_SPIN_START: f32 = 1.132;

pub const DISC_SPIN_UP_S: f32 = 0.70;  // 主轴加速（0 → 全速）
pub const DISC_SPIN_DOWN_S: f32 = 0.40; // 核心就绪后减速停止

// 兼容 app.rs 的旧导入（DISC_TRAY_*）—— 值保留但语义失效，等下一轮彻底清理。
pub const DISC_TRAY_OUT_S: f32 = 0.0;
pub const DISC_DISC_IN_S: f32 = 0.0;
pub const DISC_TRAY_IN_S: f32 = 0.0;
pub const DISC_SERVO_S: f32 = 0.0;
pub const DISC_TRAY_TOP_Y: f32 = 0.0;
pub const DISC_TRAY_HIDDEN_Y: f32 = 0.0;

/// 光盘最大旋转速度（弧度/秒），约 **3 转/秒**（≈ 180 RPM）。
/// 导出：`app.rs` 用 `clock` 推开机/关机转场里光盘的持续旋转相位（"继续转""回正"）。
pub const DISC_SPIN_RATE: f32 = std::f32::consts::TAU * 3.0;

// ── 插卡位光效（★ 2026-10-26 六六）──────────────────────────────────────────
//
// 六六原话："插卡位在动画时加点光效吧，下缘扩散光像模拟光驱透出来的，等到光盘
// 吸入后改为呼吸灯。"
//
// **呼吸阶段**（`t >= DISC_SPIN_START`，光盘已沉进洞里）：扩散光切换为**呼吸灯**，
// 周期 1.6 s（sin 半波），最低 0.20 alpha、最亮 `DISC_GLOW_PEAK`。视觉上像"光驱
// 在工作的指示灯"，比持续亮更含蓄、更像真的。
//
// ★ 光从**挖空开口的左/右/下三条内壁**漏出来（朝开口内部衰减），六六定稿："是插卡位
// 内侧发光（即挖空的左右下三条边），不是上缘对着货架发光。" 上沿朝货架发光是错的。
//
// ★ 常量统一 `DISC_` 前缀，避免与文件顶部顶条 `glow()` 用的 `GLOW_STEPS`/`GLOW_PEAK`
// 撞名（那一套 alpha 0.16 是顶条塑料反光，别的东西）。
//
// ★ 2026-10-26（六六，第二轮）：「光效应该贴着插卡位的轮廓，且光线应该再亮一点、发散范围
// 再大一点」。→ 峰值 alpha 提到 0.60、呼吸下限抬到 0.34；每条内壁衰减从 6 步 × 1.5px（9px）
// 放大到 12 步 × 2.5px（30px），光真正贴着 360×44 挖空轮廓向洞里发散一大圈。
// ★★ 2026-10-27（六六）：「**各机种的插卡位光效都跟主题色板**」。
//
// 原来死的是冷蓝白 `[0.5, 0.75, 1.0]`（当时特意写"非琥珀"）。现在一律取
// `palette::ridge_ink()` —— 换 18 档色板时它是 ramp 的 `bright`（青/琥珀/紫都跟着走），
// 换明暗主题时是 0xd8d4cc / 0x6b645b。它和 CHANGE 字、帘片缝高光是**同一个色源**，
// 所以插卡位的光和卷闸/丝印是一套的，不会各说各话。
//
// 不做 `light::gain()` 的减半：这圈光画在**洞的内壁**上（暗腔里），亮色主题下它仍然是
// 暗底上的光，减半只会把它抹没。`tests/contrast.rs` 管的是塑料上的印刷，不管洞里。
const DISC_GLOW_EDGE_N: usize = 12;  // 每条内壁的衰减步数（贴轮廓、发散更大）
const DISC_GLOW_STEP: f32 = 2.5;     // 每步像素（向内衰减 ~30 px）
const DISC_GLOW_PEAK: f32 = 0.60;    // 峰值 alpha（更亮）
const DISC_GLOW_BREATH_LO: f32 = 0.34; // 呼吸灯下限（抬高，别黑掉）
const DISC_BREATH_PERIOD: f32 = 1.6;

/// 插卡位溢光的颜色，`alpha` 单独给（衰减逐条不同）。见上面那段 ★★。
fn glow_colour(alpha: f32) -> [f32; 4] {
    let c = crate::palette::ridge_ink();
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        alpha,
    ]
}

/// 光盘核心结构体。
#[derive(Clone, Debug)]
pub struct DiscState {
    /// 光盘开始旋转的时刻（相对于插入开始）。
    pub spin_start: f32,
    /// 核心就绪的时刻（如果早于旋转完成）。
    pub core_at: Option<f32>,
    /// 光盘标签贴图（如果有）。
    pub face: Option<(TexId, u32, u32)>,
    /// ★ 2026-10-27（六六）：这次进槽的是**代币**（ARCADE / NEOGEO）还是**光盘**（PS1）。
    ///
    /// 两者走同一条"从货架位下落、进洞"的时间轴，但后半段完全不同：光盘停在洞里转、
    /// 等核心就绪；代币**全部出屏**，然后街机箱体从插卡口升起来。
    pub coin: bool,
}

/// 绘制 PS1 光盘插入动画：托盘弹出 → 放入光盘 → 托盘收回 → 加速 → 匀速等待 → 减速停止。
///
/// # 参数
/// - `out`: 绘制命令输出
/// - `t`: 当前动画时间（秒）
/// - `disc`: 光盘状态（含 `core_at`：核心就绪时刻）
/// - `dia`: 光盘直径（默认 300px）
/// - `face_override`: 外部提供的贴图（优先于 `disc.face`）
///
/// # 动画阶段（★ 2026-10-26 吸入式）
/// 1. **下落** (0 → DISC_FALL_S): `θ = ½ a t²`，末速 `v_c`。
/// 2. **吸进去** (FALL_S → SPIN_START): 匀速 `v_c`，光盘从"底沿 = slot_bottom"
///    沉到"上沿 = slot_top"。
/// 3. **主轴加速 / 匀速 / 减速** (SPIN_START → core_at + SPIN_DOWN_S):
///    到 core_at 起转停，减速停止。
pub fn draw_disc_insert(
    out: &mut Vec<Draw>,
    t: f32,
    disc: &DiscState,
    dia: f32,
    face_override: Option<&(TexId, u32, u32)>,
) {
    let cx = OUT_W as f32 / 2.0;
    let slot_top = DISC_SLOT_Y;
    let r = dia / 2.0;

    // ★ 位置模型（六六定稿）：光盘**沉进插卡位洞里**，画面只露插卡位那一段的顶部弧。
    //   阶段 1 末：光盘下缘贴 480（屏幕底）→ cy = 480 - 150 = 330（DISC_FALL_END_CY）
    //   阶段 2 末：光盘上缘贴 430（插卡位上沿）→ cy = 430 + 150 = 580（DISC_SETTLE_CY）
    let cy_start = DISC_START_Y;            // 240（货架静止位，六六：从当前位置往下掉）
    let cy_phase2_start = DISC_FALL_END_CY; // 330（下坠末 = 吸进去初）
    let cy_final = DISC_SETTLE_CY;          // 580（沉进洞里，画面只剩顶部弧）

    let face = face_override.or(disc.face.as_ref());

    let fall_end = DISC_FALL_S;
    let spin_start = DISC_SPIN_START;

    let v_c = DISC_SUCK_V;

    let (disc_cy, spinning) = if t < fall_end {
        // === 阶段 1：加速下落（到 330）===
        let disc_y = cy_start + 0.5 * DISC_FALL_A * t * t;
        (disc_y, false)
    } else if t < spin_start {
        // === 阶段 2：匀速吸进去（330 → 580）===
        let dt = (t - fall_end).max(0.0);
        let disc_y = cy_phase2_start + v_c * dt;
        (disc_y, false)
    } else {
        // === 阶段 3：主轴加速 / 匀速 / 减速（在 580 处沉进洞里旋转）===
        (cy_final, true)
    };

    // 光效（画在光盘之前）：
    //   扩散阶段（t < spin_start）：强度随光盘下落/深入 330→580 从 0 增到峰值，
    //   模拟光驱吃进盘时内部透出来的光。
    //   呼吸阶段（spinning）：吸入后转成呼吸灯。
    let glow_progress = if t < spin_start {
        // t=0 时 cy_start = 240（货架静止位，还没入槽），不发光。
        // 光盘一旦"入槽"（cy >= DISC_FALL_END_CY = 330）才开始发光，强度随深入 330→580 增强。
        if disc_cy < DISC_FALL_END_CY {
            0.0
        } else {
            ((disc_cy - DISC_FALL_END_CY) / (cy_final - DISC_FALL_END_CY)).clamp(0.0, 1.0)
        }
    } else {
        1.0
    };
    draw_disc_glow(cx, slot_top, out, glow_progress, spinning, t);

    // 绘制光盘
    let angle = if spinning {
        let spin_t = t - spin_start;
        disc_spin_angle(spin_t, disc.core_at)
    } else {
        0.0
    };
    draw_disc(cx, disc_cy, dia, angle, face, out);
}

/// 光驱里那颗**还在转**的光盘 —— 进 PS1 游戏、CRT 开机转场那一小段用。
///
/// 六六原话："加载完毕的时候光盘继续转，屏幕出现 CRT 电视开机的效果（画面从中间一点辉光
/// → 横向的一条辉光线 → 展开后是游戏画面 → 铺满屏幕，开始游玩）"。
///
/// 旧版这里把主轴**刹停**（`disc_spin_angle` 带 `core_at` 的减速段）：光盘停在随机相位，
/// 画面才硬切进来 —— 那正是他觉得"突兀"的地方。现在主轴不刹：核心一就绪就进 `Playing`，
/// `App::screen` 从 0 长到 1（`power.rs` 里现成的 CRT 曲线：中间一点 → 横向一条 → 展开），
/// 这一层只管在那段时间里把洞里那颗转着的光盘 + 呼吸光画出来。
///
/// **不需要淡出**：画面长到整屏（640×480）时把插卡位那个洞也一起盖住了，光盘自然消失。
///
/// 调用顺序：桌面图**之前**（只有这样才能被桌面图的 360×44 挖空裁住，透过洞看见它在转）、
/// 画面 `Draw::Game` **之下**（画面从中间长开，最后把它连同塑料框一起盖掉）。
///
/// - `angle`：当前相位（弧度），由 `app.rs` 用 `DISC_SPIN_RATE` 接着插入末帧推。
/// - `t`：与插入动画同一条时间轴（秒），只给呼吸光取相位用。
pub fn draw_disc_live(
    out: &mut Vec<Draw>,
    angle: f32,
    t: f32,
    dia: f32,
    face: Option<&(TexId, u32, u32)>,
) {
    let cx = OUT_W as f32 / 2.0;
    // 呼吸光：与插入末段同一条曲线（`breathe = true`），`t` 接着插入的时间轴走，
    // 交接那一帧亮度不跳。
    draw_disc_glow(cx, DISC_SLOT_Y, out, 1.0, true, t);
    draw_disc(cx, DISC_SETTLE_CY, dia, angle, face, out);
}

/// 插卡位**内侧**光效（六六定稿：挖空的左/右/下三条内壁朝开口内部漏光，
/// 不是上缘朝货架发）。画在光盘之下，模拟光盘沉进洞里时驱动内部透出的光。
///
/// - **扩散阶段**（`breathe == false`）：`alpha = DISC_GLOW_PEAK · progress`，
///   随光盘被吃进而增强。
/// - **呼吸阶段**（`breathe == true`）：光带切到 [0.20, PEAK] 的 sin 半波呼吸，
///   周期 1.6 s。
fn draw_disc_glow(
    cx: f32,
    slot_top: f32,
    out: &mut Vec<Draw>,
    progress: f32,
    breathe: bool,
    t: f32,
) {
    // 呼吸强度（0..1，sin 半波）
    let breath_val = if breathe {
        let phase = ((t % DISC_BREATH_PERIOD) / DISC_BREATH_PERIOD).max(0.0);
        (std::f32::consts::TAU * phase).sin().clamp(0.0, 1.0)
    } else {
        0.0
    };

    let base = if breathe {
        let lo = DISC_GLOW_BREATH_LO;
        let hi = DISC_GLOW_PEAK;
        lo + (hi - lo) * breath_val
    } else {
        DISC_GLOW_PEAK * progress
    };
    if base <= 0.003 {
        return;
    }

    draw_slot_walls(out, base);

    let _ = cx;
    let _ = slot_top;
}

/// ★★ 2026-10-27（六六）：「**插卡和拔卡时也加上光效（跟 PS1 一样）**」。
///
/// 卡带没有光盘那套"落进洞 → 起转 → 呼吸"，所以强度绑在**行程**上，而不是盘心位置：
/// `seat` 走完抬头（`PULL_SHARE`）之后线性升到峰值。
///
/// 为什么是行程不是"卡带下缘进洞多深"：光画在卡带**之前**，卡带下缘一过 430 就开始把它
/// 盖住 —— 若按下缘算，能看见的那几帧恰好是强度最低的几帧。按行程算，卡带还在洞上方
/// 往下掉的时候槽里已经亮起来了（抬头那一段一个像素都不画），等卡带压下来再把它一道盖掉：
/// 读起来是"槽先亮，卡带把光挤下去"，和 PS1 光盘沉进洞里是同一个意思。
///
/// 拔卡是同一条曲线倒放（`seat` 1→0）：光先被卡带压着，卡带一抬起来洞里的光就露出来，
/// 然后随行程淡掉。
pub fn draw_cart_glow(out: &mut Vec<Draw>, seat: f32) {
    let u = ((seat.clamp(0.0, 1.0) - PULL_SHARE) / (1.0 - PULL_SHARE)).clamp(0.0, 1.0);
    if u <= 0.0 {
        return;
    }
    draw_slot_walls(out, DISC_GLOW_PEAK * u);
}

/// ★★ 2026-10-27（六六）：「光效 1 按你说的做」—— **触点那一下**。
///
/// 卡带贴到底、簧片接上的那一帧，洞口**外沿**亮一圈再收。为什么在外沿、为什么要由调用方
/// 画在桌面图**之后**：
///
/// * 洞里落位后 100% 是卡带（卡带落位宽 = 开口 360），画在洞里等于没画；
/// * 洞口之外全是桌面图那块壳，画在它之前一律被盖掉。
///
/// 只画左 / 右 / 下三边 —— 上沿朝货架发光是 10-26 否过的，和 `draw_slot_walls` 同一条。
/// 两圈：贴边的亮芯 + 外面更宽更淡的一层（光从缝里挤出来落在塑料上，不是一条线）。
pub fn draw_contact_flash(out: &mut Vec<Draw>, flash: f32) {
    let a = flash.clamp(0.0, 1.0);
    if a <= 0.004 {
        return;
    }
    let (x0, y0) = (SLOT_OPEN_X, SLOT_OPEN_Y);
    let (w0, h0) = (SLOT_OPEN_W, SLOT_OPEN_H);
    // (离洞口多远, 多厚, 强度) —— 两圈**首尾相接**：贴边的亮芯 0..2.5 px，外面 2.5..8.5 px
    // 是它的光晕。中间留空会读成"两条线"。
    for (grow, thick, gain) in [(0.0, 2.5, 0.95), (2.5, 6.0, 0.30)] {
        let alpha = a * gain;
        let colour = glow_colour(alpha);
        let x = x0 - grow - thick;
        let y = y0 - grow;
        let w = w0 + (grow + thick) * 2.0;
        let h = h0 + grow * 2.0;
        // 左、右两边竖直；底边横着。上边不画。
        // ⚠️ 每一条都必须**整条落在洞口外面**：偏进来 1.5 px 就变成"洞被点亮"，
        //    而洞里落位后全是卡带，那 1.5 px 会横穿卡带正脸（`tests/contact.rs` 钉这条）。
        out.push(Draw::Rect { x, y, w: thick, h, colour });
        out.push(Draw::Rect { x: x0 + w0 + grow, y, w: thick, h, colour });
        out.push(Draw::Rect { x, y: y0 + h0 + grow, w, h: thick, colour });
    }
}

/// 三条内壁（左 / 右 / 底）朝洞内的溢光，`base` 是峰值 alpha。
///
/// ★ 2026-10-26（六六）：光效绑到**插卡位挖空**（360×44，`SLOT_OPEN_*`），不是 chrome 的
/// 口（`MOUTH_W`）、更不是光盘自身的三条边。光只能从洞内透出：
///   左壁 x=180、右壁 x=540（竖直 430..474）、底壁 y=474（水平 180..540）。
/// 上沿**不发光** —— 那是朝货架发，六六否过。
fn draw_slot_walls(out: &mut Vec<Draw>, base: f32) {
    if base <= 0.003 {
        return;
    }
    let cut_x = SLOT_OPEN_X; // 180
    let cut_w = SLOT_OPEN_W; // 360
    let cut_top = SLOT_OPEN_Y; // 430
    let cut_bottom = SLOT_OPEN_Y + SLOT_OPEN_H; // 474
    let glow_w = 2.0;

    // 一条内壁的衰减：`edge_u`（沿壁法向的边界，越往里越大）、`inc`（法向递增）
    let mut wall = |edge_u: f32, inc: f32, horizontal: bool| {
        for i in 0..DISC_GLOW_EDGE_N {
            let frac = (i as f32 + 0.5) / DISC_GLOW_EDGE_N as f32;
            let alpha = base * (1.0 - frac) * (1.0 - frac);
            if alpha <= 0.002 {
                continue;
            }
            let colour = glow_colour(alpha);
            let u = edge_u + i as f32 * inc;
            if horizontal {
                // 底壁：从 y=474 向上（朝洞内）衰减
                out.push(Draw::Rect {
                    x: cut_x,
                    y: u,
                    w: cut_w,
                    h: glow_w + 0.5,
                    colour,
                });
            } else {
                // 侧壁：左右对称，沿 x 朝洞内衰减，竖边跨 430..474
                out.push(Draw::Rect {
                    x: u,
                    y: cut_top,
                    w: glow_w,
                    h: cut_bottom - cut_top,
                    colour,
                });
            }
        }
    };

    // 底壁（从 y=474 向上 6 步进洞内）
    wall(cut_bottom - glow_w, -DISC_GLOW_STEP, true);
    // 左壁（从 x=180 向右 6 步进洞内）
    let left_edge = cut_x + glow_w;
    wall(left_edge, DISC_GLOW_STEP, false);
    // 右壁（从 x=540 向左 6 步进洞内）
    let right_edge = cut_x + cut_w - glow_w;
    for i in 0..DISC_GLOW_EDGE_N {
        let frac = (i as f32 + 0.5) / DISC_GLOW_EDGE_N as f32;
        let alpha = base * (1.0 - frac) * (1.0 - frac);
        if alpha <= 0.002 {
            continue;
        }
        let colour = glow_colour(alpha);
        out.push(Draw::Rect {
            x: right_edge - i as f32 * DISC_GLOW_STEP - glow_w,
            y: cut_top,
            w: glow_w,
            h: cut_bottom - cut_top,
            colour,
        });
    }
}

// =============================================================================================
// 投币 + 街机升起（ARCADE / NEOGEO）—— ★ 2026-10-27（六六）
// =============================================================================================
//
// 六六原话：「投币（动画同插光盘，不过**全部出屏**）后街机贴图**从插卡口升起**（洋红区发光，
// 随主题色板）直到底部贴着屏幕底部即可，随后游戏加载完毕的时候 CRT 特效进游戏画面」。
//
// 两段，都挂在插入的本地时钟 `t` 上：
//   ① `0 .. COIN_IN_S`：代币下落 —— 同光盘的前两拍（½at² 落到下缘贴屏底 330，再匀速
//      380 px/s 穿洞），但**不刹车、不留在洞里**：一路到 `COIN_EXIT_CY`(640)
//      ⇒ 上缘 640−150 = 490 > 480，整枚代币出屏，一个像素不剩。
//   ② `COIN_IN_S .. COIN_TOTAL_S`：街机箱体从插卡口升起（`ease` 收尾），末态**底边贴屏底**。
//      它画在桌面图**之前**，所以出洞那一段塑料在它前面 —— 读起来是"从槽里顶出来"，
//      不是"贴在机器脸上滑上来"。
pub const COIN_DIA: f32 = DISC_DIA_DEFAULT;
/// 选中之后**先往上提一下**再撒手（六六 2026-10-27：「选中时往上一点然后旋转着加速下坠至
/// 出屏吧」）。取值跟卡带的抬头同一条读感：40 px、0.14 s、`ease` 收尾。
pub const COIN_LIFT_S: f32 = 0.14;
pub const COIN_LIFT_PX: f32 = 40.0;
/// 撒手之后**一路加速**（`½at²`，没有匀速段）—— 加速度沿用光盘第一阶段那条 800。
pub const COIN_FALL_A: f32 = 800.0;
/// 盘心到这里就算全部出屏（480 + 半径 150 + 10 余量）。
pub const COIN_EXIT_CY: f32 = 640.0;
/// 抬到位时的高度 —— 下落从这里起步。
const COIN_TOP_CY: f32 = DISC_START_Y - COIN_LIFT_PX;
/// 转角的角加速度：撒手之后转速一路加到出屏最快。
pub const COIN_SPIN_ACC: f32 = std::f32::consts::TAU * 3.0;
/// 箱体升起用多久。
pub const CABINET_RISE_S: f32 = 0.55;
/// 升起前整块躲在屏幕下面（顶边 = 屏底）。
const CABINET_START_Y: f32 = OUT_H as f32;

/// 从抬到位到整枚出屏的时长（`½·a·T² = COIN_EXIT_CY - COIN_TOP_CY`）。
///
/// 写成函数而不是 `const`：`f32::sqrt` 在稳定的 const 上下文里还不能调。
pub fn coin_fall_s() -> f32 {
    (2.0 * (COIN_EXIT_CY - COIN_TOP_CY) / COIN_FALL_A).sqrt()
}

/// 代币走到屏外的时刻。
pub fn coin_in_s() -> f32 {
    COIN_LIFT_S + coin_fall_s()
}

/// 投币整段 = 抬一下 + 加速下落出屏 + 箱体升起。弹出就是这条时间轴倒放。
pub fn coin_total_s() -> f32 {
    coin_in_s() + CABINET_RISE_S
}

/// 代币在插入时间 `t` 时的盘心 y。
pub fn coin_cy(t: f32) -> f32 {
    if t <= 0.0 {
        DISC_START_Y
    } else if t < COIN_LIFT_S {
        DISC_START_Y - COIN_LIFT_PX * ease(t / COIN_LIFT_S)
    } else {
        let f = t - COIN_LIFT_S;
        COIN_TOP_CY + 0.5 * COIN_FALL_A * f * f
    }
}

/// 代币的转角：抬头那一段不转，撒手之后 `α` 恒定 ⇒ 转速一路加，出屏时最快。
pub fn coin_angle(t: f32) -> f32 {
    let f = (t - COIN_LIFT_S).max(0.0);
    0.5 * COIN_SPIN_ACC * f * f
}

/// 画代币：提一下 → 边转边加速下坠 → 整枚出屏；洞里的溢光跟卡带/光盘共用
/// `draw_slot_walls`（颜色跟色板）。
pub fn draw_coin_insert(
    out: &mut Vec<Draw>,
    t: f32,
    dia: f32,
    face: Option<&(TexId, u32, u32)>,
) {
    let cy = coin_cy(t);
    let progress = ((cy - COIN_TOP_CY) / (COIN_EXIT_CY - COIN_TOP_CY)).clamp(0.0, 1.0);
    if progress > 0.0 {
        draw_slot_walls(out, DISC_GLOW_PEAK * progress);
    }
    // 用 `draw_disc`（有脸就是 `Draw::Turned`，绕中心转）；代币是圆的，转起来读得对。
    draw_disc(OUT_W as f32 / 2.0, cy, dia, coin_angle(t), face, out);
}

/// 街机箱体升起。`rise` 0 = 整块躲在屏下（顶边 = 480），1 = **底边贴屏底**。
///
/// `glow` 是洋红区单独切出来烘的那圈光（见 `mark_only`），画在本体**之前**、按自己的
/// 尺寸对齐本体中心，`glow_alpha` 由调用方做呼吸。
pub fn draw_cabinet(
    out: &mut Vec<Draw>,
    face: &(TexId, u32, u32),
    glow: Option<&(TexId, u32, u32)>,
    glow_alpha: f32,
    rise: f32,
) {
    let (tex, w, h) = *face;
    let (w, h) = (w as f32, h as f32);
    let e = ease(rise.clamp(0.0, 1.0));
    let end_y = OUT_H as f32 - h;
    let y = CABINET_START_Y + (end_y - CABINET_START_Y) * e;
    let x = (OUT_W as f32 - w) / 2.0;
    if let Some(&(gtex, gw, gh)) = glow {
        let (gw, gh) = (gw as f32, gh as f32);
        out.push(Draw::Tex {
            x: x - (gw - w) / 2.0,
            y: y - (gh - h) / 2.0,
            w: gw,
            h: gh,
            tex: gtex,
            alpha: glow_alpha.clamp(0.0, 1.0),
        });
    }
    out.push(Draw::Tex {
        x,
        y,
        w,
        h,
        tex,
        alpha: 1.0,
    });
}

/// 计算旋转角度。
///
/// `spin_t` 是相对于主轴起转的时刻（已扣除 `DISC_SPIN_START`）。
/// `core_at` 是核心就绪的**绝对**时刻（相对于插入开始）。
/// 光盘主轴在 `spin_t`（相对主轴起转的秒数）时的相位，弧度。
///
/// `core_at` 是核心就绪的本地时刻：给了它就按"减速停止"算（插入动画用），
/// 给 `None` 就是**一直满速转**（进游戏的 CRT 开机转场用，见 [`draw_disc_live`]）。
///
/// ★ 2026-10-26：导出给 `app.rs` —— 交接那一刻的相位要从这里取，才能让"插入末帧"和
/// "开机转场里还在转"是同一个角度，不跳相位。
pub fn disc_spin_angle(spin_t: f32, core_at: Option<f32>) -> f32 {
    let omega_max = DISC_SPIN_RATE;
    let spin_up_s = DISC_SPIN_UP_S;
    let spin_down_s = DISC_SPIN_DOWN_S;

    // 加速结束角度：θ = ½ ω_max · t_up
    let theta_up = 0.5 * omega_max * spin_up_s;

    // 核心就绪的本地时刻（相对于主轴起转）
    let core_local = core_at.map(|ca| ca - DISC_SPIN_START);

    if spin_t < spin_up_s {
        // 加速阶段：θ = ½ α t²
        let alpha = 2.0 * omega_max / spin_up_s;
        0.5 * alpha * spin_t * spin_t
    } else if let Some(ca) = core_local {
        if ca <= spin_up_s {
            // 核心在加速期间就绪：从当时的角速度开始减速
            let alpha_up = 2.0 * omega_max / spin_up_s;
            let theta_at_core = 0.5 * alpha_up * ca * ca;
            let omega_at_core = alpha_up * ca;
            let dt = (spin_t - ca).max(0.0).min(spin_down_s);
            let alpha_down = 2.0 * omega_at_core / spin_down_s;
            theta_at_core + omega_at_core * dt - 0.5 * alpha_down * dt * dt
        } else {
            // 核心在匀速期间就绪
            let theta_at_core = theta_up + omega_max * (ca - spin_up_s);
            let dt = (spin_t - ca).max(0.0).min(spin_down_s);
            let alpha_down = 2.0 * omega_max / spin_down_s;
            theta_at_core + omega_max * dt - 0.5 * alpha_down * dt * dt
        }
    } else {
        // 核心还没就绪：持续匀速旋转
        theta_up + omega_max * (spin_t - spin_up_s)
    }
}

/// 绘制单个光盘（带旋转角度）。
///
/// `face` 是贴图三元组 `(TexId, w, h)`，`None` 时使用默认金属色绘制。
fn draw_disc(
    cx: f32,
    cy: f32,
    dia: f32,
    angle: f32,
    face: Option<&(TexId, u32, u32)>,
    out: &mut Vec<Draw>,
) {
    let r = dia / 2.0; // 半径

    if let Some((tex, _, _)) = face {
        // 有贴图：使用 Draw::Turned 旋转绘制（1:1 正方形）
        out.push(Draw::Turned {
            x: cx - r,
            y: cy - r,
            w: dia,
            h: dia,
            tex: *tex,
            alpha: 1.0,
            turn: angle,
        });
    } else {
        // 无贴图：绘制默认金属色光盘（使用简单矩形）
        draw_default_disc(cx, cy, dia, angle, out);
    }
}

/// 绘制默认金属色光盘（无贴图时）。
///
/// 光盘由三部分组成：
/// 1. 外圈金属环（银色，带轻微旋转效果）
/// 2. 中间标签区域（深灰色）
/// 3. 中心孔（黑色）
fn draw_default_disc(
    cx: f32,
    cy: f32,
    dia: f32,
    angle: f32,
    out: &mut Vec<Draw>,
) {
    let r = dia / 2.0; // 半径
    let hole_r = r * 0.15; // 中心孔半径
    let label_r = r * 0.65; // 标签区域半径

    // 绘制外圈金属环（使用多个矩形模拟圆形）
    draw_circle(cx, cy, r, [0.8, 0.8, 0.8, 1.0], angle, out);

    // 绘制标签区域（深灰色）
    draw_circle(cx, cy, label_r, [0.2, 0.2, 0.2, 1.0], angle, out);

    // 绘制中心孔（黑色）
    draw_circle(cx, cy, hole_r, [0.0, 0.0, 0.0, 1.0], angle, out);
}

/// 绘制圆形（使用矩形近似）。
///
/// 由于没有贴图，使用多个矩形来模拟圆形。
/// 这是一个简化的实现，足够用于动画效果。
fn draw_circle(
    cx: f32,
    cy: f32,
    radius: f32,
    colour: [f32; 4],
    _angle: f32,
    out: &mut Vec<Draw>,
) {
    // 使用多个水平矩形来近似圆形
    // 步长为半径的 1/10，这样足够平滑
    let step = radius / 10.0;
    let mut y = cy - radius;

    while y < cy + radius {
        // 计算该 y 位置的宽度
        let dy = (cy - y).abs();
        if dy <= radius {
            let half_w = (radius * radius - dy * dy).sqrt();
            let w = half_w * 2.0;
            if w > 1.0 {
                out.push(Draw::Rect {
                    x: cx - half_w,
                    y,
                    w,
                    h: step + 0.5, // 稍微重叠避免缝隙
                    colour,
                });
            }
        }
        y += step;
    }
}

/// 绘制 PS1 光盘**弹出**动画（★ 2026-10-26 六六定稿：吸入式反向 + 五拍节奏）。
///
/// 六六原话："光盘退出的动画太仓促，应该是**停止旋转 → 插卡位发光 → 直接弹射回原位
/// → 插卡位光线熄灭 → 回到货架可操作状态**。"
/// 后来又补了两句："退出的时候就是 CRT 电视关机那样，收束成中间一条辉光线后淡出 →
/// 光盘停转（**但必须回正**）→ 弹出到货架位 → 恢复货架可操作性"。
/// ⇒ CRT 关机那一拍不在这里，是 `app.rs` 的事（`self.screen` 倒退，游戏层被合成器收成
/// 一条 2px 辉光线再淡出）；这个函数接的是"屏幕已经黑了"之后的那五拍。
///
/// 五拍（总时长 ≈ `DISC_EJECT_S` = 1.23 s）：
/// 1. **停止旋转** (0 → 0.40 s): 光盘在洞里（cy=580）减速到停，**末角回正**（整圈）。
/// 2. **插卡位发光** (0.40 → 0.70 s): 内侧三条壁透光由暗到亮（像光驱通电吐光），
///    光盘仍在洞里静止。
/// 3. **直接弹射回原位** (0.70 → 1.00 s): 光盘从洞里 580 弹回**货架选中位** 240
///    （ease-out，像被机构顶出来；落点正好是主卡带的双向居中位置，整盘收回屏内）。
/// 4. **光线熄灭** (1.00 → 1.23 s): 光盘已在货架位，内侧光由亮到灭。
/// 5. **回货架** (≥1.23 s): 函数结束（`*t >= DISC_EJECT_S`），app 转 `Phase::Shelf`，
///    货架可操作。
pub fn draw_disc_eject(
    out: &mut Vec<Draw>,
    t: f32,
    cx: f32,
    dia: f32,
    face: Option<&(TexId, u32, u32)>,
) {
    // 弹出 *t 从负值（`-DISC_EJECT_HOLD_S` = -0.10）起走：负值区间屏幕还在 CRT 关机、
    // 光盘应在洞里静止。归一化到 0 起，避免负时间算出乱角度。
    let t = t.max(0.0);
    let slot_top = DISC_SLOT_Y;
    let omega_max = DISC_SPIN_RATE;

    const E1_SPIN_STOP: f32 = 0.40; // 拍 1：停止旋转
    const E2_GLOW: f32 = 0.30;      // 拍 2：插卡位发光
    const E3_EJECT: f32 = 0.30;     // 拍 3：直接弹射回原位
    const E4_FADE: f32 = 0.23;      // 拍 4：光线熄灭

    let e1 = E1_SPIN_STOP;
    let e2 = e1 + E2_GLOW;
    let e3 = e2 + E3_EJECT;
    let e4 = e3 + E4_FADE;

    // ── 拍 1 的角度：减速到停，**并且停正** ────────────────────────────────────────────────
    //
    // ★ 2026-10-26（六六）：「光盘停转（**但必须回正**）」。
    //   旧版按 `α = ω/T` 恒减速，末角 = `ωT/2` ≈ 0.6 圈（≈ 216°）—— 盘面歪着停死，
    //   接着弹回货架位，整盘看着像没对准位。
    //   现在这一拍的总角位移取**整圈**，曲线用三次 Hermite，同时钉住四个边界：
    //       θ(0) = 0                  接住上一帧（盘已满速在转）
    //       θ'(0) = ω_max             接住满速，不许跳速
    //       θ(1) = D 圈               正好整圈
    //       θ'(1) = 0                 停稳
    //   令 `u = t/E1_SPIN_STOP`、`w = ω_max·E1_SPIN_STOP/TAU`（归一化初速，单位「圈/拍」）、
    //   `D` 为圈数，解得 `θ(u) = TAU·((a3·u + a2)·u + a1)·u`，其中
    //       a1 = w,  a2 = 3D − 2w,  a3 = w − 2D
    //   末角 `θ(1) = TAU·D ≡ 0 (mod TAU)` ⇒ **盘面正立**，后三拍一直停在这个角上。
    //   `D = ceil(w/2)` 保证初速再大也能在 D 圈内刹住；本文件这组常数下 w = 1.2 ⇒ D = 1。
    let w = omega_max * E1_SPIN_STOP / std::f32::consts::TAU;
    let turns = (0.5 * w).ceil().max(1.0);
    let (a3, a2, a1) = (w - 2.0 * turns, 3.0 * turns - 2.0 * w, w);
    let theta_home = std::f32::consts::TAU * turns;
    let spin_angle = |u: f32| std::f32::consts::TAU * ((a3 * u + a2) * u + a1) * u;

    let (disc_cy, angle, glow) = if t < e1 {
        // 拍 1：在洞里（580）减速停止，末角回正
        let local = t.min(E1_SPIN_STOP);
        (DISC_SETTLE_CY, spin_angle(local / E1_SPIN_STOP), 0.0)
    } else if t < e3 {
        // 拍 2（发光）+ 拍 3（弹射）。角度恒为正立（`theta_home` ≡ 0 mod TAU）。
        let ang = theta_home;
        let cy = if t < e2 {
            DISC_SETTLE_CY // 拍 2：仍在洞里
        } else {
            // 拍 3：从洞里 580 弹射到「屏幕居中」240（货架选中位置）。
            // ease-out（弹射起步快、接近落点放慢），像被机构顶出来；落点正好是货架主卡带
            // 的双向居中位置，光盘整盘收回屏内（300 时占 90..390），不是飞出屏外。
            let p = ((t - e2) / E3_EJECT).clamp(0.0, 1.0);
            let ease_out = 1.0 - (1.0 - p) * (1.0 - p);
            DISC_SETTLE_CY - (DISC_SETTLE_CY - DISC_EJECT_LAND_CY) * ease_out
        };
        // 光：拍 2 由暗到亮（0→1），拍 3 保持最亮
        let g = if t < e2 {
            ((t - e1) / E2_GLOW).clamp(0.0, 1.0)
        } else {
            1.0
        };
        (cy, ang, g)
    } else if t < e4 {
        // 拍 4：光盘停在屏幕居中（240，货架选中位置），光由亮到灭（1→0）
        let g = 1.0 - ((t - e3) / E4_FADE).clamp(0.0, 1.0);
        (DISC_EJECT_LAND_CY, theta_home, g)
    } else {
        // 拍 5：回货架，函数结束
        return;
    };

    // 内侧三条壁透光（画在光盘之下）
    if glow > 0.0 {
        draw_disc_glow(cx, slot_top, out, glow, false, t);
    }
    draw_disc(cx, disc_cy, dia, angle, face, out);
}
