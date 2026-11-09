use crate::art::decode_bytes;
use crate::lang;
use crate::plate::UndoFace;
use crate::rounded;
use crate::slot_chrome::{edge, housing};
use crate::text;

/// What a held POWER offers. Off, because that is the button's common use and the one most
/// people reach for first, and restart, which a device you develop on wants one that is not
/// "off, then find the button again" — the only other thing this hardware can honestly do.
///
/// There is no Standby. The board suspends well, under 45 mA, but it cannot wake itself: the
/// RTC alarm arms, reads back, and never fires — measured on a fully awake machine as well as
/// a suspended one, and unrelated to Super Standby, which was the first two things I blamed.
/// A standby nothing can end is a slow leak with a nicer name, so the lid and the button run
/// a timer and then power off properly instead.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PowerChoice {
    PowerOff,
    Restart,
}

impl PowerChoice {
    /// Power Off first: it is the common use of the button and the one most people reach for,
    /// so it sits at the top and is the default the A press commits to. Restart stays within
    /// easy reach for the rarer "back to the picker" case.
    pub const ALL: [PowerChoice; 2] = [PowerChoice::PowerOff, PowerChoice::Restart];

    /// Position in `ALL`, which is the order the faces are uploaded in.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            PowerChoice::Restart => lang::POWER_RESTART,
            PowerChoice::PowerOff => lang::POWER_OFF,
        }
    }
}

/// The menu is read at arm's length on a 720x480 panel while the user is deciding something
/// they cannot undo, so it is set well above the key-caption type the rest of the chrome
/// uses. The shutdown line that follows a choice is rastered at the same size: the words
/// change but the voice should not.
pub const MENU_PX: f32 = 30.0;
pub const MENU_H: u32 = 40;
/// Breathing room either side of the ink, which is also what the highlight bar is padded by
/// so the bar hugs the words rather than the panel.
pub const MENU_PAD: u32 = 18;

/// ★ 2026-10-21：**游戏菜单**那一版行的尺寸。
///
/// 比 `MENU_PX` 大一档，理由不是"游戏菜单更重要"，而是两处的读法不同：
/// 电源菜单是关机前最后一眼，扫一下就走；游戏菜单是一张要读的列表，行与行之间
/// 还得放得下手指头。
///
/// ★★ **傍晚第二版**（六六：「按钮能再精致一些吗？就像其它菜单那种有个圆角矩形
/// 按钮还带点光泽的」「七个选项字号再小一档并拉开点行距」）：
/// - 行不再是"一行字"，而是**一整格圆角按钮**（`game_row_face` 烘的位图：机壳色底 +
///   `mould` 的竖向渐变与上下棱，和键位弹窗那七行是同一套模具）；
/// - 字 **34 → 26**（小一档），按钮 **44** 高、行距 **66**（拉松），于是每一格都有自己的
///   一块面，格与格之间是空气而不是一条缝。
pub const GAME_ROW_PX: f32 = 26.0;
pub const GAME_ROW_H: u32 = 44;
pub const GAME_ROW_PAD: u32 = 20;
/// 按钮的圆角与描边。半径按 44 高配 —— 圆的太多了会读成胶囊，太少又还是块方片。
const GAME_ROW_RADIUS: f32 = 10.0;
const GAME_ROW_RIM: f32 = 1.0;
/// 游戏菜单的抬头。**比货架那张标题大字一档**（30 → 38）：板里那一格一格是 26 的字，
/// 抬头只有比它们明显大才读得出层级，否则一行标题会和第一排按钮挤成一个高度。
pub const GAME_TITLE_PX: f32 = 38.0;
pub const GAME_TITLE_MIN_PX: f32 = 20.0;
pub const GAME_TITLE_H: u32 = 50;
pub const GAME_TITLE_W: u32 = 640;

