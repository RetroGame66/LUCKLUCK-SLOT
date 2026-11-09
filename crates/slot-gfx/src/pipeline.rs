use crate::lcd3x::mask_texture_rgba8;
use crate::power::{screen_brightness, screen_rect_in};
use crate::quad::Quad;
use crate::shaders::{GAME_FRAG, RECT_VERT};
use crate::surface::{GfxError, OUT_H, OUT_W};
use crate::system::{
    cell_size, mask_scale, ARCADE_DRAW_H, ARCADE_DRAW_W, FC_SRC_H, FC_SRC_W, GB_SRC_H, GB_SRC_W,
    GBA_SRC_H, GBA_SRC_W, MD_SRC_H, MD_SRC_W, NEOGEO_SRC_H, NEOGEO_SRC_W, PCE_SRC_H, PCE_SRC_W,
    PS1_SRC_H, PS1_SRC_W, SFC_SRC_H, SFC_SRC_W, System,
};

/// The GBA's frame, which is what these two meant when there was one machine. Anything that
/// wants a source size should ask `System` for the one it is drawing.
pub const SRC_W: u32 = GBA_SRC_W;
pub const SRC_H: u32 = GBA_SRC_H;

/// Column-major identity for the colour-correction matrix, uploaded when correction is off.
const IDENTITY_CC: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

pub struct GamePass {
    prog: gl::types::GLuint,
    /// One texture per machine rather than one rebuilt on a switch, so that switching costs a
    /// bind and nothing at all else. Fixed-size machines are made once and never touched again
    /// after `new`; only `Arcade` is rebuilt on the fly, because its frame size varies per
    /// board and `System`'s 640x480 is only a nominal layout for the rect.
    game_gba: gl::types::GLuint,
    game_gb: gl::types::GLuint,
    game_sfc: gl::types::GLuint,
    game_fc: gl::types::GLuint,
    game_neogeo: gl::types::GLuint,
    /// Arcade's slot is also the one that actually gets resized.
    game_arcade: gl::types::GLuint,
    game_md: gl::types::GLuint,
    /// ★ 2026-10-26 00:1x：**改回单纹理**。曾经为了「前端隔行合成」拆成 a/b 两张 +
    /// 交替上传，但设备实测证明 PCSX 交来的帧既不是逐行缺一半、也不是整帧缺一半
    /// （见 `.workbuddy/memory/2026-10-25.md` H4），那套机制的前提不成立，已整段拆除。
    game_ps1: gl::types::GLuint,
    game_pce: gl::types::GLuint,
    /// The texture dimensions the last rebuild produced. For every fixed-size machine
    /// this equals `System::src_w()` / `src_h()`. For `Arcade` (see `System::stretches`)
    /// and for `Ps1` (PCSX hands over whatever vout size the game is in — see
    /// `upload`), this is the real size the last frame came in at.
    ///
    /// Kept here rather than re-queried from the GL object because the GL call is a
    /// round-trip and this is read every draw; tracking it in a field is free.
    texture_size: (u32, u32),
    /// Whose frame `upload` takes and `draw` puts up. The GBA, because it is the machine this
    /// pass was written for.
    active: System,
    mask: gl::types::GLuint,
    u_rect: gl::types::GLint,
    u_bright: gl::types::GLint,
    u_cc: gl::types::GLint,
    u_cc_bias: gl::types::GLint,
    u_cc_gamma: gl::types::GLint,
    /// The Game Boy palette lookup: 256x1 RGBA, one texel per grey the core can hand over.
    /// A texture rather than a table in the shader because the four shades are arbitrary
    /// colours, and a 256-entry upload is a kilobyte.
    pal: gl::types::GLuint,
    /// The pixel-art lookup: 1024x32 RGBA — the 32^3 colour cube flattened (x = r + 32*g,
    /// y = b), each texel the palette colour nearest that cell. Only the pixel grade reads
    /// it; a texture for the same reason as `pal`.
    pix: gl::types::GLuint,
    /// Whether the lookup above is what draws the picture, or the matrix is.
    ///
    /// The *sampler's* location is not kept: `u_pal` is bound to texture unit 2 once in `new` and
    /// never again, so the location is asked for there and dropped. It used to be a field, holding
    /// the location of `u_pal_on` by mistake — harmless only because nothing ever read it.
    u_pal_on: gl::types::GLint,
    /// Whether the pixel-art lookup draws the picture (`u_pix` in GAME_FRAG).
    u_pix_on: gl::types::GLint,
    /// The lattice drawn over every 3x3 source pixel: its colour, how much of it to mix in, and
    /// whether it is the whole 5-of-9 mesh or the bottom row alone (a scanline). A mix of `0.0` is
    /// off, which is the default; every machine draws its mesh from here now.
    u_grid: gl::types::GLint,
    u_grid_mix: gl::types::GLint,
    u_grid_scan: gl::types::GLint,
    /// Row pitch for the scanline lattice: 1 = every source row, 2 = every other. The shader
    /// currently does not read this — `set_grid_step` accepts it so a caller has a place to
    /// push the value, and a future `u_grid_step` in GAME_FRAG picks it up with no further
    /// plumbing. `uniform_location` returns -1 for a name the program does not declare, and
    /// `Uniform1f(-1, _)` is a documented no-op, so storing the location is harmless either way.
    u_grid_step: gl::types::GLint,
    /// ★ 2026-10-28：源尺寸的 uniform 位置。以前 `upload`/`set_system` 每次现查一遍，
    /// 现在三件几何量一起推（`push_geometry`），存一次省掉每帧的 `glGetUniformLocation`。
    u_src: gl::types::GLint,
    /// ★ 2026-10-28：每个源像素在面板上占多少输出像素（`rect/tex`，两轴）。格子线宽由它
    /// 反算。**几何量**，不是缩放常数 —— PS1 的 320×240 帧落在 640×480 矩形里时是 2。
    u_cell: gl::types::GLint,
    /// ★ 2026-10-28：遮罩表的重复次数。整数倍时 == `u_src`（锚源像素，与旧版逐字相同），
    /// 非整数倍时 == `rect/3`（锚面板像素，拍频归零）。见 `system::mask_scale`。
    u_mask_scale: gl::types::GLint,
    /// A compositor with nobody driving it is a screen that is on.
    power: f32,
}

