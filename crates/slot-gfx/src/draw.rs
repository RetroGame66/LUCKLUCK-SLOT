use crate::quad::Quad;
use crate::shaders::{SPRITE_FRAG, SPRITE_VERT};
use crate::surface::{GfxError, OUT_H, OUT_W};

/// Handle to a texture the compositor owns. UI code never sees a GL name.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TexId(usize);

#[cfg(feature = "test-support")]
impl TexId {
    /// A handle by index, for callers that need to name a texture without a live context —
    /// tests and tools. Gated behind `test-support`, which no consumer enables outside its
    /// own `[dev-dependencies]`, so a forged handle cannot reach the device build and alias
    /// whatever real texture happens to sit at that index.
    pub const fn from_raw(id: usize) -> Self {
        TexId(id)
    }
}

/// Chrome geometry, in offscreen pixels with the origin at the top left.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Draw {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        colour: [f32; 4],
    },
    Tex {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
    },
    /// `Tex` turned about its own centre by `turn` radians, clockwise on the panel since y
    /// runs down. For the few faces that tilt: the lid over the core picker and its chip in
    /// flight. A variant rather than a field on `Tex`, so no existing draw had to change.
    Turned {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
        turn: f32,
    },
    /// `Tex` turned **about its own vertical axis** by `yaw` radians: the quad becomes a
    /// trapezoid, one side coming toward the viewer and the other going away.
    ///
    /// ★★ 2026-10-25（六六）：「摆动看起来还是有点奇怪，能朝着玩家晃吗（就是轴是从卡带纵向
    /// 穿过的，卡带左右两侧交替往玩家方向摆）」。原来那档是 `Turned` —— 在**屏幕平面**里转，
    /// 所以卡带看起来像被拧了一下；要的是**绕竖轴偏转**，卡像立在桌上被人左右转过来。
    ///
    /// 两者的区别是可量化的：平面内转出来的还是矩形（只是斜了），绕竖轴转出来的是**梯形**
    /// （近边高、远边矮）。梯形只能来自四个角各自的深度，所以这一档走 `SPRITE_VERT` 的
    /// `u_yaw` 分支，由 `gl_Position` 之前那一步除法给出。
    ///
    /// `yaw > 0` ⇒ **右边朝观察者**（右缘更高），`yaw < 0` ⇒ 左边。底边钉在原处不动。
    Yaw {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
        yaw: f32,
    },
    /// A quad sampling a **sub-rect** of its texture, for an atlas: `uv` is
    /// `(x0, y0, x1, y1)` in the texture's own 0..1 space. The badge glyphs live in one
    /// emoji atlas, and a texture per glyph would be a texture per emoji.
    Glyph {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        uv: [f32; 4],
        alpha: f32,
    },
    /// A film over a quad — a sticker's lamination, or a shell's edge lit along its outline.
    ///
    /// The film is **light and dye, not a layer**: it outputs only its own coverage and lets
    /// the framebuffer blend it over what is already drawn, so it takes the shape of
    /// whatever is under it automatically — a rounded sticker's corners, a cart dimmed at
    /// the side of the row — without this item knowing anything about either. That is why
    /// it carries `tex`: the same texture the quad under it was drawn from, so the shader
    /// can read what is there (how dark the paper is, where the shell's edge runs) and
    /// decide what the film is worth over it.
    ///
    /// `uv` is the sub-rect of `tex` this quad covers — the sticker's own rectangle inside
    /// the cart's face — because the film's pattern has to line up with the paper rather
    /// than with the cart around it.
    ///
    /// `foil` is `(strength, phase, highlight, effect)`: how hard the film is laid, where
    /// its animation stands (fixed per game, sweeping only under the caret), how much of
    /// the specular sweep to add, and which of the five looks this is.
    Foil {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        uv: [f32; 4],
        alpha: f32,
        foil: [f32; 4],
    },
    /// Where the game layer goes. It carries no geometry: the pass owns its own rect, since
    /// the power on squeezes it. A marker rather than a pass of its own before the list,
    /// because the panel is the front surface of the device and has to be able to come up
    /// over a cart that is already in the slot.
    Game,
    /// Where the screen glow goes, if the overlay up asks for one: the picture's own edge
    /// carried out past its window, over the bezel. A marker of its own rather than something
    /// the `Game` marker carries, because **the two are not at the same place in the list**:
    /// the glow belongs over the overlay art, and the picture belongs under it. See
    /// `Compositor::draw_reflection` for why that is the order light actually takes.
    Glow,
    /// A 240x160 still on the panel, through the game pass. Geometry comes from the pass for
    /// the same reason `Game`'s does: it is the same screen, showing a picture instead of a
    /// live frame, so it wears the same mask at the same scale.
    Shot { tex: TexId },
}

