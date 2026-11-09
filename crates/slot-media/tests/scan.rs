//! The scan, against a tree the test builds itself.
//!
//! WAVs rather than MP3s: a WAV is a header and some samples, so the test can write one in
//! twenty lines and the repository needs no fixture. What is being tested is the walk, the
//! naming and the folder-is-the-album rule — none of which care which codec is behind them.

use std::path::{Path, PathBuf};

/// A directory under the system temp that removes itself.
///
/// Hand-rolled rather than `tempfile`, and that is not a preference: `tempfile` drags in
/// `windows-sys`, and the host toolchain here has no `dlltool` to link it with. The suite this
/// belongs to is meant to run on the host, so it cannot depend on anything that only links on
/// the device.
struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        // Tests run as threads in one process, so a counter is enough to keep them apart.
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "slot-media-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Temp(p)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A 16-bit stereo PCM WAV of `frames` frames at `rate`. Silence, because nothing here listens.
fn write_wav(path: &Path, rate: u32, frames: usize) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    let bytes = (frames * 4) as u32;
    let mut h = Vec::with_capacity(44 + bytes as usize);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + bytes).to_le_bytes());
    h.extend_from_slice(b"WAVE");
    h.extend_from_slice(b"fmt ");
    h.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    h.extend_from_slice(&1u16.to_le_bytes()); // PCM
    h.extend_from_slice(&2u16.to_le_bytes()); // stereo
    h.extend_from_slice(&rate.to_le_bytes());
    h.extend_from_slice(&(rate * 4).to_le_bytes()); // byte rate
    h.extend_from_slice(&4u16.to_le_bytes()); // block align
    h.extend_from_slice(&16u16.to_le_bytes()); // bits
    h.extend_from_slice(b"data");
    h.extend_from_slice(&bytes.to_le_bytes());
    h.resize(44 + bytes as usize, 0);
    std::fs::write(path, &h).unwrap();
}

#[test]
fn a_loose_file_is_a_track_and_knows_how_long_it_is() {
    let d = Temp::new();
    // Two seconds at 8000 Hz.
    write_wav(&d.path().join("Solo.wav"), 8000, 16_000);

    let tracks = slot_media::scan_dir(d.path());
    assert_eq!(tracks.len(), 1, "a loose wav should be one track");
    assert_eq!(tracks[0].title, "Solo");
    assert_eq!(tracks[0].stem, "Solo");
    assert!(
        (tracks[0].duration - 2.0).abs() < 0.05,
        "duration was {}",
        tracks[0].duration
    );
    assert_eq!(tracks[0].clock(), "0:02");
}

#[test]
fn the_folder_a_file_sits_in_is_its_album() {
    let d = Temp::new();
    write_wav(&d.path().join("Vol.1").join("One.wav"), 8000, 8000);

    let tracks = slot_media::scan_dir(d.path());
    assert_eq!(tracks.len(), 1);
    assert_eq!(
        tracks[0].album, "Vol.1",
        "a library is Music/<album>/<track>, and the tags on a real card are often empty"
    );
}

#[test]
fn the_walk_reaches_any_depth() {
    let d = Temp::new();
    write_wav(&d.path().join("a").join("b").join("c").join("Deep.wav"), 8000, 8000);
    assert_eq!(slot_media::scan_dir(d.path()).len(), 1);
}

#[test]
fn what_is_not_music_is_not_a_track() {
    let d = Temp::new();
    std::fs::write(d.path().join("notes.txt"), b"not music").unwrap();
    std::fs::write(d.path().join("cover.jpg"), b"not music").unwrap();
    write_wav(&d.path().join("Real.wav"), 8000, 8000);
    // A file with the right extension and nothing behind it is not a track either: a card is
    // allowed to hold one, and the scan is allowed to skip it rather than fail on it.
    std::fs::write(d.path().join("Broken.mp3"), b"not really an mp3").unwrap();

    let tracks = slot_media::scan_dir(d.path());
    assert_eq!(tracks.len(), 1, "only the real wav should be listed");
    assert_eq!(tracks[0].title, "Real");
}