impl GamePass {
    pub fn new() -> Result<Self, GfxError> {
        let prog = crate::shaders::program(RECT_VERT, GAME_FRAG)?;

        // Nine source textures, one per `System`. Fixed-size machines each allocate once here
        // and never again; `game_arcade` starts at the nominal 640x480 and is rebuilt on the
        // first real frame (see `rebuild_active`). BGRA because libretro hands over
        // XRGB8888 little-endian, whose bytes are B, G, R, X.
        let game_gba =
            crate::gl::texture(GBA_SRC_W, GBA_SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
        let game_gb =
            crate::gl::texture(GB_SRC_W, GB_SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
        let game_sfc =
            crate::gl::texture(SFC_SRC_W, SFC_SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
        let game_fc =
            crate::gl::texture(FC_SRC_W, FC_SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
        let game_neogeo = crate::gl::texture(
            NEOGEO_SRC_W,
            NEOGEO_SRC_H,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::BGRA,
            None,
        );
        let game_arcade = crate::gl::texture(
            ARCADE_DRAW_W,
            ARCADE_DRAW_H,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::BGRA,
            None,
        );
        let game_md =
            crate::gl::texture(MD_SRC_W, MD_SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
        let game_ps1 =
            crate::gl::texture(PS1_SRC_W, PS1_SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
        let game_pce =
            crate::gl::texture(PCE_SRC_W, PCE_SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);

        let mask = crate::gl::texture(
            3,
            3,
            gl::NEAREST,
            gl::REPEAT,
            gl::RGBA,
            Some(&mask_texture_rgba8()),
        );
        let pal = crate::gl::texture(
            256,
            1,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            Some(&[0u8; 1024]),
        );
        let pix = crate::gl::texture(1024, 32, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::RGBA, None);

        let (
            u_rect,
            u_bright,
            u_cc,
            u_cc_bias,
            u_cc_gamma,
            u_pal_on,
            u_pix_on,
            u_grid,
            u_grid_mix,
            u_grid_scan,
            u_grid_step,
            u_src,
            u_cell,
            u_mask_scale,
        );
        unsafe {
            // What is fixed for the life of the program: which texture unit carries what, and
            // the offscreen target every rect is placed against. Not the source size — that
            // one belongs to the machine whose picture is up, see `set_system`.
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_game"), 0);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_mask"), 1);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_pal"), 2);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_pix"), 3);
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_target"),
                OUT_W as f32,
                OUT_H as f32,
            );
            u_rect = crate::gl::uniform_location(prog, "u_rect");
            u_bright = crate::gl::uniform_location(prog, "u_bright");
            u_cc = crate::gl::uniform_location(prog, "u_cc");
            u_cc_bias = crate::gl::uniform_location(prog, "u_cc_bias");
            u_cc_gamma = crate::gl::uniform_location(prog, "u_cc_gamma");
            // 1.0 = 编码空间直乘（旧行为）。app 每帧推真正的值。
            gl::Uniform1f(u_cc_gamma, 1.0);
            // Identity until a colour correction is chosen; the compositor uploads the real one.
            gl::UniformMatrix3fv(u_cc, 1, gl::FALSE, IDENTITY_CC.as_ptr());
            // No black point by default: the saturation and backlight modes are anchored on
            // black, and only a hardware palette moves it (see `u_cc_bias` in GAME_FRAG).
            gl::Uniform3f(u_cc_bias, 0.0, 0.0, 0.0);
            u_pal_on = crate::gl::uniform_location(prog, "u_pal_on");
            u_pix_on = crate::gl::uniform_location(prog, "u_pix_on");
            gl::Uniform1f(u_pix_on, 0.0);
            u_grid = crate::gl::uniform_location(prog, "u_grid");
            u_grid_mix = crate::gl::uniform_location(prog, "u_grid_mix");
            gl::Uniform1f(u_grid_mix, 0.0);
            u_grid_scan = crate::gl::uniform_location(prog, "u_grid_scan");
            gl::Uniform1f(u_grid_scan, 0.0);
            u_grid_step = crate::gl::uniform_location(prog, "u_grid_step");
            // 1.0 = every source row; the current shader ignores this, but the location is
            // stored so `set_grid_step` has somewhere to push.
            gl::Uniform1f(u_grid_step, 1.0);
            // ★ 2026-10-28：格子线宽与遮罩锚法。位置存下来，数值由 `push_geometry` 推 ——
            // 它同时要 `u_src`，而 `u_src` 属于"当前这一帧"，见 `upload`。
            u_src = crate::gl::uniform_location(prog, "u_src");
            u_cell = crate::gl::uniform_location(prog, "u_cell");
            u_mask_scale = crate::gl::uniform_location(prog, "u_mask_scale");
            // Off until a palette arrives; the matrix path is what runs by default.
            gl::Uniform1f(u_pal_on, 0.0);
            // `u_src` / `u_cell` / `u_mask_scale` 的初值不在这里推：`set_system(Gba)` 在
            // `new` 末尾调用，而它现在走 `push_geometry`。那个调用**不是** no-op 的保险 ——
            // `active` 初值就是 `Gba`，所以 `set_system` 会早退。因此 `new` 之后必须
            // 显式推一次，见下面 `pass.set_system` 前的那一行。
        }
        let mut pass = GamePass {
            prog,
            game_gba,
            game_gb,
            game_sfc,
            game_fc,
            game_neogeo,
            game_arcade,
            game_md,
            game_ps1,
            game_pce,
            texture_size: (ARCADE_DRAW_W, ARCADE_DRAW_H),
            active: System::Gba,
            mask,
            u_rect,
            u_bright,
            u_cc,
            u_cc_bias,
            u_cc_gamma,
            pal,
            pix,
            u_pal_on,
            u_pix_on,
            u_grid,
            u_grid_mix,
            u_grid_scan,
            u_grid_step,
            u_src,
            u_cell,
            u_mask_scale,
            power: 1.0,
        };
        // ★ `active` 的初值就是 `Gba`，所以下面这句 `set_system(Gba)` 会**早退**（它是
        // "机器变了才推"）。几何量必须在构造时自己推一次，否则首帧之前 `u_cell` 是 0、
        // `u_mask_scale` 是 0 ⇒ `texture2D(u_mask, 0)` 取表首格、`1/max(0,1) = 1` 使格子
        // 全不点亮。这两者在 3× 下恰好"看不出来"（表首格 × 亮度差极小），所以别指望
        // 少推一次能被发现 —— 它就是错的。
        pass.push_geometry(System::Gba.src_w(), System::Gba.src_h());
        pass.set_system(System::Gba);
        Ok(pass)
    }

    /// 把"这一帧的几何"推给 shader：源尺寸、每源像素占多少输出像素、遮罩表重复次数。
    ///
    /// ★ 2026-10-28：三件事必须一起推，因为它们互相推导 —— `u_cell` 由画面矩形除以纹理
    /// 尺寸得来，`u_mask_scale` 又由 `u_cell` 是不是整数倍决定锚源还是锚面板。分开推就会
    /// 出现"格子按新几何画、遮罩按旧几何铺"的中间态。
    ///
    /// 画面矩形取 `screen_base()`，**不是** `screen_rect_in()`：后者带电源收拢（开机那一下
    /// 从一条线张开），拿它算 `cell` 会让开机瞬间的格宽随动画变。收拢是绘制时的事，
    /// 遮罩与格子应该按"稳定后的画面"布局 —— 收拢期间画面被压扁，格跟着压扁反而是错的。
    fn push_geometry(&self, tex_w: u32, tex_h: u32) {
        let rect = self.active.screen_base();
        let (cx, cy) = cell_size(rect, tex_w, tex_h);
        let (mx, my) = mask_scale(rect, tex_w, tex_h);
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform2f(self.u_src, tex_w as f32, tex_h as f32);
            gl::Uniform2f(self.u_cell, cx, cy);
            gl::Uniform2f(self.u_mask_scale, mx, my);
        }
    }

