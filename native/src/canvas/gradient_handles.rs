//! Editing a gradient fill: its two handles on the canvas (drag to move where it starts and
//! ends) and the helpers the properties panel uses to change its kind and colour stops.
//! The handles show while a stop is picked in the panel (`App::grad_edit`) and the object is
//! the only thing selected.

use crate::*;

const GRAB_PX: f32 = 9.0;

impl App {
    /// The gradient fill of a shape / frame, if it has one
    pub(crate) fn gradient_of(&self, id: &str) -> Option<Gradient> {
        self.find_stroke(id).and_then(|s| s.gradient.clone()).or_else(|| self.board.group_cards.iter().find(|g| g.id == id).and_then(|g| g.gradient.clone()))
    }

    /// Change a shape's / frame's gradient (None = solid colour); `f` also gets its colour,
    /// to start a new gradient from
    pub(crate) fn edit_gradient(&mut self, id: &str, f: impl FnOnce(&mut Option<Gradient>, &str)) {
        if let Some(s) = self.board.world_strokes.iter_mut().find(|s| s.id == id) {
            f(&mut s.gradient, &s.color.clone());
            if s.gradient.is_some() {
                s.fill = true;
            }
        } else if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
            let base = if g.color.is_empty() || g.color.eq_ignore_ascii_case("none") || g.color.eq_ignore_ascii_case("#1e293b") { "#7c3aed".to_string() } else { g.color.clone() };
            f(&mut g.gradient, &base);
        } else {
            return;
        }
        self.mark_dirty();
    }

    /// The object's box on screen (what the handles are fractions of)
    fn gradient_box(&self, id: &str, v: View) -> Option<Rect> {
        if let Some(s) = self.find_stroke(id) {
            let pts: Vec<Pos2> = s.points.iter().map(|q| v.s(q.x, q.y)).collect();
            return (!pts.is_empty()).then(|| Rect::from_points(&pts));
        }
        let g = self.board.group_cards.iter().find(|g| g.id == id)?;
        Some(Rect::from_min_max(v.s(g.x, g.y), v.s(g.x + g.width, g.y + g.height)))
    }

    /// Which gradient's handles show right now (dropped once the object isn't the only selection)
    fn grad_target(&mut self) -> Option<(String, Gradient)> {
        let (id, _) = self.grad_edit.clone()?;
        let only = self.selected.len() == 1 && self.selected.iter().any(|s| s.1 == id) && self.tool == Tool::Select;
        match self.gradient_of(&id).filter(|_| only) {
            Some(g) => Some((id, g)),
            None => {
                self.grad_edit = None;
                None
            }
        }
    }

    /// Dragging a gradient handle takes the pointer (true while it does)
    pub(crate) fn gradient_input(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect, space: bool) -> bool {
        let did = egui::Id::new("qd_grad_drag");
        let Some((id, g)) = self.grad_target() else {
            ctx.data_mut(|d| d.remove::<u8>(did));
            return false;
        };
        let v = self.view(rect);
        let Some(bx) = self.gradient_box(&id, v) else { return false };
        let (a, b) = super::gradient::handles(&g, bx);
        let (pressed, down, hover) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.hover_pos()));
        let mut drag: Option<u8> = ctx.data(|d| d.get_temp(did));
        if pressed && !space {
            if let Some(q) = hover.filter(|q| resp.contains_pointer() && rect.contains(*q) && !ui_blocks(ctx, *q)) {
                let near = [(0u8, a), (1, b)].into_iter().filter(|(_, h)| h.distance(q) < GRAB_PX).min_by(|x, y| x.1.distance(q).total_cmp(&y.1.distance(q)));
                if let Some((h, _)) = near {
                    self.snapshot();
                    drag = Some(h);
                }
            }
        }
        if let (Some(h), Some(q)) = (drag, hover.filter(|_| down)) {
            let f = [(q.x - bx.min.x) / bx.width().max(1e-3), (q.y - bx.min.y) / bx.height().max(1e-3)];
            self.edit_gradient(&id, |gr, _| {
                if let Some(gr) = gr {
                    if h == 0 {
                        gr.from = f;
                    } else {
                        gr.to = f;
                    }
                }
            });
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        if !down {
            drag = None;
        }
        match drag {
            Some(h) => ctx.data_mut(|d| d.insert_temp(did, h)),
            None => ctx.data_mut(|d| d.remove::<u8>(did)),
        }
        drag.is_some()
    }

    /// The handles: a line from start to end, each end a dot in its colour
    pub(crate) fn paint_gradient_handles(&self, p: &egui::Painter, v: View) {
        let shown = self.grad_edit.as_ref().filter(|(id, _)| self.selected.len() == 1 && self.selected.iter().any(|s| &s.1 == id));
        let k = ease(p.ctx(), egui::Id::new("qd_grad_h"), shown.is_some());
        let Some((id, stop)) = shown.filter(|_| k > 0.001) else { return };
        let (Some(g), Some(bx)) = (self.gradient_of(id), self.gradient_box(id, v)) else { return };
        let (a, b) = super::gradient::handles(&g, bx);
        let hov = p.ctx().pointer_hover_pos();
        let dark = Color32::from_black_alpha((110.0 * k) as u8);
        p.line_segment([a, b], EStroke::new(3.0_f32, dark));
        p.line_segment([a, b], EStroke::new(1.5_f32, Color32::WHITE.gamma_multiply(k)));
        // the radial / diamond reach shown as a faint ring through the end handle
        if g.kind == 1 || g.kind == 3 {
            p.circle_stroke(a, a.distance(b), EStroke::new(1.0_f32, Color32::WHITE.gamma_multiply(0.35 * k)));
        }
        let ends = [(0usize, a, 0.0_f32), (1, b, 1.0)];
        for (i, at, t) in ends {
            let hh = ease(p.ctx(), egui::Id::new(("qd_grad_hh", i)), hov.is_some_and(|q| q.distance(at) < GRAB_PX));
            let r = (6.0 + 1.5 * hh) * k;
            // the dot shows the colour at that end; the picked stop's end rings blue
            let col = super::gradient::color32_at(&g, t);
            let pick = g.stops.get(*stop).is_some_and(|s| (s.at - t).abs() < 0.02);
            p.circle_filled(at, r + 1.5, dark);
            p.circle_filled(at, r, Color32::WHITE.gamma_multiply(k));
            p.circle_filled(at, r - 2.0, col.to_opaque().gamma_multiply(k));
            if pick {
                p.circle_stroke(at, r + 3.0, EStroke::new(1.5_f32, blue().gamma_multiply(k)));
            }
        }
    }
}
