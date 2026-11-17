//! Editing a path's points: double-click a pen path (or a rectangle / ellipse, which turns
//! into a path that looks the same) to show its points and handles. Drag a point to move it,
//! drag a handle to bend the curve (the opposite handle turns with it; Alt bends one side
//! only), click the line to add a point there, double-click a point to switch it between
//! sharp and smooth, Delete removes the picked point. Esc, or a click off the path, ends it.
//! While editing: the shape wears a faint blue hatch, the line under the pointer shows where a
//! new point would go, and a dragged point / handle snaps level with the path's own points.
//! The edit bar swaps to point buttons (ui/style_bar.rs) and the panel shows the picked point.

use crate::*;

/// The path being edited
pub(crate) struct PathEdit {
    pub id: String,
    /// picked point (Delete removes it)
    pub sel: Option<usize>,
    /// (point, 0 the point / 1 its `i` handle / 2 its `o` handle) being dragged
    drag: Option<(usize, u8)>,
    /// The stroke as it was when editing began (Reset brings it back, rectangle and all)
    orig: Stroke,
    /// Snap guide lines while dragging (world x0, y0, x1, y1)
    guides: Vec<[f32; 4]>,
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
        let orig = self.board.world_strokes[i].clone();
        if self.board.world_strokes[i].knots.is_empty() {
            let mut s = self.board.world_strokes[i].clone();
            if !s.to_path() {
                return;
            }
            self.snapshot();
            self.board.world_strokes[i] = s;
            self.mark_dirty();
        }
        self.path_edit = Some(PathEdit { id: id.to_string(), sel: None, drag: None, orig, guides: Vec::new() });
    }

    /// The picked point of the edited path
    pub(crate) fn picked_knot(&self) -> Option<Knot> {
        let e = self.path_edit.as_ref()?;
        self.board.world_strokes.iter().find(|s| s.id == e.id)?.knots.get(e.sel?).copied()
    }

    /// Changes the picked point (no undo step: the caller takes one per gesture)
    pub(crate) fn path_change_knot(&mut self, f: impl FnOnce(&mut Stroke, usize)) {
        let Some(j) = self.path_edit.as_ref().and_then(|e| e.sel) else { return };
        if let Some(s) = self.edited().filter(|s| j < s.knots.len()) {
            f(s, j);
            s.reflatten();
            self.mark_dirty();
        }
    }

    /// Bar / panel: the picked point sharp <-> smooth
    pub(crate) fn path_toggle_smooth(&mut self) {
        self.snapshot();
        self.path_change_knot(toggle_smooth);
    }

    /// How the picked point's handles follow each other (0 angle, 1 apart, 2 angle + length);
    /// the handles line up at once
    pub(crate) fn path_set_mirror(&mut self, m: u8) {
        self.snapshot();
        self.path_change_knot(|s, j| {
            let k = &mut s.knots[j];
            k.m = m;
            let p = k.p;
            let len = |a: Pt| ((a.x - p.x).powi(2) + (a.y - p.y).powi(2)).sqrt();
            if k.o == p || k.i == p || m == 1 {
                return;
            }
            let (lo, li) = (len(k.o), len(k.i));
            let l = if m == 2 { (lo + li) / 2.0 } else { li };
            let (ux, uy) = ((k.o.x - p.x) / lo, (k.o.y - p.y) / lo);
            k.i = Pt { x: p.x - ux * l, y: p.y - uy * l };
            if m == 2 {
                k.o = Pt { x: p.x + ux * l, y: p.y + uy * l };
            }
        });
    }

    /// Reset: the stroke as it was before editing (a rectangle comes back as one, which ends
    /// point editing)
    pub(crate) fn path_reset(&mut self) {
        let Some(e) = &self.path_edit else { return };
        let orig = e.orig.clone();
        self.snapshot();
        if let Some(s) = self.edited() {
            *s = orig.clone();
        }
        if orig.knots.is_empty() {
            self.path_edit = None;
        } else if let Some(e) = self.path_edit.as_mut() {
            e.sel = None;
        }
        self.mark_dirty();
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
                let mut guides = Vec::new();
                let free = ctx.input(|i| i.modifiers.ctrl);
                if let (Some(q), Some(st)) = (hover, self.edited()) {
                    let mut w = v.w(q);
                    // level with the path's own points (this point's too, for its handles);
                    // Ctrl moves freely, like the object snap
                    if !free {
                        let reach = 6.0 / v.cam.zoom;
                        let others: Vec<Pt> = st.knots.iter().enumerate().filter(|(i, _)| *i != j || h != 0).map(|(_, k)| k.p).collect();
                        let near = |d: &dyn Fn(&Pt) -> f32| others.iter().map(|o| (d(o).abs(), *o)).filter(|x| x.0 < reach).min_by(|a, b| a.0.total_cmp(&b.0)).map(|x| x.1);
                        if let Some(o) = near(&|o: &Pt| o.x - w.x) {
                            w.x = o.x;
                            guides.push([o.x, o.y, 0.0, 0.0]);
                        }
                        if let Some(o) = near(&|o: &Pt| o.y - w.y) {
                            w.y = o.y;
                            guides.push([o.x, o.y, 0.0, 0.0]);
                        }
                        // guides run from the point snapped to, to where it lands
                        for g in &mut guides {
                            (g[2], g[3]) = (w.x, w.y);
                        }
                    }
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
                                let m = k.m;
                                let (this, other) = if h == 1 { (&mut k.i, &mut k.o) } else { (&mut k.o, &mut k.i) };
                                *this = w;
                                if !alt && m == 2 {
                                    // mirrored: the other side matches it exactly
                                    *other = Pt { x: 2.0 * p.x - w.x, y: 2.0 * p.y - w.y };
                                } else if !alt && m == 0 && *other != p {
                                    // the other side turns to stay in line, keeping its own length
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
                if let Some(e) = self.path_edit.as_mut() {
                    e.guides = guides;
                }
            } else if let Some(e) = self.path_edit.as_mut() {
                e.drag = None;
                e.guides.clear();
            }
            used = true;
        }
        let over_ui = hover.is_some_and(|q| !resp.contains_pointer() || ui_blocks(ctx, q));
        if !over_ui && !space {
            if hover.is_some_and(|q| hit(q).is_some()) || drag.is_some() {
                ctx.set_cursor_icon(egui::CursorIcon::Default);
            } else if hover.is_some_and(|q| near_line(&s, &scr, q).is_some()) {
                // the arrow grows a little and carries the pen badge: a click here adds a point
                crate::ui::cursor::set_badge(ctx, "pen");
                ctx.set_cursor_icon(egui::CursorIcon::Default);
            }
        }
        used
    }

    /// The edited path's line, points and handles (in place of the selection box)
    pub(crate) fn paint_path_edit(&self, p: &egui::Painter, v: View) {
        let Some(e) = &self.path_edit else { return };
        let Some(s) = self.board.world_strokes.iter().find(|s| s.id == e.id) else { return };
        let ctx = p.ctx();
        let id = egui::Id::new(("qd_pedit", &e.id));
        let scr = |q: Pt| v.s(q.x, q.y);
        let pts = super::path_mesh::flatten(s, &scr);
        // point mode: a faint diagonal hatch over the shape, eased in
        let hk = ease(ctx, id.with("hatch"), true);
        if s.closed && pts.len() > 3 {
            hatch(p, &pts, blue().gamma_multiply(0.35 * hk));
        }
        p.add(Shape::line(pts, EStroke::new(1.5_f32, blue())));
        // snap guides (the object snap's look)
        for g in &e.guides {
            let (a, b) = (v.s(g[0], g[1]), v.s(g[2], g[3]));
            let st = EStroke::new(1.0_f32, Color32::from_rgb(242, 78, 30));
            p.line_segment([a, b], st);
            for c in [a, b] {
                p.line_segment([c + vec2(-3.0, -3.0), c + vec2(3.0, 3.0)], st);
                p.line_segment([c + vec2(-3.0, 3.0), c + vec2(3.0, -3.0)], st);
            }
        }
        // where a click would add a point: a ghost dot on the line under the pointer
        let hover = ctx.pointer_hover_pos().filter(|q| p.clip_rect().contains(*q) && !ui_blocks(ctx, *q));
        let on_knot = hover.is_some_and(|q| s.knots.iter().any(|k| [k.p, k.i, k.o].iter().any(|h| scr(*h).distance(q) < GRAB_PX)));
        let spot = hover.filter(|_| e.drag.is_none() && !on_knot).and_then(|q| near_line(s, &scr, q)).map(|(j, t)| {
            let (a, b, c, d) = s.segment(j);
            scr(cubic(a, b, c, d, t))
        });
        let gk = ease(ctx, id.with("ghost"), spot.is_some());
        if let Some(c) = spot {
            ctx.data_mut(|d| d.insert_temp(id.with("ghost_at"), c));
        }
        if gk > 0.01 {
            if let Some(c) = ctx.data(|d| d.get_temp::<Pos2>(id.with("ghost_at"))) {
                let r = 3.5 * gk;
                p.circle_filled(c, r, Color32::WHITE.gamma_multiply(gk));
                p.circle_stroke(c, r, EStroke::new(1.5_f32, blue().gamma_multiply(gk)));
            }
        }
        paint_knots(p, v, &s.knots, e.sel, e.drag, &e.id);
    }
}

