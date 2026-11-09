//! The deck's retro-LCD panel: a logical four-shade frame, blown up by a whole number.
//!
//! # The chain, and why each step is where it is
//!
//! ```text
//! lyrics / spectrum / song name
//!     → 240×160 logical frame, one 0..3 index per pixel     (this file)
//!     → 1-bit threshold on the type, at the font's own design size
//!     → look the four indices up in a PixelShift ramp
//!     → ×3 nearest, drawing each cell's gap on the way     (the grid, in `render`)
//! ```
//!
//! **The grid is the paper, not a line ruled over it.** It is drawn during the upscale, one
//! screen pixel wide, at a pitch of [`SCALE`] — and it **subtracts**: a gap is bare glass with
//! the backlight behind it, so a gap is always the lightest shade whatever the cell beside it
//! wanted. A blank panel therefore shows **no grid at all**, and the grid appears only between
//! two inked cells — which is what makes a Game Boy screen read as a picture on a light field
//! rather than as a window mesh.
//!
//! # The type is 1-bit, and each face is set at its own size
//!
//! Three rules, all hard, all learned the hard way:
//!
//! * **Every face is drawn at its own design size (or a whole multiple of it), never a chosen
//!   number.** The panel sets two scripts on one line and needs two faces to do it: Chinese at
//!   16px and Latin at 8px, each the size its own font was drawn for. Scaling either one to a
//!   size it was not drawn for is what produces the grey smudge that reads as a dirty font —
//!   the 12px interface face at 16 is 70.6% antialiased, and a 12px Latin face at 8 is 100%.
//!   Both are measured; see `slot_ui::text::Plan` and `examples/font_size_audit.rs`.
//! * **The type is thresholded to one bit, not blended.** A four-shade panel has no greys; an
//!   antialiased edge is a level the panel cannot show, so it is either in or out.
//! * **Layout arithmetic is done on the ink, not on the em box.** An 8px Latin word puts its
//!   ink in the top half of a box the Chinese face wants 16 rows of, so anything "centred
//!   between" two other things goes through `text::plan_ink_span`.
//!
//! # The layout, and why every number in it is a multiple of the line
//!
//! Six六 gave the panel's design language as a **"LCD UI KIT"** sheet: framed sections with a
//! small label in the top-left corner, an inverted status strip along the bottom, discrete
//! (segmented) bars rather than smooth ones, and a row of big dot-matrix numerals. The player
//! panel below is that sheet's vocabulary, and the arrangement is the one every monochrome
//! player of 1999–2004 uses — a MiniDisc deck or an early iRiver, not a rectangle of text:
//!
//! ```text
//!  ┌──────────────────────────────────────────────────────┐
//!  │ NOW PLAYING                            SPECTRUM      │  header    (1 line)
//!  │ 算 (Original)                              0:37      │  title + big clock (2 lines)
//!  │ CRAZYSIX                                  / 4:12      │  artist + total
//!  │──────────────────────────────────────────────────────│  rule
//!  │ ┌ SPECTRUM ────────────────────────────────────────┐ │  framed section
//!  │ │  ▁▃▅▇█▇▅▃▁▃▅▇█▇▅▃▁▃▅▇█▇▅▃▁▃▅▇█▇▅▃▁▃▅▇█▇▅▃▁▃▅▇█▇▅ │ │
//!  │ └──────────────────────────────────────────────────┘ │
//!  │  ▐███▌▐███▌▐███▌▐███▌▐███▌▐███▌▐░░░▌▐░░░▌▐░░░▌▐░░░▌  │  segmented progress
//!  │ █ TRACK 03/12                              REPEAT ██ │  inverted strip
//!  └──────────────────────────────────────────────────────┘
//!    ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐                      five square keys
//!    │ ☰  │ │ ◀◀ │ │ ‖  │ │ ▶▶ │ │ □  │
//!    └────┘ └────┘ └────┘ └────┘ └────┘
//! ```
//!
//! **Every vertical number is the line height or a multiple of it**, and that is the fix for the
//! bug this file shipped with: `line_h()` is **16**, not 12 — Ark Pixel 12px carries a four-row
//! line gap — and the status strip was sized at six rows, so a line of type drawn into it ran
//! seven rows past its bottom edge and **printed across the tops of the five keys**. A strip is
//! now exactly [`line_h`] tall, the panel is laid out from that, and the two can no longer
//! disagree.
//!
//! **The grid stops at the strip.** The strip is inverted — ink, with paper type on it — and
//! meshing it as "half ink, half paper" turns a solid bar into a row of grey blocks — so the grid
//! used to stop at the strip's top. **It covers the whole panel now**, on the user's instruction:
//! the lattice is a property of the screen rather than decoration on one layer, and the game's own
//! picture has it over everything. The strip's lattice is part of the look now, not a defect.

use crate::text::{self, Layout};

/// The logical frame: 240 by 160, one shade index per pixel.
///
/// **Index 0 is the lightest and 3 the darkest** — the same way round as every palette in
/// `slot::palettes`, which comes from gambatte's own header (game colour 0 is the paper). An
/// index is therefore "how dark this pixel is", and a frame starts all-zero because a panel with
/// nothing on it is paper.
#[derive(Clone)]
pub struct Frame {
    pub w: u32,
    pub h: u32,
    /// One byte per pixel, 0..=3. A `Vec<u8>` rather than a `Vec<u8; 3]>` because the shades
    /// themselves are the caller's: this file never sees a colour.
    pub px: Vec<u8>,
}

impl Frame {
    /// A blank frame, all paper.
    pub fn new(w: u32, h: u32) -> Frame {
        Frame {
            w,
            h,
            px: vec![0; (w * h) as usize],
        }
    }

    /// The panel's own frame.
    pub fn panel() -> Frame {
        Frame::new(LOGICAL_W, LOGICAL_H)
    }

    /// One pixel, **clamped rather than wrapped.** A caller drawing a bar one column too far
    /// would otherwise write into the next row, and the artefact of that is a stray diagonal
    /// across the picture — the kind of thing that reads as "the panel is broken" rather than
    /// as "the bar was one too long".
    pub fn put(&mut self, x: i32, y: i32, shade: u8) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = (y as u32 * self.w + x as u32) as usize;
        self.px[i] = shade.min(3);
    }

    /// A filled rectangle. Used for the rules, the spectrum's baseline and the bars.
    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, shade: u8) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, shade);
            }
        }
    }

    /// A vertical bar, bottom-aligned at `floor` — **the spectrum's one primitive.**
    ///
    /// A bar grows up from the baseline rather than down from its top, because a spectrum that
    /// hangs from the ceiling and one that stands on the floor are the same picture upside down
    /// and only one of them is how a level meter has always looked.
    pub fn bar(&mut self, x: i32, floor: i32, w: i32, h: i32, shade: u8) {
        if h <= 0 {
            return;
        }
        self.fill(x, floor - h, w, h, shade);
    }
}

/// The logical frame's width. **240**, and every horizontal number in here divides by it.
pub const LOGICAL_W: u32 = 240;
/// The logical frame's height, and **it is 160, not 120** — the frame is the whole screen.
///
/// Six六's instruction was "操作区也是像素面板，等于 720×480 的这个画面都是同一套渲染":
/// the transport bar is not a second surface with its own texture and its own shading, it is
/// **more rows of this same frame**. A separate bar texture was tried first and it is wrong in a
/// way that only shows on the device — the bar's grid does not line up with the LCD's, the two
/// have different paper, and the seam between them is a line the eye finds immediately.
///
/// So: 240 × 160, ×3 = 720 × 480, one frame, one grid, one bake.
pub const LOGICAL_H: u32 = 160;
/// 画面四周的**边框**：2px 的最深色（用户第九轮："画面四周按最深色围 2px 的圈"）。
pub const FRAME: i32 = 2;
/// 边框**内侧**那道浅边（`EDGE` 档）：**只在上方与左方** —— 光从左上打过来。
/// 用户："做 45 度的浅边（即上方和左方的框内侧），浅边按 EDGE 这个档位就好"。
pub const BEVEL: i32 = 1;
/// 内容区在边框之内再让开的边距。**所有画面都内缩到边框里面**。
pub const PAD: i32 = 4;
/// The whole-number upscale. **Never anything but an integer**: a fractional scale would
/// resample, and a resampled four-shade frame produces levels the ramp has no colour for.
pub const SCALE: u32 = 3;
/// The panel's size on the glass, which is what the drawer hands to `Draw::Tex`. **The whole
/// 720×480**, because there is one frame and it is the screen.
pub const PANEL_W: u32 = LOGICAL_W * SCALE;
pub const PANEL_H: u32 = LOGICAL_H * SCALE;

/// 内容区的左右边界（半开区间）：**边框 + 内边距之后**。
pub fn content_x0() -> i32 {
    FRAME + PAD
}
pub fn content_x1() -> i32 {
    LOGICAL_W as i32 - FRAME - PAD
}
/// 内容区的上边界。下边界不是它 —— 走带区**贴着边框的内下沿**（用户："走带区贴底"）。
pub fn content_y0() -> i32 {
    FRAME + PAD
}

/// **The panel's Chinese size — the design size of the face the panel draws Chinese with.**
///
/// Not a choice: `unifont-16px.otf` was drawn on a 16px grid and is the only face here
/// whose Chinese is clean at that size. Taking the 12px interface face up to 16 instead is
/// 70.6% antialiased, and a threshold turns that into the grey smudge that reads as a dirty
/// font. See `slot_ui::text::Plan`.
pub const TITLE_CJK_PX: f32 = 16.0;
/// **The panel's Latin size — likewise the design size of its own face.**
///
/// `press-start-2p.ttf` is drawn on an 8×8 grid, and 8px is the size that makes the clock, the
/// transport word and the EQ tag hard pixels. It is also the only way to get Latin *below* the
/// Chinese: the shared body face has no clean size under 12, and 12 is the Chinese size
/// one step down — so "smaller" is a different face, not a smaller number.
pub const TITLE_LATIN_PX: f32 = 8.0;
/// The key labels' size: **the small face's own design size**, one step below the body's.
///
/// There is no size between 12 and 10 that is clean — 11px is 99.8% antialiased — so "smaller
/// than the body" means "the other face", and the other face only carries Latin (see
/// `slot_ui::text::SMALL_PX_MAX`). That is why the key labels are English in both builds.
pub const LABEL_PX: f32 = 10.0;

/// **The plan for prose: the song's name, its artist, and its lyrics.**
///
/// ★ 六六 2026-10-06：「歌手名，歌名，歌词里的英文和数字**沿用 16px 的字体**观感才统一」。
///   所以这一路上拉丁**和汉字同一张面、同一个字号**（`Latin::Cjk`）——
///   `五月天 - 突然好想你` 里那五个拉丁字符和两边七个汉字是同一套笔画，
///   而不是两种字体拼在一行里。代价为零：那张面在 16px 上排拉丁是 **0.0% 半调**（实测）。
pub fn prose_plan() -> text::Plan {
    text::Plan {
        cjk_px: TITLE_CJK_PX,
        latin_px: TITLE_CJK_PX,
        latin: text::Latin::Cjk,
    }
}

/// **The plan for tags: the transport word, the EQ tag, the clock, the mode.**
///
/// 六六：「除此之外的用 8px」。这一路上全是**纯拉丁**，而且是"读数"而不是"句子"——
/// `PLAY`、`[REPEAT]`、`0:37`——所以用面板自己的 8px 拉丁面（`Latin::Own`）：
/// 压在元信息行和走带条里，跟 16px 的主角拉开一个档，也不至于占掉半行宽。
pub fn tag_plan() -> text::Plan {
    text::Plan {
        cjk_px: TITLE_CJK_PX,
        latin_px: TITLE_LATIN_PX,
        latin: text::Latin::Own,
    }
}

/// The shade the type is drawn in, and the shade the rules are.
const INK: u8 = 3;
/// The faint shade between paper and rule — **the dot matrix's own drop shadow**; see [`emboss`].
/// The palettes are ordered lightest-first, so index 1 is a real step the panel never used before.
const EDGE: u8 = 1;
/// The rules, one step down from the type. **A rule at full ink competes with the words**, and a
/// panel where the divider is as dark as the text is a panel with no hierarchy in it.
const RULE: u8 = 2;
/// The paper: index 0, and the lightest shade in every ramp.
const PAPER: u8 = 0;

