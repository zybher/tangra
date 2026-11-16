//! The placement bar's Align dropdown: the same wave list as the frame bar's Align
//! dropdown (left / centre / right / top / middle / bottom, even spacing for 3+).

use crate::*;

impl App {
    /// Draws the dropdown under (or over) `bar`, starting at the Align button `btn`.
    /// `open` is the bar's open dropdown (1 = Align). Returns its rect while open.
    pub(crate) fn place_align_pop(&mut self, ui: &mut egui::Ui, rect: Rect, bar: Rect, btn: Rect, open: u8, spacing: bool) -> Rect {
        let ctx = ui.ctx().clone();
        let bid = egui::Id::new("qd_place_align");
        let da = super::drop_anim::DropAnim::new(&ctx, egui::Id::new("qd_place_bar").with("drop"), open);
        let k4 = da.k(1);
        if k4 <= 0.001 {
            return Rect::NOTHING;
        }
        let slide = (1.0 - k4) * -8.0;
        let mut rows_def: Vec<(&str, Option<Align>, bool)> = vec![
            ("Left", Some(Align::Left), false),
            ("Centre", Some(Align::HCenter), false),
            ("Right", Some(Align::Right), false),
            ("Top", Some(Align::Top), false),
            ("Middle", Some(Align::VCenter), false),
            ("Bottom", Some(Align::Bottom), false),
        ];
        if spacing {
            rows_def.push(("Space across", None, true));
            rows_def.push(("Space down", None, false));
        }
        let n = rows_def.len();
        let w = super::drop_anim::fit_w(&ctx, 32.0, &rows_def.iter().map(|r| r.0).collect::<Vec<_>>());
        // Keep the last spot while it fades out (the button may already be gone)
        let (cols, ph, up) = super::drop_anim::drop_fit(&ctx, egui::Id::new("qd_place_bar"), bar, n, rect, w);
        let tw = grid_w(w, cols);
        let left = if btn.is_positive() { super::drop_anim::left_for(btn.center().x, rect, tw) } else { ctx.data(|d| d.get_temp(bid.with("x"))).unwrap_or(bar.min.x) };
        ctx.data_mut(|d| d.insert_temp(bid.with("x"), left));
        let sg = if up { -1.0 } else { 1.0 };
        let top = if up { bar.min.y - 14.0 - ph } else { bar.max.y + 14.0 } + slide * sg;
        let (rows, mags, pr) = wave_grid(&ctx, bid.with("wave"), left, top + ph, w, super::drop_anim::PITCH, n, cols);
        let dy = top - pr.min.y;
        let pr = pr.translate(vec2(0.0, dy));
            let (bgr, gx) = da.panel(&ctx, pr, up);
        let live = open == 1;
        if live {
            ui.interact(pr, bid.with("panel"), Sense::click());
        }
        let pk = ui.painter().clone();
        let mut p = pk.clone();
        p.multiply_opacity(k4);
        p.rect_filled(bgr.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
        glass(&p, bgr, 10.0, 1.0);
        p.rect_stroke(bgr, 10.0, EStroke::new(1.0_f32, border()));
        for (i, (name, al, across)) in rows_def.iter().enumerate() {
            let rk = da.row(i, rows.len(), up);
            let (rr, wm) = (rows[i].translate(vec2(gx, dy - sg * (1.0 - rk) * 16.0)), mags[i]);
            let mut p = pk.clone();
            p.multiply_opacity(k4 * rk);
            let resp = ui.interact(rr, bid.with(("row", i)), if live { Sense::click() } else { Sense::hover() });
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
            if live && resp.clicked() {
                match al {
                    Some(a) => self.align_selection(*a),
                    None => self.distribute_selection(*across),
                }
            }
        }
        if k4 < 1.0 {
            ctx.request_repaint();
        }
        if live {
            pr.expand2(vec2(14.0, 0.0))
        } else {
            Rect::NOTHING
        }
    }
}