/// One program for both variants: a rect is a textured quad sampling a white texel, so a
/// draw list never has to change program mid list.
pub struct Sprites {
    prog: gl::types::GLuint,
    white: gl::types::GLuint,
    /// The film's ramp: 256×1 RGBA, bound to texture unit 1 for the whole list. Built here
    /// rather than uploaded because it is arithmetic — seven stops, each held flat for the
    /// first half of its band and graded across the second.
    ramp: gl::types::GLuint,
    textures: Vec<gl::types::GLuint>,
    u_rect: gl::types::GLint,
    u_colour: gl::types::GLint,
    u_turn: gl::types::GLint,
    u_yaw: gl::types::GLint,
    u_uv: gl::types::GLint,
    u_foil: gl::types::GLint,
}

/// 竖轴偏转的透视强度：**卡带半宽 / 观察距离**（`Draw::Yaw`）。
///
/// 一个数决定"转过去看起来像多厚的卡"：
/// * `0` ⇒ 只压窄、不张开，等于没有透视（又回到矩形）；
/// * 太大 ⇒ 近边过分放大，像广角贴脸。
///
/// `0.36` 是在 **yaw 峰值 9°** 下让近边比远边高 **12%** 得到的：
/// `(1 + k·sin9°)/(1 − k·sin9°) = 1.12` ⇒ `k = 0.36`。
/// 12% 在 180px 高的卡上是一眼能看出"它转过来了"、又不至于像换了个透视镜头的量。
///
/// ⚠️ 离机渲染器（`tests/render_look.rs`）反解时必须用**同一个数**，所以它是 `pub`
///    且从 `lib.rs` 导出 —— 不要在下游再写一个字面量。
pub const YAW_DEPTH: f32 = 0.36;


/// The film's ramp, as the bytes of a 256×1 RGBA texture.
///
/// Flat-then-grade is the whole point: a hologram's colour changes in steps — diffraction
/// orders, not a wash — and a smoothly graded rainbow reads as a gradient somebody painted
/// on. Seven stops so the bands are wide enough to read at the size of a sticker, wrapping
/// back to the first so the ramp has no seam where it ends.
pub fn foil_ramp() -> Vec<u8> {
    const STOPS: [[u8; 3]; 7] = [
        [0x8e, 0x2f, 0x6e], // magenta
        [0x2e, 0x4b, 0x9e], // blue
        [0x1f, 0x7a, 0x8c], // teal
        [0x2e, 0x8b, 0x4a], // green
        [0xb8, 0xa3, 0x2e], // gold
        [0xc0, 0x5a, 0x2a], // orange
        [0x8e, 0x2f, 0x6e], // and back, so it wraps
    ];
    let mut out = Vec::with_capacity(256 * 4);
    for i in 0..256 {
        let t = i as f32 / 255.0 * (STOPS.len() - 1) as f32;
        let seg = (t as usize).min(STOPS.len() - 2);
        // Held flat for the band's first half, graded across its second: where one stop
        // meets the next the value is the next stop's own, so the joins are continuous.
        let f = {
            let f = t - seg as f32;
            if f < 0.5 {
                0.0
            } else {
                (f - 0.5) * 2.0
            }
        };
        let (a, b) = (STOPS[seg], STOPS[seg + 1]);
        out.extend_from_slice(&[
            (a[0] as f32 + (b[0] as f32 - a[0] as f32) * f) as u8,
            (a[1] as f32 + (b[1] as f32 - a[1] as f32) * f) as u8,
            (a[2] as f32 + (b[2] as f32 - a[2] as f32) * f) as u8,
            255,
        ]);
    }
    out
}

