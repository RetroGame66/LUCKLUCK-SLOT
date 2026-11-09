//! Shader sources are GLSL ES 1.00 so the device build compiles them unchanged. Only the
//! preamble differs: the device supplies `precision` defaults and
//! `#define FRAG_COLOR gl_FragColor`, the host maps the ES names onto GL 3.3 core.

use crate::surface::GfxError;

const VERT_PREAMBLE: &str = "#version 330 core\n#define attribute in\n#define varying out\n";

const FRAG_PREAMBLE: &str = "#version 330 core\n#define varying in\n\
                             #define texture2D texture\nout vec4 FRAG_COLOR;\n";

/// ES 1.00 is the language these are written in, so the device adds nothing but the name of
/// the output. A `#version` line is omitted rather than set: 100 is the default, and the
/// drivers that reject `#version 100` outnumber the ones that require it.
const VERT_PREAMBLE_ES: &str = "";
const FRAG_PREAMBLE_ES: &str = "#define FRAG_COLOR gl_FragColor\n";

pub fn program(vert: &str, frag: &str) -> Result<gl::types::GLuint, GfxError> {
    let (vp, fp) = match crate::gl::es() {
        true => (VERT_PREAMBLE_ES, FRAG_PREAMBLE_ES),
        false => (VERT_PREAMBLE, FRAG_PREAMBLE),
    };
    crate::gl::program(&format!("{vp}{vert}"), &format!("{fp}{frag}"))
}

/// Unit quad to a rect in target pixels, origin top left. The y flip lives here, so every
/// pass drawing into the offscreen target thinks in screen coordinates and only the blit
/// deals with the framebuffer being stored bottom up.
pub const RECT_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    vec2 p = (u_rect.xy + a_pos * u_rect.zw) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
"#;

/// `RECT_VERT` for sprites, turned about the rect's centre by `u_turn`, which holds the cosine
/// and sine of the angle. The corner is placed exactly as `RECT_VERT` places it, plus the
/// difference between the corner turned and unturned; the sprite loop passes exactly (1, 0)
/// for anything that is not turned, which makes that difference exactly zero. A shader of its
/// own rather than a change to `RECT_VERT`, because the game pass links that one too and would
/// read an unset `u_turn` as (0, 0).
pub const SPRITE_VERT: &str = r#"
attribute vec2 a_pos;
uniform vec4 u_rect;
uniform vec2 u_target;
uniform vec2 u_turn;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    vec2 mid = u_rect.zw * 0.5;
    vec2 local = a_pos * u_rect.zw - mid;
    vec2 turned = vec2(u_turn.x * local.x - u_turn.y * local.y,
                       u_turn.y * local.x + u_turn.x * local.y);
    vec2 p = (u_rect.xy + a_pos * u_rect.zw + (turned - local)) / u_target;
    gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0);
}
"#;

