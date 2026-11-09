use slot_gfx::OUT_W;

use crate::hud::PLATE_H;
use crate::icon::{haloed, HALO_PX};
use crate::lang;
use crate::palette;
use crate::text;
use crate::CartFace;

/// Everything the HUD ever says in words. One line, and never a third: a toast is
/// confirmation of something the user just did, never a message.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Toast {
    StateSaved,
    StateLoaded,
    CheatsOn,
    CheatsOff,
    /// The three audio latency profiles, shown on the shelf when SELECT+VOL steps them. They
    /// are the only confirmation that setting has: it changes no pixel of the picture, so
    /// without the line there is nothing on screen to say it moved.
    AudioStable,
    AudioBalanced,
    AudioStrict,
    /// The colouring ramps, when SELECT+R1 steps them on the shelf. A look changes every
    /// pixel on the screen, but *which* look it changed to is not readable off the colours
    /// alone — twelve ramps and the names are Latin, so the line is what says which one is up.
    LookChanged,
    /// ★ 2026-10-20：慢放（L2 双击锁存）开/关。半速的画面和全速的画面**一眼看不出区别** ——
    ///   画面照旧每个 present 都在出，变的是游戏里的时间 —— 所以这两句是那颗键唯一的回执，
    ///   没有它这颗键等于按下去没反应。
    SlowOn,
    SlowOff,
    /// ★ 2026-10-20：开机画面写成了 / 没写成。**这两句是那一行唯一的回执** ——
    ///   写进去的图是机器通电那一刻才读的，当下屏上不会有半点变化，
    ///   不说一句，这颗键就等于按下去没反应。
    BootSet,
    BootFailed,
}

impl Toast {
    pub const ALL: [Toast; 12] = [
        Toast::StateSaved,
        Toast::StateLoaded,
        Toast::CheatsOn,
        Toast::CheatsOff,
        Toast::AudioStable,
        Toast::AudioBalanced,
        Toast::AudioStrict,
        Toast::LookChanged,
        Toast::SlowOn,
        Toast::SlowOff,
        Toast::BootSet,
        Toast::BootFailed,
    ];

    /// Position in `ALL`, which is the order faces are uploaded in.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            Toast::StateSaved => lang::TOAST_STATE_SAVED,
            Toast::StateLoaded => lang::TOAST_STATE_LOADED,
            Toast::CheatsOn => lang::TOAST_CHEATS_ON,
            Toast::CheatsOff => lang::TOAST_CHEATS_OFF,
            Toast::AudioStable => lang::TOAST_AUDIO_STABLE,
            Toast::AudioBalanced => lang::TOAST_AUDIO_BALANCED,
            Toast::AudioStrict => lang::TOAST_AUDIO_STRICT,
            // The only toast that is not a constant: which look is up is the whole of what it
            // has to say, and the name is the mode's row's business. The face is re-rasterised
            // by the same rebake the look triggers, so the line is the new look's by the time
            // it fades in — `cycle_look` sets the look *before* raising the flag.
            Toast::LookChanged => look_line().leak(),
            Toast::SlowOn => lang::TOAST_SLOW_ON,
            Toast::SlowOff => lang::TOAST_SLOW_OFF,
            Toast::BootSet => lang::TOAST_BOOT_SET,
            Toast::BootFailed => lang::TOAST_BOOT_FAIL,
        }
    }
}

/// The look toast's line, in the language the build ships in. The leak is bounded: thirteen
/// names, each leaked once per look change, which is nothing next to the faces themselves.
#[cfg(feature = "lang-en")]
fn look_line() -> String {
    format!(
        "Look: {}",
        crate::palette::look_name(crate::palette::look())
    )
}

#[cfg(not(feature = "lang-en"))]
fn look_line() -> String {
    format!(
        "配色：{}",
        crate::palette::look_name(crate::palette::look())
    )
}

/// One box for both strings, so which one it is never moves the line.
const TOAST_W: u32 = 220;
const TOAST_H: u32 = 22;
const TOAST_PX: f32 = 16.0;
const TOAST_MIN_PX: f32 = 12.0;

/// Where the line lands, in offscreen pixels.
pub fn toast_rect() -> (f32, f32, f32, f32) {
    let (w, h) = toast_box();
    let (w, h) = (w as f32, h as f32);
    ((OUT_W as f32 - w) / 2.0, (PLATE_H - h) / 2.0, w, h)
}

/// The box every toast is rastered into, for callers laying the row out before they know
/// which string it will hold.
pub fn toast_box() -> (u32, u32) {
    (TOAST_W + 2 * HALO_PX, TOAST_H + 2 * HALO_PX)
}

/// Transparent apart from the type and the halo dilated out of it. Nothing backs a toast, so
/// the halo is the only thing keeping it legible over a bright game frame.
pub fn toast_face(toast: Toast) -> CartFace {
    let Some(font) = text::label_font() else {
        return CartFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let layout = text::fit(
        font,
        toast.text(),
        TOAST_W as f32,
        1,
        TOAST_PX,
        TOAST_MIN_PX,
    );
    let cov = text::coverage(TOAST_W, TOAST_H, &layout);
    haloed(&cov, TOAST_W, TOAST_H, palette::ink())
}
