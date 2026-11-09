//! 整机上电第二屏：BOOTLOGO 之后、货架首帧之前的一段逐帧动画。
//!
//! 素材是 `.slotvid`（见 `_tools/pack_slot_video.py`）：64 字节头 + 每帧 16 字节索引 +
//! 连续 [帧字节][该帧 PCM 段]。前端不解码视频，只逐帧解一张 JPEG 并把该帧的 PCM 段喂给
//! 音频 ring —— 帧号就是音频位置，不需要运行时同步算法。
//!
//! 两条时钟：**有音轨**时先 `push_blocking`（ring 满会阻塞到设备消费，天然按硬件音频时钟
//! 节流）；**静音包 / 音频没起来**时不能靠 `push_blocking`（`!consuming` 会退化成直接丢
//! 样本、不阻塞），所以两种情形都用绝对帧时刻 `t0 + i/fps` 兜底，掉帧不累积漂移也不会飞跑。
//!
//! 位置与「按 A 进游戏」的机种开机转场是两回事，别复用 `System/Carts/boot_anim/`。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slot_gfx::{Compositor, Draw, FbdevSurface, Surface};

use crate::audio::Ring;

const MAGIC: &[u8; 8] = b"SLOTVID\x01";
const HEADER: usize = 64;
const INDEX_ENTRY: usize = 16;
/// A card is untrusted input. A header claiming 65535 frames would otherwise try to allocate
/// an index for a file that cannot possibly hold it.
const MAX_FRAMES: u32 = 4096;
const MAX_PIXELS: u32 = 720 * 480 * 2;

#[derive(Debug)]
pub struct Stats {
    pub frames: u32,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub audio: bool,
    pub audio_rate: u32,
    pub ring_rate: u32,
    pub elapsed_ms: u128,
    /// 末帧解码后的 RGBA。收尾的交叉淡出要用它另建一张独立纹理，
    /// 由调用方（device_app）在主循环前若干帧里渐隐并释放。
    pub last_rgba: Vec<u8>,
    pub last_w: u32,
    pub last_h: u32,
}

#[derive(Clone, Copy)]
struct Entry {
    off: u64,
    len: u32,
    poff: u64,
    plen: u32,
}

struct Clip {
    file: File,
    width: u32,
    height: u32,
    frames: u32,
    fps: u32,
    rate: u32,
    pcm_samples: u32,
    index: Vec<Entry>,
}

fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

impl Clip {
    fn open(path: &Path) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| format!("open: {e}"))?;
        let size = file.metadata().map_err(|e| format!("stat: {e}"))?.len();
        if size < HEADER as u64 {
            return Err("file shorter than header".into());
        }
        let mut h = [0u8; HEADER];
        file.read_exact(&mut h).map_err(|e| format!("header: {e}"))?;
        if &h[0..8] != MAGIC {
            return Err("bad magic".into());
        }
        if u32le(&h, 8) != 1 {
            return Err("unsupported version".into());
        }
        let width = u32le(&h, 12);
        let height = u32le(&h, 16);
        let frames = u32le(&h, 20);
        let fps_num = u32le(&h, 24);
        let fps_den = u32le(&h, 28);
        let rate = u32le(&h, 32);
        let channels = u32le(&h, 36);
        let pcm_samples = u32le(&h, 40);
        let idx_off = u32le(&h, 44);
        let blob_off = u32le(&h, 48);
        let file_size = u32le(&h, 52);
        let codec = u32le(&h, 56);

        if frames == 0 || frames > MAX_FRAMES || fps_num == 0 || fps_den != 1 {
            return Err("bad frames/fps".into());
        }
        if width == 0 || height == 0 || width.saturating_mul(height) > MAX_PIXELS {
            return Err("bad frame geometry".into());
        }
        if idx_off as usize != HEADER
            || blob_off as u64 != HEADER as u64 + INDEX_ENTRY as u64 * frames as u64
            || file_size as u64 != size
        {
            return Err("inconsistent offsets".into());
        }
        // PNG 无损帧包（机种转场用）在格式层已产出，但运行时解码这一侧还没接线。
        // 明确拒绝比把 PNG 字节喂给 JPEG 解码器、画出一屏花屏要好。
        if codec != 0 {
            return Err("codec=1 (PNG) runtime decode not wired yet".into());
        }
        if channels != 2 || (pcm_samples > 0 && rate == 0) {
            return Err("bad audio description".into());
        }

        let mut raw = vec![0u8; INDEX_ENTRY * frames as usize];
        file.seek(SeekFrom::Start(idx_off as u64))
            .map_err(|e| format!("index seek: {e}"))?;
        file.read_exact(&mut raw)
            .map_err(|e| format!("index: {e}"))?;
        let mut index = Vec::with_capacity(frames as usize);
        for i in 0..frames as usize {
            let p = i * INDEX_ENTRY;
            let (off, len, poff, plen) = (
                u32le(&raw, p) as u64,
                u32le(&raw, p + 4),
                u32le(&raw, p + 8) as u64,
                u32le(&raw, p + 12),
            );
            if off < blob_off as u64
                || off.saturating_add(len as u64) > size
                || (pcm_samples > 0 && poff.saturating_add(plen as u64) > size)
            {
                return Err(format!("frame {i} offset out of range"));
            }
            index.push(Entry { off, len, poff, plen });
        }
        Ok(Self {
            file, width, height, frames, fps: fps_num, rate, pcm_samples, index,
        })
    }

    fn frame(&mut self, i: u32) -> Result<(Vec<u8>, Vec<i16>), String> {
        let e = *self.index.get(i as usize).ok_or("frame index out of range")?;
        let mut jpeg = vec![0u8; e.len as usize];
        self.file.seek(SeekFrom::Start(e.off)).map_err(|_| "jpeg seek")?;
        self.file.read_exact(&mut jpeg).map_err(|_| "jpeg read")?;
        let mut pcm = Vec::new();
        if self.pcm_samples > 0 && e.plen > 0 {
            let mut bytes = vec![0u8; e.plen as usize];
            self.file.seek(SeekFrom::Start(e.poff)).map_err(|_| "pcm seek")?;
            self.file.read_exact(&mut bytes).map_err(|_| "pcm read")?;
            pcm.reserve(bytes.len() / 2);
            for pair in bytes.chunks_exact(2) {
                pcm.push(i16::from_le_bytes([pair[0], pair[1]]));
            }
        }
        Ok((jpeg, pcm))
    }
}

