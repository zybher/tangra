//! Editing a gradient fill: its two handles on the canvas (drag to move where it starts and
//! ends) and the helpers the properties panel uses to change its kind and colour stops.
//! The handles show while a stop is picked in the panel (`App::grad_edit`) and the object is
//! the only thing selected.

use crate::*;

/// How near a knob a press grabs it (a little past its drawn ring)
const GRAB_PX: f32 = 13.0;

fn panel_key() -> egui::Id {
    egui::Id::new("qd_grad_panel")
}

/// The properties card says each frame whether the fill's paint panel is open; the knobs show
/// only then (a frame it isn't drawn counts as closed)
pub(crate) fn note_grad_panel(ctx: &egui::Context, open: bool) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(panel_key(), (open, pass)));
}

fn knob_pick_key() -> egui::Id {
    egui::Id::new("qd_grad_knob_pick")
}

/// The stop whose knob was just pressed on the canvas (taken once, by the paint panel)
pub(crate) fn take_knob_pick(ctx: &egui::Context) -> Option<usize> {
    ctx.data_mut(|d| d.remove_temp::<usize>(knob_pick_key()))
}

fn knobs_key() -> egui::Id {
    egui::Id::new("qd_grad_knobs")
}

/// Is `q` on one of the gradient's knobs (while they show)
pub(crate) fn on_grad_knob(ctx: &egui::Context, q: Pos2) -> bool {
    grad_panel_open(ctx) && ctx.data(|d| d.get_temp::<Vec<Pos2>>(knobs_key())).is_some_and(|v| v.iter().any(|k| k.distance(q) < GRAB_PX))
}

