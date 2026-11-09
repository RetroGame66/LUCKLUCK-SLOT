use std::path::{Path, PathBuf};

use slot_store::System;

use crate::audio::Profile;

/// The top level folders of a content root. A card that has never held slot. has none
/// of them, and every write path below assumes its own is already there.
///
/// ★ 2026-10-21 六六：阅读器下线，`Books` 不再建（实现整棵归档在工作区
/// `_archive/像素阅读器-归档-2026-10-21/`），长度 9→8。
/// `ensure` 的循环自动带上，无需别处改。
///
/// ★ 2026-10-23 六六：`Cartridge` 追加在末尾（长度 8→9）—— 整卡贴图的新家，
/// 游戏卡带贴图按机种放这里，和 `System/Carts/` 里那几张兜底外壳模板分开。
/// 追加不插序：已经发下去的卡片上目录位置就是用户的肌肉记忆。
///
/// ★★ 2026-10-28 六六：「**Labels 体系我们已经取消了，怎么还有痕迹**」—— `Labels` 从
/// 这一版起不再创建（9→8）。
///
/// ★★★ 2026-11-03 六六：**加回来**（8→9）。玩家反馈「Labels 改了很不方便」——
/// 取消之后，想给自己的游戏配张封面只剩 `Cartridge/<机种>/<stem>.png` 整卡图这条路
/// （要做整卡轮廓、对洋红槽），比"丢一张普通 png 进 Labels"门槛高得多；而且那半年里
/// 目录本身也不建了，图丢进去连个落点都没有。**放进第 3 位，就是它原来的位置**：
/// 已有各项之间的先后一位没动（BIOS、Games 仍在它前面，Saves 之后一字未改），
/// 只是把玩家手上那张卡原本就有的目录放回原处。
///
/// 卡的默认外观不变：`Labels` 里有图才贴，没图仍是纯色纸（`cart::label_paper`）。
pub const DIRS: [&str; 9] = [
    "BIOS",
    "Games",
    "Labels",
    "Saves",
    "States",
    "System",
    "Wallpapers",
    // ★ 2026-10-20 六六：「卡添加 Bootlogos 目录存放开机图」。
    // 追加在末尾（和 Books 一样）：已经发下去的卡片上目录位置就是用户的肌肉记忆，
    // 为"看起来更整齐"重排一次，代价是所有人手上的卡都变了。
    "Bootlogos",
    // ★ 2026-10-23 六六：整卡贴图。
    "Cartridge",
];

/// Where the **player's** two typefaces go, by name. **Not a folder scan** — the panel asks for
/// two specific faces, and a folder that happened to hold a third font must not change what the
/// deck looks like. See `slot_ui::text::PANEL_FONT_FILES`.
///
/// ★ 2026-10-06 六六：「播放器字体别封进二进制，放 `../Music/Fonts` 里到时随包分发」
/// ——其后澄清：字体就住在**内存卡根目录的 `Music`**，跟原本游戏的 Games 和 Labels、System 同级（和 `Music/Covers`、`Music/Lyrics` 同属播放器那一摊）。
/// 所以这张汉字面（12.6MB）不再编进二进制，设备上只留一份，由发布包放进卡里：
/// **卡根** `<card>/Music/Fonts/`（源文件在 `slot-main/deploy/Music/Fonts/`，deploy 树就是卡根镜像）。
pub const PANEL_FONT_DIR: &str = "Music/Fonts";

/// Where a card's own typefaces go. Nested under `System` rather than beside the seven:
/// the folders out there are things the user fills from the outside — ROMs, art, saves — and
/// this one is a system setting, like `theme.txt` and `selected_core.ini` next to it.
const FONT_DIR: &str = "System/fonts";

// ★ 2026-10-25：三份逐字相同的机种列表（`Games/`、`Saves/`、`Cartridge/`）提成一个常量。
//   它们在加 MD/PS1 之前就一直是三份手抄 —— 一次加机种要改三处，漏一处就是一整个机种
//   在卡上没有自己的目录（而 `Games/MD/` 恰恰是 `.bin` 唯一分得清归属的地方）。
pub type Machines = [System; 10];

/// 每个机种一份，**顺序只追加**（`dir_name()` 才是落盘的名字，这里的顺序只是遍历序）。
pub const MACHINES: Machines = [
    System::Gba,
    System::Gb,
    System::Gbc,
    System::Sfc,
    System::Fc,
    System::Neogeo,
    System::Arcade,
    System::Md,
    System::Ps1,
    System::Pce,
];

