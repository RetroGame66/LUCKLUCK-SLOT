mod draw;
mod fbdev;
mod fbo;
mod gl;
mod grade;
#[cfg(target_os = "macos")]
mod headless;
#[cfg(feature = "host")]
mod host;
mod lcd3x;
mod pipeline;
mod power;
mod quad;
mod shaders;
mod surface;
mod system;

pub use draw::{badge_uv, Draw, TexId, YAW_DEPTH};
// Built on the host too, so the port stays under the type checker and the linter that only
// ever run there. Opening it away from the device fails at the first dlopen, not at compile.
pub use fbdev::{egl_error, panel_mode, panel_size, FbdevSurface};
pub use fbo::{Compositor, BACKDROP};
pub use grade::{blue_light_gain, BLUE_LIGHT_MAX};
#[cfg(target_os = "macos")]
pub use headless::HeadlessSurface;
#[cfg(feature = "host")]
pub use host::HostSurface;
pub use lcd3x::{builtin_panel_mask, lcd3x_mask, mask_texture_rgba8};
pub use pipeline::{SRC_H, SRC_W};
pub use power::{screen_brightness, screen_rect, screen_rect_in, screen_scale, screen_width};
pub use surface::{blit_rect, blit_rect_fit, fit_rect, fit_scale, GfxError, Surface, OUT_H, OUT_W};
pub use system::{
    cell_size, mask_scale, scale_for, System, ARCADE_DRAW_H, ARCADE_DRAW_W, FC_SRC_H, FC_SRC_W,
    GBA_DRAW_H, GBA_DRAW_W, GB_SRC_H, GB_SRC_W, GBA_SRC_H, GBA_SRC_W, MD_SRC_H, MD_SRC_W,
    NEOGEO_SRC_H, NEOGEO_SRC_W, PCE_SRC_H, PCE_SRC_W, PS1_SRC_H, PS1_SRC_W, SFC_SRC_H, SFC_SRC_W,
};
