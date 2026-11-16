//! The app's own arrow pointer, from the icon pack (svg_icons.rs). Wherever the native cursor
//! would be the plain arrow, it is hidden and this one is painted on top at the pointer.
//! Other cursors (text, resize, crosshair, ...) stay native; the pan hand is its own (widgets.rs).
//!
//! Motion: the arrow follows the pointer with a very short glide (~25 ms, so clicks still land
//! where it points) and leans into sideways movement, tipping right when moving right and left
//! when moving left, then settling upright. It turns around its tip, so the click point never
//! moves. Both turn off with the app's animations (anim_k = 0).
use crate::*;

/// How fast the drawn arrow catches the pointer (1/s): higher = tighter
const FOLLOW: f32 = 40.0;
/// Largest lean, radians (~14°)
const MAX_TILT: f32 = 0.25;
/// Sideways speed (px/s) that gives about 3/4 of the lean
const TILT_SPEED: f32 = 1400.0;
/// How fast the lean follows the speed (1/s)
const TILT_EASE: f32 = 14.0;

#[derive(Clone, Copy)]
struct St {
    pos: Pos2,
    tilt: f32,
}

/// Call last in the frame, after every widget has picked its cursor
pub(crate) fn paint(ctx: &egui::Context) {
    let sid = egui::Id::new("qd_cursor_st");
    if ctx.output(|o| o.cursor_icon) != egui::CursorIcon::Default {
        ctx.data_mut(|d| d.remove::<St>(sid));
        return;
    }
    let at = match ctx.pointer_hover_pos() {
        Some(at) if ctx.input(|i| i.pointer.has_pointer()) => at,
        _ => {
            // Left the window: come back exactly where the pointer re-enters
            ctx.data_mut(|d| d.remove::<St>(sid));
            return;
        }
    };
    ctx.set_cursor_icon(egui::CursorIcon::None);
    let dt = ctx.input(|i| i.stable_dt).clamp(1e-4, 0.05);
    let ak = anim_k();
    let st = match ctx.data(|d| d.get_temp::<St>(sid)) {
        Some(s) if ak > 0.0 => {
            let pos = s.pos + (at - s.pos) * (1.0 - (-dt * FOLLOW / ak).exp());
            let vx = (pos.x - s.pos.x) / dt;
            let target = MAX_TILT * (vx / TILT_SPEED).tanh();
            let tilt = s.tilt + (target - s.tilt) * (1.0 - (-dt * TILT_EASE / ak).exp());
            St { pos, tilt }
        }
        _ => St { pos: at, tilt: 0.0 },
    };
    ctx.data_mut(|d| d.insert_temp(sid, st));
    if (st.pos - at).length() > 0.2 || st.tilt.abs() > 0.002 {
        ctx.request_repaint();
    }
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Debug, egui::Id::new("qd_cursor")));
    // The arrow's tip (5.5, 3.5 in the icon's box) sits on the pointer
    crate::ui::svg_icons::draw_cursor(&p, "cursor", st.pos, [5.5, 3.5], 26.0, st.tilt, Color32::WHITE, Color32::from_rgb(18, 18, 20));
}
