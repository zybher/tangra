//! 3D tilt for pictures: the flat picture is turned in 3D around its centre and
//! seen through a pinhole camera. Drawn as a fine grid mesh so the texture stays
//! perspective-correct; the lasso maps back through `back` to cut the right spot.

use crate::image_edit::ImageEdit;
use eframe::egui::{self, pos2, Color32, Mesh, Pos2, TextureId};

/// Grid cells per side (16×16 hides egui's affine texture interpolation)
const N: u32 = 16;

pub(crate) struct Tilt {
    r: [[f32; 3]; 3],
    f: f32,
    cx: f32,
    cy: f32,
}

impl Tilt {
    /// For a picture at world box (x, y, w, h)
    pub fn new(e: &ImageEdit, x: f32, y: f32, w: f32, h: f32) -> Tilt {
        let rot = |deg: f32| deg.to_radians().sin_cos();
        let (sx, cx) = rot(e.tilt_x);
        let (sy, cy) = rot(e.tilt_y);
        let (sz, cz) = rot(e.spin);
        let rx = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
        let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
        let rz = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
        let mul = |a: [[f32; 3]; 3], b: [[f32; 3]; 3]| {
            let mut m = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    m[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
                }
            }
            m
        };
        let r = mul(rz, mul(ry, rx));
        // Camera distance: more depth = closer camera = stronger perspective,
        // never so close that a corner swings behind it
        let diag = (w * w + h * h).sqrt();
        let f = (w.max(h) * 2.0 * 2f32.powf(-e.depth / 60.0)).max(diag * 0.6);
        Tilt { r, f, cx: x + w / 2.0, cy: y + h / 2.0 }
    }

    /// World point on the flat picture -> where it shows after tilting
    pub fn fwd(&self, x: f32, y: f32) -> (f32, f32) {
        let (u, v) = (x - self.cx, y - self.cy);
        let r = &self.r;
        let (px, py, pz) = (r[0][0] * u + r[0][1] * v, r[1][0] * u + r[1][1] * v, r[2][0] * u + r[2][1] * v);
        let k = self.f / (self.f + pz).max(1e-3);
        (self.cx + px * k, self.cy + py * k)
    }

    /// Shown world point -> the point on the flat picture under it
    pub fn back(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let (qx, qy, f, r) = (x - self.cx, y - self.cy, self.f, &self.r);
        let (a, b, c) = (f * r[0][0] - qx * r[2][0], f * r[0][1] - qx * r[2][1], qx * f);
        let (d, e, g) = (f * r[1][0] - qy * r[2][0], f * r[1][1] - qy * r[2][1], qy * f);
        let det = a * e - b * d;
        if det.abs() < 1e-6 {
            return None;
        }
        Some((self.cx + (c * e - b * g) / det, self.cy + (a * g - c * d) / det))
    }

    /// How squarely the picture faces the viewer (1 = flat on, 0 = edge on)
    pub fn facing(&self) -> f32 {
        self.r[2][2].abs()
    }

    /// The tilted picture as a textured grid mesh, in screen space
    pub fn mesh(&self, bx: [f32; 4], to_screen: impl Fn(f32, f32) -> Pos2, tex: TextureId, tint: Color32) -> Mesh {
        let [x, y, w, h] = bx;
        let mut m = Mesh::with_texture(tex);
        // Turning away from the light darkens it a touch, so the tilt reads as 3D
        let s = 0.8 + 0.2 * self.facing();
        let col = Color32::from_rgba_premultiplied(
            (tint.r() as f32 * s) as u8,
            (tint.g() as f32 * s) as u8,
            (tint.b() as f32 * s) as u8,
            tint.a(),
        );
        for j in 0..=N {
            for i in 0..=N {
                let (u, v) = (i as f32 / N as f32, j as f32 / N as f32);
                let (px, py) = self.fwd(x + u * w, y + v * h);
                m.vertices.push(egui::epaint::Vertex { pos: to_screen(px, py), uv: pos2(u, v), color: col });
            }
        }
        for j in 0..N {
            for i in 0..N {
                let a = j * (N + 1) + i;
                m.indices.extend_from_slice(&[a, a + 1, a + N + 1, a + 1, a + N + 2, a + N + 1]);
            }
        }
        m
    }

    /// The tilted picture with rounded corners (`rd`: world units), in screen space: rings
    /// shrinking from the rounded outline to the centre, fine enough to stay perspective-correct
    pub fn mesh_rounded(&self, bx: [f32; 4], rd: f32, to_screen: impl Fn(f32, f32) -> Pos2, tex: TextureId, tint: Color32) -> Mesh {
        if rd <= 0.0 {
            return self.mesh(bx, to_screen, tex, tint);
        }
        let [x, y, w, h] = bx;
        let col = self.shade(tint);
        let ol = flat_outline(bx, rd);
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let mut m = Mesh::with_texture(tex);
        let mut vert = |fx: f32, fy: f32| {
            let (px, py) = self.fwd(fx, fy);
            m.vertices.push(egui::epaint::Vertex { pos: to_screen(px, py), uv: pos2((fx - x) / w, (fy - y) / h), color: col });
        };
        vert(cx, cy);
        for i in 1..=N {
            let t = i as f32 / N as f32;
            for &(fx, fy) in &ol {
                vert(cx + (fx - cx) * t, cy + (fy - cy) * t);
            }
        }
        let n = ol.len() as u32;
        let at = |ring: u32, k: u32| 1 + (ring - 1) * n + k % n;
        for k in 0..n {
            m.indices.extend_from_slice(&[0, at(1, k), at(1, k + 1)]);
        }
        for ring in 1..N {
            for k in 0..n {
                let (a, b, c, d) = (at(ring, k), at(ring, k + 1), at(ring + 1, k), at(ring + 1, k + 1));
                m.indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
        m
    }

    /// The tilted picture's outline as shown (world), rounded corners included
    pub fn shown_outline(&self, bx: [f32; 4], rd: f32) -> Vec<(f32, f32)> {
        flat_outline(bx, rd).into_iter().map(|(fx, fy)| self.fwd(fx, fy)).collect()
    }

    /// Turning away from the light darkens it a touch, so the tilt reads as 3D
    fn shade(&self, tint: Color32) -> Color32 {
        let s = 0.8 + 0.2 * self.facing();
        Color32::from_rgba_premultiplied((tint.r() as f32 * s) as u8, (tint.g() as f32 * s) as u8, (tint.b() as f32 * s) as u8, tint.a())
    }

    /// The four shown corners (world), clockwise from top-left
    pub fn corners(&self, bx: [f32; 4]) -> [(f32, f32); 4] {
        let [x, y, w, h] = bx;
        [self.fwd(x, y), self.fwd(x + w, y), self.fwd(x + w, y + h), self.fwd(x, y + h)]
    }
}

/// The flat picture's rounded outline, clockwise (screen y down) from the top-left corner;
/// straight edges are split too so the tilted mesh stays fine everywhere
fn flat_outline(bx: [f32; 4], rd: f32) -> Vec<(f32, f32)> {
    use std::f32::consts::{FRAC_PI_2, PI};
    const ARC: u32 = 8;
    const EDGE: u32 = 12;
    let [x, y, w, h] = bx;
    let r = rd.min(w / 2.0).min(h / 2.0).max(0.0);
    let cs = [(x + r, y + r, PI), (x + w - r, y + r, 1.5 * PI), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, FRAC_PI_2)];
    let pt = |(cx, cy, a): (f32, f32, f32)| (cx + r * a.cos(), cy + r * a.sin());
    let mut out = Vec::new();
    for i in 0..4 {
        let (cx, cy, a0) = cs[i];
        for k in 0..=ARC {
            out.push(pt((cx, cy, a0 + FRAC_PI_2 * k as f32 / ARC as f32)));
        }
        let (e0, e1) = (pt((cx, cy, a0 + FRAC_PI_2)), pt(cs[(i + 1) % 4]));
        for k in 1..EDGE {
            let t = k as f32 / EDGE as f32;
            out.push((e0.0 + (e1.0 - e0.0) * t, e0.1 + (e1.1 - e0.1) * t));
        }
    }
    out
}