/// Points as small squares (the picked one filled), handles as dots on thin lines. They scale
/// in when shown, and grow a little under the pointer / while held, eased like every hover
/// (`ease`, as `hover_fx`); the picked point's fill fades to blue. `key` keeps each path's
/// animations apart.
pub(crate) fn paint_knots(p: &egui::Painter, v: View, knots: &[Knot], sel: Option<usize>, held: Option<(usize, u8)>, key: &str) {
    let ctx = p.ctx();
    let scr = |q: Pt| v.s(q.x, q.y);
    let hover = ctx.pointer_hover_pos().filter(|q| p.clip_rect().contains(*q) && !ui_blocks(ctx, *q));
    // the one thing under the pointer: points first, then handles (as path_edit_input picks)
    let under = hover.and_then(|q| {
        if let Some(j) = knots.iter().position(|k| scr(k.p).distance(q) < GRAB_PX) {
            return Some((j, 0u8));
        }
        knots.iter().enumerate().find_map(|(j, k)| [(1u8, k.i), (2, k.o)].into_iter().find(|(_, h)| *h != k.p && scr(*h).distance(q) < GRAB_PX).map(|(h, _)| (j, h)))
    });
    let id = egui::Id::new(("qd_knot", key));
    // scale in when they first show (ease's time and curve, restarted after a gap)
    let (now, fr) = (ctx.input(|i| i.time), ctx.cumulative_pass_nr());
    let (last, t0) = ctx.data(|d| d.get_temp::<(u64, f64)>(id.with("in"))).unwrap_or((0, now));
    let t0 = if last + 1 < fr { now } else { t0 };
    ctx.data_mut(|d| d.insert_temp(id.with("in"), (fr, t0)));
    let x = ((now - t0) as f32 / (0.22 * anim_k()).max(1e-3)).clamp(0.0, 1.0);
    if x < 1.0 {
        ctx.request_repaint();
    }
    let shown = egui::emath::easing::cubic_out(x);
    // hover / held grow, as hover_fx's two eases
    let fx = |j: usize, h: u8| {
        let hov = ease(ctx, id.with((j, h, "hov")), under == Some((j, h)) || held == Some((j, h)));
        let prs = ease(ctx, id.with((j, h, "prs")), held == Some((j, h)));
        (hov, prs)
    };
    for (j, k) in knots.iter().enumerate() {
        let c = scr(k.p);
        for (h, pt) in [(1u8, k.i), (2, k.o)] {
            if pt != k.p {
                let hc = c + (scr(pt) - c) * shown;
                let (hov, prs) = fx(j, h);
                let r = (3.5 + 1.5 * hov - 0.5 * prs) * shown;
                p.line_segment([c, hc], EStroke::new(1.0_f32, blue().gamma_multiply(0.6 + 0.4 * hov)));
                p.circle_filled(hc, r, mix(Color32::WHITE, blue(), prs));
                p.circle_stroke(hc, r, EStroke::new(1.5_f32, mix(blue(), Color32::WHITE, prs)));
            }
        }
    }
    for (j, k) in knots.iter().enumerate() {
        let (hov, prs) = fx(j, 0);
        let on = ease(ctx, id.with((j, "on")), sel == Some(j));
        let size = (8.0 + 3.0 * hov - 1.0 * prs) * shown;
        let r = Rect::from_center_size(scr(k.p), vec2(size, size));
        p.rect_filled(r, 1.5, mix(Color32::WHITE, blue(), on));
        p.rect_stroke(r, 1.5, EStroke::new(1.5_f32, mix(blue(), Color32::WHITE, on)));
    }
}