    /// Whose frame this pass is drawing. Called when a cart goes in or the shelf switches
    /// machines, which is to say rarely — nothing here is per frame.
    pub fn set_system(&mut self, system: System) {
        if self.active == system {
            return;
        }
        self.active = system;
        // The texture `active_texture` returns is now another one, so its size is the
        // one the new machine started with: `System`'s nominal pair, except for `Arcade`
        // which is primed to `ARCADE_DRAW_W/H` in `new` and only shrinks once a real
        // board shows up. `upload` will overwrite this the first time a frame comes in
        // at a size it does not already hold, so this is only the size `draw_source`
        // reads before that first upload — and it is the right size for a machine whose
        // frame is fixed by `System`.
        self.texture_size = if self.active == System::Arcade {
            self.texture_size // unchanged; a rebuilt arcade texture stays rebuilt
        } else {
            (self.active.src_w(), self.active.src_h())
        };
        self.push_geometry(self.texture_size.0, self.texture_size.1);
    }

    /// The texture `upload` writes into and `draw` puts up, for the machine that is active.
    pub fn active_texture(&self) -> gl::types::GLuint {
        match self.active {
            System::Gba => self.game_gba,
            System::Gb => self.game_gb,
            System::Sfc => self.game_sfc,
            System::Fc => self.game_fc,
            System::Neogeo => self.game_neogeo,
            System::Arcade => self.game_arcade,
            System::Md => self.game_md,
            System::Ps1 => self.game_ps1,
            System::Pce => self.game_pce,
        }
    }


