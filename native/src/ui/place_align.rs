//! The placement bar's Align dropdown: the same wave list as the frame bar's Align
//! dropdown (left / centre / right / top / middle / bottom, even spacing for 3+).

use crate::*;

impl App {
    /// Draws the dropdown under (or over) `bar`, starting at the Align button `btn`.
    /// `open` is the bar's open dropdown (1 = Align). Returns its rect while open.
    pub(crate) fn place_align_pop(&mut self, ui: &mut egui::Ui, rect: Rect, bar: Rect, btn: Rect, open: u8, spacing: bool) -> Rect {
        let ctx = ui.ctx().clone();
        let bid = egui::Id::new("qd_place_align");
        let k4 = ctx.animate_bool_with_time(bid.with("k"), open == 1, 0.22 * anim_k());
        if k4 <= 0.001 {
            return Rect::NOTHING;
        }
        // Keep the last spot while it fades out (the button may already be gone)
        let left = if btn.is_positive() { btn.min.x - 4.0 } else { ctx.data(|d| d.get_temp(bid.with("x"))).unwrap_or(bar.min.x) };
        ctx.data_mut(|d| d.insert_temp(bid.with("x"), left));
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
        if spacing {
            rows_def.push(("Space across", None, true));
            rows_def.push(("Space down", None, false));
        }
        let n = rows_def.len();
        let ph = n as f32 * 26.0 + 8.0;
        let up = bar.max.y + 6.0 + ph > rect.max.y - 70.0;
        let sg = if up { -1.0 } else { 1.0 };
        let top = if up { bar.min.y - 6.0 - ph } else { bar.max.y + 6.0 } + slide * sg;
        let (rows, mags, pr) = wave_rows(&ctx, bid.with("wave"), left, top + ph, 130.0, 26.0, n, &[]);
        let dy = top - pr.min.y;
        let pr = pr.translate(vec2(0.0, dy));
        let live = open == 1;
        if live {
            ui.interact(pr, bid.with("panel"), Sense::click());
        }
        let pk = ui.painter().clone();
        let mut p = pk.clone();
        p.multiply_opacity(ez(k4));
        glass(&p, pr, 10.0, 1.0);
        p.rect_stroke(pr, 10.0, EStroke::new(1.0_f32, border()));
        for (i, (name, al, across)) in rows_def.iter().enumerate() {
            let rk = ez(k4 * 1.5 - i as f32 * 0.12);
            let (rr, wm) = (rows[i].translate(vec2(0.0, dy - sg * (1.0 - rk) * 10.0)), mags[i]);
            let mut p = pk.clone();
            p.multiply_opacity(ez(k4) * rk);
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
