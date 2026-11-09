use std::io::Read;
use std::path::Path;

/// Decodes `path` and returns RGBA scaled to cover `w` by `h`, centre cropped. Cover rather
/// than fit, because a letterboxed face would break the shelf's one-set look, and cropping
/// beats squashing box art. Any decode failure is `None`; the caller draws a label instead.
pub fn cover(path: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    let (src, sw, sh) = decode(path)?;
    if sw == 0 || sh == 0 {
        return None;
    }

    let scale = (w as f32 / sw as f32).max(h as f32 / sh as f32);
    let ox = (sw as f32 - w as f32 / scale) / 2.0;
    let oy = (sh as f32 - h as f32 / scale) / 2.0;

    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let y0 = oy + y as f32 / scale;
        let y1 = oy + (y + 1) as f32 / scale;
        for x in 0..w {
            let x0 = ox + x as f32 / scale;
            let x1 = ox + (x + 1) as f32 / scale;
            let px = box_average(&src, sw, sh, x0, y0, x1, y1);
            out[((y * w + x) * 4) as usize..][..4].copy_from_slice(&px);
        }
    }
    Some(out)
}

/// Decodes `path` and returns RGBA scaled to exactly `w` by `h` — `cover` without the crop.
///
/// For art whose own shape **is** the target's, which is the contract the cart and tape
/// templates keep (a 360x202 shell for a 480x270 face): the two axes scale independently, so
/// a picture drawn at the face's shape lands on it whole instead of centre-cropped. Same box
/// averaging as `cover` — which collapses to one sample when upscaling — minus the decision
/// about what to cut.
pub fn stretch(path: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    let (src, sw, sh) = decode(path)?;
    stretch_rgba(&src, sw, sh, w, h)
}

/// The same stretch, from pixels a caller already decoded — a face that has to be scaled off a
/// resident buffer rather than read off the card (the default cart art, which is shared and
/// cannot be re-decoded per cart). `stretch` is the file's half of the operation; this is the
/// buffer's, and the two must agree about sampling or a picture would look different on the two
/// roads it can arrive by.
pub fn stretch_rgba(src: &[u8], sw: u32, sh: u32, w: u32, h: u32) -> Option<Vec<u8>> {
    if sw == 0 || sh == 0 || w == 0 || h == 0 || src.len() < (sw * sh * 4) as usize {
        return None;
    }

    let kx = sw as f32 / w as f32;
    let ky = sh as f32 / h as f32;

    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let y0 = y as f32 * ky;
        let y1 = y0 + ky;
        for x in 0..w {
            let x0 = x as f32 * kx;
            let x1 = x0 + kx;
            let px = box_average(src, sw, sh, x0, y0, x1, y1);
            out[((y * w + x) * 4) as usize..][..4].copy_from_slice(&px);
        }
    }
    Some(out)
}

/// Averages the source footprint of one destination pixel. Collapses to a single sample when
/// upscaling, so it doubles as nearest neighbour there.
fn box_average(src: &[u8], sw: u32, sh: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> [u8; 4] {
    let xa = (x0.floor().max(0.0) as u32).min(sw - 1);
    let ya = (y0.floor().max(0.0) as u32).min(sh - 1);
    let xb = ((x1.ceil() as u32).max(xa + 1)).min(sw);
    let yb = ((y1.ceil() as u32).max(ya + 1)).min(sh);

    let mut acc = [0u32; 4];
    let mut n = 0u32;
    for y in ya..yb {
        for x in xa..xb {
            let i = ((y * sw + x) * 4) as usize;
            for c in 0..4 {
                acc[c] += src[i + c] as u32;
            }
            n += 1;
        }
    }
    let mut out = [0u8; 4];
    for c in 0..4 {
        out[c] = ((acc[c] + n / 2) / n) as u8;
    }
    out
}

fn decode(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let file = std::fs::File::open(path).ok()?;
    decode_reader(std::io::BufReader::new(file))
}

/// The same decode, from bytes the binary already carries. The two faces of the about screen —
/// the sticker and the wordmark — are `include_bytes!` rather than files on a card, and nothing
/// downstream of this cares where the image came from.
pub(crate) fn decode_bytes(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    decode_reader(std::io::Cursor::new(bytes))
}

/// The cap on what either decoder may allocate, in bytes before the PNG path's own limit and as
/// a pixel count on the JPEG path. A card holds user supplied art; a hostile or broken header
/// must not become an OOM. The two numbers describe the same picture at 4 bytes a pixel, so one
/// is not more generous than the other.
const LIMIT_BYTES: usize = 64 << 20;
const LIMIT_PIXELS: usize = LIMIT_BYTES / 4;

/// Decodes a card's image, by sniffing what it is rather than trusting where it was found.
///
/// **Both formats live here because the card is allowed to hold both.** `slot_media::Covers`
/// walks `png / jpg / jpeg / webp`, and a sleeve that arrives with the music is a `.jpg` at
/// least as often as it is a `.png` — a folder of tracks with `cover.jpg` beside them is the
/// shape a digital album comes in, and it is the shape the card is holding. Reading only PNG
/// meant the index found those files, `find` returned the right path, and the face then drew an
/// **empty recess**: the art was there, nothing was logged, and the picture was simply missing.
///
/// The bytes are pulled in once and both decoders are handed a slice, because that is the one
/// form the two agree on and because sniffing a stream without consuming any of it is a second
/// thing to get right for no gain — the files are small and the read is once per face.
fn decode_reader<R: std::io::Read>(reader: R) -> Option<(Vec<u8>, u32, u32)> {
    let mut bytes = Vec::new();
    reader.take(LIMIT_BYTES as u64).read_to_end(&mut bytes).ok()?;
    // The JPEG signature, not the extension: a card can name a PNG `.jpg` and the old code
    // would have decoded it, so the sniff has to be on content to stay compatible with that.
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        decode_jpeg(&bytes)
    } else {
        decode_png(&bytes)
    }
}

