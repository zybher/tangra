//! Pen strokes as one smooth mesh: the raw points are thinned and rounded (Chaikin), then the
//! line is built as a single triangle strip with round caps and a 1px soft edge. One mesh means
//! no square cuts at zoom and no darker circles at the ends of a see-through stroke.
use crate::*;
use egui::{epaint::Vertex, Mesh, Vec2};

/// Thinned + rounded centre line (screen space)
pub(crate) fn smooth_line(pts: &[Pos2], width: f32) -> Vec<Pos2> {
    if pts.len() < 2 {
        return pts.to_vec();
    }
    // Drop points closer than ~a fifth of the width (min 1.5px): hand jitter, not shape
    let gap = (width * 0.2).max(1.5);
    let mut out: Vec<Pos2> = vec![pts[0]];
    for q in &pts[1..] {
        if q.distance(*out.last().unwrap()) >= gap {
            out.push(*q);
        }
    }
    let last = pts[pts.len() - 1];
    if out.len() < 2 || out.last().unwrap().distance(last) > 0.3 {
        out.push(last);
    }
    // Chaikin corner cutting (ends stay put)
    for _ in 0..3 {
        if out.len() < 3 || out.len() > 6000 {
            break;
        }
        let mut sm = Vec::with_capacity(out.len() * 2);
        sm.push(out[0]);
        for w in out.windows(2) {
            sm.push(w[0] + (w[1] - w[0]) * 0.25);
            sm.push(w[0] + (w[1] - w[0]) * 0.75);
        }
        sm.push(*out.last().unwrap());
        out = sm;
    }
    out
}

fn perp(d: Vec2) -> Vec2 {
    vec2(-d.y, d.x)
}

/// Half a disc of the cap at `c`, facing `dir`, from the +normal side round to the -normal side
fn cap(m: &mut Mesh, c: Pos2, dir: Vec2, hin: f32, hout: f32, col: Color32) {
    let n = perp(dir);
    let steps = ((hout * 0.8).ceil() as usize).clamp(6, 32);
    let ci = m.vertices.len() as u32;
    m.vertices.push(Vertex { pos: c, uv: egui::epaint::WHITE_UV, color: col });
    for i in 0..=steps {
        let a = std::f32::consts::PI * i as f32 / steps as f32;
        let u = n * a.cos() + dir * a.sin();
        m.vertices.push(Vertex { pos: c + u * hin, uv: egui::epaint::WHITE_UV, color: col });
        m.vertices.push(Vertex { pos: c + u * hout, uv: egui::epaint::WHITE_UV, color: Color32::TRANSPARENT });
    }
    for i in 0..steps as u32 {
        let (a_in, a_out, b_in, b_out) = (ci + 1 + 2 * i, ci + 2 + 2 * i, ci + 3 + 2 * i, ci + 4 + 2 * i);
        m.add_triangle(ci, a_in, b_in);
        m.add_triangle(a_in, a_out, b_out);
        m.add_triangle(a_in, b_out, b_in);
    }
}

/// The whole stroke as one mesh
pub(crate) fn stroke_mesh(line: &[Pos2], width: f32, col: Color32) -> Mesh {
    let mut m = Mesh::default();
    if line.len() < 2 || col.a() == 0 {
        return m;
    }
    let hw = width / 2.0;
    let (hin, hout) = ((hw - 0.5).max(0.15), hw + 0.5);
    // Thin lines: fade the colour instead of shrinking under a pixel
    let col = if hw < 0.65 { col.gamma_multiply((hw / 0.65).max(0.2)) } else { col };
    let n = line.len();
    let mut dirs: Vec<Vec2> = (0..n - 1).map(|i| (line[i + 1] - line[i]).normalized()).collect();
    for i in 0..dirs.len() {
        if !dirs[i].is_finite() {
            dirs[i] = if i > 0 { dirs[i - 1] } else { vec2(1.0, 0.0) };
        }
    }
    let base = m.vertices.len() as u32;
    for i in 0..n {
        let d = match i {
            0 => dirs[0],
            _ if i == n - 1 => dirs[n - 2],
            _ => (dirs[i - 1] + dirs[i]).normalized(),
        };
        let d = if d.is_finite() && d.length() > 0.1 { d } else { dirs[i.min(n - 2)] };
        let mut nv = perp(d);
        // Miter: keep the width across bends (capped so sharp turns don't spike)
        if i > 0 && i < n - 1 {
            let c = nv.dot(perp(dirs[i])).abs().max(0.5);
            nv /= c;
        }
        let p = line[i];
        for (off, a) in [(hout, false), (hin, true), (-hin, true), (-hout, false)] {
            m.vertices.push(Vertex { pos: p + nv * off, uv: egui::epaint::WHITE_UV, color: if a { col } else { Color32::TRANSPARENT } });
        }
    }
    for i in 0..(n - 1) as u32 {
        let (a, b) = (base + 4 * i, base + 4 * (i + 1));
        for k in 0..3 {
            m.add_triangle(a + k, a + k + 1, b + k);
            m.add_triangle(a + k + 1, b + k + 1, b + k);
        }
    }
    cap(&mut m, line[0], -dirs[0], hin, hout, col);
    cap(&mut m, line[n - 1], dirs[n - 2], hin, hout, col);
    m
}

