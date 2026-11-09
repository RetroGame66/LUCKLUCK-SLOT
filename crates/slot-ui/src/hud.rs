use slot_gfx::{Draw, TexId, OUT_W};
use slot_store::{BLUE_LIGHT_MAX, BRIGHTNESS_MAX, VOLUME_MAX};

use crate::icon::icon_box;
use crate::toast::toast_rect;
use crate::{Icon, Toast};

/// Milliseconds on the same monotonic clock the gesture machines run on. Spelled again
/// here because `slot-ui` cannot see `slot-input` and must not learn to.
pub type Millis = u64;

pub const HUD_MS: Millis = 1500;
/// The tail of that window, spent ramping out. A hard cut at 1500 ms reads as a glitch.
const FADE_MS: Millis = 250;

/// The bar has no backing of its own, so over a bright frame a white fill on a translucent
/// white track vanishes. The plate is what it is read against. Shared with the refusal and
/// with the switcher's two plates, so everything that ever sits on top of a picture reads as
/// one system.
pub const PLATE_H: f32 = 40.0;

/// Where the plate hangs when there is a letter band above it: flush against that band's lower
/// edge, so the dark starts exactly where the case stops.
///
/// The plate used to start hard at the top of the screen, which was right for a screen with
/// nothing at the top of it. On the shelf there is now a band there, with the index printed
/// along it and the clock and battery on its ends, and a plate over all of that for the second
/// and a half a level is up hides the very things the band is for. Under the band it covers
/// only the picture, which is what a plate is for.
///
/// It is the band's own height rather than a second number, so the two cannot drift apart.
pub const PLATE_Y: f32 = crate::slot_chrome::TOP_BAND_H;

/// ★ 2026-11-03（六六）：「你要不货架界面的 HUD 宽度缩到 360px 吧，跟插卡位同框可能好看点；
/// 高度加一点，两边改圆角」→ 同日再改：「放插卡位是不是更自然？反正就是一两秒的事」。
///
/// 于是这块板**住进插卡口里**：宽取插口宽、高取插口高（`SLOT_OPEN_H`，720/640 两树都是 44），
/// 底边压在插口下沿上。它只在电平/提示出来那 1.5 秒存在，盖住的是洞口而不是机壳。
/// 高度写成插口自己的常量而不是另给一个数，两块东西才不可能漂开。
/// **全屏相位（游戏里的音量/快进）仍然用 `PLATE_H`，一个像素都没动。**
pub const SHELF_PLATE_H: f32 = crate::desktop::SLOT_OPEN_H;

/// 窄板两端的圆角半径。取 12 而不是 `h / 2`：胶囊的两端是半圆，上下两条边会弯，
/// 而这块板的**底边要正对插卡口上沿**，上下两边得是一条直线。
pub const SHELF_PLATE_R: f32 = 12.0;

/// Where the HUD's plate stands this frame: its box, and how round its two ends are.
///
/// ★ 2026-11-03（六六）：这是「板」第一次拥有自己的几何。之前板就是整条 `OUT_W`，
/// 所以它只需要一个 `top`。货架上要跟插卡位同框（`SLOT_OPEN_X/W`，360 宽），
/// 于是宽、高、圆角都成了必须告诉它的东西，`top` 一个人不够了。
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Plate {
    pub x: f32,
    pub w: f32,
    pub h: f32,
    /// Two ends only: 0 is the square full-width plate, `SHELF_PLATE_R` the shelf's.
    pub radius: f32,
}

impl Plate {
    /// The plate every phase but the shelf draws: edge to edge, square, `PLATE_H` tall.
    pub const fn full() -> Self {
        Self {
            x: 0.0,
            w: OUT_W as f32,
            h: PLATE_H,
            radius: 0.0,
        }
    }

    /// ★ 2026-11-03（六六）：「宽度缩到 360px，跟插卡位同框；高度加一点，两边改圆角」
    ///   → 同日：「放插卡位是不是更自然？」。
    ///
    /// 宽高都取插口自己的常量（`SLOT_OPEN_W` / `SLOT_OPEN_H`），x 取 `SLOT_OPEN_X`，
    /// 所以板**就是**那个口，换分辨率也不会各走各的。
    pub const fn shelf() -> Self {
        Self {
            x: crate::desktop::SLOT_OPEN_X,
            w: crate::desktop::SLOT_OPEN_W,
            h: SHELF_PLATE_H,
            radius: SHELF_PLATE_R,
        }
    }

