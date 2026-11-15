//! Width/height pills around the selection (glide + rolling digits).

use crate::*;

impl App {
    /// Width/height pills around a selection rect `r` (screen space); `w`/`h` are world sizes.
    pub(crate) fn size_pills(&self, ctx: &egui::Context, p: &egui::Painter, rect: Rect, r: Rect, id: &str, w: f32, h: f32) {
        let ctx = ctx.clone();
        // When the selection changes (one object -> several, or A -> B), glide the pills
        // from where they were instead of snapping; all pill animations share one state.
        let gid = egui::Id::new("qd_dims_glide");
        let now = ctx.input(|i| i.time);
        let (lid, lr, mut from, mut t0, lt) = ctx.data(|d| d.get_temp::<(String, Rect, Rect, f64, f64)>(gid)).unwrap_or((String::new(), r, r, -1.0, -10.0));
        // Counted in frames, not seconds: the app doesn't redraw while idle
        let fr = ctx.cumulative_pass_nr() as f64;
        let shown_last_frame = fr - lt <= 1.0;
        if lid != id && shown_last_frame {
            (from, t0) = (lr, now);
        } else if lid != id {
            t0 = -1.0;
        }
        // Fresh appearance (nothing was selected a moment ago): always fade in, even if the
        // pointer is already near
        let aid = egui::Id::new("qd_dims_appear");
        if !shown_last_frame {
            ctx.data_mut(|d| d.insert_temp(aid, now));
        }
        let appear = ((now - ctx.data(|d| d.get_temp::<f64>(aid)).unwrap_or(0.0)) / 0.15).clamp(0.0, 1.0) as f32;
        if appear < 1.0 {
            ctx.request_repaint();
        }
        let k = if t0 < 0.0 { 1.0 } else { ((now - t0) / 0.3).clamp(0.0, 1.0) as f32 };
        let e = egui::emath::easing::cubic_out(k);
        let r = Rect::from_min_max(from.min.lerp(r.min, e), from.max.lerp(r.max, e));
        // Digits roll only while gliding to a new selection; resizing just updates the text
        // ...and a little after the glide lands, so digits mid-roll finish instead of snapping
        let roll = if t0 >= 0.0 && now - t0 < 0.3 + 0.3 { 0.25 } else { 0.0 };
        if k < 1.0 {
            ctx.request_repaint();
        }
        ctx.data_mut(|d| d.insert_temp(gid, (id.to_string(), r, from, t0, fr)));
                    // Size pills: width under + height right, or width over + height left, following the pointer's side
                    let hov = ctx.pointer_hover_pos();
                    // Generous zone, wider once showing so it doesn't flicker at the edge
                    let was = ctx.data(|d| d.get_temp::<bool>(egui::Id::new(("qd_dims_on", "qd")))).unwrap_or(false);
                    // Scrubbing a value or using a style-bar dropdown pins the pills (and their side),
                    // so the bar above doesn't jump while you aim at it
                    let sbid = egui::Id::new("qd_style_bar");
                    let scrub = ctx.data(|d| d.get_temp::<bool>(sbid.with("scrub")).unwrap_or(false) || d.get_temp::<u8>(sbid.with("pop")).unwrap_or(0) != 0);
                    let alt_measure = ALT_DOWN.load(std::sync::atomic::Ordering::Relaxed) && matches!(self.drag, Drag::None);
                    // Renaming a frame counts as "pointer gone": same fade-out as moving away
                    let renaming = matches!(&self.edit, Some(Edit { target: EditTarget::Group(_), .. }));
                    let near = !alt_measure && !renaming && (scrub || ctx.input(|i| i.modifiers.shift) || matches!(self.drag, Drag::Resize { .. } | Drag::BoxSelect { .. })
                        || hov.map_or(false, |q| r.expand(if was { 160.0 } else { 110.0 }).contains(q)));
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new(("qd_dims_on", "qd")), near));
                    let da = ctx.animate_bool_with_time(egui::Id::new(("qd_dims", "qd")), near, 0.15 * anim_k()).min(appear);
                    let sid2 = egui::Id::new("qd_dims_tl");
                    // Side (left/right) and edge (top/bottom) follow the cursor independently: 4 combos
                    let (mut sl, mut st) = ctx.data(|d| d.get_temp::<(bool, bool)>(sid2)).unwrap_or((false, false));
                    if let (Some(q), true, false) = (hov, near, scrub) {
                        sl = q.x < r.center().x;
                        st = q.y < r.center().y;
                    }
                    if let Drag::Resize { left, .. } = &self.drag {
                        sl = *left;
                    }
                    // The style bar owns its edge: the width pill stays on the other one, so it never
                    // starts toward the bar and backs out
                    let (bar_top0, bar_nr0) = ctx.data(|d| d.get_temp::<(bool, u64)>(egui::Id::new("qd_bar_side"))).unwrap_or((true, 0));
                    let bar_kind0 = self.selected.len() == 1 && self.sel_label.is_none() && self.edit.is_none()
                        && matches!(self.sel, Some((Kind::Group | Kind::Stroke | Kind::Text, _)));
                    if ctx.cumulative_pass_nr().saturating_sub(bar_nr0) <= 1 {
                        st = !bar_top0;
                    } else if bar_kind0 {
                        st = !(r.min.y - 32.0 - 34.0 >= rect.min.y + 50.0);
                    }
                    ctx.data_mut(|d| d.insert_temp(sid2, (sl, st)));
                    let tl = st;
                    if da > 0.01 {
                        let a = da;
                        let gap = 8.0 - (1.0 - da) * 4.0;
                        let fnt = semibold(12.0);
                        let gw = p.layout_no_wrap(format!("{:.0}", w), fnt.clone(), Color32::WHITE);
                        let ph = gw.size().y + 6.0;
                        let gh = p.layout_no_wrap(format!("{:.0}", h), fnt.clone(), Color32::WHITE);
                        let (tw, th) = (gh.size().x, gh.size().y);
                        let ez = egui::emath::easing::cubic_in_out;
                        let kx = ctx.animate_bool_with_time_and_easing(egui::Id::new(("qd_dims_sx", "qd")), sl, 0.3 * anim_k(), ez);
                        let ky = ctx.animate_bool_with_time_and_easing(egui::Id::new(("qd_dims_sy", "qd")), tl, 0.3 * anim_k(), ez);
                        // Glide between sides instead of jumping
                        let hx = egui::lerp((r.max.x + gap + ph / 2.0)..=(r.min.x - gap - ph / 2.0), kx);
                        let hp = Rect::from_center_size(pos2(hx, r.center().y), vec2(th + 6.0, tw + 14.0));
                        let wy = egui::lerp((r.max.y + gap + ph / 2.0)..=(r.min.y - gap - ph / 2.0), ky);
                        let mut inside = rect.shrink(4.0);
                        inside.min.x += crate::ui::outline::hud_shift(&ctx);
                        // The style bar sits on this edge: the width folds into the height pill
                        // (the bar is drawn after the pills, so on its first frame predict it the same way it
                        // places itself, instead of letting the pill flash on that edge for a frame)
                        let (bar_top, bar_nr) = ctx.data(|d| d.get_temp::<(bool, u64)>(egui::Id::new("qd_bar_side"))).unwrap_or((true, 0));
                        let fresh = ctx.cumulative_pass_nr().saturating_sub(bar_nr) <= 1;
                        let bar_kind = self.selected.len() == 1 && self.sel_label.is_none() && self.edit.is_none()
                            && matches!(self.sel, Some((Kind::Group | Kind::Stroke | Kind::Text, _)));
                        let bar_top = if fresh { bar_top } else { r.min.y - 32.0 - 34.0 >= rect.min.y + 50.0 };
                        let bar_here = (fresh || bar_kind) && bar_top == tl;
                        let w_hidden = bar_here || !inside.contains_rect(Rect::from_center_size(pos2(r.center().x, wy), gw.size() + vec2(14.0, 6.0)));
                        // One side out of view: fold its number into the other pill
                        let h_hidden = !inside.contains_rect(hp) && !w_hidden;
                        let mk = ctx.animate_bool_with_time_and_easing(egui::Id::new(("qd_dims_merge", "qd")), h_hidden, 0.3 * anim_k(), egui::emath::easing::cubic_out);
                        let mw = ctx.animate_bool_with_time_and_easing(egui::Id::new(("qd_dims_mergew", "qd")), w_hidden && inside.contains_rect(hp), 0.3 * anim_k(), egui::emath::easing::cubic_out);
                        // Width pill (+ " × h")
                        let gx = p.layout_no_wrap(format!(" × {:.0}", h), fnt.clone(), Color32::WHITE);
                        let full = gw.size().x + gx.size().x * mk;
                        let wp = Rect::from_center_size(pos2(r.center().x, wy), vec2(full + 14.0, ph));
                        // Fade the width pill out near the canvas edge, like the height pill
                        let fw = ctx.animate_bool_with_time_and_easing(egui::Id::new(("qd_dims_fw", "qd")), w_hidden, 0.3 * anim_k(), egui::emath::easing::cubic_out);
                        let a = a * (1.0 - fw);
                        // Pills slide "behind" the frame: each is drawn only outside the frame's edges
                        let cr = p.clip_rect();
                        let big = 1.0e5;
                        let x0 = wp.min.x + 7.0;
                        for clip in [Rect::from_min_max(pos2(-big, -big), pos2(big, r.min.y)), Rect::from_min_max(pos2(-big, r.max.y), pos2(big, big))] {
                            let q = p.with_clip_rect(clip.intersect(cr));
                            q.rect_filled(wp, 4.0, self.match_col(true).gamma_multiply(a));
                            roll_text(&q.with_clip_rect(wp.intersect(q.clip_rect())), egui::Id::new("qd_roll_w"), pos2(x0, wp.center().y - gw.size().y / 2.0), &format!("{:.0}", w), &fnt, Color32::WHITE.gamma_multiply(a), 0.0, roll);
                            if mk > 0.001 {
                                // roulette: the suffix rolls up into place, clipped to the pill
                                let cp = q.with_clip_rect(wp.intersect(q.clip_rect()));
                                let y = wp.center().y - gx.size().y / 2.0 + (1.0 - mk) * ph;
                                cp.add(egui::epaint::TextShape::new(pos2(x0 + gw.size().x, y), gx.clone(), Color32::WHITE).with_override_text_color(Color32::WHITE.gamma_multiply(a * mk)));
                            }
                        }
                        // Height pill (reads bottom-up; "w × " prefix rolls in)
                        let ha = da * (1.0 - mk);
                        if ha > 0.01 {
                            let gp = p.layout_no_wrap(format!("{:.0} × ", w), fnt.clone(), Color32::WHITE);
                            let pw = gp.size().x * mw;
                            let len = tw + pw;
                            let hp = Rect::from_center_size(pos2(hx, r.center().y), vec2(th + 6.0, len + 14.0));
                            let y0 = r.center().y + len / 2.0;
                            let rot = -std::f32::consts::FRAC_PI_2;
                            for clip in [Rect::from_min_max(pos2(-big, -big), pos2(r.min.x, big)), Rect::from_min_max(pos2(r.max.x, -big), pos2(big, big))] {
                                let q = p.with_clip_rect(clip.intersect(cr));
                                q.rect_filled(hp, 4.0, self.match_col(false).gamma_multiply(ha));
                                if mw > 0.001 {
                                    let cp = q.with_clip_rect(hp.intersect(q.clip_rect()));
                                    cp.add(egui::epaint::TextShape::new(pos2(hx - th / 2.0 + (1.0 - mw) * ph, y0), gp.clone(), Color32::WHITE).with_override_text_color(Color32::WHITE.gamma_multiply(ha * mw)).with_angle(rot));
                                }
                                roll_text(&q.with_clip_rect(hp.intersect(q.clip_rect())), egui::Id::new("qd_roll_h"), pos2(hx - th / 2.0, y0 - pw), &format!("{:.0}", h), &fnt, Color32::WHITE.gamma_multiply(ha), rot, roll);
                            }
                        }
                        // The style bar stacks outside the width pill instead of covering it
                        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_dims_push"), (tl, (gap + ph) * da)));
                    } else {
                        ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_dims_push"), (tl, 0.0_f32)));
                    }
    }
}