/// The badge's cell in the atlas, as `(x0, y0, x1, y1)` in the texture's own 0..1 space.
/// `None` when the index is past the atlas — a table written against a bigger atlas than the
/// card carries names glyphs nobody can draw, and the honest answer is no mark.
///
/// Lives here rather than in the table that names the glyph, because it is arithmetic about
/// a texture and not about a rule — and so it can be tested without a GL context.
pub fn badge_uv(glyph: u32, cols: u32, w: u32, h: u32) -> Option<[f32; 4]> {
    if cols == 0 || w == 0 || h == 0 {
        return None;
    }
    let cell = w as f32 / cols as f32;
    if cell <= 0.0 {
        return None;
    }
    let (col, row) = (glyph % cols, glyph / cols);
    if (row + 1) as f32 * cell > h as f32 {
        return None;
    }
    Some([
        col as f32 * cell / w as f32,
        row as f32 * cell / h as f32,
        (col + 1) as f32 * cell / w as f32,
        (row + 1) as f32 * cell / h as f32,
    ])
}

impl Sprites {
    pub fn new() -> Result<Self, GfxError> {
        let prog = crate::shaders::program(SPRITE_VERT, SPRITE_FRAG)?;
        let white = crate::gl::texture(
            1,
            1,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            Some(&[255u8; 4]),
        );
        let ramp = crate::gl::texture(
            256,
            1,
            gl::LINEAR,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            Some(&foil_ramp()),
        );
        let (u_rect, u_colour, u_turn, u_yaw, u_uv, u_foil);
        unsafe {
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_ramp"), 1);
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_target"),
                OUT_W as f32,
                OUT_H as f32,
            );
            // The perspective strength is a property of the scene, not of any one draw, so it
            // is set here once. `Draw::Yaw` only has to say how far round the card is.
            gl::Uniform1f(
                crate::gl::uniform_location(prog, "u_yaw_depth"),
                YAW_DEPTH,
            );
            u_rect = crate::gl::uniform_location(prog, "u_rect");
            u_colour = crate::gl::uniform_location(prog, "u_colour");
            u_turn = crate::gl::uniform_location(prog, "u_turn");
            u_yaw = crate::gl::uniform_location(prog, "u_yaw");
            u_uv = crate::gl::uniform_location(prog, "u_uv");
            u_foil = crate::gl::uniform_location(prog, "u_foil");
        }
        Ok(Sprites {
            prog,
            white,
            ramp,
            textures: Vec::new(),
            u_rect,
            u_colour,
            u_turn,
            u_yaw,
            u_uv,
            u_foil,
        })
    }

    pub fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.push(w, h, rgba, gl::LINEAR)
    }

    /// For a texture drawn at a whole number magnification, where a linear tap lands between
    /// texels and blurs the pixel grid the source is defined on. The switcher's screenshot is
    /// the only one: everything else is drawn at its own size or smaller.
    pub fn create_texture_nearest(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.push(w, h, rgba, gl::NEAREST)
    }

    fn push(&mut self, w: u32, h: u32, rgba: &[u8], filter: gl::types::GLenum) -> TexId {
        let tex = crate::gl::texture(w, h, filter, gl::CLAMP_TO_EDGE, gl::RGBA, Some(rgba));
        self.textures.push(tex);
        TexId(self.textures.len() - 1)
    }

    /// Replaces a texture's contents in place. The polaroid switcher redraws the same ten
    /// cards every time it opens, and creating a fresh set each time would grow the pool
    /// for the life of the session.
    pub fn update_texture(&mut self, id: TexId, w: u32, h: u32, rgba: &[u8]) {
        let Some(tex) = self.textures.get(id.0) else {
            return;
        };
        if rgba.len() < (w * h * 4) as usize {
            return;
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, *tex);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                crate::gl::internal_format(gl::RGBA),
                w as i32,
                h as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba.as_ptr() as *const std::ffi::c_void,
            );
        }
    }

    /// Frees one texture and leaves a hole. The slot becomes 0, which every draw and every
    /// `source` reads as "gone" rather than as a handle to something that no longer exists.
    ///
    /// Nothing reuses the index: a released face is rebuilt into a fresh slot when its cart
    /// comes back into range, so the pool grows with the number of *rebuilds* rather than with
    /// the number of carts. On a nine-hundred-game card that is the difference between sixteen
    /// megabytes of shelf and half a gigabyte of it.
    pub fn release_texture(&mut self, id: TexId) {
        let Some(slot) = self.textures.get_mut(id.0) else {
            return;
        };
        if *slot == 0 {
            return;
        }
        unsafe {
            gl::DeleteTextures(1, slot);
        }
        *slot = 0;
    }

    /// For the passes that sample a sprite texture without drawing it as a sprite.
    pub fn source(&self, id: TexId) -> Option<gl::types::GLuint> {
        self.textures.get(id.0).copied().filter(|t| *t != 0)
    }

    pub fn draw(&self, items: &[Draw], quad: &Quad) {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::UseProgram(self.prog);
            gl::ActiveTexture(gl::TEXTURE0);
            // The film's ramp, on unit 1 for the whole list. Bound once here rather than per
            // item because no item changes it — and the two film uniforms below are set for
            // **every** item, defaults included, which is what stops one item's film from
            // leaking into the next. (The first version of this set them only on the film
            // items, and a missed set turned the whole panel into a hologram.)
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, self.ramp);
            gl::ActiveTexture(gl::TEXTURE0);
        }
        for item in items {
            let (x, y, w, h, tex, colour, turn, uv, foil, yaw) = match *item {
                Draw::Rect { x, y, w, h, colour } => (
                    x,
                    y,
                    w,
                    h,
                    self.white,
                    colour,
                    0.0,
                    [0.0, 0.0, 1.0, 1.0],
                    [0.0; 4],
                    0.0,
                ),
                Draw::Tex {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                } => match self.textures.get(tex.0) {
                    Some(&t) if t != 0 => (
                        x,
                        y,
                        w,
                        h,
                        t,
                        [1.0, 1.0, 1.0, alpha],
                        0.0,
                        [0.0, 0.0, 1.0, 1.0],
                        [0.0; 4],
                        0.0,
                    ),
                    // Never drawn, or released since the list was built. A gap in a row of
                    // carts is filled by the shelf's own placeholder, not by this.
                    _ => continue,
                },
                Draw::Turned {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                    turn,
                } => match self.textures.get(tex.0) {
                    Some(&t) if t != 0 => (
                        x,
                        y,
                        w,
                        h,
                        t,
                        [1.0, 1.0, 1.0, alpha],
                        turn,
                        [0.0, 0.0, 1.0, 1.0],
                        [0.0; 4],
                        0.0,
                    ),
                    _ => continue,
                },
                // 绕**竖轴**偏转。横向的 `cos` 走 `u_turn`（那个 uniform 在 sin = 0 时的
                // 含义正好就是"只压 x"），深度走 `u_yaw` 由顶点着色器张成梯形。
                Draw::Yaw {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                    yaw,
                } => match self.textures.get(tex.0) {
                    Some(&t) if t != 0 => (
                        x,
                        y,
                        w,
                        h,
                        t,
                        [1.0, 1.0, 1.0, alpha],
                        0.0,
                        [0.0, 0.0, 1.0, 1.0],
                        [0.0; 4],
                        yaw,
                    ),
                    _ => continue,
                },
                Draw::Glyph {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    uv,
                    alpha,
                } => match self.textures.get(tex.0) {
                    Some(&t) if t != 0 => (
                        x,
                        y,
                        w,
                        h,
                        t,
                        [1.0, 1.0, 1.0, alpha],
                        0.0,
                        uv,
                        [0.0; 4],
                        0.0,
                    ),
                    _ => continue,
                },
                Draw::Foil {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    uv,
                    alpha,
                    foil,
                } => match self.textures.get(tex.0) {
                    Some(&t) if t != 0 => (
                        x,
                        y,
                        w,
                        h,
                        t,
                        [1.0, 1.0, 1.0, alpha],
                        0.0,
                        uv,
                        foil,
                        0.0,
                    ),
                    // No face to lay the film over: the mark is dropped rather than drawn
                    // over nothing, which would be a coloured rectangle with no cart under it.
                    _ => continue,
                },
                // The compositor splits the list on these and draws the game pass, the still
                // and the glow itself.
                Draw::Game | Draw::Glow | Draw::Shot { .. } => continue,
            };
            // Exactly (1, 0) for anything unturned rather than cos and sin of zero, so the
            // shader's correction term is zero by construction and not by rounding.
            //
            // 转过竖轴的那张卡借用 `u_turn` 只做**横向压窄**（sin 取 0 ⇒ 不转），深度另走
            // `u_yaw`。把 cos 放在这里算，是为了"角度只有一处知道"。
            let (cos, sin) = if yaw != 0.0 {
                (yaw.cos(), 0.0)
            } else if turn == 0.0 {
                (1.0, 0.0)
            } else {
                (turn.cos(), turn.sin())
            };
            unsafe {
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::Uniform4f(self.u_rect, x, y, w, h);
                gl::Uniform2f(self.u_turn, cos, sin);
                gl::Uniform1f(self.u_yaw, yaw);
                gl::Uniform4f(self.u_colour, colour[0], colour[1], colour[2], colour[3]);
                gl::Uniform4f(self.u_uv, uv[0], uv[1], uv[2], uv[3]);
                gl::Uniform4f(self.u_foil, foil[0], foil[1], foil[2], foil[3]);
            }
            quad.draw();
        }
        unsafe { gl::Disable(gl::BLEND) };
    }
}

