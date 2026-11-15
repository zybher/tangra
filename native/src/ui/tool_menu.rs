//! Per-tool option menus opened from the toolbar.

use crate::*;

impl App {
    /// Options for a toolbar button, opened by right-clicking it. A short list;
    /// hovering a row slides its choices out to the right.
    pub(crate) fn tool_menu_ui(&mut self, ctx: &egui::Context) {
        if let Some(m) = self.tool_menu {
            self.tool_menu_last = m;
        }
        let t = ease(ctx, egui::Id::new("qd_tool_menu"), self.tool_menu.is_some());
        // Runs every frame so the open/close cascade always starts from the right place
        let ot = ctx.animate_bool_with_time(egui::Id::new("qd_tm_stagger"), self.tool_menu.is_some(), 0.45 * anim_k());
        if t <= 0.01 {
            return;
        }
        let (kind, btn) = self.tool_menu_last;
        const WIDTHS: [(&str, f32); 5] = [("Hairline", 1.0), ("Thin", 2.0), ("Medium", 4.0), ("Thick", 8.0), ("Extra thick", 14.0)];
        const STYLES: [&str; 3] = ["Outline", "Filled", "Fill only"];
        const SIZES: [(&str, f32); 4] = [("Small", 12.0), ("Normal", 16.0), ("Large", 24.0), ("Huge", 36.0)];
        const BSIZES: [(&str, f32); 4] = [("Small", 16.0), ("Medium", 28.0), ("Large", 48.0), ("Huge", 80.0)];
        const TSTYLES: [&str; 4] = ["Regular", "Bold", "Italic", "Bold italic"];
        let zi = SIZES.iter().position(|z| (z.1 - self.text_size).abs() < 0.01);
        const PALETTE: [&str; 15] = [
            "#ef4444", "#f97316", "#f59e0b", "#facc15", "#84cc16", "#22c55e", "#14b8a6", "#06b6d4", "#3b82f6", "#a855f7", "#ec4899", "#ffffff", "#94a3b8", "#475569", "#000000",
        ];
        let rows: Vec<char> = match kind {
            0 => vec!['d'],
            1 => vec!['f', 'r'],
            2 => vec!['s', 'w', 'c'],
            4 => vec!['z', 'b', 'c'],
            5 => vec!['p'],
            _ => vec!['w', 'c'],
        };
        let wi = WIDTHS.iter().position(|w| (w.1 - self.pen_width).abs() < 0.01);
        let value = |c: char| -> String {
            match c {
                'd' => if self.hand_drag { "Pan" } else { "Select" }.into(),
                'f' => FRAME_SIZES[self.frame_size].0.into(),
                'r' => if self.frame_square { "Square" } else { "Rounded" }.into(),
                'z' => zi.map_or(format!("{:.0}px", self.text_size), |i| SIZES[i].0.into()),
                'b' => TSTYLES[self.text_style as usize].into(),
                's' => STYLES[self.shape_fill as usize].into(),
                'w' => wi.map_or(format!("{:.0}px", self.pen_width), |i| WIDTHS[i].0.into()),
                'p' => BSIZES.iter().find(|b| (b.1 - self.blur_size).abs() < 0.01).map_or(format!("{:.0}px", self.blur_size), |b| b.0.into()),
                _ => String::new(),
            }
        };
        let title = |c: char| match c {
            'd' => "Drag on canvas",
            'f' => "Frame size",
            'r' => "Corners",
            'z' => "Size",
            'b' => "Style",
            's' => "Style",
            'w' => "Thickness",
            'p' => "Brush size",
            _ => "Colour",
        };

        let (mw, rh) = (190.0_f32, 30.0_f32);
        let bottom = btn.min.y - 14.0 + (1.0 - t) * 8.0;
        let pinned: Vec<f32> = (0..rows.len()).map(|i| ease(ctx, egui::Id::new(("qd_tm_open", kind, i)), self.tool_sub == Some(i))).collect();
        // Switching tools: the panel glides to the new button and resizes, rows fade back in
        let now = ctx.input(|i| i.time);
        let dt = ctx.input(|i| i.stable_dt).min(0.05);
        let kid = egui::Id::new("qd_tm_kind");
        // Any switch to another button (even same menu kind) replays the cascade
        let key = (kind, btn.center().x.round() as i32);
        let (last_k, since): ((u8, i32), f64) = ctx.data(|d| d.get_temp(kid)).unwrap_or((key, 0.0));
        let since = if last_k != key && t > 0.5 { now } else { since };
        ctx.data_mut(|d| d.insert_temp(kid, (key, since)));
        let kf = ((now - since) / 0.45).clamp(0.0, 1.0) as f32;
        if kf < 1.0 {
            ctx.request_repaint();
        }
        let glide = |id: &str, target: f32| -> f32 {
            let gid = egui::Id::new(id);
            let cur: f32 = ctx.data(|d| d.get_temp(gid)).unwrap_or(target);
            let v = if t < 0.95 && kf >= 1.0 { target } else { cur + (target - cur) * (1.0 - (-dt * 16.0).exp()) };
            if (v - target).abs() > 0.3 {
                ctx.request_repaint();
            }
            ctx.data_mut(|d| d.insert_temp(gid, v));
            v
        };
        let ax = glide("qd_tm_ax", btn.center().x);
        let (rrs, mags, main) = wave_rows(ctx, egui::Id::new(("qd_tm_wave_main", kind)), ax - mw / 2.0, bottom, mw, rh, rows.len(), &pinned);
        let bg_top = glide("qd_tm_top", main.min.y);
        let bg = Rect::from_min_max(pos2(main.min.x, bg_top), main.max);
        let open_m = self.tool_sub.and_then(|i| mags.get(i).copied()).unwrap_or(0.0);
        let pen_col = hex(&self.pen_color);
        let mut hover_row = None;
        egui::Area::new(egui::Id::new("qd_tool_menu_area"))
            .order(egui::Order::Foreground)
            .fixed_pos(main.min)
            .constrain(false)
            .interactable(t > 0.5)
            .show(ctx, |ui| {
                let _ = ui.allocate_rect(main, Sense::hover());
                let p = &ui.painter().clone().with_clip_rect(ctx.screen_rect());
                glass(&p, bg, 10.0, t);
                p.rect_stroke(bg, 10.0, EStroke::new(1.0_f32, border().gamma_multiply(t)));
                for (i, c) in rows.iter().enumerate() {
                    // Rows rise in one after another after a switch
                    let from_bottom = (rows.len() - 1 - i) as f32;
                    let cascade = |x: f32| {
                        let e = (x * 1.5 - from_bottom * 0.15).clamp(0.0, 1.0);
                        1.0 - (1.0 - e).powi(3)
                    };
                    let rk = cascade(kf).min(cascade(ot));
                    let t = t * rk;
                    let (r, wm) = (rrs[i].translate(vec2(0.0, (1.0 - rk) * 16.0)), mags[i]);
                    let resp = ui.interact(r, egui::Id::new(("qd_tm_row", kind, i)), Sense::hover());
                    if resp.hovered() {
                        hover_row = Some(i);
                    }
                    wave_tile(p, r, wm, mix(popup(), Color32::from_gray(90), 0.35), t);
                    let fsz = 12.0 * (1.0 + 0.14 * wm);
                    let hl = wm;
                    p.text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, title(*c), FontId::proportional(fsz), mix(TEXT_DIM, TEXT, hl).gamma_multiply(t));
                    p.text(pos2(r.max.x - 10.0, r.center().y), Align2::RIGHT_CENTER, "›", FontId::proportional(14.0), mix(TEXT_FAINT, TEXT, hl).gamma_multiply(t));
                    if *c == 'c' {
                        p.circle_filled(pos2(r.max.x - 28.0, r.center().y), 6.0, pen_col.gamma_multiply(t));
                        p.circle_stroke(pos2(r.max.x - 28.0, r.center().y), 6.0, EStroke::new(1.0_f32, Color32::from_white_alpha((60.0 * t) as u8)));
                    } else {
                        p.text(pos2(r.max.x - 24.0, r.center().y), Align2::RIGHT_CENTER, value(*c), FontId::proportional(11.5), mix(TEXT_FAINT, TEXT_DIM, hl).gamma_multiply(t));
                    }
                }
            });
        if hover_row.is_some() && hover_row != self.tool_sub {
            self.tool_sub = hover_row;
            self.tool_sub_since = ctx.input(|i| i.time);
        }

