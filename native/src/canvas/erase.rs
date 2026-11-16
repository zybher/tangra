//! The eraser rubs out only what it passes over, like a paint program's eraser:
//! - pen / highlighter / arrow lines are cut where it goes, leaving the pieces either side
//!   (the first piece keeps the line's id, name, frame and place in the stack);
//! - rectangles and ellipses stay one shape and remember the rubbed spots (`Stroke::erase`),
//!   drawn with those spots cut out of the fill and outline;
//! - text is still removed whole.
use crate::*;
use egui::{pos2, Color32, Mesh, Pos2, Rect, Shape};
use std::sync::Mutex;

/// Where the eraser was on the last call, so a fast swipe erases the whole way between
static LAST: Mutex<Option<Pt>> = Mutex::new(None);

/// A new swipe of the eraser starts here
pub(crate) fn reset() {
    *LAST.lock().unwrap() = None;
}

/// Points along a..b no further apart than `step`
fn along(a: Pt, b: Pt, step: f32) -> Vec<Pt> {
    let d = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    let n = (d / step.max(1e-4)).ceil().max(1.0) as usize;
    (0..=n).map(|i| {
        let t = i as f32 / n as f32;
        Pt { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t }
    }).collect()
}

fn near_box(a: Pt, b: Pt, c: Pt, d: Pt, r: f32) -> bool {
    a.x.min(b.x) - r <= c.x.max(d.x) && c.x.min(d.x) - r <= a.x.max(b.x) && a.y.min(b.y) - r <= c.y.max(d.y) && c.y.min(d.y) - r <= a.y.max(b.y)
}

