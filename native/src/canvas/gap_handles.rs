//! Bulk spacing: when the selected objects sit in a row or column with equal gaps, a handle
//! appears in each gap; dragging any of them changes every gap at once (frames bring their contents).

use crate::*;

pub(crate) struct GapRun {
    pub axis: usize,
    pub gap: f32,
    pub items: Vec<(Sel, [f32; 4])>,
}

impl App {
    /// The selection as an evenly spaced row/column, if it is one
    pub(crate) fn gap_run(&self) -> Option<GapRun> {
        if self.selected.len() < 2 {
            return None;
        }
        let mut items: Vec<(Sel, [f32; 4])> =
            self.selected.iter().filter_map(|s| self.rect_of(s.0, &s.1).map(|r| (s.clone(), r))).collect();
        for axis in 0..2 {
            items.sort_by(|a, b| a.1[axis].total_cmp(&b.1[axis]));
            let gaps: Vec<f32> = items.windows(2).map(|w| w[1].1[axis] - (w[0].1[axis] + w[0].1[axis + 2])).collect();
            if gaps.iter().all(|g| *g >= 0.0 && (g - gaps[0]).abs() < 0.5) {
                return Some(GapRun { axis, gap: gaps[0], items });
            }
        }
        None
    }

    /// World-space handle bars, one per gap (centred where the two neighbours overlap)
    pub(crate) fn gap_handle_rects(&self, run: &GapRun) -> Vec<[f32; 4]> {
        let z = self.board.camera.zoom.max(0.01);
        let (a, c) = (run.axis, 1 - run.axis);
        let (thick, len) = (4.0 / z, 22.0 / z);
        run.items
            .windows(2)
            .map(|w| {
                let (r0, r1) = (w[0].1, w[1].1);
                let mid = (r0[a] + r0[a + 2] + r1[a]) / 2.0;
                let (lo, hi) = (r0[c].max(r1[c]), (r0[c] + r0[c + 2]).min(r1[c] + r1[c + 2]));
                let cm = if hi > lo { (lo + hi) / 2.0 } else { (r0[c] + r0[c + 2] / 2.0 + r1[c] + r1[c + 2] / 2.0) / 2.0 };
                let mut r = [0.0; 4];
                r[a] = mid - thick / 2.0;
                r[a + 2] = thick;
                r[c] = cm - len / 2.0;
                r[c + 2] = len;
                r
            })
            .collect()
    }

    /// The gap handle under a world point (with a few screen px of slack)
    pub(crate) fn gap_handle_at(&self, w: Pt) -> Option<(GapRun, usize)> {
        if ALT_DOWN.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
        let run = self.gap_run()?;
        let pad = 5.0 / self.board.camera.zoom.max(0.01);
        let i = self.gap_handle_rects(&run).iter().position(|r| {
            w.x >= r[0] - pad && w.x <= r[0] + r[2] + pad && w.y >= r[1] - pad && w.y <= r[1] + r[3] + pad
        })?;
        Some((run, i))
    }

    /// The point is inside one of the selection's gaps (the space between two neighbours, across
    /// the part where they face each other), with the handles' slack
    pub(crate) fn in_gap_zone(&self, w: Pt) -> bool {
        let Some(run) = self.gap_run() else { return false };
        let (a, c) = (run.axis, 1 - run.axis);
        let pad = 5.0 / self.board.camera.zoom.max(0.01);
        let p = [w.x, w.y];
        run.items.windows(2).any(|q| {
            let (r0, r1) = (q[0].1, q[1].1);
            let (lo, hi) = (r0[c].max(r1[c]), (r0[c] + r0[c + 2]).min(r1[c] + r1[c + 2]));
            let (lo, hi) = if hi > lo { (lo, hi) } else { (r0[c].min(r1[c]), (r0[c] + r0[c + 2]).max(r1[c] + r1[c + 2])) };
            p[a] >= r0[a] + r0[a + 2] - pad && p[a] <= r1[a] + pad && p[c] >= lo && p[c] <= hi
        })
    }

    /// Start a bulk-gap drag; frames' contents are collected so they travel with them
    pub(crate) fn begin_gap_drag(&mut self, run: GapRun, idx: usize, w: Pt) {
        let mut follow = Vec::new();
        for (pi, ((k, _), r)) in run.items.iter().enumerate() {
            if *k != Kind::Group {
                continue;
            }
            for s in self.all_items() {
                if self.selected.contains(&s) {
                    continue;
                }
                if let Some(q) = self.rect_of(s.0, &s.1) {
                    if q[0] >= r[0] && q[1] >= r[1] && q[0] + q[2] <= r[0] + r[2] && q[1] + q[3] <= r[1] + r[3] {
                        follow.push((s, q, pi));
                    }
                }
            }
        }
        self.drag_board = Some(self.board.clone());
        self.drag = Drag::Gap { idx, axis: run.axis, start: w, gap0: run.gap, items: run.items, follow };
    }