impl Drop for Sprites {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(self.textures.len() as i32, self.textures.as_ptr());
            gl::DeleteTextures(1, &self.white);
            gl::DeleteTextures(1, &self.ramp);
            gl::DeleteProgram(self.prog);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{badge_uv, foil_ramp};

    /// The ramp is the bytes of a 256×1 RGBA texture: it is uploaded by size, not by length,
    /// so a short one is four pixels of a ramp followed by whatever the driver left there.
    #[test]
    fn the_ramp_is_one_texel_wide_and_fully_opaque() {
        let ramp = foil_ramp();
        assert_eq!(ramp.len(), 256 * 4);
        for px in ramp.chunks_exact(4) {
            assert_eq!(px[3], 255);
        }
    }

    /// It wraps: the last stop is the first, so a film that walks off one end of the ramp
    /// comes back the other side rather than showing a seam.
    #[test]
    fn the_ramp_wraps() {
        let ramp = foil_ramp();
        let first = &ramp[..3];
        let last = &ramp[ramp.len() - 4..ramp.len() - 1];
        assert_eq!(first, last);
    }

    /// It is not a flat wash: seven stops, and the point of the stops is that the colour
    /// changes in bands rather than in a gradient.
    #[test]
    fn the_ramp_actually_moves() {
        let ramp = foil_ramp();
        let at = |i: usize| [ramp[i * 4], ramp[i * 4 + 1], ramp[i * 4 + 2]];
        let spread = (0..256)
            .flat_map(|i| {
                let (a, b) = (at(i), at((i + 8) % 256));
                [(a[0] as i32 - b[0] as i32).abs(), (a[1] as i32 - b[1] as i32).abs(), (a[2] as i32 - b[2] as i32).abs()]
            })
            .max()
            .unwrap_or(0);
        assert!(spread > 40, "the ramp barely changes: {spread}");
    }

