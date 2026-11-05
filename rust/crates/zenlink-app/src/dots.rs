//! Animated dot field behind the stage (port of ui/dots.js): a slow light
//! wave travels outward from the centre and dots near the cursor brighten.
//! Drawn into a pixel buffer at ~30 fps, only while there is no picture.

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

const GAP: f32 = 22.0;

pub struct Dots {
    buf: Option<SharedPixelBuffer<Rgba8Pixel>>,
}

#[derive(Clone, Copy)]
struct Rgba(f32, f32, f32, f32);

impl Dots {
    pub fn new() -> Self {
        Self { buf: None }
    }

    /// `w`, `h`: logical size; `sf`: scale factor; `t`: seconds;
    /// `mouse`: logical position in the stage, if hovering.
    pub fn render(&mut self, w: f32, h: f32, sf: f32, t: f32, mouse: Option<(f32, f32)>, dark: bool) -> Image {
        let (pw, ph) = ((w * sf).round().max(1.0) as u32, (h * sf).round().max(1.0) as u32);
        let reuse = matches!(&self.buf, Some(b) if b.width() == pw && b.height() == ph);
        if !reuse {
            self.buf = Some(SharedPixelBuffer::new(pw, ph));
        }
        let buf = self.buf.as_mut().unwrap();
        let px = buf.make_mut_slice();
        for p in px.iter_mut() {
            *p = Rgba8Pixel { r: 0, g: 0, b: 0, a: 0 };
        }
        let (base, hot) = if dark {
            (Rgba(255.0, 115.0, 188.0, 0.20), Rgba(255.0, 115.0, 188.0, 1.0))
        } else {
            (Rgba(244.0, 107.0, 172.0, 0.22), Rgba(244.0, 107.0, 172.0, 1.0))
        };
        let (cx, cy) = (w / 2.0, h / 2.0);
        let max_r = cx.hypot(cy);
        let wave = (t * 120.0) % (max_r + 240.0) - 120.0;
        let (ox, oy) = ((w % GAP) / 2.0, (h % GAP) / 2.0);
        let mut y = oy;
        while y < h {
            let mut x = ox;
            while x < w {
                let d = (x - cx).hypot(y - cy);
                let ring = (1.0 - (d - wave).abs() / 90.0).max(0.0);
                let near = mouse.map(|(mx, my)| (1.0 - (x - mx).hypot(y - my) / 140.0).max(0.0)).unwrap_or(0.0);
                let k = (ring * 0.55 + near * 0.9).min(1.0);
                let c = if k > 0.05 { hot } else { base };
                let alpha = c.3 * (0.75 + k * 0.25);
                let r = (1.0 + k * 0.9) * sf;
                dot(px, pw, ph, x * sf, y * sf, r, Rgba(c.0, c.1, c.2, alpha));
                x += GAP;
            }
            y += GAP;
        }
        Image::from_rgba8_premultiplied(buf.clone())
    }
}

/// Anti-aliased filled circle, premultiplied alpha, "over" blending.
fn dot(px: &mut [Rgba8Pixel], w: u32, h: u32, cx: f32, cy: f32, r: f32, c: Rgba) {
    let x0 = (cx - r - 1.0).floor().max(0.0) as u32;
    let y0 = (cy - r - 1.0).floor().max(0.0) as u32;
    let x1 = ((cx + r + 1.0).ceil() as u32).min(w.saturating_sub(1));
    let y1 = ((cy + r + 1.0).ceil() as u32).min(h.saturating_sub(1));
    for y in y0..=y1 {
        for x in x0..=x1 {
            let d = ((x as f32 + 0.5) - cx).hypot((y as f32 + 0.5) - cy);
            let cov = (r + 0.5 - d).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            let a = c.3 * cov;
            let p = &mut px[(y * w + x) as usize];
            let inv = 1.0 - a;
            p.r = (c.0 * a + p.r as f32 * inv) as u8;
            p.g = (c.1 * a + p.g as f32 * inv) as u8;
            p.b = (c.2 * a + p.b as f32 * inv) as u8;
            p.a = (255.0 * a + p.a as f32 * inv) as u8;
        }
    }
}