    pub fn cx(&self) -> f32 {
        self.x + self.w * 0.5
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// The narrow plate. The full-width one keeps the layout it has always had — the bar
    /// centred on the screen, the badge in the corner — so nothing about the in-game readout
    /// moves because the shelf changed.
    fn narrow(&self) -> bool {
        self.w < OUT_W as f32 - 0.5
    }
}

pub const HUD_ICON_PX: f32 = 24.0;
const ICON_GAP: f32 = 10.0;

/// The badge sits in the corner the bar never reaches, so the two never have to negotiate.
const BADGE_MARGIN: f32 = 12.0;

const BAR_W: f32 = 320.0;
const BAR_H: f32 = 6.0;
const TRACK: [f32; 4] = [1.0, 1.0, 1.0, 0.18];

/// The bar's fill. The palette's ink rather than a colour of its own, which is what the glyph
/// beside it is drawn in and what every letter on the shelf is set in: one ink per mode, and
/// the HUD is not a second one.
///
/// The track stays translucent white in both modes, and it is the one value in here that does
/// not flip. It is not type, it is the bar a fill runs along, and the plate under it is black
/// at 0.72 in the dark and would be the case's own light in the light — a track that matched
/// the plate would stop being a track.
fn fill() -> [f32; 4] {
    crate::palette::ink_f()
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub enum HudKind {
    #[default]
    Brightness,
    BlueLight,
    Volume,
    /// Not a level: the value is how much rewind history is left, as a percentage.
    Rewind,
}

impl HudKind {
    /// Silence is a state, not a low level: a bar at zero looks the same as a bar that has
    /// not been dragged down yet, so the glyph is what has to say it.
    pub fn icon(self, value: u8, muted: bool) -> Icon {
        match self {
            HudKind::Brightness => Icon::Brightness,
            HudKind::BlueLight => Icon::BlueLight,
            HudKind::Volume if muted => Icon::VolumeMuted,
            HudKind::Volume if value == 0 => Icon::VolumeZero,
            HudKind::Volume => Icon::Volume,
            HudKind::Rewind => Icon::Rewind,
        }
    }

    fn max(self) -> u8 {
        match self {
            HudKind::Brightness => BRIGHTNESS_MAX,
            HudKind::BlueLight => BLUE_LIGHT_MAX,
            HudKind::Volume => VOLUME_MAX,
            HudKind::Rewind => 100,
        }
    }
}

/// Fast forward is a mode rather than a moment, so the badge is state the HUD is told and
/// holds, not something it starts a clock on.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub enum FfState {
    #[default]
    Off,
    Held,
    Latched,
}

/// One glyph, never two. The outline and solid variants are the same shape at the same
/// width, so a latch neither widens the badge nor moves it.
pub fn ff_badge(state: FfState) -> Option<Icon> {
    match state {
        FfState::Off => None,
        FfState::Held => Some(Icon::FastForward),
        FfState::Latched => Some(Icon::FastForwardLatched),
    }
}

/// One bar for all three levels, drawn identically whichever it is: the user knows what
/// they just pressed, and three styles would read as three different controls.
#[derive(Default)]
pub struct Hud {
    pub kind: HudKind,
    pub value: u8,
    /// `None` until the first adjustment. A zero here would put a bar on screen for the
    /// first 1.5 s of every boot.
    pub shown_at: Option<Millis>,
    /// The rewind bar answers to the button, not to the clock, so while this is set the fade
    /// never starts.
    held: bool,
    /// Silenced rather than turned down. The bar is empty either way, so this is the only
    /// thing that can tell the two apart on screen.
    muted: bool,
    ff: FfState,
    /// The last thing the HUD said, and when. On its own clock rather than the bar's: the
    /// two are triggered by different actions and either can outlive the other.
    said: Option<(Toast, Millis)>,
    /// One face per icon, in `Icon::ALL` order. Empty until the binary uploads them, since
    /// only the compositor can mint a `TexId`.
    icons: Vec<TexId>,
    /// One face per toast, in `Toast::ALL` order.
    toasts: Vec<TexId>,
}

impl Hud {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_icons(&mut self, icons: Vec<TexId>) {
        self.icons = icons;
    }

    /// In `Toast::ALL` order.
    pub fn set_toasts(&mut self, toasts: Vec<TexId>) {
        self.toasts = toasts;
    }

    pub fn toast(&mut self, toast: Toast, now: Millis) {
        self.said = Some((toast, now));
    }

    pub fn toast_visible(&self, now: Millis) -> bool {
        self.toast_alpha(now) > 0.0
    }

    /// What the HUD is saying, if anything. The app reads this rather than tracking the
    /// grace period twice.
    pub fn said(&self, now: Millis) -> Option<Toast> {
        self.toast_visible(now)
            .then(|| self.said.map(|(t, _)| t))
            .flatten()
    }

    pub fn show(&mut self, kind: HudKind, value: u8, muted: bool, now: Millis) {
        self.kind = kind;
        self.value = value;
        self.muted = muted;
        self.shown_at = Some(now);
        self.held = kind == HudKind::Rewind;
    }

