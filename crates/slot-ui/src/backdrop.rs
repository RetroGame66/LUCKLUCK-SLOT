use std::path::Path;
use std::sync::OnceLock;

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};

use crate::art;
use crate::palette;
use crate::shutter::glow_face_spread;

/// The flat the screen is when the card carries no wallpaper, and what is behind the picture
/// when it does.
///
/// The device used to rely on the compositor's clear colour for this, which is black and is
/// compiled into `slot-gfx`. That is a palette fact, so it belongs here with the other one: in
/// the light mode a card with no wallpaper has to come up on paper rather than on a black
/// screen. Drawing it rather than clearing to it costs one full screen quad and keeps the
/// decision on this side of the crate boundary, where the mode lives.
fn ground() -> [f32; 4] {
    match palette::mode() {
        palette::Mode::Dark => [0.0, 0.0, 0.0, 1.0],
        palette::Mode::Light => [0.90, 0.89, 0.86, 1.0],
    }
}

/// Cover the whole panel, centre cropped. PNG only, as the labels are.
pub fn wallpaper_face(path: &Path) -> Option<Vec<u8>> {
    art::cover(path, OUT_W, OUT_H)
}

/// ★ 2026-10-28（六六）：「这是默认壁纸」—— 卡上的 `Wallpapers/` 是空的（或一张都读不出）
/// 时铺的那张：霓虹线框隧道，纯黑底 + 品红网线，**720×480**，中央留黑给货架。
///
/// **嵌在二进制里，不放卡上。** 光效按这张图自己的"油墨"烘遮罩（见 [`wallpaper_glow`]），
/// 遮罩必须和屏上那张是**同一张图** —— 放进 `Wallpapers/` 就变成"一张候选图"，`pick` 会随机
/// 轮换，光还会跟着别的图烘，落到哪张都说不清。它是**默认**，不是收藏。
const DEFAULT_WALL_PNG: &[u8] = include_bytes!("../assets/wall-default.png");

/// 解一次就够：换主题要重烘光效（色跟着主题走），底色不用重新解 PNG。
static DEFAULT_WALL: OnceLock<Option<(Vec<u8>, u32, u32)>> = OnceLock::new();

/// The built-in default wallpaper's pixels, or `None` on a build whose picture will not decode.
pub fn default_wallpaper() -> Option<&'static (Vec<u8>, u32, u32)> {
    DEFAULT_WALL
        .get_or_init(|| {
            let (rgba, w, h) = art::decode_bytes(DEFAULT_WALL_PNG)?;
            if w != OUT_W || h != OUT_H || rgba.len() < (w as usize * h as usize * 4) {
                return None;
            }
            Some((rgba, w, h))
        })
        .as_ref()
}

/// 默认壁纸四周留的空白：柔光图比底图大 `2 * WALL_GLOW_PAD`，画的时候往左上偏回来，
/// 于是网线还是原来的位置。
pub const WALL_GLOW_PAD: u32 = 10;

/// 「不是近黑」—— 这张图的判据。
///
/// ★ **不能用 `mark_only` 那套 `#ff00ff` 判据**（`LOGO_MARK_*`：R>200 且 G<60 且 B>200）：
/// 这张壁纸的品红是 `(240, 0, 144..160)`，**B 落在 200 以下**，一个像素都过不了那道闸门。
/// 而它的画面本来就只有两种东西 —— 黑底和网线 —— 所以判据直接写成"有亮度的就算线"：
/// 三通道之和 > 40，把线自己的抗锯齿和粉雾也算进去（那正是光要接上的地方）。
pub fn wallpaper_glow(
    rgba: &[u8],
    w: u32,
    h: u32,
    colour: [u8; 3],
) -> Option<(Vec<u8>, u32, u32)> {
    let len = w as usize * h as usize * 4;
    if rgba.len() < len {
        return None;
    }
    let mut mark = vec![0u8; len];
    let mut lit = 0usize;
    for p in (0..len).step_by(4) {
        if rgba[p + 3] > 0 && rgba[p] as u32 + rgba[p + 1] as u32 + rgba[p + 2] as u32 > 40 {
            mark[p + 3] = 255;
            lit += 1;
        }
    }
    if lit == 0 {
        return None;
    }
    // 半径 2 两遍：网线只有 1~2px 宽，开大就把整片糊成雾、中央那块黑也被糊进来。
    //
    // ★ 峰值**不是**用来盖品红的 —— 模糊把 1px 的线削到 1/25，峰值提到 0.85 也只有 5% 的覆盖，
    //   六六看出来的「颜色盖不太住」就出在这儿。品红由 [`wallpaper_ink`] 在底图上直接换掉，
    //   这一层只剩「线在发光」的活，0.55 是当初按观感定的那个值。
    glow_face_spread(&mark, w, h, colour, WALL_GLOW_PAD, 2, 0.55)
}

