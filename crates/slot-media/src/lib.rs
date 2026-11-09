//! Music on the card.
//!
//! What is in `Music/`, and what each file says about itself. Decoding lives here too, behind
//! the same crate, because a card's music is one subject: the scan has to open a file to learn
//! how long it is, and the player has to open it again to hear it.
//!
//! Nothing here knows about the screen. The shelf, the deck and the scope are `slot-ui`'s; this
//! is only what is on the card.

use std::path::{Path, PathBuf};

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::units::Time;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, RawValue, StandardTag};

pub mod playlist;
pub mod lrc;

/// Where the card keeps its music, spelled as this crate spells it. The same convention as
/// `Games/`: a folder the user fills from the outside, walked to any depth.
pub const DIR: &str = "Music";

/// Where the album art sits, under `Music/`. **Beside the music rather than inside it**, for the
/// same reason `Lyrics/` is: a card's music is what the user rips and a card's artwork is what
/// they look up, and mixing the two in one folder means a rename of either loses the other.
pub const COVERS: &str = "Covers";

/// `Music/Covers/` 里那枚**盒面模板**的文件名 —— 它和封面住一处，但不是封面。
///
/// 2026-10-28 六六把"带洋红区的光盘盒"从 `System/Carts/cd.png` 挪到 `Music/Covers/cd.png`，
/// 于是这个目录第一次同时装两种东西。文件名与 `slot-ui::cd_art::FILE` 是同一个约定；
/// 两处都在注释里指了对方，改一边要改另一边。
const CASE_TEMPLATE: &str = "cd.png";

/// The extensions a track may carry.
///
/// An extension is the only thing that decides, for the same reason it is for a cart: it is
/// free, it needs no header read, and a card's music is sorted by hand. `m4a` and `mp4` are the
/// same container with different names, and both are here because both are what a ripper writes.
const EXTENSIONS: [&str; 9] = [
    "mp3", "mpa", "flac", "ogg", "oga", "m4a", "mp4", "aac", "wav",
];

/// One file in `Music/`.
#[derive(Clone, Debug)]
pub struct Track {
    /// Filename stem — the key, exactly as a cart's is. Labels, favourites and the resume
    /// pointer are all filed under it.
    pub stem: String,
    pub path: PathBuf,
    /// What the file says it is called. The stem, cleaned, when it says nothing.
    pub title: String,
    pub artist: String,
    pub album: String,
    /// The track's number within its album, straight from the file's own tag (`TRCK` and its
    /// kin across containers). `None` when the file does not carry one, in which case disc
    /// ordering falls back to the file name's leading digits and then the title — the same
    /// ladder `ensure_in` writes into `list.m3u`, so a dropped-in album plays in order.
    pub track_number: Option<u32>,
    /// Seconds, or 0.0 when the container would not say.
    pub duration: f32,
    /// The shelf's letter bucket, off the title the shelf shows.
    pub initial: char,
    /// **容器里剩下的标签项，原样留住**（键小写、值去首尾空白）。
    ///
    /// ★ 2026-10-06 六六要信息页「当前曲目其它标签项**有就显示、没有就隐藏**」——
    ///   而 `title`/`artist`/`album` 是**抽出来单列**的三项，其余（年份、专辑号、碟盘号、
    ///   流派、注释…）以前在 `open()` 解析完就丢了。留着它们才能"有就显示"；
    ///   **这也是"有就显示"的前提**：不留，显示的就是那三项固定的空表。
    pub tags: Vec<(String, String)>,
}

impl Track {
    /// Whether this file is a track at all: one of the extensions above.
    pub fn is_track(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
    }