    /// L2 let go. A level bar shown during the rewind is on its own timer and stays.
    pub fn release_rewind(&mut self) {
        if self.kind == HudKind::Rewind {
            self.shown_at = None;
        }
        self.held = false;
    }

    /// The glyph on the bar, whether or not a face for it has been uploaded.
    pub fn glyph(&self) -> Icon {
        self.kind.icon(self.value, self.muted)
    }

    /// The glyph the badge is showing, which is not the same question as what reached the
    /// screen: without an uploaded face it draws nothing at all.
    pub fn badge(&self) -> Option<Icon> {
        ff_badge(self.ff)
    }

    /// One glyph's face, whether or not the HUD is showing it. Keyed by the icon rather than by
    /// the kind, because the one caller that is not a level is the status row's mode badge:
    /// `Icon::of_mode` says which of the two it wants and the list is in `Icon::ALL` order.
    pub fn face(&self, icon: Icon) -> Option<TexId> {
        self.icons.get(icon.index()).copied()
    }

    /// Pushed from outside because the badge answers to the gesture machine's latch, which
    /// nothing on this side of the app can see.
    pub fn set_ff(&mut self, ff: FfState) {
        self.ff = ff;
    }

    pub fn visible(&self, now: Millis) -> bool {
        self.alpha(now) > 0.0
    }

    /// `top` is where the plate hangs, and it is the only number that has to be told: the bar,
    /// the toast and the badge are all laid out *inside* the plate, so moving the plate moves
    /// the whole readout. Callers pass `PLATE_Y` on a screen that has the letter band at the
    /// top and zero on one that does not.
    pub fn draw(&self, now: Millis, top: f32, plate: Plate, out: &mut Vec<Draw>) {
        let alpha = self.alpha(now);
        let toast = self.toast_alpha(now);
        // The plate belongs to whatever is being read against it, bar or toast. The badge
        // never gets one: fast forward can be latched for minutes, and a full width plate
        // over the game for all of it is a worse trade than the halo it carries.
        if alpha > 0.0 || toast > 0.0 {
            // ★ 2026-11-03（六六）：「背景不透明」—— 板住进插卡口之后，0.72 的纱会把洞里的
            //   卡带、盘面光影透出来，读数压不住。货架这块改成实底；全屏相位仍是那块
            //   半透明的纱（游戏画面要透出来一点，否则盖在游戏上像一块砖）。
            //   淡出照旧走 `faded`，所以退出时还是渐隐，不是硬切。
            let mut base = crate::palette::plate();
            if plate.narrow() {
                base[3] = 1.0;
            }
            push_plate(plate, top, faded(base, alpha.max(toast)), out);
        }
        // They share the band, so only one at a time. A toast is a specific thing that just
        // happened and outranks a level the user can see the effect of anyway.
        if toast > 0.0 {
            self.draw_toast(top, plate, now, out);
        } else if alpha > 0.0 {
            self.draw_bar(top, plate, alpha, out);
        }
        self.draw_badge(top, plate, ff_badge(self.ff), out);
    }

    fn draw_bar(&self, top: f32, plate: Plate, alpha: f32, out: &mut Vec<Draw>) {
        let (icon_w, icon_h) = icon_box(HUD_ICON_PX);
        let (icon_w, icon_h) = (icon_w as f32, icon_h as f32);
        // ★ 2026-11-03（六六）：货架那块板只有 360 宽，原来 320 的条加左侧图标（24 + 10）
        //   一共 354，两端只剩 3 px —— 太满。窄板上把条按可用宽度收窄，图标和条
        //   **作为一组**在板内居中，两端各留 16 px。全宽板仍是老布局，游戏里一个像素不动。
        const PAD: f32 = 16.0;
        let (icon_x, bar_x, bar_w) = if plate.narrow() {
            let room = (plate.w - 2.0 * PAD - icon_w - ICON_GAP).max(BAR_H);
            let bar_w = BAR_W.min(room);
            let group = icon_w + ICON_GAP + bar_w;
            let icon_x = plate.cx() - group * 0.5;
            (icon_x, icon_x + icon_w + ICON_GAP, bar_w)
        } else {
            let x = (OUT_W as f32 - BAR_W) / 2.0;
            (x - ICON_GAP - icon_w, x, BAR_W)
        };
        let bar_y = top + (plate.h - BAR_H) / 2.0;
        if let Some(tex) = self.icon() {
            out.push(Draw::Tex {
                x: icon_x,
                y: top + (plate.h - icon_h) / 2.0,
                w: icon_w,
                h: icon_h,
                tex,
                alpha,
            });
        }
        out.push(Draw::Rect {
            x: bar_x,
            y: bar_y,
            w: bar_w,
            h: BAR_H,
            colour: faded(TRACK, alpha),
        });
        out.push(Draw::Rect {
            x: bar_x,
            y: bar_y,
            w: bar_w * self.fraction(),
            h: BAR_H,
            colour: faded(fill(), alpha),
        });
    }

