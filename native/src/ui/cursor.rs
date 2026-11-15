//! The app's own arrow pointer, from the icon pack (svg_icons.rs). Wherever the native cursor
//! would be the plain arrow, it is hidden and this one is painted on top at the pointer.
//! Other cursors (text, resize, crosshair, ...) stay native; the pan hand is its own (widgets.rs).
use crate::*;

/// Call last in the frame, after every widget has picked its cursor
pub(crate) fn paint(ctx: &egui::Context) {
    if ctx.output(|o| o.cursor_icon) != egui::CursorIcon::Default {
        return;
    }
    let Some(at) = ctx.pointer_hover_pos() else { return };
    if !ctx.input(|i| i.pointer.has_pointer()) {
        return;
    }
    ctx.set_cursor_icon(egui::CursorIcon::None);
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Debug, egui::Id::new("qd_cursor")));
    // The arrow's tip (5.5, 3.5 in the icon's box) sits on the pointer
    crate::ui::svg_icons::draw_cursor(&p, "cursor", at, [5.5, 3.5], 26.0, Color32::WHITE, Color32::from_rgb(18, 18, 20));
}