fn decode_png(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut dec = png::Decoder::new(bytes);
    dec.set_transformations(png::Transformations::normalize_to_color8());
    dec.set_limits(png::Limits { bytes: LIMIT_BYTES });

    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let src = &buf[..info.buffer_size()];
    let n = (info.width * info.height) as usize;

    let mut rgba = vec![255u8; n * 4];
    match info.color_type {
        png::ColorType::Rgba => rgba.copy_from_slice(src),
        png::ColorType::Rgb => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].copy_from_slice(&src[i * 3..i * 3 + 3]);
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].fill(src[i]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].fill(src[i * 2]);
                rgba[i * 4 + 3] = src[i * 2 + 1];
            }
        }
        png::ColorType::Indexed => return None,
    }
    Some((rgba, info.width, info.height))
}

/// A JPEG, asked for RGBA so the two decoders hand back the same thing and no caller has to
/// know which format it got.
///
/// **Asked for a known output rather than reported one.** JPEG is YCbCr, greyscale, CMYK or
/// YCCK on the way in and the decoder would happily hand any of those back; the shelf only ever
/// wants straight RGBA, and a card is not the place to start caring which subsampling a camera
/// chose. `RGBA` is one of the four outputs the crate documents, and the only one that needs no
/// widening here.
///
/// The size is checked **between the headers and the pixels**, which is the only moment it can
/// be: the header carries a `u16` pair, so a few hundred bytes on the card can claim 65535 by
/// 65535, and a decoder that allocates before anyone looks is how a card takes the machine down.
fn decode_jpeg(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut dec =
        zune_jpeg::JpegDecoder::new_with_options(std::io::Cursor::new(bytes), options);
    dec.decode_headers().ok()?;

    let info = dec.info()?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    let n = (w as usize).checked_mul(h as usize)?;
    if n == 0 || n > LIMIT_PIXELS {
        return None;
    }

    // Opaque by default: JPEG carries no alpha, and a decoder that leaves the channel alone
    // should still produce a sleeve rather than a hole.
    let mut rgba = vec![255u8; n * 4];
    dec.decode_into(&mut rgba).ok()?;
    Some((rgba, w, h))
}

/// Decodes `path` to straight RGBA with no scaling, as raw cover art needs. The one entry
/// point the rest of slot uses to read an image the user dropped on the card — a sleeve
/// (`System/Carts/cd.png`, a cassette, a cart) or a screen overlay (`Overlay/GB/GB.png`,
/// `Overlay/GBC/GBC.png`) — read through this so a missing or corrupt file is simply `None`:
/// the built-in look stands, rather than an error worth a log line.
pub fn decode_rgba(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    decode(path)
}

/// The traced shape, at the size the face wants. `tiny_skia` hands back premultiplied RGBA,
/// which is the same thing straight through wherever alpha is 0 or 255, but every caller here
/// pastes this over another face or hands it to the sprite pass expecting straight alpha — so
/// the antialiased edges, the only place the two differ, are un-premultiplied before they go
/// back.
pub(crate) fn render_svg(svg: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let scale =
        resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    let mut rgba = pixmap.data().to_vec();
    unpremultiply(&mut rgba);
    Some(rgba)
}

/// Undoes `tiny_skia`'s premultiply in place. Opaque and fully transparent pixels are already
/// their own straight-alpha value, so only the antialiased edges, where `0 < a < 255`, need the
/// divide.
fn unpremultiply(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3];
        if a == 0 || a == 255 {
            continue;
        }
        for c in &mut px[..3] {
            *c = ((*c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
        }
    }
}