/// Straight RGBA, top-down, matching what `Compositor::create_texture` expects.
fn decode_jpeg(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut dec = zune_jpeg::JpegDecoder::new_with_options(std::io::Cursor::new(bytes), options);
    dec.decode_headers().map_err(|_| "jpeg headers")?;
    let info = dec.info().ok_or("jpeg info")?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    if w == 0 || h == 0 || w.saturating_mul(h) > MAX_PIXELS {
        return Err("jpeg geometry".into());
    }
    let mut rgba = vec![255u8; (w as usize) * (h as usize) * 4];
    dec.decode_into(&mut rgba).map_err(|_| "jpeg decode")?;
    Ok((rgba, w, h))
}

/// Returns `Err` when the card has no clip or the clip is unusable; the caller then falls
/// back to the ordinary boot sound and first shelf frame. A missing animation must never
/// stop the machine from booting.
pub fn play(
    compositor: &mut Compositor,
    surface: &mut FbdevSurface,
    ring: Arc<Ring>,
    path: &Path,
) -> Result<Stats, String> {
    let mut clip = Clip::open(path)?;
    let window = surface.window_size();
    let ring_rate = ring.sample_rate();
    let audio = clip.pcm_samples > 0;
    let step = Duration::from_secs_f64(1.0 / clip.fps as f64);

    let (first, fw, fh) = decode_jpeg(&clip.frame(0)?.0)?;
    let tex = compositor.create_texture(fw, fh, &first);
    let t0 = Instant::now();
    let mut drawn = 0u32;
    // 末帧留一份 RGBA 给收尾淡出。只在最后一帧替换，避免每帧都多留 1.4 MB。
    let mut last: Option<(Vec<u8>, u32, u32)> = Some((first, fw, fh));

    for i in 0..clip.frames {
        let (jpeg, pcm) = clip.frame(i)?;
        if audio && !pcm.is_empty() {
            // Blocks while the device consumes; when the sink is not consuming it returns
            // immediately and the wall-clock deadline below still keeps the pace.
            ring.push_blocking(&pcm);
        }
        if i > 0 {
            let (rgba, w, h) = decode_jpeg(&jpeg)?;
            compositor.update_texture(tex, w, h, &rgba);
            if i + 1 == clip.frames {
                last = Some((rgba, w, h));
            }
        }
        let draws = [Draw::Tex {
            x: 0.0,
            y: 0.0,
            w: clip.width as f32,
            h: clip.height as f32,
            tex,
            alpha: 1.0,
        }];
        compositor.begin_frame();
        compositor.draw_list(&draws);
        compositor.end_frame(window);
        surface.swap().map_err(|e| format!("swap frame {i}: {e}"))?;
        drawn += 1;

        let deadline = t0 + step * i.saturating_add(1);
        std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
    }
    compositor.release_texture(tex);

    let (last_rgba, last_w, last_h) = last.unwrap_or_default();
    Ok(Stats {
        frames: drawn,
        fps: clip.fps,
        width: clip.width,
        height: clip.height,
        audio,
        audio_rate: clip.rate,
        ring_rate,
        elapsed_ms: t0.elapsed().as_millis(),
        last_rgba,
        last_w,
        last_h,
    })
}
