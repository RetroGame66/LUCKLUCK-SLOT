//! Lyrics, as the `.lrc` that belongs to a track.
//!
//! The format is a line of `[mm:ss.xx]` tags followed by the words those times belong to, and
//! the two things about it that matter here are both about **what a card in this part of the
//! world actually holds**:
//!
//! - **The file is GBK more often than it is UTF-8.** Both of the lyric files this was written
//!   against were GBK, with no BOM. So the bytes go through `playlist::decode`, which is the
//!   same UTF-8-then-GBK fallback the playlist reader already uses, rather than through a
//!   second decode that could disagree with it.
//! - **The `[ti:]` / `[ar:]` / `[al:]` headers carry nothing.** The two files this was written
//!   against had all three empty. A lyric file is not the place the deck learns a title from —
//!   that is the container's own metadata, which `Track::title` already holds — so these are
//!   skipped rather than parsed, and a deck that shows an empty title is a track whose
//!   *container* said nothing, not a lyric file that failed.
//!
//! Nothing here knows about the screen: which line to draw, and how far to have scrolled it,
//! is the panel's business. This is only "what words belong to what second".

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::playlist::decode;

/// Where lyrics live on a card, relative to the music root.
pub const DIR: &str = "Lyrics";

/// One line of words, and the second it starts at.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub t: f32,
    pub text: String,
}

/// The card's lyric files, found once so that looking one up later costs nothing.
///
/// **This is the game scan's shape, deliberately** (`slot_store::scan::label_for`): walk the
/// directory once into a set of keys, then answer "is this key there?" per question. The first
/// version guessed a path per track and asked the filesystem each time, which on a card of forty
/// tracks is forty failed lookups per deck visit — and, worse, a rule that could not be *listed*,
/// so there was nowhere to say what the card was expected to look like.
///
/// The keys are the file's path under `Music/Lyrics/` without its extension, in both the forms
/// the game's own rule allows: `歌名` for a flat card and `专辑/歌名` for a sorted one.
#[derive(Clone, Debug, Default)]
pub struct Index {
    keys: HashSet<String>,
    /// The `Music/` the keys are relative to. **Held rather than passed to every read** because
    /// a `PathBuf` is what `std::fs::read` wants and a relative key is what makes the index
    /// cheap to build — the two only meet here, so this is the one place that has to know it.
    root: PathBuf,
}

impl Index {
    /// Read `Music/Lyrics/` and remember what is in it. A card with no such folder gives an
    /// empty index, which is the ordinary case and not a failure.
    ///
    /// **Walked once, at the same moment the discs are walked**, because both are answers to
    /// "what is on this card" and a card that grows a lyric file mid-session is as rare as one
    /// that grows a track mid-session.
    ///
    /// `music_root` is the card's `Music/`, not the card: the walk joins `Lyrics/` onto it, and
    /// `find` hands back paths that are already absolute as far as the caller is concerned.
    pub fn scan(music_root: &Path) -> Index {
        let mut keys = HashSet::new();
        let dir = music_root.join(DIR);
        walk_indexed(&dir, &dir, 0, &mut keys, &["lrc"], false);
        Index {
            keys,
            root: dir,
        }
    }

/// The lyric file for `track`, or `None` when the card does not carry one.
///
/// **先找歌曲自己所在的那个文件夹，再走索引。**（六六 2026-10-06：「以碟片文件夹路径为优先级，
/// 用户整张数码专辑放进来，自动联动里面的歌曲同名 lrc」。）
///
/// 一个从数码专辑整张拷进来的目录长这样：
///
/// ```text
/// Music/迈克尔·杰克逊/Thriller/01 - Beat It.mp3
///                              01 - Beat It.lrc
///                              cover.jpg
/// ```
///
/// 索引只扫 `Music/Lyrics/`，**看不到这些 `.lrc`** —— 它们不在那个文件夹里。所以这一档
/// 先在歌曲自己的目录里按同名找一遍，找到了就用它。
///
/// **为什么这一档在前而不在后**：整张专辑文件夹是**用户放下来的形状**，索引是给
/// 「一堆散着的歌 + 一个 Lyrics 目录」那种卡准备的。两者撞名时，先信用户手上那个目录 ——
/// 那是他刚拖进来的东西，而索引里那份可能是上一张卡留下的。
///
/// 代价是**一次 `read_dir`**，而且只在问这一首歌的时候发生（不是每帧）：
/// `deck_lyric` 只在切轨时调一次 `find`，不是每帧问。
pub fn find(&self, track: &crate::Track) -> Option<PathBuf> {
    if let Some(p) = sibling_lrc(track) {
        return Some(p);
    }
    keys_for(track)
        .into_iter()
        .find(|key| self.keys.contains(key))
        .map(|key| self.root.join(format!("{key}.lrc")))
}