/// 机种选择面板里那一格的标识贴图，落在 `System/Carts/` 下（与 `type_gb.png` 那种
/// 机种标识同一个地方 —— 它们本来就是同一类东西：这台机器长什么样）。
///
/// ★ 2026-10-26（六六）：文件名**照用户给的那十张原样**，不另起一套命名。素材缺一张就是
/// 那一格没有标识 —— 格子还在、还占位、还能被选中（只要它有卡），只是画成一块空的圆角板。
/// 缺素材不该让一个机种从这块屏上消失，那是把"没有图"说成了"没有这台机器"。
pub fn sys_badge_file(system: System) -> &'static str {
    match system {
        System::Gba => "SLOT_SYS_GBA.png",
        System::Gb => "SLOT_SYS_GB.png",
        System::Gbc => "SLOT_SYS_GBC.png",
        System::Sfc => "SLOT_SYS_SFC.png",
        System::Fc => "SLOT_SYS_FC.png",
        System::Neogeo => "SLOT_SYS_NEOGEO.png",
        System::Arcade => "SLOT_SYS_ARCADE.png",
        System::Md => "SLOT_SYS_MD.png",
        System::Ps1 => "SLOT_SYS_PS1.png",
        System::Pce => "SLOT_SYS_PCE.png",
    }
}

/// Best effort: an unmounted or read only card is an empty shelf, not a boot failure.
pub fn ensure(root: &Path) {
    for sub in DIRS {
        let _ = std::fs::create_dir_all(root.join(sub));
    }
    // The per-machine folders inside `Games/`, made for every machine rather than left
    // to the user to spell right. Since 2026-10-23 the folder is what names the machine
    // (see `slot_store::System::from_dir`), so a card with no folders in it still works
    // every rom filed directly in `Games/` is read off its extension — but a card that
    // has them has somewhere obvious to put things, and somewhere that cannot be got
    // wrong: `.bin` is MD's, PCE's and PS1's, and only the folder tells them apart.
    for system in MACHINES {
        let _ = std::fs::create_dir_all(root.join("Games").join(system.dir_name()));
    }
        let _ = std::fs::create_dir_all(root.join(FONT_DIR));
        // 播放器那两张面。**建目录是包发下来之后的第一手保险** —— 卡上没这个目录时，
        // 面板会退回界面字体（能看，但不是它该有的样子），而建一个空目录至少让"该往哪放"
        // 摆在用户眼前。
        let _ = std::fs::create_dir_all(root.join(PANEL_FONT_DIR));
        // The screen-overlay folder, flat: the user drops `gb.png`, `gb-01.png`, `gbc.png`, …
        // straight into it. See `overlay_files` for the naming. Creating it is all upside — a
        // card with no art in it simply shows the built-in look.
        let _ = std::fs::create_dir_all(root.join("Overlay"));
        // The per-machine save folders are made here rather than left to the core: a core handed
        // a save directory that does not exist writes nothing and says nothing, and the first
        // save of the first GB game is exactly when nobody is looking.
        for system in MACHINES {
            let _ = std::fs::create_dir_all(root.join("Saves").join(system.dir_name()));
        }
        // ★ 2026-10-23：整卡贴图也按机种分目录，和 `Games/` 一个规矩 —— 往对应机种的
        //   目录里扔即可；直接平铺在 `Cartridge/` 根下的也认（未排序的卡）。
        for system in MACHINES {
            let _ = std::fs::create_dir_all(root.join("Cartridge").join(system.dir_name()));
        }
    }

/// The player's two faces, read off the card: **(Chinese, Latin)**, each `None` when the file is
/// missing or will not read.
///
/// **By exact name**, unlike [`font_file`]'s "first font in the folder": the panel is asking for
/// two specific faces — a 16px Chinese face and an 8px Latin one — and picking up whatever else
/// happened to be in the folder would silently change the deck's look. A missing file is not an
/// error: the panel falls back to the interface face and still draws.
pub fn panel_fonts(root: &Path) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let dir = root.join(PANEL_FONT_DIR);
    let read = |name: &str| std::fs::read(dir.join(name)).ok();
    let names = slot_ui::text::PANEL_FONT_FILES;
    (read(names[0]), read(names[1]))
}

/// The card's own typeface, if it carries one. First by name out of `System/fonts`; then the
/// single-file spelling, `System/font.ttf`, for a card that would rather not keep a folder for
/// one file. Absent means the embedded face, which is the whole of the default behaviour — a
/// card that says nothing about type is set in the font slot ships with.
///
/// Sorted first, so which file wins does not depend on the order a directory hands its entries
/// back: one card must pick one face, on every boot.
pub fn font_file(root: &Path) -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join(FONT_DIR))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| !slot_store::is_hidden(p))
        .filter(|p| is_font(p))
        .collect();
    files.sort();
    if let Some(first) = files.into_iter().next() {
        return Some(first);
    }
    let single = root.join("System/font.ttf");
    single.is_file().then_some(single)
}

/// The panel mask, if the card carries one. Nine RGB triples, one row per line, three
/// values per triple, 0-255: the table the game pass multiplies the picture by. `#` starts a
/// comment and blank lines are ignored, so the file documents itself.
///
/// Absent, unreadable, or the wrong shape means the built-in table, which is the whole of the
/// default behaviour — a card that says nothing about the panel is shown the panel slot ships
/// with. A malformed file is not an error worth reporting on a device with no console.
pub fn panel_mask(root: &Path) -> Option<[[[u8; 3]; 3]; 3]> {
    let text = std::fs::read_to_string(root.join("System/mask.txt")).ok()?;
    let mut rows: Vec<[[u8; 3]; 3]> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        let values: Vec<u8> = line
            .split_whitespace()
            .filter_map(|v| v.parse::<u8>().ok())
            .collect();
        if values.is_empty() {
            continue;
        }
        if values.len() != 9 {
            return None;
        }
        let mut row = [[0u8; 3]; 3];
        for (cell, chunk) in row.iter_mut().zip(values.chunks_exact(3)) {
            cell.copy_from_slice(chunk);
        }
        rows.push(row);
    }
    match rows.len() {
        3 => Some([rows[0], rows[1], rows[2]]),
        _ => None,
    }
}

