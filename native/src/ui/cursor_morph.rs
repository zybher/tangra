//! Path morphing between pointer shapes (cursor.rs): when the pointer changes (arrow → resize,
//! arrow → crosshair, ...) the old icon's strokes slide into the new one's instead of swapping.
//! Every stroke of both icons is resampled to the same number of points, strokes are paired by
//! position (each new stroke takes the nearest old one; spare old strokes fold into their
//! nearest new one), and the points blend with an ease-out. Filled shapes (the arrow's body)
//! fade their fill as they open into lines, and back.
use crate::*;

/// Points per stroke while morphing
const M: usize = 32;
/// Morph time (s), scaled by the app's animation speed
pub(crate) const DUR: f64 = 0.2;

#[derive(Clone, Copy)]
pub(crate) struct Pose {
    pub name: &'static str,
    pub hot: [f32; 2],
    pub px: f32,
}

struct Stroke {
    pts: Vec<Pos2>,
    filled: bool,
}

fn resample(v: &[Pos2]) -> Vec<Pos2> {
    let mut acc = vec![0.0f32];
    for w in v.windows(2) {
        acc.push(acc.last().unwrap() + (w[1] - w[0]).length());
    }
    let total = *acc.last().unwrap();
    if v.len() < 2 || total < 1e-4 {
        return vec![v.first().copied().unwrap_or(Pos2::ZERO); M];
    }
    let mut out = Vec::with_capacity(M);
    let mut s = 0;
    for i in 0..M {
        let d = total * i as f32 / (M - 1) as f32;
        while s + 2 < acc.len() && acc[s + 1] < d {
            s += 1;
        }
        let seg = (acc[s + 1] - acc[s]).max(1e-6);
        out.push(v[s] + (v[s + 1] - v[s]) * ((d - acc[s]) / seg).clamp(0.0, 1.0));
    }
    out
}

/// The icon's strokes on screen, placed like `draw_cursor` would
fn strokes(pose: &Pose, at: Pos2, rot: f32) -> Vec<Stroke> {
    let k = pose.px / 24.0;
    let (sn, cs) = rot.sin_cos();
    crate::ui::svg_icons::subpaths(pose.name)
        .into_iter()
        .filter(|(v, _, _)| v.len() > 1)
        .map(|(v, closed, filled)| {
            let mut q: Vec<Pos2> = v
                .iter()
                .map(|p| {
                    let (x, y) = ((p[0] - pose.hot[0]) * k, (p[1] - pose.hot[1]) * k);
                    pos2(at.x + x * cs - y * sn, at.y + x * sn + y * cs)
                })
                .collect();
            if closed {
                q.push(q[0]);
            }
            Stroke { pts: resample(&q), filled }
        })
        .collect()
}

fn centroid(s: &Stroke) -> Pos2 {
    let sum = s.pts.iter().fold(egui::Vec2::ZERO, |a, p| a + p.to_vec2());
    (sum / s.pts.len() as f32).to_pos2()
}

/// (old, new) stroke pairs: every new stroke gets a partner, spare old ones fold into the nearest
fn pair(a: &[Stroke], b: &[Stroke]) -> Vec<(usize, usize)> {
    let (ca, cb): (Vec<Pos2>, Vec<Pos2>) = (a.iter().map(centroid).collect(), b.iter().map(centroid).collect());
    let near = |q: Pos2, cs: &[Pos2], skip: &[bool]| (0..cs.len()).filter(|i| !skip[*i]).min_by(|x, y| (cs[*x] - q).length().total_cmp(&(cs[*y] - q).length()));
    let mut used = vec![false; a.len()];
    let none = vec![false; a.len()];
    let mut out = Vec::new();
    for (j, q) in cb.iter().enumerate() {
        let i = near(*q, &ca, &used).or_else(|| near(*q, &ca, &none)).unwrap();
        used[i] = true;
        out.push((i, j));
    }
    for i in 0..a.len() {
        if !used[i] {
            let j = near(ca[i], &cb, &vec![false; b.len()]).unwrap();
            out.push((i, j));
        }
    }
    out
}

fn fan(p: &egui::Painter, v: &[Pos2], col: Color32) {
    let bb = Rect::from_points(v);
    // A tiny shape (a dot): a disc, since slivers this small smear into noise
    if bb.width().max(bb.height()) < 5.0 {
        p.circle_filled(bb.center(), bb.width().max(bb.height()) / 2.0, col);
        return;
    }
    // Same triangles as the still pointer (concave outlines like a hand fill right too), as
    // one plain mesh: mid-morph the outline can fold flat, and smoothed edges then spike
    crate::ui::svg_icons::fan_mesh(p, v, col);
}

/// A stroke as separate segments with round joints (a folded polyline's mitred joins spike too)
fn line(p: &egui::Painter, v: &[Pos2], st: egui::Stroke) {
    for w in v.windows(2) {
        if (w[1] - w[0]).length() > 0.05 {
            p.line_segment([w[0], w[1]], st);
        }
    }
    for q in v {
        p.circle_filled(*q, st.width / 2.0, st.color);
    }
}

/// Draws `from` turning into `to`, `t` 0..1 (already eased)
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(p: &egui::Painter, from: &Pose, to: &Pose, at: Pos2, rot: f32, t: f32, fill: Color32, ink: Color32) {
    let (a, b) = (strokes(from, at, rot), strokes(to, at, rot));
    if a.is_empty() || b.is_empty() {
        return crate::ui::svg_icons::draw_cursor(p, to.name, at, to.hot, to.px, rot, fill, ink);
    }
    // Blend each pair (an open stroke runs whichever way lines up best)
    let shapes: Vec<(Vec<Pos2>, f32)> = pair(&a, &b)
        .into_iter()
        .map(|(i, j)| {
            let (sa, sb) = (&a[i], &b[j]);
            let fwd: f32 = sa.pts.iter().zip(&sb.pts).map(|(x, y)| (*x - *y).length()).sum();
            let rev: f32 = sa.pts.iter().zip(sb.pts.iter().rev()).map(|(x, y)| (*x - *y).length()).sum();
            let tb: Vec<Pos2> = if rev < fwd { sb.pts.iter().rev().copied().collect() } else { sb.pts.clone() };
            let pts = sa.pts.iter().zip(&tb).map(|(x, y)| *x + (*y - *x) * t).collect();
            // The old fill fades out over the first half and the new one in over the second, so
            // the folded in-between outline is never filled (it would flash as a white blob)
            let filled = if sa.filled { (1.0 - 2.0 * t).max(0.0) } else { 0.0 } + if sb.filled { (2.0 * t - 1.0).max(0.0) } else { 0.0 };
            (pts, filled)
        })
        .collect();
    let w = (1.75 * (from.px + (to.px - from.px) * t) / 24.0).max(1.2);
    // Shadow, dark edge, then the body (as draw_cursor)
    for (off, col, extra) in [(vec2(0.6, 1.0), Color32::from_black_alpha(50), 3.6), (egui::Vec2::ZERO, ink, 2.6)] {
        for (v, f) in &shapes {
            let v: Vec<Pos2> = v.iter().map(|q| *q + off).collect();
            if *f > 0.01 {
                fan(p, &v, col.gamma_multiply(*f));
            }
            line(p, &v, egui::Stroke::new(w + extra, col));
        }
    }
    for (v, f) in &shapes {
        if *f > 0.01 {
            fan(p, v, fill.gamma_multiply(*f));
        }
        let col = fill.gamma_multiply(1.0 - f.clamp(0.0, 1.0));
        if col.a() > 0 {
            line(p, v, egui::Stroke::new(w, col));
        }
    }
}
