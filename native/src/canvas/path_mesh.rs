//! Pen paths drawn as one shape each, like a design app does: the line's outline and the fill
//! are first merged into plain non-overlapping outlines (i_overlay), then cut into triangles
//! (i_triangle). Nothing is painted twice, so a see-through path shows no darker spots at
//! corners or where it crosses itself. Edge smoothing is a 1px fade around the outside only.

use crate::*;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::mesh::float::stroke::offset::StrokeOffset;
use i_overlay::mesh::float::style::{LineCap, LineJoin, StrokeStyle};
use i_triangle::float::triangulatable::Triangulatable;

type Shapes = Vec<Vec<Vec<[f32; 2]>>>;

/// The path as screen points: each curve cut into pieces about 3 px long (a straight side
/// stays one piece); a closed path ends on its first point, like `Stroke::points`
pub(crate) fn flatten(s: &Stroke, map: &impl Fn(Pt) -> Pos2) -> Vec<Pos2> {
    let mut out = Vec::new();
    let segs = s.drawn_segments();
    for &(a, b, c, d) in &segs {
        let (sa, sb, sc, sd) = (map(a), map(b), map(c), map(d));
        let straight = b == a && c == d;
        let len = sa.distance(sb) + sb.distance(sc) + sc.distance(sd);
        let steps = if straight { 1 } else { ((len / 3.0).ceil() as usize).clamp(4, 400) };
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let w = |p: Pos2, q: Pos2| p + (q - p) * t;
            let (ab, bc, cd) = (w(sa, sb), w(sb, sc), w(sc, sd));
            out.push(w(w(ab, bc), w(bc, cd)));
        }
    }
    match (s.closed, segs.first(), segs.last()) {
        (true, Some(f), _) => out.push(map(f.0)),
        (false, _, Some(l)) => out.push(map(l.3)),
        _ => {}
    }
    out
}

fn clean(pts: &[Pos2]) -> Vec<[f32; 2]> {
    let mut v: Vec<[f32; 2]> = Vec::with_capacity(pts.len());
    for p in pts {
        if v.last().map_or(true, |l| (l[0] - p.x).abs() + (l[1] - p.y).abs() > 0.01) {
            v.push([p.x, p.y]);
        }
    }
    v
}

/// The inside of a closed path (`pts` without the repeated first point); where it crosses
/// itself, every looped part is filled (non-zero rule)
pub(crate) fn fill(pts: &[Pos2], col: Color32, ppp: f32) -> egui::Mesh {
    let v = clean(pts);
    if v.len() < 3 || col.a() == 0 {
        return egui::Mesh::default();
    }
    let shapes = vec![v].simplify_shape(FillRule::NonZero);
    mesh(&shapes, col, ppp)
}

/// The path's line, `width` wide, as one shape (sharp corners, round ends on an open path)
pub(crate) fn line(pts: &[Pos2], width: f32, col: Color32, closed: bool, ppp: f32) -> egui::Mesh {
    let mut v = clean(pts);
    if closed && v.len() > 2 && v.first() == v.last() {
        v.pop();
    }
    if v.len() < 2 || col.a() == 0 {
        return egui::Mesh::default();
    }
    let f = 1.0 / ppp.max(0.1);
    // the edge fade adds about half a pixel each side: take it off the width
    let w = (width - f).max(0.1);
    // too thin to draw: fade the colour instead of shrinking under a pixel (as stroke_mesh)
    let col = if width < 1.3 { col.gamma_multiply((width / 1.3).max(0.2)) } else { col };
    let style = StrokeStyle::new(w)
        .line_join(LineJoin::Miter(0.5))
        .start_cap(LineCap::Round(0.25))
        .end_cap(LineCap::Round(0.25));
    let shapes = v.stroke(style, closed);
    mesh(&shapes, col, ppp)
}

/// Triangles for the shapes plus a 1px fade outward along every outline
fn mesh(shapes: &Shapes, col: Color32, ppp: f32) -> egui::Mesh {
    let mut m = egui::Mesh::default();
    if shapes.is_empty() {
        return m;
    }
    let tri = shapes.triangulate().to_triangulation::<u32>();
    for q in &tri.points {
        m.colored_vertex(pos2(q[0], q[1]), col);
    }
    m.indices = tri.indices;
    let f = 1.0 / ppp.max(0.1);
    let area = |c: &[[f32; 2]]| -> f32 { (0..c.len()).map(|i| { let (a, b) = (c[i], c[(i + 1) % c.len()]); a[0] * b[1] - b[0] * a[1] }).sum() };
    for shape in shapes {
        let Some(outer) = shape.first() else { continue };
        // the filled side is the same side of every outline of one shape (holes run the
        // other way round); the fade goes to the other side
        let sg = area(outer).signum();
        for c in shape {
            let n = c.len();
            if n < 3 {
                continue;
            }
            let p = |i: usize| pos2(c[i % n][0], c[i % n][1]);
            let out = |i: usize| {
                let d = (p(i + 1) - p(i)).normalized();
                vec2(d.y, -d.x) * sg
            };
            let base = m.vertices.len() as u32;
            for i in 0..n {
                let (a, b) = (out(i + n - 1), out(i));
                let mut nv = a + b;
                let l = nv.length();
                nv = if l > 1e-3 { nv / l } else { b };
                // keep the fade 1px wide across the corner (capped at sharp ones)
                let c2 = nv.dot(b).max(0.35);
                m.colored_vertex(p(i), col);
                m.colored_vertex(p(i) + nv * (f / c2), Color32::TRANSPARENT);
            }
            for i in 0..n as u32 {
                let j = (i + 1) % n as u32;
                let (a0, a1, b0, b1) = (base + 2 * i, base + 2 * i + 1, base + 2 * j, base + 2 * j + 1);
                m.add_triangle(a0, a1, b1);
                m.add_triangle(a0, b1, b0);
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    /// how many solid triangles cover the most-covered sample spot
    fn worst(m: &egui::Mesh) -> usize {
        let mut worst = 0;
        let (mut y, step) = (-50.0_f32, 0.37_f32);
        while y < 250.0 {
            let mut x = -50.0_f32;
            while x < 250.0 {
                let mut c = 0;
                for t in m.indices.chunks(3) {
                    let [a, b, d] = [t[0], t[1], t[2]].map(|i| m.vertices[i as usize]);
                    if a.color.a() == 0 || b.color.a() == 0 || d.color.a() == 0 {
                        continue;
                    }
                    let cr = |p: Pos2, q: Pos2| (q.x - p.x) * (y - p.y) - (q.y - p.y) * (x - p.x);
                    let (s1, s2, s3) = (cr(a.pos, b.pos), cr(b.pos, d.pos), cr(d.pos, a.pos));
                    if (s1 > 1e-3 && s2 > 1e-3 && s3 > 1e-3) || (s1 < -1e-3 && s2 < -1e-3 && s3 < -1e-3) {
                        c += 1;
                    }
                }
                worst = worst.max(c);
                x += step;
            }
            y += step;
        }
        worst
    }
    #[test]
    fn crossing_path_paints_once() {
        let zig = [pos2(0.0, 0.0), pos2(200.0, 150.0), pos2(200.0, 0.0), pos2(0.0, 150.0), pos2(100.0, 10.0)];
        for (closed, w) in [(false, 6.0), (true, 6.0), (true, 20.0)] {
            assert_eq!(worst(&line(&zig, w, Color32::WHITE, closed, 1.25)), 1);
        }
        assert_eq!(worst(&fill(&zig, Color32::WHITE, 1.25)), 1);
    }
}
