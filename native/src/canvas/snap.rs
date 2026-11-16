//! Smart move: snapping to other objects' edges/centres (with guide lines), Shift axis lock,
//! and Alt-hover distance measurement between the selection and another object.

use crate::*;

/// Figma-style measurement red
fn guide_col() -> Color32 {
    Color32::from_rgb(242, 78, 30)
}

/// Pixels on screen within which an edge/centre snaps
const SNAP_PX: f32 = 6.0;

/// Round a gap to a tidy number (100 > 50 > 10, bigger ones pull from further away), but only
/// while the pointer moves slowly: a quick drag means "just move it", a careful one means
/// "land on a nice number".
pub(crate) fn round_gap(gap: f32, zoom: f32) -> Option<f32> {
    let speed = f32::from_bits(PTR_SPEED.load(std::sync::atomic::Ordering::Relaxed));
    if speed > 900.0 {
        return None;
    }
    let z = zoom.max(0.01);
    // Pull zones are small in world units too, so the numbers in between stay reachable
    [(100.0_f32, 7.0_f32, 4.0_f32), (50.0, 5.0, 3.0), (10.0, 3.0, 1.5)].into_iter().find_map(|(step, px, cap)| {
        let t = (gap / step).round() * step;
        (t > 0.0 && (t - gap).abs() <= (px / z).min(cap)).then_some(t)
    })
}

/// Objects in the same row (`axis` 0: overlapping vertically) or column (`axis` 1) as box `m`,
/// as [start, end, cross_start, cross_end] along that axis, sorted
pub(crate) fn lane(axis: usize, m: [f32; 4], others: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let (a, c) = (axis, 1 - axis);
    let mut v: Vec<[f32; 4]> = others
        .iter()
        .map(|o| [o[a], o[a] + o[a + 2], o[c], o[c] + o[c + 2]])
        .filter(|o| o[2] < m[c + 2] && o[3] > m[c])
        .collect();
    v.sort_by(|p, q| p[0].total_cmp(&q[0]));
    v
}

/// Equal-gap snap along `axis` for the moving box `m` ([x0, y0, x1, y1]): extend an existing gap
/// from the nearest neighbour, or centre between two neighbours. Returns (correction, gap).
fn equal_gap(axis: usize, m: [f32; 4], others: &[[f32; 4]], extra: &[f32], tol: f32) -> Option<(f32, f32)> {
    let row = lane(axis, m, others);
    let (m0, m1) = (m[axis], m[axis + 2]);
    let (len, mid) = (m1 - m0, (m0 + m1) / 2.0);
    // Gaps between neighbours already in the row
    // plus the gaps inside the moving selection itself
    let gaps: Vec<f32> = row.windows(2).map(|w| w[1][0] - w[0][1]).chain(extra.iter().copied()).filter(|g| *g > 0.5).collect();
    let left = row.iter().filter(|o| o[1] <= mid).max_by(|p, q| p[1].total_cmp(&q[1]));
    let right = row.iter().filter(|o| o[0] >= mid).min_by(|p, q| p[0].total_cmp(&q[0]));
    let mut best: Option<(f32, f32)> = None;
    let mut take = |c: f32, g: f32| {
        if g > 0.5 && c.abs() < tol && best.map_or(true, |b| c.abs() < b.0.abs()) {
            best = Some((c, g));
        }
    };
    for &g in &gaps {
        if let Some(l) = left {
            take(l[1] + g - m0, g);
        }
        if let Some(r) = right {
            take(r[0] - g - m1, g);
        }
    }
    if let (Some(l), Some(r)) = (left, right) {
        let g = (r[0] - l[1] - len) / 2.0;
        take(l[1] + g - m0, g);
    }
    best
}

