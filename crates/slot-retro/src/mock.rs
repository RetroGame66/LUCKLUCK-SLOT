use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::core::{AvInfo, ButtonMask, CoreError, RetroCore, GBA_H, GBA_W};

const FPS: f64 = 59.7275;
const SAMPLE_RATE: f64 = 32768.0;
const TONE_HZ: f64 = 440.0;
const AMPLITUDE: f64 = 6000.0;
const SRAM_LEN: usize = 8 * 1024;

/// Total samples the tone has produced by the start of `frame`. Deriving the count from the
/// frame number rather than accumulating per frame is what keeps the mock free of the
/// half-sample-per-frame drift a naive `SAMPLE_RATE / FPS` would collect.
fn samples_at(frame: u64) -> u64 {
    (frame as f64 * SAMPLE_RATE / FPS) as u64
}

/// The codes a `MockCore` was handed through `RetroCore::set_cheat`, in the order they arrived,
/// each with the `enabled` flag it was called with. Live in a shared cell so a test can read them
/// off a core the emulator worker owns.
pub type CheatLog = Arc<Mutex<Vec<(bool, String)>>>;

pub struct MockCore {
    frame: u64,
    video: Vec<u8>,
    audio: Vec<i16>,
    sram: Option<Vec<u8>>,
    /// Every `set_cheat` call, and how many `cheat_reset`s came before it. The reset count is
    /// separate from the log because the two together are what the frontend's push protocol
    /// promises: one clear, then the live codes only.
    cheats: CheatLog,
    cheat_resets: Arc<Mutex<usize>>,
}

impl Default for MockCore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockCore {
    pub fn new() -> Self {
        let mut c = MockCore {
            frame: 0,
            video: vec![0; (GBA_W * GBA_H * 4) as usize],
            audio: Vec::new(),
            sram: None,
            cheats: Arc::new(Mutex::new(Vec::new())),
            cheat_resets: Arc::new(Mutex::new(0)),
        };
        c.render();
        c
    }

    /// The codes this core received, in arrival order, as `(enabled, code)`.
    ///
    /// Note the shape a real core gives these: mGBA's `retro_cheat_set` ignores the `enabled`
    /// argument (`UNUSED(enabled)` in libretro/mgba's source) and applies every code it is
    /// handed, so what matters for whether a code is live is **whether it was sent at all**.
    /// The frontend filters before sending; this log is how a test proves it did.
    pub fn cheats(&self) -> Vec<(bool, String)> {
        self.cheats.lock().expect("cheats").clone()
    }

    /// How many times the core was told to clear its cheat table.
    pub fn cheat_resets(&self) -> usize {
        *self.cheat_resets.lock().expect("cheat resets")
    }

    /// Purely a function of the frame counter, so `unserialize` can regenerate the exact
    /// frame from the counter alone.
    fn render(&mut self) {
        let t = self.frame as u32;
        let mut i = 0;
        for y in 0..GBA_H {
            let g = (y * 255 / (GBA_H - 1)) as u8;
            for x in 0..GBA_W {
                let r = x.wrapping_add(t) as u8;
                let b = if (x.wrapping_add(y).wrapping_add(t) / 8) % 2 == 0 {
                    0
                } else {
                    255
                };
                self.video[i] = b;
                self.video[i + 1] = g;
                self.video[i + 2] = r;
                self.video[i + 3] = 0;
                i += 4;
            }
        }
    }
}

impl RetroCore for MockCore {
    fn load(&mut self, _rom: &Path) -> Result<(), CoreError> {
        self.frame = 0;
        self.audio.clear();
        self.sram = Some(vec![0; SRAM_LEN]);
        self.render();
        Ok(())
    }

    fn run_frame(&mut self, _input: ButtonMask) {
        let start = samples_at(self.frame);
        self.frame += 1;
        let end = samples_at(self.frame);
        self.audio.reserve(((end - start) * 2) as usize);
        for n in start..end {
            let v = (std::f64::consts::TAU * TONE_HZ * n as f64 / SAMPLE_RATE).sin() * AMPLITUDE;
            let s = v as i16;
            self.audio.push(s);
            self.audio.push(s);
        }
        self.render();
    }

    fn video_xrgb8888(&self) -> &[u8] {
        &self.video
    }

    fn take_audio(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.audio)
    }

    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        Ok(self.frame.to_le_bytes().to_vec())
    }

    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        let bytes: [u8; 8] = data
            .try_into()
            .map_err(|_| CoreError::State(format!("expected 8 bytes, got {}", data.len())))?;
        self.frame = u64::from_le_bytes(bytes);
        self.audio.clear();
        self.render();
        Ok(())
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        self.sram.clone()
    }

    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        match &mut self.sram {
            Some(s) if s.len() == data.len() => {
                s.copy_from_slice(data);
                Ok(())
            }
            Some(s) => Err(CoreError::State(format!(
                "save ram is {} bytes, got {}",
                s.len(),
                data.len()
            ))),
            None => Err(CoreError::State("no rom loaded".into())),
        }
    }

    fn av_info(&self) -> AvInfo {
        AvInfo {
            fps: FPS,
            sample_rate: SAMPLE_RATE,
            width: GBA_W,
            height: GBA_H,
        }
    }

    fn set_cheat(&self, _index: u32, enabled: bool, code: &str) {
        self.cheats
            .lock()
            .expect("cheats")
            .push((enabled, code.to_string()));
    }

    fn cheat_reset(&self) {
        *self.cheat_resets.lock().expect("cheat resets") += 1;
        self.cheats.lock().expect("cheats").clear();
    }
}
