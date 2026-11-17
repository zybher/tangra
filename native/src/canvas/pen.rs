//! Pen tool: click to place points, press and drag to pull out curve handles (Alt: only the
//! outgoing one), click the first point to close the path, Enter (or another tool) to finish
//! it open. The finished path is selected, and double-clicking it edits its points
//! (path_edit.rs).

use crate::*;

/// The path being placed
pub(crate) struct PenDraft {
    knots: Vec<Knot>,
    /// pressing / dragging out the newest point's handles
    drag: bool,
    /// the press landed on the first point: closes on release
    closing: bool,
}

/// How near (screen px) the first point a click closes the path
const SNAP_PX: f32 = 8.0;

impl App {
    /// Pen tool input; true while the pen has the pointer (everything else leaves it alone)
    pub(crate) fn pen_input(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect, space: bool) -> bool {
        if self.tool != Tool::Pen {
            // switching tools finishes the path where it is
            self.pen_finish(false, false);
            return false;
        }
        let v = self.view(rect);
        let (pressed, down, released, hover, alt) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released(), i.pointer.hover_pos(), i.modifiers.alt));
        if ctx.input(|i| i.key_pressed(Key::Enter)) && !ctx.wants_keyboard_input() {
            self.pen_finish(false, true);
            return true;
        }
        if space {
            return true;
        }
        if pressed {
            if let Some(q) = hover.filter(|q| resp.contains_pointer() && rect.contains(*q) && !ui_blocks(ctx, *q)) {
                let d = self.pen.get_or_insert_with(|| PenDraft { knots: Vec::new(), drag: false, closing: false });
                let first = d.knots.first().map(|k| v.s(k.p.x, k.p.y));
                if d.knots.len() >= 2 && first.is_some_and(|f| f.distance(q) < SNAP_PX) {
                    d.closing = true;
                } else {
                    d.knots.push(Knot::corner(v.w(q)));
                }
                d.drag = true;
            }
        }
        if let (Some(d), Some(q)) = (self.pen.as_mut(), hover) {
            if d.drag && down && !d.knots.is_empty() {
                let j = if d.closing { 0 } else { d.knots.len() - 1 };
                let k = &mut d.knots[j];
                // a plain click stays a sharp corner; dragging pulls the handles out
                if v.s(k.p.x, k.p.y).distance(q) > 3.0 {
                    let w = v.w(q);
                    k.o = w;
                    if !alt {
                        k.i = Pt { x: 2.0 * k.p.x - w.x, y: 2.0 * k.p.y - w.y };
                    }
                }
            }
        }
        if released {
            let close = self.pen.as_mut().is_some_and(|d| {
                d.drag = false;
                d.closing
            });
            if close {
                self.pen_finish(true, true);
            }
        }
        true
    }

    /// Puts the path on the board (2+ points), selected and back on the Select tool when `select`
    pub(crate) fn pen_finish(&mut self, closed: bool, select: bool) {
        let Some(d) = self.pen.take() else { return };
        if d.knots.len() < 2 {
            return;
        }
        let mut s = Stroke {
            id: new_id(),
            color: self.pen_color.clone(),
            width: self.pen_width,
            opacity: Some(1.0),
            created_at: now_ms(),
            knots: d.knots,
            closed,
            ..Default::default()
        };
        s.reflatten();
        let id = s.id.clone();
        self.snapshot();
        self.board.world_strokes.push(s);
        self.mark_dirty();
        if select {
            self.tool = Tool::Select;
            self.select_only((Kind::Stroke, id));
        }
    }

    /// The path being placed, with a line on to the pointer and its points
    pub(crate) fn paint_pen(&self, p: &egui::Painter, v: View, hover: Option<Pos2>) {
        let Some(d) = &self.pen else { return };
        let mut s = Stroke {
            id: "qd_pen_draft".into(),
            color: self.pen_color.clone(),
            width: self.pen_width,
            opacity: Some(1.0),
            knots: d.knots.clone(),
            closed: d.closing,
            ..Default::default()
        };
        let mut near_first = false;
        if let (false, Some(q)) = (d.drag || self.tool != Tool::Pen, hover.filter(|q| p.clip_rect().contains(*q))) {
            let first = s.knots.first().map(|k| v.s(k.p.x, k.p.y));
            near_first = s.knots.len() >= 2 && first.is_some_and(|f| f.distance(q) < SNAP_PX);
            if near_first {
                s.closed = true;
            } else {
                s.knots.push(Knot::corner(v.w(q)));
            }
        }
        s.reflatten();
        paint_stroke(p, &s, |q| v.s(q.x, q.y), s.width * v.cam.zoom);
        let last = d.knots.len().checked_sub(1);
        super::path_edit::paint_knots(p, v, &d.knots, if near_first { Some(0) } else { last });
    }
}
