//! Ellipses painted as their own meshes. egui's closed lines mitre every join, so the sharp
//! ends of a very thin ellipse shot long spikes far past its box; here every ring is the
//! ellipse grown or shrunk per axis, which stays inside its box and never turns inside out.
use eframe::egui::{pos2, Color32, Mesh, Pos2, Rect, Shape, Vec2};

/// Edge softening each side of an edge (screen px), like egui's feathering
const F: f32 = 0.5;

struct Ell {
    c: Pos2,
    rx: f32,
    ry: f32,
    /// Each point as a unit-circle direction
    u: Vec<Vec2>,
}

impl Ell {
    fn new(pts: &[Pos2]) -> Self {
        let b = Rect::from_points(pts);
        let (c, rx, ry) = (b.center(), b.width() / 2.0, b.height() / 2.0);
        let d = |v: f32, r: f32| if r > 1e-6 { v / r } else { 0.0 };
        let u = pts.iter().map(|p| Vec2::new(d(p.x - c.x, rx), d(p.y - c.y, ry))).collect();
        Ell { c, rx, ry, u }
    }
    /// Point `i` on the ellipse grown by `d` (negative: shrunk, at most to its centre)
    fn at(&self, i: usize, d: f32) -> Pos2 {
        let u = self.u[i % self.u.len()];
        pos2(self.c.x + u.x * (self.rx + d).max(0.0), self.c.y + u.y * (self.ry + d).max(0.0))
    }
    /// A band between offsets `d0` (colour `c0`) and `d1` (colour `c1`)
    fn band(&self, m: &mut Mesh, d0: f32, c0: Color32, d1: f32, c1: Color32) {
        let n = self.u.len();
        let base = m.vertices.len() as u32;
        for i in 0..n {
            m.colored_vertex(self.at(i, d0), c0);
            m.colored_vertex(self.at(i, d1), c1);
        }
        for i in 0..n as u32 {
            let j = (i + 1) % n as u32;
            let (a, b, c, d) = (base + 2 * i, base + 2 * i + 1, base + 2 * j, base + 2 * j + 1);
            m.add_triangle(a, b, c);
            m.add_triangle(b, d, c);
        }
    }
}

/// Filled ellipse through `pts`
pub(crate) fn fill(pts: &[Pos2], col: Color32) -> Shape {
    let e = Ell::new(pts);
    let mut m = Mesh::default();
    let n = e.u.len() as u32;
    m.colored_vertex(e.c, col);
    for i in 0..n as usize {
        m.colored_vertex(e.at(i, -F), col);
    }
    for i in 0..n {
        m.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    e.band(&mut m, -F, col, F, Color32::TRANSPARENT);
    Shape::mesh(m)
}

/// Outline of `width` around the ellipse through `pts`, its centre line moved `inset` inward
pub(crate) fn stroke(pts: &[Pos2], inset: f32, width: f32, col: Color32) -> Shape {
    let e = Ell::new(pts);
    // Thinner than a pixel: fade instead of shrinking further
    let (w, col) = if width < 1.0 { (1.0, col.gamma_multiply(width.max(0.0))) } else { (width, col) };
    let (o, i) = (-inset + w / 2.0, -inset - w / 2.0);
    let mut m = Mesh::default();
    e.band(&mut m, o + F, Color32::TRANSPARENT, o - F, col);
    e.band(&mut m, o - F, col, i + F, col);
    e.band(&mut m, i + F, col, i - F, Color32::TRANSPARENT);
    Shape::mesh(m)
}