#[test]
fn hidden_entries_and_the_volume_folder_are_skipped() {
    let d = Temp::new();
    write_wav(&d.path().join("Keep.wav"), 8000, 8000);
    write_wav(&d.path().join(".trash").join("Hidden.wav"), 8000, 8000);
    write_wav(
        &d.path().join("System Volume Information").join("Junk.wav"),
        8000,
        8000,
    );

    let tracks = slot_media::scan_dir(d.path());
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].title, "Keep");
}

#[test]
fn the_scan_is_sorted_by_the_title_it_shows() {
    let d = Temp::new();
    for name in ["Zebra.wav", "Apple.wav", "Mango.wav"] {
        write_wav(&d.path().join(name), 8000, 8000);
    }
    let titles: Vec<String> = slot_media::scan_dir(d.path())
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["Apple", "Mango", "Zebra"]);
}

// ── Discs: an album with a playlist is one disc ──

#[test]
fn a_folder_with_a_playlist_is_one_disc() {
    let d = Temp::new();
    let album = d.path().join("Vol.1");
    // Three songs, listed out of alphabetical order on purpose: the playlist's order is the
    // order they play in, and alphabetical order would hide a scanner that ignored it.
    for name in ["A.wav", "B.wav", "C.wav"] {
        write_wav(&album.join(name), 8000, 8000);
    }
    std::fs::write(album.join("list.m3u"), b"C.wav\nA.wav\nB.wav\n").unwrap();

    let discs = slot_media::discs_dir(d.path());
    assert_eq!(discs.len(), 1, "a playlist folder is one disc, not three singles");
    assert_eq!(discs[0].name, "Vol.1", "the disc is named by the album folder");
    let titles: Vec<&str> = discs[0].tracks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["C", "A", "B"], "the playlist's order is the play order");
}

#[test]
fn a_folder_without_a_playlist_stays_loose() {
    let d = Temp::new();
    let album = d.path().join("Vol.1");
    write_wav(&album.join("One.wav"), 8000, 8000);
    write_wav(&album.join("Two.wav"), 8000, 8000);

    let discs = slot_media::discs_dir(d.path());
    assert_eq!(discs.len(), 2, "no playlist means each track is its own disc");
    let names: Vec<&str> = discs.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["One", "Two"], "a loose disc is named by its own track");
    assert!(discs.iter().all(|t| t.tracks.len() == 1));
}

#[test]
fn a_playlist_line_naming_a_missing_file_is_dropped() {
    let d = Temp::new();
    let album = d.path().join("Vol.1");
    write_wav(&album.join("Real.wav"), 8000, 8000);
    // The single most common way a playlist is wrong: a line naming a file that has since been
    // renamed or deleted. It must not become a track the deck will fail to open.
    std::fs::write(
        album.join("list.m3u"),
        b"Ghost.wav\nReal.wav\nAlsoGone.mp3\n",
    )
    .unwrap();

    let discs = slot_media::discs_dir(d.path());
    assert_eq!(discs.len(), 1);
    assert_eq!(discs[0].tracks.len(), 1, "only the file that is there");
    assert_eq!(discs[0].tracks[0].title, "Real");
}

#[test]
fn a_playlist_in_gbk_still_finds_its_tracks() {
    let d = Temp::new();
    let album = d.path().join("音乐");
    // The realistic shape of this: the file on disk carries its name in UTF-8 like everything
    // else on a Linux card, and the **playlist** is the GBK file — a Chinese-language ripper
    // wrote it that way. So the line has to be decoded before it can name anything.
    write_wav(&album.join("音乐.wav"), 8000, 8000);
    // 音乐.wav in GBK.
    let line = [0xD2u8, 0xF4, 0xC0, 0xD6, b'.', b'w', b'a', b'v', b'\n'];
    std::fs::write(album.join("list.m3u"), &line).unwrap();

    let discs = slot_media::discs_dir(d.path());
    assert_eq!(discs.len(), 1, "a GBK playlist is a playlist, not a broken file");
    assert_eq!(discs[0].name, "音乐", "the album folder names the disc");
    assert_eq!(
        discs[0].tracks.len(),
        1,
        "the GBK line must resolve to the file beside it, or the disc would be empty"
    );
    assert_eq!(discs[0].tracks[0].title, "音乐");
}