    pub(crate) fn continue_gap_drag(&mut self, w: Pt) {
        let Drag::Gap { idx, axis, start, gap0, items, follow } = self.drag.clone() else { return };
        let d = if axis == 0 { w.x - start.x } else { w.y - start.y };
        let raw = (gap0 + d).max(0.0).round();
        // Lay the row out with gap `g` (returns new [x, y, w, h] per item)
        // Shift: only the grabbed gap changes (everything after it slides), others keep theirs
        // Alt: the row grows / shrinks evenly out from its centre instead of from its first item
        let one = self.shift;
        let mid = ALT_DOWN.load(std::sync::atomic::Ordering::Relaxed) && !one;
        let (first, last) = (items[0].1, items[items.len() - 1].1);
        let centre = (first[axis] + last[axis] + last[axis + 2]) / 2.0;
        let sizes: f32 = items.iter().map(|(_, r)| r[axis + 2]).sum();
        let layout = |g: f32| -> Vec<[f32; 4]> {
            let mut pos = if mid { centre - (sizes + g * (items.len() - 1) as f32) / 2.0 } else { items[0].1[axis] };
            items
                .iter()
                .enumerate()
                .map(|(i, (_, r))| {
                    let mut nr = *r;
                    if one {
                        nr[axis] = if i > idx { r[axis] + g - gap0 } else { r[axis] };
                    } else {
                        nr[axis] = pos;
                        pos += r[axis + 2] + g;
                    }
                    nr
                })
                .collect()
        };
        // Other objects on screen in the same row/column
        let skip: HashSet<&Sel> = items.iter().map(|i| &i.0).chain(follow.iter().map(|f| &f.0)).collect();
        let others = self.visible_others(&skip);
        let z = self.board.camera.zoom.max(0.01);
        // Gaps in the row (with the run laid out at `g`) other than the run's own
        let equal_at = |g: f32| -> Vec<[f32; 4]> {
            let rs = layout(g);
            let (mut b0, mut b1) = ([f32::MAX; 2], [f32::MIN; 2]);
            for r in &rs {
                for k in 0..2 {
                    b0[k] = b0[k].min(r[k]);
                    b1[k] = b1[k].max(r[k] + r[k + 2]);
                }
            }
            let m = [b0[0], b0[1], b1[0], b1[1]];
            let mut all = others.clone();
            all.extend(rs.iter().copied());
            let row = super::snap::lane(axis, m, &all);
            let mine = |o: &[f32; 4]| rs.iter().any(|r| (r[axis] - o[0]).abs() < 0.01 && (r[axis] + r[axis + 2] - o[1]).abs() < 0.01);
            // Only the matching gaps outside the run (the run's own are labelled by the handles)
            row.windows(2)
                .filter(|p| !(mine(&p[0]) && mine(&p[1])) && (p[1][0] - p[0][1] - g).abs() < 0.5)
                .map(|p| {
                    let at = (p[0][2].max(p[1][2]) + p[0][3].min(p[1][3])) / 2.0;
                    if axis == 0 { [p[0][1], at, p[1][0], at] } else { [at, p[0][1], at, p[1][0]] }
                })
                .collect()
        };
        // Gaps between other on-screen objects in any row/column (not just this one), so a run
        // can match spacing used elsewhere, e.g. the row above
        let mut refs: Vec<(f32, [f32; 4])> = vec![];
        for o in &others {
            let ob = [o[0], o[1], o[0] + o[2], o[1] + o[3]];
            for p in super::snap::lane(axis, ob, &others).windows(2) {
                let gv = p[1][0] - p[0][1];
                let at = (p[0][2].max(p[1][2]) + p[0][3].min(p[1][3])) / 2.0;
                let mk = if axis == 0 { [p[0][1], at, p[1][0], at] } else { [at, p[0][1], at, p[1][0]] };
                if gv > 0.5 && !refs.iter().any(|(_, m)| m == &mk) {
                    refs.push((gv, mk));
                }
            }
        }
        // Matching another gap in the row wins, then a round number, else free
        let reach = (6.0 / z).min(6.0).ceil() as i32;
        let mut g = raw;
        let mut marks = vec![];
        let mut cands: Vec<i32> = (-reach..=reach).collect();
        cands.sort_by_key(|c| c.abs());
        if !self.snap_off {
            for c in cands {
                let t = raw + c as f32;
                if t < 0.0 {
                    continue;
                }
                let mut m = equal_at(t);
                m.extend(refs.iter().filter(|(gv, _)| (gv - t).abs() < 0.5).map(|(_, mk)| *mk));
                if !m.is_empty() {
                    (g, marks) = (t, m);
                    break;
                }
            }
            if marks.is_empty() {
                if let Some(t) = super::snap::round_gap(raw, z) {
                    g = t;
                }
            }
        }
        self.snap_guides.clear();
        self.gap_marks = marks;
        for (((s, r), nr), _) in items.iter().zip(layout(g)).zip(0..) {
            let sh = nr[axis] - r[axis];
            self.set_rect(s.0, &s.1, nr);
            for (fs, q, pi) in &follow {
                if items[*pi].0 == *s {
                    let mut nq = *q;
                    nq[axis] += sh;
                    self.set_rect(fs.0, &fs.1, nq);
                }
            }
        }
        self.drag_changed = true;
    }

