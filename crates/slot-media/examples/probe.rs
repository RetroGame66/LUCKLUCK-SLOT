//! Print what the scanner makes of a folder of music.
//!
//! A host binary, so it can be pointed at real music without the handheld: the card's music is
//! on this machine, not on the device.
//!
//!     python host_cargo.py run --target x86_64-pc-windows-gnu -p slot-media --example probe -- <dir>

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args.first().cloned().unwrap_or_else(|| ".".into());

    // Warm, then measured: the first pass pays for the filesystem cache and would report a
    // number that is about the disk rather than about the scanner.
    let _ = slot_media::scan_dir(std::path::Path::new(&dir));

    let t0 = std::time::Instant::now();
    let tracks = slot_media::scan_dir(std::path::Path::new(&dir));
    let ms = t0.elapsed().as_secs_f64() * 1000.0;

    println!("{dir}");
    println!("{} tracks in {ms:.0} ms", tracks.len());
    let total: f32 = tracks.iter().map(|t| t.duration).sum();
    println!("total {:.1} min", total / 60.0);
    println!();
    println!("{:>5}  {:>2}  {:<34} {:<18} {}", "time", "L", "title", "artist", "album");
    for t in tracks.iter().take(45) {
        println!(
            "{:>5}  {:>2}  {:<34} {:<18} {}",
            t.clock(),
            t.initial,
            t.title,
            t.artist,
            t.album
        );
    }
    if tracks.len() > 45 {
        println!("  … and {} more", tracks.len() - 45);
    }

    // The discs, which is what the deck actually shows. Printed after the tracks because a
    // folder that is an album shows up in this list and **not** in the one above — so a scan that
    // reports the tracks but not the disc is not a contradiction, it is the collection rule.
    let discs = slot_media::discs_dir(std::path::Path::new(&dir));
    println!();
    println!("{} discs", discs.len());
    println!();
    println!("{:>5}  {:>2}  {:<34} {}", "time", "L", "disc", "tracks");
    for t in discs.iter().take(45) {
        println!(
            "{:>5}  {:>2}  {:<34} {}",
            t.clock(),
            t.initial,
            t.name,
            t.tracks.len()
        );
    }
    if discs.len() > 45 {
        println!("  … and {} more", discs.len() - 45);
    }
}