// ── The type ─────────────────────────────────────────────────────────────────────────────────
//
// ★ **Everything on this panel goes through a plan.** The interface's own `text::fit` /
//   `text::coverage` are for the shelves: one face at one size, from the card, one script per
//   line. The panel is the one screen that puts two scripts on the same line, so measuring and
//   drawing both go through a plan, and a Chinese word and the English word beside it cannot end
//   up on two different baselines.
//
// ★ **Two plans, and which one a line gets is a typographic decision, not a size one** —
//   see [`prose_plan`] and [`tag_plan`]. Every text call below takes the plan it is drawn with,
//   so "measured with one, drawn with another" is not expressible.

/// Lay `line` out under `plan`, wrapped to `max_lines`.
pub fn layout(line: &str, max_w: f32, max_lines: usize, plan: text::Plan) -> Layout {
    text::plan_fit(line, max_w, max_lines, &plan)
}

/// How tall a line of the panel's type is, in logical pixels.
///
/// **From the face's own metrics rather than from `px`**: the Chinese face carries a line box of
/// exactly its design size, so this is 16 — but it is read, not assumed, because the whole
/// vertical plan below is derived from it and a face swap must move all of it at once.
///
/// **The same for both plans**, which is the property that lets prose and tags share the panel's
/// rows: the Chinese face sets the line box in either one, and neither plan changes [`TITLE_CJK_PX`].
fn line_h() -> u32 {
    text::plan_line_height(&tag_plan()).round().max(1.0) as u32
}

/// How tall a line is at an *interface* size — the one place the panel still sets type in the
/// shared face, which is [`LABEL_PX`] and the keys' own labels.
fn line_h_at(px: f32) -> u32 {
    text::line_height(px).round().max(1.0) as u32
}

/// How tall a line of panel type is, in logical pixels — **public because a caller laying out a
/// paragraph needs it and must not re-derive it.**
pub fn line_h_public() -> u32 {
    line_h()
}

/// One line's width, in logical pixels, **measured the same way `plan_coverage` will draw it.**
fn measure_px(line: &str, plan: text::Plan) -> i32 {
    text::plan_line_width(line, &plan).ceil().max(1.0) as i32
}

/// Draw `line` left-aligned at `x`, `top`, in `shade`, **thresholded to one bit**.
///
/// `top` is the line's **em box top** — not its ink top. The em box is what the vertical plan
/// allocates; where the ink falls inside it is [`text::plan_ink_span`]'s answer, and only the
/// callers that have to *centre* something against this line need it.
///
/// **The threshold is the whole point.** `text::plan_coverage` hands back an antialiased 0..255
/// because that is what a blended face needs; a four-shade panel needs a decision. Anything at
/// or above the midpoint is ink and anything below is paper, and there is no third answer.
///
/// **The buffer is the line's own width, not the panel's**, and that is what makes this
/// left-aligned: `plan_coverage` centres its block in whatever buffer it is handed, so a buffer
/// exactly as wide as the text has a zero left bearing.
pub fn text_left(frame: &mut Frame, line: &str, x: i32, top: i32, shade: u8, plan: text::Plan) {
    if line.trim().is_empty() {
        return;
    }
    let l = layout(line, f32::INFINITY, 1, plan);
    let Some(text_line) = l.lines.first() else {
        return;
    };
    let w = measure_px(text_line, plan).max(1) as u32;
    let h = line_h();
    let strip = text::plan_coverage(w, h, &l, &plan);
    for y in 0..h {
        let dst = top + y as i32;
        if dst < 0 || dst >= frame.h as i32 {
            continue;
        }
        for col in 0..w {
            if strip[(y * w + col) as usize] >= 128 {
                frame.put(x + col as i32, dst, shade);
            }
        }
    }
}

/// Draw `line` with its right edge at `x_right`. **The status strip and the clock are
/// right-aligned**, because a number that grows must grow away from the edge it is pinned to.
pub fn text_right(frame: &mut Frame, line: &str, x_right: i32, top: i32, shade: u8, plan: text::Plan) {
    let w = measure_px(line, plan);
    text_left(frame, line, x_right - w, top, shade, plan);
}

/// Draw `line` centred **in the span `[x0, x1]`**, clipped to it.
///
/// **Not [`text_at`], which centres across the whole panel.** The strip carries three fields —
/// the two times and the mode — and the middle one has to centre in the space the other two
/// leave, or a long mode word would push it under the clock.
#[allow(clippy::too_many_arguments)]
pub fn text_centred_between(
    frame: &mut Frame,
    line: &str,
    x0: i32,
    x1: i32,
    top: i32,
    shade: u8,
    plan: text::Plan,
) {
    if line.trim().is_empty() || x1 <= x0 {
        return;
    }
    let l = layout(line, f32::INFINITY, 1, plan);
    let Some(text_line) = l.lines.first() else {
        return;
    };
    let tw = measure_px(text_line, plan).max(1);
    let h = line_h() as i32;
    let strip = text::plan_coverage(tw as u32, h as u32, &l, &plan);
    let start = x0 + ((x1 - x0) - tw) / 2;
    for y in 0..h {
        let dy = top + y;
        if dy < 0 || dy >= frame.h as i32 {
            continue;
        }
        for col in 0..tw {
            let dx = start + col;
            if dx < x0 || dx >= x1 {
                continue;
            }
            if strip[(y as u32 * tw as u32 + col as u32) as usize] >= 128 {
                frame.put(dx, dy, shade);
            }
        }
    }
}

/// Draw `line` centred across the panel, at `top` (its em box top).
pub fn text_at(frame: &mut Frame, line: &str, top: i32, shade: u8, plan: text::Plan) {
    if line.trim().is_empty() {
        return;
    }
    let l = layout(line, LOGICAL_W as f32 - 2.0 * PAD as f32, 1, plan);
    let h = line_h();
    let strip = text::plan_coverage(LOGICAL_W, h, &l, &plan);
    for y in 0..h {
        let dst = top + y as i32;
        if dst < 0 || dst >= frame.h as i32 {
            continue;
        }
        for x in 0..frame.w {
            // One bit: in or out, never a level between.
            if strip[(y * frame.w + x) as usize] >= 128 {
                frame.px[(dst as u32 * frame.w + x) as usize] = shade;
            }
        }
    }
}

/// Draw `line` at `top` in full ink — the panel's ordinary type, i.e. prose.
pub fn text_1bit(frame: &mut Frame, line: &str, top: i32) {
    text_at(frame, line, top, INK, prose_plan());
}

/// Draw `line` as a row of text, `step` pixels below the last, and hand back the next `top`.
pub fn text_line(frame: &mut Frame, line: &str, top: i32, step: i32) -> i32 {
    text_1bit(frame, line, top);
    top + step
}

/// `m:ss`, or `h:mm:ss` for anything over an hour. **Minutes are not zero-padded** — `3:07`
/// reads faster than `03:07`, and this is a clock a person glances at rather than reads.
fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

// ── The spectrum ─────────────────────────────────────────────────────────────────────────────

/// How many bands. **32**.
///
/// The history matters because each step was forced by the *frame*, not by taste: the first
/// version was 16, copied from the reference player, and it looked **sparse** on a 240-wide frame
/// — sixteen bars leave a pitch of 15 logical pixels, and the gaps came out wider than the bars,
/// which reads as a row of tally marks rather than as a spectrum. Thirty-two fixed that.
///
/// ★ **Sixty-four was tried on 2026-10-07 and taken back the same day.** 六六 asked for a double
/// (「立柱能加一倍吗」), saw it on the device, and said 「4不好看还是回退为32根吧」. Two reasons it
/// lost, and the second is the real one:
///
/// * at 64 the bars are 2–3 columns wide and the panel reads as *texture*, not as a spectrum;
/// * **the frequency resolution does not double with the band count.** `TAP_LEN = 2048` at 44.1k
///   resolves about 23 Hz, while 64 logarithmically spaced bands sit 4–8 Hz apart down at
///   80–150 Hz — so the bottom dozen bars read the same value and merge into one block. Halving
///   the count doubles the spacing and keeps the bottom of the range legible.
///
/// [`spectrum_bar`] spreads the remainder either way, so the box is filled at any count.
pub const BANDS: usize = 32;
/// The gap between two bars, in logical columns. **Exactly one**, everywhere, which is what makes
/// [`spectrum_bar`] a division rather than a table.
pub const BAND_GAP: i32 = 1;
/// The lowest band, in Hz. 80 is where a handheld speaker stops being a speaker.
const F_LO: f32 = 80.0;
/// The highest band. 16 kHz, which is the top of what the reference player shows and is about
/// where a small speaker stops carrying pitch.
const F_HI: f32 = 16_000.0;

/// 第 `b` 带的分析频率，Hz。
///
/// ⚠️ **步进必须按 `log2` 算，早期这里是 `ln`。** 原来写的是
/// `(F_HI / F_LO).ln() / (BANDS - 1)`，却又把它当成 `2.0f32.powf(...)` 的指数 ——
/// 自然对数的值被当成了以 2 为底的对数，两个底差一个 `ln 2`，于是整条轴被压扁：
/// **顶带只到 3148 Hz，`F_HI = 16 kHz` 那一头从来没有被分析过。**（`2^(ln200) = 39.4`，
/// 而应该的 `2^(log2 200) = 200`。）修好之后 `b = 31` 才是真 16 kHz。
///
/// 顺手也解释了为什么旧的"440 Hz 应该落在第 6–12 段"能通过：440 Hz 在**正确**的轴上
/// 是第 9.97 带（≈442 Hz），而那个 `cos(w)` 的铁律错误把峰又往下拖了约 4.4 带，
/// 两个错误刚好在旧测试的容差里互相抵消。
pub fn band_hz(b: usize) -> f32 {
    let ratio = (F_HI / F_LO).log2() / (BANDS - 1) as f32;
    F_LO * 2.0f32.powf(b as f32 * ratio)
}