    /// Cell 3 of a 16-wide grid is the third column of the first row, in the atlas' own
    /// 0..1 space — the one thing the toolbox's baked grid and this have to agree on.
    #[test]
    fn a_cell_is_its_own_rectangle() {
        let uv = badge_uv(3, 16, 1152, 1152).expect("inside the atlas");
        let cell = 1.0 / 16.0;
        assert!((uv[0] - 3.0 * cell).abs() < 1e-6, "{}", uv[0]);
        assert!((uv[2] - 4.0 * cell).abs() < 1e-6, "{}", uv[2]);
        assert!((uv[1] - 0.0).abs() < 1e-6);
        assert!((uv[3] - cell).abs() < 1e-6);
        // The next row down: 16 is the first cell of row 1.
        let uv = badge_uv(16, 16, 1152, 1152).expect("inside the atlas");
        assert!((uv[1] - cell).abs() < 1e-6, "{}", uv[1]);
    }

    /// Past the end of the atlas there is no cell: a table naming glyph 900 against a
    /// one-row atlas asks for nothing rather than sampling outside the texture.
    #[test]
    fn a_cell_past_the_atlas_is_none() {
        assert_eq!(badge_uv(900, 16, 1152, 72), None);
        assert_eq!(badge_uv(0, 0, 1152, 1152), None);
        assert_eq!(badge_uv(0, 16, 0, 1152), None);
    }
}
