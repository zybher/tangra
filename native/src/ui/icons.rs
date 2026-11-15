//! Small line icons drawn on a 16-unit grid (like an SVG viewBox 0 0 16 16), snapped to
//! whole pixels so strokes stay crisp.
use crate::Kind;
use eframe::egui::{self, epaint::CubicBezierShape, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Stroke};
use Stroke as EStroke;

const SW: f32 = 1.25;

/// Grid point (0..16) around `c`, scaled so 16 units = `size` px
fn g(c: Pos2, size: f32, x: f32, y: f32) -> Pos2 {
    let k = size / 16.0;
    c + vec2((x - 8.0) * k, (y - 8.0) * k)
}

fn snap(p: &egui::Painter, c: Pos2) -> Pos2 {
    p.round_pos_to_pixels(c)
}

/// Eye: almond outline + round pupil; a diagonal slash through it when `off`
pub(crate) fn eye(p: &egui::Painter, c: Pos2, size: f32, col: Color32, off: bool) {
    let c = snap(p, c);
    let st = Stroke::new(SW, col);
    let q = |x, y| g(c, size, x, y);
    let curve = |a: [f32; 8]| CubicBezierShape::from_points_stroke([q(a[0], a[1]), q(a[2], a[3]), q(a[4], a[5]), q(a[6], a[7])], false, Color32::TRANSPARENT, st);
    p.add(curve([1.5, 8.0, 3.6, 4.2, 12.4, 4.2, 14.5, 8.0]));
    p.add(curve([1.5, 8.0, 3.6, 11.8, 12.4, 11.8, 14.5, 8.0]));
    p.circle_stroke(c, 2.1 * size / 16.0, st);
    if off {
        p.line_segment([q(2.5, 13.5), q(13.5, 2.5)], st);
    }
}

/// Padlock: rounded body + shackle; open (right leg lifted off the body) when `!locked`
pub(crate) fn lock(p: &egui::Painter, c: Pos2, size: f32, col: Color32, locked: bool) {
    let c = snap(p, c);
    let st = Stroke::new(SW, col);
    let q = |x, y| g(c, size, x, y);
    let k = size / 16.0;
    let body = Rect::from_min_max(q(3.5, 7.5), q(12.5, 14.0));
    p.rect_stroke(body, 2.0 * k, st);
    // Shackle: two legs and a half circle on top (centre 8, radius 2.75)
    let (lx, rx, top) = (5.25, 10.75, 4.75);
    let arc: Vec<Pos2> = (0..=16)
        .map(|i| {
            let t = std::f32::consts::PI * (1.0 + i as f32 / 16.0);
            q(8.0 + 2.75 * t.cos(), top + 2.75 * t.sin())
        })
        .collect();
    p.add(egui::Shape::line(arc, st));
    p.line_segment([q(lx, top), q(lx, 7.5)], st);
    p.line_segment([q(rx, top), q(rx, if locked { 7.5 } else { 5.6 })], st);
    let _ = pos2;
}

/// The small per-kind glyph used in outline rows (and the drag ghost)
pub(crate) fn kind_icon(p: &egui::Painter, k: Kind, ic: Pos2, icol: Color32) {
    match k {
        Kind::Group => {
            for t in [-2.0_f32, 2.0] {
                p.line_segment([ic + vec2(-5.0, t), ic + vec2(5.0, t)], EStroke::new(1.1_f32, icol));
                p.line_segment([ic + vec2(t, -5.0), ic + vec2(t, 5.0)], EStroke::new(1.1_f32, icol));
            }
        }
        Kind::Image => {
            let b = Rect::from_center_size(ic, vec2(10.0, 9.0));
            p.rect_stroke(b, 1.5, EStroke::new(1.1_f32, icol));
            p.circle_filled(b.min + vec2(3.0, 3.0), 1.2, icol);
        }
        Kind::Text => {
            p.text(ic, Align2::CENTER_CENTER, "T", FontId::proportional(11.0), icol);
        }
        Kind::Note => {
            p.rect_filled(Rect::from_center_size(ic, vec2(9.0, 9.0)), 2.0, icol.gamma_multiply(0.7));
        }
        Kind::Link => {
            p.line_segment([ic + vec2(-4.0, 4.0), ic + vec2(4.0, -4.0)], EStroke::new(1.2_f32, icol));
            p.line_segment([ic + vec2(0.0, -4.0), ic + vec2(4.0, -4.0)], EStroke::new(1.2_f32, icol));
            p.line_segment([ic + vec2(4.0, -4.0), ic + vec2(4.0, 0.0)], EStroke::new(1.2_f32, icol));
        }
        Kind::Stroke => {
            p.circle_stroke(ic, 4.5, EStroke::new(1.1_f32, icol));
        }
    }
}