    /// The rect, in offscreen pixels, the game picture currently occupies — what the screen
    /// reflection mirrors about, and how it knows where the letterbox is.
    pub fn source_rect(&self) -> (f32, f32, f32, f32) {
        screen_rect_in(self.active.screen_base(), self.power)
    }

    /// The size of the frame the active machine hands over, in pixels. The reflection's blur
    /// step is one over this, so a tap lands on a texel of whatever machine is up.
    ///
    /// For a fixed-size machine this is `System::src_w() × src_h()`. For `Arcade` and `Ps1`
    /// — the two machines whose real frame varies per game — it is whatever the last
    /// `upload` rebuilt the texture to. `fbo::draw_reflection` uses this to compute the
    /// blur's sample step (`1/w, 1/h` in UV units), which is the number of texels per UV
    /// step of the *actual* game texture; asking for the System nominal here would make the
    /// blur tap every fourth pixel of a 320×240 texture and the reflection would collapse
    /// to a 4×-too-coarse average.
    pub fn source_size(&self) -> (u32, u32) {
        self.texture_size
    }

    pub fn set_power(&mut self, t: f32) {
        self.power = t.clamp(0.0, 1.0);
    }

    /// Replace the table the picture is multiplied by. This is the whole of what this device
    /// has for a shader: LCD3x collapses to a 3x3 mask because 240x160 lands exactly three
    /// times in 720x480, and it is sampled once per source pixel. Handing the card its own
    /// table is what lets the panel look like something other than the one table that ships.
    ///
    /// Three by three and nothing else. The mask tiles once per source pixel, so a table of
    /// another shape would be sampled at three points out of its own grid and alias into
    /// noise instead of resolving into a pattern.
    pub fn set_mask(&self, rgba: &[u8]) {
        if rgba.len() < (3 * 3 * 4) as usize {
            return;
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.mask);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                3,
                3,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba.as_ptr() as *const std::ffi::c_void,
            );
        }
    }

    /// Replace the colour-correction matrix and its black point. Row-major in, uploaded
    /// column-major to match GLSL's `mat3 * vec3`. Identity with no black point leaves the
    /// picture as the mask alone would leave it; a hardware palette is a slope from its white
    /// point plus an offset to its black one, which is the only way to land four shades on the
    /// four colours the cartridge was made for.
    pub fn set_color_correction(&self, m: &[[f32; 3]; 3], bias: &[f32; 3]) {
        let col = [
            m[0][0], m[1][0], m[2][0], m[0][1], m[1][1], m[2][1], m[0][2], m[1][2], m[2][2],
        ];
        unsafe {
            gl::UseProgram(self.prog);
            gl::UniformMatrix3fv(self.u_cc, 1, gl::FALSE, col.as_ptr());
            gl::Uniform3f(self.u_cc_bias, bias[0], bias[1], bias[2]);
        }
    }

    /// Hand over the Game Boy's palette, or take it away. `None` restores the matrix path.
    ///
    /// The table is built on the CPU (`app::DisplayFilter::gb_lut`) with the four shades already
    /// interpolated between, so the shader is one nearest-neighbour fetch and lands each shade
    /// exactly on its own colour — which a 3x3 multiply cannot do for a palette whose middles
    /// sit off the line between its ends.
    pub fn set_palette(&self, lut: Option<&[u8; 1024]>) {
        unsafe {
            gl::UseProgram(self.prog);
            match lut {
                Some(l) => {
                    gl::BindTexture(gl::TEXTURE_2D, self.pal);
                    gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
                    gl::TexSubImage2D(
                        gl::TEXTURE_2D,
                        0,
                        0,
                        0,
                        256,
                        1,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        l.as_ptr() as *const std::ffi::c_void,
                    );
                    gl::Uniform1f(self.u_pal_on, 1.0);
                }
                None => gl::Uniform1f(self.u_pal_on, 0.0),
            }
        }
    }

    /// Hand over the pixel-art lookup, or take it away. `None` restores the matrix path.
    ///
    /// `lut` is 1024x32 RGBA (131072 bytes): the 32^3 colour cube flattened, each texel the
    /// palette colour nearest that cell, built on the CPU from the pixel palette
    /// (`app::DisplayFilter::pixel_lut`). One NEAREST fetch per pixel; the 5-bit snap and the
    /// 4x4 dither that go with it live in GAME_FRAG.
    pub fn set_pixel_lut(&self, lut: Option<&[u8]>) {
        unsafe {
            gl::UseProgram(self.prog);
            match lut {
                Some(l) => {
                    gl::BindTexture(gl::TEXTURE_2D, self.pix);
                    gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
                    gl::TexSubImage2D(
                        gl::TEXTURE_2D,
                        0,
                        0,
                        0,
                        1024,
                        32,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        l.as_ptr() as *const std::ffi::c_void,
                    );
                    gl::Uniform1f(self.u_pix_on, 1.0);
                }
                None => gl::Uniform1f(self.u_pix_on, 0.0),
            }
        }
    }

    /// Draw the panel lattice — or stop drawing it, with a mix of zero.
    ///
    /// `colour` is the shade the lines are mixed towards and `mix` is how much of it shows.
    /// `scanline` keeps the bottom row of every 3x3 source pixel and drops the right column: the
    /// same lit wire every third game pixel rather than a mesh. The geometry lives in the shader
    /// either way.
    pub fn set_grid(&self, colour: &[f32; 3], mix: f32, scanline: bool) {
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform3f(self.u_grid, colour[0], colour[1], colour[2]);
            gl::Uniform1f(self.u_grid_mix, mix.max(0.0));
            gl::Uniform1f(self.u_grid_scan, if scanline { 1.0 } else { 0.0 });
        }
    }

    /// 扫描线的行间距：1 = 每行，2 = 每两行。当前 `GAME_FRAG` 尚未声明 `u_grid_step`，
    /// 这里存下位置并推一次数值；`glUniform1f(-1, ..)` 在规范里就是无操作，等 shader
    /// 补上 uniform 之后这一路不用改。
    pub fn set_grid_step(&self, step: u32) {
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform1f(self.u_grid_step, step.max(1) as f32);
        }
    }

    /// The lookup texture, so the reflection pass can share it rather than keep a second copy.
    pub fn palette_texture(&self) -> gl::types::GLuint {
        self.pal
    }

    /// 色彩校正所用的 gamma。1.0 = 编码空间直乘（旧行为）；>1 = 在线性空间做校正
    /// （半彩不再发暗、单色背光更浓郁）。与矩阵一起由 `Fbo::set_cc_gamma` 推送。
    pub fn set_cc_gamma(&self, g: f32) {
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform1f(self.u_cc_gamma, g.max(0.01));
        }
    }

    /// Rebuild the active machine's texture at the given size. Called from `upload` when the
    /// incoming frame does not fit the texture as it stands today.
    ///
    /// Fixed-size machines never reach here: their frame is always `System::src_w() × src_h()`,
    /// which is the size `new` allocated them for, so the mismatch check bails out on the first
    /// comparison and `upload` writes straight into the texture it already has.
    ///
    /// The two machines whose frame genuinely varies:
    ///
    /// * `Arcade` — see `System::stretches`; a real CPS1 board hands over 384×224, and the
    ///   nominal 640×480 in `ARCADE_DRAW_W/H` is only a layout hint for the rect, not a
    ///   promise about what the core sends.
    /// * `Ps1` — PCSX_ReARmed's `vout_set_mode` picks `vout_width` and `vout_height` from
    ///   the game's real `raw_w` / `raw_h` (see `_scratch-ws/pcsxsrc/libretro.c:340`). A
    ///   15-bit low-res title sends 320×240 while `System::Ps1`'s nominal is 640×480.
    ///
    /// A texture that is bigger than the frame it holds carries undefined pixels in the
    /// uncovered region (see `gl::texture(..., None)` — `TexImage2D` with a null pointer
    /// leaves the content alone), and the shader then reads them through as if they were
    /// part of the picture. Rebuilding to the frame's own size is the only honest fix:
    /// once the texture is exactly as big as the frame, every texel is a real game pixel
    /// and `CLAMP_TO_EDGE` has nothing garbage to hand back at the seam.
    fn rebuild_active(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 || (w, h) == self.texture_size {
            return;
        }
        unsafe {
            match self.active {
                System::Arcade => {
                    gl::DeleteTextures(1, &self.game_arcade);
                    self.game_arcade =
                        crate::gl::texture(w, h, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
                }
                System::Ps1 => {
                    gl::DeleteTextures(1, &self.game_ps1);
                    self.game_ps1 =
                        crate::gl::texture(w, h, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
                }
                System::Gba | System::Gb | System::Sfc | System::Fc | System::Neogeo
                | System::Md | System::Pce => {
                    // A fixed-size machine only lands here when its `System` size changed
                    // between builds. That is a real mistake — the texture has to match
                    // what the rest of the pass believes about the machine — so fall back
                    // to the same handle the shader was pointed at. This branch should not
                    // fire in practice; if it does, the log is missing a System::src_w
                    // change somewhere.
                    gl::DeleteTextures(1, &self.active_texture());
                    let tex =
                        crate::gl::texture(w, h, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None);
                    match self.active {
                        System::Gba => self.game_gba = tex,
                        System::Gb => self.game_gb = tex,
                        System::Sfc => self.game_sfc = tex,
                        System::Fc => self.game_fc = tex,
                        System::Neogeo => self.game_neogeo = tex,
                        System::Md => self.game_md = tex,
                        System::Pce => self.game_pce = tex,
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            }
            self.texture_size = (w, h);
        }
    }

    pub fn upload(&mut self, xrgb8888: &[u8], w: u32, h: u32) {
        // Rebuild when the incoming frame does not fit the texture as it stands. For a
        // fixed-size machine the frame size is always `System::src_w() × src_h()` and so
        // always equals `texture_size`; the check bails out on the first `!=` and upload
        // goes straight to `TexSubImage2D`. For `Arcade` and `Ps1` — the two whose real
        // frame size varies per game — the check finds the mismatch and rebuilds to the
        // frame's own size (see `rebuild_active` for why a texture bigger than the frame
        // leaks undefined pixels through the shader).
        if (w, h) != self.texture_size {
            self.rebuild_active(w, h);
        }
        if xrgb8888.len() < (w * h * 4) as usize {
            return;
        }
        unsafe {
            // `u_src` is what the shader multiplies `v_uv` by to find the sub-pixel within
            // a source cell: the lattice (`fract(v_uv * u_src)`), the mask tile count, and
            // the reflection's blur step all read it. It is the *texture's* size, not the
            // System's — a 320×240 frame inside a 640×480 texture would alias the mask
            // three times per source pixel rather than once, and the lattice would fire on
            // two of the three screen positions that map to the same source pixel. Upload
            // owns it because it is the only caller that knows the real frame's dimensions.
            //
            // ★ 2026-10-28：`u_src` 现在连同 `u_cell` / `u_mask_scale` 一起由
            //   `push_geometry` 推 —— 三者互相推导，不能分开推。
            self.push_geometry(w, h);
            gl::BindTexture(gl::TEXTURE_2D, self.active_texture());
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            // ★ 2026-10-26：**直接从此切片上传，不先 `to_vec()` 拷一份。**
            //
            //   调用方（frontend）持有的 `FrameRef` 会一直活到本帧 `end_frame` 之后才 drop，
            //   所以这个切片在「纹理真正被 GPU 采样」之前始终有效。Mali 的 `TexSubImage2D`
            //   走 tiler，往往把「从客户端内存读数据」延迟到纹理首次被采样的瓦片渲染时
            //   （即本帧末 blit/swap 的那一刻）才做；先前那段 `to_vec()` 把数据放进**函数内
            //   局部 Vec**，`upload` 返回即释放，延迟拷贝读到的就是已回收、被复用的内存
            //   ⇒ 偶发整帧黑/花。PS1 帧 300KB，比 GBA 154KB 大一倍，延迟拷贝更可能还悬在
            //   绘制那一刻 ⇒ 只有 PS1 闪。直传后既修竞态、又省一次每帧 memcpy。
            //
            //   安全依据：`self.session.frame()` 经 `Frames::latest()` 把缓冲**移出**帧池
            //   （`ready.take()`），模拟线程此后无法回收或改写它；它只属于渲染线程，直到
            //   `end_frame` 之后才 drop。所以直传的源在整帧绘制期间都有效。
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                w as i32,
                h as i32,
                gl::BGRA,
                gl::UNSIGNED_BYTE,
                xrgb8888.as_ptr() as *const std::ffi::c_void,
            );
            // ★ Mali 稳健（PS1 闪屏）：同帧从客户端内存上传的纹理，强制驱动立刻把数据拷进
            // 驻留显存，避免 tiler 把这个拷贝延迟到游戏 quad 采样之后才做（那会让采样读到
            // 未就绪/回收的内存 ⇒ 偶发整片黑）。flush 让上传命令在绘制前完成。
            gl::Flush();
        }
    }

    pub fn draw(&self, quad: &Quad) {
        self.draw_source(self.active_texture(), quad);
    }

    /// The same pass over a still. A saved shot is a picture of this panel at exactly the
    /// scale the mask is built for, so it is filtered at draw time rather than blitted flat
    /// beside a game that is filtered.
    pub fn draw_still(&self, tex: gl::types::GLuint, quad: &Quad) {
        self.draw_source(tex, quad);
    }

    fn draw_source(&self, tex: gl::types::GLuint, quad: &Quad) {
        // The strike and bloom is worked against the picture's own window, not the frame: a
        // Game Boy coming up inside the panel opens inside the window its picture occupies.
        let (x, y, w, h) = screen_rect_in(self.active.screen_base(), self.power);
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform4f(self.u_rect, x, y, w, h);
            gl::Uniform1f(self.u_bright, screen_brightness(self.power));
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, self.mask);
            gl::ActiveTexture(gl::TEXTURE2);
            gl::BindTexture(gl::TEXTURE_2D, self.pal);
            gl::ActiveTexture(gl::TEXTURE3);
            gl::BindTexture(gl::TEXTURE_2D, self.pix);
            gl::ActiveTexture(gl::TEXTURE0);
        }
        quad.draw();
    }
}

impl Drop for GamePass {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.game_gba);
            gl::DeleteTextures(1, &self.game_gb);
            gl::DeleteTextures(1, &self.game_sfc);
            gl::DeleteTextures(1, &self.game_fc);
            gl::DeleteTextures(1, &self.game_neogeo);
            gl::DeleteTextures(1, &self.game_arcade);
            gl::DeleteTextures(1, &self.game_md);
            gl::DeleteTextures(1, &self.game_ps1);
            gl::DeleteTextures(1, &self.game_pce);
            gl::DeleteTextures(1, &self.mask);
            gl::DeleteTextures(1, &self.pal);
            gl::DeleteTextures(1, &self.pix);
            gl::DeleteProgram(self.prog);
        }
    }
}
