use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use slot_retro::{LibretroCore, MockCore, RetroCore};
use slot_store::Core;

use crate::root;

/// Names the dylib outright, for a build that keeps it somewhere the search does not look.
///
/// This wins over `System/selected_core.ini` for which dylib loads — it stays a developer
/// escape hatch, not a second way to pick a core. It is not silent about that: it does not
/// touch which `Core` a cart resolves to (still the ini, still what the state directory
/// under `States/<core>/` is named after), only which file `open_core` opens. Pointing this
/// at gpSP for a cart the ini never mentions runs gpSP with its states filed under
/// `States/mgba/` — correct for a developer who set the override on purpose, a trap for
/// anyone who forgot it was set.
const CORE_ENV: &str = "SLOT_CORE";

/// Which dylib backs a core. The device keeps both in `System/`, so this is a filename
/// rather than a search: whichever the cart asked for is either there or it is not.
pub fn dylib_name(core: Core) -> String {
    format!(
        "{}_libretro.{}",
        core.as_str(),
        std::env::consts::DLL_EXTENSION
    )
}

/// Most specific first: the environment, then the content root's own `System/` — where the
/// device actually keeps a core, and, on the device, also where the binary itself lives, so
/// the next candidate coincides with this one there. Off the device — a host build, or a
/// test with its own tmp root — the binary's directory and `root` are different places, and
/// only searching the root's own `System/` gives a cart's own content root somewhere a test
/// can plant a dylib under. The `vendor` directory `scripts/fetch-core.sh` writes into is
/// last: a host development convenience, not anywhere a shipped device looks.
fn candidates(root: &Path, core: Core) -> Vec<PathBuf> {
    if let Some(named) = std::env::var_os(CORE_ENV) {
        return vec![PathBuf::from(named)];
    }
    // Spelled from the platform's own convention so the same search finds the device's `.so`.
    let name = dylib_name(core);
    let mut paths = vec![root.join("System").join(&name)];
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        paths.push(dir.join(&name));
        paths.push(dir.join("vendor").join(&name));
    }
    paths.push(Path::new("vendor").join(&name));
    paths
}

/// The one place a cart's `Core` becomes a dylib path. Callers that already know which core
/// they want — `session.rs` resolves it once per insert — pass it straight through, which is
/// what keeps this call from disagreeing with the caller's own choice. It says nothing about
/// what the caller does with that `Core` afterward: `App` is the one that has to keep using
/// the same value for every later read and write, and it does that by storing it rather than
/// asking again.
pub fn open_core(
    root: &Path,
    core: Core,
    system: slot_store::System,
    rom: &Path,
) -> Box<dyn RetroCore> {
    open_core_for_rom(root, core, system, Some(rom), &candidates(root, core))
}

/// The named core if one of these opens, the mock if none of them do. A missing core is not
/// a failure to boot: the shelf, the slot and every gesture are reachable either way.
///
/// The core is told the content root's own folders, never the dylib's: on the device the
/// core lives in `System/` and the user's BIOS does not.
///
/// `core` is redundant with `paths` in production — `open_core` derived both from the same
/// `Core` — but this function stays the seam that takes `paths` explicitly, because tests
/// plant a dylib somewhere `candidates` would not otherwise look. `apply_core_options` needs
/// `core` too, and only `open_core_for` ever holds a concrete `slot_retro::LibretroCore` to
/// call it on: everything above here deals in `Box<dyn RetroCore>`, which has no `set_option`.
/// That is also why the call sits here rather than at a caller — after `open_with` succeeds,
/// before the `Box<dyn RetroCore>` is handed back and `load` becomes reachable at all.
pub fn open_core_for(
    root: &Path,
    core: Core,
    system: slot_store::System,
    paths: &[PathBuf],
) -> Box<dyn RetroCore> {
    open_core_for_rom(root, core, system, None, paths)
}