/// Diagonal lines 7 px apart over the inside of the outline `pts` (screen), cut where they
/// cross it (even-odd), so only the shape is striped
fn hatch(p: &egui::Painter, pts: &[Pos2], col: Color32) {
    if col.a() == 0 {
        return;
    }
    // along u = x + y: each stripe is where u is constant
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for q in pts {
        lo = lo.min(q.x + q.y);
        hi = hi.max(q.x + q.y);
    }
    let clip = p.clip_rect();
    // only the stripes that can be on screen
    lo = lo.max(clip.min.x + clip.min.y);
    hi = hi.min(clip.max.x + clip.max.y);
    let gap = 7.0;
    let st = EStroke::new(1.0_f32, col);
    let mut u = (lo / gap).ceil() * gap;
    let mut xs: Vec<f32> = Vec::new();
    while u < hi {
        xs.clear();
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (ua, ub) = (a.x + a.y, b.x + b.y);
            if (ua <= u) != (ub <= u) {
                let t = (u - ua) / (ub - ua);
                xs.push(a.x + (b.x - a.x) * t);
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for c in xs.chunks(2) {
            if let [x0, x1] = c {
                p.line_segment([pos2(*x0, u - x0), pos2(*x1, u - x1)], st);
            }
        }
        u += gap;
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
    s.knots.insert(j + 1, Knot { p: m, i: abc, o: bcd, ..Default::default() });
}

/// A point with handles loses them (sharp); a sharp one gets handles along its neighbours
fn toggle_smooth(s: &mut Stroke, j: usize) {
    let n = s.knots.len();
    let k = s.knots[j];
    if k.i != k.p || k.o != k.p {
        s.knots[j] = Knot { r: k.r, m: k.m, ..Knot::corner(k.p) };
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
    s.knots[j].r = 0.0;
    s.knots[j].i = Pt { x: k.p.x - ux * li, y: k.p.y - uy * li };
    s.knots[j].o = Pt { x: k.p.x + ux * lo, y: k.p.y + uy * lo };
}
