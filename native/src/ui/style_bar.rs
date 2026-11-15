//! Floating frame/shape style bar (scrub values, dropdowns, magnify).

use crate::*;

impl App {
    /// Floating style bar above the selected frame, shape, drawing or text.
    /// Numbers are scrubbed by dragging sideways; each control group staggers in as the pill grows.
    pub(crate) fn style_bar_ui(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>, rect: Rect) {
        // Out of the way while Alt-measuring
        if ALT_DOWN.load(std::sync::atomic::Ordering::Relaxed) && matches!(self.drag, Drag::None) {
            return;
        }
        #[derive(Clone, Copy, PartialEq)]
        enum Seg {
            Color,
            /// key, icon, value, min, max, units per pixel, tooltip
            Num(&'static str, u8, f32, f32, f32, f32, &'static str),
            Arrange,
            Aspect,
            /// Align / even spacing dropdown (only for something inside a frame)
            Align,
            /// Stroke inside / centre / outside (frames)
            StrokePos,
        }
        let ctx = ui.ctx().clone();
        let bid = egui::Id::new("qd_style_bar");
        let mut target = match (&self.sel, self.selected.len()) {
            (Some((k @ (Kind::Group | Kind::Stroke | Kind::Text), id)), 1) if self.sel_label.is_none() => Some((*k, id.clone())),
            // Several frames: one bar for all of them (values shown from the first)
            (_, n) if n > 1 && self.selected.iter().all(|s| s.0 == Kind::Group) => {
                let mut ids: Vec<&String> = self.selected.iter().map(|s| &s.1).collect();
                ids.sort();
                Some((Kind::Group, ids[0].clone()))
            }
            _ => None,
        };
        let multi = self.selected.len() > 1;
        // Closing: keep drawing the last item's bar for a moment so it can animate out
        // (icons drop away one by one while the pill shrinks). `closing` is 0..1 progress.
        let mut closing: Option<f32> = None;
        if target.is_some() {
            ctx.data_mut(|d| {
                d.insert_temp(bid.with("last_t"), target.clone());
                d.remove::<f64>(bid.with("t_out"));
            });
        } else if let Some(Some(lt)) = ctx.data(|d| d.get_temp::<Option<Sel>>(bid.with("last_t"))) {
            let t_out = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(bid.with("t_out"), || self.now));
            let e = ((self.now - t_out) as f32 / (0.3 * anim_k().max(1e-3))).clamp(0.0, 1.0);
            // The object may be gone (deleted): still animate out where it was
            if e < 1.0 && self.edit.is_none() && !super::bar_morph::taken(&ctx, bid, self.now) && (self.rect_of(lt.0, &lt.1).is_some() || ctx.data(|d| d.get_temp::<[f32; 4]>(bid.with("last_r"))).is_some()) {
                target = Some(lt);
                closing = Some(e);
                ctx.request_repaint();
            } else {
                ctx.data_mut(|d| {
                    d.insert_temp::<Option<Sel>>(bid.with("last_t"), None);
                    d.remove::<f64>(bid.with("t_out"));
                    d.remove::<Rect>(bid);
                });
            }
        }
        // Other bars stack on this one only while it's really up (not while it animates out)
        ctx.data_mut(|d| d.insert_temp(bid.with("closing"), closing.is_some()));
        let was_scrub: bool = ctx.data(|d| d.get_temp(bid.with("scrub"))).unwrap_or(false);
        let _ = was_scrub;
        let idle = self.edit.is_none();
        let mut segs: Vec<Seg> = Vec::new();
        let mut color = String::new();
        let mut layout = 0u8;
        let mut stroke_pos = 0u8;
        if let Some((k, id)) = &target {
            match k {
                Kind::Group => {
                    if let Some(g) = self.board.group_cards.iter().find(|g| &g.id == id) {
                        color = g.color.clone();
                        layout = g.layout;
                        stroke_pos = g.stroke_pos;
                        segs = vec![
                            Seg::Color,
                            Seg::Num("corner", 0, group_radius(g), 0.0, 60.0, 0.25, "Corner radius  ·  drag"),
                            Seg::Num("border", 1, g.border.unwrap_or(1.0), 0.0, 12.0, 0.06, "Stroke  ·  drag"),
                            Seg::StrokePos,
                            Seg::Aspect,
                        ];
                        // Auto-arrange would pull a nested frame apart: only for frames of pictures
                        if !self.descendants(id).iter().any(|s| s.0 == Kind::Group) {
                            segs.push(Seg::Arrange);
                        }
                    }
                }
                Kind::Stroke => {
                    if let Some(st) = self.find_stroke(id) {
                        color = st.color.clone();
                        segs.push(Seg::Color);
                        if st.closed_len() == Some(4) {
                            segs.push(Seg::Num("corner", 0, st.radius.unwrap_or(0.0), 0.0, 60.0, 0.25, "Corner radius  ·  drag"));
                        }
                        segs.push(Seg::Num("width", 1, st.width, 1.0, 30.0, 0.08, "Thickness  ·  drag"));
                    }
                }
                Kind::Text => {
                    if let Some(t) = self.board.world_texts.iter().find(|t| &t.id == id) {
                        color = t.color.clone();
                        let fs = if t.font_size > 0.0 { t.font_size } else { 16.0 };
                        segs = vec![Seg::Color, Seg::Num("tsize", 2, fs, 8.0, 96.0, 0.2, "Text size  ·  drag")];
                    }
                }
                _ => {}
            }
        }
        // Inside a frame: align (dropdown) and opacity join this bar, so there's one bar, not two
        let place_targets = self.place_targets();
        let opacity_now = place_targets.first().map_or(100.0, |t| (self.opacity_of(&t.1) * 100.0).round());
        if !segs.is_empty() {
            if self.share_frame(&place_targets) {
                segs.push(Seg::Align);
            }
            // Always shown, so the 0-9 opacity keys have something visible to answer to
            segs.push(Seg::Num("opacity", 3, opacity_now, 0.0, 100.0, 0.5, "Opacity  ·  drag, double-click resets"));
        }
        // Remember the controls too, so a deleted object's bar can domino out like a normal close
        if closing.is_none() && !segs.is_empty() {
            let c = (segs.clone(), color.clone(), layout);
            ctx.data_mut(|d| d.insert_temp(bid.with("last_segs"), c));
        } else if closing.is_some() && segs.is_empty() {
            if let Some((sg, c, l)) = ctx.data(|d| d.get_temp::<(Vec<Seg>, String, u8)>(bid.with("last_segs"))) {
                (segs, color, layout) = (sg, c, l);
            }
        }
        let mut item = if multi && target.is_some() { self.multi_box() } else { target.as_ref().and_then(|(k, id)| self.rect_of(*k, id)) };
        if closing.is_none() {
            if let Some(r) = item {
                ctx.data_mut(|d| d.insert_temp(bid.with("last_r"), r));
            }
        } else if item.is_none() {
            item = ctx.data(|d| d.get_temp::<[f32; 4]>(bid.with("last_r")));
        }
        let show = !segs.is_empty() && item.is_some() && idle;
        let key = target.as_ref().map(|t| t.1.clone()).unwrap_or_default();
        let last: Option<Rect> = ctx.data(|d| d.get_temp(bid));
        let was: String = ctx.data(|d| d.get_temp(bid.with("on"))).unwrap_or_default();
        let now_key = if show { key } else { String::new() };
        if now_key != was {
            // switching straight to another item replays the entrance there
            let glide = !was.is_empty() && !now_key.is_empty();
            ctx.data_mut(|d| {
                d.insert_temp(bid.with("on"), now_key.clone());
                if !glide {
                    d.insert_temp(bid.with("t0"), self.now);
                    d.remove::<(f32, f32, f32)>(bid.with("glide"));
                }
                d.insert_temp(bid.with("pop"), 0u8);
            });
            if !glide && !now_key.is_empty() {
                super::bar_morph::take(&ctx, bid, egui::Id::new("qd_place_bar"), self.now);
            }
            ctx.request_repaint();
        }
        let t0: f64 = ctx.data(|d| d.get_temp(bid.with("t0"))).unwrap_or(0.0);
        let k = anim_k().max(1e-3);
        let x = ((self.now - t0) as f32 / (0.6 * k)).clamp(0.0, 1.0);
        let p = ui.painter().clone();
        // When the bar goes away (e.g. its object was deleted) the exit is timed from that moment,
        // not from when the bar opened (which made it snap)
        let shown_now = item.is_some() && show;
        let was_shown = ctx.data(|d| d.get_temp::<bool>(bid.with("was_shown"))).unwrap_or(false);
        if was_shown && !shown_now {
            let now = self.now;
            ctx.data_mut(|d| d.insert_temp(bid.with("t_gone"), now));
        }
        ctx.data_mut(|d| d.insert_temp(bid.with("was_shown"), shown_now));
        let (Some([ix, iy, iw, ih]), true) = (item, show) else {
            if let Some(r) = last {
                let t_gone: f64 = ctx.data(|d| d.get_temp(bid.with("t_gone"))).unwrap_or(t0);
                let e = ((self.now - t_gone) as f32 / (0.25 * k)).clamp(0.0, 1.0);
                if e < 1.0 {
                    let a = (1.0 - e).powf(1.5);
                    let rr = Rect::from_center_size(r.center() + vec2(0.0, e * 8.0), r.size() * (1.0 - 0.25 * e));
                    p.rect_filled(rr, rr.height() / 2.0, popup().gamma_multiply(a));
                    p.rect_stroke(rr, rr.height() / 2.0, EStroke::new(1.0_f32, border().gamma_multiply(a)));
                    ctx.request_repaint();
                } else {
                    ctx.data_mut(|d| d.remove::<Rect>(bid));
                }
            }
            return;
        };
        let view = self.view(rect);
        let fr = Rect::from_min_max(view.s(ix, iy), view.s(ix + iw, iy + ih));