/// The same open, but told which rom is going in — which is all a PS1 cart's **own** BIOS needs.
///
/// ★★ **Why the rom has to be here at all (2026-10-25).** A Hànhuà/romhack on PlayStation tends
/// to need one particular BIOS, and the only convention a card can express is "a `.bios` beside
/// the game with the same stem" (`Ridge.cue` ↔ `Ridge.bios`). `pcsx_rearmed` cannot be pointed at
/// a filename: `pcsx_rearmed_bios` takes only `auto`/`HLE` (checked in its
/// `libretro_core_options.h`), and `loadPSXBios()` — called from `retro_init`, *not* from
/// `retro_load_game` — walks the `system` directory it was handed: it tries
/// `scph5500/5501/5502/psxonpsp660/scph101/7001/1001.bin` by name, then `find_any_bios()` over
/// everything else. So "use this one" can only be spelled as **"make that directory contain
/// nothing else"**. That is what this does for a PS1 cart that has one: a scratch directory
/// holding only the cart's own file, handed over as `system`.
///
/// What the file has to be for the core to take it — none of it is checked here, because the
/// core's own rules are the honest ones and a second copy would drift:
///
/// * **exactly 512 KiB** (`try_use_bios` with `preferred_only`, which is every call site),
/// * `data[1..5] == "\0\x08\x3c\x3f"` **or** `data[0x12c..] == "PS compatible"`,
/// * not named `*unirom*`,
/// * and one BIOS per region only — the first hit wins, so a second file would be ignored
///   anyway.
///
/// ⚠️ **The directory has to outlive the session**, not just this call: `Config.BiosDir` is
/// stored and `psxMemReset()` re-opens `BiosDir/Bios[region]` on every reset, so a directory
/// that vanished with a `TempDir` would turn the second reset into a silent HLE fallback. This
/// uses a fixed scratch path under the system temp dir (tmpfs on the device) and **clears it
/// first**, so what `find_any_bios` sees is always exactly one file.
///
/// Falls back to the card's own `BIOS/` for everything else: no override file, no PS1, or a
/// scratch directory the machine refuses to write are all "exactly the old behaviour".
pub fn open_core_for_rom(
    root: &Path,
    core: Core,
    system: slot_store::System,
    rom: Option<&Path>,
    paths: &[PathBuf],
) -> Box<dyn RetroCore> {
    let bios = match (core, system, rom) {
        (Core::Pcsx, slot_store::System::Ps1, Some(rom)) => match ps1_bios_dir(root, rom) {
            Some(dir) => dir,
            None => {
                // No cart-specific hàn huà BIOS: hand the core the shared PS1 BIOS folder
                // (`BIOS/PSBIOS/`), where the default `scph*.bin` and any per-game `.bios` live.
                let psbios = root.join("BIOS/PSBIOS");
                if psbios.is_dir() {
                    psbios
                } else {
                    root::bios_dir(root)
                }
            }
        },
        _ => root::bios_dir(root),
    };
    let saves = root::saves_dir(root, system);
    // mGBA reads `mgba_sgb_borders` and `mgba_gb_colors` during `retro_init` — both are
    // marked "requires restart", so they have to be present in the host before `open_with`
    // runs the core's init, and `apply_core_options` (which runs after `open_with`) would be
    // too late.
    //
    //  - `mgba_sgb_borders = OFF`: SLOT draws its own Game Boy bezel for every GB/GBC cart,
    //    so mGBA's Super Game Boy border would only double up; turn it off.
    //  - `mgba_gb_colors = Grayscale`: the "Default Game Boy Palette" — the palette mGBA uses
    //    for plain DMG Game Boy carts (anything not GBC/SGB-compatible). Forcing it to
    //    Grayscale makes every monochrome GB game render as true black & white, so SLOT's
    //    colour-correction filters (GRAYSCALE / AGB-001 / NDS for the colour machines)
    //    ride on a clean grayscale base instead of mGBA's colourised one. GBC and GBA carts
    //    ignore it — GBC uses its own ROM palettes, GBA is not GB — so setting it for every
    //    mGBA cart is harmless.
    //
    // gpSP's one option (`gpsp_serial`) is read at `retro_load_game`, so it stays in
    // `apply_core_options`.
    let initial_options = initial_core_options(core);
    for path in paths {
        if !path.exists() {
            continue;
        }
        // FBNeo is the one core that opens a zip itself: a zip is its native romset format, and
        // handing it one file out of a romset would leave it with a fraction of a game. Every
        // other core gets its zipped roms unpacked by the frontend instead. See `slot_retro::rom`.
        let reads_zip_natively = matches!(core, Core::Fbneo);
        match LibretroCore::open_with(
            path,
            &bios,
            &saves,
            &initial_options,
            reads_zip_natively,
        ) {
            Ok(mut opened) => {
                apply_core_options(&mut opened, core);
                eprintln!("slot: core {}", path.display());
                return Box::new(opened);
            }
            Err(e) => eprintln!("slot: {}: {e}", path.display()),
        }
    }
    // Name the core and every path that was tried. The mock renders a rainbow test pattern
    // and a sine tone, which on screen reads as "this core is broken" rather than "this core
    // is missing" — the one time that happened it cost an afternoon, so the log says which
    // file was wanted and where it was looked for.
    eprintln!(
        "slot: no {} core found, running the mock test pattern instead. Looked in: {}",
        core.as_str(),
        paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    Box::new(MockCore::new())
}

/// The scratch directory that holds a PS1 cart's own `.bios` and nothing else, or `None`
/// when the card has no such file. See `open_core_for_rom` for why this exists at all.
///
/// **Copy rather than point at the game's folder**: `find_any_bios` sweeps *every* name in the
/// directory, so handing the core `Games/PS1/` would let any stray 512 KiB file in there — a
/// `.bin` of a track, say — be read as a BIOS. One file in one directory is the only spelling
/// that cannot mean two things.
fn ps1_bios_dir(root: &Path, rom: &Path) -> Option<PathBuf> {
    let stem = rom.file_stem()?.to_str()?;
    // ★ 2026-10-25 之后：汉化 / 改版 BIOS 集中放在 `BIOS/PSBIOS/<stem>.bios`，
    // 不再散落在每张游戏旁边（见 `MEMORY-遮罩环.md` 同款思路）。默认 BIOS 也在那个目录里，
    // 见 `open_core_for_rom` 的 fallback。
    let src = root.join("BIOS/PSBIOS").join(format!("{stem}.bios"));
    if !src.is_file() {
        return None;
    }
    // `Config.Bios[region]` is 64 bytes and `prepare_bios` **discards silently** anything longer
    // ("BIOS '%s' name is too long"), which would drop the whole override with one log line. A
    // name that cannot fit is better refused here, where the message says why.
    let name = src.file_name()?.to_str()?;
    if name.len() >= 64 {
        eprintln!("slot: ps1 bios: {name} is too long for the core (63 bytes max), ignored");
        return None;
    }
    let bytes = std::fs::read(&src).ok()?;
    // The core's own acceptance rules, restated **here only to explain the refusal**. Without
    // this a wrong file is silently skipped by `try_use_bios` and the game quietly falls back to
    // HLE — which looks exactly like the override never being wired, and the device has no
    // console to say otherwise. The core is still the authority: anything that passes here goes
    // on to be checked by it again.
    if bytes.len() != 512 * 1024 {
        eprintln!(
            "slot: ps1 bios: {} is {} bytes, pcsx needs exactly 524288 — ignored",
            name,
            bytes.len()
        );
        return None;
    }
    let valid = bytes.get(1..5) == Some(&[0x00, 0x08, 0x3c, 0x3f][..])
        || bytes.get(0x12c..0x12c + 13) == Some(&b"PS compatible"[..]);
    if !valid {
        eprintln!("slot: ps1 bios: {name} does not look like a PS1 BIOS — ignored");
        return None;
    }
    let dir = std::env::temp_dir().join("slot-ps1-bios");
    // Cleared first so `find_any_bios` can only ever find this one file — a leftover BIOS from
    // the last cart would be a second candidate, and the core keeps the first it accepts.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).ok()?;
    let dst = dir.join(name);
    if let Err(e) = std::fs::copy(&src, &dst) {
        eprintln!("slot: ps1 bios: could not use {}: {e}", src.display());
        return None;
    }
    eprintln!(
        "slot: ps1 bios: using the cart's own {} ({} bytes)",
        name,
        std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0)
    );
    Some(dir)
}

