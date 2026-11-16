//! Pure geometry: rect/segment tests, stroke hit-testing, text sizes, shape constraints.

use crate::*;

pub(crate) fn inside(p: Pt, x: f32, y: f32, w: f32, h: f32) -> bool {
    p.x >= x && p.x <= x + w && p.y >= y && p.y <= y + h
}

pub(crate) fn rects_intersect(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && a[0] + a[2] > b[0] && a[1] < b[1] + b[3] && a[1] + a[3] > b[1]
}

pub(crate) fn seg_dist(p: Pt, a: Pt, b: Pt) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((a.x + dx * t - p.x).powi(2) + (a.y + dy * t - p.y).powi(2)).sqrt()
}

pub(crate) fn stroke_hit(s: &Stroke, p: Pt, r: f32) -> bool {
    match s.points.len() {
        0 => false,
        1 => seg_dist(p, s.points[0], s.points[0]) < r,
        _ => s.points.windows(2).any(|w| seg_dist(p, w[0], w[1]) < r),
    }
}

pub(crate) static CTX: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

/// Real size of a piece of text at `fs`, measured with the font (falls back to an estimate).
/// Marquee test on the line itself: some segment of the stroke enters the box (widened by
/// half the line width), not just its bounding box
pub(crate) fn stroke_in_box(s: &Stroke, b: [f32; 4]) -> bool {
    let hw = s.width / 2.0;
    let (x0, y0, x1, y1) = (b[0] - hw, b[1] - hw, b[0] + b[2] + hw, b[1] + b[3] + hw);
    let ins = |p: &Pt| p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1;
    // Liang–Barsky clip: does segment a→b cross the box?
    let crosses = |a: &Pt, c: &Pt| {
        let (dx, dy) = (c.x - a.x, c.y - a.y);
        let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
        for (pp, q) in [(-dx, a.x - x0), (dx, x1 - a.x), (-dy, a.y - y0), (dy, y1 - a.y)] {
            if pp == 0.0 {
                if q < 0.0 {
                    return false;
                }
            } else {
                let r = q / pp;
                if pp < 0.0 {
                    t0 = t0.max(r);
                } else {
                    t1 = t1.min(r);
                }
                if t0 > t1 {
                    return false;
                }
            }
        }
        true
    };
    match s.points.as_slice() {
        [] => false,
        [p] => ins(p),
        pts => pts.windows(2).any(|w| crosses(&w[0], &w[1])),
    }
}

pub(crate) fn text_wh(text: &str, fs: f32) -> (f32, f32) {
    let fs = fs.max(8.0);
    match CTX.get() {
        Some(ctx) => {
            let s = ctx.fonts(|f| f.layout_no_wrap(text.to_owned(), FontId::proportional(fs), Color32::WHITE).size());
            (s.x.max(12.0), s.y.max(fs))
        }
        None => ((text.chars().count().max(1) as f32 * fs * 0.6).max(20.0), (fs * 1.3).max(14.0)),
    }
}

pub(crate) fn label_hit(l: &Label, p: Pt, pad: f32) -> bool {
    let (w, h) = text_wh(&l.text, l.font_size);
    inside(p, l.x - pad, l.y - pad, w + pad * 2.0, h + pad * 2.0)
}

