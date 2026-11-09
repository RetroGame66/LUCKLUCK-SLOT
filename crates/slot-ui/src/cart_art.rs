//! Cart artwork the card supplies, in place of the outline SVG the build carries.
//!
//! `System/Carts/cart_gb.png`, `cart_gbc.png`, `cart_gba.png` — one per machine. Absent, the
//! built-in silhouette and flat shell colour are used and nothing changes; present, the card's
//! own art is the cart, and the shell colour is applied to it rather than to a mask.
//!
//! And `System/Carts/tpl_<name>_<machine>.png` — the same shape of file under a name a
//! `carts.tsv` row can point a single game at, so one game can be drawn from a transparent
//! or a metal cart while its neighbours keep the machine's own. The conventions are the
//! same, the decode is the same, and a template no row names is never read.
//!
//! Two conventions the art has to keep, both of them discovered rather than invented:
//!
//! * **Alpha is the shape.** A transparent pixel is not cart, which is how the notch, the
//!   bulge and the rounded corners are cut.
//! * **Magenta is the label.** `#ff00ff` marks where the game's own label goes. The code cuts
//!   that region out, fills it with the colour just outside it — so the corners of a rounded
//!   label show the recess rather than a hole — and pastes the label into it, clipped to the
//!   same shape. Nothing else has to be written down: the art says where its label goes and how
//!   round its corners are.
//!
//! And a third kind of file, one per game rather than one per machine — **not in this
//! folder**: it lives in `Cartridge/`, `<stem>.png` or `<machine>/<stem>.png` on a card that
//! sorts its roms, so a game's finished picture never sits beside the machine's own. It is
//! the **whole cart already drawn**, label and all: no magenta, nothing pasted at run time,
//! and a cart that has one is drawn from it alone with this module not consulted at all
//! (2026-10-23, 六六:「卡带贴图+洋红色贴纸的组合迭代成纯卡带贴图」). The scan resolves the
//! path — `slot_store::scan::Cart::art` — and `cart::pure_face` draws it. The settings
//! screen's 美术资源 row decides which of the two outranks the other when a cart carries
//! both; a cart with only one of them is dressed from the one it has, and a cart with
//! neither is the composite above with the game name set in the slot, unchanged.
//!
//! **What size to draw it** (2026-10-23 六六): the on-shelf size is the hard rule — a
//! landscape cart is 360 px wide in the middle and 240 at the sides, a portrait one 300 tall
//! and 200 (see `cart::CartSize`). Author the picture at the middle size — **360 wide** for a
//! landscape box (the Advance's and the consoles'), **300 tall** for the portrait one — and
//! any other size is stretched to the box by `art::stretch`: nothing is refused, and a
//! picture drawn at the box's own aspect lands on the shelf unstretched.
//!
//! The artwork is loaded once at boot and read from the face-building thread afterwards, which
//! is why it lives in a `OnceLock` rather than being passed down.

use std::path::Path;
use std::sync::OnceLock;

use slot_store::System;

use crate::art;
use crate::cart::{CartFace, CartSize};
use crate::shell::{Finish, Shell};

/// Where the card keeps it.
const DIR: &str = "System/Carts";

/// The magenta the label slot is marked with. The tolerance is generous because the art comes
/// back through a paint program: `#ff00ff` with green held well below the other two is what
/// every magenta that is meant as a marker looks like and no shell colour does.
const MARK_R: u8 = 200;
const MARK_G: u8 = 60;
const MARK_B: u8 = 200;

/// 标记向外多长几格 —— **它自己那圈抗锯齿也是标记**。见 `find_label`。
///
/// 为什么非有不可：这几张图都是画图软件出来的，洋红区的边界是 **1~2 格洋红与背景混出来的**，
/// `(248,87,246)`（接浅壳）、`(85,34,89)`（接深壳）都过不了 `is_mark`。它们既不被填 recess、
/// 也不被贴纸盖住，而卡不给 `light` 时（GB/GBC 卡带、CD 封面）就照原样画 ⇒ **亮色主题下就是
/// 贴纸周围一圈洋红**（六六 2026-10-07：「DISC 货架我切回亮色模式发现封面区还有点洋红边」）。
/// 深色主题看不见，因为那边这一圈是用 `shell.colour` 重涂的；GBA 卡带也看不见，因为它的
/// `light` 是 `Some(...)`，同一圈被重涂成机壳色。
///
/// **2 是量出来的，不是拍的**：卡上 `cd.png` 是 400×370，贴到 520×520 的面上是**放大**，
/// art 里 1 格混色到屏上是 2 行；只长 1 格 ⇒ 面里还剩 **1403** 个洋红像素，长 2 格 ⇒ **0**
///（实测，`.workbuddy/memory/MEMORY-排错与工具.md` §25）。坐标框同步外扩，否则贴纸会被截掉
/// 这一圈。**`tape_art.rs` 里有一份逐字相同的东西**，两处是同一条规矩。
const MARK_GROW: u32 = 2;

/// How bright the plastic is taken to be, as a percentile of its own pixels. Applied as
/// `colour * (luma / this)`, so anything at or below the reference keeps the shell colour's
/// value and only the true highlights may exceed it. The *median* was the first choice and it
/// was wrong: half the art then scaled up, and a shell's brightest highlight — which the built
/// in pipeline caps at 1.45x — ran to 255 and blew out to white.
const REF_PERCENTILE: f32 = 0.90;

/// The label slot, in the art's own pixels.
///
/// Fields `pub` because the default cart art reads them too (its magenta area is *not* a label
/// to paste into — it is a region to tint with the game's own colour) — see [`DefaultArt`].
pub struct Label {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
    /// Coverage of the slot, so a label with rounded corners keeps them.
    pub mask: Vec<u8>,
}

pub struct Art {
    rgba: Vec<u8>,
    w: u32,
    h: u32,
    label: Option<Label>,
    /// Median-ish brightness of the plastic, for the shell tint.
    luma: f32,
    /// The colour just outside the label slot, so the slot can be filled before the label is
    /// pasted and its corners read as recess rather than as a hole.
    recess: [u8; 3],
}

