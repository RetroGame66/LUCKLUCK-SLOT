use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use crate::scan::System;

/// GBA cartridge header: a 12 byte ASCII game title, NUL padded on the right.
const TITLE_OFF: usize = 0xa0;
const TITLE_LEN: usize = 12;

/// The four character game code, immediately after the title. The fourth character is the
/// region, so callers keying on the game itself want the first three.
const CODE_OFF: usize = 0xac;
const CODE_LEN: usize = 4;

/// Everything the shelf wants out of a header, and the point at which the read stops. The
/// game code is the last field in it, so this is exactly as far as anything here reaches.
const HEAD_LEN: u64 = (CODE_OFF + CODE_LEN) as u64;

/// Game Boy header: the title at 0x134, fifteen bytes of it. There is no game code on a
/// Game Boy cart — the title is the game's own identity, and it is what `code` holds for
/// these machines.
const GB_TITLE_OFF: usize = 0x134;
const GB_TITLE_LEN: usize = 15;

/// A 512 byte copier header, the shift a dump carrying one puts everything by.
const COPIER_SHIFT: usize = 0x200;

/// The header checksum: the sum runs over the twenty-five bytes from the title to the
/// version byte, and the answer sits one byte past them.
const GB_SUM_LEN: usize = 0x19;

/// A read that reaches the checksum of a copier-headed dump stops here.
const GB_HEAD_LEN: u64 = (COPIER_SHIFT + GB_TITLE_OFF + GB_SUM_LEN + 1) as u64;

/// Offset of the Super Game Boy flag byte in a Game Boy / Game Boy Color cartridge header.
const SGB_FLAG_OFF: usize = 0x146;

/// ★ 2026-10-24（六六）：「GBC 有两种卡带，共通和专用，得画两份」—— 这个字节就是"哪一份"。
///
/// `0x80` = **双模**（CGB enhanced，插初代 GB 也能跑，壳是**共通壳**，带锁扣缺口）；
/// `0xC0` = **GBC 专用**（壳无缺口，物理上插不进 GB）；
/// `0x00`（及别的一切）= 初代 GB 游戏，同样是共通壳。
///
/// 它和 `SGB_FLAG_OFF` 差三个字节，都在同一次头读取里，所以判壳型不额外开文件 ——
/// 卡上的目录遍历是这台机器最慢的一环，一个字节的差别不值得第二次 `open`。
const CGB_FLAG_OFF: usize = 0x143;

/// 壳型判据的两个值，写在这儿而不是散在 `slot-ui`：读 header 的和画卡带的要认同一对数。
pub const CGB_DUAL: u8 = 0x80;
pub const CGB_ONLY: u8 = 0xC0;

/// Title and code, in one read.
///
/// Read once rather than twice because the shelf asks for both for every cart on the card,
/// and on the device the card is the slow part: two opens per rom is two directory lookups
/// and two cluster chain walks where one will do, multiplied by however many carts are on
/// it. Only the header is read, never the whole rom, which can be 32 MB.
///
/// A rom that is missing, truncated, or unreadable is two empty strings rather than an
/// error. Nothing on this path can act on a failure — an unreadable rom has no title, and
/// the shelf still has to show it — so the empty string is the whole of the answer.
///
/// **`code` is the game's own identity, and it is machine shaped.** An Advance cart carries
/// a four character header code; a Game Boy cart has no code field at all, so its own
/// header title stands in — the same field the toolbox's editor and the sticker rekey read,
/// which is why a rule keyed on a Game Boy game reads as its title. `system` decides which
/// shape this is, off the extension exactly as everything else here does; `None` reads as
/// an Advance, the shape this function had before Game Boy had one.
pub fn header(rom: &Path, system: Option<System>) -> (String, String) {
    let h = read_header(rom, system);
    (h.0, h.1)
}

