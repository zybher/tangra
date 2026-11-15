//! Bottom toolbar and the zoom/undo pill.

use crate::*;

impl App {
    /// Floating tool bar, bottom-centre.
    pub(crate) fn toolbar_ui(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>, rect: Rect) {
        use TbIcon as I;
        // None = divider. A tool of None means "add this item to the middle of the view".
        let items: [Option<(I, &str, Option<Tool>)>; 14] = [
            Some((I::Select, "Select  ·  V", Some(Tool::Select))),
            None,
            Some((I::Frame, "Frame  ·  F", Some(Tool::Frame))),
            None,
            Some((I::Rect, "Rectangle  ·  R", Some(Tool::Rect))),
            Some((I::Ellipse, "Ellipse  ·  O", Some(Tool::Ellipse))),
            Some((I::Arrow, "Arrow  ·  A", Some(Tool::Arrow))),
            None,
            Some((I::Pen, "Pen  ·  P", Some(Tool::Pen))),
            Some((I::Highlighter, "Highlighter  ·  H", Some(Tool::Highlighter))),
            Some((I::Pixelate, "Pixelate  ·  B", Some(Tool::Pixelate))),
            Some((I::Text, "Text  ·  T", Some(Tool::Text))),
            Some((I::Eraser, "Eraser  ·  E", Some(Tool::Eraser))),
            Some((I::Lasso, "Lasso  ·  K", Some(Tool::Lasso))),
        ];
        let (bw, gap, div, pad, h) = (34.0_f32, 2.0_f32, 11.0_f32, 5.0_f32, 44.0_f32);
        // Dock-style magnify: the hovered button swells and lifts, its neighbours a little;
        // the bar's width follows so it stays centred
        let ctx = ui.ctx().clone();
        let n_btn = items.iter().filter(|i| i.is_some()).count();
        // A smoothed "focus" follows the pointer along the bar; each button swells by its
        // distance from it, so a fast sweep glides instead of popping button to button
        let pill_prev = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("qd_tb_pill"))).unwrap_or(Rect::NOTHING);
        let ptr = ctx.pointer_hover_pos();
        let inside = ptr.map_or(false, |q| pill_prev.expand2(vec2(0.0, 12.0)).contains(q));
        let a = spring(&ctx, egui::Id::new("qd_tb_a"), inside, 18.0);
        let base_w = pad * 2.0 + n_btn as f32 * (bw + gap) - gap + items.iter().filter(|i| i.is_none()).count() as f32 * div;
        let mut offs = Vec::new();
        let mut bx = -base_w / 2.0 + pad;
        for it in items.iter() {
            if it.is_some() {
                offs.push(bx + bw / 2.0);
                bx += bw + gap;
            } else {
                bx += div;
            }
        }
        let fid = egui::Id::new("qd_tb_focus");
        let mut focus = ctx.data(|d| d.get_temp::<f32>(fid)).unwrap_or(0.0);
        if let (true, Some(q)) = (inside, ptr) {
            let target = q.x - rect.center().x;
            if a < 0.05 {
                focus = target;
            }
            let dt = ctx.input(|i| i.stable_dt).min(0.05);
            focus += (target - focus) * (1.0 - (-dt * 11.0).exp());
            if (target - focus).abs() > 0.3 {
                ctx.request_repaint();
            }
        }
        ctx.data_mut(|d| d.insert_temp(fid, focus));
        let tools: Vec<Option<Tool>> = items.iter().flatten().map(|it| it.2).collect();
        let mag: Vec<f32> = (0..n_btn)
            .map(|k| {
                let d = (offs[k] - focus) / (bw * 1.05);
                let hov = a * (-d * d).exp();
                let act = spring(&ctx, egui::Id::new(("qd_tb_act", k)), tools[k].is_some() && tools[k] == Some(self.tool), 18.0);
                hov.max(act)
            })
            .collect();
        let bwk = |k: usize| bw * (1.0 + 0.3 * mag[k]);
        let inner: f32 = (0..n_btn).map(|k| bwk(k) + gap).sum::<f32>() + items.iter().filter(|i| i.is_none()).count() as f32 * div;
        let width = pad * 2.0 + inner - gap;
        let pill = Rect::from_center_size(pos2(rect.center().x, rect.max.y - 10.0 - h / 2.0), vec2(width, h));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_tb_pill"), pill));
        let p = ui.painter().clone();
        p.add(
            egui::epaint::Shadow { offset: vec2(0.0, 6.0), blur: 20.0, spread: 0.0, color: Color32::from_black_alpha(100) }
                .as_shape(pill, Rounding::same(13.0)),
        );
        p.rect_filled(pill, 13.0, frame_bg());
        p.rect_stroke(pill, 13.0, EStroke::new(1.0_f32, border()));
        // Swallow clicks in the gaps so they never reach the canvas underneath
        ui.allocate_rect(pill, Sense::click());

        let cy = pill.center().y;
        let mut x = pill.min.x + pad;
        let mut k = 0usize;
        let divider = |x: f32| {
            p.line_segment([pos2(x + div / 2.0, cy - 9.0), pos2(x + div / 2.0, cy + 9.0)], EStroke::new(1.0_f32, border()));
        };
        for it in items {
            let Some((icon, tip, tool)) = it else {
                divider(x);
                x += div;
                continue;
            };
            let (m, w) = (mag[k], bwk(k));
            let r = Rect::from_center_size(pos2(x + w / 2.0, cy - 6.0 * m), vec2(w, w));
            let resp = ui.allocate_rect(r, Sense::click());
            let (hv, pr) = hover_fx(ui.ctx(), &resp, true);
            let on = ease(ui.ctx(), resp.id.with("on"), tool.is_some() && tool == Some(self.tool));
            if m > 0.01 {
                // a soft lifted tile behind the swelling button; stays as the base the
                // liquid fills when it's picked
                p.rect_filled(r.translate(vec2(0.0, 3.0 * m)), 10.0, Color32::from_black_alpha((60.0 * m) as u8));
                p.rect_filled(r, 10.0, mix(frame_bg(), Color32::from_gray(90), 0.22).gamma_multiply(m));
            }
            p.rect_filled(r, 8.0 + 2.0 * m, hover_fill(hv, pr));
            let lq = ui.ctx().animate_bool_with_time(resp.id.with("liquid"), tool.is_some() && tool == Some(self.tool), 0.5 * anim_k());
            let lq = lq * lq * (3.0 - 2.0 * lq);
            if lq >= 0.999 {
                p.rect_filled(r, 8.0 + 2.0 * m, blue());
            } else if lq > 0.001 {
                liquid_fill(&p, r, 8.0 + 2.0 * m, lq, ui.input(|i| i.time) as f32 + k as f32);
            }
            paint_icon(&p, icon, r.center(), (1.0 + 0.25 * m) * (1.0 - pr * 0.1), mix(TEXT_DIM, Color32::WHITE, hv.max(on)));
            let menu_kind = match (tool, icon) {
                (Some(Tool::Frame), _) => Some(1u8),
                (Some(Tool::Rect | Tool::Ellipse), _) => Some(2),
                (Some(Tool::Text), _) => Some(4),
                (Some(Tool::Pen | Tool::Highlighter | Tool::Arrow), _) => Some(3),
                (Some(Tool::Pixelate), _) => Some(5),
                _ => None,
            };
            if let Some(k) = menu_kind {
                // Tiny corner tick: this button has options on right-click
                let c = r.right_bottom() - vec2(6.0, 6.0);
                let mc = mix(TEXT_FAINT, Color32::WHITE, hv.max(on)).gamma_multiply(0.8);
                p.line_segment([c + vec2(-3.0, 0.0), c], EStroke::new(1.0_f32, mc));
                p.line_segment([c + vec2(0.0, -3.0), c], EStroke::new(1.0_f32, mc));
                if resp.secondary_clicked() {
                    if let Some(t) = tool {
                        acts.push(Act::Tool(t));
                    }
                    acts.push(Act::ToolMenu(k, r));
                }
            }
            let resp = resp.on_hover_text(tip.to_string());
            if resp.clicked() {
                match (tool, icon) {
                    (Some(t), _) => acts.push(Act::Tool(t)),
                    (None, I::Frame) => {
                        acts.push(Act::CentreMenu);
                        acts.push(Act::AddGroup);
                    }
                    _ => {
                        acts.push(Act::CentreMenu);
                        acts.push(Act::AddNote);
                    }
                }
            }
            x += w + gap;
            k += 1;
        }
    }

    pub(crate) fn bottom_bar_ui(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>, rect: Rect) {
        let bar_h = 28.0_f32;
        let bar_y = rect.max.y - bar_h - 10.0;
        let start_x = rect.min.x + 12.0 + super::outline::hud_shift(ui.ctx());

        let can_undo = !self.undo.is_empty();
        let can_redo = !self.redo.is_empty();
        let z = self.board.camera.zoom;
        let zoom_text = format!("{:.0}%", z * 100.0);

        // Keep showing the last count while the pill shrinks closed
        let ctx = ui.ctx().clone();
        let lid = egui::Id::new("qd_bb_last");
        let live = if self.selected.len() > 1 { format!("{} selected", self.selected.len()) } else { String::new() };
        if !live.is_empty() {
            ctx.data_mut(|d| d.insert_temp(lid, live.clone()));
        }
        let sa = spring(&ctx, egui::Id::new("qd_bb_open"), !live.is_empty(), 16.0);
        let status_text: String = if live.is_empty() { ctx.data(|d| d.get_temp::<String>(lid)).filter(|_| sa > 0.01).unwrap_or_default() } else { live };

        let font_id = FontId::proportional(11.5);
        let status_w = ui.fonts(|f| f.layout_no_wrap(status_text.clone(), font_id.clone(), Color32::WHITE).size().x);

        let zoom_w = 42.0_f32;
        let status_extra = if status_text.is_empty() { 0.0 } else { (6.0 + status_w + 16.0) * sa };
        let total_pill_w = 26.0 + 26.0 + 6.0 + zoom_w + 26.0 + status_extra;
        let pill_r = Rect::from_min_size(pos2(start_x, bar_y), vec2(total_pill_w, bar_h));

        let p = ui.painter().clone();

        // 1. Sleek floating pill background
        p.rect_filled(pill_r, Rounding::same(6.0), frame_bg());
        p.rect_stroke(pill_r, Rounding::same(6.0), EStroke::new(1.0_f32, border()));

        let cy = pill_r.center().y;
        let mut cur_x = pill_r.min.x;

        // --- Undo Button ---
        let undo_r = Rect::from_min_size(pos2(cur_x, bar_y), vec2(26.0, bar_h));
        let undo_resp = ui.allocate_rect(undo_r, Sense::click());
        let (uh, up) = bar_fx(&ctx, &undo_resp, can_undo);
        p.rect_filled(undo_r, Rounding { nw: 6.0, sw: 6.0, ne: 0.0, se: 0.0 }, hover_fill(uh, up));
        let uc = undo_r.center();
        let u_col = if can_undo { mix(TEXT_DIM, Color32::WHITE, uh) } else { TEXT_FAINT.gamma_multiply(0.6) };
        // Draw crisp curved undo arrow
        p.line_segment([pos2(uc.x - 4.5, uc.y), pos2(uc.x - 1.5, uc.y - 3.0)], EStroke::new(1.3_f32, u_col));
        p.line_segment([pos2(uc.x - 4.5, uc.y), pos2(uc.x - 1.5, uc.y + 3.0)], EStroke::new(1.3_f32, u_col));
        p.line_segment([pos2(uc.x - 4.0, uc.y), pos2(uc.x + 1.0, uc.y)], EStroke::new(1.3_f32, u_col));
        p.line_segment([pos2(uc.x + 1.0, uc.y), pos2(uc.x + 4.5, uc.y + 3.0)], EStroke::new(1.3_f32, u_col));
        p.line_segment([pos2(uc.x + 4.5, uc.y + 3.0), pos2(uc.x + 4.5, uc.y + 5.0)], EStroke::new(1.3_f32, u_col));

        if undo_resp.on_hover_text("Undo (Ctrl+Z)").clicked() && can_undo {
            acts.push(Act::Undo);
        }
        cur_x += 26.0;

        // --- Redo Button ---
        let redo_r = Rect::from_min_size(pos2(cur_x, bar_y), vec2(26.0, bar_h));
        let redo_resp = ui.allocate_rect(redo_r, Sense::click());
        let (rh, rp) = bar_fx(&ctx, &redo_resp, can_redo);
        p.rect_filled(redo_r, Rounding::ZERO, hover_fill(rh, rp));
        let rc = redo_r.center();
        let r_col = if can_redo { mix(TEXT_DIM, Color32::WHITE, rh) } else { TEXT_FAINT.gamma_multiply(0.6) };
        // Draw crisp curved redo arrow
        p.line_segment([pos2(rc.x + 4.5, rc.y), pos2(rc.x + 1.5, rc.y - 3.0)], EStroke::new(1.3_f32, r_col));
        p.line_segment([pos2(rc.x + 4.5, rc.y), pos2(rc.x + 1.5, rc.y + 3.0)], EStroke::new(1.3_f32, r_col));
        p.line_segment([pos2(rc.x + 4.0, rc.y), pos2(rc.x - 1.0, rc.y)], EStroke::new(1.3_f32, r_col));
        p.line_segment([pos2(rc.x - 1.0, rc.y), pos2(rc.x - 4.5, rc.y + 3.0)], EStroke::new(1.3_f32, r_col));
        p.line_segment([pos2(rc.x - 4.5, rc.y + 3.0), pos2(rc.x - 4.5, rc.y + 5.0)], EStroke::new(1.3_f32, r_col));

        if redo_resp.on_hover_text("Redo (Ctrl+Y)").clicked() && can_redo {
            acts.push(Act::Redo);
        }
        cur_x += 26.0;

        // --- Divider 1 ---
        p.line_segment([pos2(cur_x + 3.0, cy - 6.0), pos2(cur_x + 3.0, cy + 6.0)], EStroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 25)));
        cur_x += 6.0;

        // --- Zoom Percentage Button ---
        let zoom_r = Rect::from_min_size(pos2(cur_x, bar_y), vec2(zoom_w, bar_h));
        let zoom_resp = ui.allocate_rect(zoom_r, Sense::click());
        let (zh, zp) = bar_fx(&ctx, &zoom_resp, true);
        p.rect_filled(zoom_r, Rounding::same(4.0), hover_fill(zh, zp));
        let z_col = mix(TEXT_DIM, Color32::WHITE, zh);
        p.text(zoom_r.center(), Align2::CENTER_CENTER, zoom_text, FontId::proportional(11.5), z_col);
        if zoom_resp.on_hover_text("Reset zoom to 100%").clicked() {
            acts.push(Act::ZoomReset);
        }
        cur_x += zoom_w;

        // --- Zoom to Fit Button ---
        let fit_r = Rect::from_min_size(pos2(cur_x, bar_y), vec2(26.0, bar_h));
        let fit_resp = ui.allocate_rect(fit_r, Sense::click());
        let (fh, fp) = bar_fx(&ctx, &fit_resp, true);
        p.rect_filled(fit_r, Rounding::same(4.0), hover_fill(fh, fp));
        let fc = fit_r.center();
        let f_col = mix(TEXT_DIM, Color32::WHITE, fh);
        let s = 4.0_f32;
        let l = 2.0_f32;
        let st = EStroke::new(1.2_f32, f_col);
        p.line_segment([pos2(fc.x - s, fc.y - s + l), pos2(fc.x - s, fc.y - s)], st);
        p.line_segment([pos2(fc.x - s, fc.y - s), pos2(fc.x - s + l, fc.y - s)], st);
        p.line_segment([pos2(fc.x + s, fc.y - s + l), pos2(fc.x + s, fc.y - s)], st);
        p.line_segment([pos2(fc.x + s, fc.y - s), pos2(fc.x + s - l, fc.y - s)], st);
        p.line_segment([pos2(fc.x - s, fc.y + s - l), pos2(fc.x - s, fc.y + s)], st);
        p.line_segment([pos2(fc.x - s, fc.y + s), pos2(fc.x - s + l, fc.y + s)], st);
        p.line_segment([pos2(fc.x + s, fc.y + s - l), pos2(fc.x + s, fc.y + s)], st);
        p.line_segment([pos2(fc.x + s, fc.y + s), pos2(fc.x + s - l, fc.y + s)], st);

        if fit_resp.on_hover_text("Zoom to fit").clicked() {
            acts.push(Act::ZoomFit);
        }
        cur_x += 26.0;

        if status_text.is_empty() {
            return;
        }
        // --- Divider 2 + status, revealed as the pill grows ---
        let p = p.with_clip_rect(pill_r);
        p.line_segment([pos2(cur_x + 3.0, cy - 6.0 * sa), pos2(cur_x + 3.0, cy + 6.0 * sa)], EStroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, (25.0 * sa) as u8)));
        cur_x += 6.0;
        let status_pos = pos2(cur_x + 8.0 - (1.0 - sa) * 8.0, cy);
        p.text(status_pos, Align2::LEFT_CENTER, status_text, font_id, rgba(160, 160, 160, (220.0 * sa) as u8));

    }

    // ---------- overlays ----------
}