/// One Goertzel magnitude: how much energy sits at `freq`.
///
/// **Goertzel rather than an FFT because there are sixteen questions, not two hundred.** A
/// sixteen-point DFT would cost about the same and a full FFT would cost a window's worth of
/// scratch the device does not have. The recurrence is three lines and needs no table.
fn goertzel(x: &[i16], freq: f32, rate: u32) -> f32 {
    if rate == 0 || x.is_empty() {
        return 0.0;
    }
    let w = 2.0 * std::f32::consts::PI * freq / rate as f32;
    // ⚠️ **系数是 `2·cos(w)`，不是 `cos(w)`。** 这是 Goertzel 递推的那个系数；写成
    // `cos(w)` 之后极点落在 `arccos(cos w / 2)` 而不是 `w` 上，整个响应被扭曲 ——
    // **一条 440 Hz 的正弦，峰落在 232 Hz 那一带，低了整整一个八度。**
    //
    // 它一直没被发现，是因为旧 [`bands`] 拿每一帧自己的峰做分母：扭曲只要是单调的，
    // 看上去就还像一张频谱。换成**绝对刻度**之后立刻现形 —— 下面的满量程参考是"一条
    // 正弦在**这一带自己的中心**上的响应"，响应一旦扭曲，这个参考就不再是平的
    // （实测跨带相差 **7 万倍**），拿它做分母只会得到噪声。改回 `2·cos(w)` 之后
    // 参考平到 ±0.5%。
    let cw = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for v in x {
        // Normalised to ±1 first: the recurrence is a sum of `n` terms and an i16 tap would
        // otherwise square into a number the f32 mantissa has to carry very carefully.
        let s0 = *v as f32 / 32_768.0 + cw * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - cw * s1 * s2
}

/// 显示区的下沿，**dB**（相对满量程正弦）。低于它的带读 0。
///
/// ★ 2026-10-07 六六选的档（我给了 −48 / −54 / −60 / −66 / −72 五档加实测，他挑了 **−60**）。
/// 实测（`_scratch/spectrum_study.py`，真曲 319 s、1065 帧）五档的 `|dh|` 均值 / 95 分位 / 平均柱高：
///
/// * −48 → 0.043 / 0.162 / 0.138
/// * **−60 → 0.057 / 0.161 / 0.229**
/// * −72 → 0.062 / 0.170 / 0.326
///
/// 越往下走柱子越高（用得满整个框）、顶到天花板的反而一个都没有；但再往下就只剩"越来越高"
/// 了 —— −72 已经有 0.326 平均柱高、看着像"一片一样高的柱子"。−48 又太矮（平均 0.138，
/// 一半的柱子挤在框底 1/6 里）。**−60 是这两头之间那个：95 分位的跳变与 −48 一样好
/// （0.161 对 0.162），但柱子高得多。**
const DB_FLOOR: f32 = -60.0;

/// The [`BANDS`] band heights, each 0..=1, on a **fixed dB scale**.
///
/// ★ **Rewritten 2026-10-07** —— 六六：「现在我的观感就是很踊跃的喷泉，希望看起来像是再平滑
/// 一些的变化（也可能灵敏度或者电平阈值高了）」。旧版把每一带除以**那一帧自己的峰值**，
/// 于是每次烘焙都是一张独立快照：同一根柱子这一帧满格、下一帧什么都没有 —— **那就是喷泉**。
/// 把时钟放慢从来没治好它（见 [`ballistics`]），因为跳变只是变**稀**，没有变小。
///
/// 现在做的是真分析器做的事 —— Winamp 与 foobar2000 的 `foo_vis_spectrum_analyzer` 都是这条
/// 规矩：**0 dB = 一条满量程正弦**，显示区下沿是 [`DB_FLOOR`]，带高就是它落在两者之间的位置。
/// 画面因此有了绝对含义：轻的段落画短的柱子，响的画长的。
///
/// 然后柱子还要**跨邻居模糊一次**（[`blur_bands`]）：一根柱子不许自己跳，它得拖动邻居。
///
/// 实测（`_scratch/spectrum_study.py`，真曲 319 s @44.1k，按卡座 300 ms 节拍取 1065 帧）：
/// 相邻两帧的柱高变化 `|dh|` 均值 **0.068 → 0.057**、95 分位 **0.315 → 0.161**，长期钉在
/// 满格的柱子 **3.5% → 0.0%**；而**平均柱高反而变高**（0.183 → 0.229）—— 固定刻度用得上
/// 整个框，不必每帧重新缩放到某一根峰上。
///
/// ⚠️ 这套数字是在**把 [`band_hz`] 的底数与 [`goertzel`] 的系数都修好之后**量的。那两个错
/// 是"绝对刻度"照出来的（详细在两处各自的注释里），修它们属于这次改动的必要部分。
pub fn bands(tap: &[i16], rate: u32) -> [f32; BANDS] {
    let mut out = [0.0f32; BANDS];
    if tap.is_empty() || rate == 0 {
        return out;
    }
    // 一条复用的暂存 —— 满量程参考要按每一带重造一条正弦，但不必每带新建一次 Vec。
    let mut reference = vec![0i16; tap.len()];
    for (b, slot) in out.iter_mut().enumerate() {
        let freq = band_hz(b);
        let energy = goertzel(tap, freq, rate).max(0.0);
        // 一条**满量程正弦**在这一带自己的中心上、过**同一个** `goertzel` 读出来的值。
        // 于是 `0 dB` 是个真实的绝对参考而不是约定 —— 两边都过同一个函数，它内部的任何
        // 定标都会自己约掉（这也是为什么不必去猜 Goertzel 归一化成了什么）。
        let r = full_scale_energy(&mut reference, freq, rate).max(1e-12);
        let db = 10.0 * (energy / r).max(1e-30).log10();
        *slot = ((db - DB_FLOOR) / (0.0 - DB_FLOOR)).clamp(0.0, 1.0);
    }
    blur_bands(&mut out);
    out
}

/// 一条**满量程正弦**在 `freq` 上的 [`goertzel`] 能量，窗长跟 `scratch` 一样。`scratch`
/// 复用，所以调用者把 [`BANDS`] 走一遍也不用分配。
fn full_scale_energy(scratch: &mut [i16], freq: f32, rate: u32) -> f32 {
    let step = 2.0 * std::f64::consts::PI * freq as f64 / rate as f64;
    for (i, s) in scratch.iter_mut().enumerate() {
        *s = (32_767.0 * (step * i as f64).sin()) as i16;
    }
    goertzel(scratch, freq, rate)
}

/// **带间 3 抽头模糊** `0.25 / 0.5 / 0.25`（两端按复制处理）。
///
/// 一根柱子不许自己跳 —— 它得拖动邻居。这是 [`bands`] 与 [`ballistics`] 之间那道平滑：
/// `ballistics` 管**时间**，这里管**频率**。两种跳法都要治，只治一种就还是六六最初看到的
/// "喷泉" —— 柱子在自己的带上按时钟跳，邻居一动不动。
fn blur_bands(h: &mut [f32; BANDS]) {
    let src = *h;
    for b in 0..BANDS {
        let l = src[b.saturating_sub(1)];
        let r = src[(b + 1).min(BANDS - 1)];
        h[b] = 0.25 * l + 0.5 * src[b] + 0.25 * r;
    }
}

/// 上升方向的混合：`out = prev + (raw - prev) * ATTACK`。
///
/// ⚠️ **不再是"攻击即时"。** 即时攻击允许一根柱子在**一帧之内**从 0 跳到满格 —— 那是"喷泉"
/// 的第二个来源，频率方向的模糊（[`blur_bands`]）拦不住它。0.6 是一帧走完六成距离，仍然比
/// 释放（[`RELEASE`] = 0.15）快四倍，所以在屏上读起来还是"鼓点一响就起来"，只是不再一帧到位。
const ATTACK: f32 = 0.6;

/// 下降方向的混合：`out = prev * RELEASE + raw * (1 - RELEASE)`。
///
/// 0.15 而不是旧版的 0.55 —— 一帧只落 15%，柱子是**滑下来**的。
///
/// ⚠️ 这个数**不是**靠平滑度挑的：实测（`_scratch/spectrum_study.py`）释放系数从 0.15 一路到
/// 0.55，`|dh|` 均值只在 0.057 → 0.040 之间挪（本来就够平），但**跟踪误差**（平滑后的柱子离
/// 未平滑真值多远）从 0.024 涨到 0.038 —— 越迟钝。挑 0.15 是取"平滑已经够、还跟得上音乐"
/// 的那一头。
const RELEASE: f32 = 0.15;

/// Push one bake's levels through attack/release, so the bars **follow** the music instead of
/// teleporting.
///
/// ★ **这是六六那句「太灵敏所以观感跳跃很快」的一半答案**（2026-10-07，问了三次才定位）。
/// 真因从来不是节拍：旧 [`bands`] 把每一带除以那一帧自己的峰值，于是每次烘焙都是一张独立
/// 快照，一根柱子可以一帧之内从满格到零；放慢时钟只让跳变**变稀**、没有变小。另一半在
/// 固定刻度本身（见 [`bands`]）与频率方向的模糊（见 [`blur_bands`]）。
///
/// **两个方向现在都平滑。** 旧版是攻击即时 —— 允许一帧跳到满格，那正是喷泉的第二个来源。
/// 上升用 [`ATTACK`]，下降用 [`RELEASE`]，上升仍快四倍，所以鼓点还是鼓点。
///
/// `prev` belongs to the caller, not to this module — per-band history across frames is exactly
/// the state `compose` refuses to keep, and that is what lets every panel test call it with a
/// fresh `Content` and get the same answer twice.
pub fn ballistics(prev: &mut [f32; BANDS], now: [f32; BANDS]) -> [f32; BANDS] {
    let mut out = [0.0f32; BANDS];
    for (b, raw) in now.iter().enumerate() {
        let p = prev[b];
        out[b] = if *raw >= p {
            p + (*raw - p) * ATTACK
        } else {
            p * RELEASE + *raw * (1.0 - RELEASE)
        };
    }
    *prev = out;
    out
}

/// The peak of a tap, 0..=1 — the panel's own level reading, beside the bars.
///
/// **Still worth its row after [`bands`] went absolute.** The bars ride a fixed dB scale now, so
/// this is no longer the *only* honest reading on the face — but it is a plain sample peak rather
/// than a per-band estimate, and it is what says "this passage is quiet" when every bar is sitting
/// on the floor.
pub fn peak(tap: &[i16]) -> f32 {
    tap.iter()
        .map(|v| v.unsigned_abs() as f32 / 32_768.0)
        .fold(0.0, f32::max)
        .min(1.0)
}

// ── The two panels ───────────────────────────────────────────────────────────────────────────

/// Which face to compose. **The same two the app's `Panel` names**, and an enum here rather than
/// a bool so a third face is a compile error rather than a silent `else`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Face {
    /// Song name and the spectrum.
    Name,
    /// Scrolling lyrics, or the name alone when the track has none.
    Lyrics,
}

/// What the panel is showing, as the words and the numbers behind them.
///
/// **A plain struct rather than a dozen parameters**, because the two faces want different
/// subsets of the same facts and a caller that has to pass an empty spectrum to say "no
/// spectrum" is a caller that will eventually pass a real one by accident.
pub struct Content<'a> {
    pub face: Face,
    /// The song's own name. Always shown, on both faces: a panel with no name is a panel that
    /// does not say what it is playing.
    pub title: &'a str,
    /// The artist, when the container said one. Empty is normal, not a fault.
    pub artist: &'a str,
    /// The lines around the one being sung, oldest first. Empty for a card with no `.lrc`.
    pub lines: &'a [String],
    /// The mono tap the bands come from.
    pub tap: &'a [i16],
    /// The tap's rate, for the Goertzel.
    pub rate: u32,
    /// The band levels to draw, **already smoothed** by [`ballistics`].
    ///
    /// ★ **`None` means "compose them straight from `tap`"**, with no history — which is what a
    /// test or a one-shot render wants, and it is what keeps `compose` a pure function of its
    /// `Content`. The deck passes `Some`, because it is the one caller that runs a frame *after*
    /// a frame and can therefore carry per-band state.
    pub levels: Option<[f32; BANDS]>,
    /// Seconds into the track, and its length. Both 0.0 when the container said neither.
    pub elapsed: f32,
    pub length: f32,
    /// How far the title has scrolled, in logical pixels; 0 is "at the left edge".
    ///
    /// **A marquee rather than a clip, because that is what the era did and a clipped title
    /// loses the end of the name.** The caller computes it from the clock; a title that fits
    /// passes 0 and the panel draws it still. See [`title_col_w`] and [`title_width_px`].
    pub scroll: i32,
    /// The transport bar below the LCD. **`None` draws no bar**, which is what a caller that
    /// wants the LCD alone gets — the bar is part of the frame, so "no bar" means the frame
    /// simply stops being asked for one.
    pub bar: Option<Bar>,
    /// The equaliser preset, printed at the right end of the artist's row. **Not an `Option`**:
    /// the deck always has one, and `NULL` is the honest answer for "nothing applied" — it is a
    /// preset in the table, not a special case.
    pub eq: crate::eq::Eq,
}

/// The width the title may use. **The whole panel minus its margins** — the name block is the
/// title's alone now that the clock has moved down to the strip, so a long name has the full
/// 232 logical pixels before it has to scroll.
pub fn title_col_w() -> i32 {
    content_x1() - content_x0()
}

/// **The name block's one line** — `artist - title`, or the title alone when the container gave
/// no artist.
///
/// ★ **Upper-cased here, in the one function every consumer shares.** The panel sets its type in
/// caps (it always has — see `text::plan_fit`), and the *caller* measures this string while the
/// panel draws it. If the case were applied in either of those two places instead of here, the
/// frontend would measure `CrazySix` and the panel would draw `CRAZYSIX` — a seventh of a line
/// wider — and the marquee would either start late or clip a title that fits.
pub fn title_text(artist: &str, title: &str) -> String {
    let artist = artist.trim();
    let name = if artist.is_empty() {
        title.to_string()
    } else {
        format!("{artist} - {title}")
    };
    name.to_uppercase()
}

/// A line's width **as the name block sets it**, for a caller that has to know whether it fits.
pub fn title_width_px(line: &str) -> i32 {
    measure_px(line, prose_plan())
}

/// A line's width at the **lyric's** setting. Same plan as the title; kept as a name because the
/// caller asking is laying out a different band.
pub fn body_width_px(line: &str) -> i32 {
    measure_px(line, prose_plan())
}

/// A line's width as the **strip** sets it — the tags plan, i.e. the 8px Latin face.
///
/// ⚠️ **A different plan from the title's**, which is the whole point of having two: a clock
/// measured with the name block's plan would come out twice as wide as it is drawn, and the
/// mode word would be pushed out of the strip.
pub fn strip_width_px(line: &str) -> i32 {
    measure_px(line, tag_plan())
}