/// The plain disc image a PBP wraps, or `None` when there is nothing to unwrap.
///
/// `pcsx_rearmed` cannot open a PBP. RetroArch plays them because the **frontend** ships a PBP
/// extractor, which pulls the image out and hands the core a plain `.iso` — the core never saw
/// a PBP at all, which is why the same core that opens `.chd` here fails on `.pbp`. This is
/// that step, done on this side of the dylib.
///
/// Layout: a 64-byte header, then a table of entries each padded to a 512-byte boundary from
/// its own start — 4 bytes of size, 60 bytes of NUL-padded name, then the data. One of the
/// entries is `ISO`, and that is what a PS1 core reads.
///
/// The image is written to the card, never to `/tmp`: on this device `/tmp` is a 256 MiB tmpfs
/// and a PS1 image is bigger than that, so a RAM copy dies halfway. The card has the room, and
/// the scan only walks `Games/`, so an image parked under `tmp/` is never read back as a cart.
pub fn pbp_image(rom: &Path) -> Option<PathBuf> {
    let is_pbp = rom
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pbp"))
        .unwrap_or(false);
    if !is_pbp {
        return None;
    }
    let stem = rom.file_stem()?.to_str()?;
    let mut f = match std::fs::File::open(rom) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("slot: pbp: cannot read {}: {e}", rom.display());
            return None;
        }
    };
    let mut head = [0u8; 64];
    if std::io::Read::read_exact(&mut f, &mut head).is_err() {
        eprintln!("slot: pbp: {} is shorter than its own header", rom.display());
        return None;
    }
    if &head[0..4] != b"PBPS" {
        eprintln!("slot: pbp: {} does not start with PBPS — not a PBP", rom.display());
        return None;
    }
    // 0x24 is the reserved flag word; 0x02 marks an encrypted (Store) image, whose ISO entry
    // is XOR'd with the key at 0x28. Retail discs converted by PSX2PSP carry no such bit.
    if u32::from_le_bytes(head[0x24..0x28].try_into().ok()?) & 0x02 != 0 {
        eprintln!(
            "slot: pbp: {} is encrypted, only plain images are unwrapped",
            rom.display()
        );
        return None;
    }
    let hdr_size = u32::from_le_bytes(head[0x30..0x34].try_into().ok()?);
    let hdr_start = u32::from_le_bytes(head[0x34..0x38].try_into().ok()?);

    let (off, len) = match find_iso_entry(&mut f) {
        Some(hit) => hit,
        // The header repeats the same pair, as offsets from the file's start. The table is
        // the trustworthy one; this is only there for a file that skips it.
        None => {
            eprintln!(
                "slot: pbp: {} has no ISO entry, trusting the header instead",
                rom.display()
            );
            (hdr_start as u64, hdr_size)
        }
    };
    if len == 0 {
        eprintln!("slot: pbp: {} names an image of zero bytes", rom.display());
        return None;
    }

    let dir = pbp_dir(rom);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("slot: pbp: no scratch for {}: {e}", rom.display());
        return None;
    }
    let dst = dir.join(format!("{stem}.iso"));
    let mut out = match std::fs::File::create(&dst) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("slot: pbp: cannot write {}: {e}", dst.display());
            return None;
        }
    };
    if f.seek(std::io::SeekFrom::Start(off as u64)).is_err() {
        eprintln!("slot: pbp: {} is unreadable at its ISO offset", rom.display());
        return None;
    }
    if std::io::copy(&mut f.take(len as u64), &mut out).is_err() {
        eprintln!("slot: pbp: could not unpack {}", rom.display());
        return None;
    }
    eprintln!(
        "slot: pbp: {} -> {} ({} bytes)",
        rom.display(),
        dst.display(),
        std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0)
    );
    Some(dst)
}

