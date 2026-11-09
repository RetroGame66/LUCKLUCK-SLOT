//! A playlist: the one file on a card that says what order its own music goes in.
//!
//! **Why a folder with a playlist is one disc.** A music library is `Music/<album>/<track>`, and
//! an album is a thing you put on a shelf as a whole — a disc, not a loose cassette with eleven
//! identical spines. So a folder carrying a `.m3u` is collected into one `Disc` whose face is the
//! album and whose tracks are the songs in playlist order; a folder without one is left as loose
//! tracks, because a folder of unrelated rips is not an album and calling it one would be a lie
//! about what the user put on the card.
//!
//! **Encoding.** A `.m3u` is a text file, and text files on a card arrive in whatever the
//! writing tool's platform defaulted to. UTF-8 (with or without a BOM) is read directly;
//! anything that is not valid UTF-8 is decoded as GBK, which is what a Chinese-language ripper
//! writes and what an English one never produces — so the fallback cannot misread a valid UTF-8
//! file, it only rescues one that was never UTF-8. A playlist that decodes to nothing usable
//! yields no disc rather than an empty one: a card is allowed to hold a broken playlist, and the
//! folder's own tracks are then shown as loose ones, which is the truthful answer.

use std::path::{Path, PathBuf};

/// The playlist extensions that are read, in the order one folder is searched for them.
///
/// `.m3u8` is listed after plain `.m3u` deliberately: when a folder carries both, the older
/// name wins, because that is the file a player opened first and the order the tracks were
/// authored in. `.cue` is accepted as a marker of an album but not parsed for order — a cue
/// sheet is a single-file convention, and treating it as an album boundary is all it is asked
/// to be here.
const PLAYLIST_EXTENSIONS: [&str; 3] = ["m3u", "m3u8", "cue"];

/// The playlist inside `dir`, if it has one. `None` when it has none, which is the signal that
/// its tracks stay loose.
pub fn find(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(usize, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let ext = ext.to_ascii_lowercase();
        let Some(rank) = PLAYLIST_EXTENSIONS.iter().position(|p| *p == ext) else {
            continue;
        };
        // Lowest rank wins; equal ranks cannot happen because one folder cannot hold two files
        // of the same name, so the comparison needs no tie-break.
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, path));
        }
    }
    best.map(|(_, p)| p)
}

/// The track paths a playlist lists, in the order it lists them.
///
/// **A path is only a track if it is there.** This is the rule that makes a playlist worth
/// trusting rather than worth obeying: a playlist is a text file a stranger wrote, it is edited
/// by hand, and the single most common way for one to be wrong is a line naming a file that was
/// since renamed or deleted. Such a line is dropped, not resolved by guessing an extension and
/// not turned into a track the deck will fail to open. Relative entries resolve against the
/// playlist's own folder, which is where every player resolves them; absolute ones are taken as
/// they are.
pub fn tracks_of(playlist: &Path) -> Vec<PathBuf> {
    let Ok(bytes) = std::fs::read(playlist) else {
        return Vec::new();
    };
    let text = decode(&bytes);
    let base = playlist.parent().unwrap_or(Path::new("."));
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Windows players write absolute Windows paths into a playlist that travels to a Linux
        // card. The drive letter and the backslashes are stripped rather than the line being
        // dropped, because the file name at the end of such a line is normally correct and the
        // track it names is normally present.
        let cleaned = line.replace('\\', "/");
        let cleaned = match cleaned.find(':') {
            // Only a leading `X:` is a drive; a colon later in the line is part of a file name.
            Some(i) if i == 1 => cleaned[i + 1..].trim_start_matches('/').to_string(),
            _ => cleaned,
        };
        if cleaned.is_empty() {
            continue;
        }
        let path = if cleaned.starts_with('/') {
            PathBuf::from(cleaned)
        } else {
            base.join(cleaned)
        };
        if crate::Track::is_track(&path) && path.is_file() {
            out.push(path);
        }
    }
    out
}

/// Text out of playlist bytes: UTF-8 first, GBK when that is not what it is.
///
/// The BOM is honoured rather than merely tolerated, and stripped either way — a BOM left at the
/// head of the first line would otherwise become part of that line's file name, and the first
/// line of a playlist is usually a real track.
pub fn decode(bytes: &[u8]) -> String {
    const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
    let body = bytes.strip_prefix(BOM).unwrap_or(bytes);
    match std::str::from_utf8(body) {
        Ok(s) => return s.to_string(),
        Err(_) => {
            // `decode_without_bom_handling` because the BOM is already gone, and a GBK file whose
            // first two bytes happened to be a BOM would otherwise lose a character.
            let (cow, _) = encoding_rs::GBK.decode_without_bom_handling(body);
            return cow.into_owned();
        }
    }
}

/// Walk `root/Music` and, in every subfolder that holds audio files but no playlist of its
/// own, drop a `list.m3u` listing those files in play order. A folder that already carries a
/// playlist is left alone — it is an album the user curated, and `walk_discs` does not descend
/// into it either, so neither do we.
///
/// **Why write one at all.** A folder with no playlist is shown as loose tracks, one disc per
/// song; a folder with one is shown as a single disc. The user asked that a folder of songs
/// become one disc without authoring the list by hand, so the list is written for them — once,
/// and only when missing, so a list they did write is never overwritten.
///
/// **Order.** A leading run of digits in the file name reads as the track number and sorts
/// first; files with no leading number fall back to name order. The digits are taken only from
/// the start of the stem, after any leading punctuation, so `02 - Song.mp3` is track 2 but
/// `Song (02).mp3` is not (its number is in the name, not a sequence), and a title such as
/// `歌.mp3` sorts by name rather than being mis-read as track 0.
///
/// Best effort: a read-only card or a write that fails logs and moves on. The walk is
/// depth-limited to match `walk_discs`, so a symlink loop cannot hang it.
pub fn ensure_music(root: &Path) {
    let music = root.join(crate::DIR);
    let Ok(entries) = std::fs::read_dir(&music) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "System Volume Information" {
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            ensure_in(&path, 0);
        }
    }
}