/// How far the title has scrolled at `now_ms`, for a line `px` wide in a column `col_w`.
///
/// **Quantised to whole steps of [`MARQUEE_STEP`]**, because the caller re-bakes the panel only
/// when this number changes: a smooth pixel-per-frame scroll would re-bake the whole 240×160
/// frame sixty times a second, which is the cost the bake gate exists to avoid.
pub fn marquee_scroll(px: i32, col_w: i32, now_ms: u64) -> i32 {
    if px <= col_w {
        return 0;
    }
    let span = px + MARQUEE_GAP;
    let raw = (now_ms / 1000 * MARQUEE_PX_PER_SEC as u64) as i32 % span.max(1);
    (raw / MARQUEE_STEP) * MARQUEE_STEP
}

/// How fast a long title travels, in logical pixels per second. **Two ideographs a second** —
/// the speed a MiniDisc deck's title ran at, and slow enough to read.
pub const MARQUEE_PX_PER_SEC: i32 = 24;
/// The scroll is quantised to this many pixels; see [`marquee_scroll`].
pub const MARQUEE_STEP: i32 = 3;
/// The blank gap between the title's tail and its head as it wraps.
const MARQUEE_GAP: i32 = 16;

// ── The vertical plan ────────────────────────────────────────────────────────────────────────
//
// Every row below is derived from `line_h()` at the call site, not written as a number, so a
// different face moves the whole panel with it rather than leaving one band where it was.
//
// ★ **主次现在靠"两个字号"，不靠"放大主角"** —— 上一版写的是：这块面板只有 12 的整数倍
//   干净，所以歌名只能拉到 24px 当主角、其余一律 12px。2026-10-06 换了面之后这条约束没了：
//   汉字有自己 16px 的设计面，拉丁有自己 8px 的设计面，**两者都干净**（实测半调 0.0%），
//   于是"汉字大、拉丁小"是把 **两个各自原生** 的字号并排，而不是把一个面拉伸或压缩。
//   四段的顺序没变：**名字 → 元信息 → 主体 → 走带区**。

/// The name block's row — the first row inside the frame.
pub fn title_y() -> i32 {
    content_y0()
}
/// The name block's own line height, in logical rows.
///
/// **One line now.** The artist used to be a row of its own below a 24px headline; it is now
/// part of the name (`artist - title`) on a single 16px line, which is what frees the rows the
/// lyrics and the meta row use.
pub fn headline_h() -> i32 {
    line_h() as i32
}

/// The meta row's top (`PLAY` …… `[EQ]`), **placed by ink, not by arithmetic**.
///
/// ★ **必须在「歌名行墨迹底」与「正文框顶」之间按墨迹居中**，不能按 em 框居中。
///   名字那一行的 em 框是 16 行，而它的墨只占其中一部分（纯中文约 0..15，纯拉丁只有
///   中下段），所以"框底"和"字底"差得远 —— 按框算会得到肉眼可见偏上的一行。
///   [`text::plan_ink_span`] 给的就是墨相对本行 em 框顶的偏移。
///
/// 它是**内容相关**的（歌名是中文还是纯英文，墨底差 8 行），所以这是个函数而不是常量；
/// 但 `body_top()` 是常量，正文框不会跟着动。
pub fn meta_y(artist: &str, title: &str, transport: Transport, eq_name: &str) -> i32 {
    let title_ink_bottom =
        title_y() + text::plan_ink_span(&title_text(artist, title), &prose_plan()).1;
    let probe = format!("{} [{}]", transport.word(), eq_name);
    let (lo, hi) = text::plan_ink_span(&probe, &tag_plan());
    ((title_ink_bottom + body_top() - lo - hi) as f32 / 2.0).round() as i32
}
/// The body's first row.
///
/// ⚠️ 分隔线没有了：外面有一圈 2px 的边框、频谱自己还有一圈框，再在中间拉一条 1px 的线
/// 就是第三条横线了 —— 结构已经由框说清楚，不需要再画一遍。
///
/// = `title_y + headline_h + line_h + 2` = 6 + 16 + 16 + 2 = **40**。中间那一行就是元信息行。
pub fn body_top() -> i32 {
    title_y() + headline_h() + line_h() as i32 + 2
}
/// The strip's text band: **one body line**. 第九轮回到 12px（用户："走带区数字用回 12px 字体"）。
pub fn strip_text_h() -> i32 {
    line_h() as i32
}
/// The strip's progress track, in rows. **Inside the strip**, under the text: a segmented bar on
/// paper is a bar the grid meshes, and on a four-shade palette a meshed `INK` block and a meshed
/// `RULE` block come out within a few levels of each other — so the bar stops reading as a bar.
/// On the strip's own ink, "played" is paper and "to come" is rule, and both are plain.
pub const PROG_TRACK_H: i32 = 4;
/// Blank rows above and below the track inside the strip. **They are what "centred" means here**:
/// the track sits halfway between the text and the strip's bottom edge, not flush under the text.
/// 第九轮加宽到 4：键排没了，走带区有地方了（用户："走带区可以适当加宽一些"）。
pub const PROG_PAD: i32 = 4;
/// **歌词行距：21 行**（用户 2026-10-06 定的）。
///
/// 比一行字（16 行）多 5 行 —— 四行歌词因此铺满正文框（86 行）而不是挤在中间留一大块白。
/// 它**不是** `line_h()` 的倍数，也不该是：行距是排版的选择，行高是字体的属性。
pub const LYRIC_STEP: i32 = 21;
/// The inverted strip's height: text, gap, track, gap.
pub fn strip_h() -> i32 {
    strip_text_h() + PROG_PAD + PROG_TRACK_H + PROG_PAD
}
/// The inverted strip's first row — **贴着边框的内下沿**（用户："走带区贴底"）。
pub fn strip_y() -> i32 {
    LOGICAL_H as i32 - FRAME - strip_h()
}
/// The strip's left and right edges: **边框的内侧**，比内容区更宽 —— 它是一条贴边的带子。
pub fn strip_x0() -> i32 {
    FRAME
}
pub fn strip_x1() -> i32 {
    LOGICAL_W as i32 - FRAME
}
/// The progress track's first row — centred in the strip's lower half.
pub fn prog_y() -> i32 {
    strip_y() + strip_text_h() + PROG_PAD
}
/// The body's last row (exclusive) — two rows of paper above the strip.
pub fn body_bot() -> i32 {
    strip_y() - 2
}

/// Compose one frame of the panel.
///
/// **Everything is drawn at the logical size and nothing is drawn twice**: the mask and the
/// upscale are [`render`]'s job, so this function is about what the panel says and that
/// function is about how a screen says it.
pub fn compose(c: &Content<'_>) -> Frame {
    let mut f = Frame::panel();

    // ── ① 边框与浅边。**先画框，再画内容** —— 内容全都内缩在框里面（见 content_x0 等）。
    //
    //    框是 2px 的最深色（与歌名、频谱柱同一个墨档）；框**内侧**再沿上方与左方铺一条
    //    `EDGE` 的浅边 —— 光从左上打过来，这一圈就成了一块**凹进去的屏**。
    //    用户："画面四周按最深色围 2px 的圈并做 45 度的浅边（即上方和左方的框内侧）"。
    frame(&mut f);

    // ── ② 名字块。**一行，居中** —— 卡座把歌名印在显示屏正中。歌手名并入这一行，
    //    格式 `歌手名 - 歌名`；汉字 16px、拉丁 8px 由 [`plan`] 自动分流、共用一条基线。
    //    放不下时走跑马灯（`c.scroll`）。
    let col_w = title_col_w();
    let name = title_text(c.artist, c.title);
    marquee(&mut f, &name, content_x0(), col_w, title_y(), INK, c.scroll);

    match c.face {
        Face::Name => spectrum_body(&mut f, c),
        Face::Lyrics => lyric_body(&mut f, c),
    }

    if let Some(b) = c.bar.as_ref() {
        bar(&mut f, b);
    }
    // ── ③ 元信息行：左传输态单词、右 EQ 预设名。整行按墨迹居中在歌名与正文框之间。
    meta_row(&mut f, c);
    // ── ④ 悬浮感：给墨铺右下方向的浅边。**频谱区不铺**（见 `emboss` 的 skip）。
    emboss(&mut f, body_top(), body_bot());
    f
}

/// 边框 + 内侧浅边。
///
/// 上、下、左、右各 2px 的墨；再在框的**内侧**沿上方与左方各铺一条 1px 的 `EDGE` ——
/// 两条在左上角相接，读作"光从左上来、这块屏是凹的"。右下不铺：那两侧的"亮边"在
/// 凸起的方向上，而这块屏是凹的。
fn frame(f: &mut Frame) {
    let (w, h) = (LOGICAL_W as i32, LOGICAL_H as i32);
    f.fill(0, 0, w, FRAME, INK);
    f.fill(0, h - FRAME, w, FRAME, INK);
    f.fill(0, 0, FRAME, h, INK);
    f.fill(w - FRAME, 0, FRAME, h, INK);
    // 浅边：框内侧的上方一行、左方一列。
    f.fill(FRAME, FRAME, w - 2 * FRAME, BEVEL, EDGE);
    f.fill(FRAME, FRAME, BEVEL, h - 2 * FRAME, EDGE);
}

/// **元信息行**：左边**传输态的单词**，右边 **EQ 预设名**。
///
/// ★ 这一行曾经是「歌手名那一行的两端」，两端各一个图标（▶ / ‖ / ■）和一个 EQ 名。
///   2026-10-06 用户定了两条：**歌手名并入歌名行**（那是上面那一行的事），
///   **图标换成单词**。于是整行只剩两个词，左一个右一个，中间是空的 —— 它读起来像
///   设备面板上那种状态行（`PLAY` …… `[Rock]`），而不是贴在画面上的记号。
///
/// ⚠️ **停了画规则档**（没在放就该退到次要），在放/暂停画墨；两个词同一档，所以
///    只有"整行"的轻重在变，不会一个词深一个词浅。
fn meta_row(f: &mut Frame, c: &Content<'_>) {
    let transport = c.bar.as_ref().map(|b| b.transport).unwrap_or(Transport::Stopped);
    let y = meta_y(c.artist, c.title, transport, c.eq.name());
    let shade = if transport == Transport::Stopped { RULE } else { INK };
    text_left(f, transport.word(), content_x0(), y, shade, tag_plan());
    // 右：EQ 预设名加中括号 —— 与走带区正中那个播放模式同一套写法。
    let eq = format!("[{}]", c.eq.name());
    text_right(f, &eq, content_x1(), y, RULE, tag_plan());
}

/// **点阵的悬浮感**：每个墨色像素的**右下**错开一格，铺一层浅色（[`EDGE`]，纸与规则之间的那一档）。
///
/// 一格一格地看，这是给点阵加了一道右下方向的浅边；连起来看，**整块墨就浮在纸上了** ——
/// 这就是"浮雕"的全部做法，没有渐变、没有模糊，只是把同一个形状朝光源的反方向挪一格。
///
/// # 投影只落在"背景"上，不落进"洞"里
///
/// ★ **洞不是背景。** 反白的键是"墨底纸字"，那个字是墨里的一个**洞**；墨往洞里投影，会把
/// 字靠左上的一圈啃掉 —— 这不是审美问题，是**图标真的少了一圈像素**（居中测试当场就红了，
/// 左右留白 14 : 12）。所以先**从四边把背景走一遍**（只走非墨的格子），只有背景上的纸才接投影。
///
/// 顺带解决两件事：频谱的背景柱是规则档，不是纸，所以墨柱不往它上面投（那根柱的"满格"
/// 是它存在的理由）；反白状态条是墨，投影落在它外面的纸上，条子因此看着是**浮起来的一块**。
///
/// ⚠️ 代价是**一帧一次全帧扫描加一次泛洪**（240×160 = 38400 格）。可以接受：面板只在输入
/// 变化时才重烘（见 `frontend::sync_lcd_panel` 的烘焙门），不是每帧都跑。
fn emboss(f: &mut Frame, skip0: i32, skip1: i32) {
    let (w, h) = (f.w as i32, f.h as i32);
    let mut bg = vec![false; (w * h) as usize];
    let mut stack: Vec<i32> = Vec::new();
    {
        let px = &f.px;
        let seed = |i: i32, bg: &mut Vec<bool>, stack: &mut Vec<i32>| {
            let i = i as usize;
            if px[i] != INK && !bg[i] {
                bg[i] = true;
                stack.push(i as i32);
            }
        };
        for x in 0..w {
            seed(x, &mut bg, &mut stack);
            seed((h - 1) * w + x, &mut bg, &mut stack);
        }
        for y in 0..h {
            seed(y * w, &mut bg, &mut stack);
            seed(y * w + w - 1, &mut bg, &mut stack);
        }
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            if x > 0 {
                seed(i - 1, &mut bg, &mut stack);
            }
            if x < w - 1 {
                seed(i + 1, &mut bg, &mut stack);
            }
            if y > 0 {
                seed(i - w, &mut bg, &mut stack);
            }
            if y < h - 1 {
                seed(i + w, &mut bg, &mut stack);
            }
        }
    }
    let mut add: Vec<usize> = Vec::new();
    for y in 0..h - 1 {
        // ★ **频谱区不铺浅边**（用户："频谱区取消浅边但保留立柱背景"）：柱子已经有背景柱托着，
        //   再给每根柱描一道边就是往表里加噪点。跳过 `[skip0, skip1)` 这一段。
        if y >= skip0 && y < skip1 {
            continue;
        }
        for x in 0..w - 1 {
            if f.px[(y * w + x) as usize] != INK {
                continue;
            }
            // ★ **再淡 50%**（用户）：四阶色板里 `EDGE` 已经是最浅的一档，再淡没有色可用 ——
            //   所以走**抖动**：只铺一半的格子（棋盘），远看就是"比 EDGE 再淡一半"的那一档。
            //   这是 1-bit 面板唯一诚实的"中间色"做法，当年真机也是这么调灰的。
            if (x + y) % 2 != 0 {
                continue;
            }
            let j = ((y + 1) * w + (x + 1)) as usize;
            if bg[j] && f.px[j] == PAPER {
                add.push(j);
            }
        }
    }
    for j in add {
        f.px[j] = EDGE;
    }
}