/// Walk the PBP entry table for the `ISO` entry, returning its data offset and size.
fn find_iso_entry(f: &mut std::fs::File) -> Option<(u64, u32)> {
    let mut pos = 64u64;
    // A well-formed PBP has three entries (ISO, SYSTEM.CNF, PICON0.PNG); a ceiling here turns
    // a malformed file into a short refusal instead of a tight loop over its own padding.
    for _ in 0..64 {
        let mut entry = [0u8; 64];
        if f.seek(std::io::SeekFrom::Start(pos)).is_err()
            || std::io::Read::read_exact(f, &mut entry).is_err()
        {
            return None;
        }
        let size = u32::from_le_bytes(entry[0..4].try_into().ok()?);
        if size == 0 {
            return None;
        }
        let end = entry[4..].iter().position(|&b| b == 0).unwrap_or(60);
        if &entry[4..4 + end] == b"ISO" {
            return Some((pos + 64, size));
        }
        let step = 64u64 + size as u64;
        let pad = (512 - step % 512) % 512;
        pos = match pos.checked_add(step).and_then(|p| p.checked_add(pad)) {
            Some(p) => p,
            None => return None,
        };
    }
    None
}

/// Where a PBP's image goes: the content root's `tmp/`, not the process temp dir. See
/// `pbp_image` for why the card and not RAM.
fn pbp_dir(rom: &Path) -> PathBuf {
    for anc in rom.ancestors() {
        if anc.file_name().and_then(|s| s.to_str()) == Some("Games") {
            return anc
                .parent()
                .unwrap_or(Path::new("."))
                .join("tmp")
                .join("slot-ps1-pbp");
        }
    }
    std::env::temp_dir().join("slot-ps1-pbp")
}