/// The menu type, in the palette's ink — and it has to be, because the ground these rows are
/// set on is `slot_chrome::opening`, which flips with the mode. A menu whose panel went light
/// while its type stayed light would be a screen with nothing on it.
fn menu_ink() -> [u8; 3] {
    crate::palette::ink()
}

/// ★ 2026-10-29（六六）：**电源菜单的选项恒定为白色**。
///
/// 这个菜单（长按电源键的关机/重启）不再画在主题底上，而是画在一张**固定的背景图**
/// （`powerbg-66mod.png`，白色+品红霓虹隧道、中央黑色）上 —— 背景不随主题/明暗，选项
/// 也不能随：亮色主题下 `ink()` 是深色，压在这张图中央的黑上会直接隐形。所以电源菜单
/// 的两个选项**恒定白**，不参与主题色板与暗色亮色模式。
fn menu_ink_white() -> [u8; 3] {
    [0xff, 0xff, 0xff]
}

/// Sized to its own text rather than to a fixed box, so a caller can put a bar behind it
/// that fits the words. A fixed width would make the bar the same size under "重启" and
/// "关机", which is the thing that looks wrong when the selection moves.
pub fn menu_face(label: &str) -> UndoFace {
    menu_face_at(label, MENU_PX, MENU_H, MENU_PAD)
}

/// ★ 2026-10-29（六六）：电源菜单的选项面 —— **恒定白色**版。
///
/// 与 `menu_face` 同一尺寸/同一排布，唯一差别是墨色写死白。见 `menu_ink_white`。
pub fn menu_face_white(label: &str) -> UndoFace {
    menu_face_at_ink(label, MENU_PX, MENU_H, MENU_PAD, menu_ink_white())
}

/// ★ 2026-10-21：同一张脸的**任意尺寸版**。
///
/// 存在的理由只有一个：六六看了游戏菜单说「小气巴拉的」。同一行字在电源菜单里
/// （关机前最后确认，看一眼就走）和在游戏菜单里（一张要读的列表）需要的不是同一个字号。
/// 原来的 `menu_face` 把 `MENU_PX` 写死，所以"做大"只能靠改常量 —— 而那会把
/// 电源菜单一起放大，电源菜单的克制是刻意的。
///
/// `min_px` 取 `px * 0.6` 而不是 `MENU_MIN_PX`：缩放下限应该跟着目标字号走，
/// 一个 34px 的行缩到 18px 就认不出来了。
pub fn menu_face_at(label: &str, px: f32, h: u32, pad: u32) -> UndoFace {
    menu_face_at_ink(label, px, h, pad, menu_ink())
}

/// `menu_face_at` 的着色版：墨色由调用方给。电源菜单的恒定白（`menu_face_white`）与
/// 其余菜单的主题墨（`menu_face_at`）共用同一套排布，只有颜色不同 —— 一个 8 位三元组
/// 的分叉，不值得复制整段排版代码。
fn menu_face_at_ink(label: &str, px: f32, h: u32, pad: u32, ink: [u8; 3]) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let text_w = text::line_width(font, label, px, 0.0).ceil() as u32;
    let w = text_w + 2 * pad;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let layout = text::fit(font, label, w as f32, 1, px, px * 0.6);
    text::draw_centred(&mut rgba, w, h, &layout, ink);
    UndoFace { rgba, w, h }
}

/// ★ 2026-10-29（六六）：**电源菜单的背景图**，720×480 整屏。
///
/// 长按电源键的关机/重启菜单不再画在主题底（`slot_chrome::opening`）上，而是这张固定的
/// 霓虹隧道图。**它不随主题色板、不随暗色亮色模式** —— 图本身就是成品，前端只负责把它
/// 铺满整屏，不填任何主题色。
///
/// ⚠️ 尺寸契约与 `sticker.rs` 同一条：图不是本面板尺寸就返回 `None`（整块不画，回退主题
/// 底），而不是拉伸。720 这块面板是 720×480。
pub const POWER_BG_W: u32 = 720;
pub const POWER_BG_H: u32 = 480;

