//! Rasterize text to RGBA image buffers for use as 3D quad textures (visible in XR).

use ab_glyph::{FontRef, Font, PxScale, ScaleFont};
use std::path::Path;

/// Load font from path. Returns None if file missing or invalid.
/// The font buffer is intentionally leaked so the returned FontRef is 'static.
pub fn load_font(path: &Path) -> Option<FontRef<'static>> {
    let data = std::fs::read(path).ok()?;
    let leaked: &'static [u8] = Box::leak(data.into_boxed_slice());
    FontRef::try_from_slice(leaked).ok()
}

/// Render a single line of text into an RGBA buffer (row-major, 4 bytes per pixel).
/// Background is transparent (0,0,0,0). Text uses (r,g,b) with alpha from glyph coverage.
/// Width/height must match buffer length (width * height * 4).
pub fn render_text_to_rgba(
    font: &FontRef<'static>,
    text: &str,
    width: u32,
    height: u32,
    font_size: f32,
    r: u8,
    g: u8,
    b: u8,
) -> Vec<u8> {
    let mut buf = vec![0u8; (width as usize) * (height as usize) * 4];
    let scale = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);
    let mut x = 8.0f32;
    let y = scaled.ascent() + 4.0;

    for ch in text.chars() {
        let mut glyph = scaled.scaled_glyph(ch);
        glyph.position = ab_glyph::point(x, y);
        x += scaled.h_advance(glyph.id);

        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|px, py, coverage| {
                let ix = bounds.min.x as i32 + px as i32;
                let iy = bounds.min.y as i32 + py as i32;
                if ix >= 0 && ix < width as i32 && iy >= 0 && iy < height as i32 {
                    // Flip Y so texture is right-side up in VR (GPU tex coords vs our row order)
                    let row = (height as i32 - 1) - iy;
                    let idx = ((row as u32) * width + (ix as u32)) as usize * 4;
                    let alpha = (coverage.min(1.0) * 255.0) as u8;
                    if idx + 3 < buf.len() {
                        buf[idx] = r;
                        buf[idx + 1] = g;
                        buf[idx + 2] = b;
                        buf[idx + 3] = alpha;
                    }
                }
            });
        }
    }
    buf
}

/// Render multiple lines (e.g. feed lines) into an RGBA buffer.
/// Lines are drawn from top to bottom with line_height spacing.
pub fn render_lines_to_rgba(
    font: &FontRef<'static>,
    lines: &[String],
    width: u32,
    height: u32,
    font_size: f32,
    r: u8,
    g: u8,
    b: u8,
) -> Vec<u8> {
    let mut buf = vec![0u8; (width as usize) * (height as usize) * 4];
    let scale = PxScale::from(font_size);
    let scaled = font.as_scaled(scale);
    let line_height = scaled.height() + 2.0;
    let mut y = scaled.ascent() + 4.0;

    for line in lines {
        let mut x = 8.0f32;
        for ch in line.chars() {
            let mut glyph = scaled.scaled_glyph(ch);
            glyph.position = ab_glyph::point(x, y);
            x += scaled.h_advance(glyph.id);

            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|px, py, coverage| {
                    let ix = bounds.min.x as i32 + px as i32;
                    let iy = bounds.min.y as i32 + py as i32;
                    if ix >= 0 && ix < width as i32 && iy >= 0 && iy < height as i32 {
                        let row = (height as i32 - 1) - iy;
                        let idx = ((row as u32) * width + (ix as u32)) as usize * 4;
                        let alpha = (coverage.min(1.0) * 255.0) as u8;
                        if idx + 3 < buf.len() {
                            buf[idx] = r;
                            buf[idx + 1] = g;
                            buf[idx + 2] = b;
                            buf[idx + 3] = alpha;
                        }
                    }
                });
            }
        }
        y += line_height;
    }
    buf
}