    /// `mm:ss`, for the list. Hours are dropped rather than shown as `0:`, because nothing on a
    /// card is long enough for the hour to be the interesting part and a leading `0:` is noise
    /// on every row.
    pub fn clock(&self) -> String {
        let s = self.duration.max(0.0) as u32;
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// One disc: what the deck plays, and what the shelf holds a cassette of.
///
/// **A disc is an album, or it is a single loose track** — the two cases the card actually
/// contains. A folder carrying a playlist is collected into one disc whose face is the album
/// (see `playlist` for why), and a folder without one leaves its tracks loose, each its own disc.
/// The deck does not care which it is handed; it plays a list of tracks either way, and this is
/// what lets one screen carry both a single song and an eleven-song album without a second code
/// path.
#[derive(Clone, Debug)]
pub struct Disc {
    /// The name on the disc's face: the album for a playlist folder, the track's own title for a
    /// loose one.
    pub name: String,
    /// The key a sticker is filed under — `Labels/<stem>.png`, the same rule a cart's label uses.
    ///
    /// **The folder's own name, not the album's.** An album folder called `光明之魂` holding
    /// `光明之魂1.mp3` and `光明之魂2.mp3` is one disc whose label is one picture, so the sticker
    /// has to be found by something both tracks agree on — and the folder is it. A loose track
    /// keys on its own stem, which is the degenerate case of the same rule.
    pub stem: String,
    /// The tracks, in play order. Never empty — an empty disc is not made.
    pub tracks: Vec<Track>,
    /// Seconds, or 0.0 when nothing on the disc would say. The sum of what the tracks say.
    pub duration: f32,
    /// The shelf's letter bucket, off the name.
    pub initial: char,
}

impl Disc {
    /// One loose track, as a disc of its own. The single-track case of the same thing.
    pub fn single(track: Track) -> Self {
        let name = track.title.clone();
        let stem = track.stem.clone();
        let duration = track.duration;
        let initial = track.initial;
        Disc {
            name,
            stem,
            tracks: vec![track],
            duration,
            initial,
        }
    }

    /// `mm:ss` of the whole disc, for the shelf's line under the name.
    pub fn clock(&self) -> String {
        let s = self.duration.max(0.0) as u32;
        format!("{}:{:02}", s / 60, s % 60)
    }

    /// Which disc the deck would hold after a step, over a list `len` long.
    ///
    /// **Not a ring**, which is the decision this function exists to hold: a transport key
    /// cycles because there are three of them and a hand should never fall off the end, but a
    /// disc list is the card's own contents — wrapping from the last album to the first would
    /// claim the shelf continues when it does not. Both ends stop instead. An empty list stays
    /// at zero rather than wrapping into itself, which is why the length is a parameter and not
    /// read off a slice.
    ///
    /// It lives here, beside the type it moves a cursor through, rather than in the app beside
    /// the keys that call it: this is arithmetic, it is the half of the deck most likely to be
    /// off by one, and from here a test on the host can hold it.
    pub fn step(disc: usize, len: usize, dir: isize) -> usize {
        if len == 0 {
            return 0;
        }
        (disc as isize + dir).clamp(0, len as isize - 1) as usize
    }
}

/// Open `path` far enough to read its container, its tags and its length.
///
/// The `Hint` is the file's own extension, which is what lets a bare `.mp3` with no ID3 header
/// be recognised at all: the same free metadata the extension is everywhere else here.
fn open(path: &Path) -> Option<Box<dyn FormatReader>> {
    let file = std::fs::File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .ok()
}

/// Undo a Latin-1 mis-reading of GBK text.
///
/// # The shape of the failure, and why it is exactly reversible
///
/// A tag whose **bytes** are GBK but which a reader hands back as ISO-8859-1 comes out as one
/// `char` per byte, every one of them sitting in `U+0080..=U+00FF`:
///
/// * `周杰伦` is `D6 DC BD DC C2 D7` in GBK, and arrives as `ÖÜ½ÜÂ×`;
/// * `可爱女人` is `BF C9 B0 AE C5 AE C8 CB`, and arrives as `¿É°®Å®ÈË`.
///
/// Both were photographed off the device's panel on 2026-10-07 — the deck's title read
/// `ÖÜ½ÜÂ× - ¿É°®Å®ÈË`, which is this function's whole reason to exist. The characters *are* the
/// bytes, so the bytes are the characters, and GBK puts them back. Nothing is guessed.
///
/// **Which tag it came from is not this function's business.** These files carry an ID3v2.3 tag
/// in UTF-16 *and* an ID3v1 tag whose 128 bytes are GBK — the standard says ISO-8859-1 and the
/// encoder lied — and which one a reader answers with is the reader's problem. Repairing the
/// result makes either answer right.
///
/// # Two guards, because a repair that fires on good text is worse than the bug
///
/// * **Every character must fit in one byte, and the bytes must spell a well-formed GBK
///   sequence** (see [`gbk_shaped`]). Nothing above `U+00FF` may appear — that is real Unicode,
///   not a mis-reading — and a byte string that will not parse as GBK code units is ordinary
///   text.
///
///   ⚠️ **This guard used to demand "no ASCII at all", and that was wrong.** A GBK *trail* byte
///   is allowed anywhere in `0x40..=0xFE`, so `半島鐵盒` (`b0 eb 8d 75 e8 46 ba d0`) arrives
///   carrying a literal `u` and `F`; the old guard threw it away as ordinary text, and the deck
///   showed `°ë uèF ºÐ` for it — 六六 photographed exactly that on 2026-10-07. What actually
///   protects `Björk` is the *count* guard below, not this one.
/// * **The result must contain a Han character, the input must not, and there must be more than
///   one of them.** GBK bytes are a prefix code, so Latin-1 text can decode as GBK without error
///   and come out as something that is not Chinese at all: `ÃÂ` is two bytes, pairs up quite
///   happily, and lands on a real Han character — one of them. Asking for **two** is what tells a
///   repair from a coincidence, and the cost is that a title of exactly one Chinese character
///   (two mojibake characters) is left as it is rather than repaired. That is the trade taken
///   deliberately: a one-character title is rare, and corrupting a real Latin-1 tag is not
///   recoverable by the user.
fn repair(s: &str) -> String {
    if s.is_empty() || s.is_ascii() {
        return s.to_string();
    }
    let mut bytes = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let c = ch as u32;
        if c > 0xFF {
            return s.to_string();
        }
        bytes.push(c as u8);
    }
    if !gbk_shaped(&bytes) {
        return s.to_string();
    }
    let (out, bad) = encoding_rs::GBK.decode_without_bom_handling(&bytes);
    if bad {
        return s.to_string();
    }
    let out = out.into_owned();
    if out.chars().filter(|c| han_char(*c)).count() >= 2 && !han(s) {
        // The device has no console, so this is where a repaired tag is visible — and it is
        // worth one line per card, because the alternative is a screen showing mojibake and
        // nothing anywhere saying why.
        eprintln!("slot: media: re-decoded a GBK tag: {out}");
        out
    } else {
        s.to_string()
    }
}

/// Whether `bytes` reads as a sequence of GBK code units: a lone ASCII byte, or a lead byte in
/// `0x81..=0xFE` followed by a trail byte in `0x40..=0xFE` (never `0x7F`).
///
/// This is what the first guard really wants to ask. It replaces an older "no ASCII anywhere"
/// test, which was too strict — see [`repair`] for the `半島鐵盒` case that exposed it.
///
/// ⚠️ Note what this does **not** claim: `Björk` is perfectly GBK-shaped (`F6 72` is a legal
/// pair). Telling that from a real repair is the *count* guard's job, not this one's.
fn gbk_shaped(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            i += 1;
        } else if (0x81..=0xFE).contains(&b) && i + 1 < bytes.len() {
            let trail = bytes[i + 1];
            if !(0x40..=0xFE).contains(&trail) || trail == 0x7F {
                return false;
            }
            i += 2;
        } else {
            return false;
        }
    }
    true
}