pub(crate) fn paint_stroke(p: &egui::Painter, s: &Stroke, map: impl Fn(Pt) -> Pos2, width: f32) {
    if s.points.len() < 2 {
        return;
    }
    let [r, g, b] = hex_rgb(&s.color);
    let o = s.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let col = rgba(r, g, b, (o * 255.0) as u8);
    let pts: Vec<Pos2> = s.points.iter().map(|q| map(*q)).collect();
    let zk = width / s.width.max(0.01);
    // No floor: a line keeps its size relative to the board at any zoom (too thin to draw
    // just fades out instead of staying 1px wide)
    let width = width.max(0.0);
    // Rectangle with rounded corners
    if let (Some(rad), Some(4)) = (s.radius.filter(|r| *r > 0.0), s.closed_len()) {
        let rr = Rect::from_points(&pts[..4]);
        let rad = (rad * zk).min(rr.width().min(rr.height()) / 2.0);
        if s.fill {
            p.rect_filled(rr, rad, rgba(r, g, b, (s.fill_alpha() * 255.0) as u8));
        }
        if !s.no_stroke {
            let d = stroke_inset(s.stroke_pos, width);
            p.rect_stroke(rr.shrink(d), (rad - d).max(0.0), EStroke::new(width, col));
        }
        return;
    }
    if let (Some(n), true) = (s.closed_len(), s.fill) {
        let fc = rgba(r, g, b, (s.fill_alpha() * 255.0) as u8);
        p.add(Shape::convex_polygon(pts[..n].to_vec(), fc, EStroke::NONE));
        if s.no_stroke {
            return;
        }
    }
    // Rectangle / ellipse outline: one closed line with sharp joins (never smoothed: rounding
    // it like a pen line melted the corners and left the overlap points sticking out)
    if let Some(n) = s.closed_len() {
        let line = offset_closed(&pts[..n], stroke_inset(s.stroke_pos, width));
        p.add(Shape::closed_line(line, EStroke::new(width, col)));
        return;
    }
    // Arrow (shaft, then both wings out of the tip): separate round-capped segments,
    // a single polyline doubling back on itself tears apart at high zoom
    if pts.len() == 5 && s.points[1].x == s.points[3].x && s.points[1].y == s.points[3].y {
        let st = EStroke::new(width, col);
        for (u, v) in [(pts[0], pts[1]), (pts[1], pts[2]), (pts[1], pts[4])] {
            p.line_segment([u, v], st);
            if o >= 0.99 {
                p.circle_filled(u, width / 2.0, col);
                p.circle_filled(v, width / 2.0, col);
            }
        }
        return;
    }
    // Pen / highlighter: one smooth mesh (round caps, no seams, even when see-through)
    let line = super::stroke_mesh::smooth_line(&pts, width);
    p.add(super::stroke_mesh::stroke_mesh(&line, width, col));
}

/// Corner rounding (world units) for the accent outline around a shape: a rectangle's own
/// corner radius (square when it has none); None for anything without corners to match
pub(crate) fn outline_round(s: &Stroke) -> Option<f32> {
    match (s.closed_len(), s.radius) {
        (Some(4), r) => Some(r.unwrap_or(0.0).max(0.0)),
        (_, r) => r,
    }
}

/// How far a closed outline of `width` moves inward for its stroke position (inside / centre / outside)
pub(crate) fn stroke_inset(pos: u8, width: f32) -> f32 {
    match pos {
        1 => width / 2.0,
        2 => -width / 2.0,
        _ => 0.0,
    }
}

/// A closed polygon moved `d` inward (negative: outward), corners mitred
pub(crate) fn offset_closed(v: &[Pos2], d: f32) -> Vec<Pos2> {
    let n = v.len();
    if d == 0.0 || n < 3 {
        return v.to_vec();
    }
    let area: f32 = (0..n).map(|i| v[i].x * v[(i + 1) % n].y - v[(i + 1) % n].x * v[i].y).sum();
    let sg = area.signum();
    let norm = |a: Pos2, b: Pos2| {
        let e = b - a;
        let l = e.length().max(1e-6);
        vec2(-e.y, e.x) * (sg / l)
    };
    (0..n)
        .map(|i| {
            let (a, b, c) = (v[(i + n - 1) % n], v[i], v[(i + 1) % n]);
            let (n1, n2) = (norm(a, b), norm(b, c));
            b + (n1 + n2) * (d / (1.0 + n1.dot(n2)).max(0.3))
        })
        .collect()
}

/// Freehand line (pen / highlighter): not a closed shape, rounded rectangle or arrow
pub(crate) fn is_freehand(s: &Stroke) -> bool {
    let n = s.points.len();
    let arrow = n == 5 && s.points[1].x == s.points[3].x && s.points[1].y == s.points[3].y;
    n >= 2 && s.closed_len().is_none() && s.radius.is_none() && !arrow
}

/// Shift while dragging a shape: square / circle, or an arrow snapped to 45°.
pub(crate) fn constrain(tool: Tool, a: Pt, b: Pt) -> Pt {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    if tool == Tool::Arrow {
        let len = (dx * dx + dy * dy).sqrt();
        let step = std::f32::consts::FRAC_PI_4;
        let ang = (dy.atan2(dx) / step).round() * step;
        Pt { x: a.x + len * ang.cos(), y: a.y + len * ang.sin() }
    } else {
        let d = dx.abs().max(dy.abs());
        Pt { x: a.x + d * dx.signum(), y: a.y + d * dy.signum() }
    }
}