/// Markers for every gap of size `g` in the moving box's row/column (box included), as
/// world segments drawn across the middle of each pair's overlap
fn gap_segments(axis: usize, m: [f32; 4], others: &[[f32; 4]], g: f32) -> Vec<[f32; 4]> {
    let c = 1 - axis;
    let mut row = lane(axis, m, others);
    row.push([m[axis], m[axis + 2], m[c], m[c + 2]]);
    row.sort_by(|p, q| p[0].total_cmp(&q[0]));
    row.windows(2)
        .filter(|w| (w[1][0] - w[0][1] - g).abs() < 0.75)
        .map(|w| {
            let at = (w[0][2].max(w[1][2]) + w[0][3].min(w[1][3])) / 2.0;
            if axis == 0 { [w[0][1], at, w[1][0], at] } else { [at, w[0][1], at, w[1][0]] }
        })
        .collect()
}

impl App {
    /// Adjusts a move delta: Shift locks to the dominant axis, then edges and centres snap to
    /// other objects (hold Ctrl to move freely). Records guide lines for `paint_snap`.
    pub(crate) fn smart_move(&mut self, origins: &[(Sel, Pt)], start: Pt, w: Pt) -> (f32, f32) {
        let (mut dx, mut dy) = (w.x - start.x, w.y - start.y);
        let (lock_x, lock_y) = if self.shift {
            if dx.abs() >= dy.abs() { (false, true) } else { (true, false) }
        } else {
            (false, false)
        };
        if lock_x {
            dx = 0.0;
        }
        if lock_y {
            dy = 0.0;
        }
        self.snap_guides.clear();
        self.gap_marks.clear();
        // Bounding box of what's moving, at its origin
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (s, o) in origins {
            if let Some([_, _, ww, hh]) = self.rect_of(s.0, &s.1) {
                (x0, y0, x1, y1) = (x0.min(o.x), y0.min(o.y), x1.max(o.x + ww), y1.max(o.y + hh));
            }
        }
        if x1 < x0 || self.snap_off {
            return (dx, dy);
        }
        let moving: HashSet<&Sel> = origins.iter().map(|(s, _)| s).collect();
        // Only objects the user can see count: nothing snaps to something off screen
        let others = self.visible_others(&moving);
        let tol = SNAP_PX / self.board.camera.zoom.max(0.01);
        let (w0, h0) = (x1 - x0, y1 - y0);
        let (cx, cy) = (x0 + dx + w0 / 2.0, y0 + dy + h0 / 2.0);
        // Distance from the moving box's centre to an object's box (0 if overlapping)
        let dist = |o: &[f32; 4]| {
            let ex = (o[0] - cx).max(cx - (o[0] + o[2])).max(0.0);
            let ey = (o[1] - cy).max(cy - (o[1] + o[3])).max(0.0);
            ex.hypot(ey)
        };
        // Best per axis: (correction, guide line position, distance, object). The smallest
        // correction wins; near-ties go to the closest object.
        let better = |c: f32, d: f32, b: &Option<(f32, f32, f32, usize)>| {
            b.map_or(true, |(bc, _, bd, _)| c.abs() < bc.abs() - 0.5 || ((c.abs() - bc.abs()).abs() <= 0.5 && d < bd))
        };
        let mut bx: Option<(f32, f32, f32, usize)> = None;
        let mut by: Option<(f32, f32, f32, usize)> = None;
        for (oi, o) in others.iter().enumerate() {
            let (ox, oy, ow, oh) = (o[0], o[1], o[2], o[3]);
            let d = dist(o);
            if !lock_x {
                let mx = x0 + dx;
                for m in [mx, mx + w0 / 2.0, mx + w0] {
                    for t in [ox, ox + ow / 2.0, ox + ow] {
                        let c = t - m;
                        if c.abs() < tol && better(c, d, &bx) {
                            bx = Some((c, t, d, oi));
                        }
                    }
                }
            }
            if !lock_y {
                let my = y0 + dy;
                for m in [my, my + h0 / 2.0, my + h0] {
                    for t in [oy, oy + oh / 2.0, oy + oh] {
                        let c = t - m;
                        if c.abs() < tol && better(c, d, &by) {
                            by = Some((c, t, d, oi));
                        }
                    }
                }
            }
        }
        // Equal gaps: beats an alignment snap on the same axis only if it needs a smaller nudge
        let cur = [x0 + dx, y0 + dy, x0 + dx + w0, y0 + dy + h0];
        // The moving objects themselves (at their current spot) and the gaps between them
        let inner: Vec<[f32; 4]> = origins
            .iter()
            .filter_map(|(s, o)| self.rect_of(s.0, &s.1).map(|r| [o.x + dx, o.y + dy, r[2], r[3]]))
            .collect();
        let inner_gaps = |axis: usize| -> Vec<f32> { lane(axis, cur, &inner).windows(2).map(|w| w[1][0] - w[0][1]).collect() };
        let gx = if lock_x { None } else { equal_gap(0, cur, &others, &inner_gaps(0), tol) };
        let gy = if lock_y { None } else { equal_gap(1, cur, &others, &inner_gaps(1), tol) };
        let mut marks: Vec<(usize, f32)> = Vec::new();
        if let Some((c, g)) = gx {
            if bx.map_or(true, |b| c.abs() <= b.0.abs()) {
                bx = None;
                dx += c;
                marks.push((0, g));
            }
        }
        if let Some((c, g)) = gy {
            if by.map_or(true, |b| c.abs() <= b.0.abs()) {
                by = None;
                dy += c;
                marks.push((1, g));
            }
        }
        if let Some((c, ..)) = bx {
            dx += c;
        }
        if let Some((c, ..)) = by {
            dy += c;
        }
        // Alt-drag duplicate: on an axis nothing else snapped, round the gap to the original to a
        // tidy number. Bigger round numbers pull from further away (100 > 50 > 10), so it helps
        // without fighting small adjustments.
        let snapped = (bx.is_some() || marks.iter().any(|m| m.0 == 0), by.is_some() || marks.iter().any(|m| m.0 == 1));
        if let Some(o) = self.dup_origin {
            let z = self.board.camera.zoom.max(0.01);
            let ob = [o[0], o[1], o[0] + o[2], o[1] + o[3]];
            for axis in 0..2 {
                if (if axis == 0 { lock_x || snapped.0 } else { lock_y || snapped.1 }) {
                    continue;
                }
                let d = if axis == 0 { dx } else { dy };
                let len = if axis == 0 { w0 } else { h0 };
                let m0 = [x0, y0][axis] + d;
                // Gap and which way it grows when the copy moves forward
                let (gap, sign) = if m0 >= ob[axis + 2] { (m0 - ob[axis + 2], 1.0) } else if m0 + len <= ob[axis] { (ob[axis] - (m0 + len), -1.0) } else { continue };
                if let Some(t) = round_gap(gap, z) {
                    let c = sign * (t - gap);
                    if axis == 0 { dx += c } else { dy += c }
                }
            }
        }
        // Mark every gap of that size in the row/column, including the new one(s)
        let fin = [x0 + dx, y0 + dy, x0 + dx + w0, y0 + dy + h0];
        for (axis, g) in marks {
            self.gap_marks.extend(gap_segments(axis, fin, &others, g));
            // ...and the matching gaps inside the selection
            let c = 1 - axis;
            let sh: Vec<[f32; 4]> = inner.iter().map(|r| { let mut q = *r; q[axis] += if axis == 0 { dx - (cur[0] - x0) } else { dy - (cur[1] - y0) }; let _ = c; q }).collect();
            let row = lane(axis, fin, &sh);
            for w in row.windows(2) {
                if (w[1][0] - w[0][1] - g).abs() < 0.75 {
                    let at = (w[0][2].max(w[1][2]) + w[0][3].min(w[1][3])) / 2.0;
                    self.gap_marks.push(if axis == 0 { [w[0][1], at, w[1][0], at] } else { [at, w[0][1], at, w[1][0]] });
                }
            }
        }
        // Land on whole pixels so measurements never show a 0.6 px offset as "1"
        dx = (x0 + dx).round() - x0;
        dy = (y0 + dy).round() - y0;
        // One guide per axis, to the object that won the snap
        let (mx0, my0) = (x0 + dx, y0 + dy);
        if let Some((_, gx, _, oi)) = bx {
            let o = others[oi];
            self.snap_guides.push([gx, my0.min(o[1]), gx, (my0 + h0).max(o[1] + o[3])]);
        }
        if let Some((_, gy, _, oi)) = by {
            let o = others[oi];
            self.snap_guides.push([mx0.min(o[0]), gy, (mx0 + w0).max(o[0] + o[2]), gy]);
        }
        (dx, dy)
    }