/// Whether `t` holds a Han ideograph. See `repair` for what this is asked and why.
fn han(t: &str) -> bool {
    t.chars().any(han_char)
}

/// One character's half of `han`, so `repair` can *count* them rather than only ask whether
/// there is one at all — see the second guard.
fn han_char(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x9FFF | 0xF900..=0xFAFF)
}

/// Read one file. `None` when the container would not open — a file with the right extension
/// and nothing behind it is not a track, and a card is allowed to hold one.
pub fn read(path: &Path) -> Option<Track> {
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let mut format = open(path)?;

    // Length, from the first track that admits to being audio: its frame count against its own
    // sample rate, which is the one pair every container that carries a count answers. A file
    // that will not say leaves it at zero — the row then shows no time rather than a wrong one.
    let duration = format
        .tracks()
        .iter()
        .find_map(|t| {
            let rate = match &t.codec_params {
                Some(CodecParameters::Audio(a)) => a.sample_rate,
                _ => None,
            };
            match (t.num_frames, rate) {
                (Some(frames), Some(rate)) if rate > 0 => Some(frames as f32 / rate as f32),
                _ => None,
            }
        })
        .unwrap_or(0.0);

    // Tags. The standard ones only: a raw key is whatever the encoder felt like writing, and
    // the three that matter here have standard names.
    let (mut title, mut artist, mut album) = (String::new(), String::new(), String::new());
    let mut track_number: Option<u32> = None;
    // 抽三样的同时**把其余标签留下来**（见 `Track::tags`）。判重按"键小写"，
    // 所以 `YEAR` 与 `year` 只留一个，且**保持文件里的先后顺序** ——
    // 信息页要按文件自己写的顺序列，而不是按我们排的字母序。
    let mut tags: Vec<(String, String)> = Vec::new();
    if let Some(m) = format.metadata().current() {
        for tag in &m.media.tags {
            // symphonia 0.6：原始键值在 `tag.raw`（`RawTag { key, value }`），
            // 归一化过的那一份在 `tag.std`。我们要的是"文件自己写的那个"。
            let key = tag.raw.key.trim().to_string();
            // `RawValue` 有 Display（数字/布尔/字符串都走它），二进制那块会显示成字节，
            // 那不是"标签项"该给人看的东西 —— 所以下面还要再挡一次空/二进制。
            let value = tag.raw.value.to_string();
            let value = value.trim().to_string();
            if key.is_empty() || value.is_empty() {
                continue;
            }
            // 三项抽出来单列；**其余一律按"文件自己写的那个键"留下** ——
            // 包括 symphonia 认得但我们没单列的（年份、碟盘号、流派…），因为信息页要显示的
            // 正是"文件自己标了什么"。二进制值不是标签项，不收。
            let mut single = false;
            match tag.std.as_ref() {
                Some(StandardTag::TrackTitle(s)) => {
                    title = s.to_string();
                    single = true;
                }
                Some(StandardTag::TrackNumber(n)) if track_number.is_none() => {
                    // `TRCK` reads as `u64`; real track numbers fit `u32`, and 0 is not a track.
                    if *n > 0 {
                        track_number = Some(*n as u32);
                    }
                    single = true;
                }
                Some(StandardTag::Artist(s)) if artist.is_empty() => {
                    artist = s.to_string();
                    single = true;
                }
                Some(StandardTag::Album(s)) => {
                    album = s.to_string();
                    single = true;
                }
                _ => {}
            }
            if !single
                && !matches!(tag.raw.value, RawValue::Binary(_))
                && !tags.iter().any(|(k, _)| k.eq_ignore_ascii_case(&key))
            {
                // The same mistake as the three above, on the fields that are not singled out.
                // The info page prints these, so they have to be readable too.
                tags.push((key, repair(&value)));
            }
        }
    }

    // A file with no title tag is named by its own file name, cleaned the way a cart's is — the
    // two are the same problem and want the same answer, which is why the rule lives in
    // `slot-store` rather than here.
    // ★ **The three that name the thing are repaired before anything reads them** (2026-10-07,
    //   for the deck's `ÖÜ½ÜÂ× - ¿É°®Å®ÈË`). Before the `"no title"` fallback, so a title that is
    //   nothing but mojibake is repaired into a real one rather than being judged empty first.
    let title = repair(title.trim());
    let artist = repair(artist.trim());
    let album = repair(album.trim());
    let title = match title.as_str() {
        "" => slot_store::clean_label(&stem),
        t => t.to_string(),
    };
    let initial = slot_store::initial(&title);

    Some(Track {
        stem,
        path: path.to_path_buf(),
        title,
        artist,
        album,
        track_number,
        duration,
        initial,
        tags,
    })
}