const POWER_BG_PNG: &[u8] = include_bytes!("../assets/powerbg-66mod.png");

/// The power menu's background, at this face's size.
///
/// **A size mismatch is `None`, not a resize** —— 与 `sticker_face` 同一立场，见其注释。
pub fn power_bg_face() -> Option<UndoFace> {
    let (px, w, h) = decode_bytes(POWER_BG_PNG)?;
    if (w, h) != (POWER_BG_W, POWER_BG_H) {
        return None;
    }
    Some(UndoFace {
        rgba: px,
        w: POWER_BG_W,
        h: POWER_BG_H,
    })
}

/// ★★ 2026-10-21：**游戏菜单的一格 = 一枚圆角按钮。**
///
/// 六六原话：「按钮能再精致一些吗？就像其它菜单那种有个圆角矩形按钮还带点光泽的」。
/// 这里就是把"其它菜单那种"搬过来 —— 和键位弹窗的七行（`keymap::row_face`）用**同一套
/// 模具**：`rounded::plate` 铺机壳色底加一圈 `edge` 描边，`menu::mould` 再从上面压一层
/// 竖向渐变与上下两道棱（顶亮底暗 + 内侧高光），于是每一格读起来是一块**有厚度的片**，
/// 而不是贴在面板上的字。
///
/// **两种状态两张面**，不是叠一层半透明高亮：
/// - `hot`（手里这一格）：底用主题里最亮的一档 `edge()`，字用 `opening_rgb()` —— 和电源
///   菜单选中那一行完全一致，两处不打架；
/// - 其余：底是 `housing()`、字是 `ink()`，描边仍是 `edge`。
///
/// 宽度是**格子的宽**（调用方给），不是按字数伸缩 —— 一排按钮要一样宽才成排；按字数伸缩
/// 的那套（`menu_face_at`）留给电源菜单，那里一行只有一两个词、居中放着才对。
pub fn game_row_face(label: &str, w: u32, hot: bool) -> UndoFace {
    let h = GAME_ROW_H;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let rim = if hot {
        // 亮底上再用亮边就等于没有边，改成面板底色描一圈，把这一格"抠"出来。
        crate::slot_chrome::opening()
    } else {
        edge()
    };
    let fill = if hot { edge() } else { housing() };
    rounded::plate(&mut rgba, w, h, GAME_ROW_RADIUS, GAME_ROW_RIM, fill, Some(rim));
    crate::menu::mould(&mut rgba, w, h, GAME_ROW_RADIUS, GAME_ROW_RIM);
    let Some(font) = text::label_font() else {
        return UndoFace { rgba, w, h };
    };
    let layout = text::fit(
        font,
        label,
        w as f32 - 2.0 * GAME_ROW_PAD as f32,
        1,
        GAME_ROW_PX,
        GAME_ROW_PX * 0.62,
    );
    let ink = if hot {
        crate::menu::opening_rgb()
    } else {
        menu_ink()
    };
    text::draw_centred(&mut rgba, w, h, &layout, ink);
    UndoFace { rgba, w, h }
}

/// ★★ 2026-10-21：**游戏菜单的抬头**（游戏名）。
///
/// 和货架的标题是两张面，因为两处的**层级关系正好相反**：货架上标题要压住一整排卡带，
/// 30 已经够；游戏菜单里它下面是一格一格 26 的字，标题若还是 30，两边就差 4 像素，
/// 一眼看过去是"一行略大的选项"而不是"这一屏的抬头"。38 才拉得开。
///
/// 无底、无框，只印字 —— 它是**标题**，不是第 0 个按钮。
pub fn game_title_face(label: &str) -> UndoFace {
    let (w, h) = (GAME_TITLE_W, GAME_TITLE_H);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    if let Some(font) = text::label_font() {
        let layout = text::fit(
            font,
            label,
            w as f32 - 2.0 * GAME_ROW_PAD as f32,
            1,
            GAME_TITLE_PX,
            GAME_TITLE_MIN_PX,
        );
        text::draw_centred(&mut rgba, w, h, &layout, menu_ink());
    }
    UndoFace { rgba, w, h }
}