    /// The lyrics for `track`. `None` when the card has none, or when the file it has holds no
    /// timed lines at all — a lyric file of nothing but headers is a file that would otherwise
    /// show an empty panel for the whole track.
    pub fn load(&self, track: &crate::Track) -> Option<Vec<Line>> {
        let bytes = std::fs::read(self.find(track)?).ok()?;
        let lines = parse(&decode(&bytes));
        (!lines.is_empty()).then_some(lines)
    }

    /// How many lyric files the card carries, for the startup log. Counts **files**, not
    /// tracks: a card with three songs and one lyric file is a card with one lyric file.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// 歌曲**自己那个目录**里的同名 `.lrc`。找不到就是 `None`。
///
/// **只认同名，不认「专辑里唯一那个 `.lrc`」**：后者会在歌名对不上时静默把别的歌的词挂上来，
/// 而那种错在屏上表现为"歌词和唱歌对不上"，是查不出来的那种错。
/// 同名的候选就是 `stems_for` 那几种写法（去括号限定、去艺人前缀），与索引那一档同源。
fn sibling_lrc(track: &crate::Track) -> Option<PathBuf> {
    let dir = track.path.parent()?;
    let entries = std::fs::read_dir(dir).ok()?;
    let mut found: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("lrc"))
        {
            continue;
        }
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        if stems_for(&track.stem)
            .iter()
            .any(|want| want.eq_ignore_ascii_case(&stem))
        {
            found.push(path);
        }
    }
    // **多于一个匹配时不猜。** `01 - Beat It.lrc` 与 `Beat It.lrc` 同时在手时，
    // 挑错的那份歌词不会报错，只会让屏上的字和耳里的歌慢慢错开 —— 宁可这一首没有歌词。
    match found.len() {
        1 => found.pop(),
        _ => None,
    }
}

/// The keys a track could be filed under, most specific first.
///
/// **The stem is tried three ways** because a card's music and its lyrics are written by hand
/// and the two do not always agree on the name: the same song is `CrazySix - 算(Original).mp3`
/// on one side and `算.lrc` on the other. Rather than demand the user rename things, the lookup
/// drops the artist prefix and the bracketed qualifier and takes what is left — which is the
/// part both spellings share.
fn keys_for(track: &crate::Track) -> Vec<String> {
    keys_of(&track.stem, &track.album)
}

/// The candidate keys for one item's name, most specific first.
///
/// **Shared with the cover index** (`crate::Covers`), because a card that spells its lyrics
/// `算.lrc` spells its cover `算.png` — the tolerance is one decision about how a card is filled
/// in, and making it twice is how the two drift apart. `album` is the album folder for a track
/// and the disc's own name for an album, which is the same string in both cases.
pub(crate) fn keys_of(stem: &str, album: &str) -> Vec<String> {
    let mut out = Vec::new();
    for stem in stems_for(stem) {
        let album = album.trim();
        if !album.is_empty() {
            out.push(format!("{album}/{stem}"));
        }
        out.push(stem);
    }
    out
}