    /// Objects on screen, other than those in `skip`
    pub(crate) fn visible_others(&self, skip: &HashSet<&Sel>) -> Vec<[f32; 4]> {
        let vr = self.last_rect;
        let vw = self.view(vr);
        let (v0, v1) = (vw.w(vr.min), vw.w(vr.max));
        self.all_items()
            .into_iter()
            .filter(|s| !skip.contains(s))
            .filter_map(|s| self.rect_of(s.0, &s.1))
            .filter(|o| o[0] < v1.x && o[0] + o[2] > v0.x && o[1] < v1.y && o[1] + o[3] > v0.y)
            .collect()
    }

    /// While resizing: snaps the new width/height to the size of another object on screen
    /// (the closest match wins). With `keep` the other side follows the ratio `ar` (w / h).
    /// Returns the size plus the objects whose width / height it now matches.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn size_snap(&mut self, me: &Sel, mut nw: f32, mut nh: f32, rx: bool, ry: bool, keep: bool, ar: f32) -> (f32, f32, Option<[f32; 4]>, Option<[f32; 4]>) {
        self.snap_guides.clear();
        if self.snap_off {
            return (nw, nh, None, None);
        }
        let skip: HashSet<&Sel> = [me].into_iter().collect();
        let others = self.visible_others(&skip);
        let tol = SNAP_PX / self.board.camera.zoom.max(0.01);
        // Prefer the nearest object, not the one with the closest size
        let me_r = self.rect_of(me.0, &me.1).unwrap_or([0.0; 4]);
        let (mx, my) = (me_r[0] + me_r[2] / 2.0, me_r[1] + me_r[3] / 2.0);
        let dist = |o: &[f32; 4]| (o[0] + o[2] / 2.0 - mx).hypot(o[1] + o[3] / 2.0 - my);
        // Only likely targets: heights are compared with objects in the same row (overlapping
        // vertically), widths with objects in the same column (overlapping horizontally), and
        // only the few nearest of those
        let row = |o: &[f32; 4]| o[1] < me_r[1] + me_r[3] && o[1] + o[3] > me_r[1];
        let col = |o: &[f32; 4]| o[0] < me_r[0] + me_r[2] && o[0] + o[2] > me_r[0];
        let nearest = |f: &dyn Fn(&[f32; 4]) -> bool| {
            let mut v: Vec<[f32; 4]> = others.iter().filter(|o| f(o)).copied().collect();
            v.sort_by(|a, b| dist(a).total_cmp(&dist(b)));
            v.truncate(3);
            v
        };
        let (in_col, in_row) = (nearest(&col), nearest(&row));
        let best = |cur: f32, f: fn(&[f32; 4]) -> f32| {
            let pool = if f(&[0.0, 0.0, 1.0, 2.0]) == 1.0 { &in_col } else { &in_row };
            pool
                .iter()
                .map(|o| (f(o) - cur, *o))
                .filter(|(c, _)| c.abs() < tol)
                .min_by(|a, b| dist(&a.1).total_cmp(&dist(&b.1)))
        };
        let bw = if rx || keep { best(nw, |o| o[2]) } else { None };
        let bh = if ry || keep { best(nh, |o| o[3]) } else { None };
        if keep {
            // One side drives the other: use whichever match needs the smaller nudge
            let use_w = match (bw, bh) {
                (Some(a), Some(b)) => a.0.abs() <= b.0.abs(),
                (Some(_), None) => true,
                _ => false,
            };
            if use_w {
                nw += bw.unwrap().0;
                nh = (nw / ar).round();
            } else if let Some((c, _)) = bh {
                nh += c;
                nw = (nh * ar).round();
            }
        } else {
            if let Some((c, _)) = bw {
                nw += c;
            }
            if let Some((c, _)) = bh {
                nh += c;
            }
        }
        // Report the closest object that now has the same width / height
        let me_r = self.rect_of(me.0, &me.1).unwrap_or([0.0; 4]);
        let (mx, my) = (me_r[0] + me_r[2] / 2.0, me_r[1] + me_r[3] / 2.0);
        let near = |o: &&[f32; 4]| (o[0] + o[2] / 2.0 - mx).hypot(o[1] + o[3] / 2.0 - my);
        let pick = |f: &dyn Fn(&[f32; 4]) -> bool| others.iter().filter(|o| f(o)).min_by(|a, b| near(a).total_cmp(&near(b))).copied();
        let mw = pick(&|o| (o[2] - nw).abs() < 0.5 && in_col.contains(o));
        let mh = pick(&|o| (o[3] - nh).abs() < 0.5 && in_row.contains(o));
        (nw, nh, mw, mh)
    }

    /// Size-match markers: a measuring bar above both objects (same width) or beside both
    /// (same height), drawn by `paint_snap`
    pub(crate) fn size_guides(&mut self, _r: [f32; 4], mw: Option<[f32; 4]>, mh: Option<[f32; 4]>) {
        self.size_match = (mw, mh);
    }

    /// Size pill colour: red while that side matches another object during a resize
    pub(crate) fn match_col(&self, width: bool) -> Color32 {
        let m = if width { self.size_match.0 } else { self.size_match.1 };
        if m.is_some() && matches!(self.drag, Drag::Resize { .. }) { guide_col() } else { blue() }
    }

    /// Every object on the board
    pub(crate) fn all_items(&self) -> Vec<Sel> {
        let b = &self.board;
        let mut v: Vec<Sel> = Vec::new();
        v.extend(b.images.iter().map(|i| (Kind::Image, i.id.clone())));
        v.extend(b.sticky_notes.iter().map(|i| (Kind::Note, i.id.clone())));
        v.extend(b.link_cards.iter().map(|i| (Kind::Link, i.id.clone())));
        v.extend(b.group_cards.iter().map(|i| (Kind::Group, i.id.clone())));
        v.extend(b.world_texts.iter().map(|i| (Kind::Text, i.id.clone())));
        v.extend(b.world_strokes.iter().map(|i| (Kind::Stroke, i.id.clone())));
        v
    }

    /// Bounding box (world) of the current selection
    fn selection_box(&self) -> Option<[f32; 4]> {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (k, id) in &self.selected {
            if let Some([x, y, w, h]) = self.rect_of(*k, id) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + w), y1.max(y + h));
            }
        }
        (x1 > x0).then(|| [x0, y0, x1 - x0, y1 - y0])
    }

    /// Snap guides while moving, and Alt-hover distances to another object
    pub(crate) fn paint_snap(&self, p: &egui::Painter, v: View, hover: Option<Pos2>) {
        let col = guide_col();
        let st = EStroke::new(1.0_f32, col);
        let cross = |c: Pos2| {
            let d = 3.0;
            p.line_segment([c + vec2(-d, -d), c + vec2(d, d)], st);
            p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], st);
        };
        // Guides fade in and out (the last ones stay drawn while fading out)
        let ctx = p.ctx().clone();
        let gid = egui::Id::new("qd_snap_fade");
        let live = matches!(self.drag, Drag::Move { .. } | Drag::Gap { .. }) && !(self.snap_guides.is_empty() && self.gap_marks.is_empty());
        if live {
            ctx.data_mut(|d| d.insert_temp(gid, (self.snap_guides.clone(), self.gap_marks.clone())));
        }
        let ga = ctx.animate_bool_with_time(gid.with("a"), live, 0.12 * anim_k());
        if ga > 0.01 {
            let (guides, marks) = ctx.data(|d| d.get_temp::<(Vec<[f32; 4]>, Vec<[f32; 4]>)>(gid)).unwrap_or_default();
            let mut p = p.clone();
            p.multiply_opacity(ga);
            let p = &p;
            let cross = |c: Pos2| {
                let d = 3.0;
                p.line_segment([c + vec2(-d, -d), c + vec2(d, d)], st);
                p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], st);
            };
            for g in &guides {
                let (a, b) = (v.s(g[0], g[1]), v.s(g[2], g[3]));
                p.line_segment([a, b], st);
                cross(a);
                cross(b);
            }
            // Equal gaps: a bar with end ticks across each gap, labelled with its size
            for g in &marks {
                let (a, b) = (v.s(g[0], g[1]), v.s(g[2], g[3]));
                let n = (b - a).normalized().rot90() * 4.0;
                p.line_segment([a, b], st);
                p.line_segment([a - n, a + n], st);
                p.line_segment([b - n, b + n], st);
                let len = ((g[2] - g[0]) + (g[3] - g[1])).abs();
                let gl = p.layout_no_wrap(format!("{:.0}", len), semibold(12.0), Color32::WHITE);
                let r = Rect::from_center_size(a.lerp(b, 0.5), gl.size() + vec2(12.0, 6.0));
                p.rect_filled(r, 4.0, col);
                p.galley(r.min + vec2(6.0, 3.0), gl, Color32::WHITE);
            }
        }
        let _ = cross;
        // Size match while resizing: the matched object gets a thin outline and a red pill with
        // the same number, mirroring the (now red) size pill on the object being resized; fades
        let sid = egui::Id::new("qd_size_match_fade");
        let s_live = matches!(self.drag, Drag::Resize { .. }) && (self.size_match.0.is_some() || self.size_match.1.is_some());
        if s_live {
            ctx.data_mut(|d| d.insert_temp(sid, self.size_match));
        }
        let sa = ctx.animate_bool_with_time(sid.with("a"), s_live, 0.12 * anim_k());
        if sa > 0.01 {
            let mut p = p.clone();
            p.multiply_opacity(sa);
            let p = &p;
            let (mw, mh) = ctx.data(|d| d.get_temp::<(Option<[f32; 4]>, Option<[f32; 4]>)>(sid)).unwrap_or((None, None));
            for o in [mw, mh].into_iter().flatten() {
                p.rect_stroke(Rect::from_min_max(v.s(o[0], o[1]), v.s(o[0] + o[2], o[1] + o[3])), 0.0, st);
            }
            let pill = |at: Pos2, n: f32, vertical: bool| {
                let g = p.layout_no_wrap(format!("{:.0}", n), semibold(12.0), Color32::WHITE);
                let sz = if vertical { vec2(g.size().y + 6.0, g.size().x + 12.0) } else { g.size() + vec2(12.0, 6.0) };
                let r = Rect::from_center_size(at, sz);
                p.rect_filled(r, 4.0, col);
                if vertical {
                    let t = egui::epaint::TextShape::new(pos2(r.min.x + 3.0, r.max.y - 6.0), g, Color32::WHITE).with_angle(-std::f32::consts::FRAC_PI_2);
                    p.add(t);
                } else {
                    p.galley(r.min + vec2(6.0, 3.0), g, Color32::WHITE);
                }
            };
            if let Some(o) = mw {
                pill(v.s(o[0] + o[2] / 2.0, o[1]) - vec2(0.0, 14.0), o[2], false);
            }
            if let Some(o) = mh {
                pill(v.s(o[0], o[1] + o[3] / 2.0) - vec2(14.0, 0.0), o[3], true);
            }
        }
        // Alt + hover: Figma-style measurement. From each side of the selection, a ray goes
        // outward; it measures to the nearest edge of the hovered object it meets on that side.
        // Alt-drag duplicate: measure the copy against the original while it moves
        let dup = matches!(self.drag, Drag::Move { .. }).then_some(self.dup_origin).flatten();
        let alt = ALT_DOWN.load(std::sync::atomic::Ordering::Relaxed) && matches!(self.drag, Drag::None);
        let k = p.ctx().animate_bool_with_time(egui::Id::new("qd_alt_measure"), alt || dup.is_some(), 0.15 * anim_k());
        // Keep drawing while it fades out after Alt is released (was cut off on frame 1)
        if k <= 0.001 {
            return;
        }
        let Some(sb) = self.selection_box() else { return };
        let o = if let Some(o) = dup {
            o
        } else {
            let Some(h) = hover else { return };
            let Some(hs) = self.hit_test(v.w(h)) else { return };
            if self.selected.contains(&hs) {
                return;
            }
            let Some(o) = self.rect_of(hs.0, &hs.1) else { return };
            o
        };
        let col = col.gamma_multiply(k);
        let st = EStroke::new(1.0_f32, col);
        let os = Rect::from_min_max(v.s(o[0], o[1]), v.s(o[0] + o[2], o[1] + o[3]));
        p.rect_stroke(os, 0.0, st);
        let dash = |a: Pos2, b: Pos2| {
            p.extend(egui::Shape::dashed_line(&[a, b], st, 5.0, 4.0));
        };
        let label = |at: Pos2, n: f32| {
            let g = p.layout_no_wrap(format!("{:.0}", n), semibold(12.0), Color32::WHITE.gamma_multiply(k));
            let r = Rect::from_center_size(at, g.size() + vec2(12.0, 6.0));
            p.rect_filled(r, 4.0, col);
            p.galley(r.min + vec2(6.0, 3.0), g, Color32::WHITE.gamma_multiply(k));
        };
        // Boxes as [x0, y0, x1, y1]
        let sel = [sb[0], sb[1], sb[0] + sb[2], sb[1] + sb[3]];
        let hov = [o[0], o[1], o[0] + o[2], o[1] + o[3]];
        // Per axis, Figma-style: if the two are apart on that axis, show the gap; if they overlap,
        // show the offset between matching edges (left-left/right-right, top-top/bottom-bottom).
        // Lines sit on the selection's centre line; the hovered object's edges are extended to
        // them as dashed lines when needed.
        // Measure whole pixels, the same rounding the size numbers use
        let sel = sel.map(f32::round);
        let hov = hov.map(f32::round);
        let (cx, cy) = ((sel[0] + sel[2]) / 2.0, (sel[1] + sel[3]) / 2.0);
        // `axis` 0 = horizontal measurements (x), 1 = vertical (y). `pt(along, across)` maps to screen.
        for axis in 0..2 {
            let (a0, a1, b0, b1) = (sel[axis], sel[axis + 2], hov[axis], hov[axis + 2]);
            // Across-axis: the selection's centre line, its span, and the hovered object's span
            let (c, s0, s1, h0, h1) = if axis == 0 { (cy, sel[1], sel[3], hov[1], hov[3]) } else { (cx, sel[0], sel[2], hov[0], hov[2]) };
            let pt = |along: f32, across: f32| if axis == 0 { v.s(along, across) } else { v.s(across, along) };
            let measure = |from: f32, to: f32| {
                p.line_segment([pt(from, c), pt(to, c)], st);
                label(pt((from + to) / 2.0, c), (to - from).abs());
            };
            if a1 <= b0 || b1 <= a0 {
                let (from, to) = if a1 <= b0 { (a1, b0) } else { (a0, b1) };
                measure(from, to);
                // The line passes beside the hovered object: reach its edge to the line
                if c < h0 || c > h1 {
                    dash(pt(to, if c < h0 { h0 } else { h1 }), pt(to, c));
                }
            } else {
                for (from, to) in [(a0, b0), (a1, b1)] {
                    if (to - from).abs() > 0.5 {
                        measure(from, to);
                        // Extend the hovered edge across the selection, past its centre line
                        if c < h0 || c > h1 {
                            let (near, far) = if c < h0 { (h0, s0.min(h0)) } else { (h1, s1.max(h1)) };
                            dash(pt(to, near), pt(to, far));
                        }
                    }
                }
            }
        }
    }
}
