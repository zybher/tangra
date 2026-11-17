//! The Tangra mark at the top left of the title bar, ahead of the tabs. It sits still and
//! plays a little easter egg now and then while hovered (ui/logo_anim.rs); hovering spreads its pieces,
//! clicking makes it hop. Idle it costs nothing: no wake-ups at all.
use super::logo_anim::{LogoAnim, COLORS};
use crate::*;
use std::sync::Mutex;

/// The animation and the clock time it was last stepped to
static ANIM: Mutex<Option<(LogoAnim, f64)>> = Mutex::new(None);

/// Width the mark takes before the first tab
pub(crate) const LOGO_W: f32 = 44.0;

/// Draws the mark centred at `c`, `size` px across
pub(crate) fn title_logo(ui: &mut egui::Ui, c: Pos2, size: f32) {
    let ctx = ui.ctx().clone();
    let r = Rect::from_center_size(c, vec2(size + 8.0, size + 8.0));
    let resp = ui.allocate_rect(r, Sense::click());
    let now = ctx.input(|i| i.time);
    let mut g = ANIM.lock().unwrap();
    let (a, last) = g.get_or_insert_with(|| (LogoAnim::new((now.to_bits() ^ 0x9E37_79B9_7F4A_7C15) | 1), now));
    a.reduce_motion = anim_k() == 0.0;
    a.set_hover(resp.hovered());
    // easter eggs only while the pointer rests on it, so an idle app never wakes up
    a.eggs_enabled = resp.hovered();
    if resp.clicked() {
        a.click();
    }
    // catch up on time spent idle (no frames drawn) in steps the animation accepts
    let mut dt = (now - *last).max(0.0) as f32;
    *last = now;
    while dt > 0.0 {
        let step = dt.min(0.1);
        a.update(step);
        dt -= step;
    }
    let f = size / 80.0;
    let q = |(x, y): (f32, f32)| c + vec2((x - 60.0) * f, (y - 60.0) * f);
    let p = ui.painter();
    for (tri, col) in a.triangle_points().iter().zip(COLORS) {
        p.add(Shape::convex_polygon(tri.iter().map(|v| q(*v)).collect(), Color32::from_rgb(col[0], col[1], col[2]), EStroke::NONE));
    }
    if a.is_animating() {
        ctx.request_repaint();
    } else if let Some(s) = a.seconds_until_next_event() {
        ctx.request_repaint_after(std::time::Duration::from_secs_f32(s + 0.01));
    }
}