/// The spellings of one file stem that could name its lyric file, in the order to try them.
///
/// **The two edits compose, and that is the whole point.** A real file is
/// `CrazySix - 算(Original).mp3`: dropping the bracketed qualifier leaves `CrazySix - 算`, and
/// only *then* does the artist prefix come off it to leave `算`. Applying each edit once to the
/// original stem would produce `CrazySix - 算` and `算(Original)` and never `算`, which is the
/// one spelling the card actually uses — so each round's output feeds the next.
fn stems_for(stem: &str) -> Vec<String> {
    // One round: drop a trailing qualifier, and drop an artist prefix. Both may fire, and both
    // may fire twice — `Artist - Title (Remaster) (2011)` is not a contrived name.
    let trim_qualifier = |s: &str| -> Option<String> {
        ['(', '[', '（']
            .iter()
            .filter_map(|open| s.find(*open).map(|i| s[..i].trim().to_string()))
            .filter(|head| !head.is_empty())
            .min_by_key(|head| head.len())
    };
    let drop_artist = |s: &str| -> Option<String> {
        s.split_once(" - ")
            .map(|(_, rest)| rest.trim().to_string())
            .filter(|rest| !rest.is_empty())
    };

    let mut out = vec![stem.to_string()];
    let mut frontier = vec![stem.to_string()];
    for _ in 0..3 {
        let mut next = Vec::new();
        for s in &frontier {
            for edit in [trim_qualifier, drop_artist] {
                if let Some(e) = edit(s) {
                    if !out.iter().any(|seen| *seen == e) {
                        out.push(e.clone());
                        next.push(e);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    out
}

/// Depth-limited so a symlink loop cannot hang a boot, and so the key is relative to the folder
/// it was given — which is what makes `find`'s two candidate forms possible.
///
/// **`exts` rather than a hardcoded `lrc`**, because the cover index walks a sibling folder with
/// the same shape and different extensions. One walk, two callers, and no second copy of the
/// depth limit and the separator-normalising that keeps a Windows walk comparable with a
/// composed key.
///
/// **`keep_ext`** is the other difference between the two callers, and it is not cosmetic:
/// lyrics are only ever `.lrc`, while a cover may be png / jpg / jpeg / webp — and a key with
/// the extension dropped can only be turned back into a path by **guessing** which one it was.
/// Guessing `.png` at a card whose cover is `.jpg` gives a path to a file that is not there:
/// no picture loads, and the screen shows a blank sleeve with nothing saying why. See
/// `crate::Covers`.
pub(crate) fn walk_indexed(
    dir: &Path,
    base: &Path,
    depth: usize,
    keys: &mut HashSet<String>,
    exts: &[&str],
    keep_ext: bool,
) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // The same two the music walk skips: the volume's own folder, and anything hidden.
        if name.starts_with('.') || name == "System Volume Information" {
            continue;
        }
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk_indexed(&path, base, depth + 1, keys, exts, keep_ext),
            Ok(t) if t.is_file() => {
                let wanted = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| exts.iter().any(|w| e.eq_ignore_ascii_case(w)));
                if wanted {
                    if let Some(key) = rel_key(base, &path, keep_ext) {
                        keys.insert(key);
                    }
                }
            }
            _ => {}
        }
    }
}

/// 扫出来的键：**相对 `base` 的路径，分隔符统一成 `/`**；`keep_ext` 决定带不带扩展名。
///
/// 分隔符归一化是因为这个键是**当纯文本比**的（与 `keys_of` 拼出来的那个比），而 Windows 上
/// `strip_prefix` 给出的是 `\` —— 分隔符不同会让每一次查找都 miss 而不是 fail。
pub(crate) fn rel_key(base: &Path, path: &Path, keep_ext: bool) -> Option<String> {
    let rel = path.strip_prefix(base).ok()?;
    Some(if keep_ext {
        rel.to_string_lossy().replace('\\', "/")
    } else {
        rel.with_extension("").to_string_lossy().replace('\\', "/")
    })
}

/// The lyric file sitting beside `track` in its own folder: the same path with `.lrc` on the end.
///
/// **Kept because a card may be laid out that way**, and because it needs no index — a caller
/// that has one track and no card root can still ask. `Index` is what a deck uses.
///
/// The extension is replaced rather than appended, so `song.mp3` looks for `song.lrc` and not
/// `song.mp3.lrc`. A file with no extension at all is left alone, which makes it look for a file
/// that does not exist — the honest answer, since there is no way to guess what it would have
/// been called.
pub fn beside(track: &Path) -> PathBuf {
    match track.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let mut p = track.to_path_buf();
            p.set_extension("lrc");
            // `set_extension` on a stem that already ends in a dot-like extension can produce
            // something longer than the original; the extension is only recorded for the debug
            // line below and is otherwise unused, which is why this does not have to be exact.
            let _ = ext;
            p
        }
        None => track.with_extension("lrc"),
    }
}

