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
    // rubbed-out spots / lasso cuts are cut out of whatever it paints (canvas/holes.rs)
    let holes = match super::holes::stroke_box(s) {
        Some(bx) if !s.erase.is_empty() || !s.erase_loops.is_empty() => super::holes::screen(&s.erase, &s.erase_loops, bx, &map),
        _ => Vec::new(),
    };
    super::holes::with_holes(p, &holes, || paint_stroke_plain(p, s, &map, width));
}

fn paint_stroke_plain(p: &egui::Painter, s: &Stroke, map: impl Fn(Pt) -> Pos2, width: f32) {
    if s.points.len() < 2 {
        return;
    }
    // Its looks glide to new values instead of snapping (canvas/presence.rs, audit #18-24)
    use super::presence::gf;
    let gid = egui::Id::new(("qd_sg", s.id.as_str()));
    let ctx = p.ctx();
    let [r, g, b] = hex_rgb(&s.color);
    let o = s.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let [r, g, b, a] = super::presence::gcol(ctx, gid, [r, g, b, (o * 255.0) as u8]).to_srgba_unmultiplied();
    let o = a as f32 / 255.0;
    // fill / outline turned on and off fade
    let fk = gf(ctx, gid.with("f"), if s.fill { 1.0 } else { 0.0 });
    let sk = gf(ctx, gid.with("s"), if s.no_stroke { 0.0 } else { 1.0 });
    let col = rgba(r, g, b, (o * sk * 255.0) as u8);
    let fill_col = rgba(r, g, b, (s.fill_alpha() * fk * 255.0) as u8);
    let pts: Vec<Pos2> = s.points.iter().map(|q| map(*q)).collect();
    let zk = width / s.width.max(0.01);
    // No floor: a line keeps its size relative to the board at any zoom (too thin to draw
    // just fades out instead of staying 1px wide)
    let width = (gf(ctx, gid.with("w"), s.width) * zk).max(0.0);
    // stroke position slides between inside / centre / outside
    let spk = gf(ctx, gid.with("sp"), stroke_inset(s.stroke_pos, 2.0));
    let inset = |w: f32| spk * w / 2.0;
    // Pen path: any shape, so its own fill (path_mesh.rs) and a plain line with sharp joins
    if !s.knots.is_empty() {
        // flattened on screen, finer the bigger it's drawn, so curves stay smooth when zoomed in
        let pts = super::path_mesh::flatten(s, &map);
        let n = pts.len();
        if s.closed && n > 3 {
            if fk > 0.001 {
                p.add(super::path_mesh::fill(&pts[..n - 1], fill_col, ctx.pixels_per_point()));
            }
            if sk > 0.001 {
                p.add(super::path_mesh::line(&offset_closed(&pts[..n - 1], inset(width)), width, col, true, ctx.pixels_per_point()));
            }
        } else {
            p.add(super::path_mesh::line(&pts, width, col, false, ctx.pixels_per_point()));
        }
        return;
    }
    // Rectangle with rounded corners
    if let (Some(rad), Some(4)) = (s.radius.filter(|r| *r > 0.0), s.closed_len()) {
        let rr = Rect::from_points(&pts[..4]);
        let rad = (gf(ctx, gid.with("r"), rad) * zk).min(rr.width().min(rr.height()) / 2.0);
        if fk > 0.001 {
            p.rect_filled(rr, rad, fill_col);
        }
        if sk > 0.001 {
            let width = if s.stroke_pos == 1 { width.min(rr.size().min_elem().max(0.5)) } else { width };
            // Own mesh: egui's rounded stroke kept square outer corners once the stroke was
            // thicker than the radius (round_rect_mesh.rs)
            p.add(super::round_rect_mesh::stroke(rr, rad, inset(width), width, col));
        }
        return;
    }
    if let (Some(n), true) = (s.closed_len(), fk > 0.001) {
        // Ellipses: own mesh, egui's feathering spikes out of a thin ellipse's ends (ellipse_mesh.rs)
        if n > 4 {
            p.add(super::ellipse_mesh::fill(&pts[..n], fill_col));
        } else {
            p.add(Shape::convex_polygon(pts[..n].to_vec(), fill_col, EStroke::NONE));
        }
    }
    if s.closed_len().is_some() && sk <= 0.001 {
        return;
    }
    // Rectangle / ellipse outline: one closed line with sharp joins (never smoothed: rounding
    // it like a pen line melted the corners and left the overlap points sticking out)
    if let Some(n) = s.closed_len() {
        // An inside stroke is never thicker than the shape itself
        let width = if s.stroke_pos == 1 { width.min(Rect::from_points(&pts[..n]).size().min_elem().max(0.5)) } else { width };
        if n > 4 {
            p.add(super::ellipse_mesh::stroke(&pts[..n], inset(width), width, col));
            return;
        }
        let line = offset_closed(&pts[..n], inset(width));
        p.add(Shape::closed_line(line, EStroke::new(width, col)));
        return;
    }
    // Arrow: one mesh whose pieces never overlap, so it stays one shape when see-through
    // (arrow_mesh.rs, with its head style and line ends)
    if pts.len() == 5 && s.points[1].x == s.points[3].x && s.points[1].y == s.points[3].y {
        let _ = o;
        p.add(super::arrow_mesh::arrow(&pts, width, s.tip, s.cap, col, ctx.pixels_per_point()));
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

/// A closed polygon moved `d` inward (negative: outward, corners mitred). Inward it shrinks
/// toward its box's centre instead, at most to a line, so a shape thinner than its stroke
/// never turns inside out (a mitred inset of a thin ellipse shot spikes out of its ends)
pub(crate) fn offset_closed(v: &[Pos2], d: f32) -> Vec<Pos2> {
    let n = v.len();
    if d == 0.0 || n < 3 {
        return v.to_vec();
    }
    if d > 0.0 {
        let b = Rect::from_points(v);
        let (rx, ry) = (b.width() / 2.0, b.height() / 2.0);
        let sx = if rx > 1e-6 { (rx - d).max(0.0) / rx } else { 0.0 };
        let sy = if ry > 1e-6 { (ry - d).max(0.0) / ry } else { 0.0 };
        let c = b.center();
        return v.iter().map(|p| pos2(c.x + (p.x - c.x) * sx, c.y + (p.y - c.y) * sy)).collect();
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