/// Everything under `root/Music`, at any depth, sorted the way the shelf reads.
///
/// Unreadable entries are skipped rather than fatal, for the same reason the card's games are:
/// one bad file on a user's card must not cost them the other two hundred.
pub fn scan(root: &Path) -> Vec<Track> {
    scan_dir(&root.join(DIR))
}

/// Everything under `dir`, at any depth. `scan` above is this plus the folder name, and the
/// split is what lets a caller point at a folder of music that is not on a card at all — which
/// is how the command-line probe is used, and how a test can read a tree it built itself.
pub fn scan_dir(dir: &Path) -> Vec<Track> {
    let mut out = Vec::new();
    walk(dir, 0, &mut out);
    out.sort_by(|a, b| a.title.cmp(&b.title));
    out
}

/// The discs under `root/Music`: every album with a playlist as one disc, everything else as
/// loose single tracks. Sorted by the name the shelf shows.
pub fn discs(root: &Path) -> Vec<Disc> {
    discs_dir(&root.join(DIR))
}

/// The card's album art, found once so that looking one up costs nothing.
///
/// **The same shape as `lrc::Index`, and deliberately so** — both are "walk a folder into a set
/// of keys, then answer per track", both are walked at the same moment as the discs, and both
/// tolerate a card that spells the music and the art differently. Written once here rather than
/// as a private helper of each, because the spellings they tolerate are the same spellings: a
/// card that files `CrazySix - 算(Original).mp3` beside `算.lrc` files its cover as `算.png` too.
///
/// The keys are the path under `Music/Covers/` **including its extension**, in the two forms
/// `lrc::Index` uses: `歌名` and `专辑/歌名`（加上扩展名）。
///
/// ⚠️ **这一张表带扩展名，歌词那张不带**，而且是有原因的：歌词只有 `.lrc` 一种，
/// 而封面有 png / jpg / jpeg / webp 四种。把扩展名扔掉之后 `find` 只能硬拼一个 `.png` 回去
/// —— 卡上是 `.jpg` 的封面就会得到一条指向不存在文件的路径，**读不出图，而屏上只是一张空白封套，
/// 没有任何地方说为什么**。这一张表因此存全名，匹配时用 `rel_key(..., true)` 与
/// `rel_key(..., false)` 的差值去比。
///
/// **这一层只管 `Music/Covers/`** —— 碟片自己那个文件夹里的封面**不在索引里**
/// （`find` 先去那儿找，见 `in_disc_folder`），因为索引是一次性扫出来的，而碟片的目录
/// 是每首歌各自带的；把两样东西塞进一个 `HashSet` 就得再记一份"这条键是从哪儿来的"。
#[derive(Clone, Debug, Default)]
pub struct Covers {
    keys: std::collections::HashSet<String>,
    root: PathBuf,
}

impl Covers {
    /// Read `Music/Covers/` and remember what is in it. A card with no such folder gives an
    /// empty set, which is the ordinary case: art is looked up, and a sleeve with a blank face
    /// is a sleeve.
    ///
    /// ★ 2026-10-28：**`Music/Covers/` 里还躺着盒面模板 `cd.png`**（六六把这枚"带洋红区的
    /// 光盘盒"从 `System/Carts/` 挪了过来，好让音乐那一摊的东西住一处）。它不是封面 ——
    /// 一张真叫 `cd` 的碟会把它当封面穿在身上，所以这里把它从索引里剔掉。模板自己的路径由
    /// `slot-ui::cd_art` 直接拼，不经过这张表。
    pub fn scan(music_root: &Path) -> Covers {
        let mut keys = std::collections::HashSet::new();
        let dir = music_root.join(COVERS);
        crate::lrc::walk_indexed(&dir, &dir, 0, &mut keys, &["png", "jpg", "jpeg", "webp"], true);
        keys.retain(|k| !k.eq_ignore_ascii_case(CASE_TEMPLATE));
        Covers { keys, root: dir }
    }