fn grad_panel_open(ctx: &egui::Context) -> bool {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<(bool, u64)>(panel_key())).is_some_and(|(o, p)| o && pass.saturating_sub(p) <= 1)
}

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

    /// How shown the gradient knobs are (0..1) on object `id`; its selection outline fades out
    /// as they fade in
    pub(crate) fn grad_knobs_k(&self, ctx: &egui::Context, id: &str) -> f32 {
        // never alongside another edit on it (its points being edited)
        let on = self.grad_edit.as_ref().is_some_and(|(g, _)| g == id) && self.selected.len() == 1 && self.path_edit.is_none() && grad_panel_open(ctx);
        ease(ctx, egui::Id::new("qd_grad_h"), on)
    }

    /// Which gradient's handles show right now (dropped once the object isn't the only selection)
    fn grad_target(&mut self, ctx: &egui::Context) -> Option<(String, Gradient)> {
        if !grad_panel_open(ctx) || self.path_edit.is_some() {
            return None;
        }
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
        let Some((id, g)) = self.grad_target(ctx) else {
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
                // the ends (0, 1), then the stops in between (2 + its index)
                let mut knobs = vec![(0u8, a), (1, b)];
                for (j, st) in g.stops.iter().enumerate().filter(|(_, s)| s.at > 0.02 && s.at < 0.98) {
                    knobs.push((2 + j.min(250) as u8, a.lerp(b, st.at)));
                }
                let near = knobs.into_iter().filter(|(_, h)| h.distance(q) < GRAB_PX).min_by(|x, y| x.1.distance(q).total_cmp(&y.1.distance(q)));
                if let Some((h, _)) = near {
                    self.snapshot();
                    drag = Some(h);
                    // the knob's stop gets picked and its colour panel opens (paint panel), as a
                    // click on its row there; an end knob is the stop nearest that end
                    let j = if h >= 2 {
                        (h - 2) as usize
                    } else {
                        let end = if h == 0 { 0.0 } else { 1.0 };
                        (0..g.stops.len()).min_by(|x, y| (g.stops[*x].at - end).abs().total_cmp(&(g.stops[*y].at - end).abs())).unwrap_or(0)
                    };
                    self.grad_edit = Some((id.clone(), j));
                    ctx.data_mut(|d| d.insert_temp(knob_pick_key(), j));
                }
            }
        }
        if let (Some(h), Some(q)) = (drag.filter(|h| *h >= 2), hover.filter(|_| down)) {
            // a stop in between slides along the line only, never off it
            let d = b - a;
            let t = ((q - a).dot(d) / d.length_sq().max(1e-3)).clamp(0.0, 1.0);
            let j = (h - 2) as usize;
            self.edit_gradient(&id, |gr, _| {
                if let Some(st) = gr.as_mut().and_then(|g| g.stops.get_mut(j)) {
                    st.at = t;
                }
            });
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if let (Some(h), Some(q)) = (drag, hover.filter(|_| down)) {
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
        // the release that ends a knob press (a plain click too) is the knob's: it never falls
        // through to the canvas as a click that selects what's under it
        let released = drag.is_some() && !down;
        if !down {
            drag = None;
        }
        match drag {
            Some(h) => ctx.data_mut(|d| d.insert_temp(did, h)),
            None => ctx.data_mut(|d| d.remove::<u8>(did)),
        }
        drag.is_some() || released
    }

    /// The handles: a line from start to end, each end a dot in its colour, and a dot for every
/// stop in between where it sits along the line
    pub(crate) fn paint_gradient_handles(&self, p: &egui::Painter, v: View) {
        let shown = self.grad_edit.as_ref().filter(|(id, _)| self.selected.len() == 1 && self.selected.iter().any(|s| &s.1 == id));
        let k = shown.map_or(0.0, |(id, _)| self.grad_knobs_k(p.ctx(), id));
        p.ctx().data_mut(|d| d.remove::<Vec<Pos2>>(knobs_key()));
        let Some((id, stop)) = shown.filter(|_| k > 0.001) else { return };
        let (Some(g), Some(bx)) = (self.gradient_of(id), self.gradient_box(id, v)) else { return };
        let (a, b) = super::gradient::handles(&g, bx);
        // where the knobs are, so a press on one doesn't count as a click away from the paint panel
        let mut spots = vec![a, b];
        spots.extend(g.stops.iter().filter(|s| s.at > 0.02 && s.at < 0.98).map(|s| a.lerp(b, s.at)));
        p.ctx().data_mut(|d| d.insert_temp(knobs_key(), spots));
        let hov = p.ctx().pointer_hover_pos();
        let dark = Color32::from_black_alpha((110.0 * k) as u8);
        p.line_segment([a, b], EStroke::new(3.0_f32, dark));
        p.line_segment([a, b], EStroke::new(1.5_f32, Color32::WHITE.gamma_multiply(k)));
        // the radial / diamond reach shown as a faint ring through the end handle
        if g.kind == 1 || g.kind == 3 {
            p.circle_stroke(a, a.distance(b), EStroke::new(1.0_f32, Color32::WHITE.gamma_multiply(0.35 * k)));
        }
        // every stop between the ends sits on the line where it is along the gradient, a dot
        // in its colour like the ends (the picked one rings blue)
        for (j, st) in g.stops.iter().enumerate().filter(|(_, s)| s.at > 0.02 && s.at < 0.98) {
            let at = a.lerp(b, st.at);
            let hh = ease(p.ctx(), egui::Id::new(("qd_grad_sh", j)), hov.is_some_and(|q| q.distance(at) < GRAB_PX));
            let r = (6.0 + 1.5 * hh) * k;
            let col = super::gradient::color32_at(&g, st.at);
            p.circle_filled(at, r + 1.5, dark);
            p.circle_filled(at, r, Color32::WHITE.gamma_multiply(k));
            p.circle_filled(at, r - 2.0, col.to_opaque().gamma_multiply(k));
            let pk = ease(p.ctx(), egui::Id::new(("qd_grad_sp", j)), j == *stop);
            if pk > 0.01 {
                p.circle_stroke(at, r + 3.0, EStroke::new(1.5_f32, blue().gamma_multiply(k * pk)));
            }
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
