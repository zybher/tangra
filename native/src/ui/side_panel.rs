//! Canvas panel: theme, accent, gradient, Aurora, patterns, settings.

use crate::*;

impl App {
    /// Bottom-right arrow button: slides up a small menu of canvas colours,
    /// plus a way into the shortcut settings.
    pub(crate) fn side_panel_ui(&mut self, ctx: &egui::Context, canvas: Rect) {
        let t = ease(ctx, egui::Id::new("qd_theme_pop"), self.panel_open);
        let btn = Rect::from_center_size(pos2(canvas.max.x - 24.0, canvas.max.y - 24.0), vec2(28.0, 28.0));
        let mut toggle = false;
        egui::Area::new(egui::Id::new("qd_theme_btn"))
            .order(egui::Order::Foreground)
            .fixed_pos(btn.min)
            .constrain(false)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(btn.size(), Sense::click());
                let (h, pr) = hover_fx(ctx, &resp, true);
                let p = ui.painter();
                p.rect_filled(r, 6.0, frame_bg());
                p.rect_stroke(r, 6.0, EStroke::new(1.0_f32, border()));
                p.rect_filled(r, 6.0, hover_fill(h.max(t * 0.6), pr));
                // Chevron flips from up to down as the menu opens
                let c = r.center();
                let d = 3.0 * (1.0 - 2.0 * t);
                let st = EStroke::new(1.4_f32, mix(TEXT_DIM, Color32::WHITE, h.max(t)));
                p.line_segment([pos2(c.x - 5.0, c.y + d), pos2(c.x, c.y - d)], st);
                p.line_segment([pos2(c.x, c.y - d), pos2(c.x + 5.0, c.y + d)], st);
                if resp.on_hover_text("Canvas colour and shortcuts").clicked() {
                    toggle = true;
                }
            });

        const W: f32 = 196.0;
        let grad_row = 28.0 * ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_not_aurora"), THEME.load(std::sync::atomic::Ordering::Relaxed) != AURORA, 0.4 * anim_k(), egui::emath::easing::cubic_in_out);
        let grad_k = ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_grad_row"), gradient_on(), 0.55 * anim_k(), egui::emath::easing::cubic_in_out);
        let acc_open = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_acc_open")).unwrap_or(false));
        let pat_open = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_pat_open")).unwrap_or(false));
        let acc_k = ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_acc_k"), acc_open, 0.45 * anim_k(), egui::emath::easing::cubic_in_out);
        let pat_k = ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_pat_k"), pat_open, 0.45 * anim_k(), egui::emath::easing::cubic_in_out);
        let can_open = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_can_open")).unwrap_or(false));
        let can_k = ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_can_k"), can_open, 0.45 * anim_k(), egui::emath::easing::cubic_in_out);
        let gr_open = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_gr_open")).unwrap_or(false));
        let gr_k = ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_gr_k"), gr_open, 0.45 * anim_k(), egui::emath::easing::cubic_in_out);
        let _ = grad_k;
        let ex_open = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_ex_open")).unwrap_or(false));
        let ex_k = ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_ex_k"), ex_open, 0.45 * anim_k(), egui::emath::easing::cubic_in_out);
        let ap_open = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_ap_open")).unwrap_or(false));
        let am_open = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_am_open")).unwrap_or(false));
        let ap_k = ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_ap_k"), ap_open, 0.45 * anim_k(), egui::emath::easing::cubic_in_out);
        let am_k = ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_am_k"), am_open, 0.45 * anim_k(), egui::emath::easing::cubic_in_out);
        let au_h = (1.0 - grad_row / 28.0) * (56.0 + ap_k * 66.0 + am_k * 34.0);
        let H: f32 = 176.0 + au_h + ex_k * 56.0 + grad_row * (1.0 + gr_k * 66.0 / 28.0) + can_k * 66.0 + acc_k * 66.0 + pat_k * 64.0;
        let rise = (1.0 - t) * 10.0;
        let pop = Rect::from_min_size(pos2(btn.max.x - W, btn.min.y - 8.0 - H + rise), vec2(W, H));
        if toggle {
            self.panel_open = !self.panel_open;
        } else if self.panel_open {
            let (pressed, at, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos(), i.key_pressed(Key::Escape)));
            if esc || (pressed && at.map_or(false, |p| !pop.contains(p) && !btn.contains(p))) {
                self.panel_open = false;
            }
        }

        let ot = ctx.animate_bool_with_time(egui::Id::new("qd_theme_stagger"), self.panel_open, 0.6 * anim_k());
        if t > 0.0 {
            egui::Area::new(egui::Id::new("qd_theme_pop"))
                .order(egui::Order::Foreground)
                .fixed_pos(pop.min)
                .constrain(false)
                .interactable(self.panel_open)
                .show(ctx, |ui| {
                    let p = ui.painter().clone();
                    // Staggered reveal: rows rise into place one after another, bottom row first
                    let el = |k: f32| -> (f32, f32) {
                        let e = (ot * 1.7 - k * 0.1).clamp(0.0, 1.0);
                        let e = 1.0 - (1.0 - e).powi(3);
                        (e, (1.0 - e) * 18.0)
                    };
                    p.rect_filled(pop.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha((70.0 * t) as u8));
                    glass(&p, pop, 10.0, t);
                    p.rect_stroke(pop, 10.0, EStroke::new(1.0_f32, border().gamma_multiply(t)));
                    let lx = pop.min.x + 14.0;
                    let rx = pop.max.x - 14.0;
                    let t_all = t;
                    // Collapsible header: label, a preview on the right, and a chevron that flips open
                    let header = |ui: &mut egui::Ui, y: f32, label: &str, id: &str, k: f32, t: f32| -> (bool, f32) {
                        let row = Rect::from_min_max(pos2(pop.min.x + 6.0, y - 13.0), pos2(pop.max.x - 6.0, y + 13.0));
                        let resp = ui.interact(row, egui::Id::new(id), Sense::click());
                        let (h, pr) = hover_fx(ctx, &resp, true);
                        p.rect_filled(row, 6.0, hover_fill(h, pr).gamma_multiply(t));
                        p.text(pos2(lx, y), Align2::LEFT_CENTER, label, FontId::proportional(12.0), mix(TEXT_DIM, TEXT, h).gamma_multiply(t));
                        let c = pos2(rx - 3.0, y);
                        let d = 2.5 * (1.0 - 2.0 * k);
                        let st = EStroke::new(1.3_f32, mix(TEXT_FAINT, TEXT, h).gamma_multiply(t));
                        p.line_segment([pos2(c.x - 4.0, c.y - d), pos2(c.x, c.y + d)], st);
                        p.line_segment([pos2(c.x, c.y + d), pos2(c.x + 4.0, c.y - d)], st);
                        (resp.clicked(), h)
                    };
                    let p = p.with_clip_rect(ctx.screen_rect());
                    // One tile size for every picker row: seven across
                    let cell = |i: usize, y: f32, tp: f32| Rect::from_min_size(pos2(lx + (i % 5) as f32 * 32.0, y + 17.0 + (i / 5) as f32 * 32.0 + (1.0 - tp) * 10.0), vec2(26.0, 26.0));
                    // Two-stop bar preview: transparent on the left into the colour(s) on the right
                    let bar = |y: f32, top: Color32, bot: Color32, split: f32| {
                        use egui::epaint::{Mesh, Vertex, WHITE_UV};
                        let br = Rect::from_min_max(pos2(rx - 76.0, y - 5.0), pos2(rx - 14.0, y + 5.0));
                        let my = br.min.y + br.height() * split;
                        let l = Color32::TRANSPARENT;
                        let mut m = Mesh::default();
                        for (q, c) in [
                            (br.left_top(), l), (br.right_top(), top), (pos2(br.max.x, my), top), (pos2(br.min.x, my), l),
                            (pos2(br.min.x, my), l), (pos2(br.max.x, my), bot), (br.right_bottom(), bot), (br.left_bottom(), l),
                        ] {
                            m.vertices.push(Vertex { pos: q, uv: WHITE_UV, color: c });
                        }
                        for o in [0u32, 4] {
                            m.add_triangle(o, o + 1, o + 2);
                            m.add_triangle(o, o + 2, o + 3);
                        }
                        p.add(m);
                    };
                    // Canvas
                    let cur = THEME.load(std::sync::atomic::Ordering::Relaxed);
                    let cy = pop.min.y + 22.0;
                    let (ec, dc) = el(7.0);
                    let tc = t * ec;
                    let (clk, _) = header(ui, cy + dc, "Canvas", "qd_can_hdr", can_k, tc);
                    if clk {
                        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_can_open"), !can_open));
                    }
                    {
                        let [r, g, b] = theme_rgb();
                        let up = |v: u8, k: u8| v.saturating_add(k);
                        if cur == AURORA {
                            let pc = AURA_PALETTES[AURA_PAL.load(std::sync::atomic::Ordering::Relaxed) % AURA_PALETTES.len()].1;
                            bar(cy + dc, rgb32(pc[0]).gamma_multiply(tc), rgb32(pc[2]).gamma_multiply(tc), 0.5);
                        } else {
                            bar(cy + dc, Color32::from_rgb(up(r, 0x48), up(g, 0x48), up(b, 0x48)).gamma_multiply(tc), Color32::from_rgb(up(r, 0x22), up(g, 0x22), up(b, 0x22)).gamma_multiply(tc), 0.4);
                        }
                    }
                    if can_k > 0.001 {
                        for (i, (name, [r, g, b])) in THEMES.iter().enumerate() {
                            let tp = (can_k * 2.2 - i as f32 * 0.15).clamp(0.0, 1.0).min((ot * 2.0 - 0.3 - i as f32 * 0.08).clamp(0.0, 1.0));
                            let tp = 1.0 - (1.0 - tp).powi(3);
                            if tp <= 0.0 {
                                continue;
                            }
                            let tile = cell(i, cy, tp);
                            let resp = ui.interact(tile, egui::Id::new(("qd_theme_sw", i)), Sense::click());
                            let (h, pr) = hover_fx(ctx, &resp, true);
                            let on = spring(ctx, egui::Id::new(("qd_theme_on", i)), i == cur, 18.0);
                            let a = tc * tp;
                            let tl = tile.shrink(pr);
                            let base = [*r, *g, *b];
                            p.rect_filled(tl, 5.0, shade(base, -5).gamma_multiply(a));
                            if i == AURORA {
                                let sp = p.with_clip_rect(tl.shrink(1.0));
                                glow(&sp, tl.left_bottom(), 16.0, Color32::from_rgb(45, 212, 191).gamma_multiply(0.7 * a));
                                glow(&sp, tl.center_top(), 15.0, Color32::from_rgb(59, 130, 246).gamma_multiply(0.7 * a));
                                glow(&sp, tl.right_bottom(), 16.0, Color32::from_rgb(168, 85, 247).gamma_multiply(0.7 * a));
                            }
                            // Mini preview of the 3 tones: tab bar strip, canvas, a floating bar
                            p.rect_filled(Rect::from_min_size(tl.min, vec2(tl.width(), 6.0)), Rounding { nw: 5.0, ne: 5.0, sw: 0.0, se: 0.0 }, shade(base, -10).gamma_multiply(a));
                            let bar_r = Rect::from_center_size(pos2(tl.center().x, tl.max.y - 6.0), vec2(tl.width() * 0.55, 4.0));
                            p.rect_filled(bar_r, 2.0, shade(base, 0x14).gamma_multiply(a));
                            p.rect_stroke(tl, 5.0, EStroke::new(1.0 + on * 0.6, mix(mix(mix(border(), TEXT_FAINT, 0.55), TEXT_DIM, h), blue(), on).gamma_multiply(a)));
                            if resp.on_hover_text(*name).clicked() {
                                THEME.store(i, std::sync::atomic::Ordering::Relaxed);
                                retint(ctx);
                                self.settings.canvas_theme = i;
                                save_settings(&self.settings);
                            }
                        }
                    }
                    // Accent
                    // Gradient: off, or one of the direction presets (not on Aurora)
                    let cur_i = THEME.load(std::sync::atomic::Ordering::Relaxed);
                    let grad = cur_i != AURORA;
                    let ak = ctx.animate_bool_with_time_and_easing(egui::Id::new("qd_not_aurora"), grad, 0.4 * anim_k(), egui::emath::easing::cubic_in_out);
                    let gy = cy + 28.0 + 66.0 * can_k;
                    if ak > 0.01 {
                        let (eg, dg) = el(6.0);
                        let tg = t * eg * ak;
                        let on = gradient_on();
                        let cur_d = GRAD_DIR.load(std::sync::atomic::Ordering::Relaxed);
                        let (clk, gh) = header(ui, gy + dg, "Gradient", "qd_gr_hdr", gr_k, tg);
                        if clk && grad {
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_gr_open"), !gr_open));
                        }
                        // Same as Pattern: name the current choice (the gradients are too subtle for a bar)
                        let gname = if on { GRAD_DIRS[cur_d % GRAD_DIRS.len()] } else { "Off" };
                        p.text(pos2(rx - 14.0, gy + dg), Align2::RIGHT_CENTER, gname, FontId::proportional(11.0), mix(TEXT_FAINT, TEXT, gh).gamma_multiply(tg));
                        if gr_k > 0.001 {
                            for i in 0..=GRAD_DIRS.len() {
                                let tp = (gr_k * 2.2 - i as f32 * 0.15).clamp(0.0, 1.0).min((ot * 2.0 - 0.3 - i as f32 * 0.08).clamp(0.0, 1.0));
                                let tp = 1.0 - (1.0 - tp).powi(3);
                                if tp <= 0.0 {
                                    continue;
                                }
                                let tile = cell(i, gy, tp);
                                let resp = ui.interact(tile, egui::Id::new(("qd_grad_dir", i)), Sense::click());
                                let (h, pr) = hover_fx(ctx, &resp, true);
                                let sel = if i == 0 { !on } else { on && cur_d == i - 1 };
                                let o = spring(ctx, egui::Id::new(("qd_grad_dir_on", i)), sel, 18.0);
                                let a = tg * tp;
                                let tl = tile.shrink(pr);
                                if i == 0 {
                                    p.rect_filled(tl, 5.0, bg().gamma_multiply(a));
                                    p.line_segment([tl.left_bottom() + vec2(6.0, -6.0), tl.right_top() + vec2(-6.0, 6.0)], EStroke::new(1.2_f32, TEXT_FAINT.gamma_multiply(a)));
                                } else {
                                    p.rect_filled(tl, 5.0, bg().gamma_multiply(a));
                                    let gi = tl.shrink(4.0);
                                    fill_rounded_top(&p, gi, 0.0, |q| gradient_dir_at(q, gi, i - 1).gamma_multiply(a));
                                }
                                p.rect_stroke(tl, 5.0, EStroke::new(1.0 + o * 0.6, mix(mix(mix(border(), TEXT_FAINT, 0.55), TEXT_DIM, h), blue(), o).gamma_multiply(a)));
                                let name = if i == 0 { "No gradient" } else { GRAD_DIRS[i - 1] };
                                if resp.on_hover_text(name).clicked() {
                                    if i == 0 {
                                        self.settings.gradients &= !(1 << cur_i);
                                    } else {
                                        self.settings.gradients |= 1 << cur_i;
                                        self.settings.gradient_dir = (i - 1) as u8;
                                        GRAD_DIR.store(i - 1, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    GRADIENTS.store(self.settings.gradients, std::sync::atomic::Ordering::Relaxed);
                                    save_settings(&self.settings);
                                }
                            }
                        }
                    }
                    // Aurora only: colour combo and motion preset, in the gradient row's place
                    let au = 1.0 - ak;
                    if au > 0.01 {
                        let (eg, dg) = el(6.0);
                        let tg = t * eg * au;
                        let cur_p = AURA_PAL.load(std::sync::atomic::Ordering::Relaxed) % AURA_PALETTES.len();
                        let (clk, gh) = header(ui, gy + dg, "Palette", "qd_ap_hdr", ap_k, tg);
                        if clk {
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_ap_open"), !ap_open));
                        }
                        p.text(pos2(rx - 14.0, gy + dg), Align2::RIGHT_CENTER, AURA_PALETTES[cur_p].0, FontId::proportional(11.0), mix(TEXT_FAINT, TEXT, gh).gamma_multiply(tg));
                        if ap_k > 0.001 {
                            for (i, (name, cols)) in AURA_PALETTES.iter().enumerate() {
                                let tp = (ap_k * 2.2 - i as f32 * 0.15).clamp(0.0, 1.0).min((ot * 2.0 - 0.3 - i as f32 * 0.08).clamp(0.0, 1.0));
                                let tp = 1.0 - (1.0 - tp).powi(3);
                                if tp <= 0.0 {
                                    continue;
                                }
                                let tile = cell(i, gy, tp);
                                let resp = ui.interact(tile, egui::Id::new(("qd_ap", i)), Sense::click());
                                let (h, pr) = hover_fx(ctx, &resp, true);
                                let o = spring(ctx, egui::Id::new(("qd_ap_on", i)), i == cur_p, 18.0);
                                let a = tg * tp;
                                let tl = tile.shrink(pr);
                                p.rect_filled(tl, 5.0, bg().gamma_multiply(a));
                                let sp = p.with_clip_rect(tl.shrink(1.0));
                                glow(&sp, tl.left_bottom(), 16.0, rgb32(cols[0]).gamma_multiply(0.8 * a));
                                glow(&sp, tl.center_top(), 15.0, rgb32(cols[1]).gamma_multiply(0.8 * a));
                                glow(&sp, tl.right_bottom(), 16.0, rgb32(cols[2]).gamma_multiply(0.8 * a));
                                p.rect_stroke(tl, 5.0, EStroke::new(1.0 + o * 0.6, mix(mix(mix(border(), TEXT_FAINT, 0.55), TEXT_DIM, h), blue(), o).gamma_multiply(a)));
                                if resp.on_hover_text(*name).clicked() {
                                    AURA_PAL.store(i, std::sync::atomic::Ordering::Relaxed);
                                    self.settings.aura_palette = i as u8;
                                    save_settings(&self.settings);
                                }
                            }
                        }
                        let my = gy + 28.0 + 66.0 * ap_k;
                        let cur_m = AURA_MOT.load(std::sync::atomic::Ordering::Relaxed) % AURA_MOTIONS.len();
                        let (clk, mh) = header(ui, my + dg, "Motion", "qd_am_hdr", am_k, tg);
                        if clk {
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_am_open"), !am_open));
                        }
                        p.text(pos2(rx - 14.0, my + dg), Align2::RIGHT_CENTER, AURA_MOTIONS[cur_m].0, FontId::proportional(11.0), mix(TEXT_FAINT, TEXT, mh).gamma_multiply(tg));
                        if am_k > 0.001 {
                            let now = ctx.input(|i| i.time) as f32;
                            for (i, (name, speed, sway, breathe)) in AURA_MOTIONS.iter().enumerate() {
                                let tp = (am_k * 2.2 - i as f32 * 0.15).clamp(0.0, 1.0).min((ot * 2.0 - 0.3 - i as f32 * 0.08).clamp(0.0, 1.0));
                                let tp = 1.0 - (1.0 - tp).powi(3);
                                if tp <= 0.0 {
                                    continue;
                                }
                                let tile = cell(i, my, tp);
                                let resp = ui.interact(tile, egui::Id::new(("qd_am", i)), Sense::click());
                                let (h, pr) = hover_fx(ctx, &resp, true);
                                let o = spring(ctx, egui::Id::new(("qd_am_on", i)), i == cur_m, 18.0);
                                let a = tg * tp;
                                let tl = tile.shrink(pr);
                                p.rect_filled(tl, 5.0, bg().gamma_multiply(a));
                                // Live preview: a dot moving the way this preset moves
                                let tt = now * speed * 1.6;
                                let c = tl.center() + vec2(6.0 * sway * (tt + i as f32).sin(), 4.0 * sway * (tt * 1.3 + i as f32).cos());
                                let rr = 3.5 * (1.0 + breathe * 2.0 * (now * 2.0).sin());
                                p.circle_filled(c, rr, mix(TEXT_DIM, TEXT, h.max(o)).gamma_multiply(a));
                                p.rect_stroke(tl, 5.0, EStroke::new(1.0 + o * 0.6, mix(mix(mix(border(), TEXT_FAINT, 0.55), TEXT_DIM, h), blue(), o).gamma_multiply(a)));
                                if resp.on_hover_text(*name).clicked() {
                                    AURA_MOT.store(i, std::sync::atomic::Ordering::Relaxed);
                                    self.settings.aura_motion = i as u8;
                                    save_settings(&self.settings);
                                }
                            }
                            ctx.request_repaint();
                        }
                    }
                    let ay = gy + (28.0 + 66.0 * gr_k) * ak + (56.0 + 66.0 * ap_k + 34.0 * am_k) * au;
                    let (ea, da) = el(5.0);
                    let ta = t * ea;
                    let cur_a = ACCENT.load(std::sync::atomic::Ordering::Relaxed);
                    let (clk, _) = header(ui, ay + da, "Accent", "qd_acc_hdr", acc_k, ta);
                    if clk {
                        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_acc_open"), !acc_open));
                    }
                    bar(ay + da, blue().gamma_multiply(ta), blue().gamma_multiply(ta), 0.5);
                    if acc_k > 0.001 {
                        for (i, (name, rgb)) in ACCENTS.iter().enumerate() {
                            let tp = (acc_k * 2.2 - i as f32 * 0.15).clamp(0.0, 1.0).min((ot * 2.0 - 0.3 - i as f32 * 0.08).clamp(0.0, 1.0));
                            let tp = 1.0 - (1.0 - tp).powi(3);
                            if tp <= 0.0 {
                                continue;
                            }
                            let tile = cell(i, ay, tp);
                            let resp = ui.interact(tile, egui::Id::new(("qd_accent", i)), Sense::click());
                            let (h, pr) = hover_fx(ctx, &resp, true);
                            let on = spring(ctx, egui::Id::new(("qd_accent_on", i)), *rgb == cur_a, 18.0);
                            let col = Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, *rgb as u8);
                            let a = ta * tp;
                            let tl = tile.shrink(pr);
                            p.rect_filled(tl, 5.0, bg().gamma_multiply(a));
                            p.rect_filled(tl.shrink(4.0), 3.0, col.gamma_multiply(a * (0.75 + 0.25 * h.max(on))));
                            p.rect_stroke(tl, 5.0, EStroke::new(1.0 + on * 0.6, mix(mix(border(), TEXT_DIM, h), col, on).gamma_multiply(a)));
                            if resp.on_hover_text(*name).clicked() {
                                ACCENT.store(*rgb, std::sync::atomic::Ordering::Relaxed);
                                self.settings.accent = *rgb;
                                save_settings(&self.settings);
                            }
                        }
                    }
                    // Pattern
                    let py = ay + 28.0 + 66.0 * acc_k;
                    let (e4, d4) = el(4.0);
                    let t = t * e4;
                    let pi = (self.settings.pattern as usize) % PATTERNS.len();
                    let (clk, ph) = header(ui, py + d4, "Pattern", "qd_pat_hdr", pat_k, t);
                    if clk {
                        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_pat_open"), !pat_open));
                    }
                    p.text(pos2(rx - 14.0, py + d4), Align2::RIGHT_CENTER, PATTERNS[pi], FontId::proportional(11.0), mix(TEXT_FAINT, TEXT, ph).gamma_multiply(t));
                    if pat_k > 0.001 {
                        for i in 0..PATTERNS.len() {
                            let tp = (pat_k * 2.2 - i as f32 * 0.15).clamp(0.0, 1.0).min((ot * 2.0 - 0.3 - i as f32 * 0.08).clamp(0.0, 1.0));
                            let tp = 1.0 - (1.0 - tp).powi(3);
                            if tp <= 0.0 {
                                continue;
                            }
                            let a = t * tp;
                            let tile = cell(i, py, tp);
                            let resp = ui.interact(tile, egui::Id::new(("qd_pat_tile", i)), Sense::click());
                            let (h, pr) = hover_fx(ctx, &resp, true);
                            let on = spring(ctx, egui::Id::new(("qd_pat_on", i)), i == pi, 18.0);
                            let tl = tile.shrink(pr);
                            p.rect_filled(tl, 5.0, bg().gamma_multiply(a));
                            p.rect_stroke(tl, 5.0, EStroke::new(1.0 + on * 0.6, mix(mix(border(), TEXT_DIM, h), blue(), on).gamma_multiply(a)));
                            draw_pattern(&p, tl.center(), i as u8, mix(TEXT_DIM, TEXT, h.max(on)).gamma_multiply(a), 1.4);
                            if resp.on_hover_text(PATTERNS[i]).clicked() {
                                self.settings.pattern = i as u8;
                                save_settings(&self.settings);
                            }
                        }
                        // Strength slider
                        let ty = py + 62.0 + (1.0 - pat_k) * 10.0;
                        let t = t * ((pat_k - 0.3) / 0.7).clamp(0.0, 1.0);
                        p.text(pos2(lx, ty), Align2::LEFT_CENTER, "Opacity", FontId::proportional(11.0), TEXT_DIM.gamma_multiply(t));
                        let track = Rect::from_min_max(pos2(lx + 56.0, ty - 9.0), pos2(rx - 44.0, ty + 9.0));
                        let resp = ui.interact(track, egui::Id::new("qd_dots"), Sense::click_and_drag());
                        if let (true, Some(pp)) = (resp.dragged() || resp.clicked(), resp.interact_pointer_pos()) {
                            self.settings.grid_dots = (((pp.x - track.min.x) / track.width()).clamp(0.0, 1.0) * 100.0).round();
                        }
                        if resp.drag_stopped() || resp.clicked() {
                            save_settings(&self.settings);
                        }
                        let f = self.settings.grid_dots / 100.0;
                        let (h, _) = hover_fx(ctx, &resp, true);
                        let line = Rect::from_min_max(pos2(track.min.x, ty - 2.0), pos2(track.max.x, ty + 2.0));
                        p.rect_filled(line, 2.0, border().gamma_multiply(t));
                        p.rect_filled(Rect::from_min_max(line.min, pos2(line.min.x + line.width() * f, line.max.y)), 2.0, blue().gamma_multiply(t));
                        p.circle_filled(pos2(track.min.x + track.width() * f, ty), 6.0 + h, Color32::WHITE.gamma_multiply(t));
                        p.text(pos2(rx - 14.0, ty), Align2::RIGHT_CENTER, format!("{:.0}", self.settings.grid_dots), FontId::proportional(11.0), TEXT_FAINT.gamma_multiply(t));
                    }
                    let ty0 = py + 64.0 * pat_k;
                    let t = t_all;
                    // Switch rows
                    let mut switch = |ui: &mut egui::Ui, y: f32, label: &str, on: bool, id: &str, fade: f32| -> bool {
                        let t = t * fade;
                        let row = Rect::from_min_max(pos2(pop.min.x + 6.0, y - 13.0), pos2(pop.max.x - 6.0, y + 13.0));
                        let resp = ui.interact(row, egui::Id::new(id), Sense::click());
                        let (h, pr) = hover_fx(ctx, &resp, true);
                        p.rect_filled(row, 6.0, hover_fill(h, pr).gamma_multiply(t));
                        p.text(pos2(lx, y), Align2::LEFT_CENTER, label, FontId::proportional(12.0), mix(TEXT_DIM, TEXT, h).gamma_multiply(t));
                        let k = ease(ctx, egui::Id::new((id, "k")), on);
                        let sw = Rect::from_min_size(pos2(rx - 44.0, y - 8.0), vec2(30.0, 16.0));
                        p.rect_filled(sw, 8.0, mix(border(), blue(), k).gamma_multiply(t));
                        p.circle_filled(pos2(sw.min.x + 8.0 + 14.0 * k, y), 6.0, Color32::WHITE.gamma_multiply(t));
                        resp.clicked()
                    };
                    let (e1, d1) = el(1.0);
                    let (e0, d0) = el(0.0);
                    // Extras: the less-used switches, tucked in a dropdown like the rest
                    let ey = ty0 + 28.0;
                    let (clk, _) = header(ui, ey + d1, "Extras", "qd_ex_hdr", ex_k, t * e1);
                    if clk {
                        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_ex_open"), !ex_open));
                    }
                    let r0 = (ex_k * 2.0).clamp(0.0, 1.0).min((ot * 2.0 - 0.3).clamp(0.0, 1.0));
                    let r1 = (ex_k * 2.0 - 0.3).clamp(0.0, 1.0).min((ot * 2.0 - 0.38).clamp(0.0, 1.0));
                    let y1 = ey + 28.0 + (1.0 - r0) * 10.0;
                    if r0 > 0.01 && switch(ui, y1 + d1, "Rounded corners", self.settings.round_corners, "qd_sw_round", e1 * r0) {
                        self.settings.round_corners = !self.settings.round_corners;
                        self.corners_dirty = true;
                        save_settings(&self.settings);
                    }

                    // Animation strength: click cycles
                    let ay = ey + 56.0 + d1 + (1.0 - r1) * 10.0;
                    if r1 > 0.01 {
                        let t = t * e1 * r1;
                        let row = Rect::from_min_max(pos2(pop.min.x + 6.0, ay - 13.0), pos2(pop.max.x - 6.0, ay + 13.0));
                        let resp = ui.interact(row, egui::Id::new("qd_sw_anim"), Sense::click());
                        let (h, pr) = hover_fx(ctx, &resp, true);
                        p.rect_filled(row, 6.0, hover_fill(h, pr).gamma_multiply(t));
                        p.text(pos2(lx, ay), Align2::LEFT_CENTER, "Animations", FontId::proportional(12.0), mix(TEXT_DIM, TEXT, h).gamma_multiply(t));
                        let names = ["Full", "Subtle", "Off"];
                        p.text(pos2(rx - 14.0, ay), Align2::RIGHT_CENTER, names[self.settings.anim.min(2) as usize], FontId::proportional(12.0), mix(blue(), Color32::WHITE, h * 0.4).gamma_multiply(t));
                        if resp.clicked() {
                            self.settings.anim = (self.settings.anim + 1) % 3;
                            ANIM.store(self.settings.anim, std::sync::atomic::Ordering::Relaxed);
                            save_settings(&self.settings);
                        }
                    }

                    // Divider + shortcuts entry
                    let dy = ey + 18.0 + 56.0 * ex_k + d0;
                    let t = t * e0;
                    p.line_segment([pos2(pop.min.x + 1.0, dy), pos2(pop.max.x - 1.0, dy)], EStroke::new(1.0_f32, border().gamma_multiply(t)));
                    let row = Rect::from_min_max(pos2(pop.min.x + 6.0, dy + 5.0), pos2(pop.max.x - 6.0, pop.max.y - 6.0));
                    let resp = ui.interact(row, egui::Id::new("qd_theme_keys"), Sense::click());
                    let (h, pr) = hover_fx(ctx, &resp, true);
                    p.rect_filled(row, 6.0, hover_fill(h, pr).gamma_multiply(t));
                    p.text(pos2(row.min.x + 8.0, row.center().y), Align2::LEFT_CENTER, "Keyboard shortcuts", FontId::proportional(12.5), TEXT.gamma_multiply(t));
                    p.text(pos2(row.max.x - 8.0, row.center().y), Align2::RIGHT_CENTER, "›", FontId::proportional(14.0), mix(TEXT_FAINT, TEXT, h).gamma_multiply(t));
                    if resp.clicked() {
                        self.panel_open = false;
                        self.shortcuts_open = true;
                        self.hk_error.clear();
                    }
                });
        }
        self.shortcuts_ui(ctx);
    }
}