/// Read and parse the lyrics beside `track`, with no index. See `Index::load` for what `None`
/// means; this is the same answer for a caller that has only a path.
pub fn load(track: &Path) -> Option<Vec<Line>> {
    let bytes = std::fs::read(&beside(track)).ok()?;
    let lines = parse(&decode(&bytes));
    (!lines.is_empty()).then_some(lines)
}

/// Parse the text of an `.lrc` into timed lines, in time order.
pub fn parse(text: &str) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if !line.starts_with('[') {
            continue;
        }
        // Every `[..]` at the head, in order. A line may carry several — a chorus repeated
        // later in the song is written once with two times on it, and dropping the second
        // would leave the panel blank the second time round.
        let mut times: Vec<f32> = Vec::new();
        let mut rest = line;
        while let Some(end) = rest.find(']') {
            let tag = &rest[1..end];
            if let Some(t) = tag_time(tag) {
                times.push(t);
                rest = rest[end + 1..].trim_start();
            } else {
                // A header, or anything else that is not a time: stop reading tags. The words
                // start after it.
                rest = rest[end + 1..].trim_start();
                break;
            }
        }
        let words = rest.trim();
        if times.is_empty() || words.is_empty() {
            continue;
        }
        for t in times {
            out.push(Line {
                t,
                text: words.to_string(),
            });
        }
    }
    // Sorted here rather than trusted: the files are in order in practice, but a lyric edited
    // by hand is not guaranteed to be, and a panel that walks the list linearly would then
    // stop at the first out-of-place line for the rest of the song.
    out.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Seconds from `[mm:ss]` or `[mm:ss.xx]`, where `xx` may be any number of digits.
///
/// `None` for anything else, which is what makes `[ti:]` a header and not a time: the first
/// field is not digits, so this returns before it is ever read as a minute.
fn tag_time(tag: &str) -> Option<f32> {
    let (mm, rest) = tag.split_once(':')?;
    if mm.is_empty() || !mm.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let minutes: f32 = mm.parse().ok()?;
    let (ss, frac) = match rest.split_once('.') {
        Some((s, f)) => (s, Some(f)),
        None => (rest, None),
    };
    if ss.is_empty() || !ss.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let seconds: f32 = ss.parse().ok()?;
    // Hundredths in the wild, thousandths sometimes. The divisor comes from how many digits
    // were written rather than being fixed at 100, so `[00:01.5]` is a half second and not
    // five hundredths of one.
    let frac = match frac {
        Some(f) if !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()) => {
            let n: f32 = f.parse().ok()?;
            n / 10f32.powi(f.len() as i32)
        }
        _ => 0.0,
    };
    Some(minutes * 60.0 + seconds + frac)
}

/// Which line is current at `secs`: the last one that has started.
///
/// `None` before the first line, which is the usual case at the head of a song — an intro with
/// no words is not a line of empty text, and a panel handed `None` can show the title instead
/// of a blank row.
pub fn index_at(lines: &[Line], secs: f32) -> Option<usize> {
    let mut found = None;
    for (i, l) in lines.iter().enumerate() {
        if l.t <= secs {
            found = Some(i);
        } else {
            break;
        }
    }
    found
}