/// Grid rect (x0, y0, x1, y1 on the 16 grid); `t` swaps x and y (vertical variants)
fn grid_rect(c: Pos2, size: f32, t: bool, a: [f32; 4]) -> Rect {
    let [x0, y0, x1, y1] = if t { [a[1], a[0], a[3], a[2]] } else { a };
    Rect::from_min_max(g(c, size, x0, y0), g(c, size, x1, y1))
}

/// Align icons: an edge (or centre) line with a long and a short bar against it.
/// `how`: 0 left, 1 centre, 2 right, 3 top, 4 middle, 5 bottom
pub(crate) fn align(p: &egui::Painter, c: Pos2, size: f32, col: Color32, how: u8) {
    let c = snap(p, c);
    let t = how >= 3;
    let side = how % 3;
    let k = size / 16.0;
    let line = match side {
        0 => [2.0, 1.5, 3.25, 14.5],
        1 => [7.4, 1.5, 8.6, 14.5],
        _ => [12.75, 1.5, 14.0, 14.5],
    };
    p.rect_filled(grid_rect(c, size, t, line), 0.0, col);
    let bars: [[f32; 4]; 2] = match side {
        0 => [[4.75, 3.5, 13.5, 6.75], [4.75, 9.25, 10.0, 12.5]],
        1 => [[3.0, 3.5, 13.0, 6.75], [5.0, 9.25, 11.0, 12.5]],
        _ => [[2.5, 3.5, 11.25, 6.75], [6.0, 9.25, 11.25, 12.5]],
    };
    for b in bars {
        p.rect_filled(grid_rect(c, size, t, b), 1.0 * k, col);
    }
}

/// Distribute: two edge lines with an object between them
pub(crate) fn distribute(p: &egui::Painter, c: Pos2, size: f32, col: Color32, horizontal: bool) {
    let c = snap(p, c);
    let t = !horizontal;
    let k = size / 16.0;
    p.rect_filled(grid_rect(c, size, t, [1.5, 2.0, 2.75, 14.0]), 0.0, col);
    p.rect_filled(grid_rect(c, size, t, [13.25, 2.0, 14.5, 14.0]), 0.0, col);
    p.rect_filled(grid_rect(c, size, t, [5.75, 4.0, 10.25, 12.0]), 1.0 * k, col);
}

/// Opacity: a small checkerboard square
pub(crate) fn opacity(p: &egui::Painter, c: Pos2, size: f32, col: Color32) {
    let c = snap(p, c);
    let k = size / 16.0;
    let outer = grid_rect(c, size, false, [2.5, 2.5, 13.5, 13.5]);
    p.rect_stroke(outer, 1.5 * k, Stroke::new(SW, col));
    for (x, y) in [(0, 0), (1, 1), (2, 0), (0, 2), (2, 2), (1, 3), (3, 1), (3, 3)] {
        let (x0, y0) = (4.0 + x as f32 * 2.0, 4.0 + y as f32 * 2.0);
        p.rect_filled(grid_rect(c, size, false, [x0, y0, x0 + 2.0, y0 + 2.0]), 0.0, col.gamma_multiply(0.6));
    }
}

/// Adjust: two slider lines with knobs
pub(crate) fn adjust(p: &egui::Painter, c: Pos2, size: f32, col: Color32) {
    let k = size / 16.0;
    let st = Stroke::new(SW, col);
    for (y, kx) in [(-3.0_f32, 2.5_f32), (3.0, -2.5)] {
        p.line_segment([c + vec2(-6.0 * k, y * k), c + vec2(6.0 * k, y * k)], st);
        p.circle_filled(c + vec2(kx * k, y * k), 2.6 * k, crate::frame_bg());
        p.circle_stroke(c + vec2(kx * k, y * k), 2.2 * k, st);
    }
}

/// 3D: a cube in outline
pub(crate) fn cube(p: &egui::Painter, c: Pos2, size: f32, col: Color32) {
    let k = size / 16.0;
    let st = Stroke::new(SW, col);
    let q = |x: f32, y: f32| c + vec2(x * k, y * k);
    let (t, l, r, m, bl, br, b) = (q(0.0, -6.5), q(-5.8, -3.2), q(5.8, -3.2), q(0.0, 0.1), q(-5.8, 3.3), q(5.8, 3.3), q(0.0, 6.6));
    p.add(egui::Shape::closed_line(vec![t, r, br, b, bl, l], st));
    p.line_segment([l, m], st);
    p.line_segment([r, m], st);
    p.line_segment([m, b], st);
}
