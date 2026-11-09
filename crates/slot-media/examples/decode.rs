//! Decode a track to a WAV and report what came out.
//!
//! The point is to hear it: a scan that lists the right titles proves the container was read,
//! and only a decoded file proves the audio path. Writes a plain 16-bit PCM WAV, which is a
//! header and the samples and needs no crate.
//!
//!     python host_cargo.py run --target x86_64-pc-windows-gnu -p slot-media --example decode -- <in> [out.wav]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(src) = args.first() else {
        eprintln!("usage: decode <file> [out.wav]");
        std::process::exit(2);
    };
    let out = args.get(1).cloned().unwrap_or_else(|| "out.wav".into());

    let t0 = std::time::Instant::now();
    let Some(mut source) = slot_media::Source::open(std::path::Path::new(src)) else {
        eprintln!("would not open: {src}");
        std::process::exit(1);
    };
    println!("rate {}  channels {}", source.rate(), source.channels());

    let mut samples: Vec<i16> = Vec::new();
    let mut packets = 0usize;
    while let Some(block) = source.next() {
        packets += 1;
        samples.extend_from_slice(block);
    }
    let elapsed = t0.elapsed().as_secs_f64() * 1000.0;

    let rate = source.rate();
    let frames = samples.len() / 2;
    let seconds = frames as f64 / rate as f64;
    let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    let rms = (samples.iter().map(|s| (*s as f64).powi(2)).sum::<f64>()
        / samples.len().max(1) as f64)
        .sqrt();

    println!(
        "{packets} packets, {frames} frames = {seconds:.1} s, decoded in {elapsed:.0} ms ({:.0}x realtime)",
        seconds / (elapsed / 1000.0).max(1e-9)
    );
    println!("peak {peak}  rms {rms:.0}  (of 32767)");
    if peak == 0 {
        println!("!! every sample is silent — the decode produced nothing");
    }
    write_wav(&out, &samples, rate);
    println!("wrote {out}");
}

/// A 16-bit PCM WAV. Little endian, one `fmt ` chunk and one `data` chunk, which is the whole
/// of what a player needs to be sure it is looking at audio.
fn write_wav(path: &str, samples: &[i16], rate: u32) {
    use std::io::Write;
    let bytes = (samples.len() * 2) as u32;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).expect("create"));
    let mut w = |v: &[u8]| f.write_all(v).expect("write");
    let u32le = |v: u32| v.to_le_bytes();
    let u16le = |v: u16| v.to_le_bytes();

    w(b"RIFF");
    w(&u32le(36 + bytes));
    w(b"WAVE");
    w(b"fmt ");
    w(&u32le(16));
    w(&u16le(1)); // PCM
    w(&u16le(2)); // stereo
    w(&u32le(rate));
    w(&u32le(rate * 4)); // byte rate
    w(&u16le(4)); // block align
    w(&u16le(16)); // bits
    w(b"data");
    w(&u32le(bytes));
    for s in samples {
        w(&s.to_le_bytes());
    }
}