/// The options a core has to be told **before `retro_init`**, which is a narrower set than
/// `apply_core_options` and a stricter one: a core that reads a key here and gets no answer
/// falls back to its own hardcoded zero, not to the default its option table advertises.
///
/// Pulled out of `open_core_for_rom` so `crates/slot/tests/ps1_memcard.rs` can assert on it
/// without a dylib, a rom or a device: the answer is a pure function of which core was asked
/// for, and the one bug that ever came from this list — PS1 having no save ram at all — was
/// a missing entry, which is exactly what a test on the list itself catches.
pub fn initial_core_options(core: Core) -> Vec<(&'static str, &'static str)> {
    match core {
        Core::Mgba => vec![("mgba_sgb_borders", "OFF"), ("mgba_gb_colors", "Grayscale")],
        Core::Gpsp => vec![],
        // ★★ 2026-10-24（六六）：「**SFC 进游戏仍然是黑屏但能打开游戏菜单**」。
        //
        //   supafaust 的 `supafaust_pixel_format` 默认是 **`rgb565`**（`strings` 里两个取值
        //   `rgb565` / `xrgb8888` 就在一起）。拿着 16 位帧它自己还要按 `correct_aspect` 把
        //   宽度补到 512 才能出 4:3，而 `retro_run` 交回来的东西喂不到面板上 —— 屏幕上就是
        //   黑的一片，但菜单、按键、相位全都正常（那是前端画的）。
        //   ⇒ 直接要 `xrgb8888`：和 mGBA 同一种帧，中间不再夹一层转换。
        Core::Snes => vec![
            ("supafaust_pixel_format", "xrgb8888"),
            ("supafaust_renderer", "st"),
        ],
        // fceumm hands over a 256x240 frame by default — the full NES raster, 8px of overscan
        // top and bottom that real CRTs hid. Cropping both is what makes the frame exactly 224
        // tall so it matches `FC_SRC_H` and the panel's integer 2x. These are read during
        // `retro_init`, so they have to be seeded here (before `open_with` runs the core's
        // init), the same reason mGBA's startup options are. Without the crop the core would
        // still send 240 rows and the game layer would show the wrong slice.
        //
        // ⚠️ The values are **pixel counts, not switches**. This read `"enabled"` for both,
        // which is not a value these options take (they accept 0-24, defaulting to 8), so the
        // core was being handed two options it could not parse. `8` is the documented default
        // written down on purpose, and it is also the number that lands the frame on
        // `FC_SRC_H`. The texture now resizes to whatever frame arrives regardless — see
        // `GamePass::rebuild_active` — so this is about showing the right *slice* of the
        // picture, not about not crashing.
        Core::Nes => vec![
            ("fceumm_overscan_v_top", "8"),
            ("fceumm_overscan_v_bottom", "8"),
        ],
        // FBNeo needs nothing seeded before `retro_init`: it reads its options at `load_game`
        // or later, and its defaults are the ones SLOT wants (no forced video mode, no
        // auto-rotate). Left empty rather than guessed at.
        Core::Fbneo => vec![],
        // ★ 2026-10-25：genesis_plus_gx，第一次为 MD 打开。
        //
        // * `genesis_plus_gx_overscan = disabled` —— **这条是必须的，不是偏好**。该选项在
        //   `.so` 里的取值是 `top/bottom` / `left/right` / `full` / `disabled`，而**默认
        //   `top/bottom`**：它会在 224 行的画面上下各加一道黑边，交回来的帧变成 240 行。
        //   我们的几何按 **320×224**（`MD_SRC_H = 224`，2× 得 640×448，与 NeoGeo 同规格）
        //   算整数倍，多出来的 16 行会顶到面板上 —— 这正是 FC 上 `fceumm_overscan_v_*`
        //   的同一种问题，处理方式也一致：让核心交出**正好 224 行**。
        // * `genesis_plus_gx_render = single field` —— 隔行只取一场。掌机面板不可能真的
        //   隔行，取单场是唯一诚实的读法；别的取值会把两场拼在一起，动起来是重影。
        // * **其余一概不设**：`region_detect` / `vdp_mode` 保持默认 `auto`，因为 MD 的
        //   游戏分 NTSC 和 PAL 两种，`emu.rs` 已按 `av.fps` 判 `video_locked`（50Hz 的
        //   PAL 游戏按真帧率步进，不强制 60）—— 在这里把它钉死到某一区，就等于让一半
        //   的游戏跑错速度。`aspect_ratio` 也不设：它只在 RetroArch 的 "core provided"
        //   下生效，而缩放是我们自己算的。
        Core::Gpgx => vec![
            ("genesis_plus_gx_overscan", "disabled"),
            ("genesis_plus_gx_render", "single field"),
        ],
        // ★ 2026-10-25 13:20 → 19:17 复盘：真机首跑 PS1《爱藏版封神演义》周期性闪白。
        // 关键实测（`/tmp/slot.log` 的 `slot: frame N` 采样）：
        //
        //   frame 21600  [255,255,255, 255,255,255, 255,255,255]   ← 全白
        //   frame 28200  [255,255,255, 237,161,205, 157,41,157]   ← 上半全白、下半正常
        //   frame 16200  [0,0,0, 0,0,0, 0,0,0]                     ← 全黑
        //
        // 28200 那种"上半白、下半彩"就是**隔行帧的半个画面**：PS1 title screen 白色
        // 背景占了上半天，游戏实际画面压到下半，PCSX 却把这场当整帧交出来。所以问题
        // 不是某个"隔行开关"开错了，而是 **PCSX 内部把两场合成一帧 progressive 的那
        // 一步在 slot 上失败了**——RA 上不闪是因为 RA 有 GPU 合成，slot 是纯 libretro
        // 前端，PCSX 走了另一条合成路径。
        //
        // 这一版 .so 是 `arm64v8 neon ari64 gpu=neon`（`strings` 抓的编译 banner），
        // NEON 加速的隔行开关是 `pcsx_rearmed_neon_interlace_enable_v2`。把它关掉走
        // 纯 C 路径的隔行合成——RA 上同一个核心默认就是关 NEON 分支的（RA 用 GPU
        // 后端接 PCSX，NEON 路径只服务 RA 的软渲染路径），所以"关掉"就是拉齐默认。
        //
        // 之前塞的两行强制选项都撤掉了，原因写在下面：
        //   * `pcsx_rearmed_frameskip` —— 该名字**同时**在 `.so` 里存在（strings 抓到的
        //     `pcsx_rearmed_frameskip` 和 `pcsx_rearmed_frameskip_type` 两个都还在），
        //     可能仍作为短别名支持；撤掉是因为 slot 的 emu.rs 已有自己的 pacing，
        //     核心不该再主动丢帧。若之后要关，用 `pcsx_rearmed_frameskip_type=disabled`。
        //   * `pcsx_rearmed_rgb32_output`（"Show Interlaced Video"，默认 off）—— 描述串
        //     明说开这个会产 interlaced output，撤掉是回归默认 off。实测撤掉后**仍然
        //     闪**，说明这个不是主因，主因是 NEON 隔行合成那一路。
        //
        // ⚠️ **`.m3u` 多碟本期不做**：`pcsx_rearmed` 的 `valid_extensions` 里有 `m3u`，
        //   但换碟要一整套 UI（碟片货架 + 换碟键 + 状态保存），是本轮之后的事。单碟的
        //   `.cue` / `.bin` / `.chd` / `.pbp` 直接起得来。
        // ★ 2026-10-25 20:15（六六指路）：「我认为还是可变分辨率的问题，你 MD 怎么修
        // 这边就怎么修」。MD 的修法就是「取单场」——`genesis_plus_gx_render = single field`，
        // 隔行时不做两场合成，直接取一场作为整帧。PS1 对应的选项是
        // `pcsx_rearmed_alt_flip`（Alternative frame flip），`.so` 里的取值是
        // `disabled` / `early` / `late`，默认 `disabled`（做合成）。
        //
        // 症状复核（用户 20:15 描述）：「640*480 画面框内黑屏一秒闪一秒画面，像线缆接触
        // 不良那种闪法，声音正常、反射层正常」。"声音正常 + 反射层正常"= slot 自己的
        // GPU 管线 OK，音频通道 OK；"640*480 框内黑闪"= PCSX 交出来的原始帧就是半场。
        // 根因是 **slot 没实现 `SET_HW_RENDER`**（grep libretro.rs 无结果）——PCSX 只能走
        // 软渲染路径，软路径下两场合成一帧 progressive 那一步失败，每帧只交出半场
        // （frame 210 `[0,0,0][246,254,246]`、frame 720 `[255,255,255]`、frame 780
        // `[0,0,0]` 这些"半白半黑"就是两场交替的证据）。RA 上不闪是因为 RA 用 HW_RENDER
        // 后端接 PCSX，PCSX 走的是 GPU 合成路径，另一条代码。
        //
        // 取单场（`alt_flip=early`）就是让 PCSX 干脆不合成，永远取第一场作为整帧——
        // 跟 MD 的 `single field` 同一思路，代价是动态画面会有一点点"隔行的静态感"，
        // 但比"闪"强得多。
        //
        // `neon_interlace_enable_v2=disabled` 上一版试了没用，撤掉——它只是关 NEON 分支
        // 的隔行合成，不影响 C 分支。`rgb32_output` 也撤掉，走默认 off（描述串明说
        // off 就是 progressive）。
        //
        // ⚠️ **`.m3u` 多碟本期不做**：`pcsx_rearmed` 的 `valid_extensions` 里有 `m3u`，
        //   但换碟要一整套 UI（碟片货架 + 换碟键 + 状态保存），是本轮之后的事。单碟的
        //   `.cue` / `.bin` / `.chd` / `.pbp` 直接起得来。
        // ★★ 2026-11-01（玩家反馈「640 版本 PS1 不能存档」）：
        //   **`pcsx_rearmed_memcard1` 必须由前端亲口说 `libretro`，否则这张卡的存档区根本不存在。**
        //
        //   证据链（读 `.so` 与上游 `libretro.c`，不是猜的）：
        //   1. `retro_get_memory_data(RETRO_MEMORY_SAVE_RAM)` 里写着
        //      `if (memcard_type[0] == MEMCARDTYPE_LIBRETRO) return Mcd1Data;`
        //      —— 不是 libretro 模式就 `return NULL`（`libretro.c:2231`/`2244`）。
        //   2. `memcard_type` 是**枚举零值 = MEMCARDTYPE_NONE**（`libretro.c:133-138`），
        //      只在 `load_memcards()`（`retro_load_game` 里调）被改写。
        //   3. `load_memcards()` 逐槽问 `GET_VARIABLE`：
        //      `if (!environ_cb(GET_VARIABLE, &var) || !var.value) { LogErr("non memcard%d
        //      config?"); continue; }` —— **前端不回答就 continue**，`memcard_type` 停在 NONE。
        //   4. 我们的 `environment()` 只答自己 `set_option` 过的 key（`h.options.get(key)`），
        //      **从不读核心自己声明的默认值**（前端也没实现 `SET_CORE_OPTIONS` 注册那一路）。
        //      所以「界面上写着 Libretro (Default)」对我们等于不存在 —— 一旦没人显式设过
        //      `pcsx_rearmed_memcard1`，核心对前端交出的观点就是"没有记忆卡"。
        //   5. 于是 `LibretroCore::save_ram()` 拿 NULL/0 长度 → 返回 `None` → `persist::flush`
        //      的 `sav` 为 `None` → `Saves/PS1/<stem>.sav` **从来没被写过**。
        //      游戏里同样表现为"没有记忆卡"。
        //
        //   ⇒ 这一行不是偏好，是**存档能不能存在**。PS1 从 2026-10-25 接入起就带着这个洞；
        //     与 640/720 分辨率无关（两棵树的 `core.rs` 当时逐字节相同）。
        //
        //   为什么放 `initial_options`（`retro_init` 之前注入）而不是 `apply_core_options`：
        //   选项名自带 "(Restart)"，`load_memcards` 属于 `retro_load_game` 那一步，
        //   与 mGBA 的 `mgba_sgb_borders` 同一类 —— 见上面那段注释的判据。
        //   卡 2 **不设**：不回答时它同样落在 NONE，效果与显式写 `none` 一致，少一行算一行。
        Core::Pcsx => vec![
            ("pcsx_rearmed_alt_flip", "early"),
            ("pcsx_rearmed_memcard1", "libretro"),
            // ★ 2026-11-06（读取速度优化）：`pcsx_rearmed_cd_readahead` = 提前预读多少个
            //   **CD 扇区**，用来消掉 CD 读盘 / 串流时的停顿。
            //
            //   ⚠️⚠️ **2026-11-09 更正：这个值的单位是扇区，不是 MB。** 当初写这条注释时按
            //   "MB"理解，于是把 256 想成了 256 MB、把 333000 想成了"整盘 750MB"。实测源码
            //   （`libpcsxcore/cdrom-async.c`，`licenses/` 有对应 tar）：
            //     · `cdra_set_buf_count(n)` 只是把 `acdrom.buf_cnt = n` 后重启预读线程；
            //     · `lbacache_start` 里 `calloc(buf_cnt, sizeof(struct cached_buf))`，
            //       而 `struct cached_buf` = `lba_wflag`(4) + `buf[CD_FRAMESIZE_RAW=2352]`
            //       + `buf_sub[SUB_FRAMESIZE=96]` ≈ **2 452 B / 扇区**。
            //   ⇒ 256 扇区 ≈ **613 KiB**（不是 256 MB）；333000 扇区 ≈ **779 MiB**（这才是
            //   "整盘进内存"，也正好解释当初记下的 +750MB）。所以这个档位离"压垮机器"很远，
            //   它只是一块普通的预读缓存；真要更大的缓冲，1024 扇区也才 ~2.4 MB。
            //
            //   取值表（来自 `.so` 的选项）：10/11/12/13/14/15/16/32/64/128/256/512/1024，
            //   外加 333000 = 整张盘进内存（≈780 MiB RAM，1GB 机器不取）。
            //   这是纯缓冲优化，对游戏逻辑无副作用。与 `cd_turbo`（让模拟光驱极速、但很多游戏
            //   会崩）是两回事——后者风险过高，不做默认开启，需要时在 `apply_core_options`
            //   里按需单独开、且只应对确认不崩的游戏。
            ("pcsx_rearmed_cd_readahead", "256"),
        ],
        // ★ 2026-10-25：pce_fast，第 8 个核心。**只设两项，都是"别动画面"那一类。**
        //
        // * `pce_fast_initial_scanline = 11` / `pce_fast_last_scanline = 234` —— 该核心默认
        //   3..242，即 **240 行**（两个默认值都是从 `.so` 里抠出来的：标签串分别是
        //   `3 (Default)` 与 `242 (Default)`）。240 行是带着 overscan 的整栅，而 PC Engine
        //   的可见画面是 224 行 —— 和 FC 上 `fceumm_overscan_v_*` 是**同一条规矩**：
        //   让核心交出正好 224 行，前端 `PCE_SRC_H` 才说得上是它的帧高，整数 2× 也才对得上。
        //   上下各裁 8 行（3→11、242→234），与 fceumm 的 8+8 一致。
        //   ⚠️ **这两个数待真机确认**：PC Engine 的 overscan 分布我没有第二来源，
        //   若真机上画面上下被切或露出垃圾带，先调这两个值再怀疑别的。
        //
        // * `pce_fast_frameskip` **不设**。它的取值只有 `auto` / `manual`，没有 `disabled`；
        //   而该核心自己的标签写着「Frameskip disabled - frontend does not support audio
        //   buffer status monitoring」—— 我们的前端不实现那个环境调用，所以它本来就关。
        //   硬写一个值只会覆盖掉它自己的判断，和 PS1 那边"必须写 disabled"正好相反。
        //
        // * 其余一概不设：`nospritelimit`（默认 disabled，同屏超 16 个精灵时的闪烁是原版
        //   行为）、`palette`（默认 Composite）、`hoverscan`、CD 那一组（`.cue` 才用到），
        //   保持默认。没有实测依据的值不猜。
        Core::PceFast => vec![
            ("pce_fast_initial_scanline", "11"),
            ("pce_fast_last_scanline", "234"),
        ],
    }
}