#[test]
fn a_utf8_bom_playlist_does_not_lose_its_first_track() {
    let d = Temp::new();
    let album = d.path().join("Vol.1");
    write_wav(&album.join("First.wav"), 8000, 8000);
    write_wav(&album.join("Second.wav"), 8000, 8000);
    let mut list = vec![0xEF, 0xBB, 0xBF];
    list.extend_from_slice(b"First.wav\nSecond.wav\n");
    std::fs::write(album.join("list.m3u"), &list).unwrap();

    let discs = slot_media::discs_dir(d.path());
    assert_eq!(discs[0].tracks.len(), 2, "a BOM must not become part of the first file name");
    assert_eq!(discs[0].tracks[0].title, "First");
}

#[test]
fn a_stale_playlist_does_not_hide_the_folder() {
    let d = Temp::new();
    let album = d.path().join("Vol.1");
    write_wav(&album.join("Real.wav"), 8000, 8000);
    // A playlist that names nothing playable is a stale file, not a reason to hide real music.
    std::fs::write(album.join("old.m3u"), b"Gone1.wav\nGone2.wav\n").unwrap();

    let discs = slot_media::discs_dir(d.path());
    assert_eq!(discs.len(), 1);
    assert_eq!(discs[0].name, "Real", "the folder's own track speaks for itself");
}

#[test]
fn a_discs_duration_is_the_sum_of_its_tracks() {
    let d = Temp::new();
    let album = d.path().join("Vol.1");
    // Two seconds each at 8000 Hz, three of them.
    for name in ["A.wav", "B.wav", "C.wav"] {
        write_wav(&album.join(name), 8000, 16_000);
    }
    std::fs::write(album.join("list.m3u"), b"A.wav\nB.wav\nC.wav\n").unwrap();

    let discs = slot_media::discs_dir(d.path());
    assert!(
        (discs[0].duration - 6.0).abs() < 0.15,
        "duration was {}",
        discs[0].duration
    );
    assert_eq!(discs[0].clock(), "0:06");
}

// ── Stepping the disc cursor: it stops at both ends, it does not ring ──

#[test]
fn the_disc_cursor_stops_at_both_ends() {
    // The distinction this pins: the transport keys ring (see `deck_step`), the disc list does
    // not. Wrapping would claim the card's shelf continues past its last album.
    assert_eq!(slot_media::Disc::step(0, 3, -1), 0, "back at the first disc, stay");
    assert_eq!(slot_media::Disc::step(2, 3, 1), 2, "at the last disc, stay");
    assert_eq!(slot_media::Disc::step(0, 3, 1), 1);
    assert_eq!(slot_media::Disc::step(2, 3, -1), 1);
}

#[test]
fn stepping_an_empty_disc_list_goes_nowhere() {
    // Both directions, and repeatedly: a card with no music must not be able to walk its cursor
    // off the end of nothing.
    assert_eq!(slot_media::Disc::step(0, 0, 1), 0);
    assert_eq!(slot_media::Disc::step(0, 0, -1), 0);
    assert_eq!(slot_media::Disc::step(0, 1, 1), 0, "a single disc has no next");
    assert_eq!(slot_media::Disc::step(0, 1, -1), 0, "and no previous");
}

// ── An album folder dropped in whole: the lrc and the cover come out of it ──
//
// 六六 2026-10-06：「以碟片文件夹路径为优先级 —— 用户整张数码专辑放进来，
// 自动抓取里面的 cover 图片当封面，自动联动里面的歌曲同名 lrc」。
//
// 这两条钉的就是那两档**新的**查找（`lrc::Index::find` 的 `sibling_lrc`
// 与 `Covers::find` 的 `in_disc_folder`）。它们排在**索引之前**，而索引只扫
// `Music/Lyrics/` 与 `Music/Covers/` —— 所以下面这两棵树里**那两个目录根本不存在**，
// 旧写法在这两张卡上一定什么都找不到，正是用户要修的那个形状。

/// 一张 `Music/<碟片>/` 的整张专辑：歌、同名 lrc、封面都在**同一个文件夹**里。
fn write_album(dir: &Path, album: &str, tracks: &[(&str, &str)]) {
    for (song, lrc) in tracks {
        write_wav(&dir.join(album).join(format!("{song}.wav")), 8000, 8000);
        std::fs::write(
            dir.join(album).join(format!("{song}.lrc")),
            format!("[00:00.00]{lrc}\n").as_bytes(),
        )
        .unwrap();
    }
}

