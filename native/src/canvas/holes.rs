//! Holes in any object: spots rubbed out with the eraser and loops cut with the lasso
//! (`erase` / `erase_loops` on strokes, text and pictures, stored as fractions of the
//! object's box so they move and stretch with it). The object paints as usual, then what it
//! painted is turned into triangles and the holes are cut out of those, one pixel row at a
//! time with soft ends, so any look (fills, outlines, text, pictures) gets bitten the same.
use crate::*;
use egui::epaint::{ClippedShape, Primitive, Vertex};
use egui::layers::ShapeIdx;
use egui::{Color32, Mesh, Pos2, Rect, Shape};
use std::collections::HashMap;

/// A hole in screen points
pub(crate) enum Hole {
    /// Centre and radii
    Oval(Pos2, f32, f32),
    Loop(Vec<Pos2>),
}

/// The box holes are measured against, in the stroke's own units
pub(crate) fn stroke_box(s: &Stroke) -> Option<[f32; 4]> {
    let pts = match s.closed_len() {
        Some(n) => &s.points[..n],
        None => &s.points[..],
    };
    let [x, y, w, h] = pts_bbox(pts)?;
    let pad = if s.closed_len().is_some() { 0.0 } else { (s.width / 2.0).max(0.5) };
    Some([x - pad, y - pad, w + pad * 2.0, h + pad * 2.0])
}

pub(crate) fn label_box(l: &Label) -> [f32; 4] {
    let (w, h) = text_wh(&l.text, l.font_size);
    [l.x, l.y, w, h]
}

/// Holes in screen points, `f` taking the object's own units to the screen
pub(crate) fn screen(spots: &[[f32; 4]], loops: &[Vec<[f32; 2]>], [x, y, w, h]: [f32; 4], f: impl Fn(Pt) -> Pos2) -> Vec<Hole> {
    let at = |u: f32, v: f32| f(Pt { x: x + u * w, y: y + v * h });
    let mut out: Vec<Hole> = spots
        .iter()
        .map(|[u, v, ru, rv]| {
            let c = at(*u, *v);
            Hole::Oval(c, (at(u + ru, *v) - c).length().max(0.01), (at(*u, v + rv) - c).length().max(0.01))
        })
        .collect();
    out.extend(loops.iter().filter(|l| l.len() >= 3).map(|l| Hole::Loop(l.iter().map(|[u, v]| at(*u, *v)).collect())));
    out
}

/// True when `q` (object units) is in one of the holes, so clicks go through them
pub(crate) fn in_hole(spots: &[[f32; 4]], loops: &[Vec<[f32; 2]>], [x, y, w, h]: [f32; 4], q: Pt) -> bool {
    if w <= 0.0 || h <= 0.0 {
        return false;
    }
    let (u, v) = ((q.x - x) / w, (q.y - y) / h);
    spots.iter().any(|[cu, cv, ru, rv]| ((u - cu) / ru.max(1e-6)).powi(2) + ((v - cv) / rv.max(1e-6)).powi(2) < 1.0)
        || loops.iter().any(|l| {
            let mut ins = false;
            for i in 0..l.len() {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                if (a[1] > v) != (b[1] > v) && u < a[0] + (v - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                    ins = !ins;
                }
            }
            ins
        })
}

/// Runs `draw`, then swaps what it painted for the same thing with `holes` cut out
pub(crate) fn with_holes(p: &egui::Painter, holes: &[Hole], draw: impl FnOnce()) {
    if holes.is_empty() {
        draw();
        return;
    }
    let ctx = p.ctx();
    let layer = p.layer_id();
    let a = ctx.graphics_mut(|g| g.entry(layer).next_idx().0);
    draw();
    let shapes: Vec<ClippedShape> = ctx.graphics_mut(|g| {
        let l = g.entry(layer);
        let n = l.next_idx().0;
        let v: Vec<ClippedShape> = l.all_entries().skip(a).take(n.saturating_sub(a)).cloned().collect();
        for i in a..n {
            l.reset_shape(ShapeIdx(i));
        }
        v
    });
    if shapes.is_empty() {
        return;
    }
    let prims = ctx.tessellate(shapes, ctx.pixels_per_point());
    let cut: Vec<(Rect, Mesh)> = prims
        .into_iter()
        .filter_map(|cp| match cp.primitive {
            Primitive::Mesh(m) => Some((cp.clip_rect, cut_mesh(&m, holes, cp.clip_rect))),
            _ => None,
        })
        .collect();
    // added straight to the layer: the painter's fade was already applied the first time
    ctx.graphics_mut(|g| {
        let l = g.entry(layer);
        for (clip, m) in cut {
            l.add(clip, Shape::mesh(m));
        }
    });
}

fn hole_box(h: &Hole) -> Rect {
    match h {
        Hole::Oval(c, rx, ry) => Rect::from_center_size(*c, egui::vec2(rx * 2.0, ry * 2.0)),
        Hole::Loop(l) => Rect::from_points(l),
    }
}