/// One screen overlay on the card, and whether it asks for the reflection layer.
pub struct OverlayFile {
    pub path: PathBuf,
    /// The file name carried the reflection marker (a trailing `x` on the stem), so the game's
    /// mirrored blur is drawn wherever this overlay's own alpha leaves a hole. See
    /// `overlay_files`.
    pub reflect: bool,
}

/// Every screen-overlay PNG for one system, in rotation order.
///
/// Files live flat in `Overlay/`, named `<sys>.png` (the base) or `<sys>-NN.png` (a numbered
/// variant), compared without case: for plain Game Boy, `gb.png`, `gb-01.png`, `gb-02.png`, …;
/// for Game Boy Color, `gbc.png`, `gbc-01.png`, …. The base sorts first, then the numbered ones
/// by their number, so the rotation runs base, 01, 02, …. An absent or empty `Overlay/` is no
/// overlay at all — the built-in look stands — and a missing file is not an error worth a log
/// line on a device with no console.
///
/// A trailing `x` on the stem — `gbx.png`, `gb-02x.png` — marks a **reflective** overlay: its
/// transparent areas become the zones the screen reflection is drawn in (see
/// `Compositor::set_reflection`). No marker, no reflection; the art works exactly as before.
///
/// `which` is the stem a file must start with, `"gb"` or `"gbc"`. Matching on the following
/// `-` is what keeps `gbc-01.png` out of the plain GB set: it does not begin with `gb-`.
pub fn overlay_files(root: &Path, which: &str) -> Vec<OverlayFile> {
    let lower = which.to_ascii_lowercase();
    let numbered = format!("{lower}-");
    let Ok(dir) = std::fs::read_dir(root.join("Overlay")) else {
        return Vec::new();
    };
    let mut out: Vec<(i64, bool, String, PathBuf)> = Vec::new();
    for entry in dir.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let name = name.to_ascii_lowercase();
        let Some(stem) = name.strip_suffix(".png") else {
            continue;
        };
        // A trailing `x` is the reflection marker; what remains is the plain name.
        let (rest, reflect) = match stem.strip_suffix('x') {
            Some(r) => (r, true),
            None => (stem, false),
        };
        // Numbered from `00` upward: `gb-00.png` is as valid as `gb-01.png`, and both are as
        // valid as the bare `gb.png`. (Requiring a number greater than zero silently dropped
        // `gb-00x.png` — the name the user actually used — leaving the rotation with nothing.)
        let rank = if rest == lower {
            0
        } else {
            let after = rest
                .strip_prefix(&numbered)
                .or_else(|| rest.strip_prefix(&format!("{lower}_")));
            match after.and_then(|d| d.parse::<i64>().ok()) {
                Some(n) if n >= 0 => n,
                _ => continue,
            }
        };
        out.push((rank, reflect, name, path));
    }
    // A stable order whatever the filesystem hands back, so the rotation does not depend on
    // directory order. Ties — `gb-01.png` beside `gb-01x.png` — sort by name, deterministically.
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    out.into_iter()
        .map(|(_, reflect, _, path)| OverlayFile { path, reflect })
        .collect()
}

/// A named overlay: `Overlay/<sys>-<model>x.png`, the screen art drawn for one Game Boy model.
///
/// `overlay_files` cannot see these and that is on purpose — they are not entries in a rotation,
/// they belong to the model whose art they are. The naming is `<sys>-<word>x.png`, matched
/// case-insensitively the same way `overlay_files` lowercases, so a card written on Windows
/// and a card written on the device agree about it.
///
/// `None` when the card has no art for that model, which is the normal case for a card that
/// never drew any: the overlay set is then just whatever ordinary files are there.
pub fn model_overlay(root: &Path, which: &str, model: &str) -> Option<PathBuf> {
    let want = format!(
        "{}-{}x.png",
        which.to_ascii_lowercase(),
        model.to_ascii_lowercase()
    );
    let dir = std::fs::read_dir(root.join("Overlay")).ok()?;
    for entry in dir.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.to_ascii_lowercase() == want {
            return Some(path);
        }
    }
    None
}

