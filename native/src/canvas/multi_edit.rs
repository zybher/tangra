//! Editing several objects at once: resizing a multi-selection from its box (everything scales
//! together, frames bring their contents) and nudging the selection with the arrow keys.

use crate::*;

impl App {
    /// Unselected objects sitting fully inside a selected frame, with the index of that frame in `sels`
    pub(crate) fn frame_contents(&self, sels: &[(Sel, [f32; 4])]) -> Vec<(Sel, [f32; 4], usize)> {
        let mut out = Vec::new();
        for (pi, ((k, _), _)) in sels.iter().enumerate() {
            if *k != Kind::Group {
                continue;
            }
            for s in self.descendants(&sels[pi].0 .1) {
                if self.selected.contains(&s) || out.iter().any(|(o, _, _)| *o == s) {
                    continue;
                }
                if let Some(q) = self.rect_of(s.0, &s.1) {
                    out.push((s, q, pi));
                }
            }
        }
        out
    }

    pub(crate) fn selected_rects(&self) -> Vec<(Sel, [f32; 4])> {
        self.selected.iter().filter_map(|s| self.rect_of(s.0, &s.1).map(|r| (s.clone(), r))).collect()
    }

    /// Box of a multi-selection, or None for 0/1 objects
    pub(crate) fn multi_box(&self) -> Option<[f32; 4]> {
        if self.selected.len() < 2 {
            return None;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (_, [x, y, w, h]) in self.selected_rects() {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + w), y1.max(y + h));
        }
        (x1 > x0).then(|| [x0, y0, x1 - x0, y1 - y0])
    }

    /// Which corner/edge of the multi-selection box is under `w`: (left, top, rx, ry)
    pub(crate) fn multi_handle_at(&self, w: Pt) -> Option<(bool, bool, bool, bool)> {
        let [x, y, wd, h] = self.multi_box()?;
        let grab = 12.0 / self.board.camera.zoom.max(0.01);
        for (left, top) in [(false, false), (true, false), (false, true), (true, true)] {
            let (cx, cy) = (if left { x } else { x + wd }, if top { y } else { y + h });
            if (w.x - cx).abs() < grab && (w.y - cy).abs() < grab {
                return Some((left, top, true, true));
            }
        }
        let (in_x, in_y) = (w.x > x && w.x < x + wd, w.y > y && w.y < y + h);
        let g = grab * 0.7;
        if in_y && (w.x - x).abs() < g {
            Some((true, false, true, false))
        } else if in_y && (w.x - x - wd).abs() < g {
            Some((false, false, true, false))
        } else if in_x && (w.y - y).abs() < g {
            Some((false, true, false, true))
        } else if in_x && (w.y - y - h).abs() < g {
            Some((false, false, false, true))
        } else {
            None
        }
    }

    pub(crate) fn begin_multi_resize(&mut self, w: Pt, (left, top, rx, ry): (bool, bool, bool, bool)) {
        let Some(bx) = self.multi_box() else { return };
        let items = self.selected_rects();
        let follow = self.frame_contents(&items);
        self.drag_board = Some(self.board.clone());
        self.drag = Drag::MultiResize { start: w, bx, left, top, rx, ry, items, follow };
    }

    /// Scale everything in the selection with its box. Shift keeps proportions, Alt grows from the centre.
    pub(crate) fn continue_multi_resize(&mut self, w: Pt) {
        let Drag::MultiResize { start, bx, left, top, rx, ry, items, follow } = self.drag.clone() else { return };
        let alt = ALT_DOWN.load(std::sync::atomic::Ordering::Relaxed);
        let grow = if alt { 2.0 } else { 1.0 };
        let (sx, sy) = (if left { -1.0 } else { 1.0 }, if top { -1.0 } else { 1.0 });
        let [ox, oy, ow, oh] = bx;
        let mut nw = if rx { (ow + grow * sx * (w.x - start.x)).max(20.0).round() } else { ow };
        let mut nh = if ry { (oh + grow * sy * (w.y - start.y)).max(20.0).round() } else { oh };
        if self.shift || (rx && ry && items.iter().any(|((k, _), _)| *k == Kind::Image)) {
            let f = if rx && ry { (nw / ow).max(nh / oh) } else if rx { nw / ow } else { nh / oh };
            (nw, nh) = ((ow * f).round(), (oh * f).round());
        }
        let nx = if alt { ox + (ow - nw) / 2.0 } else if left { ox + ow - nw } else { ox };
        let ny = if alt { oy + (oh - nh) / 2.0 } else if top { oy + oh - nh } else { oy };
        let (fx, fy) = (nw / ow, nh / oh);
        let map = |r: [f32; 4]| [nx + (r[0] - ox) * fx, ny + (r[1] - oy) * fy, (r[2] * fx).max(1.0), (r[3] * fy).max(1.0)];
        for (s, r) in &items {
            self.set_rect(s.0, &s.1, map(*r));
        }
        for (s, q, _) in &follow {
            self.set_rect(s.0, &s.1, map(*q));
        }
        self.drag_changed = true;
    }

    /// Corner dots on the multi-selection box, plus a resize cursor over its handles
    pub(crate) fn paint_multi_handles(&self, p: &egui::Painter, v: View, hover: Option<Pos2>) {
        if self.tool != Tool::Select || !matches!(self.drag, Drag::None | Drag::MultiResize { .. }) {
            return;
        }
        let Some([x, y, w, h]) = self.multi_box() else { return };
        for (cx, cy) in [(x, y), (x + w, y), (x, y + h), (x + w, y + h)] {
            let c = v.s(cx, cy);
            p.circle_filled(c, 4.0, Color32::WHITE);
            p.circle_stroke(c, 4.0, egui::Stroke::new(1.5_f32, blue()));
        }
        let hit = match &self.drag {
            Drag::MultiResize { left, top, rx, ry, .. } => Some((*left, *top, *rx, *ry)),
            _ => hover.filter(|hp| !ui_blocks(p.ctx(), *hp)).and_then(|hp| (self.gap_handle_at(v.w(hp)).is_none()).then(|| self.multi_handle_at(v.w(hp))).flatten()),
        };
        if let Some((left, top, rx, ry)) = hit {
            use egui::CursorIcon as C;
            p.ctx().set_cursor_icon(match (rx, ry) {
                (true, true) if left == top => C::ResizeNwSe,
                (true, true) => C::ResizeNeSw,
                (true, false) => C::ResizeHorizontal,
                _ => C::ResizeVertical,
            });
        }
    }

    /// Arrow keys move the selection 1 px (Shift: 10 px); frames bring their contents
    pub(crate) fn nudge(&mut self, dx: f32, dy: f32) {
        if self.selected.is_empty() || !matches!(self.drag, Drag::None) {
            return;
        }
        self.snapshot();
        let items = self.selected_rects();
        let follow = self.frame_contents(&items);
        for (s, r) in items.iter().map(|(s, r)| (s, *r)).chain(follow.iter().map(|(s, q, _)| (s, *q))) {
            self.set_rect(s.0, &s.1, [r[0] + dx, r[1] + dy, r[2], r[3]]);
        }
        self.mark_dirty();
    }
}
