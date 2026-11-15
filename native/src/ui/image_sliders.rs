//! Picture dropdowns from the placement bar (place_bar.rs): Adjust (colour) and 3D (tilt). Same
//! wave list as the style bar's dropdowns, a softer wave since every row is dragged; each
//! value row scrubs sideways like the bars' number fields (double-click resets it), and a
//! reset row sits at the bottom. Opens under (or over) the bar, from its button.

use crate::image_edit::{ImageEdit, ADJUST_ROWS, TILT_ROWS};
use crate::*;

const PITCH: f32 = 26.0;
pub(crate) const PANEL_W: f32 = 168.0;
/// Wave spread for these rows (the menus use 1): the hovered row rises fully, neighbours less
const SOFT: f32 = 0.55;

impl App {
    /// Draws the open dropdown (bar's `pop` 2 = Adjust, 3 = 3D) for picture `img`, under (or
    /// over) `bar`, starting at its button in `btns`. Returns the panel's rect while one is open.
    pub(crate) fn image_pop(&mut self, ui: &mut egui::Ui, rect: Rect, bar: Rect, btns: [Rect; 2], pop: u8, img: &str) -> Rect {
        let pop = pop.saturating_sub(1);
        let ctx = ui.ctx().clone();
        let bid = egui::Id::new("qd_img_pop");
        let ed: ImageEdit = self.board.images.iter().find(|i| i.id == img).and_then(|i| i.edit.clone()).unwrap_or_default();
        let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
        let mut open_rect = Rect::NOTHING;
        let mut any_drag = false;
        for kind in [1u8, 2] {
            let kk = ctx.animate_bool_with_time(bid.with(("ipop", kind)), pop == kind, 0.22 * anim_k());
            if kk <= 0.001 {
                continue;
            }
            let sliders: &[(u8, &str, f32, f32)] = if kind == 1 { &ADJUST_ROWS } else { &TILT_ROWS };
            let per_px = if kind == 1 { 0.5 } else { 0.3 };
            // Reset rows: (label, enabled, which)
            let mut resets: Vec<(&str, bool, u8)> = Vec::new();
            if kind == 1 {
                resets.push(("Reset adjustments", ed.has_color(), 0));
                if !ed.cuts.is_empty() {
                    resets.push(("Restore cut areas", true, 1));
                }
            } else {
                resets.push(("Reset 3D", ed.has_tilt() || ed.depth != 0.0, 2));
            }
            let n = sliders.len() + resets.len();
            let ph = n as f32 * PITCH + 8.0;

            // Keep the last spot while it fades out (the button may already be gone)
            let btn = btns[kind as usize - 1];
            let xid = bid.with(("ix", kind));
            let left = if btn.is_positive() { (btn.min.x - 4.0).min(rect.max.x - 8.0 - PANEL_W) } else { ctx.data(|d| d.get_temp(xid)).unwrap_or(bar.min.x) };
            ctx.data_mut(|d| d.insert_temp(xid, left));
            let up = bar.max.y + 6.0 + ph > rect.max.y - 70.0;
            let sg = if up { -1.0 } else { 1.0 };
            let top = if up { bar.min.y - 6.0 - ph } else { bar.max.y + 6.0 } + (1.0 - ez(kk)) * -8.0 * sg;

            // The row being dragged stays raised and the others stay flat until it is let go
            let drag_id = bid.with(("idrag", kind));
            let dragging: Option<usize> = ctx.data(|d| d.get_temp(drag_id)).flatten();
            let pinned: Vec<f32> = (0..n).map(|i| if dragging == Some(i) { 1.0 } else { 0.0 }).collect();
            let (rows, mags, pr) = wave_rows_k(&ctx, bid.with(("iwave", kind)), left, top + ph, PANEL_W, PITCH, n, &pinned, SOFT, dragging.is_some());
            let dy = top - pr.min.y;
            let pr = pr.translate(vec2(0.0, dy));
            let live = pop == kind;
            if live {
                open_rect = pr;
                ui.interact(pr, bid.with(("ipanel", kind)), Sense::click());
            }
            let pk = ui.painter().clone().with_clip_rect(ctx.screen_rect());
            let mut p = pk.clone();
            p.multiply_opacity(ez(kk));
            p.rect_filled(pr.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
            glass(&p, pr, 10.0, 1.0);
            p.rect_stroke(pr, 10.0, EStroke::new(1.0_f32, border()));

            let mut now_drag: Option<usize> = None;
            for i in 0..n {
                let rk = ez(kk * 1.5 - i as f32 * 0.12);
                let (rr, wm) = (rows[i].translate(vec2(0.0, dy - sg * (1.0 - rk) * 10.0)), mags[i]);
                let mut p = pk.clone();
                p.multiply_opacity(ez(kk) * rk);
                let k = 1.0 + 0.14 * wm;
                if let Some(&(f, label, lo, hi)) = sliders.get(i) {
                    let mut e = ed.clone();
                    let v = *e.field(f);
                    let resp = ui.interact(rr, bid.with(("islider", f)), if live { Sense::click_and_drag() } else { Sense::hover() });
                    wave_tile(&p, rr, wm, mix(popup(), Color32::from_gray(90), 0.35), 1.0);
                    let col = mix(TEXT_DIM, TEXT, wm);
                    p.text(pos2(rr.min.x + 12.0, rr.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0 * k), col);
                    let vcol = if v != 0.0 { mix(TEXT_DIM, TEXT, 0.6 + 0.4 * wm) } else { TEXT_FAINT };
                    p.text(pos2(rr.max.x - 12.0, rr.center().y), Align2::RIGHT_CENTER, format!("{}", v.round() as i32), FontId::proportional(12.0 * k), vcol);
                    if live && (resp.hovered() || resp.dragged()) {
                        ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                    }
                    if live && resp.double_clicked() {
                        self.set_image_edit(f, 0.0, true);
                    }
                    if live && resp.drag_started() {
                        ctx.data_mut(|d| d.insert_temp(bid.with(("iv0", f)), v));
                    }
                    if live && resp.dragged() {
                        now_drag = Some(i);
                        let v0: f32 = ctx.data(|d| d.get_temp(bid.with(("iv0", f)))).unwrap_or(v);
                        let dx = ctx.input(|i| i.pointer.press_origin().zip(i.pointer.interact_pos()).map_or(0.0, |(a, b)| b.x - a.x));
                        let nv = (v0 + dx * per_px).round().clamp(lo, hi);
                        if nv != v || resp.drag_started() {
                            self.set_image_edit(f, nv, resp.drag_started());
                        }
                    }
                } else {
                    let (label, enabled, which) = resets[i - sliders.len()];
                    let resp = ui.interact(rr, bid.with(("ireset", which)), if live { Sense::click() } else { Sense::hover() });
                    let wm = if enabled { wm } else { 0.0 };
                    wave_tile(&p, rr, wm, mix(popup(), Color32::from_gray(90), 0.35), 1.0);
                    let col = if enabled { mix(TEXT_DIM, TEXT, wm) } else { TEXT_FAINT };
                    p.text(pos2(rr.min.x + 12.0, rr.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0 * k), col);
                    if live && enabled && resp.clicked() {
                        match which {
                            0 => self.reset_adjust(),
                            1 => self.restore_cuts(),
                            _ => self.reset_tilt(),
                        }
                    }
                }
            }
            any_drag |= now_drag.is_some();
            ctx.data_mut(|d| d.insert_temp(drag_id, now_drag));
            if kk < 1.0 {
                ctx.request_repaint();
            }
        }
        // The bar holds its magnify still while a row is dragged
        ctx.data_mut(|d| d.insert_temp(bid.with("dragging"), any_drag));
        open_rect
    }
}