/// One of the board's lamps: where it sits on the board, as a share of the board. The card
/// draws them as magenta and the code fills them — one per socket, left to right, so index 0 is
/// the mGBA socket and 1 the gpSP one, which is the order `Chip::across` counts in.
pub struct Lamp {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// 卡带**背部**（翻过来看到的那面 PCB）的素材。
///
/// ⚠️ **只有一张，而且只需要一张**（2026-10-24 六六：「**只有 GBA 需要换核心啊，
/// 其它机种没这个需求**」）。背面只在"掀开卡带选核心"那个界面里出现，而那个界面
/// **只对 GBA 打开** —— `app.rs::open_core_picker` 开头就是
/// `if cart.system() != System::Gba { return; }`，理由也写在旁边：
/// *"gpSP does not take Game Boy... opening it over one socket is a question that can only
/// be refused."*。GB/GBC/SFC/FC/NeoGeo/街机各只有一个核心，没有可选项。
///
/// 所以这里**不要**按机种拆成七份 —— 那是我 2026-10-24 22:1x 一度做错的方向，六六当场
/// 纠正。只有一个例外：亮色主题下这张板要跟主题走（见 `board::board_face` 里的
/// `themed_shell`），那是"同一块板换个亮度"，不是"另一个机种的板"。
pub struct BoardArt {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
    pub lamps: Vec<Lamp>,
}

#[derive(Default)]
pub struct CartArt {
    gb: Option<Art>,
    gbc: Option<Art>,
    gba: Option<Art>,
    /// ★ 2026-10-24（六六）：「各机种的卡带背部跟随着兜底的卡带剪影」——
    ///   卡带**正面**原来也只有三张（`cart_gb/gbc/gba.png`），SFC / FC / NeoGeo / 街机
    ///   一律落到内建 SVG 轮廓，而那个轮廓是 Advance 的横版**兜底剪影**（见
    ///   `silhouette::Shape::from` 的注释）。这里给它们留好位置：卡上放了
    ///   `System/Carts/cart_sfc.png`（`cart_fc` / `cart_neogeo` / `cart_arcade` 同理）
    ///   就直接用，不必重编二进制；没放则与从前完全一致。
    sfc: Option<Art>,
    fc: Option<Art>,
    neogeo: Option<Art>,
    arcade: Option<Art>,
    /// ★ 2026-10-25：MD 与 PS1，同 SFC/FC 那批 —— 卡上放了 `cart_md.png` / `cart_ps1.png`
    ///   就用，不放就退成内建轮廓（与所有卡带一致）。**两张都还没有**，见 `cart::size_for`
    ///   里关于高度的注释：没有贴图就没有 alpha 外接框可量，高度只能先借横版占位盒。
    md: Option<Art>,
    ps1: Option<Art>,
    /// PCE 的位置，同 MD/PS1：文件还没有，不放就是 `None`。
    pce: Option<Art>,
    board: Option<BoardArt>,
    /// The templates the rules table names, each with its per-machine art. Empty unless a
    /// `carts.tsv` row points at one.
    templates: Vec<(String, TemplateArt)>,
}

/// One template's art, per machine — a template is drawn per machine exactly as the
/// built-in art is, because a Game Boy cart and an Advance cart are not the same shape.
struct TemplateArt {
    gb: Option<Art>,
    gbc: Option<Art>,
    gba: Option<Art>,
}

impl TemplateArt {
    fn for_system(&self, system: System) -> Option<&Art> {
        match system {
            System::Gb => self.gb.as_ref(),
            System::Gbc => self.gbc.as_ref(),
            System::Gba => self.gba.as_ref(),
            // Console and arcade shelves have no cartridge art yet; the template simply has no
            // entry. The arcade row's own art is the overlay ring's, not a per-game cartridge.
            // MD and PS1 are in the same position: no `tpl_*_md.png` convention has ever been
            // drawn, and a template is only ever read for a name a `carts.tsv` row asked for.
            System::Sfc
            | System::Fc
            | System::Neogeo
            | System::Arcade
            | System::Md
            | System::Ps1
            | System::Pce => None,
        }
    }
}

static ART: OnceLock<CartArt> = OnceLock::new();

/// Called once, at boot, before the face builder starts.
pub fn install(cart_art: CartArt) {
    let _ = ART.set(cart_art);
}

pub fn art_for(system: System) -> Option<&'static Art> {
    let all = ART.get()?;
    match system {
        System::Gb => all.gb.as_ref(),
        System::Gbc => all.gbc.as_ref(),
        System::Gba => all.gba.as_ref(),
        // ★ 2026-10-24：卡上画了本机种的卡带就用它；没画仍是 `None`（内建 Advance 轮廓兜底）。
        System::Sfc => all.sfc.as_ref(),
        System::Fc => all.fc.as_ref(),
        System::Neogeo => all.neogeo.as_ref(),
        System::Arcade => all.arcade.as_ref(),
        System::Md => all.md.as_ref(),
        System::Ps1 => all.ps1.as_ref(),
        // PCE 与 SFC/FC 同属横版卡带家族：卡上还没画 `cart_pce.png`，所以是 `None`，
        // 由内建 Advance 轮廓兜底（见 `silhouette`）。给了贴图就自动生效。
        System::Pce => all.pce.as_ref(),
    }
}

/// The art for one machine, or the template a card-side rule asks this game to wear.
///
/// 一张贴图的 alpha，按机种共享给 `silhouette`（轮廓）与 `cart`（阴影）。
///
/// 为什么要有这个中间类型：`silhouette` 的掩膜是**按形状**缓存的（一种机种一份），
/// 而 `Art` 是按机种实例的 —— 把 alpha 单独摊出来，两边拿的都是同一份字节。
pub struct Alpha {
    pub bytes: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

/// 卡上这个机种的正面贴图的 alpha。**卡带的轮廓与阴影都只有这一个来源**（2026-10-24 起）。
///
/// `None` = 卡上没放这张贴图（`System/Carts/cart_<machine>.png` 缺席）：调用方各自决定
/// 退成什么 —— 掩膜退成一整块平板，阴影退成"不画"。
impl Alpha {
    /// 盒式重采样到 `w × h`。卡带边缘的抗锯齿要留下来，否则掩膜边缘是一排硬台阶、
    /// 会把贴图自己的柔边切成锯齿 —— 与 `art::stretch` 同一种采样。
    pub fn resized(&self, w: u32, h: u32) -> Vec<u8> {
        let (sw, sh) = (self.w, self.h);
        if sw == 0 || sh == 0 || w == 0 || h == 0 {
            return vec![0; (w * h) as usize];
        }
        let (kx, ky) = (sw as f32 / w as f32, sh as f32 / h as f32);
        let mut out = vec![0u8; (w * h) as usize];
        for y in 0..h {
            let y0 = y as f32 * ky;
            let ya = (y0.floor() as u32).min(sh - 1);
            let yb = ((y0 + ky).ceil() as u32).max(ya + 1).min(sh);
            for x in 0..w {
                let x0 = x as f32 * kx;
                let xa = (x0.floor() as u32).min(sw - 1);
                let xb = ((x0 + kx).ceil() as u32).max(xa + 1).min(sw);
                let mut sum = 0u32;
                let mut n = 0u32;
                for sy in ya..yb {
                    for sx in xa..xb {
                        sum += self.bytes[(sy * sw + sx) as usize] as u32;
                        n += 1;
                    }
                }
                out[(y * w + x) as usize] = if n == 0 { 0 } else { (sum / n) as u8 };
            }
        }
        out
    }
}

pub fn alpha_for(system: System) -> Option<Alpha> {
    let art = art_for(system)?;
    Some(Alpha {
        w: art.w,
        h: art.h,
        bytes: art.rgba.chunks_exact(4).map(|p| p[3]).collect(),
    })
}

/// A template with no art for this machine — a `tpl_metal_gba.png` with no `_gbc` beside
/// it — falls back to the machine's own, because a half-drawn template is one that was
/// only drawn for one machine, not a cart with nothing to wear.
pub fn art_for_game(system: System, code: &str) -> Option<&'static Art> {
    let all = ART.get()?;
    if let Some(name) = crate::cart_rules::for_code(code).and_then(|r| r.art.as_deref()) {
        if let Some((_, per_machine)) = all.templates.iter().find(|(n, _)| n == name) {
            if let Some(art) = per_machine.for_system(system) {
                return Some(art);
            }
        }
    }
    art_for(system)
}