/// [`header`] 加上 CGB 标志字节 —— **一次读，三个答案**。
///
/// 第三个值是 Game Boy / Game Boy Color 卡带头 `0x143` 的原字节（`Some(0x80)` 双模、
/// `Some(0xC0)` 专用、`Some(0x00)` 初代 GB），非 Game Boy 机种与读不到的头是 `None`。
/// 判壳型只看这一个字节，见 [`CGB_ONLY`]。
pub fn read_header(rom: &Path, system: Option<System>) -> (String, String, Option<u8>) {
    // Consoles (SFC / FC) carry neither a Game Boy title nor a GBA game code, so there is
    // nothing in this header for them; a caller that asks is reading the wrong format.
    if matches!(system, Some(System::Sfc | System::Fc)) {
        return (String::new(), String::new(), None);
    }
    let gb = matches!(system, Some(System::Gb | System::Gbc));
    let want = if gb { GB_HEAD_LEN } else { HEAD_LEN };
    let Ok(file) = File::open(rom) else {
        return (String::new(), String::new(), None);
    };
    let mut buf = Vec::with_capacity(want as usize);
    // `take` rather than `read_exact`: a short rom reads the bytes that are there, so a
    // truncated file is an empty field rather than a cart that never reaches the shelf.
    if file.take(want).read_to_end(&mut buf).is_err() {
        return (String::new(), String::new(), None);
    }
    if gb {
        // A dump with a copier header in front of it carries its whole header at +0x200,
        // checksum included — and the checksum only adds up where the bytes really are, so
        // it says which of the two this is. Neither place adding up is not a dump this knows
        // (a homebrew rom with the checksum left at zero, say), and the unshifted read is
        // still the best guess: a wrong one costs a title, and the shelf shows the file name
        // rather than this.
        let plain = header_sums(&buf, GB_TITLE_OFF);
        let shifted = header_sums(&buf, GB_TITLE_OFF + COPIER_SHIFT);
        let shift = if plain {
            0
        } else if shifted {
            COPIER_SHIFT
        } else {
            0
        };
        let title = gb_title(&buf, GB_TITLE_OFF + shift).unwrap_or_default();
        // ★ 同一个 shift：带 copier 头的 dump，`0x143` 也跟着挪到 +0x200。
        //
        // ★★ 但**只有校验和认可这个头时才采信它**。卡上有一堆 `.zip`（FBNeo / 整合包），
        //   而 `read_header` 对它们照样会 open 前 0x34E 字节 —— 那是**压缩容器**，
        //   不是一个游戏头。实测（2026-10-24 23:0x 真机）：GB 的 zip 在缓存里读出
        //   `cgb = 213`（0xD5）这种值，因为那个字节是从 zip 里随手捡的。
        //   后果是随机一半的 GBC 卡被画成专用壳、另一半共通壳 —— **看起来像"判据生效了"，
        //   其实是随机的**。校验和对不上 ⇒ 不知道 ⇒ `None` ⇒ 退回机种默认壳。
        let cgb = if plain || shifted {
            buf.get(CGB_FLAG_OFF + shift).copied()
        } else {
            None
        };
        return (title.clone(), title, cgb);
    }
    (
        field_at(&buf, TITLE_OFF, TITLE_LEN).unwrap_or_default(),
        field_at(&buf, CODE_OFF, CODE_LEN).unwrap_or_default(),
        // 0x143 sits inside an Advance cart's title field, where it means nothing:
        // an Advance cart has one shell.
        None,
    )
}

/// Whether the Game Boy header checksum at `off + 0x19` vouches for the block that starts
/// at `off`. The boot ROM runs this same sum over the title-to-version bytes, so a header
/// that has been moved — a copier header's 512 bytes in front of it — stops adding up, and
/// the one that has been moved back adds up again. A witness rather than a guess: no
/// constant to transcribe, and the header is the authority on itself.
fn header_sums(buf: &[u8], off: usize) -> bool {
    let Some(block) = buf.get(off..off + GB_SUM_LEN) else {
        return false;
    };
    let Some(&want) = buf.get(off + GB_SUM_LEN) else {
        return false;
    };
    let mut sum: u8 = 0;
    for b in block {
        sum = sum.wrapping_sub(*b).wrapping_sub(1);
    }
    sum == want
}

