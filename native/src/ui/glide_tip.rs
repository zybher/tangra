//! A tooltip shared by a row of small buttons: moving from one button to its neighbour, the
//! bubble slides and stretches over (`micro::glide`) with the new words in it, instead of one
//! tip vanishing and another popping up. It fades in after
//! a short hover and out when the pointer leaves the row.

use crate::*;

/// Call every frame for the group `id` with the hovered button and its tip (or None)
pub(crate) fn show(ctx: &egui::Context, id: egui::Id, hovered: Option<(Rect, &str)>) {
    let now = ctx.input(|i| i.time);
    // a short wait before it first shows, like egui's own tooltips
    let since: Option<f64> = ctx.data(|d| d.get_temp(id.with("since")));
    let since = match (hovered.is_some(), since) {
        (true, None) => {
            ctx.data_mut(|d| d.insert_temp(id.with("since"), now));
            now
        }
        (true, Some(t)) => t,
        (false, _) => {
            ctx.data_mut(|d| d.remove::<f64>(id.with("since")));
            now
        }
    };
    let ready = hovered.is_some() && now - since > 0.35;
    if hovered.is_some() && !ready {
        ctx.request_repaint();
    }
    let k = ease(ctx, id.with("k"), ready);
    if let Some((r, t)) = hovered.filter(|_| ready) {
        ctx.data_mut(|d| d.insert_temp(id.with("v"), (r, t.to_string())));
    }
    if k <= 0.001 {
        return;
    }
    let Some((r, text)) = ctx.data(|d| d.get_temp::<(Rect, String)>(id.with("v"))) else { return };
    let font = FontId::proportional(11.5);
    let w = ctx.fonts(|f| f.layout_no_wrap(text.clone(), font.clone(), txt()).size().x) + 16.0;
    // slides and stretches to the next button (snaps in when it was hidden)
    let snap = k < 0.05;
    let cx = super::micro::glide(ctx, id.with("x"), r.center().x, snap);
    let w = super::micro::glide(ctx, id.with("w"), w, snap);
    let screen = ctx.screen_rect();
    let cx = cx.clamp(screen.min.x + w / 2.0 + 4.0, screen.max.x - w / 2.0 - 4.0);
    let tr = Rect::from_center_size(pos2(cx, r.max.y + 18.0 - (1.0 - k) * 4.0), vec2(w, 23.0));
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, id.with("layer")));
    glass(&p, tr, 6.0, k);
    p.rect_stroke(tr, 6.0, EStroke::new(1.0_f32, border().gamma_multiply(k)));
    p.with_clip_rect(tr).text(tr.center(), Align2::CENTER_CENTER, &text, font, txt().gamma_multiply(k));
}
