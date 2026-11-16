//! The app's own arrow pointer, from the icon pack (svg_icons.rs). Wherever the native cursor
//! would be the plain arrow, it is hidden and this one is painted on top at the pointer.
//! The resize arrows, the frame / shape crosshair, the text I-beam and the grabbing fist are the
//! pack's too, centred on the pointer, with the same glide, trail and press squeeze; the resize
//! arrows don't lean (they keep pointing the way they resize) and the I-beam leans half as much.
//! The pan hand is its own (widgets.rs).
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
    let icon = ctx.output(|o| o.cursor_icon);
    let Some(sh) = shape(icon) else {
        ctx.data_mut(|d| d.remove::<St>(sid));
        return;
    };
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
            let target = MAX_TILT * sh.lean * (vx / TILT_SPEED).tanh();
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
    // Faint trail from the hot spot: thins and fades with age
    let life = (TRAIL_LIFE * ak as f64).max(1e-3);
    for w in st.trail.windows(2) {
        let ((a, _), (b, tb)) = (w[0], w[1]);
        let f = (1.0 - ((now - tb) / life) as f32).clamp(0.0, 1.0);
        if f > 0.01 {
            p.line_segment([a, b], egui::Stroke::new(0.8 + 1.6 * f, Color32::WHITE.gamma_multiply(TRAIL_ALPHA * f * f)));
        }
    }
    let (pos, tilt) = (st.pos, st.tilt);
    ctx.data_mut(|d| d.insert_temp(sid, st));
    let press = ease(ctx, egui::Id::new("qd_cursor_press"), ctx.input(|i| i.pointer.primary_down()));
    let ink = Color32::from_rgb(18, 18, 20);
    crate::ui::svg_icons::draw_cursor(&p, sh.name, pos, sh.hot, sh.px * (1.0 - (1.0 - PRESS_SCALE) * press), tilt, Color32::WHITE, ink);
    // The drawing tool's own toolbar icon rides beside the crosshair, so each tool reads apart
    let pass = ctx.cumulative_pass_nr();
    let badge = ctx.data(|d| d.get_temp::<(&'static str, u64)>(badge_id())).filter(|(_, f)| *f == pass && sh.name == "crosshair").map(|(n, _)| n);
    let bk = ease(ctx, badge_id().with("k"), badge.is_some());
    if let Some(name) = badge {
        ctx.data_mut(|d| d.insert_temp(badge_id().with("last"), name));
    }
    let name = badge.or_else(|| ctx.data(|d| d.get_temp::<&'static str>(badge_id().with("last"))));
    if let (Some(name), true) = (name, bk > 0.01) {
        let (sn, cs) = tilt.sin_cos();
        let o = vec2(15.0, 15.0);
        let c = pos + vec2(o.x * cs - o.y * sn, o.x * sn + o.y * cs);
        let r = 9.5 * (0.7 + 0.3 * bk) * (1.0 - (1.0 - PRESS_SCALE) * press);
        p.circle_filled(c + vec2(0.0, 1.0), r + 1.0, Color32::from_black_alpha((50.0 * bk) as u8));
        p.circle_filled(c, r, ink.gamma_multiply(0.92 * bk));
        p.circle_stroke(c, r, egui::Stroke::new(1.0, Color32::from_white_alpha((40.0 * bk) as u8)));
        crate::ui::svg_icons::draw(&p, name, c, r * 1.25, Color32::WHITE.gamma_multiply(bk));
    }
}

fn badge_id() -> egui::Id {
    egui::Id::new("qd_cursor_badge")
}

/// The canvas names the drawing tool's icon (pack name) for the crosshair badge, every frame
pub(crate) fn set_badge(ctx: &egui::Context, name: &'static str) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(badge_id(), (name, pass)));
}

struct Shape {
    name: &'static str,
    /// The point of the icon's 24-unit box that sits on the pointer
    hot: [f32; 2],
    px: f32,
    /// How much of the lean it takes: resize arrows keep their direction, the I-beam leans less
    lean: f32,
}

/// The pack icon drawn for a native cursor (None = leave it native)
fn shape(icon: egui::CursorIcon) -> Option<Shape> {
    use egui::CursorIcon as C;
    let (name, px, lean) = match icon {
        // The arrow's tip (5.5, 3.5 in the icon's box) sits on the pointer
        C::Default => return Some(Shape { name: "cursor", hot: [5.5, 3.5], px: 26.0, lean: 1.0 }),
        C::ResizeHorizontal | C::ResizeEast | C::ResizeWest | C::ResizeColumn => ("resize-h", 30.0, 0.0),
        C::ResizeVertical | C::ResizeNorth | C::ResizeSouth | C::ResizeRow => ("resize-v", 30.0, 0.0),
        C::ResizeNwSe | C::ResizeNorthWest | C::ResizeSouthEast => ("resize-nwse", 30.0, 0.0),
        C::ResizeNeSw | C::ResizeNorthEast | C::ResizeSouthWest => ("resize-nesw", 30.0, 0.0),
        C::Crosshair => ("crosshair", 32.0, 1.0),
        C::Text => ("text-cursor", 30.0, 0.5),
        C::Grabbing => ("grabbing", 30.0, 1.0),
        _ => return None,
    };
    Some(Shape { name, hot: [12.0, 12.0], px, lean })
}
