//! Editing a path's points: double-click a pen path (or a rectangle / ellipse, which turns
//! into a path that looks the same) to show its points and handles. Drag a point to move it,
//! drag a handle to bend the curve (the opposite handle turns with it; Alt bends one side
//! only), click the line to add a point there, double-click a point to switch it between
//! sharp and smooth, Delete removes the picked point. Esc, or a click off the path, ends it.

use crate::*;

/// The path being edited
pub(crate) struct PathEdit {
    pub id: String,
    /// picked point (Delete removes it)
    pub sel: Option<usize>,
    /// (point, 0 the point / 1 its `i` handle / 2 its `o` handle) being dragged
    drag: Option<(usize, u8)>,
}

/// How near (screen px) the pointer must be to grab a point / handle / the line
const GRAB_PX: f32 = 8.0;
const LINE_PX: f32 = 6.0;

impl App {
    /// Starts editing a stroke's points (a rectangle / ellipse becomes a path first)
    pub(crate) fn begin_path_edit(&mut self, id: &str) {
        if self.path_edit.as_ref().is_some_and(|e| e.id == id) {
            return;
        }
        let Some(i) = self.board.world_strokes.iter().position(|s| s.id == id) else { return };
        if self.board.world_strokes[i].knots.is_empty() {
            let mut s = self.board.world_strokes[i].clone();
            if !s.to_path() {
                return;
            }
            self.snapshot();
            self.board.world_strokes[i] = s;
            self.mark_dirty();
        }
        self.path_edit = Some(PathEdit { id: id.to_string(), sel: None, drag: None });
    }

    fn edited(&mut self) -> Option<&mut Stroke> {
        let id = self.path_edit.as_ref()?.id.clone();
        self.board.world_strokes.iter_mut().find(|s| s.id == id)
    }

    /// Delete while editing: removes the picked point (a path keeps at least 2, closed 3).
    /// True when the key was used here (the path itself isn't deleted then)
    pub(crate) fn path_delete_knot(&mut self) -> bool {
        let Some(e) = &self.path_edit else { return false };
        let Some(j) = e.sel else { return true };
        let id = e.id.clone();
        let Some(s) = self.board.world_strokes.iter().find(|s| s.id == id) else { return false };
        if s.knots.len() <= if s.closed { 3 } else { 2 } || j >= s.knots.len() {
            return true;
        }
        self.snapshot();
        if let Some(s) = self.edited() {
            s.knots.remove(j);
            s.reflatten();
        }
        if let Some(e) = self.path_edit.as_mut() {
            e.sel = None;
        }
        self.mark_dirty();
        true
    }

    /// Point editing input; true while it has the pointer
    pub(crate) fn path_edit_input(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect, space: bool) -> bool {
        let Some(e) = &self.path_edit else { return false };
        let id = e.id.clone();
        let drag = e.drag;
        let still = self.tool == Tool::Select && self.selected.len() == 1 && self.selected.contains(&(Kind::Stroke, id.clone()));
        let Some(s) = self.board.world_strokes.iter().find(|s| s.id == id && !s.knots.is_empty()).cloned().filter(|_| still) else {
            self.path_edit = None;
            return false;
        };
        let v = self.view(rect);
        let (pressed, down, hover, alt) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.hover_pos(), i.modifiers.alt));
        let scr = |q: Pt| v.s(q.x, q.y);
        // points first, then handles (a collapsed handle isn't one)
        let hit = |q: Pos2| -> Option<(usize, u8)> {
            if let Some(j) = s.knots.iter().position(|k| scr(k.p).distance(q) < GRAB_PX) {
                return Some((j, 0));
            }
            for (j, k) in s.knots.iter().enumerate() {
                for (h, pt) in [(1u8, k.i), (2, k.o)] {
                    if pt != k.p && scr(pt).distance(q) < GRAB_PX {
                        return Some((j, h));
                    }
                }
            }
            None
        };
        let mut used = false;
        if pressed && !space {
            if let Some(q) = hover.filter(|q| resp.contains_pointer() && rect.contains(*q) && !ui_blocks(ctx, *q)) {
                if let Some(h) = hit(q) {
                    self.snapshot();
                    if let Some(e) = self.path_edit.as_mut() {
                        if h.1 == 0 {
                            e.sel = Some(h.0);
                        }
                        e.drag = Some(h);
                    }
                    used = true;
                } else if let Some((j, t)) = near_line(&s, &scr, q) {
                    // a new point on the line, the curve keeping its shape
                    self.snapshot();
                    if let Some(st) = self.edited() {
                        split(st, j, t);
                        st.reflatten();
                    }
                    if let Some(e) = self.path_edit.as_mut() {
                        e.sel = Some(j + 1);
                        e.drag = Some((j + 1, 0));
                    }
                    self.mark_dirty();
                    used = true;
                } else {
                    // anywhere else ends editing; the press then acts as usual
                    self.path_edit = None;
                    return false;
                }
            }
        }
        // double-click a point: sharp <-> smooth
        if resp.double_clicked() {
            if let Some((j, 0)) = hover.and_then(|q| hit(q)) {
                if let Some(st) = self.edited() {
                    toggle_smooth(st, j);
                    st.reflatten();
                }
                self.mark_dirty();
                used = true;
            }
        }
        if let Some((j, h)) = drag {
            if down {
                if let (Some(q), Some(st)) = (hover, self.edited()) {
                    let w = v.w(q);
                    if let Some(k) = st.knots.get_mut(j) {
                        let p = k.p;
                        match h {
                            0 => {
                                let (dx, dy) = (w.x - p.x, w.y - p.y);
                                for pt in [&mut k.p, &mut k.i, &mut k.o] {
                                    pt.x += dx;
                                    pt.y += dy;
                                }
                            }
                            _ => {
                                let (this, other) = if h == 1 { (&mut k.i, &mut k.o) } else { (&mut k.o, &mut k.i) };
                                *this = w;
                                // the other side turns to stay in line, keeping its own length
                                if !alt && *other != p {
                                    let len = ((other.x - p.x).powi(2) + (other.y - p.y).powi(2)).sqrt();
                                    let (dx, dy) = (p.x - w.x, p.y - w.y);
                                    let dl = (dx * dx + dy * dy).sqrt();
                                    if dl > 1e-4 {
                                        *other = Pt { x: p.x + dx / dl * len, y: p.y + dy / dl * len };
                                    }
                                }
                            }
                        }
                    }
                    st.reflatten();
                    self.mark_dirty();
                }
            } else if let Some(e) = self.path_edit.as_mut() {
                e.drag = None;
            }
            used = true;
        }
        if hover.is_some_and(|q| hit(q).is_some()) {
            ctx.set_cursor_icon(egui::CursorIcon::Default);
        }
        used
    }

    /// The edited path's line, points and handles (in place of the selection box)
    pub(crate) fn paint_path_edit(&self, p: &egui::Painter, v: View) {
        let Some(e) = &self.path_edit else { return };
        let Some(s) = self.board.world_strokes.iter().find(|s| s.id == e.id) else { return };
        let pts = super::path_mesh::flatten(s, &|q: Pt| v.s(q.x, q.y));
        p.add(Shape::line(pts, EStroke::new(1.5_f32, blue())));
        paint_knots(p, v, &s.knots, e.sel);
    }
}