/// A Game Boy title as the printable run it is. The field is fifteen bytes wide on a
/// monochrome cart and eleven on a Color one, with the maker code and the Color flag
/// living in what is left of it — so the run is where the title ends, rather than a width
/// that has to know which machine this is.
fn gb_title(buf: &[u8], off: usize) -> Option<String> {
    let field = buf.get(off..off + GB_TITLE_LEN)?;
    let text: String = field
        .iter()
        .take_while(|b| b.is_ascii_graphic() || **b == b' ')
        .map(|b| *b as char)
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The title alone. Kept for callers that want one field; it costs an open of its own, so
/// anything that wants both should ask `header` instead.
pub fn header_title(rom: &Path, system: Option<System>) -> Option<String> {
    let (title, _) = header(rom, system);
    (!title.is_empty()).then_some(title)
}

pub fn header_code(rom: &Path, system: Option<System>) -> Option<String> {
    let (_, code) = header(rom, system);
    (!code.is_empty()).then_some(code)
}

/// Whether a GB/GBC rom carries Super Game Boy enhancements, and so would draw an SGB
/// border under mGBA's default `mgba_sgb_borders = ON`.
///
/// The flag byte at `0x146` is `0x03` on carts that use SGB functions — the same games that
/// ship the SGB's decorative border art. `0x00` is a plain Game Boy cart; `0x01`/`0x02` are
/// reserved. A missing or truncated rom reads as not enhanced rather than erroring, because a
/// shelf still has to show it whatever its header says.
///
/// SLOT turns mGBA's SGB border off (it draws its own bezel), so this is mostly a way to
/// tell those carts apart — and to log which ones a hand debugging "framed differently"
/// complaints are really about.
pub fn sgb_enhanced(rom: &Path) -> bool {
    let Ok(mut file) = File::open(rom) else {
        return false;
    };
    // Seek to the flag and read it; `read` returns short on a rom shorter than 0x147 bytes,
    // leaving the buffer zeroed — the not-enhanced answer, not a panic on a truncated file.
    if file.seek(std::io::SeekFrom::Start(SGB_FLAG_OFF as u64)).is_err() {
        return false;
    }
    let mut flag = [0u8; 1];
    let n = file.read(&mut flag).unwrap_or(0);
    n == 1 && flag[0] == 0x03
}

fn field_at(buf: &[u8], off: usize, len: usize) -> Option<String> {
    text_from_bytes(buf.get(off..off + len)?)
}

fn text_from_bytes(buf: &[u8]) -> Option<String> {
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    let text = std::str::from_utf8(&buf[..end]).ok()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        header, read_header, sgb_enhanced, text_from_bytes, CGB_DUAL, CGB_FLAG_OFF, CGB_ONLY,
        COPIER_SHIFT, GB_SUM_LEN, GB_TITLE_OFF,
    };
    use crate::scan::System;
    use std::io::Write;

    #[test]
    fn padding_is_trimmed_but_interior_spaces_are_kept() {
        let full = text_from_bytes(b"POKEMON EMER");
        assert_eq!(full.as_deref(), Some("POKEMON EMER"));
        assert_eq!(
            text_from_bytes(b"ADVANCEWARS\0").as_deref(),
            Some("ADVANCEWARS")
        );
        assert_eq!(text_from_bytes(b"KIRBY      \0").as_deref(), Some("KIRBY"));
        assert_eq!(text_from_bytes(&[0u8; 12]), None);
    }

    #[test]
    fn one_read_returns_both_fields() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Emerald.gba");
        let mut bytes = vec![0u8; 0x100];
        bytes[0xa0..0xac].copy_from_slice(b"POKEMON EMER");
        bytes[0xac..0xb0].copy_from_slice(b"BPEE");
        std::fs::File::create(&rom)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
        assert_eq!(
            header(&rom, Some(System::Gba)),
            ("POKEMON EMER".to_string(), "BPEE".to_string())
        );
    }

    #[test]
    fn a_truncated_rom_yields_empty_fields_rather_than_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Tiny.gba");
        std::fs::write(&rom, [0u8; 8]).unwrap();
        assert_eq!(header(&rom, Some(System::Gba)), (String::new(), String::new()));
    }

    #[test]
    fn a_missing_rom_yields_empty_fields() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            header(&dir.path().join("nope.gba"), Some(System::Gba)),
            (String::new(), String::new())
        );
    }

    #[test]
    fn sgb_flag_at_0x146_distinguishes_enhanced_carts() {
        let dir = tempfile::tempdir().unwrap();
        // SGB-enhanced: flag byte == 0x03.
        let sgb = dir.path().join("Enhanced.gb");
        let mut bytes = vec![0u8; 0x148];
        bytes[0x146] = 0x03;
        std::fs::write(&sgb, &bytes).unwrap();
        assert!(sgb_enhanced(&sgb));
        // Plain cart: flag byte == 0x00. Its own buffer, not the one above — reusing that one
        // wrote 0x03 into this file as well and the assertion below could never have held.
        let plain = dir.path().join("Plain.gb");
        std::fs::write(&plain, vec![0u8; 0x148]).unwrap();
        assert!(!sgb_enhanced(&plain));
        // Reserved / other values are not enhanced.
        for f in [0x01u8, 0x02, 0xff] {
            let r = dir.path().join(format!("R{f:x}.gb"));
            let mut b = vec![0u8; 0x148];
            b[0x146] = f;
            std::fs::write(&r, &b).unwrap();
            assert!(!sgb_enhanced(&r), "flag {f:#x} is not SGB-enhanced");
        }
        // A rom shorter than the flag offset reads as not enhanced, not an error.
        let tiny = dir.path().join("Tiny.gb");
        std::fs::write(&tiny, [0u8; 8]).unwrap();
        assert!(!sgb_enhanced(&tiny));
        // A missing rom reads as not enhanced.
        assert!(!sgb_enhanced(&dir.path().join("nope.gb")));
    }

    /// The checksum the boot ROM would compute over whatever the block already holds, so a
    /// fixture is a header that vouches for itself the way a real dump's does.
    fn stamp_gb_checksum(bytes: &mut [u8], base: usize) {
        let off = base + GB_TITLE_OFF;
        let mut sum: u8 = 0;
        for b in &bytes[off..off + GB_SUM_LEN] {
            sum = sum.wrapping_sub(*b).wrapping_sub(1);
        }
        bytes[off + GB_SUM_LEN] = sum;
    }

    /// A Game Boy cart's own identity is its header title — there is no code field on one,
    /// so the title stands in, in both fields. This is what the toolbox's editor and the
    /// sticker rekey already read, and what a card-side rule keys such a game on.
    #[test]
    fn a_game_boy_cart_identifies_by_its_header_title() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Red.gb");
        let mut bytes = vec![0u8; 0x180];
        bytes[GB_TITLE_OFF..GB_TITLE_OFF + 11].copy_from_slice(b"POKEMON RED");
        stamp_gb_checksum(&mut bytes, 0);
        std::fs::write(&rom, &bytes).unwrap();
        let both = ("POKEMON RED".to_string(), "POKEMON RED".to_string());
        assert_eq!(header(&rom, Some(System::Gb)), both);
        assert_eq!(header(&rom, Some(System::Gbc)), both);
    }

    /// A dump with a 512 byte copier header in front of it carries its whole header, the
    /// checksum that vouches for it included, at +0x200 — and only there, which is how the
    /// shift is known rather than guessed.
    #[test]
    fn a_copier_header_shifts_the_game_boy_title() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Shifted.gb");
        let mut bytes = vec![0u8; COPIER_SHIFT + 0x180];
        let base = COPIER_SHIFT;
        bytes[base + GB_TITLE_OFF..base + GB_TITLE_OFF + 11].copy_from_slice(b"POKEMON RED");
        stamp_gb_checksum(&mut bytes, base);
        std::fs::write(&rom, &bytes).unwrap();
        assert_eq!(
            header(&rom, Some(System::Gb)),
            ("POKEMON RED".to_string(), "POKEMON RED".to_string())
        );
    }

    /// A Color cart's title is eleven bytes; the maker code and the 0x80 flag live in what
    /// is left of the fifteen, and neither is part of the name — but the flag is inside the
    /// checksum's block, so a real Color cart still vouches for itself with it in place.
    #[test]
    fn a_color_flag_after_the_title_ends_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Tcg.gbc");
        let mut bytes = vec![0u8; 0x180];
        bytes[GB_TITLE_OFF..GB_TITLE_OFF + 11].copy_from_slice(b"POKEMON TCG");
        bytes[GB_TITLE_OFF + 11] = 0x80;
        stamp_gb_checksum(&mut bytes, 0);
        std::fs::write(&rom, &bytes).unwrap();
        assert_eq!(
            header(&rom, Some(System::Gbc)),
            ("POKEMON TCG".to_string(), "POKEMON TCG".to_string())
        );
    }

    /// ★ 2026-10-24（六六）：「GBC 有两种卡带，共通和专用，得画两份」。
    ///
    /// 判据就是头 `0x143` 这个字节，而且它必须**跟着 copier 头的位移一起挪** ——
    /// 一个带 512 字节壳头的 dump，`0x143` 实际在 +0x200，读错位置就永远是 0（共通壳），
    /// 表现是"专用壳的游戏全都画成共通壳"，没有任何别的症状。
    #[test]
    fn the_cgb_flag_says_which_shell_a_game_boy_cart_wears() {
        let dir = tempfile::tempdir().unwrap();

        // 三种取值，各自的偏移（0 = 干净 dump，0x200 = 带壳头）
        for (name, flag, shift) in [
            ("Only", CGB_ONLY, 0usize),
            ("Dual", CGB_DUAL, 0usize),
            ("Plain", 0x00, 0usize),
            ("ShiftedOnly", CGB_ONLY, COPIER_SHIFT),
        ] {
            let rom = dir.path().join(format!("{name}.gbc"));
            let mut bytes = vec![0u8; shift + 0x180];
            let base = shift;
            bytes[base + GB_TITLE_OFF..base + GB_TITLE_OFF + 8].copy_from_slice(b"TESTCART");
            bytes[base + CGB_FLAG_OFF] = flag;
            // 有壳头时**只**给移位后的那份写校验和：校验和对不上的那份会被当成原始读取，
            // 这正是 shift 被"证明"而不是"猜"的方式。
            stamp_gb_checksum(&mut bytes, base);
            std::fs::write(&rom, &bytes).unwrap();
            let (_, _, cgb) = read_header(&rom, Some(System::Gbc));
            assert_eq!(cgb, Some(flag), "{name}: 0x143 读错了（shift={shift}）");
        }

        // 非 Game Boy 机种没有这个字节：Advance 卡的 0x143 落在标题字段里，没有意义。
        let gba = dir.path().join("Any.gba");
        let mut bytes = vec![0u8; 0x180];
        bytes[0xa0..0xac].copy_from_slice(b"ADVANCETITLE");
        std::fs::write(&gba, &bytes).unwrap();
        assert_eq!(read_header(&gba, Some(System::Gba)).2, None);


        // ★★ 真机踩过（2026-10-24 23:0x）：卡上的 `.zip` 也会被 `read_header` 打开，
        //   读到的是**压缩容器**，`0x143` 只是其中随手一个字节。实测缓存里出现过
        //   `cgb = 213`。所以校验和对不上的头**一律不采信** —— 写个 0xC0 进去也不认。
        //   否则 GBC 货架上会随机一半专用壳、一半共通壳，看着像判据生效，其实是掷骰子。
        let garbage = dir.path().join("Garbage.gbc");
        let mut bytes = vec![0u8; 0x180];
        bytes[CGB_FLAG_OFF] = CGB_ONLY;
        // 特意不盖校验和：这份头自己不算数。
        std::fs::write(&garbage, &bytes).unwrap();
        assert_eq!(
            read_header(&garbage, Some(System::Gbc)).2,
            None,
            "校验和对不上时 0x143 不可信，必须返回 None 而不是那个字节"
        );

        // 读不到的头同样是 `None`，不是 0 —— 调用方拿 `None` 退回机种默认壳。
        assert_eq!(
            read_header(&dir.path().join("nope.gbc"), Some(System::Gbc)).2,
            None
        );
    }

}