/// The spectrum, inside the sheet's framed section — **closed, and with nothing written in it**.
///
/// ★ The label used to sit in a notch in the top border. It is gone, and the notch with it: the
/// panel has exactly one framed thing on it and nothing else it could be taken for, so the word
/// was a caption on a picture that captions itself.
///
/// ★ **Every band draws twice.** First a full-height column in the rule shade — the meter's own
/// track, set in the same pale weight as the artist's name — then the live level in ink on top of
/// it. A bar with no track behind it can only be read as "a bar"; with the track under it the eye
/// reads a *level*, and a silent passage still shows an instrument that is switched on.
///
/// The bars are laid out **inside** the frame and centred in it: the pitch comes from the frame's
/// inner width and the remainder is split evenly, so the box's two margins match.
/// 第 `b` 根柱子占的 `(左沿, 宽)` —— 把 [`BANDS`] 根柱子铺进 `inner_w` 列里。
///
/// ★ **恒 [`BAND_GAP`] 列间隙，余数摊进柱宽**，而不是"每根占整数格、余数丢在两头"。
///   量级看 64：226 列 ÷ 64 = 3.53，按"每根 3 列"只占 192 列，**两端各空 17 列**，谱面会缩在框中间；
///   按这里写法左沿 = `inner_x0 + b·inner_w/BANDS`、柱宽 = 下一根左沿 − 本根左沿 − 间隙，
///   于是**无论 `BANDS` 是多少都铺满整条框**，代价是柱宽在 2/3 之间交替。
///   32 根时 226÷32 = 7.06 本来近乎整除，两种写法看不出区别 —— 这是加倍立柱时才暴露的。
///
/// `pub` 是为了守门测试能直接算版式（那一层拿不到 `Frame`）。
pub fn spectrum_bar(inner_x0: i32, inner_w: i32, b: i32) -> (i32, i32) {
    let left = |k: i32| inner_x0 + k * inner_w / BANDS as i32;
    (left(b), (left(b + 1) - BAND_GAP - left(b)).max(1))
}

fn spectrum_body(f: &mut Frame, c: &Content<'_>) {
    let (x0, y0) = (content_x0(), body_top());
    let (x1, y1) = (content_x1(), body_bot());
    section(f, x0, y0, x1, y1, "");

    let inner_x0 = x0 + 1;
    let inner_w = (x1 - 1) - inner_x0;

    // 上下各让开 1px 描边 + 1px 余量；柱子站在 `floor` 上，往上长。
    let top = y0 + 2;
    let floor = y1 - 2;
    let span = (floor - top).max(1);
    let levels = c.levels.unwrap_or_else(|| bands(c.tap, c.rate));
    for (b, level) in levels.iter().enumerate() {
        let (bx, bw) = spectrum_bar(inner_x0, inner_w, b as i32);
        // 底：整格**浅色**（`EDGE`，比规则档再淡一档 —— 用户："背景柱再淡 50%"）。
        // 色板是"最亮在前"，规则档在索引 2、`EDGE` 在 1；在"原生灰"里那是 82 → 173，
        // 亮度上正好是往纸靠了一半，就是 50%。
        f.fill(bx, top, bw, floor - top + 1, EDGE);
        // 前景：实时电平，从底线往上长。
        let h = (level * span as f32) as i32;
        f.bar(bx, floor, bw, h, INK);
    }
}

/// Draw `line` centred in `[cx0, cx1]`, starting at `top`, but **only the rows that fall inside
/// `[clip0, clip1)`** — the frame's own interior. Used for the framed lyric panel, where the line
/// above (and below) the one being sung runs past the box and must be cut off by the frame rather
/// than drawn over it. Horizontal clipping is `text_centred_between`'s own; the vertical clip here
/// is what makes a neighbour line *partial*, which is the whole "scrolling inside the box" look.
// Two ranges — the line's own box and the frame's interior — plus the line, its shade and its
// top. Splitting the ranges into tuples reads as `(a, b)` pairs that a caller can swap by
// accident, which is exactly the bug this clip exists to avoid.
#[allow(clippy::too_many_arguments)]
fn text_centred_clipped(
    f: &mut Frame,
    line: &str,
    cx0: i32,
    cx1: i32,
    top: i32,
    shade: u8,
    clip0: i32,
    clip1: i32,
) {
    if line.trim().is_empty() || cx1 <= cx0 {
        return;
    }
    let plan = prose_plan();
    let l = layout(line, f32::INFINITY, 1, plan);
    let Some(text_line) = l.lines.first() else {
        return;
    };
    let tw = measure_px(text_line, plan).max(1);
    let h = line_h() as i32;
    let strip = text::plan_coverage(tw as u32, h as u32, &l, &plan);
    let start = cx0 + ((cx1 - cx0) - tw) / 2;
    for y in 0..h {
        let dy = top + y;
        if dy < clip0 || dy >= clip1 {
            continue;
        }
        for col in 0..tw {
            let dx = start + col;
            if dx < cx0 || dx >= cx1 {
                continue;
            }
            if strip[(y as u32 * tw as u32 + col as u32) as usize] >= 128 {
                f.put(dx, dy, shade);
            }
        }
    }
}

/// The lyric lines, **framed like the spectrum and scrolling inside the frame**.
///
/// ★ **歌词 4 行、行距 [`LYRIC_STEP`]（21），整块在正文框里垂直居中。**
///
/// 2026-10-06 用户定的版式。与上一版有两处不同，都是为了让"四行"真的读得开：
/// * **行距 21 而不是一行字高 16。** 一行字的 em 框是 16 行，四行叠起来 64 行，而正文框有
///   86 行——按 16 走会在框里留出一大块空，看着像字沉在底下；21 行距（比字高多 5 行）
///   正好把四行铺满它。
/// * **整块居中，不是"唱到的那行钉在正中"。** 钉住单行意味着上下两行会被框切掉半截，
///   而四行是有地方放下的，切它没有道理。唱到的那行仍用满墨、其余退一档 —— **层级靠墨，
///   不靠裁切**。
///
/// 行数超过 4 时（窗口比框高很多）仍按同一套算：块居中、溢出框的行被裁掉——裁切是
/// [`text_centred_clipped`] 的活，它保证越界的行不会画到框外。
fn lyric_body(f: &mut Frame, c: &Content<'_>) {
    let (x0, y0) = (content_x0(), body_top());
    let (x1, y1) = (content_x1(), body_bot());
    // 与频谱区同一道外框（1px RULE，无标签）。
    section(f, x0, y0, x1, y1, "");
    // 框内区域：上下让开描边，左右让开 1px。
    let (cx0, cx1) = (x0 + 1, x1 - 1);
    let (cy0, cy1) = (y0 + 2, y1 - 1);
    if c.lines.is_empty() {
        // No lyrics: the name again, and nothing pretending otherwise. **This is the whole
        // reason the two faces exist separately** — a card with no `.lrc` gets a panel that says
        // what it is playing, not an empty box.
        text_at(f, c.title, cy0 + (cy1 - cy0) / 2 - line_h() as i32 / 2, INK, prose_plan());
        return;
    }
    let n = c.lines.len() as i32;
    let h = line_h() as i32;
    let total = (n - 1) * LYRIC_STEP + h;
    let first = cy0 + ((cy1 - cy0) - total) / 2;
    let mid = c.lines.len() / 2;
    for (i, line) in c.lines.iter().enumerate() {
        let em_top = first + i as i32 * LYRIC_STEP;
        if em_top + h <= cy0 || em_top >= cy1 {
            continue; // 整行都在框外（窗口比框高很多时才会发生）
        }
        // **The sung line at full ink and the rest one step down.** A lyric panel where every
        // line is the same weight is a wall, and the eye has nowhere to land first.
        let shade = if i == mid { INK } else { RULE };
        text_centred_clipped(f, line, cx0, cx1, em_top, shade, cy0, cy1);
    }
}

/// The sheet's framed section: a 1px rule box with its label in the top-left corner, the top
/// border **broken** where the label sits. That notch is what makes a box read as a named
/// section rather than as a rectangle someone drew.
fn section(f: &mut Frame, x0: i32, y0: i32, x1: i32, y1: i32, label: &str) {
    f.fill(x0, y0, x1 - x0, 1, RULE);
    f.fill(x0, y1 - 1, x1 - x0, 1, RULE);
    f.fill(x0, y0, 1, y1 - y0, RULE);
    f.fill(x1 - 1, y0, 1, y1 - y0, RULE);
    if label.trim().is_empty() {
        return;
    }
    let lx = x0 + PAD;
    let lw = measure_px(label, prose_plan());
    // The notch: the top border, cleared where the label's own width plus a hair on each side.
    f.fill(lx - 2, y0, lw + 4, 1, PAPER);
    text_left(f, label, lx, y0 + 1, RULE, prose_plan());
}

/// The sheet's PROGRESS BAR, drawn **inside the inverted strip**: discrete blocks, not a smooth
/// fill. Every bar on a monochrome LCD of the era is drawn as a row of segments, because that is
/// how the driver could address it.
///
/// **The two levels are paper and rule**, not ink and rule: the strip's own background is ink, so
/// "played" is the lightest shade and "to come" is one step down. Drawn on paper instead, a
/// meshed `INK` and a meshed `RULE` land within a few levels of each other and the bar stops
/// reading as a bar — which is what the first attempt did.
/// The strip's progress track: **a run of segments, centred in the strip.**
///
/// ★ **段宽和段数是从可用宽度反推的，不是写死的。** 写死 `SEG = 5` 会让整排的余数落在一边
/// （上一版左边距 5、右边距 6，一像素的差在一排等宽方块里看得出来）。现在：
/// 段数按"一段约 6px"估，节距凑成整数，**余数必须是偶数** —— 差一格就把段数减一。
/// 少一段看不出来，偏一格看得出来。
fn segmented_bar(f: &mut Frame, y: i32, frac: f32) {
    const GAP: i32 = 2;
    /// 一段大约多宽（缝另算）；真实段宽由可用宽度反推。
    const SEG_WANT: i32 = 6;
    let avail = strip_x1() - strip_x0();
    let mut n = (avail / (SEG_WANT + GAP)).max(1);
    while n > 1 && (avail - (n * ((avail + GAP) / n) - GAP)) % 2 != 0 {
        n -= 1;
    }
    let pitch = (avail + GAP) / n;
    let seg = (pitch - GAP).max(2);
    let run = n * pitch - GAP; // 最后一段后面没有缝
    let x = strip_x0() + (avail - run) / 2;
    let lit = (frac.clamp(0.0, 1.0) * n as f32).round() as i32;
    for i in 0..n {
        f.fill(
            x + i * pitch,
            y,
            seg,
            PROG_TRACK_H,
            if i < lit { PAPER } else { RULE },
        );
    }
}