#[test]
fn a_lyric_beside_its_song_is_found_without_a_lyrics_folder() {
    let d = Temp::new();
    // No `Lyrics/` anywhere: the old index has nothing to say about this card.
    write_album(
        d.path(),
        "Thriller",
        &[("01 - Beat It", "They don't like the things you do")],
    );

    let discs = slot_media::discs_dir(d.path());
    assert_eq!(discs.len(), 1, "one folder is one disc");
    let idx = slot_media::lrc::Index::scan(d.path());
    assert_eq!(idx.len(), 0, "there is no Lyrics/ folder to index");

    let lines = idx
        .load(&discs[0].tracks[0])
        .expect("the lrc sitting beside the song is the whole point");
    assert_eq!(lines[0].text, "They don't like the things you do");
}

#[test]
fn a_lrc_whose_name_only_half_matches_does_not_get_hung_on_the_song() {
    // The failure this rules out is quiet and expensive: `Beat It.lrc` mounted onto
    // `01 - Beat It.mp3` would *look* right — lyrics appear, they are just the wrong song's,
    // and nothing anywhere says why. So the tolerance is the same one `stems_for` allows
    // (drop the bracket, drop the `Artist - ` prefix), and nothing looser.
    let d = Temp::new();
    let album = d.path().join("Thriller");
    write_wav(&album.join("01 - Beat It.wav"), 8000, 8000);
    std::fs::write(album.join("Billie Jean.lrc"), b"[00:00.00wrong\n").unwrap();

    let discs = slot_media::discs_dir(d.path());
    assert!(
        slot_media::lrc::Index::scan(d.path()).load(&discs[0].tracks[0]).is_none(),
        "another song's lyric must not appear on this one"
    );
}

#[test]
fn two_matching_lrc_files_in_one_folder_are_not_a_guess() {
    // `01 - Beat It.lrc` and `Beat It.lrc` both beside one song: picking either could be wrong,
    // and a wrong lyric is worse than none because it looks right.
    let d = Temp::new();
    let album = d.path().join("Thriller");
    write_wav(&album.join("01 - Beat It.wav"), 8000, 8000);
    std::fs::write(album.join("01 - Beat It.lrc"), b"[00:00.00a\n").unwrap();
    std::fs::write(album.join("Beat It.lrc"), b"[00:00.00b\n").unwrap();

    let discs = slot_media::discs_dir(d.path());
    assert!(slot_media::lrc::Index::scan(d.path()).load(&discs[0].tracks[0]).is_none());
}

#[test]
fn a_cover_beside_the_album_is_found_without_a_covers_folder() {
    let d = Temp::new();
    write_album(d.path(), "Thriller", &[("01 - Beat It", "x")]);
    std::fs::write(d.path().join("Thriller").join("cover.jpg"), b"jpeg").unwrap();

    let discs = slot_media::discs_dir(d.path());
    let covers = slot_media::Covers::scan(d.path());
    assert_eq!(covers.len(), 0, "there is no Covers/ folder to index");

    assert_eq!(
        covers.find(&discs[0]).as_deref(),
        Some(d.path().join("Thriller").join("cover.jpg").as_path()),
        "the sleeve has to be the one lying next to the songs"
    );
}

#[test]
fn a_cover_named_after_the_album_counts_as_the_albums() {
    // The other shape a real album folder arrives in: `Thriller/Thriller.jpg`.
    let d = Temp::new();
    write_album(d.path(), "Thriller", &[("01 - Beat It", "x")]);
    std::fs::write(d.path().join("Thriller").join("Thriller.jpg"), b"jpeg").unwrap();

    let discs = slot_media::discs_dir(d.path());
    assert_eq!(
        slot_media::Covers::scan(d.path()).find(&discs[0]).as_deref(),
        Some(d.path().join("Thriller").join("Thriller.jpg").as_path())
    );
}

#[test]
fn a_lone_image_beside_the_songs_is_the_sleeve_and_two_are_a_guess() {
    let d = Temp::new();
    write_album(d.path(), "Thriller", &[("01 - Beat It", "x")]);
    // One image, no name we recognise: a folder of three songs and one picture is a sleeve.
    std::fs::write(d.path().join("Thriller").join("front_scan.jpg"), b"jpeg").unwrap();
    let one = slot_media::discs_dir(d.path());
    assert_eq!(
        slot_media::Covers::scan(d.path()).find(&one[0]).as_deref(),
        Some(d.path().join("Thriller").join("front_scan.jpg").as_path())
    );

    // A second one and the guess is off: either could be the sleeve and neither says which.
    std::fs::write(d.path().join("Thriller").join("back_scan.jpg"), b"jpeg").unwrap();
    assert!(
        slot_media::Covers::scan(d.path()).find(&one[0]).is_none(),
        "two unnamed images must not be guessed between"
    );
}

