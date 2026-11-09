use slot_store::{ArtSource, Cart};

use crate::art;
use crate::shell::{shell_for, Finish, Shell};
use crate::silhouette::{cart_depth, cart_mask, detail_mask, Shape};
use crate::text;
use std::sync::atomic::{AtomicU8, Ordering};

/// Where the time inside `cart_face` goes, accumulated over every call since the last take.
///
/// A cart face costs something like 70 ms on this hardware, and the device has no profiler
/// and no console: this is how "which pass is actually expensive" gets answered from a
/// `boot.log` on a card instead of by argument.
pub mod face_profile {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    pub const PHASES: [&str; 6] = ["shell", "label", "mould", "recess", "paste", "clip"];

    static NANOS: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];

    pub fn add(phase: usize, d: Duration) {
        if let Some(slot) = NANOS.get(phase) {
            slot.fetch_add(d.as_nanos() as u64, Ordering::Relaxed);
        }
    }

    /// Milliseconds per phase, and resets the counters.
    pub fn take_ms() -> [f64; 6] {
        let mut out = [0.0; 6];
        for (i, slot) in NANOS.iter().enumerate() {
            out[i] = slot.swap(0, Ordering::Relaxed) as f64 / 1e6;
        }
        out
    }

    /// `shell 12.3 label 4.5 ...` — one line, for the boot log.
    pub fn line() -> String {
        let ms = take_ms();
        PHASES
            .iter()
            .zip(ms)
            .map(|(name, v)| format!("{name} {v:.1}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// The traced outline's own aspect, so `cart.svg` rasterises unstretched. Three across a
/// 720 wide row exactly, so the shelf can show a neighbour either side of the selection.
pub const CART_W: u32 = 240;
pub const CART_H: u32 = 135;
/// Internal face resolution multiplier. The cart face (shell, label, masks) is rasterised and
/// composited at `FACE_SCALE` times the logical cart size, then the shelf draws it into the
/// logical `CART_W`×`CART_H` quad. This keeps the centre cart crisp when the shelf scales it up
/// to 1.5x (and any larger scale): the texture is only ever downsampled, never stretched.
pub const FACE_SCALE: u32 = 2;
pub const FACE_W: u32 = CART_W * FACE_SCALE;
pub const FACE_H: u32 = CART_H * FACE_SCALE;

/// A cart's own size, per machine — and the row it stands on.
///
/// ★ 2026-10-23 23:5x 六六定的规矩（最终版）：**主卡居中画面，脚线由它逆推，两侧对称**。
///
/// * **主卡（中间那张）在任何比例下都居中**：水平居中于 720，垂直也居中于 480 ⇒
///   脚线 `foot_y = 240 + 主卡高/2`。**脚线不是从屏幕推出来的，是从主卡反推出来的** ——
///   所以换机种、换比例，主卡始终在画面正中，两侧跟着它站。
/// * **屏上尺寸是硬约束，按卡的朝向分两档**：横版（Advance，以及 SFC/FC/NeoGeo/街机）
///   中间卡 **360 宽**、两侧 **240 宽**；竖版/方形（Game Boy / Color）中间卡 **300 高**、
///   两侧 **200 高**（宽随比例）。整卡贴图按这两档预缩放存入；尺寸不符照样缩
///   （`art::stretch` 拉到面尺寸），按盒子自身宽高比画的图落上去不变形。
/// * **两侧的位置由两条不同的尺子定，都不是"贴上去"**：
///   - **横版：侧卡露一半在画面内** ⇒ 侧卡中心正好落在屏幕边上（x = 0 / 720），
///     240 里看得见 120。圆心距因此是 **360**。
///   - **竖版：侧卡中心取「主卡侧缘」与「屏幕边」的中点** ⇒ 右卡中心
///     `(360 + 主宽/2 + 720) / 2`，三张**全框**、左右对称、间距 ≈27.9。
///
/// ⚠️ **2026-10-23 23:1x 那版把圆心距改成 `(中宽+侧宽)/2`（零间距紧贴）是错的** ——
///   那是"把侧卡推到主卡身上"，六六 23:5x 明确否掉：「GBA 候选卡带不齐，残留之前取消的
///   排列方式」。横版侧卡要露一半，竖版要站在中点上，**两种都不是零间距**。
///
/// The label panel is per mille of the face, x0 y0 x1 y1. The Advance cart's is the reference's;
/// the Game Boy's is measured (42 x 37 mm on a 57 x 65.5 mm shell — 7.5 mm in from each side,
/// 6 mm down), and nearly square where the Advance cart's is wide.
#[derive(Copy, Clone)]
pub struct CartSize {
    pub w: u32,
    pub h: u32,
    pub pitch: f32,
    /// How much bigger the selected cart is than its neighbours.
    pub middle: f32,
    /// And how big the neighbours are, as a share of their own size.
    pub side: f32,
    /// How far **below** the derived foot line this row stands, in logical pixels.
    ///
    /// **Zero for every cartridge row** (2026-10-23 23:5x): a cart's foot is downstream of the
    /// main cart being centred, so a machine row has nothing left to nudge. A row that is not a cartridge may want a different balance — a tape is short
    /// (135 px) and wide, and sitting on the same line as an Advance cart left it jammed against
    /// the letter ring with a wide band under it — and this is the knob for that, rather than
    /// the alternative of nudging `h`, which lands the row in `foot_y`'s portrait branch and
    /// throws it 32 px the *other* way.
    pub drop: f32,
    label: [u32; 4],
}

pub const fn size_for(system: slot_store::System) -> CartSize {
    match system {
        // ★★ 2026-10-27 23:1x（六六）：「**我给的都是不需要重缩放的，按一比一像素就行了**」。
        //
        //   这一句把整张表的规矩定死了：**中卡 = 贴图自己的像素尺寸，1:1 印上去**。
        //   （PS1 那条 10-26 的注释、MD 那条 10-27 的注释其实已经在按这条做，只是没有
        //   推广到全表 —— 其余机种还留着"按 alpha 比例算高度"的老算法。）
        //
        //   基座仍是 `middle = 1.5`，所以 `h = round(贴图高 / 1.5)`；中卡尺寸与贴图最多
        //   差半像素（`h` 是整数），MD 那条已经接受过这个误差。
        //   GBA 贴图 **360×215** ⇒ 240×143（中卡 360×214.5）。
        slot_store::System::Gba => CartSize {
            w: CART_W,
            h: 143,
            // ★ 2026-10-23 23:5x（六六）：「横版露一半在画面内」—— 侧卡中心落在屏幕边上
            //   （左右 x = 0 / 720），240 里只见 120。圆心距 360 就是这个事实的直接结果：
            //   主卡居中 360，侧卡中心 0 / 720。**不是零间距**。
            pitch: 360.0,
            // 中间 360 宽 / 两侧 240 宽。
            middle: 1.5,
            side: 1.0,
            drop: 0.0,
            label: [90, 228, 910, 863],
        },
        // ★★ 2026-10-27 23:1x：贴图 **300×340** ⇒ 基座 200×227，中卡 **300×340.5**（1:1）。
        //
        //   原来是基座 240×276 / `middle = 300/276`（中卡 260.9×300）—— 那是"主卡 300 高"
        //   的旧约束，不是贴图的尺寸，所以图被横向压了 13%。
        //
        //   **副作用**（六六要的话可以再调）：中卡从 260.9 宽变 300 宽，侧卡仍按"200 高"
        //   （`side = 200/227`，宽 176.2），于是主卡与侧卡的间距从 **27.8 px 收到 16.9 px**。
        //   要保住 27.8 就得把侧卡缩到 175 高 —— 那是另一条命令。
        slot_store::System::Gb | slot_store::System::Gbc => CartSize {
            w: 200,
            h: 227,
            // ★ 2026-10-23 23:5x（六六）：「竖版取中间卡带对应侧与屏幕边的中点」——
            //   侧卡**中心**站在 (主卡右缘 + 720)/2 上。主卡右缘 = 360 + 150 = 510，
            //   中点 615 ⇒ 圆心距 **255**，侧卡 526.9..703.1 全框、左右对称。
            pitch: 255.0,
            // 中卡 = 贴图原尺寸：200 × 1.5 = 300 宽、227 × 1.5 = 340.5 高。
            middle: 1.5,
            // 侧卡固定 200 高（原来也是这条规矩），宽随之 176.2。
            side: 200.0 / 227.0,
            // 脚线由主卡居中逆推：240 + 340.5/2 = 410.25，卡顶 y=69.75，整张卡在窗口
            // （y10–429）里居中。
            drop: 0.0,
            label: [132, 92, 868, 657],
        },
        // ★★ 2026-10-24（六六）：「**SFC/FC 卡带比例不对，像是被拉伸了**」。
        //
        //   根因：这两个机种借的是 Advance 的横版盒子，而卡上的贴图是实物翻拍，自带形状
        //   不是盒子的比例。★ 2026-10-27 23:1x 起改用**贴图原尺寸**这条更硬的规矩
        //   （六六：「我给的都是不需要重缩放的」）：贴图 **360×266** ⇒ 基座 240×177，
        //   中卡 360×265.5。原来那条"按 alpha 比例 1.3534 算 1.4521"的老注释作废。
        slot_store::System::Sfc => CartSize {
            w: CART_W,
            h: 177,
            pitch: 360.0,
            middle: 1.5,
            side: 1.0,
            drop: 0.0,
            label: [90, 228, 910, 863],
        },
        // ★★ 2026-10-27 23:1x：贴图 **360×229** ⇒ 基座 240×153，中卡 360×229.5。
        slot_store::System::Fc => CartSize {
            w: CART_W,
            h: 153,
            pitch: 360.0,
            middle: 1.5,
            side: 1.0,
            drop: 0.0,
            label: [90, 228, 910, 863],
        },
        // Arcade and NeoGeo still have no cartridge art of their own, so they keep the
        // landscape box the built-in Advance silhouette is drawn in (see `Shape::from`).
        //
        // ★ **MD joins them as a placeholder, and the height is deliberately left at
        // `CART_H`** (2026-10-25). SFC and FC earned their own heights only *after* the card
        // carried `cart_sfc.png` / `cart_fc.png` and the alpha's bounding box could be measured
        // (1.4521 and 1.528, see above). There is no `cart_md.png` on the card yet, so there
        // is nothing to measure and a guessed height would put a real number in this table
        // that nobody could stand behind. When 六六 supplies the picture, the job is exactly
        // what SFC/FC got: measure the alpha box, set `h = round(240 / aspect)`, add the
        // measurement to the comment.
        // ★★ 2026-10-27（六六）：**ARCADE / NEOGEO 的载体是代币**（「等同于卡带/光盘」）。
        //   素材 `66SLOT_DISC_ARCADE.png` / `66SLOT_DISC_NEOGEO.png` 都是 **300×300** 的圆牌，
        //   所以盒子跟 PS1 那条一样走正方形：基座 200×200、`middle=1.5` ⇒ 中卡 **300×300**、
        //   侧卡 200×200、`pitch=300`。原来借 GBA 的 240×135（16:9）会把圆牌拉扁。
        slot_store::System::Neogeo | slot_store::System::Arcade => CartSize {
            w: 200,
            h: 200,
            pitch: 300.0,
            middle: 1.5,
            side: 1.0,
            drop: 0.0,
            label: [90, 228, 910, 863],
        },
        slot_store::System::Pce => CartSize {
            w: CART_W,
            h: CART_H,
            pitch: 360.0,
            middle: 1.5,
            side: 1.0,
            drop: 0.0,
            label: [90, 228, 910, 863],
        },
        // ★★ 2026-10-27（六六）：「**MD 的插卡动画解决了** …… 外形是 gba 最早的那个兜底壳」。
        //
        //   上一版 MD 借的是 Advance 的 240×135（16:9）占位盒，理由写在原来的注释里：
        //   卡上没有 `cart_md.png`，没有东西可量。现在卡上有 **`66SLOT_CART_MD.png`**
        //   （缺省卡带图，360×244，自带 alpha 轮廓），可以量了：
        //
        //   实测（2026-10-27，按 alpha>8 取外接框）：canvas **360×244**，bbox **360×245**，
        //   比例 **1.4694** ⇒ `h = round(240 / 1.4694) = ` **163**。
        //   中卡 1.5× 之后是 **360×244.5** —— 与那张图自己的画布 360×244 差半像素，
        //   也就等于"这张图按 1:1 印在落位的卡带上"。原来的 135 会把它横向拉长 **21%**，
        //   那就是六六眼里"gba 最早那个兜底壳"的来源：盒子是 Advance 的，图被摊平了。
        //
        //   动画本身不用动：这里只改盒子，`Seat::from` 给 MD 的仍是 `Partly(0.50)`
        //   （插一半），两段动作（抬头 40 px → `u²` 下坠）与 GBA 逐行相同。
        slot_store::System::Md => CartSize {
            w: CART_W,
            h: 163,
            pitch: 360.0,
            middle: 1.5,
            side: 1.0,
            drop: 0.0,
            label: [90, 228, 910, 863],
        },
        // ★★ 2026-10-26（六六）：「**光盘贴图被压扁了，按一比一比例即300*300放货架**」。
        //
        //   PS1 的载体是光盘（不是卡带），光盘是正方形的。之前 PS1 借的是 Advance 的
        //   横版盒子 240×135（16:9），塞进 1.5 倍的中卡就是 360×202.5 —— 压扁。
        //
        //   现在独立一条：基座 200×200，middle=1.5 ⇒ 中卡 **300×300**（正方形），
        //   side=1.0 ⇒ 侧卡 200×200。pitch=300（与 GBA 一致，中卡宽度 = 间距）。
        //   foot_y = 240 + 200×1.5/2 = 390，卡顶 y=90、卡底 y=390 —— 与光盘插槽
        //   （DISC_SLOT_Y=430）之间留 40px 间距，插卡动画时光盘从上方落入。
        slot_store::System::Ps1 => CartSize {
            w: 200,
            h: 200,
            pitch: 300.0,
            middle: 1.5,
            side: 1.0,
            drop: 0.0,
            label: [90, 228, 910, 863],
        },
    }
}

/// ★★ 2026-10-27 23:1x（六六）：「**NES 卡带是接近竖版卡带，应该是 300 宽原比例才对**」。
///
/// `size_for` 只有**机种**这一维，而 `System::Fc` 同时代表 `Games/FC` 和 `Games/NES` 两个
/// 文件夹 —— 它们的缺省图一个是 360×229（1.572）、一个是 300×334（0.898），同一个盒子不可能
/// 都对（NES 那张被横向拉宽 **70%**）。所以盒子再开一个**按目录名**的入口：
///
/// * `NES`  —— 贴图 300×334 ⇒ 基座 200×223，中卡 **300×334.5**（1:1），侧卡 200 高、pitch 255；
/// * `SNES` —— 贴图 360×234 ⇒ 基座 240×156，中卡 **360×234**（1:1），与横版同一套缩放；
/// * 其余目录名与 `PSX` / `GENESIS` 这类别名，一律回到 `size_for(system)`。
///
/// 货架是**按文件夹**分的，一行里通常全是同一个目录，所以"一行一个盒子"这个前提仍然成立
/// （`Shelf::set_view` 会拿这一行各卡的目录去问）。
pub fn size_for_dir(dir: Option<&str>, system: slot_store::System) -> CartSize {
    let is = |name: &str| dir.is_some_and(|d| d.eq_ignore_ascii_case(name));
    if is("NES") {
        return CartSize {
            w: 200,
            h: 223,
            pitch: 255.0,
            middle: 1.5,
            side: 200.0 / 223.0,
            drop: 0.0,
            label: [132, 92, 868, 657],
        };
    }
    if is("SNES") {
        return CartSize {
            w: CART_W,
            h: 156,
            pitch: 360.0,
            middle: 1.5,
            side: 1.0,
            drop: 0.0,
            label: [90, 228, 910, 863],
        };
    }
    size_for(system)
}

/// ★★ 2026-10-27（六六）：「GB/GBC 按现在的位置，其它的按你建议的」。
///
/// **收藏夹星标的锚点**，`(fx, fy)` 是卡带矩形的分数，星标**中心**落在这里（尺寸另算，绝对
/// 20 px —— 星标是"读数"而不是画面，不跟选中放大）。
///
/// 原来这个位置是从「洋红框右上角」推的（`cart_art::label_share`）。贴图换成整张之后洋红框
/// ≈ 整张脸，那个"角"就是卡带自己的角，星标中心压在角上 ⇒ MD / NES / SNES 有一半挂到卡带
/// 上方（-6.1 / -8.6 / -4.8 px），而且各机种锚点在 0.6%~33% 之间乱跳，切机种时星标会在卡上
/// 滑一段。所以位置改为一张**写死的表**，不再跟素材走：
///
/// * **GB / GBC 保持原值**（六六 22:5x 定）—— 这两台是竖版卡，标签纸在卡面下三分之一，
///   星标落在纸角上本来就对；
/// * **圆形贴图（PS1 盘、ARCADE / NEOGEO 代币）**：星标中心放在半径 `R − 星标半高` 的 45°
///   方向上，整个星都在圆内（矩形分数放到圆上会落到盘外的空气里）；
/// * **其余横向机种**：离顶边、右边各约 1.5 个星标半径（20 px 星 ⇒ 上缘 14~20 px、
///   右距 26 px）。
///
/// 判据（`tests/star.rs` 钉住）：每个机种在**中卡与侧卡两种缩放**下，20 px 的星标都要整个
/// 落在卡带矩形里；圆形那三台还要落在内切圆里。
pub const fn star_at(system: slot_store::System) -> (f32, f32) {
    match system {
        // 现在的位置 = 洋红框右上角。竖版卡，标签纸低位，星标正好压在纸角。
        slot_store::System::Gb => (0.873, 0.291),
        slot_store::System::Gbc => (0.863, 0.329),
        // 圆：0.5 ± 0.31 是 45° 上半径 0.44 的点（0.44 × √2/2 = 0.311），星标半径 0.033。
        slot_store::System::Ps1 | slot_store::System::Arcade | slot_store::System::Neogeo => {
            (0.81, 0.19)
        }
        _ => (0.90, 0.12),
    }
}

impl CartSize {
    /// A size stated from its parts, for a shape that is not a machine.
    ///
    /// `size_for` above is the machine table; this is the door a tape comes in by, so the tape's
    /// own module can state its own numbers without this table having to learn about it. The
    /// `label` field stays private even so: it is four per-mille values, and a caller that takes
    /// it for a width and a height is the mistake that once cost every card with a game on it its
    /// boot.
    /// `drop` is how far below the derived foot line this row stands; see the field.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        w: u32,
        h: u32,
        pitch: f32,
        middle: f32,
        side: f32,
        drop: f32,
        label: [u32; 4],
    ) -> Self {
        CartSize {
            w,
            h,
            pitch,
            middle,
            side,
            drop,
            label,
        }
    }

    pub const fn face_w(&self) -> u32 {
        self.w * FACE_SCALE
    }

    pub const fn face_h(&self) -> u32 {
        self.h * FACE_SCALE
    }

    /// Where the row's feet stand: the carts stand on the row rather than float on it.
    ///
    /// A portrait cart is lifted clear of the title. The formula is right for the Advance row,
    /// where the gap it leaves under a 135 px cart is 37 px and the title has room in it. A
    /// 276 px cart pushes its foot 70 px further down, and the same gap collapses to 2 px with
    /// the title jammed against the row. Lifting the portrait row restores it — and takes the
    /// title with it, since `shelf_title_y` is derived from the foot: the row comes up by the
    /// whole lift and the title by half of it, so the gap grows by the other half.
    pub fn foot_y(&self) -> f32 {
        // ★ 2026-10-23 23:5x（六六）：「主卡带居中画面，脚线根据它来逆推」。
        //   **脚线 = 半屏 + 主卡半高**：主卡在 720×480 上双向居中，它的下缘就是这一行的
        //   脚线，两侧候选卡站在同一条线上。不再有"横版一条公式、竖版减 32"的两分支 ——
        //   那个 `- 32` 是为竖卡别贴住标题调的，而主卡一居中，标题在顶条里，两者不相干了。
        let foot = crate::draw::OUT_H as f32 / 2.0 + self.h as f32 * self.middle / 2.0;
        foot + self.drop
    }

    /// The paper label's panel on the face, in face pixels.
    pub fn label_panel(&self) -> (u32, u32, u32, u32) {
        self.label_panel_at(self.face_w(), self.face_h())
    }

    /// The same panel against any size, for a caller drawing the cart smaller than its face —
    /// the row draws placeholders at the quad's size and needs the panel in those units.
    ///
    /// **Corners, not a size**: `(x0, y0, x1, y1)`, exactly as `label` stores them. A caller
    /// that wants to fill or paste *inside* the panel wants `label_box` instead; the two were
    /// confused once and cost every card with a game on it its boot.
    pub fn label_panel_at(&self, w: u32, h: u32) -> (u32, u32, u32, u32) {
        let p = |v: u32, of: u32| (of * v + 500) / 1000;
        (
            p(self.label[0], w),
            p(self.label[1], h),
            p(self.label[2], w),
            p(self.label[3], h),
        )
    }

    /// The panel as an origin and a size — `(x, y, w, h)`. What every caller that draws into
    /// the panel wants, because a pasted label is sized, not cornered: `label_panel`'s third
    /// and fourth numbers are the far edge, and handing those to `paste_label` as a width and
    /// a height pastes a label wider and taller than the cart it is going onto.
    pub fn label_box(&self) -> (u32, u32, u32, u32) {
        let (x0, y0, x1, y1) = self.label_panel();
        (x0, y0, x1 - x0, y1 - y0)
    }
}

/// The paper label, inset in the shell rather than covering it: 9% to 91% across and 22.8%
/// to 86.3% down. The vertical placement is the reference's, and the band it leaves above is
/// the moulded grip; that asymmetry is most of what makes the face read as a cartridge
/// rather than a bordered rectangle. The horizontal inset is deliberately tighter than the
/// reference's 14.1%, which was an icon's proportion rather than a cartridge's: a real
/// label runs nearly the full width with only a thin edge of plastic beside it.
pub const fn label_panel(w: u32, h: u32) -> (u32, u32, u32, u32) {
    (
        (w * 90 + 500) / 1000,
        (h * 228 + 500) / 1000,
        (w * 910 + 500) / 1000,
        (h * 863 + 500) / 1000,
    )
}

/// The label panel is laid out at the higher face resolution, so the title text is rasterised
/// sharp and only ever downsampled when drawn at the logical cart size.
pub const LABEL_X: u32 = label_panel(FACE_W, FACE_H).0;
pub const LABEL_Y: u32 = label_panel(FACE_W, FACE_H).1;
pub const LABEL_W: u32 = label_panel(FACE_W, FACE_H).2 - LABEL_X;
pub const LABEL_H: u32 = label_panel(FACE_W, FACE_H).3 - LABEL_Y;

const PAD: u32 = 10 * FACE_SCALE;

/// How far the translucent edge reaches in. Zero at this depth exactly, so a pixel any
/// further in is the plastic's own colour.
const RIM: u32 = 4 * FACE_SCALE;

pub struct CartFace {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
}


/// A cart with nothing on it: the default shell, its moulding, and the empty recess the label
/// would sit in — no label.
///
/// The stand-in for a cart whose own face has not been built yet. A jump across the alphabet
/// crosses hundreds of carts and they cannot all be rasterised, but they can all be *a cart*:
/// this one. Drawn where that cart is, it slides past as a cartridge rather than as the label's
/// colour alone, which read as a bar of paint going by.
/// The built-in shell as the current theme wants it. On the dark theme that is the table's own
/// colour; on the light theme the **default** shell takes the housing's — the off-white the top
/// and bottom bands are drawn in — so a blank cart reads as part of the same device as the
/// furniture around it. A shell with its own colour (a Pokemon Ruby's red, a Sapphire's blue)
/// is **left alone**: the special colour is the whole of what makes that cart that cart, and
/// repainting it to housing's off-white on the light theme was the one thing the user asked to
/// stop (2026-10-17). The light theme still wants a cart to belong to the device, but it gets
/// there by lifting the shell's *value* to roughly the housing's brightness, not by overwriting
/// its hue — see [`lighten_keep_hue`] for the one call that does that.
///
/// `card_face` applies the same rule to a card's own Advance art; this is the built-in path's
/// half of it, and the reason the blank cart has to be re-minted when the theme changes and
/// not only when the machine does.
///
/// Note this reads the mode at *bake* time, so it is a colour that goes stale the moment
/// `palette::set_mode` runs: every caller of `cart_placeholder` / `cart_face` is expected to
/// re-bake after a theme change.
pub(crate) fn themed_shell(shell: &Shell) -> Shell {
    match crate::palette::mode() {
        crate::palette::Mode::Light => {
            // The default shell has no identity to keep — it is the grey every unlisted cart
            // wears — so on the light theme it takes the housing's colour and reads as furniture.
            if shell.colour == crate::shell::DEFAULT_SHELL.colour {
                let h = crate::slot_chrome::housing();
                Shell {
                    colour: [
                        (h[0] * 255.0) as u8,
                        (h[1] * 255.0) as u8,
                        (h[2] * 255.0) as u8,
                    ],
                    finish: shell.finish,
                }
            } else {
                // A coloured shell keeps its hue and saturation, lifted toward the housing's
                // own value so a light-theme cart is still brighter than its dark-theme self
                // and still reads as part of a light device.
                Shell {
                    colour: lighten_keep_hue(shell.colour),
                    finish: shell.finish,
                }
            }
        }
        crate::palette::Mode::Dark => *shell,
    }
}

/// Lift a colour toward the light theme's brightness while **keeping its hue and saturation**.
///
/// ★ 2026-10-17 六六「特殊卡带不变色」: 亮色主题下原先把整个壳色换成 `housing()`，红蓝绿
///   全成了同一片米色。这一条把 RGB → HSV，只抬 V 到接近 housing 的 luma，H/S 原样 ——
///   红的还是红、蓝的还是蓝，只是亮了一档，跟浅色设备站在一起。
///
/// **目标 V 取 `housing` 与 `1.0` 的中间**：纯白是 1.0、housing 约 0.78，中点约 0.89，比
/// Dark 下任何壳色（最高 Sapphire 蓝 luma ~0.40）都亮，但不到纯白（纯白会洗掉中饱和度
/// 壳色上的细节压模）。一个数而不是 per-shell 调过 —— 每个特殊色单独点过亮度意味着维护
/// 一张表，而这里的目标只是「亮一档」。
fn lighten_keep_hue([r, g, b]: [u8; 3]) -> [u8; 3] {
    let (h, s, _v) = rgb_to_hsv([r, g, b]);
    let h_light = crate::slot_chrome::housing();
    let v_light = 0.89_f32.min((h_light[0] + h_light[1] + h_light[2]) / 3.0 * 1.05);
    hsv_to_rgb(h, s, v_light)
}

/// 这张卡穿哪种壳 —— **GBC 是两种，不只看机种**。
///
/// ★ 2026-10-24（六六）：「GBC 有两种卡带，共通和专用，得画两份」。
///
/// 判据是卡带头 `0x143`（`slot_store::CGB_ONLY` / `CGB_DUAL`）：
///
/// * `0xC0` **GBC 专用** ⇒ `Shape::Gbc`：壳无锁扣缺口，物理上插不进初代 GB；
/// * `0x80` **双模**（CGB enhanced）⇒ `Shape::Gb`：**共通壳**，带缺口，插 GB 也能跑；
/// * `0x00` / 其它 ⇒ 初代 GB 游戏，同样是共通壳。
///
/// 这一条只对 **GBC 货架**有意义：GB 货架上的卡本来就都是共通壳，GBA 只有一种壳。
/// `cgb == 0` 同时表示"不知道"（非 Game Boy 卡 / 读不到头），那时退回机种默认，
/// 与从前完全一致。
fn shape_of(cart: &Cart) -> Shape {
    match cart.system() {
        slot_store::System::Gbc if cart.cgb != slot_store::CGB_ONLY => Shape::Gb,
        system => Shape::from(system),
    }
}

pub fn cart_placeholder(size: CartSize, system: slot_store::System) -> CartFace {    // The stand-in wears the current theme's shell rather than the table's dark grey, which is
    // what makes it belong to the light theme's furniture too.
    let shell = themed_shell(&shell_for(""));
    bare_face(Shape::from(system), &shell, size)
}

/// The built-in face pipeline for any shape: shell, moulding, the label's recess, the label
/// itself, and the whole lot cut to the outline.
///
/// Split out of `cart_face` for the tape, which wants this and nothing else — no shell table, no
/// card art, no per-phase timing. `cart_face` is this plus all three of those, and keeping the
/// order here is what stops the two from drifting apart: a label that went down before the
/// moulding would be a label with the moulding drawn over it.
pub(crate) fn built_face(
    shape: Shape,
    shell: &Shell,
    size: CartSize,
    label: &[u8],
) -> CartFace {
    let box_ = size.label_box();
    let mut face = dressed(shape, shell, size);
    paste_label(&mut face, label, box_);
    clip_to_silhouette(&mut face, shape);
    face
}

/// The same without the label: the shell, its moulding and the empty recess the paper would sit
/// in. What a face whose own art has not been built yet is drawn as.
pub(crate) fn bare_face(shape: Shape, shell: &Shell, size: CartSize) -> CartFace {
    let mut face = dressed(shape, shell, size);
    clip_to_silhouette(&mut face, shape);
    face
}

/// The three steps every face starts with, in the one order they can be in.
fn dressed(shape: Shape, shell: &Shell, size: CartSize) -> CartFace {
    let mut face = shell_face(shell, size, shape);
    mould_detail(&mut face, shell, size, shape);
    recess_label(&mut face, shell, size.label_box());
    face
}

/// Which art source outranks the other, as the settings screen's 美术资源 row left it.
///
/// A global for the reason `palette`'s mode is one: `cart_face` is called from `FaceBuilder`'s
/// worker thread while the main loop keeps drawing, and the answer has exactly one value at
/// any moment. An atomic is the honest shape of that — a parameter would have to cross a
/// thread boundary to say the same thing.
///
/// `0` is `ArtSource::Whole` and `1` is `Sticker`, the encoding `ArtSource::word` spells, so
/// the two cannot drift apart.
static ART_SOURCE: AtomicU8 = AtomicU8::new(0);

/// What the shelf is dressing carts from right now.
pub fn art_source() -> ArtSource {
    match ART_SOURCE.load(Ordering::Relaxed) {
        1 => ArtSource::Sticker,
        _ => ArtSource::Whole,
    }
}

/// Push the card's pick down before the first face is built — `App::boot` does it beside
/// `palette::set_mode`, for the same reason: a face bakes its source in.
pub fn set_art_source(s: ArtSource) {
    ART_SOURCE.store(
        match s {
            ArtSource::Whole => 0,
            ArtSource::Sticker => 1,
        },
        Ordering::Relaxed,
    );
}

/// The cart as one finished picture — `cart.art`, the whole-cart png the scan resolved off
/// `Cartridge/`. ★ 2026-10-23 六六：「把卡带贴图+洋红色贴纸的组合迭代成纯卡带贴图」——
/// the art already carries its label, so there is no magenta slot to find, no label to mint
/// or paste and no shell colour to tint it with: what is in the file is what is drawn. The
/// alpha is the cart's shape, exactly as the machine templates' alpha is, and the face is
/// clipped to the silhouette for the same reason the built-in path clips — so even a
/// rectangular export still reads as a cart.
///
/// `None` when the card carries no such file or it will not decode, which is not a failure:
/// the composite path answers for that cart, the way it always has.
fn pure_face(cart: &Cart, size: CartSize, shape: Shape) -> Option<CartFace> {
    let path = cart.art.as_deref()?;
    let (w, h) = (size.face_w(), size.face_h());
    let mut face = CartFace {
        rgba: art::stretch(path, w, h)?,
        w,
        h,
    };
    // ★★ 2026-10-24（六六）：「**GBC 现在用卡带贴图会出现兜底图的轮廓**」。
    //
    //   根因就是下一行：一张已经按真卡带形状裁好的贴图，又被内建轮廓剪了一次。两套形状
    //   本来就不一样 —— GBC 的 SVG 是「没有右上台阶 + 顶上一道鼓包」，而他手里那些贴图是
    //   实物翻拍（右上带台阶），于是贴图的两个上角被 SVG 的圆角切掉，屏幕上看到的就是
    //   **兜底图的边**，而不是贴图自己的边。实测这盘卡贴图在 480×552 面上被切掉 2115 px。
    //
    //   剪轮廓这件事本身是对的，但它只对"整块不透明导出"有意义 —— 那种图没有自己的形状，
    //   不剪就是个方片。所以判据是**这张图有没有自带 alpha 形状**：有，就用它自己的边；
    //   没有，才退回内建轮廓。
    if !has_own_outline(&face.rgba) {
        clip_to_silhouette(&mut face, shape);
    }
    Some(face)
}

/// Whether a picture already carries its own outline, i.e. at least 1% of it is clear.
///
/// A finished cart picture is a cut-out: the plastic is the shape and everything round it is
/// transparent. A raw export is a rectangle of pixels with no alpha at all, and that one wants
/// the built-in silhouette over it or it does not read as a cart.
fn has_own_outline(rgba: &[u8]) -> bool {
    let pixels = rgba.len() / 4;
    if pixels == 0 {
        return false;
    }
    let clear = rgba.chunks_exact(4).filter(|p| p[3] < 8).count();
    clear * 100 >= pixels
}

/// ★ 2026-10-26（六六）：「缺省图用于没有贴图的游戏」。
///
/// 一张卡没有自己的 `Cartridge/<stem>.png` 整图（`pure_face` 落空）时，用它机种的缺省图
/// （`System/Carts/66SLOT_*.png`）—— 那张图整张卡带已经画好（含前端名字），只有贴纸区还
/// 留着一大块洋红。**不贴标签**：把洋红区换成这个游戏自己的颜色（`label_colour`，稳定
/// hash，和旧 label 同源），所以每张卡上货架都有它自己的颜色，而同一张卡每次开机同色。
///
/// FC/NES、SFC/SNES 各两张，按卡所在目录（`Cart::dir_name`）选 —— 用户给的图就是按目录
/// 命名的（六六原话：「FC/NES和SFC/SNES根据Games文件夹的命名决定用哪个」）。
///
/// `None` = 这个目录没有缺省图（NEOGEO/ARCADE/PCE，或图没放、放坏），调用方回退 ——
/// 与从前完全一致。
fn default_face(cart: &Cart, shape: Shape) -> Option<CartFace> {
    let art = crate::cart_art::default_art_for(cart.dir_name()?)?;
    default_face_with(art, cart, shape)
}

/// The same, against a default already in hand — what the tests exercise without going through
/// the process-wide `DEFAULTS` (which can only be installed once).
fn default_face_with(
    art: &crate::cart_art::DefaultArt,
    cart: &Cart,
    shape: Shape,
) -> Option<CartFace> {
    // ★★ 2026-11-07（六六：「为什么已经是 360px 的还要先放大再缩小二次采样呢」）—— **不放大。**
    //
    // 这里以前取 `size.face_w()/face_h()`，也就是逻辑尺寸 × `FACE_SCALE`（GBA 是 480×286），
    // 于是在卡上那张**已经按中卡 1:1 画好的 360×215** 上放大 1.33 倍，GPU 再把 480 缩回屏上
    // 的 360 —— 一次毫无意义的重采样往返，`LUCKLUCK SLOT.` 的字边就是这么毛掉的（离机复刻：
    // 同一条链路 vs 1:1 直出，边缘毛刺肉眼可见）。
    //
    // `FACE_SCALE` 那套是给**内建壳**的：壳在内存里从零画，画大一点再让屏去缩才不会糊。
    // 缺省图不需要 —— 它自己的像素就是屏上的像素。
    //
    // ★★ 2026-10-27（六六）：「**换随机色吧**」。
    //
    //   20:0x 那一版我把 ARCADE/NEOGEO 的洋红区改成了主题亮色（因为进槽那枚是主题色）。
    //   他看过真机后改口要**每张卡自己的随机色** —— 也就是回到 2026-10-26 给九张缺省图定的
    //   那套（`label_colour`，按游戏名稳定 hash）。
    //   ⇒ 进槽那枚**直接复用货架烘出来的这张脸**（`App::draw_coin` 从 `Shelf::find` 里取），
    //   所以货架和进槽看到的是同一张图、同一个颜色，不存在换色。
    let (w, h) = (art.w, art.h);
    let rgba = art.tinted(label_colour(&label_text(cart)));
    let mut face = CartFace { rgba, w, h };
    // ★ 2026-10-24（六六）：「GBC 现在用卡带贴图会出现兜底图的轮廓」—— 同一套判据
    //   见 `pure_face`。缺省图是整张画好的、大多自带 alpha 轮廓，剪一次会切掉它自己的边；
    //   只有整块不透明导出才需要内建剪影。
    if !has_own_outline(&face.rgba) {
        clip_to_silhouette(&mut face, shape);
    }
    Some(face)
}

pub fn cart_face(cart: &Cart) -> CartFace {
    let system = cart.system();
    let size = size_for(system);
    // ★★ 2026-10-24（六六）：「**GBC 有两种卡带，共通和专用，得画两份**」——
    //   壳型不再只按机种问，GBC 的卡还要看它自己的头。见 `shape_of`。
    let shape = shape_of(cart);
    // ★ 2026-10-23 六六：「美术资源 整体贴图/局部贴纸」—— the switch decides **priority,
    //   not availability**. Both sources stay live either way: a cart carrying only one of
    //   the two is dressed from the one it has, and a cart carrying neither falls through to
    //   the magenta-slot shell with the game name in the slot — which is exactly what the
    //   composite path draws when the card has no `Labels/` png for it. So the whole picture
    //   is asked for first when it outranks the sticker, and when the sticker outranks it,
    //   only for a cart that has no sticker png to be composited with.
    if art_source() == ArtSource::Whole || cart.label.is_none() {
        if let Some(face) = pure_face(cart, size, shape) {
            return face;
        }
    }
    // ★ 2026-10-26（六六）：**缺省图** —— 没有自己的 `Cartridge/` 整图时，用机种缺省图
    //   （洋红区填这个游戏自己的颜色）。放在机器模板（`cart_<sys>.png` 贴标签那条旧路）和
    //   内建壳之前：缺省图是"没有贴图的游戏"的最终外观，旧路是它要取代的。
    //
    // ★ 2026-11-03（六六）：**有 `Labels/` 贴纸的卡不走缺省图**。缺省图是"两样都没有"时的
    //   兜底，而贴纸是玩家自己给的纸 —— 让缺省图先命中，贴纸就永远轮不到（缺省图整张卡
    //   都画好了，贴纸没处落）。
    if cart.label.is_none() {
        if let Some(face) = default_face(cart, shape) {
            return face;
        }
    }
    let shell = shell_for(&cart.code);
    // A card that supplies its own cart art is drawn from that instead of from the built-in
    // silhouette: see `cart_art`. Nothing below changes for a card that does not.
    if let Some(face) = card_face(cart, &shell, size) {
        return face;
    }
    // Past the card's own art the built-in shell answers to the theme as well: on the light
    // theme it takes the housing's colour, which is the same rule `card_face` has just applied
    // to an Advance cart's own art — so a card with art and a card without still read as one
    // device. On the dark theme the game's own shell colour stands.
    let shell = themed_shell(&shell);

    let (lx, ly, lw, lh) = size.label_box();

    let t = std::time::Instant::now();
    let mut face = shell_face(&shell, size, shape);
    face_profile::add(0, t.elapsed());

    let t = std::time::Instant::now();
    // ★★ 2026-10-28（六六）：「3 去掉」—— **卡面不印游戏名**（2026-10-26 就定了：名字在顶栏）。
    //   凹槽还画（那是壳的模具），但纸不再排字。
    //
    // ★★★ 2026-11-03（六六）：**纸回到"玩家给了图就贴图"**。玩家反馈「Labels 改了很不方便」——
    //   取消之后只剩整卡图一条路，门槛高。`Labels/` 里有这张卡的 png 就铺它（保持比例、
    //   居中裁进凹槽），没有才回到这块游戏自己的色。默认外观一位没变：**没图的卡仍是纯色**，
    //   不会突然多出一行字来。
    let label = label_paper(cart, lw, lh);
    face_profile::add(1, t.elapsed());

    let t = std::time::Instant::now();
    mould_detail(&mut face, &shell, size, shape);
    face_profile::add(2, t.elapsed());

    let t = std::time::Instant::now();
    recess_label(&mut face, &shell, (lx, ly, lw, lh));
    face_profile::add(3, t.elapsed());

    let t = std::time::Instant::now();
    paste_label(&mut face, &label, (lx, ly, lw, lh));
    face_profile::add(4, t.elapsed());

    let t = std::time::Instant::now();
    clip_to_silhouette(&mut face, shape);
    face_profile::add(5, t.elapsed());

    face
}

/// Colour is left alone and only alpha is cut, because the sprite pass blends straight
/// alpha rather than premultiplied.
///
/// ★ 2026-11-07：**掩膜按这张脸自己的尺寸取**，不再问 `size`。两者本来总是相等（脸的尺寸
/// 就是 `size.face_w()/face_h()`），但缺省图那条路现在按**贴图自己的像素**出脸（见
/// `default_face_with`，1:1 直出、省掉一次放大往返），尺寸与 `size` 不再相同。`masks` 的
/// 缓存键本来就是 `(Shape, w, h)`，多一份小掩膜而已，形状比例一致。
fn clip_to_silhouette(face: &mut CartFace, shape: Shape) {
    for (px, cover) in face.rgba.chunks_exact_mut(4).zip(cart_mask(shape, face.w, face.h)) {
        px[3] = ((px[3] as u32 * *cover as u32 + 127) / 255) as u8;
    }
}

/// Stable across runs, which the standard hasher is not: the same game must be the same
/// colour on every boot, or the shelf is unrecognisable from memory.
pub fn label_colour(title: &str) -> [u8; 3] {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in title.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hsv_to_rgb((h % 360) as f32, 0.52, 0.74)
}

/// The header title is capped at twelve characters, so it reads `POKEMON EMER`. The
/// filename holds the real name.
///
/// **街机不是这条路**：NeoGeo / 街机的 romset 名是硬件编号（`mslug`），
/// 没有“人写的名字”可再清洗，所以它们走 `slot_store::shown_name`
/// 去查内置的中文表。标签贴纸、字母轮、标题行都读这一个函数，
/// 所以它们不可能说出两个不同的名字。
pub fn label_text(cart: &Cart) -> String {
    slot_store::shown_name(cart.system(), &cart.stem)
}

fn shell_face(shell: &Shell, size: CartSize, shape: Shape) -> CartFace {
    let (w, h) = (size.face_w(), size.face_h());
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    let edge = rim_colour(shell.colour);
    for depth in cart_depth(shape, w, h) {
        let c = match shell.finish {
            Finish::Solid => shell.colour,
            Finish::Translucent => lerp(edge, shell.colour, (*depth as u32).min(RIM), RIM),
        };
        rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
    }
    CartFace { rgba, w, h }
}

/// Light through the plastic reads as a lighter, less saturated edge. Desaturating as well
/// as lightening is what keeps it from looking like a white outline drawn on the shell.
fn rim_colour(base: [u8; 3]) -> [u8; 3] {
    let mean = (base[0] as u16 + base[1] as u16 + base[2] as u16) / 3;
    base.map(|c| {
        let grey = (3 * c as u16 + mean) / 4;
        (grey + (255 - grey) * 2 / 5) as u8
    })
}

fn lerp(a: [u8; 3], b: [u8; 3], num: u32, den: u32) -> [u8; 3] {
    let mut out = [0u8; 3];
    for c in 0..3 {
        out[c] = ((a[c] as u32 * (den - num) + b[c] as u32 * num) / den) as u8;
    }
    out
}

/// The wall of the moulded recess the label sits in. Light comes from the upper left, so
/// the top and left walls are turned away from it and fall into shadow while the bottom and
/// right walls catch it. Painted before the label, so the label sits on the floor of the
/// recess with the wall showing around it.
const BEVEL: u32 = 3 * FACE_SCALE;

/// The grip ridge and the thumb notch, cut into the shell. Darkened rather than coloured:
/// moulded plastic is the same plastic, just turned away from the light.
fn mould_detail(face: &mut CartFace, shell: &Shell, size: CartSize, shape: Shape) {
    let dark = [
        (shell.colour[0] as f32 * 0.62) as u8,
        (shell.colour[1] as f32 * 0.62) as u8,
        (shell.colour[2] as f32 * 0.62) as u8,
    ];
    for (px, cover) in
        face.rgba
            .chunks_exact_mut(4)
            .zip(detail_mask(shape, size.face_w(), size.face_h()))
    {
        let a = *cover as u32;
        if a == 0 {
            continue;
        }
        for c in 0..3 {
            px[c] = ((dark[c] as u32 * a + px[c] as u32 * (255 - a) + 127) / 255) as u8;
        }
    }
}

fn recess_label(face: &mut CartFace, shell: &Shell, panel: (u32, u32, u32, u32)) {
    let (lx, ly, lw, lh) = panel;
    let (face_w, face_h) = (face.w, face.h);
    let shade = |c: [u8; 3], f: f32| -> [u8; 3] {
        [
            (c[0] as f32 * f).clamp(0.0, 255.0) as u8,
            (c[1] as f32 * f).clamp(0.0, 255.0) as u8,
            (c[2] as f32 * f).clamp(0.0, 255.0) as u8,
        ]
    };
    let dark = shade(shell.colour, 0.55);
    let lit = shade(shell.colour, 1.45);

    // Saturating, not plain: a label panel is per-mille of the face and **a panel that starts at
    // 0 is a legal thing to ask for** — a full-bleed sleeve cover has exactly that box, and it
    // is the built-in CD case's own. `lx - BEVEL` on 0 wraps to `u32::MAX` in release and
    // panics in debug, and the loop then runs from there, so the bevel was never drawn and the
    // face came out wrong in a way that looked like a missing recess rather than an overflow.
    let (x0, y0) = (lx.saturating_sub(BEVEL), ly.saturating_sub(BEVEL));
    let (x1, y1) = (lx + lw + BEVEL, ly + lh + BEVEL);
    let mut put = |x: u32, y: u32, c: [u8; 3]| {
        if x >= face_w || y >= face_h {
            return;
        }
        let d = ((y * face_w + x) * 4) as usize;
        face.rgba[d] = c[0];
        face.rgba[d + 1] = c[1];
        face.rgba[d + 2] = c[2];
    };
    for y in y0..y1 {
        for x in x0..x1 {
            let inside = (lx..lx + lw).contains(&x) && (ly..ly + lh).contains(&y);
            if inside {
                continue;
            }
            // Which wall a pixel belongs to: the nearer of the two edges it sits between.
            let from_top = y.saturating_sub(y0);
            let from_left = x.saturating_sub(x0);
            let from_bottom = y1.saturating_sub(y + 1);
            let from_right = x1.saturating_sub(x + 1);
            let upper = from_top.min(from_left);
            let lower = from_bottom.min(from_right);
            put(x, y, if upper <= lower { dark } else { lit });
        }
    }
}

/// Source over, so a label with an alpha channel shows the shell through it rather than
/// punching a hole in the cart.
///
/// Credited rather than trusted: `panel` is `(x, y, w, h)`, the label is `w * h` pixels, and
/// both are clipped to the face rather than assumed to fit inside it. The panel used to be
/// handed in as corners, which pasted a label a corner's width too wide and a corner's height
/// too tall, and the write past the end of the face took the whole frontend down before its
/// first frame — on any card with a game on it, so the machine came up as a boot loop. A
/// labelled cart drawn slightly wrong is a bug; a panic here is a handheld that cannot start.
fn paste_label(face: &mut CartFace, label: &[u8], panel: (u32, u32, u32, u32)) {
    let (lx, ly, lw, lh) = panel;
    let face_w = face.w;
    for y in 0..lh {
        // Past the bottom of the face, so is every row below this one.
        if y + ly >= face.h {
            break;
        }
        for x in 0..lw {
            if x + lx >= face_w {
                break;
            }
            let s = ((y * lw + x) * 4) as usize;
            // The same mistake from the other side: a label buffer shorter than the panel it
            // is being pasted into. Clipped too, for the same reason.
            let Some(src) = label.get(s..s + 4) else {
                break;
            };
            let a = src[3] as u32;
            if a == 0 {
                continue;
            }
            let d = (((y + ly) * face_w + x + lx) * 4) as usize;
            if d + 4 > face.rgba.len() {
                continue;
            }
            for (c, s) in src.iter().take(3).enumerate() {
                face.rgba[d + c] =
                    ((*s as u32 * a + face.rgba[d + c] as u32 * (255 - a) + 127) / 255) as u8;
            }
        }
    }
}

/// The card's own cart art, when it supplies any for this machine — or a template a rule
/// points this one game at. The label it gets is set at the size of the slot the art marks
/// out rather than at the built-in one, because the art is the thing that says where its
/// label goes.
fn card_face(cart: &Cart, shell: &Shell, size: CartSize) -> Option<CartFace> {
    let art = crate::cart_art::art_for_game(cart.system(), &cart.code)?;
    let (lw, lh) = art.label_size(size)?;
    // ★ 2026-11-03（六六）：**这张卡自己的 `Labels/` 图优先于这一块色** —— 模板只说明
    //   "槽在哪里、多大"，不该把玩家放的图顶掉。没图时仍是纯色（`label_paper` 的兜底）。
    let label = label_paper(cart, lw, lh);
    // ★ 2026-10-17 六六「特殊卡带不变色」: GBA 在亮色下原先被传 `Some(housing)`，整块按
    //   luma 重涂成 housing —— 红蓝绿全成同一片米色。现在传 `None`，让 `Art::face` 走
    //   「`light=None` ⇒ 亮色下保留 art 自身 RGB」的分支（和 GB/GBC 一样）。
    //   **art 的 RGB 本身**就是卡带的样貌 —— 卡上那张 `cart_gba.png` 已经是设计画好的，
    //   不需要再被重涂。
    //   这里仍可改：若将来发现 art 在浅色下太暗（毕竟它是为深色主题画的），可改成传
    //   一个「保色相抬亮度」的标记，但那要给 `Art::face` 加第二种 light 语义，比现在这步大。
    let light: Option<[u8; 3]> = None;
    Some(art.face(
        shell,
        &label,
        (lw, lh),
        (size.face_w(), size.face_h()),
        light,
    ))
}

/// 一张「没有字的纸」：按这个游戏自己的色铺满 slot。
///
/// ★★ 2026-10-28（六六）：以前这里是 `sized_label()` —— 把游戏名排进 slot（带字号自动缩、
/// 三行折行那套）。卡面**不印游戏名**之后，这条路只剩"填一块色"的作用，`text::fit` /
/// `text::draw_centred` 与那三个字号常量一并退出卡面。标题照旧由顶栏、字母轮、收藏星那一侧
/// 从 `label_text()` 取，那个函数没动。
fn blank_label(cart: &Cart, w: u32, h: u32) -> Vec<u8> {
    let [r, g, b] = label_colour(&label_text(cart));
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..w * h {
        rgba.extend_from_slice(&[r, g, b, 255]);
    }
    rgba
}

/// 卡面凹槽里那张「纸」：玩家在 `Labels/` 里给这张卡放了 png 就铺它，否则铺一块这个游戏
/// 自己的色。
///
/// ★★★ 2026-11-03（六六）：玩家反馈「Labels 改了很不方便」—— 把这条加回来。取图用
/// `art::cover` 而不是 `stretch`：贴纸是玩家随手给的封面或方图，比例跟凹槽对不上，
/// cover 保持比例、居中裁剪，比横向压扁好看；`stretch` 留给"图自己就是卡形"那条整卡路
/// （见 `pure_face`）。
///
/// 读不到、解不开、尺寸为零 —— 一律回到纯色。**一张坏图不该让整张卡变成空白**，
/// 这和 `sticker_face` 对尺寸不符就整块不画是同一条：宁可少一层，不留半层。
///
/// 透明像素由 `paste_label` 按 alpha 混合到壳色上，所以带透明边的贴纸也是贴在塑料上，
/// 不是贴在一块白板上。
fn label_paper(cart: &Cart, w: u32, h: u32) -> Vec<u8> {
    if let Some(path) = cart.label.as_deref() {
        if let Some(px) = art::cover(path, w, h) {
            return px;
        }
    }
    blank_label(cart, w, h)
}

fn rgb_to_hsv([r, g, b]: [u8; 3]) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    let h = if d == 0.0 {
        0.0
    } else if mx == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if mx == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    let s = if mx == 0.0 { 0.0 } else { d / mx };
    (h, s, mx)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a tiny RGBA png, the way a card carries one.
    fn write_png(path: &std::path::Path, rgba: &[u8], w: u32, h: u32) {
        let f = std::fs::File::create(path).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(rgba).unwrap();
    }

    fn cart_with_art(stem: &str, art: Option<std::path::PathBuf>) -> Cart {
        Cart {
            initial: 'P',
            stem: stem.into(),
            rom: format!("Games/{stem}.gba").into(),
            label: None,
            art,
            title: stem.to_uppercase(),
            code: String::new(),
            cgb: 0,
        }
    }

    /// The whole-cart picture is the face, whole: its own pixels at the machine's face size,
    /// with nothing composited over them. The magenta pixel is the case that proves it — the
    /// composite path would have read that colour as a label slot and cut it out, and the
    /// pure path must leave the picture's own colour exactly where it is.
    #[test]
    fn a_whole_cart_picture_is_the_face_untouched() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("POKEMON EMER.png");
        // Two by two: magenta over white on top, opaque black over transparent black below.
        let rgba: Vec<u8> = [
            0xff, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
            0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00,
        ]
        .to_vec();
        write_png(&path, &rgba, 2, 2);

        let cart = cart_with_art("POKEMON EMER", Some(path));
        let size = size_for(cart.system());
        let face = cart_face(&cart);
        assert_eq!((face.w, face.h), (size.face_w(), size.face_h()));
        let rgb = |x: u32, y: u32| {
            let i = ((y * face.w + x) * 4) as usize;
            [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]]
        };
        assert_eq!(rgb(0, 0), [0xff, 0x00, 0xff], "the picture's own magenta");
        assert_eq!(rgb(face.w - 1, 0), [0xff, 0xff, 0xff], "and its white");
        // The bottom row is transparent in the picture, and the silhouette leaves it so:
        // the alpha is the cart's shape, the same rule the machine templates keep.
        let i = (((face.h - 1) * face.w + face.w / 2) * 4) as usize;
        assert_eq!(face.rgba[i + 3], 0, "transparency is the shape");
    }

    /// Without the file — or with one that will not decode — the composite path answers, and
    /// the cart is still a cart drawn the way it was before the pure path existed.
    #[test]
    fn a_cart_without_the_picture_is_the_composite() {
        let d = tempfile::tempdir().unwrap();
        let cart = cart_with_art("POKEMON EMER", Some(d.path().join("nope.png")));
        let face = cart_face(&cart);
        let i = ((face.h / 2 * face.w + face.w / 2) * 4) as usize;
        assert_eq!(face.rgba[i + 3], 255, "the built-in shell is opaque");
        assert_ne!(
            [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]],
            [0xff, 0x00, 0xff],
            "and it is not the absent picture's magenta"
        );
    }

    /// The switch breaks the one tie there is: a cart carrying **both** a whole picture and a
    /// sticker png. Whichever way it is set, a cart with only one of the two is dressed from
    /// the one it has — the switch never hides a source — and a cart with neither is the
    /// magenta-slot shell with the game name set in the slot.
    #[test]
    fn the_switch_only_breaks_a_tie_between_the_two_sources() {
        // The source is a process-wide fact and this module's tests run in parallel: every
        // other test here is a single-source cart the switch cannot move, but the ones that
        // *set* it take the lock so the two of them cannot interleave — the same shape
        // `slot::core`'s tests use for the environment.
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let d = tempfile::tempdir().unwrap();
        // A whole picture: magenta on the left, white on the right, opaque.
        let whole = d.path().join("whole.png");
        write_png(&whole, &[0xff, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff], 2, 1);
        // A sticker: cyan throughout, opaque.
        let sticker = d.path().join("sticker.png");
        write_png(&sticker, &[0x00, 0xff, 0xff, 0xff, 0x00, 0xff, 0xff, 0xff], 2, 1);

        let magenta = |face: &CartFace| {
            face.rgba
                .chunks_exact(4)
                .any(|p| p[0] == 0xff && p[1] == 0x00 && p[2] == 0xff)
        };
        let cyan = |face: &CartFace| {
            face.rgba
                .chunks_exact(4)
                .any(|p| p[0] == 0x00 && p[1] == 0xff && p[2] == 0xff)
        };
        let dressed = |art: Option<&std::path::Path>, label: Option<&std::path::Path>| {
            let cart = Cart {
                initial: 'P',
                stem: "POKEMON EMER".into(),
                rom: "Games/POKEMON EMER.gba".into(),
                label: label.map(std::path::Path::to_path_buf),
                art: art.map(std::path::Path::to_path_buf),
                title: "POKEMON EMER".into(),
                code: String::new(),
                cgb: 0,
            };
            cart_face(&cart)
        };

        // Both files: the switch decides, and only this case asks it.
        set_art_source(ArtSource::Whole);
        let face = dressed(Some(&whole), Some(&sticker));
        assert!(magenta(&face), "整体优先：整卡贴图该赢");
        assert!(!cyan(&face), "贴纸没有上");

        set_art_source(ArtSource::Sticker);
        let face = dressed(Some(&whole), Some(&sticker));
        assert!(cyan(&face), "局部优先：贴纸该赢");
        assert!(!magenta(&face), "整卡贴图没有上");

        // Only one of the two: the one there is used, whichever way the switch sits.
        for source in [ArtSource::Whole, ArtSource::Sticker] {
            set_art_source(source);
            let face = dressed(Some(&whole), None);
            assert!(magenta(&face), "只有整卡贴图时它就该上（{source:?}）");
            let face = dressed(None, Some(&sticker));
            assert!(cyan(&face), "只有贴纸时它就该上（{source:?}）");
        }

        set_art_source(slot_store::ART_SOURCE_DEFAULT);
    }

    /// ★ 2026-10-24（六六）：「GBC 有两种卡带，共通和专用，得画两份」。
    ///
    /// 壳型原来是**按机种**定的（`Shape::from`），于是 GBC 货架上每张卡都穿专用壳。
    /// 现在 GBC 要再看卡带头 `0x143`：
    ///
    /// * `0xC0` 专用 ⇒ 专用壳（`Shape::Gbc`，无锁扣缺口）
    /// * `0x80` 双模 ⇒ 共通壳（`Shape::Gb`，带缺口，插初代 GB 也能跑）
    /// * `0x00` 初代 GB 游戏 ⇒ 同样是共通壳
    ///
    /// 钉住的是**判据方向**：改反了（专用穿共通壳）这台机器上看不出来，要拿实物对着数缺口。
    #[test]
    fn a_game_boy_color_cart_wears_the_shell_its_header_asks_for() {
        let mk = |system: slot_store::System, cgb: u8| slot_store::Cart {
            stem: "TEST".into(),
            rom: format!("Games/{system:?}/TEST.gb").into(),
            label: None,
            art: None,
            title: "TEST".into(),
            code: "TEST".into(),
            cgb,
            initial: 'T',
        };

        use slot_store::System;
        // 专用壳：无缺口
        assert_eq!(
            shape_of(&mk(System::Gbc, slot_store::CGB_ONLY)),
            Shape::Gbc,
            "0xC0 是 GBC 专用，画的是无缺口的专用壳"
        );
        // 双模：共通壳
        assert_eq!(
            shape_of(&mk(System::Gbc, slot_store::CGB_DUAL)),
            Shape::Gb,
            "0x80 是双模，穿带缺口的共通壳"
        );
        // 初代 GB 游戏塞进 GBC 文件夹（cgb=0 也可能是"没读到"）：同样是共通壳
        assert_eq!(shape_of(&mk(System::Gbc, 0)), Shape::Gb);
        // 其它机种不受这个字节影响
        assert_eq!(shape_of(&mk(System::Gb, slot_store::CGB_ONLY)), Shape::Gb);
        assert_eq!(shape_of(&mk(System::Gba, 0)), Shape::Gba);
    }

    /// ★ 2026-10-26（六六）：缺省图用于没有贴图的游戏。一张卡没有 `Cartridge/` 整图、而它
    /// 的目录有缺省图时，`cart_face` 走缺省图（洋红区填这个游戏自己的颜色），而不是内建壳。
    ///
    /// ⚠️ 这个过程全局只 install 一次（`DEFAULTS` 是 `OnceLock`）：它改的是所有"无整图且
    /// 目录命中"的 cart 的画法，而这类 cart 只有本测试的目录名（GBA）才命中 —— 别处测试的
    /// rom 都在 `Games/<stem>.gba`，`dir_name` 不是机种目录，落不到缺省图上，不受影响。
    #[test]
    fn a_cart_without_a_picture_uses_its_machines_default() {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // 一张 2×2 青绿色不透明缺省图，装进 GBA 槽。没有洋红区（label=None），所以
        // `tinted` 原样返回 —— 这正是要的：验证接线，而不是填色（填色另有测试）。
        crate::cart_art::install_defaults(crate::cart_art::DefaultArtSet {
            gba: Some(crate::cart_art::DefaultArt {
                rgba: vec![0x12, 0x34, 0x56, 0xff].repeat(16),
                w: 2,
                h: 2,
                label: None,
            }),
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
        });

        let cart = Cart {
            stem: "TEST".into(),
            rom: "Games/GBA/TEST.gba".into(),
            label: None,
            art: None,
            title: "TEST".into(),
            code: String::new(),
            cgb: 0,
            initial: 'T',
        };
        let face = cart_face(&cart);
        let i = ((face.h / 2 * face.w + face.w / 2) * 4) as usize;
        assert_eq!(
            [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]],
            [0x12, 0x34, 0x56],
            "无整图且目录有缺省图 ⇒ 画缺省图，而不是内建壳"
        );
    }

    /// ★ 2026-10-26（六六）：「原本卡带贴纸部分仍然留有大面积的洋红区...上货架的时候给
    /// 随机颜色就好」。`default_face_with` 把洋红区换成这个游戏自己的颜色（`label_colour`，
    /// 稳定 hash），而不是把洋红留在屏上。
    #[test]
    fn a_default_face_tints_the_mark_with_the_carts_own_colour() {
        // 2×2 全洋红缺省图，洋红区 = 全图。
        let art = crate::cart_art::DefaultArt {
            rgba: vec![0xff, 0x00, 0xff, 0xff].repeat(16),
            w: 2,
            h: 2,
            label: Some(crate::cart_art::Label {
                x0: 0,
                y0: 0,
                x1: 1,
                y1: 1,
                mask: vec![255; 4],
            }),
        };
        let cart = Cart {
            stem: "TEST".into(),
            rom: "Games/GBA/TEST.gba".into(),
            label: None,
            art: None,
            title: "TEST".into(),
            code: String::new(),
            cgb: 0,
            initial: 'T',
        };
        let shape = shape_of(&cart);
        let face = default_face_with(&art, &cart, shape).expect("a default has to face");
        let i = ((face.h / 2 * face.w + face.w / 2) * 4) as usize;
        let own = label_colour(&label_text(&cart));
        assert_eq!(
            [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]],
            own,
            "洋红区被填成这个游戏自己的颜色"
        );
        assert_ne!(
            [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]],
            [0xff, 0x00, 0xff],
            "不再是大洋红"
        );
    }

    /// `default_face_with` 对一张没有洋红区的缺省图原样画（label=None ⇒ 不填色）——
    /// 前端名字、壳色都在图里，缺省图自己是完整外观。
    #[test]
    fn a_default_without_a_mark_is_drawn_untouched() {
        let art = crate::cart_art::DefaultArt {
            rgba: vec![0xaa, 0xbb, 0xcc, 0xff].repeat(16),
            w: 2,
            h: 2,
            label: None,
        };
        let cart = Cart {
            stem: "TEST".into(),
            rom: "Games/GBA/TEST.gba".into(),
            label: None,
            art: None,
            title: "TEST".into(),
            code: String::new(),
            cgb: 0,
            initial: 'T',
        };
        let shape = shape_of(&cart);
        let face = default_face_with(&art, &cart, shape).expect("a default has to face");
        let i = ((face.h / 2 * face.w + face.w / 2) * 4) as usize;
        assert_eq!(
            [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]],
            [0xaa, 0xbb, 0xcc]
        );
    }

}