/// Draw a line, **centred in its column when it fits and scrolling when it does not**.
///
/// **Clipped to `[x, x + w)` by construction**, so a title wider than its column can never run
/// past the margin. When it scrolls, the tail is drawn a gap behind the head, so the wrap is
/// continuous rather than a jump back to the start.
///
/// `centre` is the panel's own choice and not a general one: the name block is centred (a deck
/// prints the song's name in the middle of its display), while a scrolling line has to start at
/// the left edge or the eye cannot follow it.
///
/// **It measures through [`plan`] like everything else**, so a `五月天 - 人生海海` measures with
/// the Chinese face and a `CRAZYSIX - Track 1` with the Latin one — and the width that decides
/// "does it fit" is the same width `plan_coverage` will draw.
fn marquee(f: &mut Frame, line: &str, x: i32, w: i32, top: i32, shade: u8, scroll: i32) {
    if line.trim().is_empty() || w <= 0 {
        return;
    }
    let plan = prose_plan();
    let l = layout(line, f32::INFINITY, 1, plan);
    let Some(text_line) = l.lines.first() else {
        return;
    };
    let tw = measure_px(text_line, plan).max(1);
    let h = line_h() as i32;
    let strip = text::plan_coverage(tw as u32, h as u32, &l, &plan);
    let mut blit = |ox: i32| {
        for y in 0..h {
            let dy = top + y;
            if dy < 0 || dy >= f.h as i32 {
                continue;
            }
            for col in 0..tw {
                let dx = ox + col;
                if dx < x || dx >= x + w {
                    continue;
                }
                if strip[(y as u32 * tw as u32 + col as u32) as usize] >= 128 {
                    f.put(dx, dy, shade);
                }
            }
        }
    };
    if scroll <= 0 {
        // Fits ⇒ centred across the column; too wide ⇒ scrolled, and a scroll only reads if it
        // starts at the left edge.
        let ox = if tw <= w { x + (w - tw) / 2 } else { x };
        blit(ox);
    } else {
        blit(x - scroll);
        blit(x - scroll + tw + MARQUEE_GAP);
    }
}

// ── The transport bar ─────────────────────────────────────────────────────────────────────
//
// ★ **矩形键 + 键下一行小标签**，与六六给的 LCD UI KIT 的 MEDIA CONTROLS 同一套语言。
//   写在这里而不是另一张纹理，是因为「整屏 720×480 是同一套渲染」——见 LOGICAL_H 的注释。
//
//   第四轮从**方形**改成矩形（六六："改成三比二或二比一的矩形按钮，不就把底下字母标识的
//   空间让出来了？顺便还能多让点空间给频谱"）。算术是：方形键要 33 行，40 行的走带区里
//   再塞一行标签（14 行）就装不下；矩形键 20 行 + 标签 14 行 = 36 行，**省下的 4 行给了频谱**
//   （`LCD_H` 120 → 124）。
//
//   键位由左至右（六六定的）：MENU / ◀◀ / ▶ / ▶▶ / MODE。

/// The label band's height: **one line of the 10px face**, derived like every other vertical
/// number here. 14 on this font — and it must move with the face if the face ever changes,
/// which is why it is a function and not the constant 14.
pub fn label_h() -> i32 {
    line_h_at(LABEL_PX) as i32
}

/// What the transport is doing. **Three states, not two** —— 一台只能 play/pause 的机器
/// 停不下来，而随机播完一首之后你要的正是「停」。
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Transport {
    #[default]
    Stopped,
    Playing,
    Paused,
}

impl Transport {
    /// **The word the meta row prints for it.**
    ///
    /// ★ 2026-10-06：原来是三个图标（▶ / ‖ / ■）。用户定的：**取消图标、改为单词** ——
    ///   图标在 8px 拉丁的旁边是一块孤零零的几何形，而这一行现在是一句话
    ///   （`PLAY` …… `[EQ]`），一个记号插在句子左边读起来是两套语言。
    ///   词是英文，理由同 [`Key::label`]：拉丁面没有汉字，而这三个词本来就是面板的原文。
    pub fn word(self) -> &'static str {
        match self {
            Transport::Stopped => "STOP",
            Transport::Playing => "PLAY",
            Transport::Paused => "PAUSE",
        }
    }
}

/// How the list repeats. **Four stops**, which is what a deck of the era offered and what the
/// user asked for by name: 单曲循环 / 列表播放 / 列表循环 / 随机播放.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    /// One song, over and over.
    #[default]
    One,
    /// The list, in order, once — and then it stops.
    Once,
    /// The list, in order, from the top again.
    Repeat,
    /// The list, in no particular order.
    Shuffle,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::One, Mode::Once, Mode::Repeat, Mode::Shuffle];

    /// The next mode round — **four positions on one key**, which the kit's own ON/OFF switch
    /// becomes when it has four stops. The glyph says which stop you are on.
    pub fn next(self) -> Mode {
        match self {
            Mode::One => Mode::Once,
            Mode::Once => Mode::Repeat,
            Mode::Repeat => Mode::Shuffle,
            Mode::Shuffle => Mode::One,
        }
    }

    /// The previous mode, the reverse of [`Mode::next`]. Used to **undo the play-mode advance a
    /// lone MODE tap made** when a quick second tap turns out to be an EQ cycle instead — so a
    /// double-tap lands on "EQ moved, play mode where it was" rather than "both moved".
    pub fn prev(self) -> Mode {
        match self {
            Mode::One => Mode::Shuffle,
            Mode::Once => Mode::One,
            Mode::Repeat => Mode::Once,
            Mode::Shuffle => Mode::Repeat,
        }
    }

    /// The word the strip prints for it, **inside square brackets**.
    ///
    /// ★ 第九轮：四个符号换回英文词（用户："原本表示播放模式的符号换回英文单词加中括号居中
    /// 显示"）。词是**英文**而不是 `lang.rs` 里那张双语表 —— 用户点的是英文词，而且
    /// `[列表循环]` 在 12px 上比 `[REPEAT]` 宽一倍，中间那一栏放不下。
    pub fn word(self) -> &'static str {
        match self {
            Mode::One => "ONE",
            Mode::Once => "ONCE",
            Mode::Repeat => "REPEAT",
            Mode::Shuffle => "SHUFFLE",
        }
    }
}

/// The five keys, left to right.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Key {
    /// Out of playing, into the list.
    Menu,
    /// Rewind on a disc, previous on a disc.
    Prev,
    /// Play, pause and stop on one key, because that is how a transport works.
    Play,
    /// Fast-forward, next.
    Next,
    /// How the list repeats.
    Mode,
}

impl Key {
    pub const ALL: [Key; 5] = [Key::Menu, Key::Prev, Key::Play, Key::Next, Key::Mode];

    pub fn index(self) -> usize {
        self as usize
    }

    /// The word printed under the key.
    ///
    /// ★ **Latin, and not in `lang.rs`.** Two reasons, and both are hard:
    /// * the label band is **14 rows** — one line of the 10px face — and the 10px face carries
    ///   **1076 ideographs against the 12px face's 18299**, so a Chinese label at this size comes
    ///   out with holes;
    /// * at 12px the line box is 16 rows, which is more than the band.
    ///
    /// So these are the era's own words — a 1999–2004 deck's transport was lettered in English —
    /// and they are the key's own name rather than a translatable string, which is exactly the
    /// case `lang.rs` says does not belong in it.
    pub fn label(self) -> &'static str {
        match self {
            Key::Menu => "MENU",
            Key::Prev => "PREV",
            Key::Play => "PLAY",
            Key::Next => "NEXT",
            Key::Mode => "MODE",
        }
    }
}

/// What the bar draws, gathered in one place so the drawer cannot read half from the app and
/// half from itself.
///
/// **`PartialEq` and not `Eq`**: the progress is an `f32`, so equality is the float one. Nothing
/// here is ever used as a map key, and a bar is compared to decide whether to re-bake, where a
/// "same to the bit" answer is exactly what is wanted.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Bar {
    pub transport: Transport,
    pub mode: Mode,
    /// Where the track is: seconds in, and its length. **The strip draws the pair from these** —
    /// the caller passes the numbers rather than a formatted string so the panel owns the one
    /// clock format (a second `m:ss` in the app is a second thing to keep in step).
    ///
    /// ⚠️ **第九轮没有第三个字段了**：曲目序号（`03/12`）与键排一起取消，焦点也随之取消 ——
    /// 屏上不再有"光标在哪颗键上"这件事可画。
    pub elapsed: f32,
    pub length: f32,
}