/// 换色之后网线压掉多少亮度 —— **墙面的线不是字**。
///
/// `ridge_ink` 是给字的：暗态 `#d8d4cc`，相对亮度约 0.83，满屏铺一圈会刺眼。原来的品红
/// `(240,0,150)` 亮度只有 0.24 左右，直接换成主题色等于把整张墙提亮三倍多。0.60 让它读起来
/// 还是"霓虹管"而不是"白线"，剩下的亮度交给呼吸光去顶（`breath_alpha` 0.55..1.0）。
const WALL_INK_GAIN_NUM: u32 = 3;
const WALL_INK_GAIN_DEN: u32 = 5;

/// 把默认壁纸的**油墨**换成主题色 —— 网线还是那些网线，颜色不再是品红。
///
/// ★ 2026-10-28（六六两次）：「洋红区还是比较明显」「颜色盖不太住」。上一版想用那圈呼吸光去
/// **盖**它，可光谱是**模糊过的**：`glow_face_spread` 跑两遍半径 2 的分离式盒式模糊，1px 宽的
/// 横线经过之后覆盖率只剩 `1/25`，再把 `peak` 从 0.55 提到 0.85 也只有 5% 的主题色压在品红上
/// —— **盖不住是算法决定的，不是峰值不够**。而且呼吸本来就有一半时间在暗处（`breath_alpha`
/// 0.55..1.0），暗的时候品红必然透回来。
///
/// 所以改从根上办：**底图自己就上主题色**。这张图只有黑底和网线两样东西，线有多亮（三通道取
/// 最大）就是它有多实，把它乘到主题色上，品红一个像素都不留；呼吸光退回它本来的活 —— 在线上
/// 加一层会明暗的光晕。跟「印刷跟 look 走、发光不跟」是同一条账。
///
/// 中央那块黑照样是黑的（线亮度为 0 ⇒ 乘出来还是 0），货架、字、卡带压上去一样清楚。
pub fn wallpaper_ink(rgba: &[u8], w: u32, h: u32, colour: [u8; 3]) -> Option<Vec<u8>> {
    let len = w as usize * h as usize * 4;
    if rgba.len() < len {
        return None;
    }
    let mut out = vec![0u8; len];
    let mut lit = 0usize;
    for p in (0..len).step_by(4) {
        // 「线有多实」= 最亮那一路。这张图的品红是 `(240, 0, 144..160)`，取最大就是 240 ——
        // 换成加权亮度只有 0.35，线会暗掉一半以上，那是另一次返工。
        let m = rgba[p].max(rgba[p + 1]).max(rgba[p + 2]);
        out[p + 3] = 255;
        if m == 0 {
            continue;
        }
        lit += 1;
        let k = m as u32 * WALL_INK_GAIN_NUM;
        let den = 255 * WALL_INK_GAIN_DEN;
        out[p] = (colour[0] as u32 * k / den) as u8;
        out[p + 1] = (colour[1] as u32 * k / den) as u8;
        out[p + 2] = (colour[2] as u32 * k / den) as u8;
    }
    if lit == 0 {
        return None;
    }
    Some(out)
}

/// The ground, the picture and the scrim over it, as the first things on the screen.
///
/// The ground goes down whether or not there is a picture, and the picture and its scrim only
/// when there is one: that way the two modes differ by a colour rather than by a branch, and
/// neither has a case where the bottom of the stack is whatever the last frame left.
///
/// `glow` 是**内置默认壁纸**那圈呼吸光（`(tex, w, h, alpha)`，只认这一张 —— 卡上自己的图
/// 不发光）。它压在纱**之上**：纱把壁纸拿走 62%，光要是垫在底下就跟着一起暗掉，那不是"线
/// 在发光"，是"线被擦亮了一点"。
///
/// ★ 也因此这张图在**暗态不压纱**：纱的活是"把一张照片按回去"，而默认壁纸本来就是为了这块
/// 屏画的 —— 中央纯黑、网线全在边上，货架与字都落在黑里，压它只会把霓虹洗成灰粉。**亮态照
/// 压**（白纱 0.74）：亮态的墨是深色，不压的话深字压在纯黑上等于看不见 —— 那是可读性的账，
/// 不是审美的账。
pub fn draw_backdrop(face: Option<TexId>, glow: Option<(TexId, u32, u32, f32)>, out: &mut Vec<Draw>) {
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        colour: ground(),
    });
    let Some(tex) = face else {
        return;
    };
    out.push(Draw::Tex {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        tex,
        alpha: 1.0,
    });
    let veiled = match palette::mode() {
        palette::Mode::Dark => glow.is_none(),
        palette::Mode::Light => true,
    };
    if veiled {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: palette::scrim(),
        });
    }
    let Some((gtec, gw, gh, alpha)) = glow else {
        return;
    };
    let (gw, gh) = (gw as f32, gh as f32);
    out.push(Draw::Tex {
        x: (OUT_W as f32 - gw) / 2.0,
        y: (OUT_H as f32 - gh) / 2.0,
        w: gw,
        h: gh,
        tex: gtec,
        alpha: alpha.clamp(0.0, 1.0),
    });
}
