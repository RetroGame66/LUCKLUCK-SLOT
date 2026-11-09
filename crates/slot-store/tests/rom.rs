//! Reading a rom off the card, out of a `.zip` when that is how it is stored.

use std::fs;
use std::io::Write as _;

use slot_store::{is_zip, read_rom, RomError};
use tempfile::TempDir;

/// A rom's worth of bytes. Not a real rom — nothing here reads a header — just something with a
/// rom's size, because size is the only thing that picks the entry.
fn rom_bytes(tag: u8, kib: usize) -> Vec<u8> {
    vec![tag; kib * 1024]
}

fn dir() -> TempDir {
    tempfile::tempdir().expect("tempdir")
}

/// Write one entry into an archive under construction.
fn add(
    zip: &mut zip::ZipWriter<fs::File>,
    name: &str,
    bytes: &[u8],
    deflated: bool,
) -> zip::result::ZipResult<()> {
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(if deflated {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        });
    zip.start_file(name, opts)?;
    zip.write_all(bytes)?;
    Ok(())
}

/// Build an archive on disk from `(name, bytes, deflated)` triples.
fn build(d: &TempDir, file: &str, entries: &[(&str, Vec<u8>, bool)]) -> std::path::PathBuf {
    let path = d.path().join(file);
    let f = fs::File::create(&path).expect("create archive");
    let mut zip = zip::ZipWriter::new(f);
    for (name, bytes, deflated) in entries {
        add(&mut zip, name, bytes, *deflated).expect("write entry");
    }
    zip.finish().expect("finish archive");
    path
}

/// A rom that is not stored in an archive is read exactly as it lies on the card.
#[test]
fn a_plain_rom_is_read_as_it_lies() {
    let d = dir();
    let bytes = rom_bytes(0xA5, 512);
    let path = d.path().join("Emerald.gba");
    fs::write(&path, &bytes).unwrap();
    let got = read_rom(&path, true).expect("read a plain rom");
    assert_eq!(got, bytes);
}

/// A zip holding one file is that file, and the unpacking is what gets it out.
#[test]
fn a_zip_with_one_file_is_that_file() {
    let d = dir();
    let bytes = rom_bytes(0x11, 256);
    let path = build(
        &d,
        "Mother 3.zip",
        &[("Mother 3 (Japan).gba", bytes.clone(), true)],
    );
    let got = read_rom(&path, true).expect("read a zipped rom");
    assert_eq!(got, bytes);
}

/// The largest entry is the rom. This is the whole picking rule, and the reason it needs no table
/// of extensions: a rom is megabytes and a readme is kilobytes, so size separates them by orders
/// of magnitude.
#[test]
fn the_largest_entry_is_the_rom() {
    let d = dir();
    let rom = rom_bytes(0x22, 1024);
    let path = build(
        &d,
        "a-collection.zip",
        &[
            ("readme.txt", b"dump it and play it\n".to_vec(), true),
            ("Mother 3 (Japan).gba", rom.clone(), true),
            ("notes.nfo", b"whatever\n".to_vec(), true),
        ],
    );
    let got = read_rom(&path, true).expect("read a zipped rom");
    assert_eq!(got, rom);
}

/// What macOS leaves beside a copy is card bookkeeping, not content. It is small enough that it
/// would never win anyway; skipping it just keeps an archive that is *all* bookkeeping from
/// answering with one of its sidecars.
#[test]
fn card_bookkeeping_is_never_the_rom() {
    let d = dir();
    let rom = rom_bytes(0x33, 512);
    let path = build(
        &d,
        "bookkeeping.zip",
        &[
            ("__MACOSX/._Mother 3.gba", vec![0x00; 128], true),
            ("Mother 3 (Japan).gba", rom.clone(), true),
            (".DS_Store", vec![0x01; 64], true),
        ],
    );
    let got = read_rom(&path, true).expect("read a zipped rom");
    assert_eq!(got, rom);
}

/// An archive that holds nothing but bookkeeping is an error rather than a silently wrong rom.
#[test]
fn an_archive_of_nothing_but_bookkeeping_is_an_error() {
    let d = dir();
    let path = build(
        &d,
        "junk.zip",
        &[
            ("__MACOSX/._x", vec![0u8; 32], true),
            (".DS_Store", vec![1u8; 32], true),
        ],
    );
    match read_rom(&path, true) {
        Err(RomError::EmptyZip(_)) => {}
        other => panic!("expected EmptyZip, got {other:?}"),
    }
}

/// A core that opens zips itself — FBNeo, and only FBNeo — is handed the archive whole. One file
/// out of a romset is not a romset, so unpacking it would break the core that wanted it.
#[test]
fn a_core_that_reads_zips_itself_gets_the_archive_whole() {
    let d = dir();
    let inner = rom_bytes(0x44, 128);
    let path = build(&d, "mslug.zip", &[("mslug.rom", inner, true)]);
    let whole = fs::read(&path).unwrap();
    let got = read_rom(&path, false).expect("hand the archive over whole");
    assert_eq!(got, whole, "the archive was unpacked for a core that reads it itself");
}

/// Whether a file is a zip is sniffed off its first four bytes, not off its name. The extension
/// is not what decided this rom was a cart — the folder did — so a zip named `.gba` is exactly
/// as much a zip as one named `.zip`.
#[test]
fn a_zip_is_sniffed_not_named() {
    let d = dir();
    let path = build(&d, "Mother 3.gba", &[("x.gba", vec![7u8; 64], true)]);
    assert!(is_zip(&path), "a zip named .gba was not recognised as one");

    let plain = d.path().join("actually-a-zip.zip");
    fs::write(&plain, b"this is not a zip at all, just text").unwrap();
    assert!(!is_zip(&plain), "a plain file named .zip was taken for an archive");
}

/// A zip whose entry is stored rather than deflated is still read. Real archives are usually
/// deflated, but a stored entry is legal and nothing here should care.
#[test]
fn a_stored_entry_is_read_too() {
    let d = dir();
    let bytes = rom_bytes(0x55, 256);
    let path = build(&d, "stored.zip", &[("rom.gba", bytes.clone(), false)]);
    assert_eq!(read_rom(&path, true).unwrap(), bytes);
}

/// A file that is not there is an io error, not a zip error — the two mean different things to
/// whoever reads the log.
#[test]
fn a_missing_file_is_an_io_error() {
    let d = dir();
    let gone = d.path().join("nowhere.gba");
    match read_rom(&gone, true) {
        Err(RomError::Io(_)) => {}
        other => panic!("expected Io, got {other:?}"),
    }
}
