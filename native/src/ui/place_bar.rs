//! Placement bar: shows up once the selection sits inside a frame (the frame gives a clear
//! boundary). Align left / centre / right / top / middle / bottom, opacity, fill / fit for a
//! picture, and even spacing for 3+ objects. Sits beside the style bar when the object has
//! one (frames, shapes, text), else above the object. Same pill, entrance, magnify, hover
//! and tooltip as the style bar.
use crate::*;

#[derive(Clone, Copy, PartialEq)]
enum Seg {
    Al(Align),
    Dist(bool),
    Fill(bool),
    Opacity,
    Corner,
    Div,
}

fn align_code(a: Align) -> u8 {
    match a {
        Align::Left => 0,
        Align::HCenter => 1,
        Align::Right => 2,
        Align::Top => 3,
        Align::VCenter => 4,
        Align::Bottom => 5,
    }
}

impl App {
    /// Figma's keys: 1..9 = 10..90 % opacity, 0 = 100 %; Alt+A/D/W/S/H/V align; Shift+Enter parent
    fn place_keys(&mut self, ctx: &egui::Context) {
        if self.selected.is_empty() || self.edit.is_some() || self.prompt.is_some() || self.shortcuts_open || ctx.wants_keyboard_input() || ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        const DIGITS: [Key; 10] = [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
        let (digit, al) = ctx.input(|i| {
            let m = i.modifiers;
            let digit = if m.is_none() { DIGITS.iter().position(|k| i.key_pressed(*k)) } else { None };
            let al = if m.alt && !m.ctrl && !m.shift && !m.command {
                [(Key::A, Align::Left), (Key::D, Align::Right), (Key::W, Align::Top), (Key::S, Align::Bottom), (Key::H, Align::HCenter), (Key::V, Align::VCenter)]
                    .iter()
                    .find(|(k, _)| i.key_pressed(*k))
                    .map(|x| x.1)
            } else {
                None
            };
            (digit, al)
        });
        if let Some(d) = digit {
            let v = if d == 0 { 1.0 } else { d as f32 / 10.0 };
            self.snapshot();
            self.set_selection_opacity(v);
            self.toast(format!("Opacity {}%", (v * 100.0).round() as i32));
        }
        if let Some(a) = al {
            self.align_selection(a);
        }
        // Ctrl+Alt+Shift+R: resize the selected frames to fit their contents (Figma)
        if ctx.input(|i| i.modifiers.command && i.modifiers.alt && i.modifiers.shift && i.key_pressed(Key::R)) {
            self.fit_frames_to_content();
        }
        // Ctrl+Alt+G: wrap the selection in a new frame (Figma)
        if ctx.input(|i| i.modifiers.command && !i.modifiers.shift && ((i.modifiers.alt && i.key_pressed(Key::G)) || (!i.modifiers.alt && i.key_pressed(Key::F)))) {
            self.frame_selection();
        }
        // Shift+Enter: select the frame around the selection (Figma)
        let up = ctx.input(|i| i.modifiers.shift && !i.modifiers.command && !i.modifiers.alt && i.key_pressed(Key::Enter));
        if up {
            if let Some(f) = self.sel.clone().and_then(|s| self.parent_of(&s.1).map(str::to_string)) {
                self.select_only((Kind::Group, f));
            }
        }
    }

    pub(crate) fn place_bar_ui(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let ctx = ui.ctx().clone();
        let bid = egui::Id::new("qd_place_bar");
        self.place_keys(&ctx);

        let targets = self.place_targets();
        // One object in a frame, or several that share a frame. When the style bar is up it
        // carries align + opacity itself (one bar, not two stacked)
        let sbid0 = egui::Id::new("qd_style_bar");
        let style_up = !ctx.data(|d| d.get_temp::<String>(sbid0.with("on"))).unwrap_or_default().is_empty()
            && !ctx.data(|d| d.get_temp::<bool>(sbid0.with("closing"))).unwrap_or(false);
        let framed = self.share_frame(&targets);
        // A picture on its own (no frame) gets a short bar too: opacity + corners
        let free_img = !framed && matches!(targets.as_slice(), [(Kind::Image, _)]);
        let in_frame = (framed && !style_up) || free_img;
        let alt = ALT_DOWN.load(std::sync::atomic::Ordering::Relaxed) && matches!(self.drag, Drag::None);
        let cm_open = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_cm_open"))).unwrap_or(false);
        let item = if targets.len() == 1 { self.rect_of(targets[0].0, &targets[0].1) } else { self.place_bounds(&targets) };
        let show = in_frame && item.is_some() && self.edit.is_none() && self.sel_label.is_none() && !alt && !cm_open;

        let k = anim_k().max(1e-3);
        let p = ui.painter().clone();
        let was_shown = ctx.data(|d| d.get_temp::<bool>(bid.with("was_shown"))).unwrap_or(false);
        if was_shown && !show {
            let now = self.now;
            ctx.data_mut(|d| d.insert_temp(bid.with("t_gone"), now));
        }
        ctx.data_mut(|d| d.insert_temp(bid.with("was_shown"), show));
        // Hidden: replay the last bar closing, the same domino exit as the style bar
        let (mut targets, mut free_img, mut item) = (targets, free_img, item);
        let mut closing: Option<f32> = None;
        type Last = (Vec<Sel>, bool, [f32; 4]);
        if let (true, Some(r)) = (show, item) {
            ctx.data_mut(|d| d.insert_temp::<Last>(bid.with("last"), (targets.clone(), free_img, r)));
        } else {
            let t_gone: f64 = ctx.data(|d| d.get_temp(bid.with("t_gone"))).unwrap_or(self.now);
            let e = ((self.now - t_gone) as f32 / (0.3 * k)).clamp(0.0, 1.0);
            match ctx.data(|d| d.get_temp::<Last>(bid.with("last"))) {
                Some((t, fi, r)) if e < 1.0 && !t.is_empty() && self.edit.is_none() && !super::bar_morph::taken(&ctx, bid, self.now) => {
                    (targets, free_img, item) = (t, fi, Some(r));
                    closing = Some(e);
                    ctx.request_repaint();
                }
                _ => {
                    ctx.data_mut(|d| {
                        d.insert_temp(bid.with("on"), String::new());
                        d.remove::<Last>(bid.with("last"));
                        d.remove::<Rect>(bid);
                    });
                    return;
                }
            }
        }
        let Some([ix, iy, iw, ih]) = item else { return };

        let single_img = matches!(targets.as_slice(), [(Kind::Image, _)]);
        let mut segs = vec![
            Seg::Al(Align::Left),
            Seg::Al(Align::HCenter),
            Seg::Al(Align::Right),
            Seg::Div,
            Seg::Al(Align::Top),
            Seg::Al(Align::VCenter),
            Seg::Al(Align::Bottom),
            Seg::Div,
            Seg::Opacity,
        ];
        if free_img {
            segs = vec![Seg::Opacity, Seg::Corner];
        } else if single_img {
            segs.extend([Seg::Corner, Seg::Div, Seg::Fill(true), Seg::Fill(false)]);
        }
        if targets.len() >= 3 {
            segs.extend([Seg::Div, Seg::Dist(true), Seg::Dist(false)]);
        }
        let layout_locked = self.place_locked_by_layout(&targets);
        let opacity = (self.opacity_of(&targets[0].1) * 100.0).round();
        let corner = self.board.images.iter().find(|i| i.id == targets[0].1).map_or(0.0, |i| i.corner.round());

        let mut morphed = false;
        // Entrance replays when the selection changes
        let key = format!("{}|{}", targets.iter().map(|t| t.1.as_str()).collect::<Vec<_>>().join(","), segs.len());
        let was: String = ctx.data(|d| d.get_temp(bid.with("on"))).unwrap_or_default();
        if key != was {
            let glide = !was.is_empty();
            ctx.data_mut(|d| {
                d.insert_temp(bid.with("on"), key.clone());
                if !glide {
                    d.insert_temp(bid.with("t0"), self.now);
                    d.remove::<(f32, f32, f32)>(bid.with("glide"));
                }
            });
            if !glide {
                morphed = super::bar_morph::take(&ctx, bid, egui::Id::new("qd_style_bar"), self.now);
            }
            ctx.request_repaint();
        }
        let t0: f64 = ctx.data(|d| d.get_temp(bid.with("t0"))).unwrap_or(0.0);
        let x = ((self.now - t0) as f32 / (0.6 * k)).clamp(0.0, 1.0);

        let view = self.view(rect);
        let fr = Rect::from_min_max(view.s(ix, iy), view.s(ix + iw, iy + ih));
        let (pad, h) = (4.0_f32, 34.0_f32);
        let seg_w = |s: &Seg| match s {
            Seg::Opacity => 62.0,
            Seg::Corner => 50.0,
            Seg::Div => 9.0,
            _ => 28.0,
        };
        let was_scrub: bool = ctx.data(|d| d.get_temp(bid.with("scrub"))).unwrap_or(false);
        let was_cscrub: bool = ctx.data(|d| d.get_temp(bid.with("cscrub"))).unwrap_or(false);
        // Dock-style magnify, same as the style bar
        let mags: Vec<f32> = {
            let prev = ctx.data(|d| d.get_temp::<Rect>(bid)).unwrap_or(Rect::NOTHING);
            let ptr = ctx.pointer_hover_pos();
            let inside = was_scrub || was_cscrub || ptr.map_or(false, |q| prev.expand2(vec2(0.0, 8.0)).contains(q));
            let sa = spring(&ctx, bid.with("mag_a"), inside, 18.0);
            let base = pad * 2.0 + segs.iter().map(seg_w).sum::<f32>();
            let mut offs = Vec::new();
            let mut bx = -base / 2.0 + pad;
            for sg in segs.iter() {
                offs.push(bx + seg_w(sg) / 2.0);
                bx += seg_w(sg);
            }
            let fid = bid.with("mag_f");
            let mut focus = ctx.data(|d| d.get_temp::<f32>(fid)).unwrap_or(0.0);
            if let (true, Some(q), false) = (inside, ptr, was_scrub || was_cscrub) {
                let target = q.x - prev.center().x;
                if sa < 0.05 {
                    focus = target;
                }
                let dt = ctx.input(|i| i.stable_dt).min(0.05);
                focus += (target - focus) * (1.0 - (-dt * 11.0).exp());
                if (target - focus).abs() > 0.3 {
                    ctx.request_repaint();
                }
            }
            ctx.data_mut(|d| d.insert_temp(fid, focus));
            segs.iter()
                .enumerate()
                .map(|(i, sg)| {
                    if *sg == Seg::Div {
                        return 0.0;
                    }
                    let d = (offs[i] - focus) / (seg_w(sg) * 1.05);
                    sa * (-d * d).exp()
                })
                .collect()
        };
        let seg_ws = |i: usize, sg: &Seg| seg_w(sg) * (1.0 + 0.22 * mags[i]) + 10.0 * mags[i];
        let w = pad * 2.0 + segs.iter().enumerate().map(|(i, sg)| seg_ws(i, sg)).sum::<f32>();

        // Stacked on the style bar when it's up (one column of bars), else above the object
        let sbid = egui::Id::new("qd_style_bar");
        let sb_on: String = ctx.data(|d| d.get_temp(sbid.with("on"))).unwrap_or_default();
        let sb_closing: bool = ctx.data(|d| d.get_temp(sbid.with("closing"))).unwrap_or(false);
        let sb: Option<Rect> = if sb_on.is_empty() || sb_closing { None } else { ctx.data(|d| d.get_temp::<Rect>(sbid)) };
        let (cx, cy) = match sb {
            Some(sb) => {
                // Away from the object: above a style bar that sits above it, below one that sits below
                let above = sb.center().y <= fr.center().y && sb.min.y - 8.0 - h >= rect.min.y + 8.0;
                let cy = if above { sb.min.y - 8.0 - h / 2.0 } else { sb.max.y + 8.0 + h / 2.0 };
                let cx = sb.center().x.clamp(rect.min.x + w / 2.0 + 8.0, rect.max.x - w / 2.0 - 8.0);
                (cx, cy)
            }
            None => {
                let mut cy = fr.min.y - 32.0 - h / 2.0;
                let on_top = cy - h / 2.0 >= rect.min.y + 50.0;
                if !on_top {
                    cy = fr.max.y + 32.0 + h / 2.0;
                }
                let nr = ctx.cumulative_pass_nr();
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_bar_side"), (on_top, nr)));
                let cx = fr.center().x.clamp(rect.min.x + w / 2.0 + 8.0, rect.max.x - w / 2.0 - 8.0);
                (cx, cy.clamp(rect.min.y + h / 2.0 + 8.0, rect.max.y - h / 2.0 - 70.0))
            }
        };

        // Pill springs open sideways with the same jelly as the style bar
        let mut f = 1.0 - (-4.5 * x).exp() * (8.0 * x).cos();
        let mut wob = (-5.5 * x).exp() * (16.0 * x).sin() * 0.06;
        let mut a = (x / 0.15).min(1.0);
        if let Some(e) = closing {
            // Shrink back toward a small pill, fading at the very end (as the style bar)
            f = 1.0 - egui::emath::easing::cubic_in(e);
            wob = 0.0;
            a = ((1.0 - e) / 0.35).min(1.0);
        }
        if x < 1.0 {
            ctx.request_repaint();
        }
        let (gx, gy, gw) = {
            // A fresh entrance starts where it belongs (no slide in from where it was last time)
            let cur: Option<(f32, f32, f32)> = if was_shown || morphed { ctx.data(|d| d.get_temp(bid.with("glide"))) } else { None };
            let (mut gx, mut gy, mut gw) = cur.unwrap_or((cx, cy, w));
            let dt = ctx.input(|i| i.stable_dt).min(0.05);
            let kf = if anim_k() == 0.0 { 1.0 } else { 1.0 - (-dt * 16.0 / anim_k()).exp() };
            gx += (cx - gx) * kf;
            gy += (cy - gy) * kf;
            gw += (w - gw) * kf;
            if (cx - gx).abs() + (cy - gy).abs() + (w - gw).abs() < 0.3 {
                (gx, gy, gw) = (cx, cy, w);
            } else {
                ctx.request_repaint();
            }
            ctx.data_mut(|d| d.insert_temp(bid.with("glide"), (gx, gy, gw)));
            if closing.is_none() {
                super::bar_morph::publish(&ctx, bid, (gx, gy, gw));
            }
            (gx, gy, gw)
        };
        let bar = Rect::from_center_size(pos2(gx, gy), vec2(gw, h));
        ctx.data_mut(|d| d.insert_temp(bid, bar));
        let sw = (gw * (0.12 + 0.88 * f) * (1.0 + wob)).max(h);
        let sh = h * (0.8 + 0.2 * f.min(1.0)) * (1.0 - wob * 0.6);
        let shown = Rect::from_center_size(bar.center() + vec2(0.0, (1.0 - f) * 6.0), vec2(sw, sh));
        p.rect_filled(shown.translate(vec2(0.0, 3.0)), sh / 2.0, Color32::from_black_alpha((60.0 * a) as u8));
        p.rect_filled(shown, sh / 2.0, frame_bg().gamma_multiply(a));
        p.rect_stroke(shown, sh / 2.0, EStroke::new(1.0_f32, border().gamma_multiply(a)));
        ui.interact(bar, bid.with("bg"), Sense::click_and_drag());

        let mut tip: Option<(f32, &str)> = None;
        let mut clicked: Option<Seg> = None;
        let mut scrub: Option<f32> = None;
        let mut cscrub: Option<f32> = None;
        let mut reset_opacity = false;
        let mut xx = bar.center().x - w / 2.0 + pad;
        let mut gi_n = 0usize;
        for (i, s) in segs.iter().enumerate() {
            let wd = seg_ws(i, s);
            if *s == Seg::Div {
                let lx = xx + wd / 2.0;
                let fade = 1.0 - mags.get(i.wrapping_sub(1)).copied().unwrap_or(0.0).max(mags.get(i + 1).copied().unwrap_or(0.0)) * 0.8;
                p.line_segment([pos2(lx, bar.center().y - 7.0), pos2(lx, bar.center().y + 7.0)], EStroke::new(1.0_f32, border().gamma_multiply(0.7 * a * fade)));
                xx += wd;
                continue;
            }
            // Each control rises into place one after another as the pill widens
            let gi = match closing {
                // Domino: left to right, each icon slides down and out a beat after the last
                Some(e) => 1.0 - ((e - gi_n as f32 * 0.07) / 0.3).clamp(0.0, 1.0),
                None => ((x - 0.02 - gi_n as f32 * 0.06) / 0.3).clamp(0.0, 1.0),
            };
            gi_n += 1;
            let ca = 1.0 - (1.0 - gi).powi(3);
            let cyy = bar.center().y + (1.0 - ca) * 9.0;
            let mg = mags[i];
            let r = Rect::from_center_size(pos2(xx + wd / 2.0, cyy), vec2(seg_w(s), 26.0));
            xx += wd;
            let lid = egui::LayerId::new(egui::Order::Foreground, bid.with(("seg", i)));
            let sc = 1.0 + 0.22 * mg;
            let tv = r.center().to_vec2() * (1.0 - sc) + vec2(0.0, -4.0 * mg);
            ctx.set_transform_layer(lid, egui::emath::TSTransform::new(tv, sc));
            let pb = p.clone();
            let tf = move |q: Pos2| pos2(q.x * sc + tv.x, q.y * sc + tv.y);
            let tr = move |rr: Rect| Rect::from_min_max(tf(rr.min), tf(rr.max));
            let pl = p.clone().with_layer_id(lid).with_clip_rect(Rect::from_min_max(pos2(rect.min.x + crate::ui::outline::hud_shift(&ctx), -1e6), pos2(1e6, 1e6)));
            if ca <= 0.0 {
                continue;
            }
            let enabled = !(layout_locked && matches!(s, Seg::Al(_)));
            let sense = if matches!(s, Seg::Opacity | Seg::Corner) { Sense::click_and_drag() } else { Sense::click() };
            let resp = ui.interact(r, bid.with(("b", i)), sense);
            let (hh, pp) = hover_fx(&ctx, &resp, enabled);
            pb.rect_filled(tr(r), 8.0 * sc, hover_fill(hh, pp).gamma_multiply(ca));
            let col = mix(TEXT_DIM, TEXT, hh).gamma_multiply(ca * if enabled { 1.0 } else { 0.35 });
            match *s {
                Seg::Al(al) => {
                    super::icons::align(&pl, r.center(), 16.0, col, align_code(al));
                    if resp.hovered() {
                        tip = Some((
                            r.center().x,
                            match (al, enabled) {
                                (_, false) => "The frame arranges this picture",
                                (Align::Left, _) => "Align left  ·  Alt+A",
                                (Align::HCenter, _) => "Align centre  ·  Alt+H",
                                (Align::Right, _) => "Align right  ·  Alt+D",
                                (Align::Top, _) => "Align top  ·  Alt+W",
                                (Align::VCenter, _) => "Align middle  ·  Alt+V",
                                (Align::Bottom, _) => "Align bottom  ·  Alt+S",
                            },
                        ));
                    }
                }
                Seg::Dist(hz) => {
                    super::icons::distribute(&pl, r.center(), 16.0, col, hz);
                    if resp.hovered() {
                        tip = Some((r.center().x, if hz { "Even spacing across" } else { "Even spacing down" }));
                    }
                }
                Seg::Fill(cover) => {
                    super::icons::fill_fit(&pl, r.center(), 16.0, col, cover);
                    if resp.hovered() {
                        tip = Some((r.center().x, if cover { "Fill the frame" } else { "Fit in the frame" }));
                    }
                }
                Seg::Opacity => {
                    if resp.hovered() || resp.dragged() {
                        ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                        tip = Some((r.center().x, "Opacity  ·  drag, 0-9 keys"));
                    }
                    super::icons::opacity(&pl, pos2(r.min.x + 14.0, cyy), 14.0, col);
                    pb.text(tf(pos2(r.max.x - 9.0, cyy)), Align2::RIGHT_CENTER, format!("{}%", opacity as i32), FontId::proportional(12.0 * sc), mix(TEXT_DIM, TEXT, hh.max(0.6)).gamma_multiply(ca));
                    if resp.drag_started() {
                        ctx.data_mut(|d| d.insert_temp(bid.with("v0"), opacity));
                    }
                    if resp.dragged() {
                        let v0: f32 = ctx.data(|d| d.get_temp(bid.with("v0"))).unwrap_or(opacity);
                        let dx = ctx.input(|i| i.pointer.press_origin().zip(i.pointer.interact_pos()).map_or(0.0, |(a, b)| b.x - a.x));
                        scrub = Some((v0 + dx * 0.5).round().clamp(0.0, 100.0));
                    }
                    if resp.double_clicked() {
                        reset_opacity = true;
                    }
                }
                Seg::Corner => {
                    if resp.hovered() || resp.dragged() {
                        ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                        tip = Some((r.center().x, "Corner radius  ·  drag"));
                    }
                    // Same corner glyph as the style bar's
                    let ic = pos2(r.min.x + 14.0, cyy);
                    let pts = vec![ic + vec2(-5.0, 6.0), ic + vec2(-5.0, -1.0), ic + vec2(-4.0, -4.0), ic + vec2(-1.0, -5.0), ic + vec2(6.0, -5.0)];
                    pl.add(egui::Shape::line(pts, EStroke::new(1.3_f32, col)));
                    pb.text(tf(pos2(r.max.x - 9.0, cyy)), Align2::RIGHT_CENTER, format!("{}", corner as i32), FontId::proportional(12.0 * sc), mix(TEXT_DIM, TEXT, hh.max(0.6)).gamma_multiply(ca));
                    if resp.drag_started() {
                        ctx.data_mut(|d| d.insert_temp(bid.with("c0"), corner));
                    }
                    if resp.dragged() {
                        let v0: f32 = ctx.data(|d| d.get_temp(bid.with("c0"))).unwrap_or(corner);
                        let dx = ctx.input(|i| i.pointer.press_origin().zip(i.pointer.interact_pos()).map_or(0.0, |(a, b)| b.x - a.x));
                        cscrub = Some((v0 + dx * 0.5).round().clamp(0.0, 400.0));
                    }
                }
                Seg::Div => {}
            }
            if enabled && resp.clicked() && !matches!(s, Seg::Opacity | Seg::Corner) {
                clicked = Some(*s);
            }
        }

        // Opacity scrub: one undo step per drag
        if scrub.is_some() && !was_scrub {
            self.snapshot();
        }
        ctx.data_mut(|d| d.insert_temp(bid.with("scrub"), scrub.is_some()));
        if let Some(v) = scrub {
            self.set_selection_opacity(v / 100.0);
        }
        // Corner scrub: one undo step per drag
        if cscrub.is_some() && !was_cscrub {
            self.snapshot();
        }
        ctx.data_mut(|d| d.insert_temp(bid.with("cscrub"), cscrub.is_some()));
        if let Some(v) = cscrub {
            let id = targets[0].1.clone();
            if let Some(i) = self.board.images.iter_mut().find(|i| i.id == id) {
                i.corner = v;
            }
            self.mark_dirty();
        }
        if reset_opacity && opacity < 100.0 {
            self.snapshot();
            self.set_selection_opacity(1.0);
        }
        match clicked {
            Some(Seg::Al(al)) => self.align_selection(al),
            Some(Seg::Dist(hz)) => self.distribute_selection(hz),
            Some(Seg::Fill(cover)) => self.fill_frame(cover),
            _ => {}
        }
        ui_block(&ctx, bar);

        // Tooltip above the pill, same as the style bar's
        let tk = ease(&ctx, bid.with("tip"), tip.is_some());
        if let Some(t) = tip {
            ctx.data_mut(|d| d.insert_temp(bid.with("tipv"), (t.0, t.1.to_string())));
        }
        if tk > 0.0 {
            if let Some((tx, text)) = ctx.data(|d| d.get_temp::<(f32, String)>(bid.with("tipv"))) {
                let gl = p.layout_no_wrap(text, FontId::proportional(11.5), TEXT.gamma_multiply(tk));
                let tr = Rect::from_center_size(pos2(tx, bar.min.y - 18.0 + (1.0 - tk) * 4.0), gl.size() + vec2(16.0, 8.0));
                glass(&p, tr, 6.0, tk);
                p.rect_stroke(tr, 6.0, EStroke::new(1.0_f32, border().gamma_multiply(tk)));
                p.galley(tr.min + vec2(8.0, 4.0), gl, Color32::WHITE);
            }
        }
    }
}