#[test]
fn a_named_cover_beats_an_unnamed_one_however_many_there_are() {
    // The name is the answer; the lone-image rule is only the last resort. With three images
    // around, `cover.jpg` is still the sleeve.
    let d = Temp::new();
    write_album(d.path(), "Thriller", &[("01 - Beat It", "x")]);
    for name in ["cover.jpg", "disc1.jpg", "back.jpg"] {
        std::fs::write(d.path().join("Thriller").join(name), b"jpeg").unwrap();
    }
    let discs = slot_media::discs_dir(d.path());
    assert_eq!(
        slot_media::Covers::scan(d.path()).find(&discs[0]).as_deref(),
        Some(d.path().join("Thriller").join("cover.jpg").as_path())
    );
}

#[test]
fn an_album_folder_with_no_image_keeps_the_covers_folder_way_working() {
    // The new first look must not *replace* the old one: a card that files its art in
    // `Covers/` and its music loose is still a card this frontend supports.
    //
    // **两个形状必须分清**（这一条栽过）：`Covers/` 这一层是**按碟**找的（`disc.stem` /
    // `disc.name`），一张封套是整张碟一张；而**没有 m3u 的文件夹是散曲**（见
    // `a_folder_without_a_playlist_stays_loose`），一首歌自成一碟、碟名是歌名。
    // 所以要按碟找就得先让那个文件夹成碟 —— 这里放一个 `list.m3u`。
    let d = Temp::new();
    let album = d.path().join("Solo");
    write_wav(&album.join("One.wav"), 8000, 8000);
    std::fs::write(album.join("list.m3u"), b"One.wav\n").unwrap();
    let art = d.path().join("Covers");
    std::fs::create_dir_all(&art).unwrap();
    std::fs::write(art.join("Solo.png"), b"png").unwrap();

    let discs = slot_media::discs_dir(d.path());
    assert_eq!(discs[0].name, "Solo", "the folder with a playlist is one disc");
    assert_eq!(
        slot_media::Covers::scan(d.path()).find(&discs[0]).as_deref(),
        Some(art.join("Solo.png").as_path())
    );
}

#[test]
fn a_cover_is_found_whatever_its_extension_is() {
    // The failure this pins is silent and looks like "the art is just missing": the key drops
    // the extension, so turning it back into a path means guessing `.png` — and a card whose
    // cover is a `.jpg` then gets a path to a file that is not there. No error, no blank-face
    // warning, just a sleeve.
    let d = Temp::new();
    let art = d.path().join("Covers");
    std::fs::create_dir_all(&art).unwrap();
    for (i, (album, ext)) in [("A", "png"), ("B", "jpg"), ("C", "JPEG")].iter().enumerate() {
        let folder = d.path().join(album);
        write_wav(&folder.join(format!("Song{}.wav", i)), 8000, 8000);
        // A playlist per album: without one the folder is loose tracks and each song is its own
        // disc, and then the sleeve would be keyed on the song's name rather than the album's.
        std::fs::write(folder.join("list.m3u"), format!("Song{}.wav\n", i).as_bytes()).unwrap();
        std::fs::write(art.join(format!("{album}.{ext}")), b"img").unwrap();
    }
    let discs = slot_media::discs_dir(d.path());
    let covers = slot_media::Covers::scan(d.path());
    assert_eq!(covers.len(), 3, "all three are pictures this index should hold");
    for (album, ext) in [("A", "png"), ("B", "jpg"), ("C", "JPEG")] {
        let got = covers
            .find(discs.iter().find(|x| x.name == album).expect("that disc"))
            .unwrap_or_else(|| panic!("{album} has a cover beside its name"));
        assert!(
            got.exists(),
            "{album}: handed back {:?}, which is not the file that is there",
            got.display().to_string()
        );
        assert_eq!(got.file_name().unwrap(), std::ffi::OsStr::new(&format!("{album}.{ext}")));
    }
}