/// The overlay ring a machine owns: its slot number, and the stem its files are named after.
///
/// `None` for a machine with no overlay set at all — the Advance — which is what tells the
/// caller to drop whatever the last cart left on screen.
///
/// The slot numbers are the order the machines were added and never change: a card written by
/// an older build still reads the Game Boy's and the Game Boy Color's picks where it left
/// them, and a machine that appears later is given the next number rather than one that
/// shifts the others. Same rule as the card's own folders — append, never insert.
///
/// The consoles are one bucket each, and that is the whole of the "American edition" question:
/// a Famicom and an NES are the same machine with different letters on the box, so a `.nes`
/// cartridge is a `Fc` and wears the same bezel, the same reflection and the same scanlines
/// as the Japanese one. The letters in the file name are the *machine's*, never the region's.
pub fn overlay_ring(system: System) -> Option<(usize, &'static str)> {
    match system {
        System::Gb => Some((0, "gb")),
        System::Gbc => Some((1, "gbc")),
        // ★ 2026-10-29（640 树）：GBA 在 640×480 面板上的画面矩形是 636×424（`GBA_DRAW_W`），
        //   不是满屏 —— 六六拍板钉在 **x=2,y=2 贴顶**（左右各 2px、上 2px 下 54px 露面板黑底，
        //   下缘留给插卡槽/UI 区），与 `Overlay/gba-00.png` 开孔逐像素对齐。
        //   **不带 `x` 尾缀**：636×424 窗口只剩 2px 边距，没有空间开反射层。
        System::Gba => Some((9, "gba")),
        // ★ 2026-10-25：共享 bezel 只给**拉伸到 640×480 的那两台**——
        //   · `Arcade` = 街机除 NeoGeo（六六之前定的街机拉伸 640×480）；
        //   · `Ps1`（源帧 640×480）。
        // 这俩共用 `Overlay/ALL_4-3*.png`（`ALL_4-3_00x.png` 一张，带 `x` 尾缀=要反射）。
        // 其余能整倍缩放的机种（SFC/FC/NeoGeo/MD/PCE）保持各自已经做好的遮罩，不改 stem。
        // 遮罩必须 720×480 全屏、洞区 alpha < 0.5 —— 见 `MEMORY-遮罩环.md`。
        System::Sfc => Some((2, "sfc")),
        System::Fc => Some((3, "fc")),
        System::Neogeo => Some((4, "neogeo")),
        System::Arcade => Some((5, "all_4-3")),
        System::Md => Some((6, "md")),
        System::Ps1 => Some((7, "all_4-3")),
        // PCE 占第 8 槽（0-based）。OVERLAY_RINGS 已随之推到 9，否则这里 `8` 越界。
        System::Pce => Some((8, "pce")),
    }
}

/// How many overlay rings there are — one per machine that has one. The order is
/// `overlay_ring`'s, and it only grows.
/// ★ 2026-10-29（640 树）：GBA 占第 9 槽（0-based），故数组长 10。
pub const OVERLAY_RINGS: usize = 10;

/// Which overlay each system is showing, as indices into `overlay_files`. Kept in
/// `System/overlay.txt` as one integer per ring, in `overlay_ring`'s order. A missing or
/// unreadable file reads as zero for all of them, and the caller clamps any index past the
/// end of a system's list, so a card the user edits without rebooting cannot land out of
/// range. A file written by an older build holds fewer numbers and the rest read as zero.
pub fn overlay_indices(root: &Path) -> [usize; OVERLAY_RINGS] {
    let vals: Vec<usize> = std::fs::read_to_string(root.join("System/overlay.txt"))
        .ok()
        .map(|t| {
            t.split_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    let mut out = [0usize; OVERLAY_RINGS];
    for (slot, v) in out.iter_mut().zip(vals.iter()) {
        *slot = *v;
    }
    out
}

pub fn write_overlay_indices(root: &Path, indices: [usize; OVERLAY_RINGS]) {
    let _ = std::fs::write(
        root.join("System/overlay.txt"),
        indices
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" "),
    );
}

/// Which machine's shelf is up, from `System/shelf.txt`. Absent or misspelt reads as GBA — the
/// machine this device is a frontend for, and the natural default for a card that has not been
/// told otherwise.
pub fn shelf_system(root: &Path) -> System {
    std::fs::read_to_string(root.join("System/shelf.txt"))
        .ok()
        .and_then(|t| t.split_whitespace().next().map(str::to_string))
        .and_then(|w| match w.to_ascii_lowercase().as_str() {
            "gb" => Some(System::Gb),
            "gbc" => Some(System::Gbc),
            "gba" => Some(System::Gba),
            "sfc" => Some(System::Sfc),
            "fc" => Some(System::Fc),
            "neogeo" => Some(System::Neogeo),
            "arcade" => Some(System::Arcade),
            _ => None,
        })
        .unwrap_or(System::Gba)
}

pub fn write_shelf_system(root: &Path, system: System) {
    let _ = std::fs::write(root.join("System/shelf.txt"), system.dir_name());
}

/// The colour-correction matrix, if the card carries one. Nine floats, three rows of three,
/// row-major (output row, input column), 0.0-2.0: the matrix the game pass multiplies the
/// picture by, in the spirit of a colour-saturation shader but applied as a single 3x3 multiply so
/// it costs nothing on this device. `#` starts a comment and blank lines are ignored.
///
/// Absent, unreadable, or the wrong shape means the built-in table (a gentle desaturation),
/// which is the whole of the default behaviour. A malformed file is not an error worth
/// reporting on a device with no console.
pub fn color_correction(root: &Path) -> Option<[[f32; 3]; 3]> {
    let text = std::fs::read_to_string(root.join("System/cc.txt")).ok()?;
    let mut rows: Vec<[f32; 3]> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        let values: Vec<f32> = line
            .split_whitespace()
            .filter_map(|v| v.parse::<f32>().ok())
            .collect();
        if values.is_empty() {
            continue;
        }
        if values.len() != 3 {
            return None;
        }
        rows.push([values[0], values[1], values[2]]);
    }
    match rows.len() {
        3 => Some([rows[0], rows[1], rows[2]]),
        _ => None,
    }
}

