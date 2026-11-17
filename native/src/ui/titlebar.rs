//! Title bar: the Tangra mark, tabs, board switching, window buttons.

use crate::*;

impl App {
    pub(crate) fn titlebar_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, rect: Rect) {
        enum TabAct {
            Switch(String),
            Rename(String, String),
            Close(String),
            New,
        }
        let mut tab_act: Option<TabAct> = None;
        let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let painter = ui.painter().clone();
        let cy = rect.min.y + 22.0_f32;

        // 1. Far Right: Windows Control Buttons (Minimize, Maximize/Restore, Close)
        let btn_w = 46.0_f32;
        let btn_h = 44.0_f32;
        let min_x = rect.max.x - btn_w * 3.0;

        // Minimize Button
        let min_r = Rect::from_min_size(pos2(min_x, rect.min.y), vec2(btn_w, btn_h));
        let min_resp = ui.allocate_rect(min_r, Sense::click());
        let (h, pr) = hover_fx(ctx, &min_resp, true);
        painter.rect_filled(min_r, 0.0, hover_fill(h, pr));
        let mc = min_r.center();
        painter.line_segment([pos2(mc.x - 5.0, cy), pos2(mc.x + 5.0, cy)], EStroke::new(1.2_f32, mix(txt_dim(), hi(), h)));
        if min_resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }

        // Maximize / Restore Button
        let max_r = Rect::from_min_size(pos2(min_x + btn_w, rect.min.y), vec2(btn_w, btn_h));
        let max_resp = ui.allocate_rect(max_r, Sense::click());
        let (h, pr) = hover_fx(ctx, &max_resp, true);
        painter.rect_filled(max_r, 0.0, hover_fill(h, pr));
        let xc = max_r.center();
        let ic = EStroke::new(1.1_f32, mix(txt_dim(), hi(), h));
        if is_max {
            painter.rect_stroke(Rect::from_center_size(pos2(xc.x + 1.5, cy - 1.5), vec2(8.0, 8.0)), 0.0, ic);
            painter.rect_filled(Rect::from_center_size(pos2(xc.x - 1.5, cy + 1.5), vec2(8.0, 8.0)), 0.0, mix(chrome(), gray_ui(66), h));
            painter.rect_stroke(Rect::from_center_size(pos2(xc.x - 1.5, cy + 1.5), vec2(8.0, 8.0)), 0.0, ic);
        } else {
            painter.rect_stroke(Rect::from_center_size(pos2(xc.x, cy), vec2(10.0, 10.0)), 0.0, ic);
        }
        if max_resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_max));
        }

        // Close Button
        let cls_r = Rect::from_min_size(pos2(min_x + btn_w * 2.0, rect.min.y), vec2(btn_w, btn_h));
        let cls_resp = ui.allocate_rect(cls_r, Sense::click());
        let (h, pr) = hover_fx(ctx, &cls_resp, true);
        painter.rect_filled(cls_r, 0.0, DANGER.gamma_multiply(h * (1.0 - pr * 0.25)));
        let cc = cls_r.center();
        let ic = EStroke::new(1.2_f32, mix(txt_dim(), hi(), h));
        painter.line_segment([pos2(cc.x - 4.5, cy - 4.5), pos2(cc.x + 4.5, cy + 4.5)], ic);
        painter.line_segment([pos2(cc.x + 4.5, cy - 4.5), pos2(cc.x - 4.5, cy + 4.5)], ic);
        if cls_resp.clicked() {
            self.begin_hide(ctx);
        }

        // Light / dark toggle, left of the window buttons (ui/theme_toggle.rs)
        let tog_r = Rect::from_center_size(pos2(min_x - 22.0, cy), vec2(28.0, 28.0));
        if let Some(light) = super::theme_toggle::button(ui, ctx, tog_r) {
            self.settings.dark_theme = !light;
            save_settings(&self.settings);
        }
        let min_x = min_x - 44.0;

        // 2. Far left: the Tangra mark, ahead of the tabs (ui/title_logo.rs)
        super::title_logo::title_logo(ui, pos2(rect.min.x + 26.0, cy), 20.0);

        // 3. Chrome Tabs
        let mut cur_x = rect.min.x + 8.0 + super::title_logo::LOGO_W;
        let avail_for_tabs = (min_x - cur_x - 36.0).max(60.0);
        let num_boards = self.boards.len().max(1);
        let max_per_tab = (avail_for_tabs / num_boards as f32).max(50.0);
        let mut prev_here = 1.0_f32;
        // The active tab's shape goes here, under every title, so it can slide across them
        let tab_slot = painter.add(egui::Shape::Noop);
        let mut active_at: Option<(f32, f32, f32)> = None;

        for (i, (id, name)) in self.boards.iter().enumerate() {
            let is_active = *id == self.board.id;
            let text_w = name.chars().count() as f32 * 7.5;
            let tab_w = (text_w + 56.0).clamp(90.0, 180.0).min(max_per_tab);
            let tab_h = 37.0_f32;
            // Tabs slide and resize to their place when one opens, closes or is renamed (audit #86)
            let tg = egui::Id::new(("qd_tab_g", id.as_str()));
            let gx = super::micro::glide(ctx, tg.with("x"), cur_x, false);
            let gw = super::micro::glide(ctx, tg.with("w"), tab_w, false);
            let tab_r = Rect::from_min_size(pos2(gx, rect.min.y + 7.0), vec2(gw, tab_h));

            let tab_resp = ui.allocate_rect(tab_r, Sense::click());
            let is_hovered = tab_resp.hovered();

            // Right-click on tab to rename
            tab_resp.context_menu(|ui| {
                cm_begin(ui);
                ui.set_min_width(150.0);
                ui.set_max_width(150.0);
                if item(ui, "Rename", "Double-click", true) {
                    tab_act = Some(TabAct::Rename(id.clone(), name.clone()));
                }
                cm_end(ui);
            });

            // Chrome divider: only between adjacent inactive tabs, neither hovered; fades out
            // and back as a neighbour is hovered or becomes active
            let act_k = ease(ctx, tab_resp.id.with("act"), is_active);
            let hov_k = ease(ctx, tab_resp.id.with("hov"), is_hovered);
            let here = (1.0 - act_k) * (1.0 - hov_k);
            let div_a = if i > 0 { here * prev_here } else { 0.0 };
            if div_a > 0.01 {
                let div_x = cur_x - 3.0;
                painter.line_segment(
                    [pos2(div_x, cy - 8.0), pos2(div_x, cy + 8.0)],
                    EStroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 45).gamma_multiply(div_a)),
                );
            }
            prev_here = here;

            if is_active {
                active_at = Some((tab_r.min.y, cur_x, tab_w));
            }
            let (tab_h_t, tab_p_t) = hover_fx(ctx, &tab_resp, !is_active);
            if !is_active {
                // Same gap below the title as above it
                let hover_r = Rect::from_min_max(tab_r.min, pos2(tab_r.max.x, cy * 2.0 - tab_r.min.y));
                painter.rect_filled(hover_r, Rounding::same(7.0), wa((tab_h_t * 14.0 + tab_p_t * 12.0) as u8));
            }


            // Title text (optically centered on cy)
            let title_col = mix(mix(txt_dim(), txt(), tab_h_t), hi(), act_k);
            let max_title_chars = ((tab_w - 44.0) / 7.2).max(1.0) as usize;
            let display_title = if name.chars().count() > max_title_chars {
                let mut s: String = name.chars().take(max_title_chars.saturating_sub(1)).collect();
                s.push('…');
                s
            } else {
                name.clone()
            };
            painter.text(
                pos2(tab_r.min.x + 13.0, cy - 1.0),
                Align2::LEFT_CENTER,
                display_title,
                FontId::proportional(12.5),
                title_col,
            );

            // Close Tab Button (if more than 1 board): fades and grows in when a second board appears
            let x_k = ease(ctx, egui::Id::new("qd_tab_x_show"), self.boards.len() > 1);
            if x_k > 0.01 {
                let x_center = pos2(tab_r.max.x - 14.0, cy);
                let x_r = Rect::from_center_size(x_center, vec2(18.0, 18.0));
                let x_resp = ui.allocate_rect(x_r, if self.boards.len() > 1 { Sense::click() } else { Sense::hover() });
                let (xh, xp) = hover_fx(ctx, &x_resp, true);
                painter.circle_filled(x_center, (8.0 - xp) * x_k, wa((xh * 30.0 * x_k) as u8));
                let x_col = mix(mix(txt_faint(), txt_dim(), tab_h_t), txt_dim(), act_k);
                let x_col = mix(x_col, hi(), xh).gamma_multiply(x_k);
                let a = 3.5 * (0.6 + 0.4 * x_k);
                painter.line_segment([pos2(x_center.x - a, x_center.y - a), pos2(x_center.x + a, x_center.y + a)], EStroke::new(1.1_f32, x_col));
                painter.line_segment([pos2(x_center.x + a, x_center.y - a), pos2(x_center.x - a, x_center.y + a)], EStroke::new(1.1_f32, x_col));
                if self.boards.len() > 1 && x_resp.clicked() {
                    tab_act = Some(TabAct::Close(id.clone()));
                }
            }

            // Click & double-click tab
            if tab_act.is_none() {
                if tab_resp.clicked() && !is_active {
                    tab_act = Some(TabAct::Switch(id.clone()));
                } else if tab_resp.double_clicked() {
                    tab_act = Some(TabAct::Rename(id.clone(), name.clone()));
                } else if tab_resp.middle_clicked() && self.boards.len() > 1 {
                    tab_act = Some(TabAct::Close(id.clone()));
                }
            }

            cur_x += tab_w + 6.0;
        }
        // Slide the active tab to its board instead of jumping (ui/micro.rs)
        if let Some((top, x, w)) = active_at {
            let gid = egui::Id::new("qd_active_tab");
            let gx = super::micro::glide(ctx, gid.with("x"), x, false);
            let gw = super::micro::glide(ctx, gid.with("w"), w, false);
            painter.set(tab_slot, egui::Shape::Vec(active_tab_shapes(top, gx, gw, rect.max.y, self.last_canvas, self.now)));
        }

        // 4. New Tab (+) Button
        let plus_center = pos2(cur_x + 14.0, cy);
        let plus_r = Rect::from_center_size(plus_center, vec2(24.0, 24.0));
        let plus_resp = ui.allocate_rect(plus_r, Sense::click());
        let (ph, pp) = hover_fx(ctx, &plus_resp, true);
        painter.circle_filled(plus_center, 12.0 - pp * 1.5, hover_fill(ph, pp));
        let p_col = mix(txt_dim(), hi(), ph);
        painter.line_segment([pos2(plus_center.x - 4.5, plus_center.y), pos2(plus_center.x + 4.5, plus_center.y)], EStroke::new(1.3_f32, p_col));
        painter.line_segment([pos2(plus_center.x, plus_center.y - 4.5), pos2(plus_center.x, plus_center.y + 4.5)], EStroke::new(1.3_f32, p_col));
        if plus_resp.clicked() {
            tab_act = Some(TabAct::New);
        }

        // 5. Blank Titlebar Space: Draggable window region
        let drag_start_x = cur_x + 34.0;
        let drag_limit_x = min_x;
        if drag_limit_x > drag_start_x {
            let drag_r = Rect::from_min_max(pos2(drag_start_x, rect.min.y), pos2(drag_limit_x, rect.max.y));
            let drag_resp = ui.allocate_rect(drag_r, Sense::click_and_drag());
            if drag_resp.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if drag_resp.double_clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_max));
            }
        }

        match tab_act {
            Some(TabAct::Switch(id)) => self.switch_board(&id),
            Some(TabAct::Rename(id, name)) => {
                if id != self.board.id {
                    self.switch_board(&id);
                }
                self.prompt = Some(Prompt::Rename(name));
            }
            Some(TabAct::Close(id)) => {
                if id == self.board.id {
                    self.prompt = Some(Prompt::ConfirmDelete);
                } else {
                    self.switch_board(&id);
                    self.prompt = Some(Prompt::ConfirmDelete);
                }
            }
            Some(TabAct::New) if !PROMPT_BUSY.load(std::sync::atomic::Ordering::Relaxed) => self.prompt = Some(Prompt::NewBoard(String::new())),
            Some(TabAct::New) => {}
            None => {}
        }
    }
}
