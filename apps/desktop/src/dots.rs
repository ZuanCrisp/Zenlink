//! Animated dot field behind the stage (port of ui/dots.js): a slow light
//! wave travels outward from the centre and dots near the cursor brighten.
//!
//! Drawn in tiles: a frame redraws (and the window uploads) only the tiles the wave or the
//! cursor lights, or lit in the frame before; the rest keep their texture. Measured: one
//! full-stage image per frame cost ~20% of a core at 30 fps, mostly the texture upload.

use std::rc::Rc;

use slint::{Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::DotTile;

const GAP: f32 = 22.0;
/// Tile edge in physical pixels.
const TILE: u32 = 160;

#[derive(Clone, Copy)]
struct Rgba(f32, f32, f32, f32);

struct Tile {
    /// Physical position and size.
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    /// Every dot at rest.
    base: Vec<Rgba8Pixel>,
    /// Drawn with lit dots last frame.
    lit: bool,
}

pub struct Dots {
    key: (u32, u32, u32, bool),
    tiles: Vec<Tile>,
    model: Rc<VecModel<DotTile>>,
}

impl Dots {
    pub fn new() -> Self {
        Self {
            key: (0, 0, 0, false),
            tiles: Vec::new(),
            model: Rc::new(VecModel::default()),
        }
    }

    /// `w`, `h`: logical size; `sf`: scale factor; `t`: seconds;
    /// `mouse`: logical position in the stage, if hovering. Returns the tile model when the
    /// layout changed (size, scale, theme); later frames update its rows in place.
    pub fn render(
        &mut self,
        w: f32,
        h: f32,
        sf: f32,
        t: f32,
        mouse: Option<(f32, f32)>,
        dark: bool,
    ) -> Option<ModelRc<DotTile>> {
        let (pw, ph) = (
            (w * sf).round().max(1.0) as u32,
            (h * sf).round().max(1.0) as u32,
        );
        let (base, hot) = if dark {
            (
                Rgba(255.0, 115.0, 188.0, 0.20),
                Rgba(255.0, 115.0, 188.0, 1.0),
            )
        } else {
            (
                Rgba(244.0, 107.0, 172.0, 0.22),
                Rgba(244.0, 107.0, 172.0, 1.0),
            )
        };
        let (ox, oy) = ((w % GAP) / 2.0, (h % GAP) / 2.0);
        let grid = || {
            let mut v = Vec::new();
            let mut y = oy;
            while y < h {
                let mut x = ox;
                while x < w {
                    v.push((x, y));
                    x += GAP;
                }
                y += GAP;
            }
            v
        };
        let fresh = self.key != (pw, ph, sf.to_bits(), dark);
        if fresh {
            self.key = (pw, ph, sf.to_bits(), dark);
            self.tiles.clear();
            let rest = Rgba(base.0, base.1, base.2, base.3 * 0.75);
            let dots = grid();
            for ty in (0..ph).step_by(TILE as usize) {
                for tx in (0..pw).step_by(TILE as usize) {
                    let (tw, th) = (TILE.min(pw - tx), TILE.min(ph - ty));
                    let mut px = vec![
                        Rgba8Pixel {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0
                        };
                        (tw * th) as usize
                    ];
                    for &(x, y) in &dots {
                        dot(
                            &mut px,
                            tw,
                            th,
                            x * sf - tx as f32,
                            y * sf - ty as f32,
                            sf,
                            rest,
                        );
                    }
                    self.tiles.push(Tile {
                        x: tx,
                        y: ty,
                        w: tw,
                        h: th,
                        base: px,
                        lit: true,
                    });
                }
            }
            let rows: Vec<DotTile> = self
                .tiles
                .iter()
                .map(|t| DotTile {
                    x: t.x as f32 / sf,
                    y: t.y as f32 / sf,
                    w: t.w as f32 / sf,
                    h: t.h as f32 / sf,
                    img: Image::default(),
                })
                .collect();
            self.model = Rc::new(VecModel::from(rows));
        }
        // The lit dots: centre and radius (physical), colour.
        let (cx, cy) = (w / 2.0, h / 2.0);
        let max_r = cx.hypot(cy);
        let wave = (t * 120.0) % (max_r + 240.0) - 120.0;
        let mut lit = Vec::new();
        for (x, y) in grid() {
            let d = (x - cx).hypot(y - cy);
            let ring = (1.0 - (d - wave).abs() / 90.0).max(0.0);
            let near = mouse
                .map(|(mx, my)| (1.0 - (x - mx).hypot(y - my) / 140.0).max(0.0))
                .unwrap_or(0.0);
            let k = (ring * 0.55 + near * 0.9).min(1.0);
            if k > 0.05 {
                let alpha = hot.3 * (0.75 + k * 0.25);
                lit.push((
                    x * sf,
                    y * sf,
                    (1.0 + k * 0.9) * sf,
                    Rgba(hot.0, hot.1, hot.2, alpha),
                ));
            }
        }
        for (i, tile) in self.tiles.iter_mut().enumerate() {
            let (x0, y0) = (tile.x as f32, tile.y as f32);
            let (x1, y1) = (x0 + tile.w as f32, y0 + tile.h as f32);
            let mine: Vec<_> = lit
                .iter()
                .filter(|(x, y, r, _)| {
                    x + r + 1.0 >= x0 && x - r - 1.0 < x1 && y + r + 1.0 >= y0 && y - r - 1.0 < y1
                })
                .collect();
            if mine.is_empty() && !tile.lit {
                continue; // at rest, as uploaded before
            }
            tile.lit = !mine.is_empty();
            let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(tile.w, tile.h);
            let px = buf.make_mut_slice();
            px.copy_from_slice(&tile.base);
            for &&(x, y, r, c) in &mine {
                dot(px, tile.w, tile.h, x - x0, y - y0, r, c);
            }
            if let Some(mut row) = self.model.row_data(i) {
                row.img = Image::from_rgba8_premultiplied(buf);
                self.model.set_row_data(i, row);
            }
        }
        fresh.then(|| ModelRc::from(self.model.clone()))
    }
}

/// Anti-aliased filled circle, premultiplied alpha, "over" blending.
fn dot(px: &mut [Rgba8Pixel], w: u32, h: u32, cx: f32, cy: f32, r: f32, c: Rgba) {
    if cx + r + 1.0 < 0.0
        || cy + r + 1.0 < 0.0
        || cx - r - 1.0 >= w as f32
        || cy - r - 1.0 >= h as f32
    {
        return;
    }
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

#[cfg(test)]
mod tests {
    #[test]
    fn only_lit_tiles_are_redrawn() {
        let mut d = super::Dots::new();
        assert!(
            d.render(1280.0, 760.0, 1.5, 0.5, None, true).is_some(),
            "first frame lays out"
        );
        let model = d.model.clone();
        let img = |i: usize| slint::Model::row_data(&*model, i).unwrap().img;
        let before: Vec<_> = (0..d.tiles.len()).map(img).collect();
        assert!(
            d.render(1280.0, 760.0, 1.5, 0.55, None, true).is_none(),
            "same layout"
        );
        let changed = (0..d.tiles.len()).filter(|&i| img(i) != before[i]).count();
        assert!(
            changed > 0 && changed < d.tiles.len() / 2,
            "{changed} of {} tiles redrawn",
            d.tiles.len()
        );
    }

    #[test]
    #[ignore]
    fn render_cost() {
        let mut d = super::Dots::new();
        let t0 = std::time::Instant::now();
        for i in 0..30 {
            d.render(
                1280.0,
                760.0,
                1.5,
                i as f32 / 30.0,
                Some((300.0, 300.0)),
                true,
            );
        }
        eprintln!(
            "render: {:.2} ms/frame",
            t0.elapsed().as_secs_f64() * 1000.0 / 30.0
        );
    }
}