/// The parts of row `yc` inside any hole, sorted and merged
fn row_cuts(holes: &[Hole], yc: f32) -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = Vec::new();
    let mut xs: Vec<f32> = Vec::new();
    for h in holes {
        match h {
            Hole::Oval(c, rx, ry) => {
                let d = (yc - c.y) / ry;
                if d.abs() < 1.0 {
                    let w = rx * (1.0 - d * d).sqrt();
                    v.push((c.x - w, c.x + w));
                }
            }
            Hole::Loop(l) => {
                xs.clear();
                for i in 0..l.len() {
                    let (a, b) = (l[i], l[(i + 1) % l.len()]);
                    if (a.y <= yc) != (b.y <= yc) {
                        xs.push(a.x + (yc - a.y) / (b.y - a.y) * (b.x - a.x));
                    }
                }
                xs.sort_by(|a, b| a.total_cmp(b));
                v.extend(xs.chunks_exact(2).map(|p| (p[0], p[1])));
            }
        }
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f32, f32)> = Vec::new();
    for (a, b) in v {
        match out.last_mut() {
            Some(l) if a <= l.1 => l.1 = l.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

fn lerp(a: &Vertex, b: &Vertex, t: f32) -> Vertex {
    Vertex { pos: a.pos + (b.pos - a.pos) * t, uv: a.uv + (b.uv - a.uv) * t, color: mix(a.color, b.color, t) }
}

/// Keeps the part of `poly` with x (or y) >= v (`ge`) or <= v
fn clip(poly: &[Vertex], use_x: bool, v: f32, ge: bool) -> Vec<Vertex> {
    let c = |q: &Vertex| if use_x { q.pos.x } else { q.pos.y };
    let ins = |q: &Vertex| if ge { c(q) >= v } else { c(q) <= v };
    let mut out = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let (a, b) = (&poly[i], &poly[(i + 1) % poly.len()]);
        let (ia, ib) = (ins(a), ins(b));
        if ia {
            out.push(*a);
        }
        if ia != ib {
            let t = (v - c(a)) / (c(b) - c(a));
            out.push(lerp(a, b, t));
        }
    }
    out
}

fn emit(m: &mut Mesh, poly: &[Vertex]) {
    if poly.len() < 3 {
        return;
    }
    let base = m.vertices.len() as u32;
    m.vertices.extend_from_slice(poly);
    for i in 1..poly.len() as u32 - 1 {
        m.add_triangle(base, base + i, base + i + 1);
    }
}

/// Fades each corner by `k(x)` (0..1)
fn faded(poly: Vec<Vertex>, k: impl Fn(f32) -> f32) -> Vec<Vertex> {
    poly.into_iter()
        .map(|mut q| {
            let t = k(q.pos.x).clamp(0.0, 1.0);
            q.color = mix(Color32::TRANSPARENT, q.color, t);
            q
        })
        .collect()
}

fn cut_mesh(m: &Mesh, holes: &[Hole], clip_rect: Rect) -> Mesh {
    let union = holes.iter().map(hole_box).fold(Rect::NOTHING, |a, b| a.union(b));
    let mut out = Mesh::with_texture(m.texture_id);
    out.vertices = m.vertices.clone();
    let mut rows: HashMap<i32, Vec<(f32, f32)>> = HashMap::new();
    for tri in m.indices.chunks_exact(3) {
        let v = [m.vertices[tri[0] as usize], m.vertices[tri[1] as usize], m.vertices[tri[2] as usize]];
        let tb = Rect::from_points(&[v[0].pos, v[1].pos, v[2].pos]);
        if !tb.intersects(union) || !tb.intersects(clip_rect) {
            out.indices.extend_from_slice(tri);
            continue;
        }
        let poly = v.to_vec();
        let ya = union.min.y.floor().max(tb.min.y.floor());
        let yb = union.max.y.ceil().min(tb.max.y.ceil());
        emit(&mut out, &clip(&poly, false, ya, false));
        emit(&mut out, &clip(&poly, false, yb, true));
        let mut y = ya;
        while y < yb {
            let band = clip(&clip(&poly, false, y, true), false, y + 1.0, false);
            if band.len() >= 3 {
                let cuts = rows.entry(y as i32).or_insert_with(|| row_cuts(holes, y + 0.5));
                // what's left between the holes on this row
                let mut lo = f32::NEG_INFINITY;
                for &(c0, c1) in cuts.iter().chain(std::iter::once(&(f32::INFINITY, f32::INFINITY))) {
                    keep(&mut out, &band, lo, c0);
                    lo = c1;
                }
            }
            y += 1.0;
        }
    }
    out
}

/// Draws the row piece between holes lo..hi, faded over a pixel at each hole edge
fn keep(m: &mut Mesh, band: &[Vertex], lo: f32, hi: f32) {
    if hi <= lo {
        return;
    }
    let (fl, fh) = (lo.is_finite(), hi.is_finite());
    let part = |a: f32, b: f32| -> Vec<Vertex> {
        let mut q = band.to_vec();
        if a.is_finite() {
            q = clip(&q, true, a, true);
        }
        if b.is_finite() {
            q = clip(&q, true, b, false);
        }
        q
    };
    if fl && fh && hi - lo < 1.0 {
        let w = hi - lo;
        emit(m, &faded(part(lo, hi), |_| w));
        return;
    }
    let a = if fl { lo + 0.5 } else { lo };
    let b = if fh { hi - 0.5 } else { hi };
    emit(m, &part(a, b));
    if fl {
        emit(m, &faded(part(lo - 0.5, lo + 0.5), |x| x - (lo - 0.5)));
    }
    if fh {
        emit(m, &faded(part(hi - 0.5, hi + 0.5), |x| hi + 0.5 - x));
    }
}
