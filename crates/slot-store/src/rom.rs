//! Reading a rom off the card, out of a `.zip` when that is how it is stored.
//!
//! Only one core in the set opens a zip itself — FBNeo, whose romsets *are* zips — so every
//! other core is handed a path and expects a rom at it. A zipped rom for one of those has to be
//! unpacked here before it reaches the core.
//!
//! This lives in `slot-store` rather than in `slot-retro` for two reasons. It is where
//! reading a rom already happened — the header reader opens the same files — and this crate
//! can be tested on a Windows host, which `slot-retro` cannot: `libloading` needs a
//! `dlltool` the sandbox will not spawn, so every test in that crate is compile-only.

use std::fs;
use std::io::Read;
use std::path::Path;

use zip::ZipArchive;

/// What went wrong reading a rom, in terms a person can act on.
#[derive(Debug)]
pub enum RomError {
    /// The file is not there, or cannot be read.
    Io(std::io::Error),
    /// The file is a zip and cannot be opened as one.
    NotAZip(String),
    /// The zip opened but holds nothing that could be the rom.
    EmptyZip(String),
}

impl std::fmt::Display for RomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RomError::Io(e) => write!(f, "{e}"),
            RomError::NotAZip(p) => write!(f, "{p} is not a readable zip"),
            RomError::EmptyZip(p) => write!(f, "{p} holds no file that could be a rom"),
        }
    }
}

impl std::error::Error for RomError {}

impl From<std::io::Error> for RomError {
    fn from(e: std::io::Error) -> Self {
        RomError::Io(e)
    }
}

/// The rom's bytes, unpacked if it is stored as a zip.
///
/// `unpack_zip` is [`RetroCore::reads_zip_natively`]'s answer: false means the core wants the
/// archive whole, which is FBNeo and only FBNeo.
///
/// The bytes are handed to the core as `GameInfo::data` either way, so this changes nothing
/// about the memory picture — a rom was always read into memory in full, because the core keeps
/// a pointer into it for as long as the game is loaded. Unpacking replaces one buffer with
/// another of the same kind.
pub fn read_rom(path: &Path, unpack_zip: bool) -> Result<Vec<u8>, RomError> {
    Ok(read_rom_named(path, unpack_zip)?.0)
}

/// The same bytes, plus **the name of the archive entry they came out of** when they came out
/// of one.
///
/// ★ 2026-10-24：调用方要那个名字不是为了显示，是为了给核心一个**带对扩展名的真文件**
/// —— 见 `slot_retro` 的 `load`。压缩包里的名字是唯一的线索（`Games/` 下那盘 rom 叫
/// `C 口袋炸弹人.zip`，里面是 `C 口袋炸弹人.nes`），而扩展名正是有些核心挑加载器用的东西。
pub fn read_rom_named(path: &Path, unpack_zip: bool) -> Result<(Vec<u8>, Option<String>), RomError> {
    if !unpack_zip || !is_zip(path) {
        return Ok((fs::read(path)?, None));
    }
    unpack(path)
}

/// Whether a file is a zip, by its first four bytes.
///
/// A sniff rather than a test of the extension, because the extension is not what decided this
/// rom was a cart — the folder did (see `slot_store::System::from_dir`) — and a zip named
/// `.gba` is exactly as much a zip as one named `.zip`.
pub fn is_zip(path: &Path) -> bool {
    let mut head = [0u8; 4];
    match fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)) {
        Ok(()) => &head == b"PK\x03\x04",
        Err(_) => false,
    }
}

/// Pull the one file out of the archive that is the rom.
fn unpack(path: &Path) -> Result<(Vec<u8>, Option<String>), RomError> {
    let file = fs::File::open(path)?;
    let shown = path.display().to_string();
    let mut archive = ZipArchive::new(file).map_err(|e| RomError::NotAZip(format!("{shown}: {e}")))?;
    let Some(index) = biggest(&mut archive).map_err(|e| RomError::NotAZip(format!("{shown}: {e}")))? else {
        return Err(RomError::EmptyZip(shown));
    };
    let mut entry = archive
        .by_index(index)
        .map_err(|e| RomError::NotAZip(format!("{shown}: {e}")))?;
    let name = entry.name().to_string();
    let mut out = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut out)
        .map_err(|e| RomError::NotAZip(format!("{shown}: {e}")))?;
    Ok((out, Some(name)))
}

/// Which entry in the archive is the rom: **the largest one**.
///
/// Size separates a rom from everything else by orders of magnitude — a rom is megabytes, a
/// readme or a `.nfo` is kilobytes, a screenshot is hundreds of kilobytes — so this needs no
/// table of extensions to guess from and no knowledge of which machine the rom is for, neither
/// of which this layer has.
///
/// A zip holding two roms is a "2-in-1" the user should have split. The larger one is as good
/// an answer as any, and the shelf shows the archive's own name either way, so nothing about
/// the choice is hidden from the player.
///
/// Directories and card bookkeeping are skipped: `__MACOSX` and dot-files are what macOS leaves
/// beside a copy, and they are small enough that they would never win anyway — skipping them
/// just keeps them from being the *only* thing in an archive that is all bookkeeping.
fn biggest<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<Option<usize>, zip::result::ZipError> {
    let mut best: Option<(usize, u64)> = None;
    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i)?;
        if entry.is_dir() || is_bookkeeping(entry.name()) {
            continue;
        }
        if best.is_none_or(|(_, size)| entry.size() > size) {
            best = Some((i, entry.size()));
        }
    }
    Ok(best.map(|(i, _)| i))
}

/// Whether a name is card bookkeeping rather than content.
fn is_bookkeeping(name: &str) -> bool {
    name.starts_with('.')
        || name.starts_with("__MACOSX")
        || name.split(['/', '\\']).any(|c| c.starts_with('.'))
}