/// Read whatever the card offers. A missing, unreadable or index-coloured file is simply not
/// there: the built-in cart is a perfectly good cart.
pub fn load(root: &Path) -> CartArt {
    let dir = root.join(DIR);
    // The template names come from the rules table, read here rather than handed in: the
    // table is one small file, and this way the loader answers the same whether or not
    // anything has installed the rules yet — the render harness loads art off a deploy
    // directory and gets the same answer the device does.
    let templates = crate::cart_rules::load(root)
        .templates()
        .iter()
        .map(|name| (name.clone(), read_template(&dir, name)))
        .collect();
    CartArt {
        gb: read(&dir.join("cart_gb.png")),
        gbc: read(&dir.join("cart_gbc.png")),
        gba: read(&dir.join("cart_gba.png")),
        // ★ 2026-10-24：四个主机/街机机种也有位置了 —— 不放文件就是 `None`，行为与从前一致。
        sfc: read(&dir.join("cart_sfc.png")),
        fc: read(&dir.join("cart_fc.png")),
        neogeo: read(&dir.join("cart_neogeo.png")),
        arcade: read(&dir.join("cart_arcade.png")),
        // ★ 2026-10-25：MD 与 PS1 的位置也留好了（文件都还没有）。同上面那批一样，不放就是
        //   `None`，行为与从前完全一致 —— 加机种不该让一张旧卡有任何不同。
        md: read(&dir.join("cart_md.png")),
        ps1: read(&dir.join("cart_ps1.png")),
        pce: read(&dir.join("cart_pce.png")),
        // ★ 2026-10-24：卡带背部按机种读，`board_gba.png` 仍是一切缺省值的落点。
        // ★ 2026-10-24：**就这一张**。背面只在"掀开卡带选核心"里出现，而那个界面只对
        //   GBA 打开（`open_core_picker` 的 `if cart.system() != System::Gba { return; }`）——
        //   其余机种各只有一个核心，没有可选项，也就不需要各自的背板。
        board: read_board(&dir.join("board_gba.png")),
        templates,
    }
}

/// One template's three files, each read the way the machine's own art is. A name the card
/// does not carry is an empty set, which reads as "fall back to the machine's own art".
fn read_template(dir: &Path, name: &str) -> TemplateArt {
    TemplateArt {
        gb: read(&dir.join(format!("tpl_{name}_gb.png"))),
        gbc: read(&dir.join(format!("tpl_{name}_gbc.png"))),
        gba: read(&dir.join(format!("tpl_{name}_gba.png"))),
    }
}

/// The card's board, if it supplied one.
///
/// **No machine parameter, by design** (2026-10-24 六六：「只有 GBA 需要换核心啊，其它机种没
/// 这个需求」)：背面只服务"掀开卡带选核心"，而那个界面只对 GBA 打开。曾经按机种拆过七份，
/// 是被这句话纠正回来的 —— 别再加回去。
pub fn board_art() -> Option<&'static BoardArt> {
    ART.get()?.board.as_ref()
}

fn read_board(path: &Path) -> Option<BoardArt> {
    let (rgba, w, h) = art::decode_rgba(path)?;
    if w < 8 || h < 8 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    let lamps = find_lamps(&rgba, w, h);
    Some(BoardArt { rgba, w, h, lamps })
}

/// The magenta marks on the board, grouped into blocks and ordered left to right. Split on the
/// columns that carry no mark at all, which is enough for lamps set into separate chips and
/// avoids a flood fill for what is, at most, a handful of rectangles.
fn find_lamps(rgba: &[u8], w: u32, h: u32) -> Vec<Lamp> {
    let marked = |x: u32, y: u32| is_mark(&rgba[((y * w + x) * 4) as usize..][..4]);
    let mut columns = vec![false; w as usize];
    for y in 0..h {
        for x in 0..w {
            if marked(x, y) {
                columns[x as usize] = true;
            }
        }
    }
    let mut out = Vec::new();
    let mut run: Option<u32> = None;
    for x in 0..=w {
        let on = x < w && columns[x as usize];
        match (run, on) {
            (None, true) => run = Some(x),
            (Some(x0), false) => {
                let (mut y0, mut y1) = (u32::MAX, 0u32);
                for y in 0..h {
                    for xx in x0..x {
                        if marked(xx, y) {
                            y0 = y0.min(y);
                            y1 = y1.max(y);
                        }
                    }
                }
                if y0 <= y1 {
                    out.push(Lamp {
                        x: x0 as f32 / w as f32,
                        y: y0 as f32 / h as f32,
                        w: (x - x0) as f32 / w as f32,
                        h: (y1 - y0 + 1) as f32 / h as f32,
                    });
                }
                run = None;
            }
            _ => {}
        }
    }
    out
}

