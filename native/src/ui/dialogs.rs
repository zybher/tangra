//! Label editing, board prompts and hotkey recording.

use crate::*;

impl App {
    pub(crate) fn edit_ui(&mut self, ctx: &egui::Context, v: View) {
        let Some(mut e) = self.edit.take() else { return };
        let multiline = matches!(e.target, EditTarget::Note(_));
        let width = match &e.target {
            EditTarget::Note(id) => self
                .board
                .sticky_notes
                .iter()
                .find(|n| &n.id == id)
                .map(|n| (n.width * v.cam.zoom).clamp(200.0, 520.0))
                .unwrap_or(240.0),
            _ => 240.0,
        };
        let mut done: Option<bool> = None;
        // Titles and canvas text are edited right where they sit, in their own
        // font, size and colour. Only notes get a popup editor.
        let z = v.cam.zoom;
        let inline: Option<(Pos2, f32, Color32, u8)> = match &e.target {
            EditTarget::Note(_) => None,
            EditTarget::Group(id) => self.board.group_cards.iter().find(|g| &g.id == id).map(|g| {
                // Same size, weight and spot as the drawn title (which is selected, so accent)
                let size = FRAME_TITLE_PX;
                let col = TEXT;
                let row = ctx.fonts(|f| f.row_height(&semibold(size)));
                let bw = g.border.unwrap_or(1.0) * z;
                (v.s(g.x, g.y) + vec2(5.0, -5.0 - row - bw / 2.0), size, col, 0)
            }),
            EditTarget::WorldText(id) => self.board.world_texts.iter().find(|t| &t.id == id).map(|t| {
                let fs = if t.font_size > 0.0 { t.font_size } else { 16.0 };
                (v.s(t.x, t.y), (fs * z).max(6.0), hex(&t.color), t.style)
            }),
            EditTarget::ImageLabel { image, label } => self.board.images.iter().find(|i| &i.id == image).and_then(|img| {
                let (sx, sy) = img.scale();
                img.annotations.texts.iter().find(|l| &l.id == label).map(|t| {
                    let fs = if t.font_size > 0.0 { t.font_size } else { 16.0 };
                    (v.s(img.x + t.x * sx, img.y + t.y * sy), (fs * sx * z).max(6.0), hex(&t.color), t.style)
                })
            }),
            _ => Some((v.s(e.world.x, e.world.y), (self.text_size * z).max(6.0), hex(&self.pen_color), self.text_style)),
        };
        if let Some((pos, size, col, style)) = inline {
            let fid = if matches!(e.target, EditTarget::Group(_)) { semibold(size) } else { styled_font(FontId::proportional(size), style) };
            // Room for the next keystroke too: the width is measured before this frame's typing,
            // so without it the field scrolls and clips the first letters instead of growing
            let mut text_w = ctx.fonts(|f| f.layout_no_wrap(e.text.clone(), fid.clone(), col).size().x) + size;
            // A frame title's editor never runs past the frame
            let title = match &e.target {
                EditTarget::Group(id) => self.board.group_cards.iter().find(|g| &g.id == id).map(|g| (v.s(g.x, g.y).x, g.width * z)),
                _ => None,
            };
            if let Some((_, fw)) = title {
                text_w = text_w.min((fw - 12.0).max(16.0));
            }
            egui::Area::new(egui::Id::new("qd_edit_inline"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .constrain(false)
                .show(ctx, |ui| {
                    // The area remembers last frame's size and caps the field to it, which made the
                    // field lag a keystroke behind and scroll the text left: lift the cap first
                    ui.set_max_width(text_w.max(24.0) + 12.0);
                    let out = egui::TextEdit::singleline(&mut e.text)
                        .font(fid)
                        .text_color(col)
                        .frame(false)
                        .margin(egui::Margin::ZERO)
                        .clip_text(title.is_some())
                        .desired_width(text_w.max(24.0) + 8.0)
                        .show(ui);
                    let r = out.response;
                    if let Some((fx, fw)) = title {
                        // Frame titles: an accent outline that starts on the frame's left edge and
                        // never passes its right edge
                        let right = (r.rect.min.x + text_w.max(24.0) + 6.0).min(fx + fw);
                        let b = Rect::from_min_max(pos2(fx, r.rect.min.y - 2.0), pos2(right, r.rect.max.y + 2.0));
                        ui.painter().rect_filled(b, 3.0, blue().gamma_multiply(0.12));
                        ui.painter().rect_stroke(b, 3.0, EStroke::new(1.0_f32, blue()));
                    } else {
                        // A thin underline is the only sign you are editing
                        let y = r.rect.max.y + 1.0;
                        ui.painter().line_segment(
                            [pos2(r.rect.min.x, y), pos2(r.rect.min.x + text_w.max(24.0), y)],
                            EStroke::new(1.0_f32, blue()),
                        );
                    }
                    if e.fresh > 0 {
                        e.fresh -= 1;
                        r.request_focus();
                    } else if r.lost_focus() {
                        let esc = ui.input(|i| i.key_pressed(Key::Escape));
                        done = Some(!esc);
                    }
                });
        } else {
        egui::Area::new(egui::Id::new("qd_edit"))
            .order(egui::Order::Foreground)
            .fixed_pos(v.s(e.world.x, e.world.y))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let r = if multiline {
                        ui.add(egui::TextEdit::multiline(&mut e.text).desired_width(width).desired_rows(5).hint_text("Write a note…"))
                    } else {
                        ui.add(egui::TextEdit::singleline(&mut e.text).desired_width(width).hint_text("Type, then press Enter"))
                    };
                    if e.fresh > 0 {
                        e.fresh -= 1;
                        r.request_focus();
                    } else if r.lost_focus() {
                        // Esc cancels a new label; notes always keep what you typed
                        let esc = ui.input(|i| i.key_pressed(Key::Escape));
                        done = Some(multiline || !esc);
                    }
                    if multiline {
                        ui.label(egui::RichText::new("Click outside or Ctrl+Enter to save").size(10.5).color(TEXT_FAINT));
                        if ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter)) {
                            done = Some(true);
                        }
                    }
                });
            });
        }
        if done.is_some() {
            // Lets the canvas tell that this press only ended the edit (see canvas input)
            let n = ctx.cumulative_pass_nr();
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_edit_ended"), n));
        }
        match done {
            Some(true) => {
                self.edit = Some(e);
                self.commit_edit();
            }
            Some(false) => {}
            None => {
                if e.fresh > 0 {
                    ctx.request_repaint();
                }
                self.edit = Some(e);
            }
        }
    }

    pub(crate) fn prompt_ui(&mut self, ctx: &egui::Context) {
        // Pops in from slightly smaller and fades; on close a non-interactive copy shrinks away
        let tid = egui::Id::new("qd_prompt_t");
        let gid = egui::Id::new("qd_prompt_ghost");
        let t = ctx.animate_bool_with_time_and_easing(tid, self.prompt.is_some(), 0.34 * anim_k(), egui::emath::easing::cubic_out);
        let live = self.prompt.is_some();
        PROMPT_BUSY.store(live || t > 0.0, std::sync::atomic::Ordering::Relaxed);
        let ot = ctx.animate_bool_with_time(egui::Id::new("qd_prompt_ot"), live, 0.6 * anim_k());
        let Some(mut pr) = self.prompt.take().or_else(|| ctx.data(|d| d.get_temp::<Prompt>(gid))) else { return };
        if !live && t <= 0.0 {
            ctx.data_mut(|d| d.remove::<Prompt>(gid));
            return;
        }
        let (title, ok_label) = match pr {
            Prompt::NewBoard(_) => ("New board", "Create"),
            Prompt::Rename(_) => ("Rename board", "Rename"),
            Prompt::ConfirmDelete => ("Delete board?", "Delete"),
        };
        let (w, h) = (340.0, if matches!(pr, Prompt::ConfirmDelete) { 158.0 } else { 150.0 });
        let r = Rect::from_center_size(ctx.screen_rect().center() + vec2(0.0, (1.0 - t) * 10.0), vec2(w, h));
        // Overshoot only on the way in; closing with back_out made the window bulge first
        let sc = 0.7 + 0.3 * if self.prompt.is_some() { egui::emath::easing::back_out(t.clamp(0.0, 1.0)) } else { egui::emath::easing::cubic_out(t.clamp(0.0, 1.0)) };
        let mid = egui::Id::new("qd_prompt_modal");
        ctx.set_transform_layer(egui::LayerId::new(egui::Order::Foreground, mid), egui::emath::TSTransform::new(r.center().to_vec2() * (1.0 - sc), sc));
        let (mut ok, mut cancel) = (false, false);
        let danger = matches!(pr, Prompt::ConfirmDelete);
        let board_name = clip_name(&self.board.name, 24);
        egui::Area::new(mid)
            .order(egui::Order::Foreground)
            .fixed_pos(r.min)
            .constrain(false)
            .interactable(live)
            .show(ctx, |ui| {
                let p = ui.painter().clone();
                p.rect_filled(r.translate(vec2(0.0, 4.0)), 12.0, Color32::from_black_alpha((90.0 * t) as u8));
                glass(&p, r, 12.0, t);
                p.rect_stroke(r, 12.0, EStroke::new(1.0_f32, border().gamma_multiply(t)));
                // Title + close
                let a0 = stagger(ot, 0).min(t);
                p.text(pos2(r.min.x + 20.0 - (1.0 - a0) * 18.0, r.min.y + 26.0), Align2::LEFT_CENTER, title, FontId::proportional(14.5), TEXT.gamma_multiply(a0));
                let xr = Rect::from_center_size(pos2(r.max.x - 24.0, r.min.y + 26.0), vec2(24.0, 24.0));
                let xresp = ui.interact(xr, egui::Id::new("qd_prompt_x"), Sense::click());
                let (xh, xp) = hover_fx(ctx, &xresp, true);
                p.rect_filled(xr, 6.0, hover_fill(xh, xp).gamma_multiply(a0));
                let c = xr.center();
                let xs = EStroke::new(1.2_f32, mix(TEXT_DIM, Color32::WHITE, xh).gamma_multiply(a0));
                p.line_segment([c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)], xs);
                p.line_segment([c + vec2(4.0, -4.0), c + vec2(-4.0, 4.0)], xs);
                cancel |= xresp.clicked();

                // Field / message
                let a1 = stagger(ot, 1).min(t);
                let dy1 = (1.0 - a1) * 18.0;
                match &mut pr {
                    Prompt::NewBoard(text) | Prompt::Rename(text) => {
                        let fr = Rect::from_min_size(pos2(r.min.x + 20.0, r.min.y + 52.0 + dy1), vec2(w - 40.0, 34.0));
                        let id = egui::Id::new("qd_prompt_field");
                        let focused = ctx.memory(|m| m.has_focus(id));
                        let fk = ease(ctx, id.with("f"), focused);
                        p.rect_filled(fr, 8.0, frame_bg().gamma_multiply(a1));
                        p.rect_stroke(fr, 8.0, EStroke::new(1.0_f32, mix(border(), blue(), fk).gamma_multiply(a1)));
                        ui.multiply_opacity(a1);
                        let resp = ui.put(
                            fr.shrink2(vec2(10.0, 7.0)),
                            egui::TextEdit::singleline(text).id(id).frame(false).font(FontId::proportional(13.0)).hint_text("Board name"),
                        );
                        ui.multiply_opacity(1.0 / a1.max(0.01));
                        if live && !resp.has_focus() {
                            resp.request_focus();
                        }
                    }
                    Prompt::ConfirmDelete => {
                        let g = p.layout(
                            format!("Delete \"{}\"?\nAll its images go too. This cannot be undone.", board_name),
                            FontId::proportional(12.5),
                            TEXT_DIM.gamma_multiply(a1),
                            w - 40.0,
                        );
                        p.galley(pos2(r.min.x + 20.0, r.min.y + 58.0 + dy1), g, TEXT_DIM.gamma_multiply(a1));
                    }
                }

                // Buttons, bottom-right: Cancel (quiet) and the action (blue, red for delete)
                let a2 = stagger(ot, 2).min(t);
                let by = r.max.y - 34.0 + (1.0 - a2) * 18.0;
                let mut button = |label: &str, right: f32, primary: bool, key: &str| -> (bool, f32) {
                    let g = p.layout_no_wrap(label.to_string(), FontId::proportional(12.5), Color32::WHITE);
                    let bw = g.size().x + 28.0;
                    let br = Rect::from_min_max(pos2(right - bw, by - 15.0), pos2(right, by + 15.0));
                    let resp = ui.interact(br, egui::Id::new(("qd_prompt_btn", key)), Sense::click());
                    let (hh, pp) = hover_fx(ctx, &resp, true);
                    if primary {
                        let base = if danger { rgba(220, 70, 70, 255) } else { blue() };
                        p.rect_filled(br, 8.0, mix(base, Color32::WHITE, hh * 0.12 - pp * 0.0).gamma_multiply(a2));
                        p.text(br.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.5), Color32::WHITE.gamma_multiply(a2));
                    } else {
                        p.rect_filled(br, 8.0, hover_fill(hh, pp).gamma_multiply(a2));
                        p.rect_stroke(br, 8.0, EStroke::new(1.0_f32, border().gamma_multiply(a2)));
                        p.text(br.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.5), mix(TEXT_DIM, TEXT, hh).gamma_multiply(a2));
                    }
                    (resp.clicked(), br.min.x)
                };
                let (go, left) = button(ok_label, r.max.x - 20.0, true, "ok");
                let (no, _) = button("Cancel", left - 8.0, false, "cancel");
                ok |= go;
                cancel |= no;
                let (enter, esc) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
                ok |= enter && !danger && live;
                cancel |= esc && live;
            });
        if !live {
            return;
        }
        // Clicking anywhere outside the dialog cancels it
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|q| !r.expand(4.0).contains(q)));
        cancel |= outside && !ok;
        if cancel || ok {
            ctx.data_mut(|d| d.insert_temp(gid, pr.clone()));
        }
        if cancel {
            return;
        }
        if !ok {
            self.prompt = Some(pr);
            return;
        }
        match pr {
            Prompt::NewBoard(s) if !s.trim().is_empty() => {
                let id = create_board(s.trim());
                self.boards = list_boards();
                self.switch_board(&id);
            }
            Prompt::Rename(s) if !s.trim().is_empty() => {
                self.board.name = s.trim().to_string();
                self.mark_dirty();
                self.save_now();
                self.boards = list_boards();
            }
            Prompt::ConfirmDelete => {
                let id = self.board.id.clone();
                self.dirty_since = None;
                delete_board(&id);
                self.boards = list_boards();
                let next = self.boards[0].0.clone();
                self.switch_board(&next);
            }
            _ => {}
        }
    }

    /// Turns a key press into the hotkey text format ("Ctrl+Shift+Space")
    pub(crate) fn combo_text(m: egui::Modifiers, key: Key) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if m.ctrl || m.command {
            parts.push("Ctrl");
        }
        if m.alt {
            parts.push("Alt");
        }
        if m.shift {
            parts.push("Shift");
        }
        let mut s = parts.join("+");
        if !s.is_empty() {
            s.push('+');
        }
        s.push_str(key.name());
        s
    }

    pub(crate) fn set_hotkey(&mut self, which: u8, combo: String) {
        let old = (self.settings.summon_hotkey.clone(), self.settings.quick_dump_hotkey.clone());
        if which == 0 {
            self.settings.summon_hotkey = combo;
        } else {
            self.settings.quick_dump_hotkey = combo;
        }
        match self.register_hotkeys() {
            Ok(()) => {
                self.hk_error.clear();
                save_settings(&self.settings);
            }
            Err(e) => {
                // Put the working hotkeys back so the app is never left without them
                (self.settings.summon_hotkey, self.settings.quick_dump_hotkey) = old;
                let _ = self.register_hotkeys();
                self.hk_error = e;
            }
        }
    }

    pub(crate) fn stop_recording(&mut self) {
        if self.recording.take().is_some() {
            let _ = self.register_hotkeys();
        }
    }
}
