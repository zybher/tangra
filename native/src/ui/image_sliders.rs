//! Picture dropdowns from the edit bar (style_bar.rs): Adjust (colour) and 3D (tilt). Same
//! wave list as the style bar's dropdowns, a softer wave since every row is dragged; each
//! value row scrubs sideways like the bars' number fields (double-click resets it). Each panel
//! is only as wide as its labels plus a value; resetting is a round arrow button floating by
//! the panel's top-right corner (shown while there's something to reset). Opens from its button, placed like the bars' other dropdowns
//! (`drop_fit`: wraps into columns or moves the bar rather than cover the picture).

use crate::image_edit::{ImageEdit, ADJUST_ROWS, TILT_ROWS};
use crate::*;

const PITCH: f32 = super::drop_anim::PITCH;
/// Room for a value ("-100") right of the longest label
const VALUE_W: f32 = 40.0;
/// The floating reset button
const RESET_D: f32 = 26.0;
/// Wave spread for these rows (the menus use 1): the hovered row rises fully, neighbours less
const SOFT: f32 = 1.0;

impl App {
    /// Draws the open dropdown (bar's `pop` 6 = Adjust, 7 = 3D, run by the bar's `da`) for picture `img`, under (or
    /// over) `bar`, starting at its button in `btns`. Returns the panel's rect while one is open.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn image_pop(&mut self, ui: &mut egui::Ui, rect: Rect, bar: Rect, btns: [Rect; 2], da: &super::drop_anim::DropAnim, bar_id: egui::Id, pop: u8, img: &str) -> Rect {
        let ctx = ui.ctx().clone();
        let bid = egui::Id::new("qd_img_pop");
        let bar_pop = pop;
        let mut open_rect = Rect::NOTHING;
        let mut any_drag = false;
        // A reset in progress eases the values back (reset_morph.rs); grabbing a row stops it
        let was_drag: bool = ctx.data(|d| d.get_temp(bid.with("dragging"))).unwrap_or(false);
        self.reset_morph_step(&ctx, img, was_drag);
        let ed: ImageEdit = self.board.images.iter().find(|i| i.id == img).and_then(|i| i.edit.clone()).unwrap_or_default();
        for kind in [1u8, 2] {
            let kk = da.k(kind + 5);
            if kk <= 0.001 {
                continue;
            }
            let sliders: &[(u8, &str, f32, f32)] = if kind == 1 { &ADJUST_ROWS } else { &TILT_ROWS };
            let per_px = if kind == 1 { 0.5 } else { 0.3 };
            // The reset button: (tooltip, enabled, which). Restoring lasso cuts is its own button
            // on the bar (style_bar.rs)
            let (rs_tip, rs_on, rs_which) = if kind == 1 { ("Reset adjustments", ed.has_color(), 0u8) } else { ("Reset 3D", ed.tilt_x != 0.0 || ed.tilt_y != 0.0 || ed.depth != 0.0, 2u8) };
            let n = sliders.len();
            let panel_w = super::drop_anim::fit_w(&ctx, 12.0, &sliders.iter().map(|s| s.1).collect::<Vec<_>>()) + VALUE_W;
            // The row being dragged stays raised and the others stay flat until it is let go
            let drag_id = bid.with(("idrag", kind));
            let dragging: Option<usize> = ctx.data(|d| d.get_temp(drag_id)).flatten();
            // Eased like the toolbar menu's pinned rows: letting go plays the rise backwards
            let pinned: Vec<f32> = (0..n).map(|i| ease(&ctx, bid.with(("ipin", kind, i)), dragging == Some(i))).collect();
            // Placed, animated and closed like every bar dropdown (bar_drop.rs)
            let btn = btns[kind as usize - 1];
            let btn_cx = if btn.is_positive() { btn.center().x } else { f32::NAN };
            let wave = super::bar_drop::Wave { pinned: &pinned, soft: SOFT, lock: dragging.is_some() };
            let dd = super::bar_drop::open(ui, da, bar_id, bar_pop, kind + 5, bar, rect, btn_cx, n, panel_w, wave);
            let (bgr, pr, live, pk) = (dd.bgr, dd.pr, dd.live, dd.base.clone());
            // Reset: floats just right of the panel's top corner, fading in while there's
            // something to reset
            let rk = ease(&ctx, bid.with(("ien", rs_which)), rs_on) * kk;
            // It rides the rows' wave: a hovered row popping out past the panel pushes it aside
            let (rl, rr_) = dd.rest_rows().fold((bgr.min.x, bgr.max.x), |a, r| (a.0.min(r.min.x), a.1.max(r.max.x)));
            let mut rb = Rect::from_center_size(pos2(rr_ + 8.0 + RESET_D / 2.0, bgr.min.y + RESET_D / 2.0 + 2.0), vec2(RESET_D, RESET_D));
            // no room right of it at the screen edge: the top-left corner instead
            if bgr.max.x + 8.0 + RESET_D > rect.max.x - 4.0 {
                rb = rb.translate(vec2(rl - 8.0 - RESET_D / 2.0 - rb.center().x, 0.0));
            }
            if live {
                open_rect = if rs_on { pr.union(rb) } else { pr };
            }

            let mut now_drag: Option<usize> = None;
            for i in 0..n {
                let (rr, wm, p) = dd.row(da, i);
                let k = 1.0 + 0.14 * wm;
                {
                    let (f, label, lo, hi) = sliders[i];
                    let mut e = ed.clone();
                    let v = *e.field(f);
                    let resp = ui.interact(rr, bid.with(("islider", f)), if live { Sense::click_and_drag() } else { Sense::hover() });
                    wave_tile(&p, rr, wm, mix(popup(), gray_ui(90), 0.35), 1.0);
                    let col = mix(txt_dim(), txt(), wm);
                    p.text(pos2(rr.min.x + 12.0, rr.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0 * k), col);
                    // Double-click to type it (num_edit.rs)
                    let ne = super::num_edit::NumEdit::run(&ctx, &resp, live, v, lo, hi);
                    if let Some(nv) = ne.apply {
                        self.set_image_edit(f, nv, ne.first);
                    }
                    // The value brightens/dims as it leaves or returns to 0, instead of switching
                    let nz = ease(&ctx, bid.with(("inz", f)), v != 0.0);
                    let vcol = mix(txt_faint(), mix(txt_dim(), txt(), 0.6 + 0.4 * wm), nz);
                    let vcol = mix(vcol, txt(), ne.e);
                    let vr = pos2(rr.max.x - 12.0, rr.center().y);
                    ne.draw(&p, None, vr, 12.0 * k);
                    let txt = ne.text.clone().unwrap_or_else(|| format!("{}", v.round() as i32));
                    p.text(vr, Align2::RIGHT_CENTER, txt, FontId::proportional(12.0 * k), vcol);
                    if live && (resp.hovered() || resp.dragged()) && ne.text.is_none() {
                        ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                    }
                    if live && resp.drag_started() {
                        ctx.data_mut(|d| d.insert_temp(bid.with(("iv0", f)), v));
                    }
                    if live && resp.dragged() {
                        now_drag = Some(i);
                        let v0: f32 = ctx.data(|d| d.get_temp(bid.with(("iv0", f)))).unwrap_or(v);
                        let dx = super::scrub_wrap::dx(&ctx);
                        let nv = (v0 + dx * per_px).round().clamp(lo, hi);
                        if nv != v || resp.drag_started() {
                            self.set_image_edit(f, nv, resp.drag_started());
                        }
                    }
                }
            }
            if rk > 0.01 {
                let resp = ui.interact(rb, bid.with(("ireset", rs_which)), if live && rs_on { Sense::click() } else { Sense::hover() }).on_hover_text(rs_tip);
                let (hh, pp) = hover_fx(&ctx, &resp, rs_on);
                let mut p = pk.clone();
                p.multiply_opacity(rk);
                let br = rb.expand(1.5 * hh - pp);
                p.circle_filled(br.center() + vec2(0.0, 2.0), br.width() / 2.0, Color32::from_black_alpha(60));
                glass(&p, br, br.width() / 2.0, 1.0);
                p.circle_filled(br.center(), br.width() / 2.0, hover_fill(hh, pp));
                p.circle_stroke(br.center(), br.width() / 2.0, EStroke::new(1.0_f32, border()));
                super::icons::reset(&p, br.center(), 14.0, mix(txt_dim(), hi(), hh));
                if live && rs_on && resp.clicked() {
                    match rs_which {
                        0 => self.reset_morph_start(&ctx, img, &[0, 1, 2, 3, 4], &ed),
                        _ => self.reset_morph_start(&ctx, img, &[5, 6, 7, 8], &ed),
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