/// What the card has, for the startup log: a card with art that was refused has nothing else to
/// say so.
pub fn summary() -> String {
    match ART.get() {
        None => "none".to_string(),
        Some(all) => [
            ("gb", &all.gb),
            ("gbc", &all.gbc),
            ("gba", &all.gba),
            ("sfc", &all.sfc),
            ("fc", &all.fc),
            ("neogeo", &all.neogeo),
            ("arcade", &all.arcade),
            ("md", &all.md),
            ("ps1", &all.ps1),
            ("pce", &all.pce),
        ]
        .iter()
            .map(|(name, art)| match art {
                Some(a) if a.label.is_some() => format!("{name} {}x{}+label", a.w, a.h),
                Some(a) => format!("{name} {}x{}", a.w, a.h),
                None => format!("{name} -"),
            })
            // 背部就一张（只有 GBA 会掀卡带选核心，见 `BoardArt` 的注释）。
            .chain(std::iter::once(match &all.board {
                Some(b) => format!("board {}x{} {} lamps", b.w, b.h, b.lamps.len()),
                None => "board -".to_string(),
            }))
            .chain(std::iter::once(format!("{} templates", all.templates.len())))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// ★ 2026-10-26（六六）：「这是部分缺省的卡带图，用于没有贴图的游戏」。
///
/// 一张**整张卡带已经画好**的图（含前端名字），贴纸区仍然留着一大块洋红。它和 [`Art`]
/// （`cart_<sys>.png` 机器模板）是两回事：
///
/// * `Art` 的洋红是**贴标签的槽** —— 运行时找出来、填 recess、把游戏的 label 贴进去；
/// * `DefaultArt` 的洋红**不是槽，是待填色的区** —— 没有标签可贴，上货架时把它换成这个
///   游戏自己的颜色（`label_colour`，稳定 hash），所以每张卡都有自己的颜色（六六原话：
///   「原本卡带贴纸部分仍然留有大面积的洋红区（只是多了前端名字），上货架的时候给随机
///   颜色就好」）。
///
/// 每机种一张、开机加载一次、常驻共享（`DEFAULTS`），不参与面缓存 LRU —— 同一张图重复画
/// 零成本。`tinted` 按卡克隆填色，所以共享的字节永远不被改。
pub struct DefaultArt {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
    /// 洋红贴纸区（可能没有 —— 图里没画洋红）。
    pub label: Option<Label>,
}

impl DefaultArt {
    /// 把洋红贴纸区换成 `colour`，返回一份新的 rgba。原图不动：它是常驻共享的，而每张卡
    /// 的颜色不同，填色只能在**这张卡**的副本上做。
    ///
    /// ★★ 2026-11-07（六六：「洋红色字体糊了，检查一下着色参数」）—— **按洋红的混合比例
    /// 染色，不再用二元 mask。**
    ///
    /// 旧实现只认 [`is_mark`] 那一批**纯洋红**像素（`r>200 / g<60 / b>200`），而图上的洋红
    /// 是**带抗锯齿的**：文字边缘是洋红与底色的 1~2 px 过渡，`(248,87,246)`、`(128,0,128)`
    /// 这一档全落在门槛外 —— 不染。结果就是字的心里换了色、字边还留着一圈洋红，真机上读作
    /// "糊"。旧那批图没有这个毛病，是因为它们的洋红是**整块面板**，几乎没有边缘。
    ///
    /// 现在把洋红当 key：`k = (min(r,b) - g) / 255` —— 纯 `#ff00ff` 是 1，黑底 / 白底 /
    /// 彩色壳是 0（黄色的 `(253,172,21)` 是负的），抗锯齿边按比例落在中间。
    ///
    /// ★★ 更关键的一步是**怎么混**：`out = P + k * (colour - 洋红)`，即把这一像素里的洋红
    /// 成分**减掉**、换成目标色的同量。先前写成 `lerp(P, colour, k)` 是错的 —— 那只把目标色
    /// "兑"进去，`P` 自己的洋红还在：k=0.5 的边上剩 25% 洋红，几条边串起来就是一圈紫描边
    /// （真机抓帧 `_tools/shots/shot-fixed.png` 上看得见）。
    ///
    /// 实测（同一张 GBA 缺省图，360×215 → 屏上 360 宽）：残留洋红像素 **846 → 6**。
    pub fn tinted(&self, colour: [u8; 3]) -> Vec<u8> {
        let mut rgba = self.rgba.clone();
        let Some(l) = &self.label else {
            return rgba;
        };
        // 洋红本身。混色的参照，不是"检测阈值"。
        const MARK: [f32; 3] = [255.0, 0.0, 255.0];
        for y in l.y0..=l.y1 {
            for x in l.x0..=l.x1 {
                let d = ((y * self.w + x) * 4) as usize;
                if rgba[d + 3] == 0 {
                    continue;
                }
                let (r, g, b) = (rgba[d], rgba[d + 1], rgba[d + 2]);
                let k = (r.min(b) as f32 - g as f32) / 255.0;
                // 0.04 以下当成"没有洋红"：黑底、彩壳、以及压缩噪声都在这一档。
                if k <= 0.04 {
                    continue;
                }
                let k = k.min(1.0);
                for c in 0..3 {
                    let v = rgba[d + c] as f32 + k * (colour[c] as f32 - MARK[c]);
                    rgba[d + c] = v.clamp(0.0, 255.0).round() as u8;
                }
            }
        }
        rgba
    }
}

/// 全卡那批缺省图，按**目录名**键（不是机种）：FC 与 NES、SFC 与 SNES 各一张，因为用户给
/// 的图就是按目录命名的（`66SLOT_CART_NES.png` / `66SLOT_CART_SNES.png`），而机种只能到
/// FC/SFC。**NEOGEO / ARCADE 是 2026-10-27 六六补的两张**（`66SLOT_DISC_*`，300×300 的代币
/// 圆牌 —— 那两台机种的载体是代币，不是卡带）。PCE 仍然没有缺省图。
pub struct DefaultArtSet {
    pub gba: Option<DefaultArt>,
    pub gb: Option<DefaultArt>,
    pub gbc: Option<DefaultArt>,
    pub md: Option<DefaultArt>,
    pub ps1: Option<DefaultArt>,
    pub fc: Option<DefaultArt>,
    pub nes: Option<DefaultArt>,
    pub sfc: Option<DefaultArt>,
    pub snes: Option<DefaultArt>,
    pub neogeo: Option<DefaultArt>,
    pub arcade: Option<DefaultArt>,
}

impl DefaultArtSet {
    /// 一张卡该用哪张缺省图，按它自己的目录名。大小写不敏感（与 `System::from_dir` 同）；
    /// 一个不是机种目录的名字（`Games/` 下的裸 rom、乱起的文件夹）自然落空，返回 `None`。
    pub fn for_dir(&self, dir: &str) -> Option<&DefaultArt> {
        let is = |name: &str| dir.eq_ignore_ascii_case(name);
        if is("GBA") {
            return self.gba.as_ref();
        }
        if is("GB") {
            return self.gb.as_ref();
        }
        if is("GBC") {
            return self.gbc.as_ref();
        }
        if is("MD") || is("GENESIS") || is("MEGADRIVE") {
            return self.md.as_ref();
        }
        if is("PS1") || is("PSX") {
            return self.ps1.as_ref();
        }
        if is("FC") {
            return self.fc.as_ref();
        }
        if is("NES") {
            return self.nes.as_ref();
        }
        if is("SFC") {
            return self.sfc.as_ref();
        }
        if is("SNES") {
            return self.snes.as_ref();
        }
        if is("NEOGEO") {
            return self.neogeo.as_ref();
        }
        if is("ARCADE") {
            return self.arcade.as_ref();
        }
        None
    }
}

static DEFAULTS: OnceLock<DefaultArtSet> = OnceLock::new();

/// Called once at boot, before the face builder thread starts — the same rule `install` keeps
/// for the machine art. A card with no defaults installed answers with the built-in shell.
pub fn install_defaults(set: DefaultArtSet) {
    let _ = DEFAULTS.set(set);
}

/// The default art a directory's shelf draws, if the card carries one. `None` = no default for
/// that directory (NEOGEO/ARCADE/PCE, or a rom not filed under a machine folder): the caller
/// falls back to the built-in shell, exactly as before.
pub fn default_art_for(dir: &str) -> Option<&'static DefaultArt> {
    DEFAULTS.get()?.for_dir(dir)
}

/// Read the card's defaults. File names are the user's own (`66SLOT_CART_<DIR>.png`, PS1 is
/// `66SLOT_DISC_PS1.png`), in `System/Carts/` — see [`DIR`]. A missing or undecodable file is
/// simply not there: the machine keeps its built-in look.
pub fn load_defaults(root: &Path) -> DefaultArtSet {
    let dir = root.join(DIR);
    DefaultArtSet {
        gba: read_default(&dir.join("66SLOT_CART_GBA.png")),
        gb: read_default(&dir.join("66SLOT_CART_GB.png")),
        gbc: read_default(&dir.join("66SLOT_CART_GBC.png")),
        md: read_default(&dir.join("66SLOT_CART_MD.png")),
        ps1: read_default(&dir.join("66SLOT_DISC_PS1.png")),
        fc: read_default(&dir.join("66SLOT_CART_FC.png")),
        nes: read_default(&dir.join("66SLOT_CART_NES.png")),
        sfc: read_default(&dir.join("66SLOT_CART_SFC.png")),
        snes: read_default(&dir.join("66SLOT_CART_SNES.png")),
        neogeo: read_default(&dir.join("66SLOT_DISC_NEOGEO.png")),
        arcade: read_default(&dir.join("66SLOT_DISC_ARCADE.png")),
    }
}

/// One default: decode + find the magenta area. A missing file, or one that will not decode,
/// is `None`; a default that decodes but has no magenta area still stands (its `label` is
/// `None`, and the cart is drawn as-is — the picture is the face).
fn read_default(path: &Path) -> Option<DefaultArt> {
    let (rgba, w, h) = art::decode_rgba(path)?;
    if w < 8 || h < 8 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    let label = find_label(&rgba, w, h);
    Some(DefaultArt { rgba, w, h, label })
}

/// What the boot log says the defaults were.
pub fn defaults_summary() -> String {
    match DEFAULTS.get() {
        None => "none".to_string(),
        Some(all) => [
            ("gba", &all.gba),
            ("gb", &all.gb),
            ("gbc", &all.gbc),
            ("md", &all.md),
            ("ps1", &all.ps1),
            ("fc", &all.fc),
            ("nes", &all.nes),
            ("sfc", &all.sfc),
            ("snes", &all.snes),
            ("neogeo", &all.neogeo),
            ("arcade", &all.arcade),
        ]
        .iter()
            .map(|(name, d)| match d {
                Some(a) => format!(
                    "{name} {}x{}{}",
                    a.w,
                    a.h,
                    if a.label.is_some() { "+mark" } else { "" }
                ),
                None => format!("{name} -"),
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Colour bleed at the alpha edge.
///
/// A cut-out carried over from a paint program keeps the colour it was cut from in its
/// semi-transparent pixels — on the cart art that is a near-black keyline, invisible while the
/// shell is dark and a dark halo the moment it is painted light grey. Three passes of a 3×3
/// average taken from the opaque pixels inward re-colours the edge with the cart's own colour,
/// which is what it should have been. Alpha is untouched: only the colour under it changes.
fn bleed_edge(rgba: &mut [u8], w: u32, h: u32) {
    for _ in 0..3 {
        let src = rgba.to_vec();
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let a = src[i + 3];
                if a == 0 || a == 255 {
                    continue;
                }
                let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                            continue;
                        }
                        let j = ((ny as u32 * w + nx as u32) * 4) as usize;
                        if src[j + 3] == 255 {
                            r += src[j] as u32;
                            g += src[j + 1] as u32;
                            b += src[j + 2] as u32;
                            n += 1;
                        }
                    }
                }
                if n == 0 {
                    // A pixel with no opaque neighbour keeps the colour it already had.
                    continue;
                }
                rgba[i] = (r / n) as u8;
                rgba[i + 1] = (g / n) as u8;
                rgba[i + 2] = (b / n) as u8;
            }
        }
    }
}