/// The words current at `secs`.
pub fn at(lines: &[Line], secs: f32) -> Option<&str> {
    index_at(lines, secs).map(|i| lines[i].text.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_tag_becomes_seconds() {
        assert_eq!(tag_time("00:10.96"), Some(10.96));
        assert_eq!(tag_time("01:00"), Some(60.0));
        assert_eq!(tag_time("02:03.5"), Some(123.5));
    }

    #[test]
    fn headers_are_not_times() {
        // The three that are empty in every file this was written against, plus the one that
        // is not.
        for tag in ["ti:", "ar:", "al:", "by:九九Lrc歌词网～www.99Lrc.com", "offset:0"] {
            assert_eq!(tag_time(tag), None, "{tag} should not read as a time");
        }
    }

    #[test]
    fn a_header_line_is_skipped_but_the_song_still_parses() {
        let lines = parse("[ti:]\n[ar:]\n[al:]\n[by:x]\n[00:10.96]情 绪 小 集\n[00:13.14]2 0 2 6\n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "情 绪 小 集");
        assert_eq!(lines[1].t, 13.14);
    }

    #[test]
    fn several_tags_on_one_line_become_several_lines() {
        let lines = parse("[01:10.41][02:18.94]倘若时间能冲淡过去\n");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].t, 70.41);
        assert_eq!(lines[1].t, 138.94);
        assert_eq!(lines[0].text, lines[1].text);
    }

    #[test]
    fn out_of_order_lines_come_out_sorted() {
        let lines = parse("[00:20.00]second\n[00:10.00]first\n");
        assert_eq!(lines[0].text, "first");
        assert_eq!(lines[1].text, "second");
    }

    #[test]
    fn a_gbk_file_decodes() {
        // "音" in GBK is D2 F4 — the pair the project's own encoding note warns about.
        let mut bytes = b"[00:01.00]".to_vec();
        bytes.extend_from_slice(&[0xD2, 0xF4]);
        let text = decode(&bytes);
        let lines = parse(&text);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "音");
    }

    #[test]
    fn index_at_is_none_before_the_first_line() {
        let lines = parse("[00:24.26]YAYA\n[00:28.44]WE MADE IN POELAND\n");
        assert_eq!(index_at(&lines, 0.0), None);
        assert_eq!(index_at(&lines, 24.26), Some(0));
        assert_eq!(index_at(&lines, 27.0), Some(0));
        assert_eq!(index_at(&lines, 28.44), Some(1));
        assert_eq!(at(&lines, 100.0), Some("WE MADE IN POELAND"));
    }

    #[test]
    fn beside_replaces_the_extension() {
        assert_eq!(beside(Path::new("Music/a/song.mp3")), PathBuf::from("Music/a/song.lrc"));
        assert_eq!(beside(Path::new("Music/a/song.flac")), PathBuf::from("Music/a/song.lrc"));
    }

    #[test]
    fn nothing_timed_means_nothing() {
        let lines = parse("[ti:]\n[ar:]\nno tags here\n");
        assert!(lines.is_empty());
    }

    // ── The index: the card's `Music/Lyrics/` ────────────────────────────────────────────

    /// A card laid out the way the game's own scan expects: albums in folders, words beside them
    /// in the same shape under `Lyrics/`. The paths given are **relative to `Music/Lyrics/`**,
    /// which is where the index looks and where a user files them.
    fn card(lyrics: &[&str]) -> tempfile::TempDir {
        let d = tempfile::tempdir().expect("tempdir");
        for f in lyrics {
            let p = d.path().join("Music").join(DIR).join(f);
            std::fs::create_dir_all(p.parent().unwrap()).expect("mkdir");
            std::fs::write(&p, b"[00:01.00]hello\n").expect("write");
        }
        d
    }

    fn track(stem: &str, album: &str) -> crate::Track {
        crate::Track {
            stem: stem.to_string(),
            path: PathBuf::from(format!("Music/{album}/{stem}.mp3")),
            title: stem.to_string(),
            artist: String::new(),
            album: album.to_string(),
            duration: 0.0,
            initial: 'x',
            track_number: None,
            // ★ 2026-10-06：这个测试辅助漏了 `tags`，而 `Track` 加 `tags` 的时候
            //   **只有 lib 与 examples 编得过**（它们不走 `#[cfg(test)]` 里的这段），
            //   所以 `--all-targets` 才把它露出来。测试辅助与结构体字段不同步的典型。
            tags: Vec::new(),
        }
    }

    #[test]
    fn an_album_folder_is_matched_before_the_flat_form() {
        let d = card(&["情绪小集/定错的闹钟.lrc", "定错的闹钟.lrc"]);
        let idx = Index::scan(&d.path().join("Music"));
        let found = idx.find(&track("定错的闹钟", "情绪小集")).expect("found");
        assert!(
            found.to_string_lossy().contains("情绪小集"),
            "专辑优先：{found:?}"
        );
    }

    /// The naming a real card does not agree on: the music says `CrazySix - 算(Original).mp3`
    /// and the lyric says `算.lrc`. **Both halves of the disagreement are handled** — the
    /// artist prefix and the bracketed qualifier — and this is the test that says so, because
    /// it is the case that actually came up.
    #[test]
    fn a_lyric_filed_under_the_bare_title_is_still_found() {
        let d = card(&["算.lrc"]);
        let idx = Index::scan(&d.path().join("Music"));
        let t = track("CrazySix - 算(Original)", "CrazySix");
        assert_eq!(
            idx.find(&t).map(|p| p.file_name().unwrap().to_string_lossy().into_owned()),
            Some("算.lrc".to_string()),
            "艺术家前缀与 (Original) 后缀都应被剥掉再匹配"
        );
    }

    #[test]
    fn a_renamed_card_is_not_matched_by_guessing() {
        let d = card(&["Completely Different.lrc"]);
        let idx = Index::scan(&d.path().join("Music"));
        assert!(
            idx.find(&track("算", "情绪小集")).is_none(),
            "对不上就是没有歌词，不该按相似度猜一个"
        );
    }

    #[test]
    fn a_card_with_no_lyrics_folder_is_an_empty_index_not_a_failure() {
        // Music with no `Lyrics/` beside it at all — the folder is never created, which is what
        // "a card with no lyrics" actually looks like on disk.
        let d = tempfile::tempdir().expect("tempdir");
        let album = d.path().join("Music").join("情绪小集");
        std::fs::create_dir_all(&album).expect("mkdir");
        std::fs::write(album.join("算.mp3"), b"not really audio").expect("write");
        let idx = Index::scan(&d.path().join("Music"));
        assert!(idx.is_empty());
        assert!(idx.load(&track("算", "情绪小集")).is_none());
    }

    /// A lyric file of nothing but headers must not become an empty panel for the whole track.
    #[test]
    fn headers_alone_are_not_lyrics() {
        let d = tempfile::tempdir().expect("tempdir");
        let dir = d.path().join("Music").join(DIR);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("算.lrc"), b"[ti:]\n[ar:]\n").expect("write");
        let idx = Index::scan(&d.path().join("Music"));
        assert!(idx.load(&track("算", "")).is_none());
    }

    /// Depth is bounded so a symlink loop cannot hang a boot. A chain deeper than the bound
    /// gives an index with nothing in it rather than a walk that never returns.
    #[test]
    fn the_walk_is_depth_limited() {
        let d = tempfile::tempdir().expect("tempdir");
        let mut p = d.path().join("Music").join(DIR);
        for _ in 0..12 {
            p = p.join("d");
        }
        std::fs::create_dir_all(&p).expect("mkdir");
        std::fs::write(p.join("deep.lrc"), b"[00:01.00]x\n").expect("write");
        let idx = Index::scan(&d.path().join("Music"));
        assert!(idx.is_empty(), "超过深度上限的文件不进索引");
    }
}