/// The display modes, read from `System/display.txt`. The first field is the Advance's aperture
/// preset; the rest is one colour-correction pick **per machine**, and those picks are persisted
/// as one `key value` line each so adding a console core later never reshuffles another machine's
/// remembered choice:
///
/// ```text
/// mask 2
/// gba 0
/// gb 0
/// gbc 0
/// sfc 0
/// fc 0
/// ```
///
/// The *parameters* are shared — every colour machine (Advance, Color, SFC, FC and the later
/// MD/PCE/PS1/FBNEO) walks the one colour-grade ring in `GRADES`, and a Game Boy walks its own
/// palette table instead (`gb_lut`) — but each machine holds its own index, so they are cycled
/// and stored independently. `mask` is `0..MASK_STATES` (OFF, four LCD3x strengths, two
/// scanlines). The Game Boy's pick is an index into the whole palette table, the others into
/// `GRADES`; both clamp in `DisplayFilter::new`, not here, because only the app knows how long
/// those lists are.
///
/// The aperture count is clamped here (it is a small fixed ring); the colour indices are not —
/// a cap written into the reader would silently retire a Game Boy palette the next time the card
/// booted. A legacy single-line file (`mask cc_gba cc_gb cc_gbc`, all integers) is still read for
/// a card written by an earlier build, mapped onto the per-machine slots by position.
/// How many aperture presets `display.txt`'s first number may name: OFF, four strengths of the
/// LCD3x table, the scanline at two.
///
/// **It lives here and not beside the ring it describes**, because this is the file that indexes
/// it: `read` clamps against this number and `DisplayFilter::MASK_STATES` — the count of arms in
/// `applied_mask`, the length `cycle_mask` walks — is the ring agreeing with the card. One number,
/// because the file, the ring and the cycle all have to agree about it.
///
/// It was a literal `4` in the clamp until 2026-09-30 — the count when the ring held five presets.
/// It was never raised when the scanline was added, so a card that saved the scanline (5 or 6) was
/// read back as LCD3x 100% on the next boot: the two newest presets were written, shown, persisted
/// and then silently replaced. `tests/display.rs` now walks the whole ring through the file.
pub const MASK_STATES: usize = 7;

/// The display modes a card asks for: the Advance's aperture preset, the console scanline state,
/// then one colour-correction pick **per machine** — `mask` and `scanline` on their own lines,
/// then `gba`/`gb`/`gbc`/`sfc`/`fc`/`neogeo`/`arcade`/`md`/`ps1`/`pce` each on a line with its
/// own index. The *parameters* are shared (one `GRADES` ring for every colour machine, one palette
/// table for the Game Boy); only the remembered pick is per machine.
///
/// ★ **The array grew from 7 to 9 on 2026-10-25** (MD and PS1), by appending — the first seven
/// entries keep the order and meaning they had, so a card written by any earlier build reads
/// back exactly as it left. Note the *slot numbers* here are `App::DisplayFilter::machine_slot`'s,
/// not `overlay_ring`'s: the two are different numbering schemes on purpose and neither may be
/// reordered.
///
/// ★ **2026-10-26 六六**：加 `scanline` 行，NEOGEO/ARCADE/MD/PS1/PCE 的扫描线档三态
/// （无/50%/100%）。FC/SFC 仍走遮罩环，不用这个字段。手改过的卡写 `scanline 999` 也夹回 0。
pub fn display_modes(root: &Path) -> (u8, [u16; 10], u8) {
    let text = match std::fs::read_to_string(root.join("System/display.txt")) {
        Ok(t) => t,
        Err(_) => return (2, [0; 10], 0),
    };
    let toks: Vec<&str> = text.split_whitespace().collect();
    // A legacy file is a single line of integers (`mask cc_gba cc_gb cc_gbc`), no machine names.
    // A current file carries `key value` lines whose keys are not numbers, so "every token parses
    // as a number" cleanly separates the two forms.
    let all_numeric = !toks.is_empty() && toks.iter().all(|t| t.parse::<u16>().is_ok());
    if all_numeric {
        let mask = toks
            .first()
            .and_then(|t| t.parse::<u16>().ok())
            .unwrap_or(2)
            .min(MASK_STATES as u16 - 1) as u8;
        let one = toks.get(1).and_then(|t| t.parse::<u16>().ok()).unwrap_or(0);
        let p = |i: usize| toks.get(i).and_then(|t| t.parse::<u16>().ok()).unwrap_or(0);
        // Positional legacy order is gba, gb, gbc; the two console cores had no slot then, and
        // neither did MD or PS1 (they fall in the zeroed tail). Scanline is always off on legacy
        // files — they predate the field.
        let cc = if toks.len() >= 4 {
            [p(1), p(2), p(3), 0, 0, 0, 0, 0, 0, 0]
        } else if toks.len() == 3 {
            [p(1), p(2), 0, 0, 0, 0, 0, 0, 0, 0]
        } else if toks.len() == 2 {
            // `mask cc`: one grade for every machine, as the older build meant it.
            [one, one, one, one, one, one, one, one, one, one]
        } else {
            [0; 10]
        };
        return (mask, cc, 0);
    }
    // Current keyed format: one `key value` line per machine, plus `mask` and `scanline`.
    let mut mask = 2u16;
    let mut cc = [0u16; 10];
    let mut scanline = 0u16;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let key = it.next();
        let val = it.next().and_then(|t| t.parse::<u16>().ok());
        match (key, val) {
            (Some("mask"), Some(n)) => mask = n,
            (Some("scanline"), Some(n)) => scanline = n,
            (Some("gba"), Some(n)) => cc[0] = n,
            (Some("gb"), Some(n)) => cc[1] = n,
            (Some("gbc"), Some(n)) => cc[2] = n,
            (Some("sfc"), Some(n)) => cc[3] = n,
            (Some("fc"), Some(n)) => cc[4] = n,
            (Some("neogeo"), Some(n)) => cc[5] = n,
            (Some("arcade"), Some(n)) => cc[6] = n,
            (Some("md"), Some(n)) => cc[7] = n,
            (Some("ps1"), Some(n)) => cc[8] = n,
            (Some("pce"), Some(n)) => cc[9] = n,
            _ => {}
        }
    }
    (mask.min(MASK_STATES as u16 - 1) as u8, cc, scanline.min(2) as u8)
}

