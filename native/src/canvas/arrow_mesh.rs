//! An arrow as one mesh: its shaft and head are cut so they tile without overlapping, so a
//! see-through arrow reads as one shape (no darker joints where pieces cross, item 15).
//! Edge smoothing is only added along the outside of the whole shape, never between pieces.
//! Tip styles (`Stroke::tip`): 0 line (open chevron), 1 triangle, 2 reversed triangle,
//! 3 circle, 4 diamond. Caps (`Stroke::cap`, the tail and a line head's wing ends): 0 round,
//! 1 square.
use crate::egui::epaint::{Mesh, Vertex, WHITE_UV};
use crate::egui::{Color32, Pos2, Vec2};

/// Names for the tip picker (props.rs), by `Stroke::tip`
pub(crate) const TIPS: [&str; 5] = ["Line", "Triangle", "Reversed", "Circle", "Diamond"];

struct B {
    m: Mesh,
    col: Color32,
    /// edge-smoothing width (one screen pixel)
    f: f32,
}

impl B {
    fn v(&mut self, p: Pos2, c: Color32) -> u32 {
        self.m.vertices.push(Vertex { pos: p, uv: WHITE_UV, color: c });
        self.m.vertices.len() as u32 - 1
    }
    /// A piece fanned from `c` over the closed `ring`; `out[i]`: edge ring[i] -> ring[i+1] is
    /// on the outside of the whole arrow (gets edge smoothing; shared edges don't)
    fn piece(&mut self, c: Pos2, ring: &[(Pos2, bool)]) {
        let n = ring.len();
        if n < 2 {
            return;
        }
        let ci = self.v(c, self.col);
        let base = self.m.vertices.len() as u32;
        for (p, _) in ring {
            self.v(*p, self.col);
        }
        for i in 0..n {
            self.m.add_triangle(ci, base + i as u32, base + ((i + 1) % n) as u32);
        }
        // Fade strip outward along the outside edges (direction away from the piece's centre)
        let clear = Color32::TRANSPARENT;
        for i in 0..n {
            let (a, out) = ring[i];
            let b = ring[(i + 1) % n].0;
            if !out {
                continue;
            }
            let e = b - a;
            if e.length() < 1e-4 {
                continue;
            }
            let mut nn = Vec2::new(-e.y, e.x).normalized();
            let mid = a + e * 0.5;
            if nn.dot(mid - c) < 0.0 {
                nn = -nn;
            }
            let (ia, ib) = (self.v(a, self.col), self.v(b, self.col));
            let (oa, ob) = (self.v(a + nn * self.f, clear), self.v(b + nn * self.f, clear));
            self.m.add_triangle(ia, ib, ob);
            self.m.add_triangle(ia, ob, oa);
            // round off the corner gap to the next outside edge
            let (b2, out2) = ring[(i + 1) % n];
            let c2 = ring[(i + 2) % n].0;
            if out2 && (c2 - b2).length() > 1e-4 {
                let e2 = c2 - b2;
                let mut n2 = Vec2::new(-e2.y, e2.x).normalized();
                if n2.dot(b2 + e2 * 0.5 - c) < 0.0 {
                    n2 = -n2;
                }
                let o2 = self.v(b + n2 * self.f, clear);
                self.m.add_triangle(ib, ob, o2);
            }
        }
    }
}

/// Half turn round `c` (radius `r`) from `c + a*r` to `c - a*r`, bulging toward `bulge`;
/// the inner points only
fn half(c: Pos2, r: f32, a: Vec2, bulge: Vec2, steps: usize) -> Vec<Pos2> {
    (1..steps).map(|i| {
        let t = std::f32::consts::PI * i as f32 / steps as f32;
        c + a * (r * t.cos()) + bulge * (r * t.sin())
    }).collect()
}