impl Bar {
    /// How far through the track, 0..=1 — **derived, not passed**, so the bar's own text and its
    /// own track can never disagree.
    ///
    /// **A length of zero is an empty track**, not a full one: a container that would not say how
    /// long the track is must not read as "finished".
    pub fn progress_frac(&self) -> f32 {
        if self.length > 0.0 {
            (self.elapsed / self.length).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}









/// The inverted status strip: **the panel's bottom band, flush to the frame's inner edge**.
///
/// ★ **第九轮：操作区整条取消了**（用户："取消操作区，需要显示状态就在左上角浮窗显示就好"）。
///   键排那 40 行还给了频谱，走带区也从一个 20 行的小条长成一条 28 行的带子 —— 它现在是
///   屏上除了频谱之外唯一的东西，所以它有资格占宽一点。
///
/// 条里三样，**左中右**：
/// * 左：**已播时间**（`0:37`）
/// * 中：**播放模式，英文词加中括号**（`[REPEAT]`）—— 用户："原本表示播放模式的符号换回
///   英文单词加中括号居中显示"。第九轮之前这里是四个符号，现在回到词。
/// * 右：**总时长**（`4:00`）—— 用户："原本居中的播放时间/曲目时间拆分到两侧显示"。
///
/// ⚠️ **`03/12` 那个曲目序号没有了**（用户："取消 TRACK 显示"）：盘里第几首是卡座自己的事，
///   屏上要说的是"放到哪儿了"。
pub fn bar(f: &mut Frame, b: &Bar) {
    let sy = strip_y();
    f.fill(strip_x0(), sy, strip_x1() - strip_x0(), strip_h(), INK);

    // ★ 三个字段都按**墨迹**在「走带条顶边 … 进度条顶」之间居中，各自算各自的 ——
    //   拉丁 8px 的墨只占它 em 框的上半（那一面把 descent 报成 0），按 em 框放会整体偏下。
    //   三条都是 tag（8px 拉丁），**量法和画法同一个 plan**。
    let plan = tag_plan();
    // 左：已播。**贴着内容区的左沿**，与上面的名字块同一条竖线。
    if b.elapsed > 0.0 || b.length > 0.0 {
        let t = clock(b.elapsed);
        text_left(f, &t, content_x0(), strip_text_y(&t), PAPER, plan);
    }
    // 右：总时长。
    if b.length > 0.0 {
        let t = clock(b.length);
        text_right(f, &t, content_x1(), strip_text_y(&t), PAPER, plan);
    }
    // 中：模式词加中括号，**在整幅面板上居中**。
    let mode = format!("[{}]", b.mode.word());
    text_centred_between(f, &mode, strip_x0(), strip_x1(), strip_text_y(&mode), PAPER, plan);

    // 进度轨：反白条的下半，在文字与条底之间居中。
    segmented_bar(f, prog_y(), b.progress_frac());
}

/// 走带条上那一行文字的 **em 框顶**：按墨迹居中在「走带条顶边」与「进度条顶」之间。
///
/// ★ 用户 2026-10-06 定的："走带条的播放时间与播放模式，在走带条顶边与进度条顶之间垂直居中"。
///   居中要按墨迹算 —— 见 [`text::plan_ink_span`] 的说明。
fn strip_text_y(line: &str) -> i32 {
    let (lo, hi) = text::plan_ink_span(line, &tag_plan());
    strip_y() + (((prog_y() - strip_y() - (hi - lo)) as f32) / 2.0).round() as i32 - lo
}

// ── The info page: the same screen, saying something else ────────────────────────────────────
//
// ★ 六六 2026-10-06：「信息页好丑，按播放界面的像素风重新做，共用播放器色板以及字体」。
//
// **它曾经不是这块屏。** 那一版走 `plate::word_face` —— 界面的 Plix Bold 矢量字体、
// `PCT_PX` 那个字号、逐行一张纹理，底色是 `opening()`。那是**货架与菜单那一套**渲染：
// 抗锯齿的矢量字、主题的一个色阶、**没有格网**。所以它和面板并排放的时候不像同一台机器 ——
// 不是"字号没调好"，是**两套渲染**。
//
// 现在它走**面板的每一个零件**：同一张 1-bit 帧、同一圈 2px 墨框与左上浅边（[`frame`]）、
// 同一套四阶色板（调用方给的 `ramp`）、同一张格网（[`render`] 的 `grid`）、同一套点阵字体、
// **同样两条 plan**（正文 [`prose_plan`] / 读数 [`tag_plan`]）。
//
// ⚠️ **所以它不是 `Content` 的第三张面。** `Content` 是"这一首正在放"的读数——歌名、歌词、
//   频谱、EQ、走带条——而这一页要的是碟片、曲目、时长、标签、键位指南，**没有一样在
//   `Content` 里**。塞进去就要给六个字段加 `Option` 并在 `compose` 的每一处判空，
//   而播放那一屏永远不需要它们。所以是独立的 [`Info`]，**共用零件、不共用数据**。
//
// ── 为什么是两页 ──
//
// 面板的逻辑帧是 **240×160**，而 `line_h()` 是 **16** ⇒ 一屏只有 **10 行**。这一页要说的
// 东西有二十来行（碟片 3 + 曲目 3 + 标签若干 + 键位 14），**一屏放不下是算术，不是排版**。
//
// ★ **于是分两页，而不是把字号缩到 8px。** 汉字面的设计尺寸是 16，**8px 上它有 62% 抗锯齿**
//   （见 `examples/font_size_audit.rs` 的实测）—— 缩字号换来的是"能塞下"和"字是脏的"，
//   而这一页的全部意义就是字。**宁可多按一次键。**
//
//   * **第 1 页 = 碟片与当前曲目**（含容器里的其它标签项）
//   * **第 2 页 = 键位指南**
//
// 翻页用 `← →`，底下一行常驻 `1/2   L / R 翻页`。这两个键**本来在这一页是空着的**
//   （它们是快退快进，而停下来看字的时候没有人要快退），所以拿它翻页不占用任何别的动作。
//   ⚠️ 页脚上**印的是 `L / R` 而不是箭头** —— 箭头在 `tag_plan` 下没有字形，见 `info_foot`。

/// 这一页要印的东西。**借用而不是拥有**——调用方（`App::deck_info`）手里已经是 `String`，
/// 而这一页一个字都不加工：它只决定**排在哪一档、写在哪一行**。
pub struct Info<'a> {
    /// 碟片名（= 文件夹名）。**这一页的身份**，用 [`prose_plan`]。
    pub disc: &'a str,
    /// 「曲目 12 · 48:20」那一行。用 [`tag_plan`]。
    pub disc_line: &'a str,
    /// 「3/12 歌名」那一行。用 [`prose_plan`]。
    pub track_line: &'a str,
    /// 歌手。用 [`prose_plan`]；空就是没有，**不留空行**。
    pub artist: &'a str,
    /// 「时长 3:45」。用 [`tag_plan`]。
    pub length_line: &'a str,
    /// 容器里剩下的标签项。**有就显示、没有就一行都不占。**
    pub tags: &'a [(String, String)],
    /// 键位指南，`(键, 说明)`。**两栏排**，见 [`info_keys`]。
    pub keys: &'a [(&'a str, &'a str)],
}

/// 详情里的一行，**带着它该用哪套 plan 与哪一档**。
///
/// ★★ **2026-10-06 六六：「示例页全用 16PX 的那个字体吧，观感好点」**
///   ——**这一页只有一套字号**：`prose_plan`（寒蝉 16px，汉字与拉丁同一张面）。
///
///   之前是两条 plan 混排（正文 16 + 读数 8），而那一版**难看的不是字号小，
///   是同一页里有两种字高**：`year 1998` 那一行只有 8px 高，它上下两行是 16px，
///   于是屏上出现一条**比别的行矮一半的横行**，扫过去像一道疤。
///   **一档比两档整齐**，这是这一版的全部理由。
///
/// ⚠️ **代价：拉丁也跟着变 16px**（`prose_plan` 的拉丁走的是汉字那张面）。
///   所以键位指南那些 `SELECT` `START` 从 53px 宽变成 ~96px —— **两栏 114px 装不下了**，
///   那一处的排法跟着改（见 `info_keys`）。
#[derive(Copy, Clone)]
enum Detail<'a> {
    /// 一句正文 —— [`prose_plan`]，墨。
    Prose(&'a str),
    /// 一个读数（时长）—— [`prose_plan`]，**规则档**（它比上面那两行次要）。
    Readout(&'a str),
    /// **一个标签项**：键与值并排，**一行**，都在 [`prose_plan`]。
    ///
    /// ⚠️ **不是"键一行、值一行"**：`year` 与 `1998` 各占一行的话，
    ///   一张有 6 个标签的盘要 12 行，而一页放得下 6 行 ⇒ **标签会被截掉一半**。
    ///   而它们本来是**一个词条**，拆成两行读作"两样东西"。
    Pair(&'a str, &'a str),
}

impl Info<'_> {
    /// 详情行表，**空的已经剔掉了**，所以每一项都真的占一行。
    ///
    /// ⚠️ **这个顺序就是屏上从上到下的顺序**，而它也是**分页的顺序**：
    ///   `info_page_frame` 按 `skip(page * per).take(per)` 取段。
    fn details(&self) -> Vec<Detail<'_>> {
        let mut out: Vec<Detail<'_>> = Vec::new();
        for t in [self.track_line, self.artist] {
            if !t.trim().is_empty() {
                out.push(Detail::Prose(t));
            }
        }
        if !self.length_line.trim().is_empty() {
            out.push(Detail::Readout(self.length_line));
        }
        // 标签项**接在后面**，所以标签多到装不下时被截掉的是**最后几个标签**，
        // 而曲名与歌手永远在 —— 那一页先说"这是什么歌"，再说这张盘有多少元数据。
        for (k, v) in self.tags {
            if v.trim().is_empty() {
                continue;
            }
            out.push(Detail::Pair(k.as_str(), v.as_str()));
        }
        out
    }
}

/// 这一页有几页 = **详情页数 + 1（键位）**。
///
/// ⚠️ **不是常数**：标签多的盘详情页就多。写死 2 的那一版会**静默丢掉第 3 页之后的标签**
///   —— 而"这张盘有 6 个标签项"里丢了 2 个，是屏上看不出来的那种信息损失。
pub fn info_pages(c: &Info<'_>) -> usize {
    info_detail_pages(c) + 1
}

/// 底部提示那一行的 em 框顶：`n/2   L / R 翻页`。
///
/// **贴着边框的内下沿**，与走带条同一个位置（`strip_y` 的做法）——这一页不画走带条，
/// 而它需要一条同样贴底的带子来说"怎么翻页"。**不贴底的话它就是正文里飘着的一行小字**，
/// 而贴底的那一行读作"状态"，与面板上走带条读作"状态"是同一件事。
pub fn info_foot_y() -> i32 {
    LOGICAL_H as i32 - FRAME - line_h() as i32
}

/// 正文最后一行的下沿（不含）。**在底部提示之上让开 2 行。**
fn info_body_bot() -> i32 {
    info_foot_y() - 2
}

/// 抬头块的底（不含）：**碟片名 + 碟片那一行读数**，两行 `line_h`。
fn info_head_bot() -> i32 {
    content_y0() + 2 * line_h() as i32
}

/// 第 1..n 页：碟片抬头 + 详情，**详情跨页流动**。
///
/// ★ **整页左对齐，不是居中** —— 这是它与面板同一个决定的延续：面板的歌名块走
///   `marquee(&mut f, …, content_x0(), col_w, …)`，**贴内容区左沿**，而那一页的
///   元信息行左端也与它同一条竖线（`bar()` 的"左：已播 贴着内容区的左沿"）。
///   ⇒ **这一页也全部贴 `content_x0()`**，只有底部那一行页码是居中的
///   （它对应面板走带条正中那个 `[REPEAT]` —— **状态行居中，说明行左对齐**）。
///
/// ⚠️ **抬头每一页都重画一遍**：翻页时碟片名跟着走，所以第 2 页不会变成一张
///   "不知道在讲哪张碟"的页。抬头两行 + 一道分隔线，剩下是详情。
///
/// **"空的不占行"在这里生效**（`put_line`）：一张没有歌手的盘，这一页就少一行，
/// 而详情会**往上补**一行标签 —— 也就是说**空行不会在屏上留一个洞**。
fn info_detail_frame(c: &Info<'_>, detail: &[Detail<'_>]) -> Frame {
    let mut f = Frame::panel();
    frame(&mut f);
    text_left(&mut f, c.disc, content_x0(), content_y0(), INK, prose_plan());
    // ⚠️ **参数顺序是 `(line, x, top, …)`** —— 把 x 与 top 写反不会报错，
    //   只是两行字**画在同一行上叠起来**（看着像"标题糊了"）。
    //   上一版就是这么错的：抬头两行重在一起，而下面的分隔线与详情都正常，
    //   所以第一眼像是"字体没装好"。
    text_left(
        &mut f,
        c.disc_line,
        content_x0(),
        content_y0() + line_h() as i32,
        INK,
        prose_plan(),
    );
    let top = info_head_bot() + 2;
    f.fill(content_x0(), top - 2, content_x1() - content_x0(), 1, RULE);
    let mut y = top;
    let bot = info_body_bot();
    for d in detail {
        match d {
            Detail::Prose(t) => put_line(&mut f, t, INK, &mut y, bot),
            Detail::Readout(t) => put_line(&mut f, t, RULE, &mut y, bot),
            Detail::Pair(k, v) => {
                if y + line_h() as i32 > bot {
                    continue;
                }
                // ★ **键贴左沿、值跟在键后面** —— 与键位指南那一格同一个排法
                //   （`INFO_KEY_GAP`）。不这么排的话 `year` 与 `1998` 会顶在一起，
                //   而这个间隔是"这是两样东西"的全部信号。
                //
                //   ⚠️ **不是"值贴右沿"**：那一版里键在左边**居中**，
                //   于是 `year` / `genre` / `albumartist` 三行的键各起各的位置 ——
                //   **扫不下去**，而这一页的标签项本来就是给人扫的。
                text_left(&mut f, k, content_x0(), y, RULE, prose_plan());
                let kw = measure_px(k, prose_plan());
                // ★ **间隔是"至少一个汉字，而且不小于键宽的四分之一"**。
                //   固定 12 不够：`albumartist` 那种长键自己就快占满一栏，
                //   而**它的字身盒右缘到下一个字的左缘**（`measure_px` 给的是含边距的
                //   推进宽度）**只有几个像素**，于是读作 `ALBUMARTIST示例歌手`。
                //   键越长，值就该被推得越开 —— 这不是"排版讲究"，是**长键必须让位**。
                let gap = INFO_KEY_GAP.max(kw / 4);
                text_left(&mut f, v, content_x0() + kw + gap, y, RULE, prose_plan());
                y += line_h() as i32;
            }
        }
    }
    f
}

/// 详情页能放几行。**从可用高度反推**，不是常数。
///
/// `(bot - top)` 里 `top` 让出 2 行给分隔线、`bot` 让出 2 行给页脚；
/// 分母是 `line_h()` 而**不是行距** —— 这一页没有额外的行距，
/// 每一行的行盒就是字体自己的 16 行（`line_h()`）。
pub fn info_detail_rows() -> usize {
    ((info_body_bot() - info_head_bot() - 4) / line_h() as i32).max(1) as usize
}

/// 详情页那一帧，**按 `page` 取那一段**。`page` 是"详情页里的第几页"（从 0 起）。
pub fn info_page_frame(c: &Info<'_>, page: usize) -> Frame {
    let per = info_detail_rows();
    let detail: Vec<Detail<'_>> = c
        .details()
        .iter()
        .skip(page * per)
        .take(per)
        .copied()
        .collect();
    let mut f = info_detail_frame(c, &detail);
    info_foot(&mut f, page + 1, info_pages(c));
    emboss(&mut f, info_foot_y(), LOGICAL_H as i32);
    f
}

/// 详情一共有几行。**给例程量版式用**（`info_check` 打印它）——
/// 屏上不需要这个数，而 `details()` 是私有的，例程也不该知道"一行"是怎么定义的。
pub fn info_detail_rows_of(c: &Info<'_>) -> usize {
    c.details().len()
}