fn read(path: &Path) -> Option<Art> {
    let (rgba, w, h) = art::decode_rgba(path)?;
    if w < 8 || h < 8 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    let mut rgba = rgba;
    bleed_edge(&mut rgba, w, h);
    // Note: hardening this alpha was tried and made things worse. A cut-out's antialiased edge is
    // partly transparent, and a side slot draws its cart at 0.7 over a black silhouette, so the
    // silhouette shows through that band as a dark rim on a light theme. Pushing the edge to 0 or
    // 255 turns the soft band into a hard transparent one and lets *more* black through. The rim
    // is the silhouette's, not the edge's: see `Shelf::draw_row`, which does not draw it under a
    // cart that came with its own art.
    let label = find_label(&rgba, w, h);
    let luma = reference_luma(&rgba, w, h, label.as_ref());
    let recess = label
        .as_ref()
        .map(|l| ring_colour(&rgba, w, h, l))
        .unwrap_or([80, 80, 80]);
    Some(Art {
        rgba,
        w,
        h,
        label,
        luma,
        recess,
    })
}

fn is_mark(p: &[u8]) -> bool {
    p[3] > 0 && p[0] > MARK_R && p[1] < MARK_G && p[2] > MARK_B
}

fn find_label(rgba: &[u8], w: u32, h: u32) -> Option<Label> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            if is_mark(&rgba[((y * w + x) * 4) as usize..][..4]) {
                any = true;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if !any {
        return None;
    }
    let mut mask = vec![0u8; (w * h) as usize];
    for y in y0..=y1 {
        for x in x0..=x1 {
            if is_mark(&rgba[((y * w + x) * 4) as usize..][..4]) {
                mask[(y * w + x) as usize] = 255;
            }
        }
    }
    // ★ 标记连同它自己那圈抗锯齿一起算 —— 见 `MARK_GROW`。**读快照不读自己**：一边长一边读会让
    // 刚长出去的一圈再长一圈，那就不是"两格"而是"一直长到碰边"。
    let base = mask.clone();
    let r = MARK_GROW as i32;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if base[i] != 0 {
                continue;
            }
            'ring: for dy in -r..=r {
                for dx in -r..=r {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    if base[(ny as u32 * w + nx as u32) as usize] != 0 {
                        mask[i] = 255;
                        break 'ring;
                    }
                }
            }
        }
    }
    Some(Label {
        x0: x0.saturating_sub(MARK_GROW),
        y0: y0.saturating_sub(MARK_GROW),
        x1: (x1 + MARK_GROW).min(w - 1),
        y1: (y1 + MARK_GROW).min(h - 1),
        mask,
    })
}

