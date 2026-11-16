//! The app's own arrow pointer, from the icon pack (svg_icons.rs). Wherever the native cursor
//! would be the plain arrow, it is hidden and this one is painted on top at the pointer.
//! Other cursors (text, resize, crosshair, ...) stay native; the pan hand is its own (widgets.rs).
//!
//! Motion: the arrow follows the pointer with a very short glide (~25 ms, so clicks still land
//! where it points) and leans into sideways movement, tipping right when moving right and left
//! when moving left, then settling upright. It turns around its tip, so the click point never
//! moves; it leans in quickly and comes back upright more slowly. Pressing squeezes it a little.
//! A faint trail fades out behind the tip. All of it turns off with the app's
//! animations (anim_k = 0).
use crate::*;

/// How fast the drawn arrow catches the pointer (1/s): higher = tighter
const FOLLOW: f32 = 40.0;
/// Largest lean, radians (~31°)
const MAX_TILT: f32 = 0.55;
/// Sideways speed (px/s) that gives about 3/4 of the lean
const TILT_SPEED: f32 = 650.0;
/// How fast it leans in (1/s), and how much slower it comes back upright
const TILT_IN: f32 = 16.0;
const TILT_OUT: f32 = 6.5;
/// Pressing squeezes the arrow to this size (eased like every other press)
const PRESS_SCALE: f32 = 0.86;
/// How long the faint trail behind the tip lasts (s) and its strongest alpha
const TRAIL_LIFE: f64 = 0.12;
const TRAIL_ALPHA: f32 = 0.22;

#[derive(Clone)]
struct St {
    pos: Pos2,
    tilt: f32,
    /// Recent tip positions (newest last) for the trail
    trail: Vec<(Pos2, f64)>,
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
    let now = ctx.input(|i| i.time);
    let st = match ctx.data(|d| d.get_temp::<St>(sid)) {
        Some(s) if ak > 0.0 => {
            let pos = s.pos + (at - s.pos) * (1.0 - (-dt * FOLLOW / ak).exp());
            let vx = (pos.x - s.pos.x) / dt;
            let target = MAX_TILT * (vx / TILT_SPEED).tanh();
            // Quick to lean in, unhurried coming back
            let rate = if target.abs() > s.tilt.abs() && target * s.tilt >= 0.0 { TILT_IN } else { TILT_OUT };
            let tilt = s.tilt + (target - s.tilt) * (1.0 - (-dt * rate / ak).exp());
            let mut trail = s.trail;
            trail.retain(|(_, t)| now - t < TRAIL_LIFE * ak as f64);
            if trail.last().map_or(true, |(q, _)| (*q - pos).length() > 0.5) {
                trail.push((pos, now));
            }
            St { pos, tilt, trail }
        }
        _ => St { pos: at, tilt: 0.0, trail: Vec::new() },
    };
    if (st.pos - at).length() > 0.2 || st.tilt.abs() > 0.002 || st.trail.len() > 1 {
        ctx.request_repaint();
    }
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Debug, egui::Id::new("qd_cursor")));
    // Faint trail from the tip: thins and fades with age
    let life = (TRAIL_LIFE * ak as f64).max(1e-3);
    for w in st.trail.windows(2) {
        let ((a, _), (b, tb)) = (w[0], w[1]);
        let f = (1.0 - ((now - tb) / life) as f32).clamp(0.0, 1.0);
        if f > 0.01 {
            p.line_segment([a, b], egui::Stroke::new(0.8 + 1.6 * f, Color32::WHITE.gamma_multiply(TRAIL_ALPHA * f * f)));
        }
    }
    if let Some((a, _)) = st.trail.last() {
        p.line_segment([*a, st.pos], egui::Stroke::new(2.4, Color32::WHITE.gamma_multiply(TRAIL_ALPHA)));
    }
    let (pos, tilt) = (st.pos, st.tilt);
    ctx.data_mut(|d| d.insert_temp(sid, st));
    // The arrow's tip (5.5, 3.5 in the icon's box) sits on the pointer
    let press = ease(ctx, egui::Id::new("qd_cursor_press"), ctx.input(|i| i.pointer.primary_down()));
    crate::ui::svg_icons::draw_cursor(&p, "cursor", pos, [5.5, 3.5], 26.0 * (1.0 - (1.0 - PRESS_SCALE) * press), tilt, Color32::WHITE, Color32::from_rgb(18, 18, 20));
}