    fn draw_badge(&self, top: f32, plate: Plate, badge: Option<Icon>, out: &mut Vec<Draw>) {
        let Some(icon) = badge else {
            return;
        };
        let Some(tex) = self.icons.get(icon.index()).copied() else {
            return;
        };
        let (w, h) = icon_box(HUD_ICON_PX);
        let (w, h) = (w as f32, h as f32);
        // ★ 2026-11-03（六六）：窄板上角标贴**板**的内右沿，不再贴屏幕右缘——
        //   否则它会飘到 540 以外、落在板外面。
        let x = if plate.narrow() {
            plate.right() - BADGE_MARGIN - w
        } else {
            OUT_W as f32 - BADGE_MARGIN - w
        };
        out.push(Draw::Tex {
            x,
            y: top + (plate.h - h) / 2.0,
            w,
            h,
            tex,
            alpha: 1.0,
        });
    }

    /// No placeholder. A toast is type, and absent type is absent rather than a bar sitting
    /// where two words should be.
    fn draw_toast(&self, top: f32, plate: Plate, now: Millis, out: &mut Vec<Draw>) {
        let alpha = self.toast_alpha(now);
        if alpha <= 0.0 {
            return;
        }
        let Some(tex) = self
            .said
            .and_then(|(t, _)| self.toasts.get(t.index()))
            .copied()
        else {
            return;
        };
        // `toast_rect` places the box inside a plate standing at zero, so the plate's own top
        // is added here rather than being worked out again down there. Its y is measured
        // against `PLATE_H`, so a taller plate (the shelf's) centres it by the difference.
        // Its x is already centred on the screen, which is the same centre the shelf plate has.
        let (x, y, w, h) = toast_rect();
        out.push(Draw::Tex {
            x,
            y: top + y + (plate.h - PLATE_H) / 2.0,
            w,
            h,
            tex,
            alpha,
        });
    }

    fn icon(&self) -> Option<TexId> {
        self.icons.get(self.glyph().index()).copied()
    }

    fn fraction(&self) -> f32 {
        let max = self.kind.max();
        if max == 0 {
            return 0.0;
        }
        self.value.min(max) as f32 / max as f32
    }

    fn alpha(&self, now: Millis) -> f32 {
        let Some(shown) = self.shown_at else {
            return 0.0;
        };
        if self.held {
            return 1.0;
        }
        fade(shown, now)
    }

    fn toast_alpha(&self, now: Millis) -> f32 {
        match self.said {
            Some((_, shown)) => fade(shown, now),
            None => 0.0,
        }
    }
}

/// One curve for everything the HUD shows, so the bar and the toast leave together when they
/// arrived together.
fn fade(shown: Millis, now: Millis) -> f32 {
    let age = now.saturating_sub(shown);
    if age >= HUD_MS {
        return 0.0;
    }
    ((HUD_MS - age) as f32 / FADE_MS as f32).min(1.0)
}

/// Push the plate: one rect when its ends are square, a rounded box when they are not.
///
/// `Draw` has no rounded rectangle, so the two ends are laid in as one-pixel rows, each inset
/// by its circle at that height — twelve rows a corner for the shelf's plate, which is 24 more
/// rects. That is only ever paid on the frame a level or a toast is up (1.5 s), and nothing
/// else on that screen draws a plate, so it is not worth a new `Draw` variant and a shader for.
fn push_plate(plate: Plate, top: f32, colour: [f32; 4], out: &mut Vec<Draw>) {
    let r = plate.radius.min(plate.w * 0.5).min(plate.h * 0.5);
    if r <= 0.0 {
        out.push(Draw::Rect {
            x: plate.x,
            y: top,
            w: plate.w,
            h: plate.h,
            colour,
        });
        return;
    }
    // The straight middle: everything between the two corner curves.
    out.push(Draw::Rect {
        x: plate.x,
        y: top + r,
        w: plate.w,
        h: plate.h - 2.0 * r,
        colour,
    });
    let rows = r.ceil() as i32;
    for i in 0..rows {
        // Distance above the corner circle's centre, sampled at the middle of the row.
        let dy = (i as f32 + 0.5) - r;
        let inset = r - (r * r - dy * dy).max(0.0).sqrt();
        let w = plate.w - 2.0 * inset;
        out.push(Draw::Rect {
            x: plate.x + inset,
            y: top + i as f32,
            w,
            h: 1.0,
            colour,
        });
        out.push(Draw::Rect {
            x: plate.x + inset,
            y: top + plate.h - 1.0 - i as f32,
            w,
            h: 1.0,
            colour,
        });
    }
}

fn faded(colour: [f32; 4], alpha: f32) -> [f32; 4] {
    [colour[0], colour[1], colour[2], colour[3] * alpha]
}