/// Persist the display modes. Best effort: a read only card simply keeps the default.
///
/// The new lines go **at the end**, so a diff of a card's `display.txt` before and after this
/// build shows only the two added keys and nothing reordered.
pub fn write_display_modes(root: &Path, mask: u8, cc: [u16; 10], scanline: u8) {
    let _ = std::fs::write(
        root.join("System/display.txt"),
        format!(
            "mask {mask}\nscanline {scanline}\ngba {}\ngb {}\ngbc {}\nsfc {}\nfc {}\nneogeo {}\narcade {}\nmd {}\nps1 {}\npce {}",
            cc[0], cc[1], cc[2], cc[3], cc[4], cc[5], cc[6], cc[7], cc[8], cc[9]
        ),
    );
}

/// Where the Game Boy's palette list stood before the browser, and where those same looks are in
/// `palettes::GB_PALETTES` now.
///
/// The old list was five house looks followed by ten PixelShift picks: 原生灰, DMG 初代绿,
/// DMG 经典绿, GBP 暖白, GBL 青, then PS01/03/05/17/18/24/31/32/40/44. Three of those — the three
/// model palettes — are no longer entries at all: they are the colour half of the screen art they
/// were drawn with (`palettes::GB_MODELS`), reached by choosing the art rather than by picking a
/// colour. A card left holding one of those indices was showing the model it had landed on; the
/// number itself no longer names anything, so it falls back to 初代绿, the one house look that is
/// still a palette in its own right.
const GB_LEGACY: [u16; 15] = [0, 1, 1, 1, 1, 2, 4, 6, 18, 19, 25, 32, 33, 41, 45];

/// Written once `migrate_palette` has run, so it runs exactly once per card.
pub const PALETTE_MARKER: &str = "System/palette.v2";

/// Bring a card written before the palette browser onto the table it has now, so the Game Boy look
/// it was set to is the look it still shows.
///
/// Keyed on a marker file rather than on the value, because the two tables overlap across 0..=14
/// and no reading of a single number can say which table wrote it. Best effort throughout: a card
/// that cannot be written to simply keeps the default, exactly as it would have anyway.
pub fn migrate_palette(root: &Path) {
    let marker = root.join(PALETTE_MARKER);
    if marker.exists() {
        return;
    }
    let (mask, mut cc, scanline) = display_modes(root);
    if let Some(mapped) = GB_LEGACY.get(cc[1] as usize) {
        cc[1] = *mapped;
    }
    write_display_modes(root, mask, cc, scanline);
    let _ = std::fs::write(&marker, "palette.v2\n");
}