/// The shell's own brightness, ignoring the label slot.
fn reference_luma(rgba: &[u8], w: u32, h: u32, label: Option<&Label>) -> f32 {
    let mut lums = Vec::new();
    for y in (0..h).step_by(3) {
        for x in (0..w).step_by(3) {
            let inside = label.is_some_and(|l| x >= l.x0 && x <= l.x1 && y >= l.y0 && y <= l.y1);
            let p = &rgba[((y * w + x) * 4) as usize..][..4];
            if p[3] > 200 && !inside {
                lums.push(0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32);
            }
        }
    }
    if lums.is_empty() {
        return 128.0;
    }
    lums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let i = ((lums.len() - 1) as f32 * REF_PERCENTILE) as usize;
    lums[i].max(1.0)
}

/// Just outside the slot all the way round, median per channel.
fn ring_colour(rgba: &[u8], w: u32, h: u32, l: &Label) -> [u8; 3] {
    let mut samples: Vec<[u8; 3]> = Vec::new();
    let mut take = |x: u32, y: u32| {
        if x < w && y < h {
            let p = &rgba[((y * w + x) * 4) as usize..][..4];
            if p[3] > 0 {
                samples.push([p[0], p[1], p[2]]);
            }
        }
    };
    for x in l.x0..=l.x1 {
        take(x, l.y0.saturating_sub(3));
        take(x, (l.y1 + 3).min(h - 1));
    }
    for y in l.y0..=l.y1 {
        take(l.x0.saturating_sub(3), y);
        take((l.x1 + 3).min(w - 1), y);
    }
    if samples.is_empty() {
        return [80, 80, 80];
    }
    let mut out = [0u8; 3];
    for c in 0..3 {
        let mut v: Vec<u8> = samples.iter().map(|s| s[c]).collect();
        v.sort_unstable();
        out[c] = v[v.len() / 2];
    }
    out
}

/// Where the card's own label slot sits, as a share of the cart art: `x0, y0, x1, y1`. This is
/// the magenta region, which is where the label actually goes on this cart — so anything that
/// has to line up with the label has to line up with this rather than with the built-in panel.
pub fn label_share(system: System) -> Option<(f32, f32, f32, f32)> {
    art_for(system)?.label_share()
}

impl Art {
    /// The label slot as a share of the art, for a caller aligning something to it — the shelf's
    /// favourites star sits on its top right corner.
    pub fn label_share(&self) -> Option<(f32, f32, f32, f32)> {
        let l = self.label.as_ref()?;
        Some((
            l.x0 as f32 / self.w as f32,
            l.y0 as f32 / self.h as f32,
            (l.x1 + 1) as f32 / self.w as f32,
            (l.y1 + 1) as f32 / self.h as f32,
        ))
    }

    /// The label slot in face pixels, so the caller can mint a label of the right size. The
    /// target face is the machine's own, which is why this cannot be a constant: the same art
    /// drawn on a taller cart has a taller slot.
    pub fn label_size(&self, size: CartSize) -> Option<(u32, u32)> {
        let l = self.label.as_ref()?;
        let w = ((l.x1 - l.x0 + 1) as f32 * size.face_w() as f32 / self.w as f32).round() as u32;
        let h = ((l.y1 - l.y0 + 1) as f32 * size.face_h() as f32 / self.h as f32).round() as u32;
        (w > 0 && h > 0).then_some((w, h))
    }