    /// The cover file for `disc`, or `None` when the card carries no art for it.
    ///
    /// **先找碟片自己那个文件夹，再走索引** —— 与 `lrc::Index::find` 同一个理由、同一个次序：
    /// 用户整张数码专辑拖进来时，封面就躺在那堆歌旁边，名字通常是 `cover.jpg` / `folder.png` /
    /// `封面.jpg` / 有时干脆和专辑同名。这是**用户放下来的形状**，所以先信它。
    ///
    /// `Music/Covers/` 那一档是给「一堆散着的歌 + 一个专门放图的目录」那种卡准备的，
    /// 它排在后面不是因为次要，而是因为**用户手上那个目录更新鲜**。
    ///
    /// 碟片自己的文件夹 = **第一首曲目的目录**（而不是碟片的 `stem` 反推出来的路径）：
    /// 碟片是「一个文件夹里的若干首歌」这句话的产物，而那若干首的第一首就在那个文件夹里。
    /// 反推路径要重新拼一遍 `Music/<stem>`，而**那个拼法在单曲碟上是错的** —— 单曲碟的
    /// `stem` 是文件名不是目录名。
    pub fn find(&self, disc: &Disc) -> Option<PathBuf> {
        if let Some(p) = in_disc_folder(disc) {
            return Some(p);
        }
        // 表里带扩展名，而候选键不带 ⇒ 逐个文件**去掉扩展名**再比。
        let wanted = crate::lrc::keys_of(&disc.stem, &disc.name);
        self.keys
            .iter()
            .find(|key| wanted.iter().any(|w| stem_of(key) == *w))
            .map(|key| self.root.join(key))
    }

    /// How many cover files the card carries, for the startup log.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The root the keys are relative to, for a caller that walks the folder itself.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// 一个带扩展名的键去掉扩展名。**只去最后一段**，所以 `专辑/歌名.副标题` 这种名字不受影响
/// （`Path::extension` 对它是 `None`，因为 `副标题` 前面那个点不在扩展名的定义里）。
fn stem_of(key: &str) -> String {
    let p = Path::new(key);
    match p.extension() {
        Some(_) => p.with_extension("").to_string_lossy().replace('\\', "/"),
        None => key.to_string(),
    }
}

/// 碟片**自己那个文件夹**里的封面图。找不到就是 `None`。
///
/// 认的文件名，**按这个次序**，第一个命中的就是它：
///
/// 1. `cover` / `folder` / `front` / `album` / `封面` / `专辑`（不分大小写、不管扩展名）
/// 2. **碟片名或 stem 本身**（`Thriller.jpg`）—— 这也是数码专辑最常见的那个名字
/// 3. 该目录里**唯一的**一张图 —— 只在这一步，且**多于一张就不猜**
///
/// 第 3 条是最后一条也是唯一"认内容不认名字"的一条：一个只放了三首歌和一张图的目录，
/// 那张图几乎必然是这张碟的封面；而**两张图时不猜**（可能是曲目图、可能是别处的图），
/// 猜错的后果和歌词猜错一样 —— 屏上是一张别的专辑的封面，而没有任何地方说为什么。
fn in_disc_folder(disc: &Disc) -> Option<PathBuf> {
    const NAMED: [&str; 6] = ["cover", "folder", "front", "album", "封面", "专辑"];
    const EXTS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

    let first = disc.tracks.first()?;
    let dir = first.path.parent()?;
    let mut images: Vec<PathBuf> = Vec::new();
    let mut named: Option<PathBuf> = None;
    let mut by_disc: Option<PathBuf> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXTS.iter().any(|w| e.eq_ignore_ascii_case(w)))
        {
            continue;
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let low = stem.to_lowercase();
        if NAMED.iter().any(|w| *w == low) {
            named.get_or_insert(path.clone());
        }
        // 碟片名与 stem 谁都不为空才比 —— 否则空碟片名会把 `""` 当成一个真命中。
        let is_disc = (!disc.name.is_empty() && disc.name.eq_ignore_ascii_case(&stem))
            || (!disc.stem.is_empty() && disc.stem.eq_ignore_ascii_case(&stem));
        if is_disc {
            by_disc.get_or_insert(path.clone());
        }
        images.push(path);
    }
    named.or(by_disc).or(if images.len() == 1 {
        images.pop()
    } else {
        None
    })
}