/// Where the colour machines' grade list stood before the rebuild, and where those same looks
/// are in the new three-entry list now.
///
/// The old list (0..=6) was FULLCOLOR, HALFCOLOR, NOCOLOR, DMG green, ice-blue, amber, pink. The
/// rebuild keeps just the three that mean something on a colour machine: 0 GRAYSCALE, 1 AGB-001
/// (the authentic original panel), 2 NDS (the soft near-sRGB middle ground). The single-colour
/// backlights (DMG green / ice-blue / amber / pink) have no equivalent — a colour grade has no
/// one signature tint — so they collapse to GRAYSCALE, the nearest the new list has. FULLCOLOR was
/// the shipped default and the faithful-restore intent, so it lands on AGB-001; HALFCOLOR was
/// already the softened look, so it lands on NDS.
/// ★ 2026-10-25：长度跟 `display_modes` 的 7→9 一起长了两格零。这**不是**占位凑数：
/// `migrate_display_modes` 只拿它去查 slot 0 和 2（见下），而 `.get()` 对越界返回 `None`，
/// 所以多这两格不会改变任何行为 —— 但留着 7 而 `cc` 是 9，会让"这个表覆盖哪些 slot"这件事
/// 靠读者自己数，下一个给 MD/PS1 做迁移的人会算错。
const CC_LEGACY: [u16; 10] = [1, 2, 0, 0, 0, 0, 0, 0, 0, 0];

/// Written once `migrate_display_modes` has run, so it runs exactly once per card.
pub const DISPLAY_MARKER: &str = "System/display.v2";

/// Bring a card written before the colour-grade rebuild onto the new three-entry list, so the
/// look it was set to is still the look it shows.
///
/// Keyed on a marker file (like `migrate_palette`) rather than on the value, because the old and
/// new lists overlap across 0..=2 and no single number can say which list wrote it. Only the
/// colour machines are remapped — the Game Boy's slot (1) is its own palette table and is left
/// untouched by this. Best effort throughout: a card that cannot be written to simply keeps the
/// default, exactly as it would have anyway.
pub fn migrate_display_modes(root: &Path) {
    let marker = root.join(DISPLAY_MARKER);
    if marker.exists() {
        return;
    }
    let (mask, mut cc, scanline) = display_modes(root);
    for slot in [0usize, 2usize] {
        // GB slot (1) is a palette index, not a grade; skip it.
        if let Some(mapped) = CC_LEGACY.get(cc[slot] as usize) {
            cc[slot] = *mapped;
        }
    }
    write_display_modes(root, mask, cc, scanline);
    let _ = std::fs::write(&marker, "display.v2\n");
}

/// The audio profile, read from `System/audio.txt`: `stable`, `balanced` or `strict`.
///
/// This is a latency dial, not a quality one. The GBA's own 32768 Hz already carries
/// everything the console can produce, so what is being traded here is how much audio sits
/// queued between the emulator and the speaker — which is exactly the offset a rhythm game
/// judges you against. `stable` is the shipped behaviour; a missing, blank or misspelled file
/// reads as `stable`, so a card edited on a PC cannot make the machine click by typo.
pub fn audio_profile(root: &Path) -> Profile {
    std::fs::read_to_string(root.join("System/audio.txt"))
        .ok()
        .and_then(|t| t.lines().find_map(Profile::parse))
        .unwrap_or_default()
}

/// Persist the chosen profile. Best effort, like the display modes: a read only card keeps the
/// profile it booted with.
pub fn write_audio_profile(root: &Path, profile: Profile) {
    let _ = std::fs::write(root.join("System/audio.txt"), profile.as_str());
}

/// One cheat code for a cart. `code` is the raw code fed to the core verbatim; `desc` is the
/// human label from the file's `cheatN_desc`, used only for the on-device list — no core ever
/// sees it. `enabled` is the file's own `cheatN_enable`: absent or anything but `true`/`1`
/// reads as off, which is the `.cht` format's default and what leaves a freshly seated cart
/// with nothing applied until the player switches a row on in the table.
pub struct CheatEntry {
    pub code: String,
    pub desc: String,
    pub enabled: bool,
}

