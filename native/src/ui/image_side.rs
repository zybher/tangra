//! Picture side bar: a small upright pill beside a selected picture (right, else left) with
//! the Adjust and 3D buttons; their dropdown opens next to it, so nothing covers the picture.
//! Same pill, hover and lit-while-open look as the placement bar. Publishes its area so the
//! height pill folds into the width pill instead of sitting under it.

use crate::*;

const BW: f32 = 34.0;
const BTN: f32 = 28.0;

impl App {
    /// `fr`: the picture on screen. `show` false lets it fade out.
    pub(crate) fn image_side_ui(&mut self, ui: &mut egui::Ui, rect: Rect, fr: Rect, img: &str, show: bool) {
        let ctx = ui.ctx().clone();
        let bid = egui::Id::new("qd_img_side");
        let a = ease(&ctx, bid.with("a"), show);
        let mut pop: u8 = ctx.data(|d| d.get_temp(bid.with("pop"))).unwrap_or(0);
        let last: String = ctx.data(|d| d.get_temp(bid.with("img"))).unwrap_or_default();
        if !show || last != img {
            pop = 0;
        }
        ctx.data_mut(|d| d.insert_temp(bid.with("img"), img.to_string()));
        if a <= 0.01 {
            ctx.data_mut(|d| d.insert_temp(bid.with("pop"), 0u8));
            return;
        }
        let bh = 4.0 * 2.0 + BTN * 2.0;
        let left_edge = rect.min.x + crate::ui::outline::hud_shift(&ctx) + 8.0;
        // Right of the picture when the bar and its dropdown fit there, else left
        let right = fr.max.x + 10.0 + BW + 8.0 + super::image_sliders::PANEL_W <= rect.max.x - 8.0 || fr.min.x - 10.0 - BW < left_edge;
        let x = if right { fr.max.x + 10.0 } else { fr.min.x - 10.0 - BW };
        let y = fr.min.y.clamp(rect.min.y + 8.0, (rect.max.y - 70.0 - bh).max(rect.min.y + 8.0));
        let bar = Rect::from_min_size(pos2(x, y + (1.0 - a) * 6.0), vec2(BW, bh));
        let p = ui.painter().clone();
        p.rect_filled(bar.translate(vec2(0.0, 3.0)), BW / 2.0, Color32::from_black_alpha((60.0 * a) as u8));
        p.rect_filled(bar, BW / 2.0, frame_bg().gamma_multiply(a));
        p.rect_stroke(bar, BW / 2.0, EStroke::new(1.0_f32, border().gamma_multiply(a)));
        if show {
            ui.interact(bar, bid.with("bg"), Sense::click_and_drag());
        }
        for (i, kind) in [1u8, 2].into_iter().enumerate() {
            let r = Rect::from_center_size(pos2(bar.center().x, bar.min.y + 4.0 + BTN / 2.0 + i as f32 * BTN), vec2(BTN, BTN - 2.0));
            let resp = ui.interact(r, bid.with(("b", kind)), if show { Sense::click() } else { Sense::hover() });
            let (hh, pp) = hover_fx(&ctx, &resp, show);
            let hh = if pop == kind { hh.max(1.0) } else { hh };
            p.rect_filled(r, 8.0, hover_fill(hh, pp).gamma_multiply(a));
            let col = mix(TEXT_DIM, TEXT, hh).gamma_multiply(a);
            if kind == 1 {
                super::icons::adjust(&p, r.center(), 16.0, col);
            } else {
                super::icons::cube(&p, r.center(), 16.0, col);
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
        if a < 1.0 {
            ctx.request_repaint();
        }
    }
}