/// `u_src` is the source size in pixels — the picture's *own* grid, which the lattice is drawn
/// on. `u_cell` is how many *output* pixels one source pixel covers, per axis. `u_mask_scale`
/// is how many times the 3x3 mask table repeats across the window — **not** the same grid:
/// see both fields' notes in `pipeline.rs` for why a whole multiple anchors it to source
/// pixels and a fractional one to the panel's own pixels.
pub const GAME_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_game;
uniform sampler2D u_mask;
uniform vec2 u_src;
// 每个**源像素**在面板上占多少输出像素（`rect/tex`，两轴各一个）。**不**是缩放常数：
// 它是几何量，从某一帧的实际纹理尺寸和这一机种的画面矩形除出来，所以 320×240 的 PS1 帧
// 落在 640×480 的矩形里时这里是 2。格子线宽由它反算（见下面的 `1 - 1/cell`）。
// 不能复用 `u_rect`：`RECT_VERT` 里有同名的那个，顶点阶段默认 highp、这里 mediump，
// 同名不同精度会直接链接失败 —— 2026-10-26 的 `REFLECT_FRAG` 就在这上面黑过一次屏。
uniform vec2 u_cell;
// 遮罩表的重复次数（表是 3×3，一个重复 = 三格）。整数倍时等于 `u_src`（锚源像素），
// 非整数倍时等于 `rect/3`（锚面板像素）。两轴可能不同，所以是 vec2。
uniform vec2 u_mask_scale;
uniform float u_bright;
uniform mat3 u_cc;
// 色彩校正的黑点：在矩阵之后加上去（同一空间）。饱和度/单色背光那一类用不上它（恒 0），
// 但**实机色板用得上**——DMG 最亮档不是白、最暗档也不是黑（#B8F878 到 #102000），
// 只靠一个 3x3 乘只能做「黑 → 某色」的斜率，做不出这种两点映射。加了它，
// 一个色板就是「白点 - 黑点」的斜率加一个偏移，四个灰阶能落在实机那四个颜色上。
uniform vec3 u_cc_bias;
// 实机色板的查找表：256×1 的 RGBA，按画面本来的灰度直接取色（见 app::DisplayFilter::gb_lut）。
uniform sampler2D u_pal;
// 1.0 = 走色板，0.0 = 走上面的矩阵。uniform 分支，两条路各自编译好，代价可忽略。
uniform float u_pal_on;
// 像素艺术模式（彩色机的第 4 档）：把画面量化到一张经典像素画色板（DB16，16 色）。
// 32^3 的查找表平铺成 1024×32 的纹理：x = r + 32*g，y = b（每通道取 5 bit，正好覆盖
// 一个 GBA 能出的全部 RGB555 颜色）。查询前按屏幕像素位置加一个 4×4 有序抖动偏移，
// 把量化产生的色带变成点阵——像素画就是这么干的。
uniform sampler2D u_pix;
uniform float u_pix_on;
// 格子：颜色（sRGB 0..1，取色板最浅那一档）与混入量（0 = 不画）。
// 画在每个源像素 3x3 的**右列与下行**——重叠的右下角只算一次，5 个像素——所以一个游戏像素
// 保留左上 2x2 的画面。用 `fract(v_uv * u_src)` 取子位置，几个 ALU 加一次 mix 就够，
// 比再取一张遮罩纹理便宜，而且格子色**自动跟着色板走**。
uniform vec3 u_grid;
uniform float u_grid_mix;
// 1.0 = 只画**下行**（扫描线），0.0 = 画整个 5/9 格子。扫描线是同一格子的横线那一半，
// 所以不用第二条渲染路径 —— 一个 uniform 分支就够。
uniform float u_grid_scan;
// ★ 2026-10-28：**扫描线隔几个源行画一条**，1 = 每行，2 = 隔一行。
//
//   这个 uniform 自 2026-10-24 就一直在往这里推（`app::DisplayFilter::CONSOLE_SCANLINE_STEP
//   = 2`，六六原话「FC/SFC 的扫描线太密了画面基本看不见，能不能减少一半密度？」），
//   但 shader 一直没声明它 ⇒ `glUniform1f(-1, 2.0)` 是无声无操作，**分散度减半从未生效**，
//   2× 主机实际一直是每源行一条（占屏 1/2）。2026-10-25 的复盘里记过这一条，这里补上。
uniform float u_grid_step;
// 色彩校正所在的 gamma：1.0 = 直接在编码空间乘（旧行为，两步 pow 互为逆）；
// 2.2 = 先转线性、乘完再转回。饱和度与单色背光映射必须在线性空间里做才不发闷——
// 这是 RetroArch 手持着色器（nds-color / lcd1x_nds）的通行做法，直接乘编码值会把
// 「半彩」压暗、把单色背光冲淡。
uniform float u_cc_gamma;
varying vec2 v_uv;
// 4×4 有序抖动（Bayer）。两次 2×2 用算术叠出来，避开 GLSL ES 1.00 没有的位运算与数组索引。
float Bayer2(vec2 a) {
    vec2 f = floor(a);
    return fract(f.x * 0.5 + f.y * f.y * 0.75);
}
float Bayer4(vec2 a) {
    return Bayer2(a * 0.5) * 0.25 + Bayer2(a);
}
void main() {
    vec3 c = texture2D(u_game, v_uv).rgb;
    // 隔行合成（PS1 专用，max 取亮）。`u_field_parity` 现在不再被 shader 读取，
    // 但 pipeline 仍然推——保留 uniform 位以便未来需要「仅取偶数行」时不用重编。
    if (u_pix_on > 0.5) {
        // 抖动 ±半个 5-bit 步，再吸附到 32 级（floor+0.5 即四舍五入），最后查表。
        float bayer = Bayer4(floor(gl_FragCoord.xy)) - 0.5;
        vec3 q = clamp(c + bayer / 31.0, 0.0, 1.0);
        vec3 lv = floor(q * 31.0 + 0.5);
        vec2 uv = vec2((lv.x + 32.0 * lv.y + 0.5) / 1024.0, (lv.z + 0.5) / 32.0);
        c = texture2D(u_pix, uv).rgb;
    } else if (u_pal_on > 0.5) {
        // 画面是四阶灰，色板按灰阶取色。表在 CPU 上就按四档插好了（含中间过渡，反射那边
        // 拿到的是模糊过的连续值），这里一次 NEAREST 取样 —— 比两次 pow + 九次乘加便宜，
        // 而且四档落点精确，多色相的色板也保得住。
        float l = dot(c, vec3(0.299, 0.587, 0.114));
        c = texture2D(u_pal, vec2(l, 0.5)).rgb;
    } else {
        c = pow(c, vec3(u_cc_gamma));
        c = u_cc * c + u_cc_bias;
        c = pow(max(c, vec3(0.0)), vec3(1.0 / u_cc_gamma));
    }
    if (u_grid_mix > 0.0) {
        vec2 sub = fract(v_uv * u_src);
        // The mesh is the right column and the bottom row of every source pixel (5 of its 9,
        // overlap counted once); a scanline is the bottom row alone — the same lit wire every
        // nth game pixel, which is what a panel's row gap looks like.
        //
        // ★ 2026-10-28：线宽由 `u_cell` 反算，不再写死 `0.66667`。
        //
        //   写死的 `0.66667` 是"一个源格子里最下面那 1/3"。它的**物理含义**是 3× 下的
        //   一条 1px 输出行（3× 时 1/3 格 = 正好 1 屏像素），但把 1/3 这个**比例**当成了
        //   常数。换到 2× 时它变成"源格子最下面的 1/3 格"，而一个源格子只有 2 个输出
        //   像素、中心落在 0.25 / 0.75 —— 于是只有 0.75 那一个被点亮。
        //
        //   所以**点亮行数其实没变**（2× 下旧版也是 1 行），变的是"点亮哪一行还说得通"：
        //   0.75 落在格子靠下的位置，不是格子的**底线**。`1 - 1/cell` 把"1 个输出像素宽"
        //   这件事直接写进公式，2× 下阈值 0.5 ⇒ 点亮的正是下半行（0.75 ≥ 0.5），3× 下
        //   `1 - 1/3 = 0.66667`（差 3e-6，远在中心点 0.167/0.5/0.833 的间隙之外）⇒
        //   **720 档逐像素不变**。逐格枚举见 `_tools/_mask_anchor_sim.py`。
        //
        //   ⚠️ 1× 要单独挡：一个源格子只有一个输出像素，`1 - 1/1 = 0` ⇒ `sub >= 0` 恒真
        //   ⇒ 整屏全黑。1× 下扫描线本来也无处安放（一个源行就是一个屏行），所以不画 ——
        //   旧版 0.66667 在 1× 下也正好全部不点亮（中心 0.5 < 0.667），这一挡把那个保住。
        vec2 inv = 1.0 / max(u_cell, vec2(1.0));
        // 扫描线的**行分散**：源行号 % step。`u_src` 就是源格数，`floor` 取到当前源行。
        // step=1 时 `mod` 恒 0 ⇒ 每行都画（GB/GBC 的格子走 `u_grid_scan = 0`，不受影响）。
        // step=2 时只有偶数源行画 ⇒ 间距从 2 屏行变 4 屏行，浓度减半。
        float row = floor(v_uv.y * u_src.y);
        bool spread = u_grid_step <= 1.0 || mod(row, u_grid_step) < 0.5;
        bool hit = false;
        if (spread && u_cell.y > 1.001) {
            hit = sub.y >= 1.0 - inv.y;
        }
        if (!hit && u_grid_scan < 0.5 && u_cell.x > 1.001) {
            hit = sub.x >= 1.0 - inv.x;
        }
        if (hit) {
            c = mix(c, u_grid, u_grid_mix);
        }
    }
    // 面板遮罩照旧在编码空间乘（顺序与旧版等价：矩阵与遮罩都是乘法，可交换）。
    //
    // ★★ 2026-10-28：平铺次数改由 `u_mask_scale` 给，不再写 `u_src`。
    //   整数倍时 `u_mask_scale == u_src`（与旧版**逐字相同**）；非整数倍时是 `rect/3`，
    //   每 3 个屏像素一个周期，格推进率恒 1、拍频归零。为什么必须条件化见 `system.rs`
    //   的 `mask_scale`：2× 下一律锚面板会把格子边界从块间挪进块内，720 机器上所有 2×
    //   主机的画面都会变。
    vec3 rgb = c * texture2D(u_mask, v_uv * u_mask_scale).rgb;
    FRAG_COLOR = vec4(rgb * u_bright, 1.0);
}
"#;

