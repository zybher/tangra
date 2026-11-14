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
