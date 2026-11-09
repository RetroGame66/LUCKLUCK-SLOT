use std::ffi::c_void;
use std::fmt;

/// ★ 2026-10-28：**本树是 640×480 专用构建。**
///
/// `slot-main`（720×480）与 `slot-main-640`（本树，640×480）是两份**互不共享 git 历史**
/// 的独立树；两个版本的整屏几何在这里分岔，从此各自演化，谁的改动都不会波及对方。
///
/// 640×480 是 H700 家族里机型最多的面板（8 台：35XX Plus / 35XX SP / 28XX / 40XX H …），
/// 4:3 的比例也让 320×224、256×224 这一批主机帧**横向正好填满**而不是两侧留黑 ——
/// 所以这一档不是"720 缩一缩"，它有自己的最优解。
///
/// ⚠️ 改这两个数会同时移动**所有**派生量：`scale_for` 的整数倍、`screen_base` 的画面矩形、
/// `cell_size`/`mask_scale` 推给 shader 的格子几何、以及 49 个文件里 391 处 `OUT_W/OUT_H`
/// 引用。布局常量那一层（`slot-ui`）要按 640 逐条重算，不是自动跟随。
pub const OUT_W: u32 = 640;
pub const OUT_H: u32 = 480;

#[derive(Debug)]
pub enum GfxError {
    Context(String),
    Shader(String),
    Framebuffer(u32),
}

impl fmt::Display for GfxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GfxError::Context(m) => write!(f, "gl context: {m}"),
            GfxError::Shader(m) => write!(f, "shader: {m}"),
            GfxError::Framebuffer(s) => write!(f, "framebuffer incomplete: 0x{s:x}"),
        }
    }
}

impl std::error::Error for GfxError {}

pub trait Surface {
    fn make_current(&mut self) -> Result<(), GfxError>;
    fn window_size(&self) -> (u32, u32);
    fn swap(&mut self) -> Result<(), GfxError>;
    fn proc_address(&self, name: &str) -> *const c_void;
}

/// Largest whole multiple of the 720x480 output that fits in the window, floored at 1.
/// A fractional blit would resample the LCD3x mask and destroy its per pixel phase, so
/// undersized windows crop rather than shrink.
pub fn fit_scale(win_w: u32, win_h: u32) -> u32 {
    (win_w / OUT_W).min(win_h / OUT_H).max(1)
}

/// Origin and size of the blit rect inside a window, centred with black bars.
pub fn fit_rect(win_w: u32, win_h: u32) -> (i32, i32, i32, i32) {
    let s = fit_scale(win_w, win_h) as i32;
    let (w, h) = (OUT_W as i32 * s, OUT_H as i32 * s);
    ((win_w as i32 - w) / 2, (win_h as i32 - h) / 2, w, h)
}

/// The composite scaled to fit, aspect preserved, for a panel too small to hold it whole.
/// The scale is fractional, so the mask is resampled and the picture is soft. That is the
/// bring-up trade on a 640x480 device panel, where the integer path would crop 80 px of
/// chrome off the sides instead.
pub fn blit_rect_fit(panel: (u32, u32), shake: f32) -> (i32, i32, i32, i32) {
    let scale = (panel.0 as f32 / OUT_W as f32).min(panel.1 as f32 / OUT_H as f32);
    let w = (OUT_W as f32 * scale).round() as i32;
    let h = (OUT_H as f32 * scale).round() as i32;
    let dx = (shake * scale).round() as i32;
    (
        (panel.0 as i32 - w) / 2 + dx,
        (panel.1 as i32 - h) / 2,
        w,
        h,
    )
}

/// The rect the composite is presented in, displaced by a refusal. `shake` is in offscreen
/// pixels and is multiplied by the blit scale, so the flinch is the same fraction of the
/// picture at any size. Horizontal only: side to side is the gesture that means no, and
/// adding the other axis would make it a rumble.
pub fn blit_rect(window: (u32, u32), shake: f32) -> (i32, i32, i32, i32) {
    // Below native there is no whole multiple left to crop to, so a target that cannot hold
    // the composite shows all of it softly rather than part of it sharply.
    if window.0 < OUT_W || window.1 < OUT_H {
        return blit_rect_fit(window, shake);
    }
    let (x, y, w, h) = fit_rect(window.0, window.1);
    let dx = shake * fit_scale(window.0, window.1) as f32;
    (x + dx.round() as i32, y, w, h)
}