/// A cheat code row in the cheat table. Smaller than a menu line because codes are short and
/// there are often many of them on one panel; the same auto-width sizing as `menu_face` so the
/// ON/OFF chip drawn beside it hugs the words. `desc` is the human label from the cheat file
/// (the text after `#`); `code` is the raw code. When `desc` is empty the code stands alone on
/// one line, as before.
const CHEAT_PX: f32 = 16.0;
const CHEAT_MIN_PX: f32 = 11.0;
const CHEAT_H: u32 = 22;
const CHEAT_PAD: u32 = 8;
/// The description and code sizes for the two-line layout; the code sits beneath the description
/// in a dimmer ink so the label reads first and the code second — dimmer meaning *towards the
/// ground*, which is the palette's business rather than this file's.
const CHEAT_DESC_PX: f32 = 16.0;
const CHEAT_CODE_PX: f32 = 11.0;

fn cheat_code_ink() -> [u8; 3] {
    crate::palette::dim_ink()
}

pub fn cheat_row_face(desc: &str, code: &str) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let track = |px: f32| (px * 0.10).round();
    let desc = desc.trim();
    let has_desc = !desc.is_empty();

    let desc_w = if has_desc {
        text::line_width(font, desc, CHEAT_DESC_PX, track(CHEAT_DESC_PX)).ceil() as u32
    } else {
        0
    };
    let code_w = text::line_width(font, code, CHEAT_CODE_PX, track(CHEAT_CODE_PX)).ceil() as u32;
    let ink = desc_w.max(code_w);
    let w = ink + 2 * CHEAT_PAD;

    if has_desc {
        // Two lines: description above, raw code beneath.
        let desc_band: u32 = 24;
        let code_band: u32 = 16;
        let h = desc_band + code_band;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let desc_layout = text::fit(font, desc, w as f32, 1, CHEAT_DESC_PX, CHEAT_DESC_PX * 0.7);
        let mut top = vec![0u8; (w * desc_band * 4) as usize];
        text::draw_centred(&mut top, w, desc_band, &desc_layout, menu_ink());
        blit_band(&mut rgba, w, &top, 0);
        let code_layout = text::fit(font, code, w as f32, 1, CHEAT_CODE_PX, CHEAT_CODE_PX * 0.7);
        let mut bot = vec![0u8; (w * code_band * 4) as usize];
        text::draw_centred(&mut bot, w, code_band, &code_layout, cheat_code_ink());
        blit_band(&mut rgba, w, &bot, desc_band);
        UndoFace { rgba, w, h }
    } else {
        // No description: the code alone, exactly as the old single-line row did.
        let mut rgba = vec![0u8; (w * CHEAT_H * 4) as usize];
        let layout = text::fit(font, code, w as f32, 1, CHEAT_PX, CHEAT_MIN_PX);
        text::draw_centred(&mut rgba, w, CHEAT_H, &layout, menu_ink());
        UndoFace {
            rgba,
            w,
            h: CHEAT_H,
        }
    }
}

/// Copy a transparent-backed text band into `dst` starting at row `y0`. The bands never overlap
/// and the destination is transparent there, so source-over collapses to a straight copy.
fn blit_band(dst: &mut [u8], dst_w: u32, src: &[u8], y0: u32) {
    let row_w = dst_w as usize;
    let rows = src.len() / 4 / row_w;
    for row in 0..rows {
        let dy = y0 as usize + row;
        let s = row * row_w * 4;
        let d = dy * row_w * 4;
        for col in 0..row_w {
            let si = s + col * 4;
            if src[si + 3] == 0 {
                continue;
            }
            dst[d + col * 4..d + col * 4 + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
}