/// Rubs `s` along the eraser path a..b (in `s`'s own units, eraser radius `r`). Returns the
/// pieces left of an open line (None: untouched), or adds spots to a shape (Some(vec![])
/// means touched, nothing to replace)
fn rub(s: &mut Stroke, a: Pt, b: Pt, r: f32) -> Option<Vec<Vec<Pt>>> {
    let w = s.width;
    if let Some(n) = s.closed_len() {
        let [x, y, bw, bh] = pts_bbox(&s.points[..n])?;
        if bw < 1e-4 || bh < 1e-4 {
            return None;
        }
        let mut hit = false;
        for q in along(a, b, r * 0.4) {
            let over = stroke_hit(s, q, r + w / 2.0) || (s.fill && inside(q, x - r, y - r, bw + 2.0 * r, bh + 2.0 * r));
            if !over {
                continue;
            }
            let m = [(q.x - x) / bw, (q.y - y) / bh, r / bw, r / bh];
            let dup = s.erase.last().is_some_and(|l| ((l[0] - m[0]) * bw).abs() < r * 0.25 && ((l[1] - m[1]) * bh).abs() < r * 0.25);
            if !dup {
                s.erase.push(m);
                hit = true;
            }
        }
        return hit.then(Vec::new);
    }
    // Open line: finer points only where the eraser comes near, then drop what it covers
    let reach = r + w * 0.25;
    let mut pts: Vec<Pt> = Vec::with_capacity(s.points.len());
    for (i, seg) in s.points.windows(2).enumerate() {
        if near_box(seg[0], seg[1], a, b, reach) {
            let fine = along(seg[0], seg[1], r * 0.4);
            pts.extend(if i == 0 { &fine[..] } else { &fine[1..] });
        } else {
            if i == 0 {
                pts.push(seg[0]);
            }
            pts.push(seg[1]);
        }
    }
    if s.points.len() == 1 {
        pts.push(s.points[0]);
    }
    let keep: Vec<bool> = pts.iter().map(|q| seg_dist(*q, a, b) >= reach).collect();
    if keep.iter().all(|k| *k) {
        return None;
    }
    let mut out: Vec<Vec<Pt>> = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    for (q, k) in pts.into_iter().zip(keep) {
        if k {
            cur.push(q);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    out.push(cur);
    out.retain(|v| v.len() >= 2);
    Some(out)
}

/// Rubs every line in `list`; pieces past the first get new ids sharing the old one's frame
/// and stack place (pushed to `born` as (old id, new id))
fn rub_list(list: &mut Vec<Stroke>, a: Pt, b: Pt, r: f32, scale: f32, born: &mut Vec<(String, String)>, gone: &mut Vec<String>) -> bool {
    use super::presence::is_dying;
    let mut changed = false;
    let mut i = 0;
    while i < list.len() {
        if is_dying(&list[i].id) {
            i += 1;
            continue;
        }
        let res = rub(&mut list[i], a, b, r / scale);
        let Some(pieces) = res else {
            i += 1;
            continue;
        };
        changed = true;
        if list[i].closed_len().is_some() {
            i += 1;
            continue;
        }
        if pieces.is_empty() {
            gone.push(list.remove(i).id);
            continue;
        }
        let old = list[i].clone();
        let mut it = pieces.into_iter();
        list[i].points = it.next().unwrap_or_default();
        i += 1;
        for p in it {
            let mut s = old.clone();
            s.id = new_id();
            s.points = p;
            born.push((old.id.clone(), s.id.clone()));
            list.insert(i, s);
            i += 1;
        }
    }
    changed
}

impl App {
    /// Rubs out what's under the eraser between its last spot and `p`. True if anything changed
    pub(crate) fn erase_at(&mut self, p: Pt) -> bool {
        let zoom = self.board.camera.zoom;
        let r = 9.0 / zoom;
        let a = LAST.lock().unwrap().replace(p).unwrap_or(p);
        use super::presence::is_dying;
        let mut born = Vec::new();
        let mut gone = Vec::new();
        let b = &mut self.board;
        let mut changed = rub_list(&mut b.world_strokes, a, p, r, 1.0, &mut born, &mut gone);
        // Text still goes whole (fades out)
        let mut texts: Vec<String> = Vec::new();
        for q in along(a, p, r * 0.5) {
            for l in b.world_texts.iter().filter(|l| !is_dying(&l.id) && label_hit(l, q, r * 0.3)) {
                if !texts.contains(&l.id) {
                    texts.push(l.id.clone());
                }
            }
        }
        if let Some(img) = b.images.iter_mut().rev().find(|i| inside(p, i.x, i.y, i.width, i.height)) {
            let (sx, sy) = img.scale();
            let loc = |q: Pt| Pt { x: (q.x - img.x) / sx, y: (q.y - img.y) / sy };
            let (la, lp) = (loc(a), loc(p));
            changed |= rub_list(&mut img.annotations.strokes, la, lp, r, sx, &mut born, &mut gone);
            texts.extend(img.annotations.texts.iter().filter(|l| !is_dying(&l.id) && label_hit(l, lp, r * 0.3 / sx)).map(|l| l.id.clone()));
        }
        for (old, new) in born {
            if let Some(par) = b.parents.get(&old).cloned() {
                b.parents.insert(new.clone(), par);
            }
            if let Some(o) = b.layer_order.get(&old).copied() {
                b.layer_order.insert(new, o);
            }
        }
        for id in &gone {
            b.parents.remove(id);
            b.layer_order.remove(id);
            b.layer_names.remove(id);
        }
        if !gone.is_empty() {
            let g: HashSet<String> = gone.into_iter().collect();
            self.selected.retain(|s| !g.contains(&s.1));
        }
        changed |= !texts.is_empty();
        self.kill_or_remove(texts);
        if changed {
            self.mark_dirty();
        }
        changed
    }
}

/// A rubbed rectangle / ellipse, built from 1px rows with soft ends: the fill, then the
/// outline ring (between the shape moved out and in by half the line), minus the spots
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_erased(p: &egui::Painter, s: &Stroke, poly: &[Pos2], rad: Option<f32>, fill: Color32, fk: f32, col: Color32, sk: f32, width: f32, inset: f32) {
    let bb = Rect::from_points(poly);
    let shape: Vec<Pos2> = match rad {
        Some(r) => super::round_clip::rounded_poly(bb, r),
        None => poly.to_vec(),
    };
    let holes: Vec<(Pos2, f32, f32)> = s.erase.iter().map(|[u, v, ru, rv]| (pos2(bb.min.x + u * bb.width(), bb.min.y + v * bb.height()), ru * bb.width(), rv * bb.height())).collect();
    let clip = p.clip_rect();
    let mut mesh = Mesh::default();
    if fk > 0.001 && fill.a() > 0 {
        rows(&mut mesh, &[&shape], &holes, fill, clip);
    }
    if sk > 0.001 && width > 0.0 && col.a() > 0 {
        let outer = offset_closed(&shape, inset - width / 2.0);
        let inner = offset_closed(&shape, inset + width / 2.0);
        rows(&mut mesh, &[&outer, &inner], &holes, col, clip);
    }
    p.add(Shape::mesh(mesh));
}

/// Fills the even-odd inside of `polys`, less the elliptic `holes`, one pixel row at a time
fn rows(m: &mut Mesh, polys: &[&[Pos2]], holes: &[(Pos2, f32, f32)], col: Color32, clip: Rect) {
    let mut bb = Rect::NOTHING;
    for poly in polys {
        for q in poly.iter() {
            bb.extend_with(*q);
        }
    }
    let bb = bb.intersect(clip);
    if !bb.is_positive() {
        return;
    }
    let mut xs: Vec<f32> = Vec::new();
    let mut cut: Vec<(f32, f32)> = Vec::new();
    let mut y = bb.min.y.floor();
    while y < bb.max.y {
        let yc = y + 0.5;
        xs.clear();
        for poly in polys {
            let n = poly.len();
            for i in 0..n {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                if (a.y <= yc) != (b.y <= yc) {
                    xs.push(a.x + (yc - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        cut.clear();
        for (c, rx, ry) in holes {
            let d = (yc - c.y) / ry.max(1e-6);
            if d.abs() < 1.0 {
                let h = rx * (1.0 - d * d).sqrt();
                cut.push((c.x - h, c.x + h));
            }
        }
        cut.sort_by(|a, b| a.0.total_cmp(&b.0));
        for pair in xs.chunks_exact(2) {
            let (x0, x1) = (pair[0].max(clip.min.x - 1.0), pair[1].min(clip.max.x + 1.0));
            let mut cur = x0;
            for &(c0, c1) in &cut {
                if c1 <= cur {
                    continue;
                }
                if c0 >= x1 {
                    break;
                }
                if c0 > cur {
                    span(m, cur, c0, y, col);
                }
                cur = cur.max(c1);
            }
            if cur < x1 {
                span(m, cur, x1, y, col);
            }
        }
        y += 1.0;
    }
}

/// One row piece x0..x1, faded over its last pixel each end so edges don't look jagged
fn span(m: &mut Mesh, x0: f32, x1: f32, y: f32, col: Color32) {
    let w = x1 - x0;
    if w <= 0.0 {
        return;
    }
    let (y0, y1) = (y, y + 1.0);
    let quad = |m: &mut Mesh, a: f32, b: f32, ca: Color32, cb: Color32| {
        let i = m.vertices.len() as u32;
        m.colored_vertex(pos2(a, y0), ca);
        m.colored_vertex(pos2(b, y0), cb);
        m.colored_vertex(pos2(b, y1), cb);
        m.colored_vertex(pos2(a, y1), ca);
        m.add_triangle(i, i + 1, i + 2);
        m.add_triangle(i, i + 2, i + 3);
    };
    if w < 1.0 {
        let c = col.gamma_multiply(w);
        quad(m, x0, x1, c, c);
        return;
    }
    let t = Color32::TRANSPARENT;
    quad(m, x0 - 0.5, x0 + 0.5, t, col);
    quad(m, x0 + 0.5, x1 - 0.5, col, col);
    quad(m, x1 - 0.5, x1 + 0.5, col, t);
}
