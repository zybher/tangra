//! Responsive UI size: everything (toolbars, dropdowns, panels) grows with the window so a
//! maximised window on a big screen doesn't leave the chrome tiny. The small default window
//! keeps its 1:1 look.

/// Window width (in the OS's logical points) at which the UI is drawn 1:1
const BASE_W: f32 = 1300.0;
const MAX: f32 = 1.45;

use eframe::egui;

pub(crate) fn update_ui_scale(ctx: &egui::Context) {
    // egui's own Ctrl+0 / Ctrl+plus / Ctrl+minus would fight this scale
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    // The window in UI points times the current zoom = its width in the OS's own points,
    // which doesn't change when we change the zoom (no feedback loop)
    let os_w = ctx.screen_rect().width() * ctx.zoom_factor();
    let target = ((os_w / BASE_W).clamp(1.0, MAX) * 20.0).round() / 20.0;
    if (ctx.zoom_factor() - target).abs() > 0.01 {
        ctx.set_zoom_factor(target);
    }
}
