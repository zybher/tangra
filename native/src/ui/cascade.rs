//! The bar dropdowns' row cascade (drop_anim.rs) for panels whose insides are laid out freely
//! (the paint panel and its colour panel): everything painted into the panel's layer after its
//! background drops in from 16 px above and fades, top first, one band after another; closing
//! plays it backwards. Works on the painted shapes, so the panel's own code stays as it is.
use crate::*;

/// The panel's cascade progress (0..1 over 0.45 s, like `DropAnim`'s). Call every frame, also
/// while the panel is hidden, so it starts from 0 on the next open
pub(crate) fn progress(ctx: &egui::Context, id: egui::Id, open: bool) -> f32 {
    ctx.animate_bool_with_time(id.with("cascade"), open, 0.45 * anim_k())
}

/// How many shapes `layer` holds so far (the cascade moves the ones painted after this)
pub(crate) fn mark(ctx: &egui::Context, layer: egui::LayerId) -> usize {
    ctx.graphics(|g| g.get(layer).map_or(0, |l| l.next_idx().0))
}

/// Moves and fades the shapes `layer` got since `from`, by how far down the panel (`top`,
/// `h` tall) each one sits, at cascade progress `t`
pub(crate) fn apply(ctx: &egui::Context, layer: egui::LayerId, from: usize, top: f32, h: f32, t: f32) {
    if t >= 0.999 {
        return;
    }
    ctx.request_repaint();
    let h = h.max(1.0);
    ctx.graphics_mut(|g| {
        let list = g.entry(layer);
        let end = list.next_idx().0;
        for i in from..end {
            let idx = egui::layers::ShapeIdx(i);
            list.mutate_shape(idx, |cs| {
                let b = cs.shape.visual_bounding_rect();
                let y = if b.is_positive() { b.center().y } else { top };
                let frac = ((y - top) / h).clamp(0.0, 1.0);
                // the same ease-out rise as a dropdown row, the lowest starting a little later
                let e = (t * 1.8 - frac * 0.8).clamp(0.0, 1.0);
                let k = 1.0 - (1.0 - e).powi(3);
                // from 16 px above, like a dropdown hanging under its bar (its clip stays put,
                // so nothing shows outside the panel on the way in)
                let dy = -(1.0 - k) * 16.0;
                cs.shape.transform(egui::emath::TSTransform::from_translation(vec2(0.0, dy)));
                egui::epaint::shape_transform::adjust_colors(&mut cs.shape, move |c| {
                    if *c != Color32::PLACEHOLDER {
                        *c = c.gamma_multiply(k);
                    }
                });
            });
        }
    });
}