/// The discs under `dir`, at any depth. `discs` above is this plus the folder name; the split
/// means a caller can point at a folder that is not on a card at all, which is how a test reads
/// a tree it built itself and how the command-line probe is used.
///
/// A folder that carries a playlist is **not** descended into for loose tracks: the playlist is
/// the folder's own statement of what belongs on the disc, and a song sitting beside it that the
/// playlist does not name is a file the user has not chosen to include rather than one to
/// surface anyway.
pub fn discs_dir(dir: &Path) -> Vec<Disc> {
    let mut out = Vec::new();
    walk_discs(dir, 0, &mut out);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Depth-limited so a symlink loop cannot hang the boot. Eight levels is far past anything a
/// card holds and well short of anything that spins.
fn walk(dir: &Path, depth: usize, out: &mut Vec<Track>) {
    if depth > 8 {
        return;
    }
    // The folder a file sits in is its album.
    //
    // A music library is organised `Music/<album>/<track>`, and the tags a card's files carry
    // are frequently nothing at all — the SUNO rips this was written against write one comment
    // and no title, artist or album between them. The folder is then the only thing that knows,
    // and it is the thing the user named on purpose. The tag wins when there is one, because a
    // tag is a statement about the file and a folder is a statement about where it was put.
    let album = dir
        .file_name()
        .map(|n| slot_store::clean_label(&n.to_string_lossy()))
        .unwrap_or_default();

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // The same two the game scan skips: the volume's own metadata folder, and anything the
        // user hid.
        if name.starts_with('.') || name == "System Volume Information" {
            continue;
        }
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(&path, depth + 1, out),
            Ok(t) if t.is_file() && Track::is_track(&path) => {
                if let Some(mut track) = read(&path) {
                    if track.album.is_empty() {
                        track.album = album.clone();
                    }
                    out.push(track);
                }
            }
            _ => {}
        }
    }
}

/// The disc walk. A folder with a playlist becomes one disc and is not descended; a folder
/// without one contributes its own files as loose discs and is descended for its subfolders,
/// because a library that is `Music/<artist>/<album>/<track>` has albums one level down and
/// nothing marks where they begin except the playlist.
fn walk_discs(dir: &Path, depth: usize, out: &mut Vec<Disc>) {
    if depth > 8 {
        return;
    }
    // The folder's own name, which becomes the album name on a disc and the album tag on a loose
    // track — the same rule `walk` applies, read once here so the two walks cannot drift.
    let album = dir
        .file_name()
        .map(|n| slot_store::clean_label(&n.to_string_lossy()))
        .unwrap_or_default();

    // A playlist makes this folder a disc. Built before anything else so a folder that is an
    // album does not also leak its songs onto the shelf as loose discs.
    if let Some(list) = playlist::find(dir) {
        let paths = playlist::tracks_of(&list);
        if !paths.is_empty() {
            let tracks: Vec<Track> = paths.iter().filter_map(|p| read(p)).collect();
            if !tracks.is_empty() {
                out.push(disc_named(album, tracks));
                return;
            }
        }
        // A playlist that names nothing playable is not a reason to hide the folder: fall
        // through and let the files speak for themselves, which is the truthful answer for a
        // card that holds a stale list beside real music.
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut had_subfolder = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "System Volume Information" {
            continue;
        }
        match entry.file_type() {
            Ok(t) if t.is_dir() => {
                had_subfolder = true;
                walk_discs(&path, depth + 1, out);
            }
            Ok(t) if t.is_file() && Track::is_track(&path) => {
                if let Some(mut track) = read(&path) {
                    if track.album.is_empty() {
                        track.album = album.clone();
                    }
                    out.push(Disc::single(track));
                }
            }
            _ => {}
        }
    }
    // A folder of nothing but subfolders is a shelf, not a disc, and the walk above already
    // put each subfolder's own discs on it. The flag is here so that intent is written down
    // rather than left implicit in the recursion.
    let _ = had_subfolder;
}

/// One disc from a folder's tracks, named by the folder — the album's own name, because that is
/// what a disc's face says.
fn disc_named(album: String, mut tracks: Vec<Track>) -> Disc {
    // The album folder is the sticker's key; see `Disc::stem`.
    let stem = album.clone();
    // A dropped-in album plays in album order: the file's own track number first, then a
    // leading number in the file name (for tagged-but-unnumbered or untagged files), then the
    // title. `None` sorts last (`u32::MAX`) so a numbered disc is never scattered by an
    // untagged file, and the same ladder `ensure_in` writes into `list.m3u`. `sort_by_key` is
    // stable, so equal keys keep the file order they arrived in.
    tracks.sort_by_key(|t| (
        t.track_number.unwrap_or(u32::MAX),
        crate::playlist::leading_number(&t.stem).unwrap_or(u32::MAX),
        t.title.clone(),
    ));
    for t in &mut tracks {
        if t.album.is_empty() {
            t.album = album.clone();
        }
    }
    let duration: f32 = tracks.iter().map(|t| t.duration).sum();
    let name = if album.is_empty() {
        // A playlist at the very root of the music folder has no folder name to be its album.
        // Its own name is then the best thing there is, and an unnamed list is a list of one
        // track more often than not.
        tracks
            .first()
            .map(|t| t.title.clone())
            .unwrap_or_else(|| DIR.to_string())
    } else {
        album
    };
    let initial = slot_store::initial(&name);
    Disc {
        name,
        stem,
        tracks,
        duration,
        initial,
    }
}

