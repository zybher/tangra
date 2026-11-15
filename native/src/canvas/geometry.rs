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
    let width = width.max(0.6);
    // Rectangle with rounded corners
    if let (Some(rad), Some(4)) = (s.radius.filter(|r| *r > 0.0), s.closed_len()) {
        let rr = Rect::from_points(&pts[..4]);
        let rad = (rad * zk).min(rr.width().min(rr.height()) / 2.0);
        if s.fill {
            p.rect_filled(rr, rad, rgba(r, g, b, (s.fill_alpha() * 255.0) as u8));
        }
        if !s.no_stroke {
            p.rect_stroke(rr, rad, EStroke::new(width, col));
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