/// A 3x3 box average of the game frame, rendered into a much smaller texture (a quarter of
/// the frame each way). That downscale is the blur: the next pass samples the small texture
/// with linear filtering, and the wide magnification is what makes it genuinely soft — a
/// handful of taps against the pixel-art source would only band it.
/// Written out flat rather than as a loop. A `for` in a GLSL ES 1.00 fragment shader is the
/// one construct a strict mobile driver is most likely to refuse — and this tree had none
/// before, so there was nothing to say the device takes them. Nine taps is not worth finding
/// out on a machine with no console.
pub const BLUR_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform vec2 u_step;
varying vec2 v_uv;
void main() {
    vec3 s =
        texture2D(u_tex, v_uv + vec2(-1.0, -1.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2( 0.0, -1.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2( 1.0, -1.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2(-1.0,  0.0) * u_step).rgb +
        texture2D(u_tex, v_uv).rgb +
        texture2D(u_tex, v_uv + vec2( 1.0,  0.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2(-1.0,  1.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2( 0.0,  1.0) * u_step).rgb +
        texture2D(u_tex, v_uv + vec2( 1.0,  1.0) * u_step).rgb;
    FRAG_COLOR = vec4(s / 9.0, 1.0);
}
"#;

/// The screen glow: the blurred frame's own edge extended straight out past the picture's
/// window, drawn over the backdrop and under the game. Not a mirror — see `m` below.
///
/// `u_win` is the window rect in offscreen pixels; `u_margin` is how far, in pixels, the
/// reflection reaches beyond the window — full at its edge, gone that far out. The zone the
/// reflection can ever appear in is therefore the window grown by `u_margin` on every side,
/// which is the whole of what an overlay has to leave clear for it. `u_mask` is the overlay's
/// own texture, and its alpha is the **zone**: it is a hole or it is a wall — see `hole` below
/// for why it is not also the strength.
///
/// Drawn at the list's `Draw::Glow` marker, which is *after* the overlay art, so the art is
/// what the light falls on rather than a filter the light passes through.
pub const REFLECT_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_blur;
uniform sampler2D u_mask;
uniform vec4 u_win;
// Deliberately NOT `u_target`: that name is already a uniform of `RECT_VERT`, and a uniform
// shared between the two stages must carry the same precision in both. The vertex stage
// defaults floats to highp, this stage's `precision mediump float` makes it mediump, and the
// link fails outright — which is exactly how the first build of this went dark on the device.
// A name only the fragment knows has no counterpart to disagree with.
uniform vec2 u_panel;
uniform float u_margin;
uniform float u_gain;
uniform mat3 u_cc;
uniform vec3 u_cc_bias;
uniform sampler2D u_pal;
uniform float u_pal_on;
uniform float u_cc_gamma;
varying vec2 v_uv;
void main() {
    vec2 panel = v_uv * u_panel;
    vec2 t = (panel - u_win.xy) / u_win.zw;
    // The picture's own edge, carried straight out — light spilling past the screen onto the
    // bezel keeps going, it does not turn around. Clamping samples the nearest edge texel, so
    // the colour outside the window is the colour at the window's edge, smeared outward, and
    // the blur in the source texture softens it into a glow. (It was a mirror about each edge
    // once: a mirror needs a mirrored surface, and the plastic around a screen is not one.)
    vec2 m = clamp(t, 0.0, 1.0);
    // `u_blur` was rendered through `RECT_VERT`, which flips y on the way into a texture: its
    // v=1 row holds the picture's top, the other way round from the overlay and game textures,
    // which are uploaded and sampled as they are. So the y has to be put back here. Left
    // unflipped, the band above the screen shows what is *below* it — which reads as a mirror
    // and goes on reading as one however `m` is changed.
    vec3 c = texture2D(u_blur, vec2(m.x, 1.0 - m.y)).rgb;
    // The same colour correction the game pass applies — so the reflection carries the filter
    // (GRAYSCALE, AGB-001, NDS) instead of mirroring the raw picture, which is what switching a
    // filter would otherwise leave disagreeing with the screen. Keep this block in step with
    // GAME_FRAG's own. The pixel-art grade is the one exception: quantising a *blurred* glow
    // would only band it, so GAME_FRAG's `u_pix` branch has no counterpart here and the light
    // round the screen stays continuous (the app hands this pass the identity matrix for it).
    if (u_pal_on > 0.5) {
        c = texture2D(u_pal, vec2(dot(c, vec3(0.299, 0.587, 0.114)), 0.5)).rgb;
    } else {
        c = pow(c, vec3(u_cc_gamma));
        c = u_cc * c + u_cc_bias;
        c = pow(max(c, vec3(0.0)), vec3(1.0 / u_cc_gamma));
    }
    // How far outside the window, in offscreen pixels — so the band is the same width on all
    // four sides whatever the letterbox happens to be.
    vec2 d = max(vec2(0.0), max(-t, t - 1.0)) * u_win.zw;
    float fade = clamp(1.0 - max(d.x, d.y) / u_margin, 0.0, 1.0);
    fade *= fade;
    // Strictly outside the window. `fade` is 1 *inside* it as well — d is zero there — and the
    // glow only ever got away with that by being drawn under the picture, which covered it.
    // Now that the glow is over the art it would be over the picture too, and the picture would
    // be veiled by a blurred copy of itself. The window is the screen; the light lands beside it.
    float outside = (t.x < 0.0 || t.y < 0.0 || t.x > 1.0 || t.y > 1.0) ? 1.0 : 0.0;
    // A hole or a wall, never a strength. The alpha used to scale the light as well, and the
    // art is drawn over this pass on top of that, so a bezel painted at 40% took 40% out of the
    // glow twice — one number in a PNG doing two unrelated jobs. What the light is worth is
    // `u_gain`, decided here, and how dark the bezel is stays the art's own business.
    float hole = texture2D(u_mask, v_uv).a < 0.5 ? 1.0 : 0.0;
    FRAG_COLOR = vec4(c, fade * outside * hole * u_gain);
}
"#;

pub const SPRITE_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform sampler2D u_ramp;
uniform vec4 u_colour;
uniform vec4 u_uv;
uniform vec4 u_foil;
varying vec2 v_uv;

// One value per cell, stable across frames. The constants are kept small on purpose: this
// shader is mediump, and the well-known hash's 123.34/456.21 multiply runs off the end of a
// mediump mantissa on its way to infinity.
float hash21(vec2 p) {
    p = fract(p * vec2(0.1031, 0.1030));
    p += dot(p, p + 33.33);
    return fract(p.x * p.y);
}

void main() {
    // The sub-rect of the texture this quad covers: all of it for a plain sprite, the
    // sticker's own rectangle for a film, one atlas cell for a glyph.
    vec2 t = mix(u_uv.xy, u_uv.zw, v_uv);
    vec4 src = texture2D(u_tex, t);
    vec4 c = src * u_colour;
    if (u_foil.x > 0.0) {
        float luma = dot(src.rgb, vec3(0.299, 0.587, 0.114));
        // The dark parts of what is under the film eat it and the bright parts barely take
        // it. This is the difference between a film laid over something and a coloured
        // rectangle drawn on top of it.
        float shade = 1.0 - 0.7 * luma;
        float effect = u_foil.w;
        vec3 film = vec3(0.0);
        float cover = 1.0;
        if (effect < 0.5) {
            // Holo: the whole sticker, one rainbow crossing it at an angle, held a quarter
            // back from the ramp's own chroma - diffraction at full saturation over a whole
            // label reads as candy rather than as foil.
            float a = t.x * 0.7 + t.y * 0.3 + u_foil.y;
            vec3 ramp = texture2D(u_ramp, vec2(fract(a), 0.5)).rgb;
            film = mix(vec3(dot(ramp, vec3(0.299, 0.587, 0.114))), ramp, 0.72);
        } else if (effect < 1.5) {
            // Shatter: cracked cells, each its own hue off the ramp and its own twinkle,
            // some of them lit at a time, the seams between them read as the cracks.
            vec2 cells = vec2(14.0, 8.0);
            vec2 f = fract(t * cells);
            vec2 cell = floor(t * cells);
            float h = hash21(cell);
            float tw = 0.5 + 0.5 * sin(u_foil.y * 6.28318 + h * 6.28318);
            cover = step(0.58, h) * (0.30 + 0.70 * tw);
            vec2 d = min(f, 1.0 - f);
            float seam = smoothstep(0.0, 0.05, min(d.x, d.y));
            cover *= mix(0.35, 1.0, seam);
            film = texture2D(u_ramp, vec2(h, 0.5)).rgb;
        } else if (effect < 2.5) {
            // Metal: brushed streaks running one way with a hard specular crossing them,
            // and the sticker's own bright passages etched down into the plate.
            float streak = hash21(vec2(floor(t.x * 120.0), floor(t.y * 2.0)));
            vec3 silver = mix(vec3(0.30, 0.31, 0.33), vec3(0.90, 0.91, 0.93), streak);
            float diag = fract(t.x * 0.8 - t.y * 0.6 + u_foil.y * 0.12);
            float spec = pow(1.0 - abs(diag - 0.5) * 2.0, 6.0);
            film = silver + vec3(spec) * 0.45;
            float etch = smoothstep(0.45, 0.85, luma);
            film = mix(film, film * 0.45, etch * 0.7);
        } else if (effect < 3.5) {
            // Glitter: sparse warm points, each with its own slow twinkle.
            vec2 cells = vec2(26.0, 15.0);
            vec2 f = fract(t * cells) - 0.5;
            vec2 cell = floor(t * cells);
            float h = hash21(cell);
            float on = step(0.90, h);
            float tw = 0.5 + 0.5 * sin(u_foil.y * 9.0 + h * 40.0);
            cover = (1.0 - smoothstep(0.02, 0.20, length(f))) * on * tw;
            film = vec3(1.0, 0.94, 0.82);
        } else {
            // Prism: the shell's own edge and nowhere else. The cart face's alpha *is* its
            // outline, so the band where the alpha falls off just inside the edge is the
            // edge itself - no second shape to draw and no outline to trace.
            vec2 inward = mix(t, vec2(0.5), 0.012);
            float edge = src.a * (1.0 - texture2D(u_tex, inward).a);
            cover = edge;
            vec3 ramp = texture2D(u_ramp, vec2(fract(t.x * 0.6 + t.y * 0.4 + u_foil.y), 0.5)).rgb;
            film = mix(vec3(dot(ramp, vec3(0.299, 0.587, 0.114))), ramp, 0.8);
        }
        // The sweep: one specular band crossing the sticker, and only on the cart under the
        // caret - a still film on every cart and a moving one where the eye already is.
        if (u_foil.z > 0.0) {
            float s = fract(t.x * 0.9 - t.y * 0.25 - u_foil.y * 0.35);
            float band = pow(max(0.0, 1.0 - abs(s - 0.5) * 2.0), 5.0);
            film += vec3(1.0) * band * u_foil.z * 0.8;
            cover = max(cover, band * u_foil.z);
        }
        // Light and dye, not a layer: the film hands over only its own coverage, and the
        // framebuffer's blend lays it over the face that is already drawn. `u_colour.a` is
        // the quad's own opacity, so a cart dimmed at the side of the row dims its film too.
        float a = clamp(cover * shade * u_foil.x, 0.0, 1.0) * u_colour.a;
        FRAG_COLOR = vec4(film, a);
    } else {
        FRAG_COLOR = c;
    }
}
"#;

pub const BLIT_VERT: &str = r#"
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
    v_uv = a_pos;
    gl_Position = vec4(a_pos * 2.0 - 1.0, 0.0, 1.0);
}
"#;

pub const BLIT_FRAG: &str = r#"
precision mediump float;
uniform sampler2D u_tex;
uniform vec3 u_gain;
varying vec2 v_uv;
void main() {
    FRAG_COLOR = vec4(texture2D(u_tex, v_uv).rgb * u_gain, 1.0);
}
"#;