/// An open file, ready to be read for samples.
///
/// Samples come out **interleaved stereo `i16`, at the file's own rate**. The device's rate is
/// `slot`'s business, not this crate's: what a card holds and what the codec runs at is one
/// question, and what the panel's audio clock runs at is another, and the two only meet at the
/// resampler.
pub struct Source {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    rate: u32,
    channels: usize,
    /// Reused across packets, so a track that runs for an hour does not allocate an hour of
    /// buffers on the way.
    buf: Vec<i16>,
}

impl Source {
    /// Open `path` for playback. `None` when it will not open, or carries no audio track this
    /// build has a decoder for — which is a file the scanner will still list, because it could
    /// read its container, and the deck then refuses it out loud rather than silently.
    pub fn open(path: &Path) -> Option<Self> {
        let format = open(path)?;
        // Taken by value and the borrow dropped: the reader is wanted mutably from here on.
        let (track_id, params) = {
            let track = format.default_track(TrackType::Audio)?;
            let params = match &track.codec_params {
                Some(CodecParameters::Audio(a)) => a.clone(),
                _ => return None,
            };
            (track.id, params)
        };
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .ok()?;
        let rate = params.sample_rate.unwrap_or(44_100);
        let channels = params.channels.as_ref().map_or(2, |c| c.count()).max(1);
        Some(Source {
            format,
            decoder,
            track_id,
            rate,
            channels,
            buf: Vec::new(),
        })
    }

    /// **把解码位置挪到 `sec` 秒 —— 能真的跳，不是从头解一遍。**
    ///
    /// ★ 2026-10-06 六六给卡座定键时写了「← 快退 · → 快进」，而这一条是它的前提：
    ///   在此之前**整个播放器没有任何定位能力**（`Cmd` 只有 `Play`/`Pause`/`Stop`），
    ///   于是"快退"只能是重新 open 整个文件 = 回到开头，那不叫快退。
    ///
    /// 底座是 symphonia 的 `MediaSourceStream`：有索引表的容器（FLAC / WAV / MP4…）按表直接定位，
    /// 纯流式的（部分 MP3）会退化成"从头解到那里"—— 慢，但结果一样。
    /// `false` = 这个文件不支持定位（**调用方要当什么也没发生**，别改播放态）。
    pub fn seek_to(&mut self, sec: f32) -> bool {
        // ★ symphonia 0.6 的 seek 形状：**`FormatReader::seek(mode, to)` 负责跳，
        //   `AudioDecoder` 只有 `reset()`、没有 `seek()`** —— 容器跳到哪，解码器就从那重新起步。
        //   （这也是 0.6 与 0.5 的差别；按 0.5 的写法写会编译不过。）
        let nanos = (sec.max(0.0) as f64 * 1_000_000_000.0) as i128;
        let Some(time) = Time::try_from_nanos_i128(nanos) else {
            return false;
        };
        let to = SeekTo::Time {
            time,
            track_id: Some(self.track_id),
        };
        // `Accurate`：宁可落在目标之前一点，也别落在之后 —— 落在之后会漏掉一截，
        // 而"漏掉一截"在跳上一首时会听成"少唱了一句"。
        match self.format.seek(SeekMode::Accurate, to) {
            Ok(_) => {
                self.decoder.reset();
                true
            }
            Err(_) => false,
        }
    }

