//! Title bar: tabs, board switching, window buttons.

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
        painter.line_segment([pos2(mc.x - 5.0, cy), pos2(mc.x + 5.0, cy)], EStroke::new(1.2_f32, mix(TEXT_DIM, Color32::WHITE, h)));
        if min_resp.clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }

        // Maximize / Restore Button
        let max_r = Rect::from_min_size(pos2(min_x + btn_w, rect.min.y), vec2(btn_w, btn_h));
        let max_resp = ui.allocate_rect(max_r, Sense::click());
        let (h, pr) = hover_fx(ctx, &max_resp, true);
        painter.rect_filled(max_r, 0.0, hover_fill(h, pr));
        let xc = max_r.center();
        let ic = EStroke::new(1.1_f32, mix(TEXT_DIM, Color32::WHITE, h));
        if is_max {
            painter.rect_stroke(Rect::from_center_size(pos2(xc.x + 1.5, cy - 1.5), vec2(8.0, 8.0)), 0.0, ic);
            painter.rect_filled(Rect::from_center_size(pos2(xc.x - 1.5, cy + 1.5), vec2(8.0, 8.0)), 0.0, mix(frame_bg(), Color32::from_gray(66), h));
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
        let ic = EStroke::new(1.2_f32, mix(TEXT_DIM, Color32::WHITE, h));
        painter.line_segment([pos2(cc.x - 4.5, cy - 4.5), pos2(cc.x + 4.5, cy + 4.5)], ic);
        painter.line_segment([pos2(cc.x + 4.5, cy - 4.5), pos2(cc.x - 4.5, cy + 4.5)], ic);
        if cls_resp.clicked() {
            self.begin_hide(ctx);
        }

        // 3. Chrome Tabs
        let mut cur_x = rect.min.x + 16.0;
        let avail_for_tabs = (min_x - cur_x - 36.0).max(60.0);
        let num_boards = self.boards.len().max(1);
        let max_per_tab = (avail_for_tabs / num_boards as f32).max(50.0);
        let mut prev_hovered = false;
        let mut prev_is_active = false;

        for (i, (id, name)) in self.boards.iter().enumerate() {
            let is_active = *id == self.board.id;
            let text_w = name.chars().count() as f32 * 7.5;
            let tab_w = (text_w + 56.0).clamp(90.0, 180.0).min(max_per_tab);
            let tab_h = 37.0_f32;
            let tab_r = Rect::from_min_size(pos2(cur_x, rect.min.y + 7.0), vec2(tab_w, tab_h));

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

            // Chrome divider: only between adjacent inactive tabs, neither hovered!
            if i > 0 && !is_active && !prev_is_active && !is_hovered && !prev_hovered {
                let div_x = cur_x - 3.0;
                painter.line_segment(
                    [pos2(div_x, cy - 8.0), pos2(div_x, cy + 8.0)],
                    EStroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 45)),
                );
            }
            prev_hovered = is_hovered;
            prev_is_active = is_active;

            if is_active {
                paint_active_tab(&painter, tab_r.min.y, cur_x, tab_w, rect.max.y, self.last_canvas, self.now);
            }
            let (tab_h_t, tab_p_t) = hover_fx(ctx, &tab_resp, !is_active);
            if !is_active {
                // Same gap below the title as above it
                let hover_r = Rect::from_min_max(tab_r.min, pos2(tab_r.max.x, cy * 2.0 - tab_r.min.y));
                painter.rect_filled(hover_r, Rounding::same(7.0), Color32::from_white_alpha((tab_h_t * 14.0 + tab_p_t * 12.0) as u8));
            }

            // Dot indicator
            let dot_col = if is_active { Color32::WHITE } else { TEXT_FAINT };
            painter.circle_filled(pos2(tab_r.min.x + 13.0, cy), 3.5, dot_col);

            // Title text (optically centered on cy)
            let title_col = if is_active { Color32::WHITE } else { mix(TEXT_DIM, TEXT, tab_h_t) };
            let max_title_chars = ((tab_w - 44.0) / 7.2).max(1.0) as usize;
            let display_title = if name.chars().count() > max_title_chars {
                let mut s: String = name.chars().take(max_title_chars.saturating_sub(1)).collect();
                s.push('…');
                s
            } else {
                name.clone()
            };
            painter.text(
                pos2(tab_r.min.x + 23.0, cy - 1.0),
                Align2::LEFT_CENTER,
                display_title,
                FontId::proportional(12.5),
                title_col,
            );

            // Close Tab Button (if more than 1 board)
            if self.boards.len() > 1 {
                let x_center = pos2(tab_r.max.x - 14.0, cy);
                let x_r = Rect::from_center_size(x_center, vec2(18.0, 18.0));
                let x_resp = ui.allocate_rect(x_r, Sense::click());
                let (xh, xp) = hover_fx(ctx, &x_resp, true);
                painter.circle_filled(x_center, 8.0 - xp, Color32::from_white_alpha((xh * 30.0) as u8));
                let x_col = mix(if is_active { TEXT_DIM } else { mix(TEXT_FAINT, TEXT_DIM, tab_h_t) }, Color32::WHITE, xh);
                painter.line_segment([pos2(x_center.x - 3.5, x_center.y - 3.5), pos2(x_center.x + 3.5, x_center.y + 3.5)], EStroke::new(1.1_f32, x_col));
                painter.line_segment([pos2(x_center.x + 3.5, x_center.y - 3.5), pos2(x_center.x - 3.5, x_center.y + 3.5)], EStroke::new(1.1_f32, x_col));
                if x_resp.clicked() {
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

        // 4. New Tab (+) Button
        let plus_center = pos2(cur_x + 14.0, cy);
        let plus_r = Rect::from_center_size(plus_center, vec2(24.0, 24.0));
        let plus_resp = ui.allocate_rect(plus_r, Sense::click());
        let (ph, pp) = hover_fx(ctx, &plus_resp, true);
        painter.circle_filled(plus_center, 12.0 - pp * 1.5, hover_fill(ph, pp));
        let p_col = mix(TEXT_DIM, Color32::WHITE, ph);
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