/// A soft shadow under a convex outline (screen points): stacked faint copies grown and shrunk
/// around its edge, matching the flat pictures' `Shadow` (offset, blur, darkness)
pub(crate) fn soft_shadow(p: &egui::Painter, pts: &[Pos2], offset: egui::Vec2, blur: f32, alpha: f32) {
    let mut q: Vec<Pos2> = Vec::new();
    for &pt in pts {
        if q.last().map_or(true, |l: &Pos2| (*l - pt).length() > 0.25) {
            q.push(pt + offset);
        }
    }
    if q.len() > 2 && (q[0] - q[q.len() - 1]).length() <= 0.25 {
        q.pop();
    }
    if q.len() < 3 {
        return;
    }
    let n = q.len();
    let c = q.iter().fold(egui::Vec2::ZERO, |a, p| a + p.to_vec2()) / n as f32;
    // outward direction at each point: its two edges' normals, turned away from the centre
    let normals: Vec<egui::Vec2> = (0..n)
        .map(|i| {
            let e = |a: Pos2, b: Pos2| {
                let d = (b - a).normalized();
                let nn = egui::vec2(d.y, -d.x);
                if nn.dot(a.lerp(b, 0.5).to_vec2() - c) < 0.0 { -nn } else { nn }
            };
            (e(q[(i + n - 1) % n], q[i]) + e(q[i], q[(i + 1) % n])).normalized()
        })
        .collect();
    const L: usize = 8;
    let a = 1.0 - (1.0 - alpha).powf(1.0 / L as f32);
    let col = Color32::from_black_alpha((a * 255.0).round() as u8);
    for l in 0..L {
        let d = blur * ((l as f32 + 0.5) / L as f32 - 0.5);
        let poly: Vec<Pos2> = q.iter().zip(&normals).map(|(p, nn)| *p + *nn * d).collect();
        p.add(egui::Shape::convex_polygon(poly, col, egui::Stroke::NONE));
    }
}