    /// Handles plus the gap number; hovering lights up the gap and shows a resize cursor
    pub(crate) fn paint_gap_handles(&self, p: &egui::Painter, v: View, hover: Option<Pos2>) {
        let dragging = matches!(self.drag, Drag::Gap { .. });
        // Alt hides them (it measures instead), except mid-drag where it spreads from the centre
        let allowed = dragging || (matches!(self.drag, Drag::None) && !ALT_DOWN.load(std::sync::atomic::Ordering::Relaxed));
        let run = if allowed { self.gap_run() } else { None };
        // Handles fade and grow in when an evenly spaced run is selected
        let show = p.ctx().animate_bool_with_time(egui::Id::new("qd_gap_show"), run.is_some(), 0.12 * anim_k());
        let Some(run) = run else { return };
        let mut p = p.clone();
        p.multiply_opacity(show);
        let p = &p;
        let col = spacing_col();
        let hot = hover.and_then(|h| self.gap_handle_at(v.w(h))).map(|(_, i)| i);
        if hot.is_some() || dragging {
            p.ctx().set_cursor_icon(if run.axis == 0 { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical });
        }
        let (a, c) = (run.axis, 1 - run.axis);
        for (i, (r, pair)) in self.gap_handle_rects(&run).into_iter().zip(run.items.windows(2)).enumerate() {
            let on = dragging || hot == Some(i);
            // Per gap, so moving between neighbouring gaps crossfades instead of snapping
            let t = p.ctx().animate_bool(egui::Id::new(("qd_gap_hot", run.axis, i)), on);
            // The gap itself: tinted, with a hatch so it reads as "space"
            if on && t > 0.0 {
                let (r0, r1) = (pair[0].1, pair[1].1);
                let (lo, hi) = (r0[c].max(r1[c]), (r0[c] + r0[c + 2]).min(r1[c] + r1[c + 2]));
                let (lo, hi) = if hi > lo { (lo, hi) } else { (r0[c].min(r1[c]), (r0[c] + r0[c + 2]).max(r1[c] + r1[c + 2])) };
                let mut g = [0.0; 4];
                g[a] = r0[a] + r0[a + 2];
                g[a + 2] = r1[a] - g[a];
                g[c] = lo;
                g[c + 2] = hi - lo;
                let gr = Rect::from_min_max(v.s(g[0], g[1]), v.s(g[0] + g[2], g[1] + g[3]));
                p.rect_filled(gr, 0.0, col.gamma_multiply(0.22 * t));
                let clip = p.with_clip_rect(gr);
                let st = egui::Stroke::new(1.0_f32, col.gamma_multiply(0.6 * t));
                let mut k = gr.min.x - gr.height();
                while k < gr.max.x {
                    clip.line_segment([pos2(k, gr.max.y), pos2(k + gr.height(), gr.min.y)], st);
                    k += 7.0;
                }
            }
            // The handle: a pill with a white outline, bigger when live
            let sr = Rect::from_min_max(v.s(r[0], r[1]), v.s(r[0] + r[2], r[1] + r[3]));
            let grow = (if on { 1.0 + 0.3 * t } else { 1.0 }) * (0.6 + 0.4 * show);
            let size = if a == 0 { vec2(sr.width() * grow, sr.height() * grow) } else { vec2(sr.width() * grow, sr.height() * grow) };
            let sr = Rect::from_center_size(sr.center(), size);
            let rad = sr.width().min(sr.height()) / 2.0;
            p.rect_filled(sr, rad, if on { col } else { col.gamma_multiply(0.85) });
            if hot == Some(i) || dragging {
                let gap = (pair[1].1[a] - (pair[0].1[a] + pair[0].1[a + 2])).round();
                let gl = p.layout_no_wrap(format!("{}", gap as i64), semibold(12.0), Color32::WHITE);
                let hp = hover.unwrap_or(sr.center());
                // Dragging: a number in every gap, below its handle; hovering: beside the pointer
                let cpos = if dragging {
                    if a == 0 { pos2(sr.center().x, sr.max.y + 16.0) } else { pos2(sr.max.x + 12.0 + gl.size().x / 2.0, sr.center().y) }
                } else if a == 0 {
                    pos2(sr.max.x + 10.0 + gl.size().x / 2.0, hp.y - 14.0)
                } else {
                    pos2(hp.x + 14.0 + gl.size().x / 2.0, sr.max.y + 12.0)
                };
                // The number badge fades in with the gap's hover
                let br = Rect::from_center_size(cpos, gl.size() + vec2(10.0, 4.0));
                p.rect_filled(br, 4.0, col.gamma_multiply(t));
                p.galley(br.center() - gl.size() / 2.0, gl, Color32::WHITE.gamma_multiply(t));
            }
        }
    }
}