/// Cheat codes for a cart, if the card carries any — **RetroArch's `.cht` format**, one file
/// per cart at `System/Cheats/<stem>.cht`. The same file RetroArch itself reads, so codes can
/// be lifted straight out of `libretro-database` (or from RetroArch's own cheat editor) and
/// dropped on the card with no conversion.
///
/// The format is libretro's config format: `key = value`, one per line, `#` starting a comment.
/// What is read here is the same three keys RetroArch's `cheat_manager_load` reads per entry:
///
/// ```text
/// cheats = 2
/// cheat0_desc = "Infinite Health"
/// cheat0_code = "3200E924+0096"
/// cheat0_enable = false
/// ```
///
/// Notes that come from reading RetroArch's parser rather than guessing:
/// - `cheats = N` (its count-of-entries key) is **not required here**. RetroArch fails the whole
///   file without it; this reader takes whatever `cheatN_*` groups it finds, so a hand-written
///   three-line file works. A file that also carries the key reads identically either way.
/// - A value in double quotes is taken whole, `#` and spaces included; a bare value runs to the
///   end of its line (minus a trailing `#` comment) — see [`unquote`] for why that deliberately
///   differs from RetroArch's "stop at the first space". `cheatN_enable` is compared exactly —
///   `True`, `YES` and `on` are all off, because RetroArch compares against `"true"` and `"1"`.
/// - Multi-part codes keep their `+` separators, e.g. `3200E924+0096+330034B8+0096`. mGBA's
///   `retro_cheat_set` turns every `+` and every whitespace run into a space and re-splits on
///   8-digit groups, so `+` and ` ` mean the same thing to it; the string is passed on as found.
/// - An entry with a code but no `desc` keeps an empty label, and the on-device row then draws
///   the code alone on one line — the layout `cheat_row_face` already had for desc-less codes.
///
/// Absent or unreadable means no cheats for this cart — best effort, a device with no console
/// is not the place to surface a missing file.
pub fn cheats(root: &Path, stem: &str) -> Vec<CheatEntry> {
    let path = root
        .join("System")
        .join("Cheats")
        .join(format!("{stem}.cht"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };

    // Keyed by the `N` in `cheatN_*`, so a file whose entries are out of order (or that skips a
    // number) still comes out in the order the author meant, not the order the lines fell in.
    let mut found: std::collections::BTreeMap<usize, CheatEntry> = std::collections::BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let Some(rest) = key.strip_prefix("cheat") else {
            continue;
        };
        let (digits, field) = match rest.find('_') {
            Some(i) => rest.split_at(i),
            None => continue,
        };
        // RetroArch reads the index with base 10 only, so `cheat0A_*` is not an entry there and
        // is not one here either.
        let Ok(index) = digits.parse::<usize>() else {
            continue;
        };
        let field = &field[1..];
        let value = unquote(value);
        let entry = found.entry(index).or_insert_with(|| CheatEntry {
            code: String::new(),
            desc: String::new(),
            enabled: false,
        });
        match field {
            "code" => entry.code = value.trim().to_string(),
            "desc" => entry.desc = value.trim().to_string(),
            // Exact match, as RetroArch does: only these two spellings switch a code on.
            "enable" => {
                let v = value.trim();
                entry.enabled = v == "true" || v == "1";
            }
            // `cheatN_cheat_type`, `cheatN_handler`, the rumble/repeat keys and anything newer
            // than this build: RetroArch ignores what it does not know, and so does this — a
            // code's own format decides how it is read, not a key beside it.
            _ => {}
        }
    }

    found
        .into_values()
        .filter(|e| !e.code.is_empty())
        .collect()
}

/// The value half of a `.cht` line. A quoted value is taken up to its closing quote (an
/// unterminated one runs to end of line, as in RetroArch); a bare value runs to end of line,
/// minus any trailing `#` comment. A `#` inside the quotes is part of the value, not a comment.
///
/// **Not** RetroArch's rule: its bare-value reader stops at the first space. Copying that would
/// cut `cheat0_code = 3200E924 0096` down to half a code and hand the fragment to mGBA, which
/// treats a space between groups exactly as it treats `+` — so a space-separated code that works
/// in RetroArch's own editor would silently do nothing here. Taking the rest of the line is the
/// permissive direction and cannot turn a good code bad; a `.cht` line is one key's value, so
/// there is nothing after a space worth protecting.
fn unquote(raw: &str) -> String {
    let s = raw.trim();
    if let Some(body) = s.strip_prefix('"') {
        return match body.find('"') {
            Some(end) => body[..end].to_string(),
            None => body.to_string(),
        };
    }
    match s.find('#') {
        Some(end) => s[..end].trim().to_string(),
        None => s.to_string(),
    }
}

/// TTC as well as TTF and OTF: `text::set_font` reads face 0, and several of the common
/// Chinese faces a desktop already has are collections rather than single faces.
fn is_font(p: &Path) -> bool {
    p.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case("ttf")
            || e.eq_ignore_ascii_case("otf")
            || e.eq_ignore_ascii_case("ttc")
    })
}

/// Bring a card written before states were namespaced up to the current layout. Best
/// effort on purpose: a read only or half mounted card is an empty shelf, not a boot
/// failure, exactly as `ensure` treats it.
///
/// A per-entry failure does not stop the sweep — the rest of the shelf still gets a chance —
/// but silently eating every one of them would leave a cart stuck pre-migration forever with
/// nothing on the card to say so. Logged here, once per boot, rather than inside
/// `migrate_states` itself, which only counts and has no read on where "once per boot" ends.
pub fn migrate(root: &Path) {
    match slot_store::migrate_states(root) {
        Ok(report) if report.failed > 0 => {
            eprintln!(
                "slot: migrate: {} of {} state director{} did not move",
                report.failed,
                report.moved + report.failed,
                if report.moved + report.failed == 1 {
                    "y"
                } else {
                    "ies"
                }
            );
        }
        _ => {}
    }
}

/// Reported to the core as the libretro system directory. `gba_bios.bin` present means the
/// real BIOS, absent means mGBA's HLE BIOS. Neither is an error.
pub fn bios_dir(root: &Path) -> PathBuf {
    root.join("BIOS")
}

/// The battery saves, under the machine's own folder. A GB `Tetris` and a GBA `Tetris` share a
/// stem, so on a flat card they also share one `.sav`; sorting by machine is what keeps each
/// game's bytes its own. `bios_dir` is the one folder that does **not** sort: a bios is the
/// machine, not the game, so it stays shared.
pub fn saves_dir(root: &Path, system: System) -> PathBuf {
    root.join("Saves").join(system.dir_name())
}