/// 详情页有几页（**不含键位那一页**）。
///
/// ⚠️ **至少 1** —— 一张没有歌手也没有标签的盘仍然要有一页详情（它有曲名和时长），
///   而 `0` 页详情会让 `←` 在第 1 页上翻不动。
pub fn info_detail_pages(c: &Info<'_>) -> usize {
    c.details().len().div_ceil(info_detail_rows()).max(1)
}

/// 键位指南那一帧。**同一个外壳、同一个色板、同一张格网。**
pub fn info_keys_frame(c: &Info<'_>) -> Frame {
    let mut f = Frame::panel();
    frame(&mut f);
    text_left(&mut f, "键位", content_x0(), content_y0(), INK, prose_plan());
    let top = content_y0() + line_h() as i32 + 2;
    f.fill(content_x0(), top - 2, content_x1() - content_x0(), 1, RULE);
    info_keys(&mut f, c.keys, top, info_body_bot());
    info_foot(&mut f, info_pages(c), info_pages(c));
    emboss(&mut f, info_foot_y(), LOGICAL_H as i32);
    f
}

/// Draw `line` centred **in the content area**, at `top` (its em box top).
///
/// **不是 [`text_at`]**（那个横跨整幅面板）—— 这一页的边框是 2px 的墨，而字要是按整幅居中，
/// 它与边框之间只有 2px，一行稍长的读数就会贴上那道框。居中要按**内容区**算，
/// 与歌名块（`marquee(&mut f, ..., content_x0(), col_w, ...)`）同一列。
///
/// 薄封装 [`text_centred_between`]：三个入参写死成本页的内容区，**调用处因此不必重复它们**——
/// 而重复了就有"这一处忘了写 content_x0"的可能，而那种错看着只是"这一行没对齐"。
pub fn text_centred(frame: &mut Frame, line: &str, top: i32, shade: u8, plan: text::Plan) {
    text_centred_between(frame, line, content_x0(), content_x1(), top, shade, plan)
}

/// 印一行，**装不下就不印**。
///
/// ⚠️ **"不画"而不是"画一半"**：`Frame::put` 会把越界的像素**裁掉**，所以一个装不下的行
///   会印出上半个字——那比没有这一行难看得多，而且看不出是排版坏了还是内容有问题。
fn put_line(f: &mut Frame, text: &str, shade: u8, y: &mut i32, bot: i32) {
    if text.trim().is_empty() || *y + line_h() as i32 > bot {
        return;
    }
    text_left(f, text, content_x0(), *y, shade, prose_plan());
    *y += line_h() as i32;
}

/// 底部那一行：`1/2   L / R 翻页`。**页码与翻页键写在一起**，因为它们是同一件事。
///
/// ★★ **写 `L / R` 而不是 `← →`**：实测 `←` `→` **在两张面里都没有字形**——
///
/// ```text
/// 页脚 "1/2  ←→翻页" = 95.0 逻辑像素；丢的字 = ['←', '→', '翻', '页']
/// ```
///
/// 而键位指南里的 `← → ↑ ↓` **画得出来**，因为那一格是**键走 `tag_plan`、说明走
/// `prose_plan`**，而箭头落在 `prose_plan` 那半——**同一批符号在汉字面有码位、
/// 在 Press Start 2P 没有**。页脚整行是 `tag_plan`，所以它一个箭头也画不出来。
///
/// ⇒ **这一行只能用拉丁字母**。面板自己的走带条也是这么写的（`[REPEAT]`、`0:37`，
/// 一个箭头都没有）——这不是巧合，是**同一张面上哪些符号存在**的问题。
fn info_foot(f: &mut Frame, page: usize, pages: usize) {
    text_centred(
        f,
        &format!("{page}/{pages}   L / R 翻页"),
        info_foot_y(),
        RULE,
        prose_plan(),
    );
}

/// 键位指南：**两栏，`(键, 说明)` 各自成对**。
///
/// **14 条排 2 栏 = 7 行**，7×16 = 112，而正文可用高度是 `info_body_bot() - top`
/// = `142 - 24` = 118 ⇒ **刚好装下**。这就是为什么这一页的两栏必须是两栏。
///
/// ⚠️ **栏宽是算出来的，键与说明的字号各走各的 plan**：键是拉丁 → [`tag_plan`]（8px），
///   说明是汉字 → [`prose_plan`]（16px）。**两栏的中线在内容区的正中**，而每栏里键靠左、
///   说明跟在 `INFO_KEY_GAP` 之后 —— 与面板底部那排键标签同一套语言（`plate::hint_face`
///   的 `HINT_GAP`）。
///
/// ⚠️ **中英文两套文案的宽度不同，英文更长** ⇒ `lang::DECK_KEY_GUIDE` 里的说明一律写得很短
///   （两三个字）。**栏宽是固定的，所以"说明多长"是写文案时的约束，不是排版时能解决的**——
///   超宽了会被 `Frame::put` 裁掉句尾，而裁掉的恰好是句尾。
const INFO_KEY_COLS: usize = 2;
/// 键与说明之间那一格的空白。**12 行 —— 约一个汉字宽**。
///
/// ⚠️ **从 6 改成 12 的原因**：这一页现在全是 16px，而**6px 在 16px 的字旁边
///   读作"顶在一起"**（第一版 `YEAR1998` 与 `ALBUMARTIS` 就是这么糊在一起的 ——
///   键是拉丁、间隔只有 6，而拉丁的字身盒本身就窄，视觉上等于没有间隔）。
///   12 是**一个汉字的宽度**，所以"这是两样东西"这件事不靠数字对齐就已经成立。
///
/// **代价**：`SELECT 本页` 从 117 变成 123，而**两栏只有 114** —— 所以两栏放不下了，
/// 见 `info_keys` 的注释。
const INFO_KEY_GAP: i32 = 12;

/// 键与说明之间那一格的空白。**公有，因为例程要量它**——
/// 它是"这一页能不能排进两栏"这个判据里的那个数，而判据在例程那边。
pub fn info_key_gap() -> i32 {
    INFO_KEY_GAP
}

fn info_keys(f: &mut Frame, keys: &[(&str, &str)], top: i32, bot: i32) {
    let per_col = keys.len().div_ceil(INFO_KEY_COLS);
    let col_w = (content_x1() - content_x0()) / INFO_KEY_COLS as i32;
    for (i, (key, what)) in keys.iter().enumerate() {
        let y = top + (i % per_col) as i32 * line_h() as i32;
        if y + line_h() as i32 > bot {
            // 装不下的**不画**，而不是画到一半被裁。
            break;
        }
        let x0 = content_x0() + (i / per_col) as i32 * col_w;
        text_left(f, key, x0, y, INK, tag_plan());
        let kw = measure_px(key, tag_plan());
        text_left(f, what, x0 + kw + INFO_KEY_GAP, y, INK, prose_plan());
    }
}

// ── The grid, and the upscale ───────────────────────────────────────────────────────────────

/// The last logical row the grid may touch — **the whole panel, now.**
///
/// ★ 用户第五轮："操作区同样也是有网格的（屏幕为整体，跟游戏一样的）"。所以切点从 `strip_y()`
///   挪到屏底：**格网是这块屏的物理属性，不是某一层的装饰**，游戏画面里它压在一切之上。
///
/// ⚠️ 代价要认：反白状态条是一块墨，被网过之后会显出格点阵（早先一版正是因此把切点收在
///   状态条上沿的）。现在按用户的判断走 —— 他要的就是"整屏同一套渲染"。
fn grid_bottom() -> i32 {
    LOGICAL_H as i32
}

/// The frame as RGBA at the panel's own size, ready for `Draw::Tex`.
///
/// **Nearest-neighbour by copying each logical pixel into an SCALE×SCALE block**, which is what
/// "whole number" has to mean if the scale is to cost nothing. An interpolating scale would blend
/// the four shades into colours the ramp does not have, and the grid would smear.
///
/// `ramp` is four colours, lightest first — the order every palette in this project is written in.
///
/// # The grid is the game's own grid
///
/// **Transcribed from `GAME_FRAG` in `slot-gfx/src/shaders.rs`**, which is where the lattice
/// is actually drawn — the `u_grid_scan < 0.5` arm of the grid block:
///
/// ```text
/// vec2 sub = fract(v_uv * u_src);
/// hit = sub.y >= 0.66667 || sub.x >= 0.66667;
/// if (hit) { c = mix(c, u_grid, u_grid_mix); }
/// ```
///
/// (A scanline is the other arm of that branch and adds `u_grid_step`, which spaces the
/// wires out; the lattice spans every row and has no step.)
///
/// with `u_grid` the palette's own lightest shade (`App::display_grid` passes `gb_shades()[0]`)
/// and `u_grid_mix` = `DisplayFilter::GB_GRID_MIX` = **0.5**. The comment there says the value
/// was tried at 35% and set back to 50% on the device.
///
/// **So the gap is half-and-half, not a colour of its own**, and it follows the palette because
/// both halves come out of `ramp`.
pub fn render(f: &Frame, ramp: [[u8; 3]; 4], grid: bool) -> Vec<u8> {
    render_scaled(f, ramp, grid, SCALE)
}

/// The same upscale at an explicit ratio — **the whole-number factor is the only parameter**
/// (720 = 240×3 = 360×2 = 180×4), so the panel's grain can be compared without touching the
/// drawing code. See `examples/lcd_scale_compare.rs`.
///
/// ⚠️ **This is the ratio, not the base.** The logical frame's size is a separate decision: a
/// finer grain means a *bigger* base and a *smaller* ratio (360×240 at ×2), and the layout is
/// written in logical pixels, so changing the base is a layout rework rather than a constant.
pub fn render_scaled(f: &Frame, ramp: [[u8; 3]; 4], grid: bool, scale: u32) -> Vec<u8> {
    let (w, h) = (f.w * scale, f.h * scale);
    let mut out = vec![0u8; (w * h * 4) as usize];
    let paper = ramp[0];
    let cut_row = (grid_bottom().max(0) as u32) * scale;
    for y in 0..h {
        let sy = y / scale;
        for x in 0..w {
            let sx = x / scale;
            let cell = f.px[(sy * f.w + sx) as usize];
            let mut c = ramp.get(cell as usize).copied().unwrap_or(ramp[3]);
            if grid && y < cut_row {
                // The lattice: the right column and the bottom row of every cell, `>= 2/3` in
                // each axis, so it is exactly the same five of nine sub-pixels the shader takes.
                let sub_x = x % scale;
                let sub_y = y % scale;
                let cut = (2 * scale) as f32 / 3.0;
                if sub_y as f32 >= cut || sub_x as f32 >= cut {
                    // ★ 2026-10-06 六六：「网格的渲染透明度调到现在的一半」。
                    //   原来是"与纸各半"（1/2），现在**往纸走四分之一**（1/4）——
                    //   格线仍然围得出格子，但不再把满墨的字切成麻点，反白状态条上尤其明显。
                    const TO_PAPER: u32 = 1; // 分子；分母 4
                    c = [
                        (((c[0] as u32) * (4 - TO_PAPER) + paper[0] as u32 * TO_PAPER) / 4) as u8,
                        (((c[1] as u32) * (4 - TO_PAPER) + paper[1] as u32 * TO_PAPER) / 4) as u8,
                        (((c[2] as u32) * (4 - TO_PAPER) + paper[2] as u32 * TO_PAPER) / 4) as u8,
                    ];
                }
            }
            let d = ((y * w + x) * 4) as usize;
            out[d] = c[0];
            out[d + 1] = c[1];
            out[d + 2] = c[2];
            out[d + 3] = 255;
        }
    }
    out
}

/// Compose and render in one call — the path the drawer takes.
///
/// `grid` is whether the pixel grid goes on. **A panel without it reads as a clean vector
/// display**, which is a legitimate thing to want and is not what a four-shade panel is.
pub fn panel(c: &Content<'_>, ramp: [[u8; 3]; 4], grid: bool) -> Vec<u8> {
    render(&compose(c), ramp, grid)
}

/// Compose and render at an explicit ratio; see [`render_scaled`].
pub fn panel_scaled(c: &Content<'_>, ramp: [[u8; 3]; 4], grid: bool, scale: u32) -> Vec<u8> {
    render_scaled(&compose(c), ramp, grid, scale)
}