    /// The face the shelf draws: this art, in the cart's shell colour, with `label` pasted into
    /// the slot the art marks out.
    ///
    /// The colour is applied as a multiplier on the art's own brightness — `shell * luma/luma`
    /// for the shell's own value at the reference brightness — which is what the built-in path
    /// does too, where the shell colour is painted through the silhouette and the moulding is
    /// the same colour turned away from the light. So a card that paints a dark shell with a lit
    /// edge keeps the edge when the shell colour changes, which is the whole point of tinting
    /// rather than recolouring.
    /// `light` is what this machine's cart becomes on the light theme: `Some(grey)` to repaint
    /// it flat, `None` to use the art's own colour untouched. The dark theme always uses the
    /// shell colour, which is what the art was drawn to sit under.
    pub fn face(
        &self,
        shell: &Shell,
        label: &[u8],
        label_size: (u32, u32),
        face_size: (u32, u32),
        light: Option<[u8; 3]>,
    ) -> CartFace {
        let (label_w, label_h) = label_size;
        let (face_w, face_h) = face_size;
        let mut rgba = vec![0u8; (face_w * face_h * 4) as usize];
        let sx = self.w as f32 / face_w as f32;
        let sy = self.h as f32 / face_h as f32;
        let l = self.label.as_ref();
        // The slot in face pixels, so the fill and the paste land on the same rectangle the
        // label was minted for.
        let (lx, ly) = match l {
            Some(l) => (
                (l.x0 as f32 / sx).round() as i64,
                (l.y0 as f32 / sy).round() as i64,
            ),
            None => (0, 0),
        };
        for y in 0..face_h {
            let sy0 = y as f32 * sy;
            for x in 0..face_w {
                let sx0 = x as f32 * sx;
                let p = &self.rgba[(((sy0 as u32).min(self.h - 1) * self.w
                    + (sx0 as u32).min(self.w - 1))
                    * 4) as usize..][..4];
                if p[3] == 0 {
                    continue;
                }
                let in_slot = label_w > 0
                    && l.is_some()
                    && (x as i64) >= lx
                    && (x as i64) < lx + label_w as i64
                    && (y as i64) >= ly
                    && (y as i64) < ly + label_h as i64;
                let src = if in_slot {
                    // The recess the label sits in: the card's own colour there, held back a
                    // little, so a rounded label's corners read as the well it is stuck into.
                    [
                        (self.recess[0] as f32 * 0.78) as u8,
                        (self.recess[1] as f32 * 0.78) as u8,
                        (self.recess[2] as f32 * 0.78) as u8,
                        255,
                    ]
                } else {
                    let luma = 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
                    // The dark theme paints the shell colour through the art's own shading, which
                    // is what the art is drawn for. Under the light theme that came out too dark
                    // against paler furniture, so a cart keeps its own colour there and the
                    // Advance cart — whose art is a dark grey drawing — is repainted a light one.
                    let (base, k) = match crate::palette::mode() {
                        crate::palette::Mode::Light => match light {
                            None => ([p[0], p[1], p[2]], 1.0),
                            Some(flat) => (flat, (luma / self.luma).clamp(0.0, 1.6)),
                        },
                        crate::palette::Mode::Dark => {
                            let c = match shell.finish {
                                Finish::Solid => shell.colour,
                                Finish::Translucent => shell.colour,
                            };
                            (c, (luma / self.luma).clamp(0.0, 1.6))
                        }
                    };
                    [
                        (base[0] as f32 * k).min(255.0) as u8,
                        (base[1] as f32 * k).min(255.0) as u8,
                        (base[2] as f32 * k).min(255.0) as u8,
                        p[3],
                    ]
                };
                let d = ((y * face_w + x) * 4) as usize;
                rgba[d..d + 4].copy_from_slice(&src);
            }
        }
        // The label itself, clipped to whatever shape the card drew its slot with.
        if let (Some(l), true) = (l, label_w > 0 && label_h > 0) {
            let mx = l.x1 - l.x0 + 1;
            let my = l.y1 - l.y0 + 1;
            for y in 0..label_h {
                for x in 0..label_w {
                    // Where this face pixel lands in the art's own slot mask.
                    let ax = l.x0 + (x as f32 * mx as f32 / label_w as f32) as u32;
                    let ay = l.y0 + (y as f32 * my as f32 / label_h as f32) as u32;
                    if l.mask[(ay.min(self.h - 1) * self.w + ax.min(self.w - 1)) as usize] == 0 {
                        continue;
                    }
                    let s = ((y * label_w + x) * 4) as usize;
                    let a = label[s + 3] as u32;
                    if a == 0 {
                        continue;
                    }
                    let d = (((y as i64 + ly).max(0) as u32 * face_w
                        + (x as i64 + lx).max(0) as u32)
                        * 4) as usize;
                    if d + 4 > rgba.len() {
                        continue;
                    }
                    for c in 0..3 {
                        let dst = rgba[d + c] as u32;
                        let src = label[s + c] as u32;
                        rgba[d + c] = ((src * a + dst * (255 - a) + 127) / 255) as u8;
                    }
                }
            }
        }
        CartFace {
            rgba,
            w: face_w,
            h: face_h,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// A `w×h` sheet: a magenta block of half-width `m`, **wrapped in one pixel of magenta
    /// blended halfway into the shell** — `(248,87,246)`, which is the value measured on the
    /// card's own `cd.png` where the mark meets the pale shell. That pixel fails `is_mark`.
    fn art_with_a_blended_edge(w: u32, h: u32, m: u32) -> Vec<u8> {
        let mut px = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let d = [x, y, w - 1 - x, h - 1 - y].into_iter().min().unwrap();
                let c: [u8; 4] = if d > m {
                    [255, 0, 255, 255]
                } else if d == m {
                    [248, 87, 246, 255]
                } else {
                    [40, 40, 44, 255]
                };
                let i = ((y * w + x) * 4) as usize;
                px[i..i + 4].copy_from_slice(&c);
            }
        }
        px
    }

    /// ★ 2026-10-07 六六：「DISC 货架我切回亮色模式发现封面区还有点洋红边」。
    ///
    /// That ring is **not** the sticker failing to fill its slot. It is the one pixel of blend a
    /// paint program leaves along the mark, and it fails `is_mark` — so it is neither filled with
    /// the recess nor pasted over, and a card that passes no `light` colour draws it **as the art
    /// itself**. The dark theme never showed it because there that pixel comes out in
    /// `shell.colour`, which is exactly why it went unnoticed.
    ///
    /// The guard is the rule — "a mark includes its own anti-aliased edge" — and not a pixel
    /// count: it says the blend is inside the mask and that the mask stops where `MARK_GROW` says.
    #[test]
    fn the_mark_swallows_its_own_anti_aliased_edge() {
        let (w, h, m) = (40u32, 40u32, 10u32);
        let px = art_with_a_blended_edge(w, h, m);
        let l = find_label(&px, w, h).expect("a magenta block has to be found");

        // The strict block is `m + 1 ..= w - 2 - m`; the box has to have grown by `MARK_GROW` on
        // every side or the expanded mask would be cut off again by the paste.
        let (lo, hi) = (m + 1, w - 2 - m);
        assert_eq!((l.x0, l.y0), (lo - MARK_GROW, lo - MARK_GROW), "box did not grow");
        assert_eq!((l.x1, l.y1), (hi + MARK_GROW, hi + MARK_GROW), "box did not grow");

        let at = |x: u32, y: u32| l.mask[(y * w + x) as usize];
        assert_ne!(at(m, 20), 0, "the blended ring was left out of the mark");
        assert_ne!(at(lo - MARK_GROW, 20), 0, "the last grown pixel was left out");
        assert_eq!(at(lo - MARK_GROW - 1, 20), 0, "the grow ran past MARK_GROW");
        assert_eq!(at(0, 0), 0, "the shell in the corner was eaten");
    }
}

#[cfg(test)]
mod mask_tests {
    use super::*;

    /// 一张 4×2 的贴图：左半不透明，右半全透明。
    fn half_opaque() -> Art {
        let mut rgba = Vec::new();
        for _ in 0..2 {
            for x in 0..4 {
                let a = if x < 2 { 255 } else { 0 };
                rgba.extend_from_slice(&[0x80, 0x80, 0x80, a]);
            }
        }
        Art {
            rgba,
            w: 4,
            h: 2,
            label: None,
            luma: 0.5,
            recess: [0, 0, 0],
        }
    }

    /// `Alpha` 是轮廓的唯一来源（`silhouette::masks` 拿它当掩膜）。重采样到别的尺寸时
    /// 只搬 alpha：不透明那半边仍然是满的，透明那半边仍然是空的。
    ///
    /// ⚠️ 2026-10-25：这里原来还有两条，钉的是 `Art::mask` / `Art::silhouette`
    /// （「阴影 = 同一个 alpha 涂黑」）。**侧位卡带那块黑影整条删除之后那两个函数没有了**，
    /// 阴影相关的断言随之消失 —— 不要再加回来（来历见 `MEMORY-卡内布局.md`）。
    #[test]
    fn an_alpha_resamples_to_the_size_it_is_asked_for() {
        let a = half_opaque();
        let alpha = Alpha {
            bytes: a.rgba.chunks_exact(4).map(|p| p[3]).collect(),
            w: 4,
            h: 2,
        };
        let m = alpha.resized(8, 4);
        assert_eq!(m.len(), 32);
        // 8 宽时左一半（x<4）不透明。
        assert_eq!(m[0], 255);
        assert_eq!(m[7], 0);
    }
}

#[cfg(test)]
mod default_tests {
    use super::*;