/// `pts`: the stored arrow on screen [tail, tip, wing, tip, wing]; `width`: line width (px)
pub(crate) fn arrow(pts: &[Pos2], width: f32, tip_style: u8, cap: u8, col: Color32, ppp: f32) -> Mesh {
    let mut b = B { m: Mesh::default(), col, f: 1.0 / ppp.max(0.1) };
    if pts.len() < 5 || col.a() == 0 || width <= 0.0 {
        return b.m;
    }
    let (tail, tip) = (pts[0], pts[1]);
    let len = (tip - tail).length();
    if len < 1e-3 {
        return b.m;
    }
    let u = (tip - tail) / len;
    let n = Vec2::new(-u.y, u.x);
    let hw = width / 2.0;
    // the wing on the +n side first
    let (w1, w2) = if (pts[2] - tip).dot(n) >= 0.0 { (pts[2], pts[4]) } else { (pts[4], pts[2]) };
    let mid = w1 + (w2 - w1) * 0.5;
    let h = (tip - mid).dot(u).max(1e-3);
    let s = ((w1 - w2).length() / 2.0).max(hw * 1.2);
    let segs = 10;

    // The shaft's tip end, from its -n edge round to its +n edge (each point: is that edge outside)
    let mut end: Vec<(Pos2, bool)> = Vec::new();
    // a pointed back (apex `a`, sides opening forward at `tb` = half-width per length):
    // the shaft runs on until the head is as wide as it, ending in a notch round the apex
    let notch = |a: Pos2, tb: f32| -> (Pos2, Pos2) {
        let d = hw / tb.max(1e-3);
        (a + u * d - n * hw, a + u * d + n * hw)
    };
    match tip_style {
        1 => {
            // Triangle: the shaft stops flat at its base
            let (bm, bp) = (mid - n * hw, mid + n * hw);
            end.push((bm, false));
            end.push((bp, true));
            let c = tip + (mid - tip) * (2.0 / 3.0);
            b.piece(c, &[(tip, true), (w1, true), (bp, false), (bm, true), (w2, true)]);
        }
        2 => {
            // Reversed triangle: base across the tip, apex pointing back at the shaft
            let (b1, b2, a) = (tip + n * s, tip - n * s, tip - u * h);
            let (qm, qp) = notch(a, s / h);
            end.push((qm, false));
            end.push((a, false));
            end.push((qp, true));
            let c = tip - u * (h / 3.0);
            b.piece(c, &[(a, false), (qp, true), (b1, true), (b2, true), (qm, false)]);
        }
        3 => {
            // Circle sitting on the tip
            let r = s.min(h).max(hw * 1.3);
            let c = tip - u * r;
            // round the circle by angle from its back (t = 0, facing the shaft)
            let at = |t: f32| c - u * (r * t.cos()) + n * (r * t.sin());
            let ti = (hw / r).min(1.0).asin();
            let (im, ip) = (at(-ti), at(ti));
            end.push((im, false));
            for i in 1..16 {
                end.push((at(-ti + 2.0 * ti * i as f32 / 16.0), false));
            }
            end.push((ip, true));
            let mut ring: Vec<(Pos2, bool)> = Vec::new();
            // far side from +n round the front to -n, then the shaft-side arc back (shared)
            for i in 0..32 {
                ring.push((at(ti + (std::f32::consts::TAU - 2.0 * ti) * i as f32 / 32.0), true));
            }
            for i in 0..16 {
                ring.push((at(-ti + 2.0 * ti * i as f32 / 16.0), false));
            }
            b.piece(c, &ring);
        }
        4 => {
            // Diamond: front on the tip, back point toward the shaft
            let (a, l, r) = (tip - u * h, tip - u * (h / 2.0) + n * (s * 0.8), tip - u * (h / 2.0) - n * (s * 0.8));
            let (qm, qp) = notch(a, (s * 0.8) / (h / 2.0));
            end.push((qm, false));
            end.push((a, false));
            end.push((qp, true));
            let c = tip - u * (h / 2.0);
            b.piece(c, &[(a, false), (qp, true), (l, true), (tip, true), (r, true), (qm, false)]);
        }
        _ => {
            // Line: an open chevron of two thick wings meeting in a mitred point
            let d1 = (w1 - tip).normalized();
            let sin_a = d1.dot(n).abs().max(0.05);
            let to = tip + u * (hw / sin_a);
            let nt = tip - u * (hw / sin_a);
            // the shaft's edges meet the wings' inside edges
            let t = |w: Pos2| {
                let d = (w - tip).normalized();
                nt + d * (hw / d.dot(n).abs().max(0.05)).min((w - tip).length())
            };
            for (w, side) in [(w1, 1.0f32), (w2, -1.0f32)] {
                let d = (w - tip).normalized();
                // the wing's outside edge faces away from the axis
                let o = Vec2::new(-d.y, d.x) * if Vec2::new(-d.y, d.x).dot(n) * side >= 0.0 { 1.0 } else { -1.0 };
                let (wo, wi) = (w + o * hw, w - o * hw);
                let mut ring: Vec<(Pos2, bool)> = vec![(to, true), (wo, true)];
                if cap == 1 {
                    ring.push((wo + d * hw, true));
                    ring.push((wi + d * hw, true));
                } else {
                    for p in half(w, hw, o, d, segs) {
                        ring.push((p, true));
                    }
                }
                ring.push((wi, true));
                // the part of the inside edge along the shaft is shared with it
                ring.push((t(w), false));
                ring.push((nt, false));
                let c = tip + (w - tip) * 0.5;
                b.piece(c, &ring);
            }
            end.push((t(w2), false));
            end.push((nt, false));
            end.push((t(w1), true));
        }
    }

    // Shaft: tail cap, -n edge, the tip end, +n edge (fanned from the tail, which sees it all)
    let mut ring: Vec<(Pos2, bool)> = Vec::new();
    ring.push((tail - n * hw, true));
    // never let the end reach back past the tail
    for (p, o) in end {
        let back = (p - tail).dot(u);
        ring.push((if back < 0.0 { p - u * back } else { p }, o));
    }
    ring.push((tail + n * hw, true));
    if cap == 1 {
        ring.push((tail + n * hw - u * hw, true));
        ring.push((tail - n * hw - u * hw, true));
    } else {
        for p in half(tail, hw, n, -u, segs) {
            ring.push((p, true));
        }
    }
    b.piece(tail, &ring);
    b.m
}
