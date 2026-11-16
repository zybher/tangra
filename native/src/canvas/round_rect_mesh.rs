//! Rounded rectangle outlines painted as their own mesh. egui strokes a rounded rect around a
//! path at the band's inner edge, so a stroke thicker than the corner radius kept square outer
//! corners (and a choppy curve just past it). Here every ring is the rectangle grown or shrunk
//! with its corner radius grown or shrunk alongside, the same way the ellipses do it.
use eframe::egui::{pos2, Color32, Mesh, Pos2, Rect, Shape};

/// Edge softening each side of an edge (screen px), like egui's feathering
const F: f32 = 0.5;

struct Ring {
    r: Rect,
    rad: f32,
    /// Points per corner
    k: usize,
}

impl Ring {
    /// Point `j` of the outline grown by `d` (negative: shrunk, at most to its centre line)
    fn at(&self, j: usize, d: f32) -> Pos2 {
        let (corner, s) = (j / self.k, (j % self.k) as f32 / (self.k - 1) as f32);
        let c = self.r.center();
        let (hw, hh) = ((self.r.width() / 2.0 + d).max(0.0), (self.r.height() / 2.0 + d).max(0.0));
        let rad = (self.rad + d).max(0.0).min(hw).min(hh);
        // Corners clockwise from top-left, each sweeping a quarter turn
        let (sx, sy, a0) = match corner {
            0 => (-1.0, -1.0, std::f32::consts::PI),
            1 => (1.0, -1.0, 1.5 * std::f32::consts::PI),
            2 => (1.0, 1.0, 0.0),
            _ => (-1.0, 1.0, 0.5 * std::f32::consts::PI),
        };
        let a = a0 + s * std::f32::consts::FRAC_PI_2;
        pos2(c.x + sx * (hw - rad) + rad * a.cos(), c.y + sy * (hh - rad) + rad * a.sin())
    }
    /// A band between offsets `d0` (colour `c0`) and `d1` (colour `c1`)
    fn band(&self, m: &mut Mesh, d0: f32, c0: Color32, d1: f32, c1: Color32) {
        let n = self.k * 4;
        let base = m.vertices.len() as u32;
        for j in 0..n {
            m.colored_vertex(self.at(j, d0), c0);
            m.colored_vertex(self.at(j, d1), c1);
        }
        for i in 0..n as u32 {
            let j = (i + 1) % n as u32;
            let (a, b, c, d) = (base + 2 * i, base + 2 * i + 1, base + 2 * j, base + 2 * j + 1);
            m.add_triangle(a, b, c);
            m.add_triangle(b, d, c);
        }
    }
}

/// Outline of `width` around `r` (corner radius `rad`), its centre line moved `inset` inward
pub(crate) fn stroke(r: Rect, rad: f32, inset: f32, width: f32, col: Color32) -> Shape {
    // Thinner than a pixel: fade instead of shrinking further
    let (w, col) = if width < 1.0 { (1.0, col.gamma_multiply(width.max(0.0))) } else { (width, col) };
    let (o, i) = (-inset + w / 2.0, -inset - w / 2.0);
    let k = ((rad + o).max(0.0).sqrt() * 3.0).clamp(2.0, 40.0) as usize;
    let ring = Ring { r, rad, k };
    let mut m = Mesh::default();
    ring.band(&mut m, o + F, Color32::TRANSPARENT, o - F, col);
    // The band fills the whole shape: no inner edge to soften
    let solid = r.width() / 2.0 + i <= 0.0 || r.height() / 2.0 + i <= 0.0;
    if solid {
        ring.band(&mut m, o - F, col, i, col);
    } else {
        ring.band(&mut m, o - F, col, i + F, col);
        ring.band(&mut m, i + F, col, i - F, Color32::TRANSPARENT);
    }
    Shape::mesh(m)
}
