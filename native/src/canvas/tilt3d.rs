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

    /// The four shown corners (world), clockwise from top-left
    pub fn corners(&self, bx: [f32; 4]) -> [(f32, f32); 4] {
        let [x, y, w, h] = bx;
        [self.fwd(x, y), self.fwd(x + w, y), self.fwd(x + w, y + h), self.fwd(x, y + h)]
    }
}
