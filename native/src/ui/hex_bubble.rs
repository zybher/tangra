//! Typing a colour's hex without opening the colour panel: double-click a colour chip in the
//! properties panel and a small bubble opens under it, its tail pointing at the chip, with the
//! colour on the left and the hex ready to type on the right. Enter or a click elsewhere keeps
//! it (when it's a full six-digit colour), Esc leaves it as it was. It's one of the panel's
//! floating panels (`ui/panel_drop.rs`), so opening it closes the colour panel and the reverse.

use crate::*;

/// One click (after the double-click window has passed: opens the colour panel) or a double
/// click (types the hex). Some(false) = single, Some(true) = double.
pub(crate) fn chip_click(ctx: &egui::Context, id: egui::Id, resp: &egui::Response) -> Option<bool> {
    let pid = id.with("pending");
    let now = ctx.input(|i| i.time);
    if resp.double_clicked() {
        ctx.data_mut(|d| d.remove::<f64>(pid));
        return Some(true);
    }
    if resp.clicked() {
        ctx.data_mut(|d| d.insert_temp(pid, now));
    }
    let t0: Option<f64> = ctx.data(|d| d.get_temp(pid));
    match t0 {
        Some(t0) if now - t0 > 0.3 => {
            ctx.data_mut(|d| d.remove::<f64>(pid));
            Some(false)
        }
        Some(_) => {
            ctx.request_repaint();
            None
        }
        None => None,
    }
}

/// Open the bubble `id` (closes any other floating panel)
pub(crate) fn open(ctx: &egui::Context, id: egui::Id) {
    if !super::panel_drop::is_open(ctx, id) {
        super::panel_drop::toggle(ctx, id);
    }
    ctx.data_mut(|d| d.remove::<String>(id.with("buf")));
}

/// The bubble under `chip` while `id` is open; returns the typed colour ("#rrggbb") when kept
pub(crate) fn show(ctx: &egui::Context, id: egui::Id, chip: Rect, color: &str) -> Option<String> {
    let on = super::panel_drop::is_open(ctx, id);
    let k = ease(ctx, id.with("k"), on);
    if k <= 0.001 {
        return None;
    }
    let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
    let (w, h, tail) = (124.0, 32.0, 6.0);
    let screen = ctx.screen_rect();
    let x = (chip.center().x - w / 2.0).clamp(screen.min.x + 8.0, screen.max.x - w - 8.0);
    let top = chip.max.y + tail + 2.0 - (1.0 - ez(k)) * 4.0;
    let bg = Rect::from_min_size(pos2(x, top), vec2(w, h));
    let bid = id.with("buf");
    let fresh = on && ctx.data(|d| d.get_temp::<String>(bid)).is_none();
    let mut buf: String = ctx.data(|d| d.get_temp(bid)).unwrap_or_else(|| color.trim_start_matches('#').to_uppercase());
    let mut out = None;
    egui::Area::new(id.with("area")).order(egui::Order::Foreground).fixed_pos(bg.min - vec2(0.0, tail)).interactable(on).show(ctx, |ui| {
        ui.set_opacity(ez(k));
        let p = ui.painter().clone().with_clip_rect(screen);
        p.rect_filled(bg.translate(vec2(0.0, 3.0)), 8.0, Color32::from_black_alpha(70));
        glass(&p, bg, 8.0, 1.0);
        p.rect_stroke(bg, 8.0, EStroke::new(1.0_f32, border()));
        // the tail, pointing up at the chip
        let tx = chip.center().x.clamp(bg.min.x + 12.0, bg.max.x - 12.0);
        p.add(egui::Shape::convex_polygon(vec![pos2(tx - tail, bg.min.y + 0.5), pos2(tx, bg.min.y - tail), pos2(tx + tail, bg.min.y + 0.5)], popup(), EStroke::NONE));
        p.line_segment([pos2(tx - tail, bg.min.y), pos2(tx, bg.min.y - tail)], EStroke::new(1.0_f32, border()));
        p.line_segment([pos2(tx, bg.min.y - tail), pos2(tx + tail, bg.min.y)], EStroke::new(1.0_f32, border()));
        // the colour as typed so far (the old one until it's a full colour)
        let clean = buf.trim().trim_start_matches('#').to_string();
        let typed = (clean.len() == 6).then(|| u32::from_str_radix(&clean, 16).ok()).flatten();
        let c = typed.map_or(hex(color), |v| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8));
        let sq = Rect::from_center_size(pos2(bg.min.x + 8.0 + 9.0, bg.center().y), vec2(18.0, 18.0));
        p.rect_filled(sq, 4.0, c);
        p.rect_stroke(sq, 4.0, EStroke::new(1.0_f32, wa(60)));
        let fr = Rect::from_min_max(pos2(sq.max.x + 6.0, bg.min.y + 4.0), pos2(bg.max.x - 4.0, bg.max.y - 4.0));
        if on {
            let ring = super::inline_text::Ring::Fixed(fr, 5.0);
            let res = super::inline_text::edit(ui, id.with("field"), fr.shrink2(vec2(6.0, 0.0)), &mut buf, FontId::proportional(12.5), txt(), fresh, ring);
            buf = buf.chars().filter(|c| c.is_ascii_hexdigit() || *c == '#').take(7).collect();
            match res {
                Some(keep) => {
                    if keep {
                        out = typed.map(|v| format!("#{:06x}", v));
                    }
                    super::panel_drop::toggle(ctx, id);
                }
                None => {}
            }
        } else {
            p.text(fr.shrink2(vec2(6.0, 0.0)).left_center(), Align2::LEFT_CENTER, &buf, FontId::proportional(12.5), txt());
        }
        ui_block(ctx, bg.expand2(vec2(0.0, tail)));
    });
    if on {
        ctx.data_mut(|d| d.insert_temp(bid, buf));
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && super::panel_drop::is_open(ctx, id) {
            super::panel_drop::toggle(ctx, id);
        }
    }
    if k < 1.0 {
        ctx.request_repaint();
    }
    out
}
