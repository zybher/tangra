//! Shortcut list overlay.

use crate::*;

impl App {
    /// Modal for the two global shortcuts: click a row, press the new keys.
    pub(crate) fn shortcuts_ui(&mut self, ctx: &egui::Context) {
        let t = ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_keys_modal"), self.shortcuts_open, 0.34 * anim_k(), egui::emath::easing::cubic_out);
        if self.recording.is_some() && !self.shortcuts_open {
            self.stop_recording();
        }
        // Listening for a new shortcut
        if let Some(which) = self.recording {
            let got = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                    _ => None,
                })
            });
            if let Some((key, m)) = got {
                if key == Key::Escape {
                    self.stop_recording();
                } else if m.ctrl || m.alt || m.shift || m.command || (key.name().starts_with('F') && key.name().len() > 1) {
                    self.recording = None;
                    self.set_hotkey(which, Self::combo_text(m, key));
                }
            }
            ctx.request_repaint();
        }
        if t <= 0.0 {
            return;
        }
        let ot = ctx.animate_bool_with_time(egui::Id::new("qd_keys_ot"), self.shortcuts_open, 0.6 * anim_k());
        let hint_k = ease(ctx, egui::Id::new("qd_keys_hint"), !self.hk_error.is_empty() || self.recording.is_some());
        let (w, h) = (340.0, 150.0 + 28.0 * hint_k);
        let screen = ctx.screen_rect();
        let r = Rect::from_center_size(screen.center() + vec2(0.0, (1.0 - t) * 10.0), vec2(w, h));
        let mut close = false;
        // Overshoot only on the way in; closing with back_out made the window bulge first
        let sc = 0.7 + 0.3 * if self.shortcuts_open { egui::emath::easing::back_out(t.clamp(0.0, 1.0)) } else { egui::emath::easing::cubic_out(t.clamp(0.0, 1.0)) };
        ctx.set_transform_layer(
            egui::LayerId::new(egui::Order::Foreground, egui::Id::new("qd_keys_modal")),
            egui::emath::TSTransform::new(r.center().to_vec2() * (1.0 - sc), sc),
        );
        egui::Area::new(egui::Id::new("qd_keys_modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(r.min)
            .constrain(false)
            .interactable(self.shortcuts_open)
            .show(ctx, |ui| {
                let p = ui.painter().clone();
                let a = t;
                p.rect_filled(r.translate(vec2(0.0, 4.0)), 12.0, Color32::from_black_alpha((90.0 * a) as u8));
                glass(&p, r, 12.0, a);
                p.rect_stroke(r, 12.0, EStroke::new(1.0_f32, border().gamma_multiply(a)));
                let a0 = stagger(ot, 0).min(t);
                p.text(pos2(r.min.x + 20.0 + (1.0 - a0) * -18.0, r.min.y + 26.0), Align2::LEFT_CENTER, "Keyboard shortcuts", FontId::proportional(14.5), TEXT.gamma_multiply(a0));
                // Close (x)
                let xr = Rect::from_center_size(pos2(r.max.x - 24.0, r.min.y + 26.0), vec2(24.0, 24.0));
                let xresp = ui.interact(xr, egui::Id::new("qd_keys_x"), Sense::click());
                let (xh, xp) = hover_fx(ctx, &xresp, true);
                p.rect_filled(xr, 6.0, hover_fill(xh, xp).gamma_multiply(a));
                let c = xr.center();
                let xs = EStroke::new(1.2_f32, mix(TEXT_DIM, Color32::WHITE, xh).gamma_multiply(a));
                p.line_segment([c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)], xs);
                p.line_segment([c + vec2(4.0, -4.0), c + vec2(-4.0, 4.0)], xs);
                if xresp.clicked() {
                    close = true;
                }

                let rows = [
                    (0u8, "Show / hide window", self.settings.summon_hotkey.clone()),
                    (1u8, "Quick-dump clipboard", self.settings.quick_dump_hotkey.clone()),
                ];
                for (n, (which, label, combo)) in rows.into_iter().enumerate() {
                    let a = stagger(ot, n + 1).min(t);
                    let row = Rect::from_min_size(pos2(r.min.x + 12.0, r.min.y + 54.0 + n as f32 * 40.0 + (1.0 - a) * 18.0), vec2(w - 24.0, 36.0));
                    let resp = ui.interact(row, egui::Id::new(("qd_keys_row", which)), Sense::click());
                    let (h, pr) = hover_fx(ctx, &resp, true);
                    let rec = self.recording == Some(which);
                    p.rect_filled(row, 8.0, hover_fill(h, pr).gamma_multiply(a));
                    p.text(pos2(row.min.x + 8.0, row.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), TEXT_DIM.gamma_multiply(a));
                    // The key badges and "Press keys…" crossfade (and the blue eases in) instead of swapping (#72)
                    let rk = ease(ctx, egui::Id::new(("qd_keys_rec", which)), rec);
                    let combo_caps: Vec<String> = combo
                        .split('+')
                        .map(|k| match k.trim() {
                            "CommandOrControl" | "CmdOrCtrl" | "Control" => "Ctrl".to_string(),
                            o => o.to_string(),
                        })
                        .collect();
                    let pulse = 0.5 + 0.5 * (self.now as f32 * 5.0).sin();
                    let badges = |caps: &[String], k: f32, recording: bool| {
                        if k <= 0.01 {
                            return;
                        }
                        let a = a * k;
                        let mut x = row.max.x - 8.0;
                        for cap in caps.iter().rev() {
                            let g = p.layout_no_wrap(cap.clone(), FontId::proportional(11.5), (if recording { blue() } else { TEXT }).gamma_multiply(a));
                            let cw = g.size().x + 14.0;
                            let kr = Rect::from_min_size(pos2(x - cw, row.center().y - 11.0), vec2(cw, 22.0));
                            p.rect_filled(kr, 5.0, frame_bg().gamma_multiply(a));
                            p.rect_stroke(kr, 5.0, EStroke::new(1.0_f32, (if recording { blue().gamma_multiply(0.4 + 0.6 * pulse) } else { border() }).gamma_multiply(a)));
                            p.galley(pos2(kr.min.x + 7.0, kr.center().y - g.size().y / 2.0), g, TEXT.gamma_multiply(a));
                            x = kr.min.x - 4.0;
                        }
                    };
                    badges(&combo_caps, 1.0 - rk, false);
                    badges(&["Press keys…".to_string()], rk, true);
                    if resp.clicked() {
                        if rec {
                            self.stop_recording();
                        } else {
                            // Free the old shortcuts so pressing them now does not fire them
                            if let Some(mgr) = &self.hk_mgr {
                                let _ = mgr.unregister_all(&self.hk_registered);
                            }
                            self.recording = Some(which);
                            self.hk_error.clear();
                        }
                    }
                }
                // The instruction and the red error crossfade instead of flipping (#73); the last
                // error is kept so it can fade out after it clears
                let err_id = egui::Id::new("qd_keys_err_last");
                if !self.hk_error.is_empty() {
                    ctx.data_mut(|d| d.insert_temp(err_id, self.hk_error.clone()));
                }
                let err: String = ctx.data(|d| d.get_temp(err_id)).unwrap_or_default();
                let ek = ease(ctx, egui::Id::new("qd_keys_err_k"), !self.hk_error.is_empty());
                let ik = ease(ctx, egui::Id::new("qd_keys_ins_k"), self.hk_error.is_empty() && self.recording.is_some());
                let hp = pos2(r.min.x + 20.0, r.max.y - 22.0);
                if ik > 0.01 {
                    p.text(hp, Align2::LEFT_CENTER, "Hold Ctrl, Alt or Shift and press a key · Esc cancels", FontId::proportional(11.0), TEXT_FAINT.gamma_multiply(a * hint_k * ik));
                }
                if ek > 0.01 {
                    p.text(hp, Align2::LEFT_CENTER, err, FontId::proportional(11.0), rgba(248, 113, 113, 255).gamma_multiply(a * hint_k * ek));
                }
            });
        if self.shortcuts_open && self.recording.is_none() {
            let (pressed, at, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos(), i.key_pressed(Key::Escape)));
            if esc || (pressed && at.map_or(false, |p| !r.contains(p))) {
                close = true;
            }
        }
        if close {
            self.shortcuts_open = false;
            self.stop_recording();
        }
    }
}