/// Options a core needs before `load`, because libretro cores read them during
/// `retro_load_game` rather than continuously.
///
/// `auto` resolves the serial protocol from the ROM, so two devices running the same game
/// agree on a mode without either being told which. Anything more deliberate belongs to a
/// link session, which knows what the other end picked. mGBA gets nothing: it has no
/// `gpsp_serial` option, and handing it one anyway is a landmine the day it grows one.
pub fn apply_core_options(core: &mut LibretroCore, which: Core) {
    if which == Core::Gpsp {
        core.set_option("gpsp_serial", "auto");
    }
    // ★★ 2026-10-27（六六）：「FBNeo 有没有把 NeoGeo BIOS 默认设成 UNIBIOS 的选项？有的话
    //   我们也默认设成这个」。
    //
    //   有。`fbneo-neogeo-mode` 这一版核心的取值是 7 个（从 `.so` 的
    //   `retro_core_option_definition` 结构里直接读出来的，不是猜的）：
    //     `DIPSWITCH` / `MVS_EUR` / `MVS_USA` / `MVS_JAP` / `AES_EUR` / `AES_JAP` / `UNIBIOS`
    //   界面上最后那一项写的是 "Unibios mode"，**取值 token 是全大写的 `UNIBIOS`**。
    //
    //   前提：卡上得有 uni-bios 的 ROM。这台机器的 `Games/NEOGEO/neogeo.zip` 里带
    //   `uni-bios_1_0 … 2_3`（实测列过），所以核心能取到；没有的话它会退回官方 BIOS。
    //
    //   同一个核心还有一项**画质**开关：`fbneo-allow-depth-32`（"Use 32-bits color depth
    //   when available"）—— 打开后核心改用 XRGB8888 绘制，Sprite 混合/半透明比 16 位准。
    //   前端的 `SET_PIXEL_FORMAT` 认 XRGB8888（`slot-retro/src/libretro.rs`），所以能接。
    if which == Core::Fbneo {
        core.set_option("fbneo-neogeo-mode", "UNIBIOS");
        core.set_option("fbneo-allow-depth-32", "enabled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Both tests below mutate `SLOT_CORE`, which is process-global; the test harness runs
    /// tests in this module on separate threads by default, so without this they can
    /// interleave and read back a value neither of them set.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The half of "the dylib chosen and the state directory used agree" that lives entirely
    /// in this module: `candidates`, and therefore `open_core`, never spells a filename that
    /// does not match the `Core` it was handed. `crates/slot/tests/gpsp.rs` pins the other
    /// half — that a cart resolved to the same `Core` reads its resume state from the
    /// matching `States/<core>/` directory — through the real `Session`.
    #[test]
    fn candidates_search_the_named_cores_own_filename_only() {
        let _g = lock();
        // A developer's shell leaking `SLOT_CORE` into this run would short-circuit the
        // search this test exists to check, so it cannot assume the var is unset.
        std::env::remove_var(CORE_ENV);

        let root = Path::new("/root");
        let mgba = candidates(root, Core::Mgba);
        let gpsp = candidates(root, Core::Gpsp);
        assert_ne!(mgba, gpsp, "the two cores searched the same paths");
        assert!(!mgba.is_empty());
        assert!(!gpsp.is_empty());
        for path in &mgba {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with("mgba_libretro"), "{name} is not mGBA's");
        }
        for path in &gpsp {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with("gpsp_libretro"), "{name} is not gpSP's");
        }
    }

    /// The property `crates/slot/tests/gpsp.rs`'s integration test relies on: a content root
    /// with its own tmp directory is not near `current_exe()` or `./vendor`, so without this
    /// candidate an integration test has nowhere to plant a fake dylib for `open_core` to
    /// find. Root cause of the gap F3 closed — before this candidate existed, a mutation
    /// that fed `open_core` the wrong `Core` had no candidate list a test could observe
    /// disagree.
    #[test]
    fn candidates_search_the_roots_own_system_directory() {
        let _g = lock();
        std::env::remove_var(CORE_ENV);
        let root = Path::new("/some/content/root");
        assert_eq!(
            candidates(root, Core::Gpsp)[0],
            root.join("System").join(dylib_name(Core::Gpsp)),
        );
    }

    /// The override is a filename, not a `Core`: it must win regardless of which core asked,
    /// which is what makes it a trap when the ini disagrees rather than a second selector.
    #[test]
    fn the_env_override_ignores_which_core_was_asked_for() {
        let _g = lock();
        std::env::set_var(CORE_ENV, "/dev/null/named-core");
        let root = Path::new("/root");
        let mgba = candidates(root, Core::Mgba);
        let gpsp = candidates(root, Core::Gpsp);
        std::env::remove_var(CORE_ENV);
        assert_eq!(mgba, vec![PathBuf::from("/dev/null/named-core")]);
        assert_eq!(
            mgba, gpsp,
            "the override stopped winning for one of the cores"
        );
    }
}