fn ensure_in(dir: &Path, depth: usize) {
    if depth > 8 {
        return;
    }
    // An album the user already curated: leave it, and do not descend (matches `walk_discs`).
    if find(dir).is_some() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut tracks: Vec<PathBuf> = Vec::new();
    let mut subdirs: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "System Volume Information" {
            continue;
        }
        match entry.file_type() {
            Ok(t) if t.is_dir() => subdirs.push(path),
            Ok(t) if t.is_file() && crate::Track::is_track(&path) => tracks.push(path),
            _ => {}
        }
    }
    if !tracks.is_empty() {
        tracks.sort_by_key(|p| sort_key(p));
        let list = dir.join("list.m3u");
        if !list.exists() {
            let body: String = tracks
                .iter()
                .filter_map(|t| t.file_name().and_then(|s| s.to_str()))
                .map(|f| format!("{f}\n"))
                .collect();
            if let Err(e) = std::fs::write(&list, body) {
                eprintln!("slot: playlist: 自动建 {list:?} 失败：{e}");
            }
        }
    }
    for sub in subdirs {
        ensure_in(&sub, depth + 1);
    }
}

/// `(track number, file stem)` — numbers first, then name. `None` sorts after every number, so a
/// folder mixing numbered and un-numbered files still reads in a sane order.
/// The file's place in the list: its own leading number, then its name.
///
/// ⚠️ **`u32::MAX`, not `None`** (2026-10-07). This returned `Option<u32>`, and `None` sorts
/// *before* `Some(1)` — so an untitled-numeric file landed at the **front** of the list, which is
/// the opposite of what this module's own doc says and of what `slot_media::disc_named` does with
/// the same ladder. `list.m3u` is re-sorted downstream, so nothing played in the wrong order; what
/// was wrong was the file the user can read and edit, which is the one thing a written playlist
/// is for.
fn sort_key(p: &Path) -> (u32, String) {
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    (leading_number(&stem).unwrap_or(u32::MAX), stem)
}

/// The leading run of digits in `stem`, after any leading punctuation, as a track number.
///
/// `is_alphanumeric` is Unicode-aware, so a CJK first character stops the trim (a Chinese title
/// is not punctuation to skip) while `(`/space/`-` are; the digits are then taken only from the
/// very start, so `歌01` keeps its number in the name and is not mis-read as track 1.
pub(crate) fn leading_number(stem: &str) -> Option<u32> {
    let trimmed = stem.trim_start_matches(|c: char| !c.is_alphanumeric());
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok()
}


#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn a_bom_does_not_become_part_of_the_first_line() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("Track.mp3\n".as_bytes());
        let text = decode(&bytes);
        assert_eq!(text.lines().next(), Some("Track.mp3"));
    }

    #[test]
    fn gbk_is_read_when_the_file_is_not_utf8() {
        // 音乐 in GBK. Not valid UTF-8, which is the whole point: this is the byte sequence a
        // Chinese-language ripper writes and an English one never produces.
        let bytes = [0xD2, 0xF4, 0xC0, 0xD6, b'\n'];
        assert!(
            std::str::from_utf8(&bytes[..4]).is_err(),
            "the fixture must not be valid UTF-8, or it tests nothing"
        );
        let text = decode(&bytes);
        assert_eq!(text.trim(), "音乐");
    }

    #[test]
    fn utf8_is_preferred_over_gbk() {
        let text = decode("音乐\n".as_bytes());
        assert_eq!(text.trim(), "音乐");
    }

    #[test]
    fn auto_playlist_lists_tracks_in_number_order() {
        let tmp = std::env::temp_dir().join(format!("slot_media_pl_{}.", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let music = tmp.join("Music");
        let album = music.join("Album");
        std::fs::create_dir_all(&album).unwrap();
        for name in ["03 尾声.mp3", "01 序曲.mp3", "歌名.mp3", "02 间奏.mp3"] {
            std::fs::write(album.join(name), b"x").unwrap();
        }
        let sub = album.join("Disc2");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.mp3"), b"x").unwrap();
        std::fs::write(sub.join("b.mp3"), b"x").unwrap();

        super::ensure_music(&tmp);

        let sub_list = std::fs::read_to_string(sub.join("list.m3u")).unwrap();
        assert_eq!(sub_list, "a.mp3\nb.mp3\n");

        let list = std::fs::read_to_string(album.join("list.m3u")).unwrap();
        assert_eq!(
            list,
            "01 序曲.mp3\n02 间奏.mp3\n03 尾声.mp3\n歌名.mp3\n"
        );

        std::fs::write(album.join("list.m3u"), "manual\n").unwrap();
        super::ensure_music(&tmp);
        assert_eq!(std::fs::read_to_string(album.join("list.m3u")).unwrap(), "manual\n");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