        // Layout: 4px padding, a divider between every control
        let (pad, dv, h) = (4.0_f32, 9.0_f32, 34.0_f32);
        let seg_w = |s: &Seg| match s {
            Seg::Color => 30.0,
            Seg::Num("opacity", ..) => 62.0,
            Seg::Num(..) => 50.0,
            Seg::Arrange => 40.0,
            Seg::Aspect => 40.0,
            Seg::Align => 40.0,
            Seg::StrokePos => 40.0,
        };
        // Dock-style magnify, like the bottom toolbar: a smoothed focus follows the pointer
        // and each segment swells (and the bar widens) by its distance from it
        let mags: Vec<f32> = {
            let prev = ctx.data(|d| d.get_temp::<Rect>(bid)).unwrap_or(Rect::NOTHING);
            let ptr = ctx.pointer_hover_pos();
            // While scrubbing a value the magnify freezes on that segment
            let inside = was_scrub || ptr.map_or(false, |q| prev.expand2(vec2(0.0, 8.0)).contains(q));
            let sa = spring(&ctx, bid.with("mag_a"), inside, 18.0);
            let base = pad * 2.0 + segs.iter().map(seg_w).sum::<f32>() + dv * (segs.len() as f32 - 1.0);
            let mut offs = Vec::new();
            let mut bx = -base / 2.0 + pad;
            for sg in segs.iter() {
                offs.push(bx + seg_w(sg) / 2.0);
                bx += seg_w(sg) + dv;
            }
            let fid = bid.with("mag_f");
            let mut focus = ctx.data(|d| d.get_temp::<f32>(fid)).unwrap_or(0.0);
            if let (true, Some(q), false) = (inside, ptr, was_scrub) {
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
            // The button whose dropdown is open stays swollen, like the toolbar's active tool
            let open: u8 = ctx.data(|d| d.get_temp(bid.with("pop"))).unwrap_or(0);
            segs.iter().enumerate().map(|(i, sg)| {
                let d = (offs[i] - focus) / (seg_w(sg) * 1.05);
                let k = match sg { Seg::Color => 1, Seg::Arrange => 2, Seg::Aspect => 3, Seg::Align => 4, Seg::StrokePos => 5, _ => 0 };
                let act = spring(&ctx, bid.with(("mag_on", i)), k != 0 && open == k, 18.0);
                (sa * (-d * d).exp()).max(act)
            }).collect()
        };
        // Slot grows a little more than the drawing so a swollen button keeps clear of the dividers
let seg_ws = |i: usize, sg: &Seg| seg_w(sg) * (1.0 + 0.22 * mags[i]) + 10.0 * mags[i];
        let w = pad * 2.0 + segs.iter().enumerate().map(|(i, sg)| seg_ws(i, sg)).sum::<f32>() + dv * (segs.len() as f32 - 1.0);
        // The bar keeps its side (above if there's room, else below); the size pills never push
        // it: a pill that would land on the bar's side folds into the other pill instead
        let mut cy = fr.min.y - 32.0 - h / 2.0;
        let on_top = cy - h / 2.0 >= rect.min.y + 50.0;
        if !on_top {
            cy = fr.max.y + 32.0 + h / 2.0;
        }
        let nr = ctx.cumulative_pass_nr();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_bar_side"), (on_top, nr)));
        let cx = fr.center().x.clamp(rect.min.x + w / 2.0 + 8.0, rect.max.x - w / 2.0 - 8.0);
        let cy = cy.clamp(rect.min.y + h / 2.0 + 8.0, rect.max.y - h / 2.0 - 70.0);
        // Scrolled far from its object (the bar would sit beside it, not over it): put it away
        let inview = cx >= fr.min.x && cx <= fr.max.x && fr.max.y > rect.min.y && fr.min.y < rect.max.y;
        // A right-click menu puts the bar away too (same exit), since both share the top layer
        // and the bar would draw over the menu
        let cm_open = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("qd_cm_open"))).unwrap_or(false);
        let vis = ctx.animate_bool_with_time(bid.with("inview"), inview && !cm_open, 0.25 * anim_k());
        if vis <= 0.0 {
            return;
        }
        if closing.is_none() && vis < 1.0 {
            closing = Some(1.0 - vis);
        }

        // Pill springs open sideways; a jelly wobble settles it
        // (slower than before so the pill visibly grows outward as its controls appear)
        let mut f = 1.0 - (-4.5 * x).exp() * (8.0 * x).cos();
        let mut wob = (-5.5 * x).exp() * (16.0 * x).sin() * 0.06;
        let mut a = (x / 0.15).min(1.0);
        if let Some(e) = closing {
            // Shrink back toward a small pill, fading at the very end
            f = 1.0 - egui::emath::easing::cubic_in(e);
            wob = 0.0;
            a = ((1.0 - e) / 0.35).min(1.0);
        }
        if x < 1.0 {
            ctx.request_repaint();
        }
        // Glide smoothly to the new spot/width when the selection moves to another item
        let (gx, gy, gw) = {
            let cur: Option<(f32, f32, f32)> = ctx.data(|d| d.get_temp(bid.with("glide")));
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
        let (w, cx) = (gw, gx);
        let content_w = pad * 2.0 + segs.iter().enumerate().map(|(i, sg)| seg_ws(i, sg)).sum::<f32>() + dv * (segs.len() as f32 - 1.0);
        let _ = cx;
        ctx.data_mut(|d| d.insert_temp(bid, bar));
        let sw = (w * (0.12 + 0.88 * f) * (1.0 + wob)).max(h);
        let sh = h * (0.8 + 0.2 * f.min(1.0)) * (1.0 - wob * 0.6);
        let shown = Rect::from_center_size(bar.center() + vec2(0.0, (1.0 - f) * 6.0), vec2(sw, sh));
        p.rect_filled(shown.translate(vec2(0.0, 3.0)), sh / 2.0, Color32::from_black_alpha((60.0 * a) as u8));
        p.rect_filled(shown, sh / 2.0, frame_bg().gamma_multiply(a));
        p.rect_stroke(shown, sh / 2.0, EStroke::new(1.0_f32, border().gamma_multiply(a)));
        ui.interact(bar, bid.with("bg"), Sense::click_and_drag());

        let pop: u8 = ctx.data(|d| d.get_temp(bid.with("pop"))).unwrap_or(0);
        let mut set_pop = pop;
        let started = ui.input(|i| i.pointer.any_pressed());
        let font = FontId::proportional(12.0);
        let mut tip: Option<(f32, &str)> = None;
        let mut scrubbed: Vec<(&'static str, f32)> = Vec::new();
        let mut reset_opacity = false;
        let mut xx = bar.center().x - content_w / 2.0 + pad;
        let n = segs.len();
        for (i, s) in segs.iter().enumerate() {
            // Each group rises into place one after another as the pill widens
            let gi = match closing {
                // Domino: left to right, each icon slides down and out a beat after the last
                Some(e) => 1.0 - ((e - i as f32 * 0.07) / 0.3).clamp(0.0, 1.0),
                None => ((x - 0.02 - i as f32 * 0.06) / 0.3).clamp(0.0, 1.0),
            };
            let ca = 1.0 - (1.0 - gi).powi(3);
            let cyy = bar.center().y + (1.0 - ca) * 9.0;
            let wd = seg_ws(i, s);
            let mg = mags[i];
            let r = Rect::from_center_size(pos2(xx + wd / 2.0, cyy), vec2(seg_w(s), 26.0));
            // Swell + lift the segment's drawing on its own layer
            let lid = egui::LayerId::new(egui::Order::Foreground, bid.with(("seg", i)));
            let sc = 1.0 + 0.22 * mg;
            ctx.set_transform_layer(lid, egui::emath::TSTransform::new(r.center().to_vec2() * (1.0 - sc) + vec2(0.0, -4.0 * mg), sc));
            // Text and hover fill go on the base layer at real size so glyphs stay crisp;
            // only the vector icons ride the scaled layer
            let pb = p.clone();
            let tv = r.center().to_vec2() * (1.0 - sc) + vec2(0.0, -4.0 * mg);
            let tf = move |q: Pos2| pos2(q.x * sc + tv.x, q.y * sc + tv.y);
            let tr = move |rr: Rect| Rect::from_min_max(tf(rr.min), tf(rr.max));
            let p = p.clone().with_layer_id(lid).with_clip_rect(Rect::from_min_max(pos2(rect.min.x + crate::ui::outline::hud_shift(&ctx), -1e6), pos2(1e6, 1e6)));
            xx += wd;
            if i + 1 < n {
                let lx = xx + dv / 2.0;
                // Subtle divider that fades out beside a swollen neighbour
                let fade = 1.0 - mags[i].max(mags[i + 1]) * 0.8;
                p.line_segment([pos2(lx, bar.center().y - 7.0), pos2(lx, bar.center().y + 7.0)], EStroke::new(1.0_f32, border().gamma_multiply(0.7 * ca * fade)));
                xx += dv;
            }
            if ca <= 0.0 {
                continue;
            }
            match *s {
                Seg::Color => {
                    let resp = ui.interact(r, bid.with("col"), Sense::click());
                    let (hh, pp) = hover_fx(&ctx, &resp, true);
                    let on = pop == 1;
                    pb.rect_filled(tr(r), 8.0 * sc, hover_fill(hh, pp).gamma_multiply(ca));
                    // Open: a soft neutral highlight, so the swatch colour is the only colour here
                    let lq = ctx.animate_bool_with_time(bid.with(("liquid", 1)), on, 0.5 * anim_k());
                    pb.rect_filled(tr(r), 8.0 * sc, Color32::from_white_alpha((22.0 * lq * ca) as u8));
                    let _ = hh;
                    let col = if color.is_empty() || color.eq_ignore_ascii_case("none") { TEXT_DIM } else { hex(&color) };
                    if color.eq_ignore_ascii_case("none") {
                        // No fill: an empty swatch with a red slash
                        p.line_segment([r.center() + vec2(-4.9, 4.9), r.center() + vec2(4.9, -4.9)], EStroke::new(1.5_f32, hex("#ef4444").gamma_multiply(ca)));
                    } else {
                        p.circle_filled(r.center(), 7.0, col.gamma_multiply(ca));
                    }
                    p.circle_stroke(r.center(), 7.0, EStroke::new(1.0_f32, Color32::from_white_alpha((60.0 * ca) as u8)));
                    if resp.hovered() {
                        tip = Some((r.center().x, "Colour"));
                    }
                    if resp.clicked() {
                        set_pop = if pop == 1 { 0 } else { 1 };
                    }
                }
                Seg::Num(key, icon, val, lo, hi, per_px, name) => {
                    let id = bid.with(key);
                    let resp = ui.interact(r, id, Sense::click_and_drag());
                    let (hh, pp) = hover_fx(&ctx, &resp, true);
                    if resp.hovered() || resp.dragged() {
                        ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                        tip = Some((r.center().x, name));
                    }
                    pb.rect_filled(tr(r), 8.0 * sc, hover_fill(hh, pp).gamma_multiply(ca));
                    let ic = pos2(r.min.x + 14.0, cyy);
                    let st = EStroke::new(1.3_f32, mix(TEXT_DIM, TEXT, hh).gamma_multiply(ca));
                    match icon {
                        0 => {
                            let pts = vec![ic + vec2(-5.0, 6.0), ic + vec2(-5.0, -1.0), ic + vec2(-4.0, -4.0), ic + vec2(-1.0, -5.0), ic + vec2(6.0, -5.0)];
                            p.add(egui::Shape::line(pts, st));
                        }
                        1 => {
                            p.rect_stroke(Rect::from_center_size(ic, vec2(11.0, 11.0)), 2.0, st);
                            p.rect_stroke(Rect::from_center_size(ic, vec2(5.0, 5.0)), 1.0, EStroke::new(1.0_f32, st.color.gamma_multiply(0.5)));
                        }
                        3 => super::icons::opacity(&p, ic, 14.0, st.color),
                        _ => {
                            pb.text(tf(ic), Align2::CENTER_CENTER, "Aa", FontId::proportional(12.0 * sc), st.color);
                        }
                    }
                    let txt = if icon == 3 { format!("{}%", val.round() as i32) } else { format!("{}", val.round() as i32) };
                    pb.text(tf(pos2(r.max.x - 9.0, cyy)), Align2::RIGHT_CENTER, txt, FontId::proportional(12.0 * sc), mix(TEXT_DIM, TEXT, hh.max(0.6)).gamma_multiply(ca));
                    if icon == 3 && resp.double_clicked() {
                        reset_opacity = true;
                    }
                    if resp.drag_started() {
                        ctx.data_mut(|d| d.insert_temp(id.with("v0"), val));
                    }
                    if resp.dragged() {
                        let v0: f32 = ctx.data(|d| d.get_temp(id.with("v0"))).unwrap_or(val);
                        let dx = ctx.input(|i| i.pointer.press_origin().zip(i.pointer.interact_pos()).map_or(0.0, |(a, b)| b.x - a.x));
                        scrubbed.push((key, (v0 + dx * per_px).round().clamp(lo, hi)));
                    }
                }
                Seg::Aspect => {
                    let resp = ui.interact(r, bid.with("asp"), Sense::click());
                    let (hh, pp) = hover_fx(&ctx, &resp, true);
                    let on = pop == 3;
                    pb.rect_filled(tr(r), 8.0 * sc, hover_fill(hh, pp).gamma_multiply(ca));
                    // Open: accent liquid fill, like the toolbar's active tool
                    let lq = ctx.animate_bool_with_time(bid.with(("liquid", 3)), on, 0.5 * anim_k());
                    let lq = lq * lq * (3.0 - 2.0 * lq);
                    if lq >= 0.999 {
                        pb.rect_filled(tr(r), 8.0 * sc, blue().gamma_multiply(ca));
                    } else if lq > 0.001 {
                        liquid_fill(&pb, tr(r), 8.0 * sc, lq, ctx.input(|i| i.time) as f32 + 3.0);
                    }
                    let hh = hh.max(lq);
                    let gc = mix(TEXT_DIM, Color32::WHITE, hh).gamma_multiply(ca);
                    let ic = Rect::from_center_size(pos2(r.min.x + 14.0, cyy), vec2(12.0, 9.0));
                    p.rect_stroke(ic, 1.5, EStroke::new(1.2_f32, gc));
                    p.line_segment([ic.left_bottom() + vec2(2.5, -2.5), ic.right_top() + vec2(-2.5, 2.5)], EStroke::new(1.0_f32, gc));
                    let cv = pos2(r.max.x - 10.0, cyy);
                    p.add(egui::Shape::line(vec![cv + vec2(-3.0, -1.5), cv + vec2(0.0, 1.5), cv + vec2(3.0, -1.5)], EStroke::new(1.2_f32, gc)));
                    if resp.hovered() {
                        tip = Some((r.center().x, "Aspect ratio"));
                    }
                    if resp.clicked() {
                        set_pop = if pop == 3 { 0 } else { 3 };
                    }
                }
                Seg::StrokePos => {
                    let resp = ui.interact(r, bid.with("spos"), Sense::click());
                    let (hh, pp) = hover_fx(&ctx, &resp, true);
                    let on = pop == 5;
                    pb.rect_filled(tr(r), 8.0 * sc, hover_fill(hh, pp).gamma_multiply(ca));
                    let lq = ctx.animate_bool_with_time(bid.with(("liquid", 5)), on, 0.5 * anim_k());
                    let lq = lq * lq * (3.0 - 2.0 * lq);
                    if lq >= 0.999 {
                        pb.rect_filled(tr(r), 8.0 * sc, blue().gamma_multiply(ca));
                    } else if lq > 0.001 {
                        liquid_fill(&pb, tr(r), 8.0 * sc, lq, ctx.input(|i| i.time) as f32 + 5.0);
                    }
                    let hh = hh.max(lq);
                    let gc = mix(TEXT_DIM, Color32::WHITE, hh).gamma_multiply(ca);
                    stroke_pos_icon(&p, pos2(r.min.x + 14.0, cyy), 1.0, gc, stroke_pos);
                    let cv = pos2(r.max.x - 10.0, cyy);
                    p.add(egui::Shape::line(vec![cv + vec2(-3.0, -1.5), cv + vec2(0.0, 1.5), cv + vec2(3.0, -1.5)], EStroke::new(1.2_f32, gc)));
                    if resp.hovered() {
                        tip = Some((r.center().x, "Stroke position"));
                    }
                    if resp.clicked() {
                        set_pop = if pop == 5 { 0 } else { 5 };
                    }
                }
                Seg::Align => {
                    let resp = ui.interact(r, bid.with("aln"), Sense::click());
                    let (hh, pp) = hover_fx(&ctx, &resp, true);
                    let on = pop == 4;
                    pb.rect_filled(tr(r), 8.0 * sc, hover_fill(hh, pp).gamma_multiply(ca));
                    // Open: accent liquid fill, like the other dropdown buttons
                    let lq = ctx.animate_bool_with_time(bid.with(("liquid", 4)), on, 0.5 * anim_k());
                    let lq = lq * lq * (3.0 - 2.0 * lq);
                    if lq >= 0.999 {
                        pb.rect_filled(tr(r), 8.0 * sc, blue().gamma_multiply(ca));
                    } else if lq > 0.001 {
                        liquid_fill(&pb, tr(r), 8.0 * sc, lq, ctx.input(|i| i.time) as f32 + 4.0);
                    }
                    let hh = hh.max(lq);
                    let gc = mix(TEXT_DIM, Color32::WHITE, hh).gamma_multiply(ca);
                    super::icons::align(&p, pos2(r.min.x + 14.0, cyy), 15.0, gc, 0);
                    let cv = pos2(r.max.x - 10.0, cyy);
                    p.add(egui::Shape::line(vec![cv + vec2(-3.0, -1.5), cv + vec2(0.0, 1.5), cv + vec2(3.0, -1.5)], EStroke::new(1.2_f32, gc)));
                    if resp.hovered() {
                        tip = Some((r.center().x, "Align"));
                    }
                    if resp.clicked() {
                        set_pop = if pop == 4 { 0 } else { 4 };
                    }
                }
                Seg::Arrange => {
                    let resp = ui.interact(r, bid.with("arr"), Sense::click());
                    let (hh, pp) = hover_fx(&ctx, &resp, true);
                    let on = pop == 2;
                    pb.rect_filled(tr(r), 8.0 * sc, hover_fill(hh, pp).gamma_multiply(ca));
                    // Open: accent liquid fill, like the toolbar's active tool
                    let lq = ctx.animate_bool_with_time(bid.with(("liquid", 2)), on, 0.5 * anim_k());
                    let lq = lq * lq * (3.0 - 2.0 * lq);
                    if lq >= 0.999 {
                        pb.rect_filled(tr(r), 8.0 * sc, blue().gamma_multiply(ca));
                    } else if lq > 0.001 {
                        liquid_fill(&pb, tr(r), 8.0 * sc, lq, ctx.input(|i| i.time) as f32 + 2.0);
                    }
                    let hh = hh.max(lq);
                    let ic = pos2(r.min.x + 14.0, cyy);
                    let gc = mix(TEXT_DIM, Color32::WHITE, hh).gamma_multiply(ca);
                    for (dx, dy) in [(-3.5, -3.5), (3.5, -3.5), (-3.5, 3.5), (3.5, 3.5)] {
                        p.rect_filled(Rect::from_center_size(ic + vec2(dx, dy), vec2(5.0, 5.0)), 1.0, gc);
                    }
                    let cv = pos2(r.max.x - 10.0, cyy);
                    p.add(egui::Shape::line(vec![cv + vec2(-3.0, -1.5), cv + vec2(0.0, 1.5), cv + vec2(3.0, -1.5)], EStroke::new(1.2_f32, gc)));
                    if resp.hovered() {
                        tip = Some((r.center().x, "Arrange images"));
                    }
                    if resp.clicked() {
                        set_pop = if pop == 2 { 0 } else { 2 };
                    }
                }
            }
        }

        // Live scrub: one undo step per drag
        let scrubbing = !scrubbed.is_empty();
        if scrubbing && !was_scrub {
            self.snapshot();
        }
        ctx.data_mut(|d| d.insert_temp(bid.with("scrub"), scrubbing));
        if let Some(v) = scrubbed.iter().find(|s| s.0 == "opacity").map(|s| s.1) {
            self.set_selection_opacity(v / 100.0);
        }
        scrubbed.retain(|s| s.0 != "opacity");
        if reset_opacity && opacity_now < 100.0 {
            self.snapshot();
            self.set_selection_opacity(1.0);
        }
        if let (true, Some((kind, id))) = (scrubbing && !scrubbed.is_empty(), &target) {
            for (key, v) in scrubbed {
                match kind {
                    Kind::Group => {
                        let ids: Vec<String> = if multi { self.selected.iter().map(|s| s.1.clone()).collect() } else { vec![id.clone()] };
                        for g in self.board.group_cards.iter_mut().filter(|g| ids.contains(&g.id)) {
                            match key {
                                "corner" => g.corner = Some(v),
                                "border" => g.border = Some(v),
                                _ => g.font_size = v,
                            }
                        }
                    }
                    Kind::Stroke => {
                        let b = &mut self.board;
                        let strokes = b.world_strokes.iter_mut().chain(b.images.iter_mut().flat_map(|i| i.annotations.strokes.iter_mut()));
                        for s in strokes.filter(|s| &s.id == id) {
                            if key == "corner" {
                                s.radius = Some(v);
                            } else {
                                s.width = v;
                            }
                        }
                    }
                    Kind::Text => {
                        if let Some(t) = self.board.world_texts.iter_mut().find(|t| &t.id == id) {
                            t.font_size = v;
                        }
                    }
                    _ => {}
                }
            }
            self.mark_dirty();
        }

        // Popovers under the bar
        let mut pop_rect = Rect::NOTHING;
        let is_group = matches!(target, Some((Kind::Group, _)));
        // Colour: a vertical list like the other dropdowns, with "Custom" (a full picker) at the end
        let k1 = ctx.animate_bool_with_time(bid.with("k1"), pop == 1, 0.22 * anim_k());
        let cid = bid.with("custom");
        // Closing the dropdown lets the custom panel fade out with it; only when the colour
        // dropdown is opened again is a still-running exit cut short (no half-faded panel)
        let was_open = ctx.data(|d| d.get_temp::<bool>(cid.with("was1"))).unwrap_or(false);
        if pop == 1 && !was_open {
            ctx.data_mut(|d| d.insert_temp(cid, false));
            ctx.animate_bool_with_time(cid.with("k"), false, 0.0);
        }
        ctx.data_mut(|d| d.insert_temp(cid.with("was1"), pop == 1));
        if pop != 1 {
            ctx.data_mut(|d| d.insert_temp(cid, false));
        }
        if k1 > 0.001 {
            let pk = p.clone();
            let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
            let slide = (1.0 - ez(k1)) * -8.0;
            // Frames get a last row, "No fill" (just the outline and title)
            let n = PEN_COLORS.len() + 1 + is_group as usize;
            let ph = n as f32 * 26.0 + 8.0;
            let up = bar.max.y + 6.0 + ph > rect.max.y - 70.0;
            let sg = if up { -1.0 } else { 1.0 };
            let top = if up { bar.min.y - 6.0 - ph } else { bar.max.y + 6.0 } + slide * sg;
            let (rows, mags, pr) = wave_rows(&ctx, bid.with("wave1"), bar.min.x, top + n as f32 * 26.0 + 8.0, 120.0, 26.0, n, &[]);
            let dy = top - pr.min.y;
            let pr = pr.translate(vec2(0.0, dy));
            if pop != 0 {
                pop_rect = pr.expand2(vec2(14.0, 0.0));
            }
            if pop == 1 {
                ui.interact(pr, bid.with("pop1"), Sense::click());
            }
            let mut p = pk.clone();
            p.multiply_opacity(ez(k1));
            p.rect_filled(pr.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
            glass(&p, pr, 10.0, 1.0);
            p.rect_stroke(pr, 10.0, EStroke::new(1.0_f32, border()));
            let custom_on: bool = ctx.data(|d| d.get_temp(cid)).unwrap_or(false);
            let in_list = PEN_COLORS.iter().any(|c| color.eq_ignore_ascii_case(c));
            for i in 0..n {
                let rk = ez(k1 * 1.5 - i as f32 * 0.12);
                let (rr, wm) = (rows[i].translate(vec2(0.0, dy - sg * (1.0 - rk) * 10.0)), mags[i]);
                let mut p = pk.clone();
                p.multiply_opacity(ez(k1) * rk);
                let resp = ui.interact(rr, bid.with(("sw", i)), Sense::click());
                wave_tile(&p, rr, wm, mix(popup(), Color32::from_gray(90), 0.35), 1.0);
                let k = 1.0 + 0.14 * wm;
                let dot = pos2(rr.min.x + 16.0, rr.center().y);
                let (name, on) = if i < PEN_COLORS.len() {
                    let c = PEN_COLORS[i];
                    p.circle_filled(dot, 6.0 * k, hex(c));
                    (c.trim_start_matches('#').to_uppercase(), color.eq_ignore_ascii_case(c))
                } else if i == PEN_COLORS.len() + 1 {
                    // an empty ring with a slash
                    let r = 6.0 * k;
                    p.circle_stroke(dot, r, EStroke::new(1.5_f32, TEXT_DIM));
                    p.line_segment([dot + vec2(-r, r) * 0.7, dot + vec2(r, -r) * 0.7], EStroke::new(1.5_f32, hex("#ef4444")));
                    ("No fill".to_string(), color.eq_ignore_ascii_case("none"))
                } else {
                    // a four-colour ring for "custom"
                    for (j, c) in ["#ef4444", "#eab308", "#22c55e", "#3b82f6"].iter().enumerate() {
                        let a0 = j as f32 * std::f32::consts::FRAC_PI_2;
                        let pts: Vec<Pos2> = (0..=6).map(|t| dot + 6.0 * k * vec2((a0 + t as f32 * 0.26).cos(), (a0 + t as f32 * 0.26).sin())).collect();
                        p.add(egui::Shape::line(pts, EStroke::new(2.0_f32, hex(c))));
                    }
                    ("Custom".to_string(), custom_on || (!in_list && !color.is_empty() && !color.eq_ignore_ascii_case("none")))
                };
                if on {
                    p.circle_stroke(dot, 9.0 * k, EStroke::new(1.2_f32, Color32::WHITE));
                }
                p.text(pos2(rr.min.x + 32.0, rr.center().y), Align2::LEFT_CENTER, name, FontId::proportional(12.0 * k), if on { TEXT } else { mix(TEXT_DIM, TEXT, wm) });
                if pop == 1 && resp.clicked() {
                    if i < PEN_COLORS.len() {
                        let c = PEN_COLORS[i];
                        acts.push(if is_group { Act::GroupColor(c.to_string()) } else { Act::Color(c.to_string()) });
                    } else if i == PEN_COLORS.len() + 1 {
                        acts.push(Act::GroupColor("none".to_string()));
                    } else {
                        ctx.data_mut(|d| d.insert_temp(cid, !custom_on));
                    }
                }
            }
            // Custom panel: unfolds left to right with its parts fading in one after another;
            // closing plays the same thing backwards
            let kc = ctx.animate_bool_with_time(cid.with("k"), pop == 1 && custom_on, 0.3 * anim_k().max(1e-3));
            if kc > 0.001 {
                let live = pop == 1 && custom_on;
                let mut c = if color.is_empty() || color.eq_ignore_ascii_case("none") { Color32::GRAY } else { hex(&color) };
                let at = pos2(pr.max.x + 8.0, if up { pr.max.y - 290.0 } else { pr.min.y });
                let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
                let ir = egui::Area::new(bid.with("cpick")).order(egui::Order::Foreground).fixed_pos(at).interactable(live).show(&ctx, |ui| {
                    let full = 262.0;
                    let wv = 40.0 + (full - 40.0) * ez(kc);
                    let hv = 0.35 + 0.65 * ez(kc);
                    ui.set_clip_rect(Rect::from_min_size(at, vec2(wv, 400.0 * hv)));
                    ui.set_opacity(ez(kc * 1.4));
                    egui::Frame::popup(ui.style())
                        .fill(popup())
                        .rounding(12.0)
                        .inner_margin(egui::Margin::same(14.0))
                        .show(ui, |ui| super::color_picker::color_panel(ui, bid.with("cp_hsv"), &mut c, kc))
                        .inner
                });
                if live {
                    pop_rect = pop_rect.union(ir.response.rect.expand(14.0));
                }
                if kc < 1.0 {
                    ctx.request_repaint();
                }
                if live && ir.inner.1 {
                    ctx.data_mut(|d| d.insert_temp(cid, false));
                }
                if live && ir.inner.0 {
                    let h = format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
                    acts.push(if is_group { Act::GroupColor(h) } else { Act::Color(h) });
                }
            }
        }
        // Dropdowns fade/slide in and their rows cascade, like the toolbar menus; they also animate out
        let k2 = ctx.animate_bool_with_time(bid.with("k2"), pop == 2, 0.22 * anim_k());
        let k3 = ctx.animate_bool_with_time(bid.with("k3"), pop == 3, 0.22 * anim_k());
        if k2 > 0.001 {
            let pk = p.clone();
            let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
            let slide = (1.0 - ez(k2)) * -8.0;
            // Open upward when the list wouldn't fit between the bar and the bottom toolbar
            let ph = 5.0 * 26.0 + 8.0;
            let up = bar.max.y + 6.0 + ph > rect.max.y - 70.0;
            let sg = if up { -1.0 } else { 1.0 };
            let top = if up { bar.min.y - 6.0 - ph } else { bar.max.y + 6.0 } + slide * sg;
            let (rows, mags, pr) = wave_rows(&ctx, bid.with("wave2"), bar.max.x - 110.0, top + 5.0 * 26.0 + 8.0, 110.0, 26.0, 5, &[]);
            let dy = top - pr.min.y;
            let pr = pr.translate(vec2(0.0, dy));
            if pop != 0 {
                pop_rect = pr.expand2(vec2(14.0, 0.0));
            }
            if pop == 2 {
                ui.interact(pr, bid.with("pop2"), Sense::click());
            }
            let mut p = pk.clone();
            p.multiply_opacity(ez(k2));
            p.rect_filled(pr.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
            glass(&p, pr, 10.0, 1.0);
            p.rect_stroke(pr, 10.0, EStroke::new(1.0_f32, border()));
            // "None": the frame stops arranging/hugging; pasted images keep their size and spot
            for (i, name) in ["Row", "Column", "Grid", "Gallery", "None"].iter().enumerate() {
                let rk = ez(k2 * 1.5 - i as f32 * 0.15);
                let (rr, wm) = (rows[i].translate(vec2(0.0, dy - sg * (1.0 - rk) * 10.0)), mags[i]);
                let mut p = pk.clone();
                p.multiply_opacity(ez(k2) * rk);
                let resp = ui.interact(rr, bid.with(("ar", i)), Sense::click());
                wave_tile(&p, rr, wm, mix(popup(), Color32::from_gray(90), 0.35), 1.0);
                let on = if i == 4 { layout == 0 } else { layout as i32 - 1 == i as i32 };
                p.text(pos2(rr.min.x + 10.0, rr.center().y), Align2::LEFT_CENTER, *name, FontId::proportional(12.0 * (1.0 + 0.14 * wm)), if on { blue() } else { mix(TEXT_DIM, TEXT, wm) });
                if pop == 2 && resp.clicked() {
                    acts.push(Act::FrameLayout(if i == 4 { 255 } else { i as u8 }));
                    set_pop = 0;
                }
            }
        }
        if k3 > 0.001 {
            let pk = p.clone();
            let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
            let slide = (1.0 - ez(k3)) * -8.0;
            const RATIOS: [(&str, f32); 6] = [("1 : 1", 1.0), ("4 : 3", 4.0 / 3.0), ("3 : 4", 0.75), ("16 : 9", 16.0 / 9.0), ("9 : 16", 9.0 / 16.0), ("3 : 2", 1.5)];
            let cur = target.as_ref().and_then(|(_, id)| self.board.group_cards.iter().find(|g| &g.id == id)).map_or(0.0, |g| g.width / g.height.max(1.0));
            let n = RATIOS.len();
            let ph = n as f32 * 26.0 + 8.0;
            let up = bar.max.y + 6.0 + ph > rect.max.y - 70.0;
            let sg = if up { -1.0 } else { 1.0 };
            let top = if up { bar.min.y - 6.0 - ph } else { bar.max.y + 6.0 } + slide * sg;
            let (rows, mags, pr) = wave_rows(&ctx, bid.with("wave3"), bar.max.x - 150.0, top + n as f32 * 26.0 + 8.0, 110.0, 26.0, n, &[]);
            let dy = top - pr.min.y;
            let pr = pr.translate(vec2(0.0, dy));
            if pop != 0 {
                pop_rect = pr.expand2(vec2(14.0, 0.0));
            }
            if pop == 3 {
                ui.interact(pr, bid.with("pop3"), Sense::click());
            }
            let mut p = pk.clone();
            p.multiply_opacity(ez(k3));
            p.rect_filled(pr.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
            glass(&p, pr, 10.0, 1.0);
            p.rect_stroke(pr, 10.0, EStroke::new(1.0_f32, border()));
            for (i, (name, rt)) in RATIOS.iter().enumerate() {
                let rk = ez(k3 * 1.5 - i as f32 * 0.15);
                let (rr, wm) = (rows[i].translate(vec2(0.0, dy - sg * (1.0 - rk) * 10.0)), mags[i]);
                let mut p = pk.clone();
                p.multiply_opacity(ez(k3) * rk);
                let resp = ui.interact(rr, bid.with(("asp", i)), Sense::click());
                wave_tile(&p, rr, wm, mix(popup(), Color32::from_gray(90), 0.35), 1.0);
                let on = (cur - rt).abs() < 0.01;
                // little shape preview of the ratio
                let k = 1.0 + 0.14 * wm;
                let sz = if *rt >= 1.0 { vec2(14.0, 14.0 / rt) } else { vec2(14.0 * rt, 14.0) } * k;
                let col = if on { blue() } else { mix(TEXT_DIM, TEXT, wm) };
                p.rect_stroke(Rect::from_center_size(pos2(rr.min.x + 16.0, rr.center().y), sz), 1.5, EStroke::new(1.2_f32, col));
                p.text(pos2(rr.min.x + 32.0, rr.center().y), Align2::LEFT_CENTER, *name, FontId::proportional(12.0 * k), col);
                if pop == 3 && resp.clicked() {
                    acts.push(Act::FrameAspect(*rt));
                    set_pop = 0;
                }
            }
        }
        // Align dropdown: icon rows like the placement bar's buttons, even spacing for 3+
        let k4 = ctx.animate_bool_with_time(bid.with("k4"), pop == 4, 0.22 * anim_k());
        if k4 > 0.001 {
            let pk = p.clone();
            let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
            let slide = (1.0 - ez(k4)) * -8.0;
            let mut rows_def: Vec<(&str, Option<Align>, bool)> = vec![
                ("Left", Some(Align::Left), false),
                ("Centre", Some(Align::HCenter), false),
                ("Right", Some(Align::Right), false),
                ("Top", Some(Align::Top), false),
                ("Middle", Some(Align::VCenter), false),
                ("Bottom", Some(Align::Bottom), false),
            ];
            if place_targets.len() >= 3 {
                rows_def.push(("Space across", None, true));
                rows_def.push(("Space down", None, false));
            }
            let n = rows_def.len();
            let ph = n as f32 * 26.0 + 8.0;
            let up = bar.max.y + 6.0 + ph > rect.max.y - 70.0;
            let sg = if up { -1.0 } else { 1.0 };
            let top = if up { bar.min.y - 6.0 - ph } else { bar.max.y + 6.0 } + slide * sg;
            let (rows, mags, pr) = wave_rows(&ctx, bid.with("wave4"), bar.max.x - 160.0, top + n as f32 * 26.0 + 8.0, 130.0, 26.0, n, &[]);
            let dy = top - pr.min.y;
            let pr = pr.translate(vec2(0.0, dy));
            if pop != 0 {
                pop_rect = pr.expand2(vec2(14.0, 0.0));
            }
            if pop == 4 {
                ui.interact(pr, bid.with("pop4"), Sense::click());
            }
            let mut p = pk.clone();
            p.multiply_opacity(ez(k4));
            p.rect_filled(pr.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
            glass(&p, pr, 10.0, 1.0);
            p.rect_stroke(pr, 10.0, EStroke::new(1.0_f32, border()));
            for (i, (name, al, across)) in rows_def.iter().enumerate() {
                let rk = ez(k4 * 1.5 - i as f32 * 0.12);
                let (rr, wm) = (rows[i].translate(vec2(0.0, dy - sg * (1.0 - rk) * 10.0)), mags[i]);
                let mut p = pk.clone();
                p.multiply_opacity(ez(k4) * rk);
                let resp = ui.interact(rr, bid.with(("aln", i)), Sense::click());
                wave_tile(&p, rr, wm, mix(popup(), Color32::from_gray(90), 0.35), 1.0);
                let k = 1.0 + 0.14 * wm;
                let col = mix(TEXT_DIM, TEXT, wm);
                let ic = pos2(rr.min.x + 16.0, rr.center().y);
                match al {
                    Some(a) => super::icons::align(&p, ic, 15.0 * k, col, match a {
                        Align::Left => 0,
                        Align::HCenter => 1,
                        Align::Right => 2,
                        Align::Top => 3,
                        Align::VCenter => 4,
                        Align::Bottom => 5,
                    }),
                    None => super::icons::distribute(&p, ic, 15.0 * k, col, *across),
                }
                p.text(pos2(rr.min.x + 32.0, rr.center().y), Align2::LEFT_CENTER, *name, FontId::proportional(12.0 * k), col);
                if pop == 4 && resp.clicked() {
                    match al {
                        Some(a) => self.align_selection(*a),
                        None => self.distribute_selection(*across),
                    }
                }
            }
        }
        // Stroke position dropdown
        let k5 = ctx.animate_bool_with_time(bid.with("k5"), pop == 5, 0.22 * anim_k());
        if k5 > 0.001 {
            let pk = p.clone();
            let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
            let slide = (1.0 - ez(k5)) * -8.0;
            let rows_def: [(&str, u8); 3] = [("Inside", 1), ("Center", 0), ("Outside", 2)];
            let n = rows_def.len();
            let ph = n as f32 * 26.0 + 8.0;
            let up = bar.max.y + 6.0 + ph > rect.max.y - 70.0;
            let sg = if up { -1.0 } else { 1.0 };
            let top = if up { bar.min.y - 6.0 - ph } else { bar.max.y + 6.0 } + slide * sg;
            let (rows, mags, pr) = wave_rows(&ctx, bid.with("wave5"), bar.center().x - 55.0, top + n as f32 * 26.0 + 8.0, 110.0, 26.0, n, &[]);
            let dy = top - pr.min.y;
            let pr = pr.translate(vec2(0.0, dy));
            if pop != 0 {
                pop_rect = pr.expand2(vec2(14.0, 0.0));
            }
            if pop == 5 {
                ui.interact(pr, bid.with("pop5"), Sense::click());
            }
            let mut p = pk.clone();
            p.multiply_opacity(ez(k5));
            p.rect_filled(pr.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
            glass(&p, pr, 10.0, 1.0);
            p.rect_stroke(pr, 10.0, EStroke::new(1.0_f32, border()));
            for (i, (name, v)) in rows_def.iter().enumerate() {
                let rk = ez(k5 * 1.5 - i as f32 * 0.12);
                let (rr, wm) = (rows[i].translate(vec2(0.0, dy - sg * (1.0 - rk) * 10.0)), mags[i]);
                let mut p = pk.clone();
                p.multiply_opacity(ez(k5) * rk);
                let resp = ui.interact(rr, bid.with(("spos", i)), Sense::click());
                let cur = *v == stroke_pos;
                wave_tile(&p, rr, wm, mix(popup(), Color32::from_gray(90), 0.35), 1.0);
                let k = 1.0 + 0.14 * wm;
                let col = if cur { blue() } else { mix(TEXT_DIM, TEXT, wm) };
                stroke_pos_icon(&p, pos2(rr.min.x + 16.0, rr.center().y), k, col, *v);
                p.text(pos2(rr.min.x + 32.0, rr.center().y), Align2::LEFT_CENTER, *name, FontId::proportional(12.0 * k), col);
                if pop == 5 && resp.clicked() {
                    acts.push(Act::StrokePos(*v));
                    set_pop = 0;
                }
            }
        }
        if pop != 0 && set_pop == pop && started {
            let q = ui.input(|i| i.pointer.interact_pos());
            if !q.is_some_and(|q| bar.contains(q) || pop_rect.contains(q)) {
                set_pop = 0;
            }
        }
        // Auto-collapse, like the toolbar dropdowns: the pointer stays well clear of the bar and
        // its popover for a moment (leaving the window counts too)
        ui_block(&ctx, if pop != 0 && pop_rect.is_positive() { bar.union(pop_rect) } else { bar });
        if pop != 0 && set_pop == pop {
            let zone = if pop_rect.is_positive() { bar.union(pop_rect) } else { bar };
            let far = ctx.input(|i| i.pointer.hover_pos()).map_or(true, |q| {
                let d = vec2((zone.min.x - q.x).max(q.x - zone.max.x).max(0.0), (zone.min.y - q.y).max(q.y - zone.max.y).max(0.0));
                d.length() > 140.0
            });
            let aid = bid.with("away");
            let now = ctx.input(|i| i.time);
            if far {
                let t = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(aid, || now));
                ctx.request_repaint();
                if now - t > 0.35 {
                    set_pop = 0;
                }
            } else {
                ctx.data_mut(|d| d.remove::<f64>(aid));
            }
        }
        if set_pop == 0 {
            ctx.data_mut(|d| d.remove::<f64>(bid.with("away")));
        }
        ctx.data_mut(|d| d.insert_temp(bid.with("pop"), set_pop));

        // Tooltip above the pill (hidden while a popover is open)
        let tip = tip.filter(|_| set_pop == 0);
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

/// Square with a thick edge sitting inside / on / outside a thin outline
fn stroke_pos_icon(p: &egui::Painter, c: Pos2, k: f32, col: Color32, pos: u8) {
    let r = Rect::from_center_size(c, vec2(10.0, 10.0) * k);
    p.rect_stroke(r, 1.5, EStroke::new(1.0_f32, col.gamma_multiply(0.45)));
    let thick = match pos {
        1 => r.shrink(2.0 * k),
        2 => r.expand(1.0 * k),
        _ => r.shrink(0.5 * k),
    };
    p.rect_stroke(thick, 1.5, EStroke::new(1.6_f32, col));
}
