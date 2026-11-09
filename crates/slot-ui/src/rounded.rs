//! Rounded rectangles, rasterised.
//!
//! `Draw` has no rounded rect, and a stack of rects stepping in at each end is more code with
//! worse edges. So the one maths every GPU does in a shader is done here once, at bake time,
//! where the cost is a few thousand pixels nobody is waiting for.

/// How much of the pixel at `(x, y)` a rounded rectangle covers, 0 to 1.
///
/// The shape's core is the box shrunk by the radius, so a pixel's distance to that core is its
/// distance to the shape's edge; the last half pixel is feathered so a corner does not
/// stair-step.
pub(crate) fn cover(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let cx = x.clamp(r, w - r);
    let cy = y.clamp(r, h - r);
    let dx = x - cx;
    let dy = y - cy;
    (r - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0)
}

/// A rounded plate in `fill`, with a rim in `rim` inset by `rim_px` when there is one.
///
/// Both are `[f32; 4]` in the palette's own 0..1 terms — the plate's alpha is what makes it a
/// plate rather than a panel, and the rim's is what makes it a hairline rather than a border.
pub(crate) fn plate(
    rgba: &mut [u8],
    w: u32,
    h: u32,
    radius: f32,
    rim_px: f32,
    fill: [f32; 4],
    rim: Option<[f32; 4]>,
) {
    let (fw, fh) = (w as f32, h as f32);
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let outer = cover(px, py, fw, fh, radius);
            if outer <= 0.0 {
                continue;
            }
            let (mut c, mut a) = (fill, outer * fill[3]);
            if let Some(rim_c) = rim {
                let inner = cover(
                    px - rim_px,
                    py - rim_px,
                    fw - 2.0 * rim_px,
                    fh - 2.0 * rim_px,
                    (radius - rim_px).max(1.0),
                );
                let band = (outer - inner).max(0.0);
                if band > 0.0 {
                    for k in 0..3 {
                        c[k] = fill[k] * (1.0 - band) + rim_c[k] * band;
                    }
                    a = a.max(band * rim_c[3]);
                }
            }
            let i = ((y * w + x) * 4) as usize;
            rgba[i] = (c[0].clamp(0.0, 1.0) * 255.0) as u8;
            rgba[i + 1] = (c[1].clamp(0.0, 1.0) * 255.0) as u8;
            rgba[i + 2] = (c[2].clamp(0.0, 1.0) * 255.0) as u8;
            rgba[i + 3] = (a.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
}