/// Points as small squares (the picked one filled), handles as dots on thin lines
pub(crate) fn paint_knots(p: &egui::Painter, v: View, knots: &[Knot], sel: Option<usize>) {
    let scr = |q: Pt| v.s(q.x, q.y);
    for k in knots {
        let c = scr(k.p);
        for h in [k.i, k.o] {
            if h != k.p {
                let hc = scr(h);
                p.line_segment([c, hc], EStroke::new(1.0_f32, blue()));
                p.circle_filled(hc, 3.5, Color32::WHITE);
                p.circle_stroke(hc, 3.5, EStroke::new(1.5_f32, blue()));
            }
        }
    }
    for (j, k) in knots.iter().enumerate() {
        let r = Rect::from_center_size(scr(k.p), vec2(8.0, 8.0));
        let on = sel == Some(j);
        p.rect_filled(r, 1.5, if on { blue() } else { Color32::WHITE });
        p.rect_stroke(r, 1.5, EStroke::new(1.5_f32, if on { Color32::WHITE } else { blue() }));
    }
}

/// Nearest spot on the path's line to `q` within reach: (segment, t)
fn near_line(s: &Stroke, scr: &impl Fn(Pt) -> Pos2, q: Pos2) -> Option<(usize, f32)> {
    let mut best: Option<(f32, usize, f32)> = None;
    for j in 0..s.segments() {
        let (a, b, c, d) = s.segment(j);
        for i in 1..48 {
            let t = i as f32 / 48.0;
            let dist = scr(cubic(a, b, c, d, t)).distance(q);
            if best.map_or(true, |bb| dist < bb.0) {
                best = Some((dist, j, t));
            }
        }
    }
    best.filter(|b| b.0 < LINE_PX).map(|b| (b.1, b.2))
}

/// Cuts segment `j` at `t` into two that trace the same curve, with a new point between them
fn split(s: &mut Stroke, j: usize, t: f32) {
    let (a, b, c, d) = s.segment(j);
    let l = |p: Pt, q: Pt| Pt { x: p.x + (q.x - p.x) * t, y: p.y + (q.y - p.y) * t };
    let (ab, bc, cd) = (l(a, b), l(b, c), l(c, d));
    let (abc, bcd) = (l(ab, bc), l(bc, cd));
    let m = l(abc, bcd);
    let n = s.knots.len();
    s.knots[j].o = ab;
    s.knots[(j + 1) % n].i = cd;
    s.knots.insert(j + 1, Knot { p: m, i: abc, o: bcd });
}

/// A point with handles loses them (sharp); a sharp one gets handles along its neighbours
fn toggle_smooth(s: &mut Stroke, j: usize) {
    let n = s.knots.len();
    let k = s.knots[j];
    if k.i != k.p || k.o != k.p {
        s.knots[j] = Knot::corner(k.p);
        return;
    }
    let prev = if j > 0 { s.knots[j - 1].p } else if s.closed { s.knots[n - 1].p } else { k.p };
    let next = if j + 1 < n { s.knots[j + 1].p } else if s.closed { s.knots[0].p } else { k.p };
    let (dx, dy) = (next.x - prev.x, next.y - prev.y);
    let dl = (dx * dx + dy * dy).sqrt();
    if dl < 1e-4 {
        return;
    }
    let (ux, uy) = (dx / dl, dy / dl);
    let dist = |a: Pt| ((a.x - k.p.x).powi(2) + (a.y - k.p.y).powi(2)).sqrt() / 3.0;
    let (li, lo) = (dist(prev), dist(next));
    s.knots[j].i = Pt { x: k.p.x - ux * li, y: k.p.y - uy * li };
    s.knots[j].o = Pt { x: k.p.x + ux * lo, y: k.p.y + uy * lo };
}
