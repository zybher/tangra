//! Rounded clipping: egui can only clip to rectangles, so an object inside a rounded frame is
//! cut to the frame's rounded shape by hand. Both shapes are convex (rounded rectangles), so
//! their overlap is a convex polygon: images are drawn as a textured mesh of that polygon,
//! frames as a filled polygon. Edges get a 1px soft ring so they stay smooth.
use crate::*;
use egui::Vec2;

/// Rounded rectangle as a polygon (clockwise on screen)
pub(crate) fn rounded_poly(r: Rect, rad: f32) -> Vec<Pos2> {
    let rad = rad.clamp(0.0, (r.width().min(r.height()) / 2.0).max(0.0));
    if rad < 0.5 {
        return vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
    }
    let n = ((rad * 0.6).ceil() as usize).clamp(4, 24);
    let mut out = Vec::with_capacity(4 * (n + 1));
    // centre of each corner arc and its start angle (screen: y down)
    let arcs = [
        (pos2(r.min.x + rad, r.min.y + rad), std::f32::consts::PI),
        (pos2(r.max.x - rad, r.min.y + rad), 1.5 * std::f32::consts::PI),
        (pos2(r.max.x - rad, r.max.y - rad), 0.0),
        (pos2(r.min.x + rad, r.max.y - rad), 0.5 * std::f32::consts::PI),
    ];
    for (c, a0) in arcs {
        for i in 0..=n {
            let a = a0 + std::f32::consts::FRAC_PI_2 * i as f32 / n as f32;
            out.push(c + rad * vec2(a.cos(), a.sin()));
        }
    }
    out
}

fn cross(o: Pos2, a: Pos2, b: Pos2) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn area2(p: &[Pos2]) -> f32 {
    (0..p.len()).map(|i| {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        a.x * b.y - b.x * a.y
    }).sum()
}

/// Sutherland–Hodgman: the part of `subject` inside the convex polygon `clip`
pub(crate) fn clip_convex(subject: &[Pos2], clip: &[Pos2]) -> Vec<Pos2> {
    let s = area2(clip).signum();
    let mut out = subject.to_vec();
    for i in 0..clip.len() {
        if out.is_empty() {
            break;
        }
        let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
        let inside = |p: Pos2| cross(a, b, p) * s >= 0.0;
        let input = std::mem::take(&mut out);
        for j in 0..input.len() {
            let (p, q) = (input[j], input[(j + 1) % input.len()]);
            let (pi, qi) = (inside(p), inside(q));
            if pi {
                out.push(p);
            }
            if pi != qi {
                let (dp, dq) = (cross(a, b, p), cross(a, b, q));
                let t = dp / (dp - dq);
                out.push(p + (q - p) * t);
            }
        }
    }
    out
}

/// Outward normal at each corner of a convex polygon
fn normals(p: &[Pos2]) -> Vec<Vec2> {
    let s = -area2(p).signum();
    (0..p.len())
        .map(|i| {
            let (prev, cur, next) = (p[(i + p.len() - 1) % p.len()], p[i], p[(i + 1) % p.len()]);
            let n1 = (cur - prev).rot90().normalized();
            let n2 = (next - cur).rot90().normalized();
            let n = (n1 + n2).normalized();
            if n.is_finite() { n * s } else { Vec2::ZERO }
        })
        .collect()
}

/// A convex polygon filled with a texture (uv from where each point sits in `img`), with a
/// 1px fade at the edge
pub(crate) fn textured_poly(poly: &[Pos2], img: Rect, tex: egui::TextureId, tint: Color32) -> egui::Mesh {
    let mut m = egui::Mesh::with_texture(tex);
    if poly.len() < 3 || img.width() <= 0.0 || img.height() <= 0.0 {
        return m;
    }
    let uv = |q: Pos2| pos2(((q.x - img.min.x) / img.width()).clamp(0.0, 1.0), ((q.y - img.min.y) / img.height()).clamp(0.0, 1.0));
    let ns = normals(poly);
    let n = poly.len() as u32;
    // inner ring (solid, half a pixel in) then outer ring (clear, half a pixel out)
    for (q, nv) in poly.iter().zip(&ns) {
        let q = *q - *nv * 0.5;
        m.vertices.push(egui::epaint::Vertex { pos: q, uv: uv(q), color: tint });
    }
    for (q, nv) in poly.iter().zip(&ns) {
        let q = *q + *nv * 0.5;
        m.vertices.push(egui::epaint::Vertex { pos: q, uv: uv(q), color: Color32::TRANSPARENT });
    }
    for i in 1..n - 1 {
        m.add_triangle(0, i, i + 1);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        m.add_triangle(i, j, n + i);
        m.add_triangle(j, n + j, n + i);
    }
    m
}

impl App {
    /// Rounded clipping frames around an object: (world rect, world radius), only when at
    /// least one of them is rounded
    pub(crate) fn round_clips(&self, id: &str) -> Vec<([f32; 4], f32)> {
        let mut out = Vec::new();
        let mut any_round = false;
        let mut cur = self.parent_of(id);
        for _ in 0..64 {
            let Some(pid) = cur else { break };
            // Clipping switched on / off just now: the eased box, its corners rounding with it
            // (canvas/clip_fade.rs)
            let eased = super::clip_fade::easing(pid);
            if let Some(g) = self.board.group_cards.iter().find(|g| g.id == pid && (g.clip || eased.is_some())) {
                let rad = group_radius(g);
                any_round |= rad > 0.5;
                // Cropped at the stroke's inner edge, like the rect clip
                let full = self.crop_box(g);
                let r = (rad - (full[0] - g.x)).max(0.0);
                match eased {
                    Some((b, k)) => out.push((b, r * k)),
                    None => out.push((full, r)),
                }
            }
            cur = self.parent_of(pid);
        }
        if any_round { out } else { Vec::new() }
    }
}
