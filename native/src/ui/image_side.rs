//! Picture side bar: a small upright pill beside a selected picture (right, else left) with
//! the Adjust and 3D buttons; their dropdown opens next to it, so nothing covers the picture.
//! Behaves like the placement bar turned upright: jelly entrance and domino exit, dock-style
//! magnify along its length, the same lagging glide when the view moves, and the accent liquid
//! on the button whose dropdown is open. Publishes its area so the height pill folds into the
//! width pill instead of sitting under it.

use crate::*;

const BW: f32 = 34.0;
const BTN: f32 = 28.0;
const PAD: f32 = 4.0;

impl App {
    /// `fr`: the picture on screen. `show` false plays the exit.
    pub(crate) fn image_side_ui(&mut self, ui: &mut egui::Ui, rect: Rect, fr: Rect, img: &str, show: bool) {
        let ctx = ui.ctx().clone();
        let bid = egui::Id::new("qd_img_side");
        let k = anim_k().max(1e-3);
        let mut pop: u8 = ctx.data(|d| d.get_temp(bid.with("pop"))).unwrap_or(0);
        let last: String = ctx.data(|d| d.get_temp(bid.with("img"))).unwrap_or_default();
        let was_shown: bool = ctx.data(|d| d.get_temp(bid.with("was_shown"))).unwrap_or(false);
        // Entrance replays for a new picture or after being hidden; exit is timed from hiding
        if show && (!was_shown || last != img) {
            let now = self.now;
            ctx.data_mut(|d| {
                d.insert_temp(bid.with("t0"), now);
                if !was_shown {
                    d.remove::<(f32, f32)>(bid.with("glide"));
                }
            });
            pop = 0;
        }
        if was_shown && !show {
            let now = self.now;
            ctx.data_mut(|d| d.insert_temp(bid.with("t_gone"), now));
        }
        if !show {
            pop = 0;
        }
        ctx.data_mut(|d| {
            d.insert_temp(bid.with("img"), img.to_string());
            d.insert_temp(bid.with("was_shown"), show);
        });
        let closing: Option<f32> = if show {
            None
        } else {
            let t_gone: f64 = ctx.data(|d| d.get_temp(bid.with("t_gone"))).unwrap_or(self.now);
            Some(((self.now - t_gone) as f32 / (0.3 * k)).clamp(0.0, 1.0))
        };
        if closing.is_some_and(|e| e >= 1.0) {
            ctx.data_mut(|d| d.insert_temp(bid.with("pop"), 0u8));
            return;
        }
        let t0: f64 = ctx.data(|d| d.get_temp(bid.with("t0"))).unwrap_or(0.0);
        let x = ((self.now - t0) as f32 / (0.6 * k)).clamp(0.0, 1.0);

        // Dock-style magnify along the pill (as the bars' sideways one)
        let was_drag: bool = ctx.data(|d| d.get_temp(egui::Id::new("qd_img_pop").with("dragging"))).unwrap_or(false);
        let mags: [f32; 2] = {
            let prev = ctx.data(|d| d.get_temp::<Rect>(bid)).unwrap_or(Rect::NOTHING);
            let ptr = ctx.pointer_hover_pos();
            let inside = show && !was_drag && ptr.is_some_and(|q| prev.expand2(vec2(8.0, 0.0)).contains(q));
            let sa = spring(&ctx, bid.with("mag_a"), inside, 18.0);
            let fid = bid.with("mag_f");
            let mut focus = ctx.data(|d| d.get_temp::<f32>(fid)).unwrap_or(0.0);
            if let (true, Some(q)) = (inside, ptr) {
                let target = q.y - prev.center().y;
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
            let mut m = [0.0; 2];
            for (i, mm) in m.iter_mut().enumerate() {
                let off = -BTN / 2.0 + i as f32 * BTN;
                let d = (off - focus) / (BTN * 1.05);
                // The button whose dropdown is open stays swollen
                let act = spring(&ctx, bid.with(("mag_on", i)), pop == i as u8 + 1, 18.0);
                *mm = (sa * (-d * d).exp()).max(act);
            }
            m
        };
        let slot = |i: usize| BTN * (1.0 + 0.22 * mags[i]) + 10.0 * mags[i];
        let bh = PAD * 2.0 + slot(0) + slot(1);
        let bh_rest = PAD * 2.0 + BTN * 2.0;
        let left_edge = rect.min.x + crate::ui::outline::hud_shift(&ctx) + 8.0;
        // Right of the picture when the bar and its dropdown fit there, else left
        let right = fr.max.x + 10.0 + BW + 8.0 + super::image_sliders::PANEL_W <= rect.max.x - 8.0 || fr.min.x - 10.0 - BW < left_edge;
        let cx = if right { fr.max.x + 10.0 + BW / 2.0 } else { fr.min.x - 10.0 - BW / 2.0 };
        let cy = fr.min.y.clamp(rect.min.y + 8.0, (rect.max.y - 70.0 - bh_rest).max(rect.min.y + 8.0)) + bh_rest / 2.0;
        // Lags behind its spot like the bars do when the view moves
        let (gx, gy) = {
            let (mut gx, mut gy) = ctx.data(|d| d.get_temp::<(f32, f32)>(bid.with("glide"))).unwrap_or((cx, cy));
            let dt = ctx.input(|i| i.stable_dt).min(0.05);
            let kf = if anim_k() == 0.0 { 1.0 } else { 1.0 - (-dt * 16.0 / anim_k()).exp() };
            gx += (cx - gx) * kf;
            gy += (cy - gy) * kf;
            if (cx - gx).abs() + (cy - gy).abs() < 0.3 {
                (gx, gy) = (cx, cy);
            } else {
                ctx.request_repaint();
            }
            ctx.data_mut(|d| d.insert_temp(bid.with("glide"), (gx, gy)));
            (gx, gy)
        };
        // Grows from its top edge as the buttons swell
        let bar = Rect::from_min_size(pos2(gx - BW / 2.0, gy - bh_rest / 2.0), vec2(BW, bh));
        ctx.data_mut(|d| d.insert_temp(bid, bar));

        // Pill springs open lengthwise with the bars' jelly; shrinks away on exit
        let mut f = 1.0 - (-4.5 * x).exp() * (8.0 * x).cos();
        let mut wob = (-5.5 * x).exp() * (16.0 * x).sin() * 0.06;
        let mut a = (x / 0.15).min(1.0);
        if let Some(e) = closing {
            f = 1.0 - egui::emath::easing::cubic_in(e);
            wob = 0.0;
            a = ((1.0 - e) / 0.35).min(1.0);
            ctx.request_repaint();
        }
        if x < 1.0 {
            ctx.request_repaint();
        }
        let sh = (bh * (0.12 + 0.88 * f) * (1.0 + wob)).max(BW);
        let sw = BW * (0.8 + 0.2 * f.min(1.0)) * (1.0 - wob * 0.6);
        let shown = Rect::from_center_size(bar.center() + vec2(0.0, (1.0 - f) * 6.0), vec2(sw, sh));
        let p = ui.painter().clone();
        p.rect_filled(shown, sw / 2.0, frame_bg().gamma_multiply(a));
        p.rect_stroke(shown, sw / 2.0, EStroke::new(1.0_f32, border().gamma_multiply(a)));
        if show {
            ui.interact(bar, bid.with("bg"), Sense::click_and_drag());
        }
        let mut yy = bar.min.y + PAD;
        for (i, kind) in [1u8, 2].into_iter().enumerate() {
            let sl = slot(i);
            let mg = mags[i];
            // Each button rises into place a beat after the last; domino out on exit
            let gi = match closing {
                Some(e) => 1.0 - ((e - i as f32 * 0.07) / 0.3).clamp(0.0, 1.0),
                None => ((x - 0.02 - i as f32 * 0.06) / 0.3).clamp(0.0, 1.0),
            };
            let ca = 1.0 - (1.0 - gi).powi(3);
            let r = Rect::from_center_size(pos2(bar.center().x, yy + sl / 2.0 + (1.0 - ca) * 9.0), vec2(BTN, BTN - 2.0));
            yy += sl;
            if ca <= 0.0 {
                continue;
            }
            // Swell + lift outward (away from the picture) on its own layer, like the bars
            let lid = egui::LayerId::new(egui::Order::Foreground, bid.with(("seg", i)));
            let sc = 1.0 + 0.22 * mg;
            let tv = r.center().to_vec2() * (1.0 - sc) + vec2(if right { 4.0 } else { -4.0 } * mg, 0.0);
            ctx.set_transform_layer(lid, egui::emath::TSTransform::new(tv, sc));
            let tf = move |q: Pos2| pos2(q.x * sc + tv.x, q.y * sc + tv.y);
            let tr = move |rr: Rect| Rect::from_min_max(tf(rr.min), tf(rr.max));
            let pl = p.clone().with_layer_id(lid);
            let resp = ui.interact(r, bid.with(("b", kind)), if show { Sense::click() } else { Sense::hover() });
            let (hh, pp) = hover_fx(&ctx, &resp, show);
            p.rect_filled(tr(r), 8.0 * sc, hover_fill(hh, pp).gamma_multiply(ca));
            // Open: accent liquid fill, like the bars' dropdown buttons
            let lq = ctx.animate_bool_with_time(bid.with(("liquid", kind)), pop == kind, 0.5 * anim_k());
            let lq = lq * lq * (3.0 - 2.0 * lq);
            if lq >= 0.999 {
                p.rect_filled(tr(r), 8.0 * sc, blue().gamma_multiply(ca));
            } else if lq > 0.001 {
                liquid_fill(&p, tr(r), 8.0 * sc, lq, ctx.input(|i| i.time) as f32 + kind as f32);
            }
            let col = mix(TEXT_DIM, Color32::WHITE, hh.max(lq)).gamma_multiply(ca);
            if kind == 1 {
                super::icons::adjust(&pl, r.center(), 16.0, col);
            } else {
                super::icons::cube(&pl, r.center(), 16.0, col);
            }
            if show && resp.clicked() {
                pop = if pop == kind { 0 } else { kind };
            }
        }
        let pop_rect = self.image_pop(ui, rect, bar, right, pop, img);
        // A press anywhere else closes the dropdown
        if pop != 0 && ctx.input(|i| i.pointer.primary_pressed()) {
            let q = ctx.input(|i| i.pointer.interact_pos());
            if !q.is_some_and(|q| bar.contains(q) || pop_rect.contains(q)) {
                pop = 0;
            }
        }
        ctx.data_mut(|d| d.insert_temp(bid.with("pop"), pop));
        let zone = if pop_rect.is_positive() { bar.union(pop_rect) } else { bar };
        if show {
            ui_block(&ctx, zone);
        }
        // The size pills read this (one pass late) to fold the height number away from it
        let nr = ctx.cumulative_pass_nr();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_img_side_zone"), (zone, nr)));
    }
}