    /// 一张 `w×h` 全洋红的不透明图 —— 一个只有贴纸区、没有壳的极端缺省图，够 `for_dir`
    /// 和 `tinted` 用。
    fn mark_art(w: u32, h: u32) -> DefaultArt {
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for p in rgba.chunks_exact_mut(4) {
            p[0] = 0xff;
            p[1] = 0x00;
            p[2] = 0xff;
            p[3] = 0xff;
        }
        let label = find_label(&rgba, w, h);
        DefaultArt { rgba, w, h, label }
    }

    fn empty_set() -> DefaultArtSet {
        DefaultArtSet {
            gba: None,
            gb: None,
            gbc: None,
            md: None,
            ps1: None,
            fc: None,
            nes: None,
            sfc: None,
            snes: None,
            neogeo: None,
            arcade: None,
        }
    }

    /// ★ 2026-10-26（六六）：「FC/NES和SFC/SNES根据Games文件夹的命名决定用哪个」——
    /// 缺省图按**目录名**键，不是按机种：FC 与 NES 是两张不同的图（`System` 都是 `Fc`），
    /// SFC 与 SNES 同理。目录名是唯一能分开它们的东西，`System::dir_name` 会把 NES 拍平回
    /// FC —— 所以这里用 `Cart::dir_name`（原始拼法），不经过 `System`。
    #[test]
    fn a_default_is_keyed_by_directory_name_not_machine() {
        let mut set = empty_set();
        set.fc = Some(mark_art(2, 2));
        set.nes = Some(mark_art(3, 3));
        set.sfc = Some(mark_art(4, 4));
        set.snes = Some(mark_art(5, 5));

        // 同一个机种，两张图，按目录各取各的。
        assert_eq!(set.for_dir("FC").unwrap().w, 2, "FC 目录用 FC 图");
        assert_eq!(set.for_dir("NES").unwrap().w, 3, "NES 目录用 NES 图");
        assert_eq!(set.for_dir("SFC").unwrap().w, 4, "SFC 目录用 SFC 图");
        assert_eq!(set.for_dir("SNES").unwrap().w, 5, "SNES 目录用 SNES 图");

        // 大小写不敏感（与 `System::from_dir` 同）。
        assert_eq!(set.for_dir("fc").unwrap().w, 2);
        assert_eq!(set.for_dir("nes").unwrap().w, 3);
    }

    /// 别名目录（GENESIS/MEGADRIVE → MD、PSX → PS1）落到同一张图上；一个不是机种目录的
    /// 名字（裸 rom、乱起的文件夹）落空为 `None`。
    #[test]
    fn aliases_fall_onto_the_same_picture_and_junk_falls_through() {
        let mut set = empty_set();
        set.md = Some(mark_art(2, 2));
        set.ps1 = Some(mark_art(3, 3));

        assert!(set.for_dir("MD").is_some());
        assert!(set.for_dir("GENESIS").is_some());
        assert!(set.for_dir("megadrive").is_some());
        assert!(set.for_dir("PS1").is_some());
        assert!(set.for_dir("PSX").is_some());

        // 没有缺省图的机种。
        assert!(set.for_dir("NEOGEO").is_none());
        assert!(set.for_dir("ARCADE").is_none());
        assert!(set.for_dir("PCE").is_none());
        // 非目录名：直接放 `Games/` 的 rom、乱起的文件夹。
        assert!(set.for_dir("POKEMON EMER.gba").is_none());
        assert!(set.for_dir("随便").is_none());
    }

    /// `tinted` 把洋红区换成指定色、且不碰原图 —— 共享的缺省图字节必须原样，因为每张卡
    /// 的颜色不同，填色只发生在卡自己的副本上。
    #[test]
    fn tinted_replaces_the_mark_and_leaves_the_original_alone() {
        let art = mark_art(4, 4);
        let out = art.tinted([0x12, 0x34, 0x56]);
        // 全部被填成目标色。
        assert!(out.chunks_exact(4).all(|p| p[0] == 0x12 && p[1] == 0x34 && p[2] == 0x56));
        // 原图还是洋红。
        assert!(art
            .rgba
            .chunks_exact(4)
            .all(|p| p[0] == 0xff && p[1] == 0x00 && p[2] == 0xff));
    }

    /// ★★ 2026-11-07（六六：「洋红色字体糊了」）：**抗锯齿的洋红边要跟着一起染**。
    ///
    /// 旧实现只认纯洋红，图中那条 `(128,0,128)` 的过渡像素原样留下 —— 真机上就是字的心里
    /// 换了色、字边一圈洋红，读作"糊"。这条测试把那一档钉住：它必须被染成目标色与自身的
    /// 混合（介于两者之间），而不是保持纯洋红。
    #[test]
    fn tinted_takes_the_antialiased_edge_with_it() {
        // 一行六格：纯洋红 / 纯洋红 / 半洋红（抗锯齿边）/ 黑底 / 白 / 纯洋红。
        // 黑白夹在框里，才能顺带验证"不是洋红的一律不动"。
        let rgba = vec![
            0xff, 0x00, 0xff, 0xff, //
            0xff, 0x00, 0xff, 0xff, //
            0x80, 0x00, 0x80, 0xff, // 半洋红
            0x0f, 0x0f, 0x0f, 0xff, // 黑底
            0xff, 0xff, 0xff, 0xff, // 白
            0xff, 0x00, 0xff, 0xff, //
        ];
        let label = find_label(&rgba, 6, 1);
        let art = DefaultArt {
            rgba,
            w: 6,
            h: 1,
            label,
        };
        let out = art.tinted([0x10, 0x20, 0x30]);
        // 纯洋红 → 完全变成目标色。
        assert_eq!(&out[0..3], &[0x10, 0x20, 0x30]);
        assert_eq!(&out[20..23], &[0x10, 0x20, 0x30]);
        // 半洋红 → 这一像素里 50% 是洋红、50% 是黑底，所以出目标色的**一半**、且一点洋红
        // 都不剩：`(128,0,128) + 0.502 * (colour - 洋红)` ≈ `(8,16,24)`。
        // 旧写法 `lerp(P, colour, k)` 在这里会给出 `(72,16,84)` —— 蓝色被洋红顶高一倍，
        // 串起来就是一圈紫描边。
        assert!((out[8] as i32 - 8).abs() <= 2, "r = {}", out[8]);
        assert!((out[9] as i32 - 16).abs() <= 2, "g = {}", out[9]);
        assert!((out[10] as i32 - 24).abs() <= 2, "b = {}", out[10]);
        // 黑底与白一动不动。
        assert_eq!(&out[12..15], &[0x0f, 0x0f, 0x0f]);
        assert_eq!(&out[16..19], &[0xff, 0xff, 0xff]);
    }
}