        // Submenu, to the right of the list, bottom edges lined up (so it stays clear of the toolbar)
        let mut sub_rect = Rect::NOTHING;
        enum Pick {
            Item(char, usize),
            Colour(Color32),
        }
        let mut pick: Option<Pick> = None;
        if let Some(si) = self.tool_sub.filter(|i| *i < rows.len()) {
            let c = rows[si];
            let k = ((ctx.input(|i| i.time) - self.tool_sub_since) / 0.16).clamp(0.0, 1.0) as f32;
            if k < 1.0 {
                ctx.request_repaint();
            }
            let st = (1.0 - (1.0 - k).powi(3)) * t;
            // Same bottom-up cascade as the main dropdown
            let sk = ((ctx.input(|i| i.time) - self.tool_sub_since) / 0.45).clamp(0.0, 1.0) as f32;
            if sk < 1.0 {
                ctx.request_repaint();
            }
            let cascade = |from_bottom: f32| {
                let e = (sk * 1.5 - from_bottom * 0.1).clamp(0.0, 1.0);
                1.0 - (1.0 - e).powi(3)
            };
            let items: Vec<String> = match c {
                'd' => vec!["Box select".into(), "Pan (hand)".into()],
                'f' => FRAME_SIZES.iter().map(|f| f.0.to_string()).collect(),
                'r' => vec!["Rounded".into(), "Square".into()],
                'z' => SIZES.iter().map(|x| x.0.to_string()).collect(),
                'b' => TSTYLES.iter().map(|x| x.to_string()).collect(),
                's' => STYLES.iter().map(|x| x.to_string()).collect(),
                'w' => WIDTHS.iter().map(|x| x.0.to_string()).collect(),
                'p' => BSIZES.iter().map(|x| x.0.to_string()).collect(),
                _ => Vec::new(),
            };
            let sel = match c {
                'd' => Some(self.hand_drag as usize),
                'f' => Some(self.frame_size),
                'r' => Some(self.frame_square as usize),
                'z' => zi,
                'b' => Some(self.text_style as usize),
                's' => Some(self.shape_fill as usize),
                'w' => wi,
                'p' => BSIZES.iter().position(|b| (b.1 - self.blur_size).abs() < 0.01),
                _ => None,
            };
            let widest = items.iter().map(|x| ctx.fonts(|fo| fo.layout_no_wrap(x.clone(), FontId::proportional(12.5), Color32::WHITE).size().x)).fold(0.0_f32, f32::max);
            let lead = match c {
                'w' => 52.0,
                'z' => 34.0,
                'f' | 'r' | 's' => 32.0,
                _ => 0.0,
            };
            let (sw, sh) = if c == 'c' { (12.0 * 2.0 + 5.0 * 30.0 - 8.0, 10.0 + 3.0 * 30.0 + 4.0 + 14.0 + 14.0) } else { ((widest * 1.15 + lead + 30.0).max(70.0), items.len() as f32 * 28.0 + 8.0) };
            let sub_left = main.max.x + 6.0 + 14.0 * open_m - (1.0 - st) * 10.0;
            let sub_pinned: Vec<f32> = (0..items.len()).map(|i| ease(ctx, egui::Id::new(("qd_tm_on", c, i)), sel == Some(i))).collect();
            let (srows, smags, sp) = wave_rows(ctx, egui::Id::new(("qd_tm_wave_sub", c)), sub_left, main.max.y, sw, 28.0, items.len(), &sub_pinned);
            sub_rect = if c == 'c' { Rect::from_min_size(pos2(sub_left, main.max.y - sh), vec2(sw, sh)) } else { sp };
            let r0 = sub_rect;
            egui::Area::new(egui::Id::new("qd_tool_sub_area"))
                .order(egui::Order::Foreground)
                .fixed_pos(r0.min)
                .constrain(false)
                .interactable(t > 0.5)
                .show(ctx, |ui| {
                    let _ = ui.allocate_rect(r0, Sense::hover());
                    let p = &ui.painter().clone().with_clip_rect(ctx.screen_rect());
                    p.rect_filled(r0, 10.0, popup().gamma_multiply(st));
                    p.rect_stroke(r0, 10.0, EStroke::new(1.0_f32, border().gamma_multiply(st)));
                    if c == 'c' {
                        let cur = hex(&self.pen_color);
                        let top = r0.min.y + 10.0;
                        // Wave over the grid: smoothed pointer focus, swatches near it grow into rounded squares
                        let fid = egui::Id::new("qd_tm_sw_focus");
                        let ptr = ctx.input(|i| i.pointer.hover_pos());
                        let grid = Rect::from_min_size(pos2(r0.min.x + 8.0, top - 4.0), vec2(r0.width() - 16.0, 3.0 * 30.0 + 4.0));
                        let inside = ptr.filter(|q| grid.contains(*q));
                        let pres = spring(ctx, egui::Id::new("qd_tm_sw_pres"), inside.is_some(), 20.0);
                        let prev: Pos2 = ctx.data(|d| d.get_temp(fid)).unwrap_or(inside.unwrap_or(grid.center()));
                        let dt = ctx.input(|i| i.stable_dt).min(0.05);
                        let f = match inside {
                            Some(q) => {
                                let k = 1.0 - (-14.0 * dt).exp();
                                if (q - prev).length() > 0.3 {
                                    ctx.request_repaint();
                                }
                                prev + (q - prev) * k
                            }
                            None => prev,
                        };
                        ctx.data_mut(|d| d.insert_temp(fid, f));
                        for (i, hx) in PALETTE.iter().enumerate() {
                            let sp = cascade(1.0 + (2 - i / 5) as f32 + (i % 5) as f32 * 0.08);
                            let base = pos2(r0.min.x + 12.0 + 11.0 + (i % 5) as f32 * 30.0, top + 11.0 + (i / 5) as f32 * 30.0);
                            let col = hex(hx);
                            let on = ease(ctx, egui::Id::new(("qd_tm_sw_on", i)), self.pen_color.eq_ignore_ascii_case(hx));
                            let d = (base - f).length() / 22.0;
                            let m = (0.7 * pres * (-d * d).exp()).max(on);
                            let ctr = base + vec2(0.0, (1.0 - sp) * 14.0 - 3.0 * m);
                            let hit = Rect::from_center_size(base, vec2(28.0, 28.0));
                            let resp = ui.interact(hit, egui::Id::new(("qd_tm_sw", i)), Sense::click());
                            let (hv, pr) = hover_fx(ctx, &resp, true);
                            let m = m.max(hv);
                            let rad = 9.0 + 3.0 * m - pr;
                            let sq = Rect::from_center_size(ctr, vec2(rad * 2.0, rad * 2.0));
                            let round = rad * (1.0 - 0.62 * m * m);
                            let a = st * sp;
                            p.rect_filled(sq, round, col.gamma_multiply(a));
                            p.rect_stroke(sq, round, EStroke::new(1.0_f32, Color32::from_white_alpha((40.0 + 50.0 * m) as u8).gamma_multiply(a)));
                            if on > 0.01 {
                                p.rect_stroke(sq.expand(3.0), round + 3.0, EStroke::new(2.0_f32, Color32::WHITE.gamma_multiply(a * on)));
                            }
                            if hv > 0.01 {
                                p.rect_stroke(sq.expand(3.0 + 2.5 * on), round + 3.0, EStroke::new(2.5_f32, blue().gamma_multiply(a * hv)));
                            }
                            if resp.clicked() {
                                pick = Some(Pick::Colour(col));
                            }
                        }
                        // Any hue: click or drag along the rainbow
                        let bar = Rect::from_min_size(pos2(r0.min.x + 12.0, top + 3.0 * 30.0 + 4.0), vec2(r0.width() - 24.0, 14.0));
                        let rr = bar.height() / 2.0;
                        let hue = |f: f32| -> Color32 { egui::ecolor::Hsva::new(f, 0.8, 0.95, 1.0).into() };
                        p.circle_filled(pos2(bar.min.x + rr, bar.center().y), rr, hue(0.0).gamma_multiply(st));
                        p.circle_filled(pos2(bar.max.x - rr, bar.center().y), rr, hue(0.999).gamma_multiply(st));
                        // One gradient mesh (separate slices left hairline seams between them)
                        let inner = bar.shrink2(vec2(rr, 0.0));
                        let n = 48;
                        let mut m = egui::Mesh::default();
                        for k in 0..=n {
                            let f = k as f32 / n as f32;
                            let x = inner.min.x + inner.width() * f;
                            let c = hue(f.min(0.999)).gamma_multiply(st);
                            m.colored_vertex(pos2(x, bar.min.y), c);
                            m.colored_vertex(pos2(x, bar.max.y), c);
                            if k > 0 {
                                let i = (k * 2) as u32;
                                m.add_triangle(i - 2, i - 1, i);
                                m.add_triangle(i - 1, i + 1, i);
                            }
                        }
                        p.add(egui::Shape::mesh(m));
                        p.rect_stroke(bar, rr, EStroke::new(1.0_f32, Color32::from_white_alpha((35.0 * st) as u8)));
                        let resp = ui.interact(bar.expand(4.0), egui::Id::new("qd_tm_hue"), Sense::click_and_drag());
                        let (hv, _) = hover_fx(ctx, &resp, true);
                        let ch = egui::ecolor::Hsva::from(hex(&self.pen_color));
                        let kx = inner.min.x + inner.width() * ch.h;
                        let kr = 8.0 + 1.5 * hv;
                        p.circle_filled(pos2(kx, bar.center().y), kr, Color32::WHITE.gamma_multiply(st));
                        p.circle_filled(pos2(kx, bar.center().y), kr - 2.5, hue(ch.h).gamma_multiply(st));
                        if resp.clicked() || resp.dragged() {
                            if let Some(q) = resp.interact_pointer_pos() {
                                let f = ((q.x - inner.min.x) / inner.width()).clamp(0.0, 0.999);
                                pick = Some(Pick::Colour(hue(f)));
                            }
                        }
                    } else {
                        for (i, label) in items.iter().enumerate() {
                            let ck = cascade((items.len() - 1 - i) as f32);
                            let st = st * ck;
                            let (r, wm) = (srows[i].translate(vec2(0.0, (1.0 - ck) * 14.0)), smags[i]);
                            let resp = ui.interact(r, egui::Id::new(("qd_tm_item", c, i)), Sense::click());
                            let (_, pr) = hover_fx(ctx, &resp, true);
                            let on = sub_pinned[i];
                            wave_tile(p, r, wm, mix(mix(popup(), Color32::from_gray(90), 0.35), blue(), on), st);
                            p.rect_filled(r, 8.0, hover_fill(0.0, pr).gamma_multiply(st));
                            let tc = mix(mix(TEXT_DIM, TEXT, wm), Color32::WHITE, on).gamma_multiply(st);
                            if c == 'w' {
                                let lw = WIDTHS[i].1.min(10.0);
                                p.line_segment([pos2(r.min.x + 10.0, r.center().y), pos2(r.min.x + 50.0, r.center().y)], EStroke::new(lw, tc));
                                p.text(pos2(r.min.x + 62.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0 * (1.0 + 0.14 * wm)), tc);
                            } else if c == 'b' {
                                let g = label_galley(p, label, FontId::proportional(12.5 * (1.0 + 0.14 * wm)), tc, i as u8);
                                let gp = pos2(r.min.x + 10.0, r.center().y - g.size().y / 2.0);
                                draw_label(p, gp, g, tc, i as u8, 12.5);
                            } else if c == 'z' {
                                p.text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, "Aa", FontId::proportional(SIZES[i].1.min(20.0) * 0.8), tc);
                                p.text(pos2(r.min.x + 44.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0 * (1.0 + 0.14 * wm)), tc);
                            } else if matches!(c, 'f' | 'r' | 's') {
                                let ic = pos2(r.min.x + 21.0, r.center().y);
                                let sk = EStroke::new(1.4_f32, tc);
                                match c {
                                    'f' => {
                                        let (_, w0, h0) = FRAME_SIZES[i];
                                        let sc = 16.0 / w0.max(h0);
                                        let fr = Rect::from_center_size(ic, vec2(w0 * sc, h0 * sc));
                                        if i == 0 {
                                            // Free: dashed outline
                                            let pts = [fr.left_top(), fr.right_top(), fr.right_bottom(), fr.left_bottom(), fr.left_top()];
                                            p.extend(egui::Shape::dashed_line(&pts, sk, 2.5, 2.0));
                                        } else {
                                            p.rect_stroke(fr, 2.0, sk);
                                        }
                                    }
                                    'r' => {
                                        p.rect_stroke(Rect::from_center_size(ic, vec2(16.0, 13.0)), if i == 0 { 4.5 } else { 0.0 }, sk);
                                    }
                                    _ => {
                                        let rr = Rect::from_center_size(ic, vec2(16.0, 13.0));
                                        if i > 0 {
                                            p.rect_filled(rr, 3.0, tc.gamma_multiply(if i == 1 { 0.45 } else { 1.0 }));
                                        }
                                        if i < 2 {
                                            p.rect_stroke(rr, 3.0, sk);
                                        }
                                    }
                                }
                                p.text(pos2(r.min.x + 40.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0 * (1.0 + 0.14 * wm)), tc);
                            } else {
                                p.text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0 * (1.0 + 0.14 * wm)), tc);
                            }
                            if resp.clicked() {
                                pick = Some(Pick::Item(c, i));
                            }
                        }
                    }
                });
        }
        match pick {
            Some(Pick::Item('d', i)) => self.hand_drag = i == 1,
            Some(Pick::Item('f', i)) => {
                // Free: drag one out on the canvas. A ratio drops that frame in the middle of the view
                self.frame_size = 0;
                self.tool = Tool::Frame;
                if i > 0 {
                    let r = self.last_rect;
                    let c = self.view(r).w(r.center());
                    self.snapshot();
                    let id = self.board.add_group(c);
                    let (_, fw, fh) = FRAME_SIZES[i];
                    let sq = self.frame_square;
                    if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                        g.x = c.x - fw / 2.0;
                        g.y = c.y - fh / 2.0;
                        g.width = fw;
                        g.height = fh;
                        g.square = sq;
                    }
                    self.select_only((Kind::Group, id));
                    self.mark_dirty();
                    self.tool = Tool::Select;
                    self.tool_menu = None;
                }
            }
            Some(Pick::Item('r', i)) => self.frame_square = i == 1,
            Some(Pick::Item('z', i)) => self.text_size = SIZES[i].1,
            Some(Pick::Item('b', i)) => self.text_style = i as u8,
            Some(Pick::Item('s', i)) => self.shape_fill = i as u8,
            Some(Pick::Item('p', i)) => self.blur_size = BSIZES[i].1,
            Some(Pick::Item(_, i)) => self.pen_width = WIDTHS[i].1,
            Some(Pick::Colour(c)) => self.pen_color = format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()),
            None => {}
        }
        // Click anywhere else or Esc closes it
        let clicked_out = ctx.input(|i| {
            i.pointer.any_pressed() && i.pointer.interact_pos().map_or(false, |q| !main.contains(q) && !sub_rect.contains(q) && !btn.contains(q))
        });
        // Auto-collapse: the pointer has clearly left (well clear of the menu and its button)
        // for a short moment. Leaving the window counts too. A brief overshoot doesn't close it.
        let zone = main.union(btn);
        let zone = if sub_rect.is_positive() { zone.union(sub_rect) } else { zone };
        let far = ctx.input(|i| i.pointer.hover_pos()).map_or(true, |q| {
            let d = vec2((zone.min.x - q.x).max(q.x - zone.max.x).max(0.0), (zone.min.y - q.y).max(q.y - zone.max.y).max(0.0));
            d.length() > 140.0
        });
        let aid = egui::Id::new("qd_tool_menu_away");
        let now = ctx.input(|i| i.time);
        let away_for = if far {
            now - ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(aid, || now))
        } else {
            ctx.data_mut(|d| d.remove::<f64>(aid));
            0.0
        };
        if far {
            ctx.request_repaint();
        }
        if clicked_out || away_for > 0.35 || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.data_mut(|d| d.remove::<f64>(aid));
            self.tool_menu = None;
        }
    }
}