    /// The file's own rate in Hz. Read off the first decoded packet, so it is what the decoder
    /// actually produces rather than what the header promised.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// How many channels the file has, before the stereo fold below.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Decode the next packet. `None` at the end of the file.
    ///
    /// Packets that belong to another track, that will not decode, or that carry no frames are
    /// skipped rather than reported: a damaged frame in the middle of a track is a click, and
    /// stopping the whole track over it would be a much worse answer.
    pub fn next(&mut self) -> Option<&[i16]> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                // End of stream, and anything unrecoverable, both end the track. A player has
                // nothing useful to do with the difference.
                _ => return None,
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let Ok(decoded) = self.decoder.decode(&packet) else {
                continue;
            };
            if decoded.frames() == 0 || decoded.is_unused() {
                continue;
            }
            self.rate = decoded.spec().rate();
            self.channels = decoded.spec().channels().count().max(1);
            self.buf.clear();
            decoded.copy_to_vec_interleaved(&mut self.buf);
            if self.buf.is_empty() {
                continue;
            }
            // Mono is played on both speakers. The ring is stereo throughout, so folding here
            // is one copy rather than a second path through everything downstream.
            if self.channels == 1 {
                let mut stereo = Vec::with_capacity(self.buf.len() * 2);
                for s in &self.buf {
                    stereo.push(*s);
                    stereo.push(*s);
                }
                self.buf = stereo;
            }
            return Some(&self.buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(stem: &str, title: &str, track_number: Option<u32>) -> Track {
        Track {
            stem: stem.to_string(),
            path: PathBuf::from(format!("Music/_/{stem}.mp3")),
            title: title.to_string(),
            artist: String::new(),
            album: String::new(),
            track_number,
            duration: 0.0,
            initial: 'x',
            tags: Vec::new(),
        }
    }

    // A dropped-in album must play in album order: the file's own track number first, then the
    // file name's leading digits, then the title. File names here are deliberately scrambled and
    // titles deliberately reversed, so the order can only come from `track_number`/filename.
    #[test]
    fn disc_plays_in_track_number_order() {
        let tracks = vec![
            sample("09 尾奏.mp3", "尾奏", None),
            sample("02 间奏.mp3", "间奏", Some(2)),
            sample("歌名.mp3", "歌名", None),
            sample("01 序曲.mp3", "序曲", Some(1)),
            sample("11 隐藏.mp3", "隐藏", Some(11)),
            sample("03 主歌.mp3", "主歌", None),
        ];
        let disc = disc_named("Album".to_string(), tracks);
        let order: Vec<&str> = disc.tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(order, vec!["序曲", "间奏", "隐藏", "主歌", "尾奏", "歌名"]);
    }

    // ── The GBK repair, against the two strings the device actually showed ──────────────
    //
    // These are not made up: `ÖÜ½ÜÂ× - ¿É°®Å®ÈË` is what the deck's panel read on 2026-10-07,
    // photographed, and each half is a real GBK byte pair sequence read as ISO-8859-1. See
    // `repair` for the bytes.

    #[test]
    fn a_gbk_artist_read_as_latin1_comes_back() {
        assert_eq!(super::repair("ÖÜ½ÜÂ×"), "周杰伦");
    }

    #[test]
    fn a_gbk_title_read_as_latin1_comes_back() {
        assert_eq!(super::repair("¿É°®Å®ÈË"), "可爱女人");
    }

    /// The whole line as the panel ends up showing it. **It is repaired as two halves, not as
    /// one string** — `repair` runs on the tag `read` pulled out, and the ` - ` the panel joins
    /// them with is ASCII, which is the first guard above. So this composes the line the way
    /// `lcd::title_text` does, from the two repaired halves.
    #[test]
    fn the_panels_own_line_comes_back() {
        let artist = super::repair("ÖÜ½ÜÂ×");
        let title = super::repair("¿É°®Å®ÈË");
        assert_eq!(format!("{artist} - {title}"), "周杰伦 - 可爱女人");
    }

    /// **The guard that matters most.** A repair that fired on real Latin-1 would corrupt every
    /// European tag on the card, so text with any ASCII in it is left alone even though its
    /// accented characters are perfectly valid GBK lead bytes.
    #[test]
    fn ordinary_latin_text_is_untouched() {
        for s in ["Björk", "Café Tacvba", "Motörhead", "Sigur Rós"] {
            assert_eq!(super::repair(s), s, "{s} must not be repaired");
        }
    }

    /// And the second guard, on the case that gave it its shape. `ÃÂ` is a genuinely nasty one:
    /// its two bytes are a valid GBK pair, they decode without error, and they land on a real Han
    /// character (`寐`). Only the *count* tells it from a repair, which is why `repair` asks for
    /// more than one.
    /// ★ **A GBK mis-reading that carries ASCII inside it must still be repaired.** This is the
    /// one that got away: `半島鐵盒` is `b0 eb 8d 75 e8 46 ba d0` in GBK, and read as ISO-8859-1
    /// it keeps a literal `u` and `F` — which the old "no ASCII anywhere" guard took as proof
    /// that this was ordinary text. 六六 photographed the result (`°ë uèF ºÐ`) off the deck's
    /// panel on 2026-10-07. `半獸人` is the same file's neighbour and the same shape (`°ë«FÈË`),
    /// so the fix is not a one-title special case.
    #[test]
    fn a_gbk_title_with_ascii_bytes_inside_comes_back() {
        assert_eq!(super::repair("°ë\u{8d}uèFºÐ"), "半島鐵盒");
        assert_eq!(super::repair("°ë«FÈË"), "半獸人");
    }

    #[test]
    fn a_lone_gbk_pair_is_not_a_repair() {
        assert_eq!(super::repair("ÃÂ"), "ÃÂ");
    }

    #[test]
    fn ascii_and_empty_pass_through() {
        assert_eq!(super::repair(""), "");
        assert_eq!(super::repair("Jay"), "Jay");
        assert_eq!(super::repair("[2000]JAY"), "[2000]JAY");
    }
}
