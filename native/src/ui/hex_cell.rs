//! A colour cell like Figma's: its square (click: the colour panel, at once) and its hex, typed
//! in place on a click (Enter or a click away keeps it when it's a full six-digit colour, Esc
//! leaves it). The properties card's solid Fill / Stroke rows and the paint panel's stop rows.
use crate::*;

pub(crate) struct Cell {
    /// The square was clicked
    pub square: bool,
    /// Typing the hex just started
    pub typing: bool,
    /// The typed colour ("#rrggbb") when kept
    pub typed: Option<String>,
}

/// The cell in `r`. `open_k`: its colour panel is open (the square stays lit); `ring`: the
/// square's accent ring (0..1)
pub(crate) fn show(ui: &mut egui::Ui, id: egui::Id, r: Rect, color: &str, open_k: f32, ring: f32, a: f32) -> Cell {
    let ctx = ui.ctx().clone();
    let none = color == "none";
    let sq_r = Rect::from_min_size(r.min, vec2(28.0, r.height()));
    let hx_r = Rect::from_min_max(pos2(sq_r.max.x - 2.0, r.min.y), r.max);
    let hx_id = id.with("hx");
    let editing: Option<String> = ctx.data(|d| d.get_temp(hx_id));
    let resp = ui.interact(sq_r, id.with("sq"), Sense::click());
    let hresp = ui.interact(hx_r, id.with("hxr"), Sense::click());
    if hresp.hovered() && editing.is_none() && !none {
        ctx.set_cursor_icon(egui::CursorIcon::Text);
    }
    let (hh, pp) = hover_fx(&ctx, &resp, true);
    let (xh, xp) = hover_fx(&ctx, &hresp, editing.is_none() && !none);
    let p = ui.painter().clone();
    p.rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
    p.rect_filled(r, 6.0, hover_fill(xh * 0.6, xp).gamma_multiply(a));
    p.rect_filled(sq_r.shrink(2.0), 5.0, hover_fill(hh.max(open_k * 0.6), pp).gamma_multiply(a));
    if ring > 0.01 {
        p.rect_stroke(sq_r.shrink(2.5), 5.0, EStroke::new(1.0_f32, blue().gamma_multiply(ring * a)));
    }
    let mut out = Cell { square: resp.clicked(), typing: false, typed: None };
    match editing {
        None => {
            super::paint_panel::color_chip(&ctx, &p, id.with("cc"), r, color, hh.max(open_k).max(xh), true, a);
            if hresp.clicked() && !none {
                out.typing = true;
                ctx.data_mut(|d| {
                    d.insert_temp(hx_id, color.trim_start_matches('#').to_uppercase());
                    d.insert_temp(hx_id.with("fresh"), true);
                });
            }
        }
        Some(mut buf) => {
            super::paint_panel::color_chip(&ctx, &p, id.with("cc"), sq_r, color, hh.max(open_k), false, a);
            let fresh = ctx.data_mut(|d| d.remove_temp::<bool>(hx_id.with("fresh"))).unwrap_or(false);
            let ring = super::inline_text::Ring::Fixed(hx_r.shrink2(vec2(0.0, 2.0)), 5.0);
            let res = super::inline_text::edit(ui, hx_id.with("ed"), Rect::from_min_max(pos2(r.min.x + 28.0, r.min.y), r.max), &mut buf, FontId::proportional(12.0), txt(), fresh, ring);
            buf = buf.chars().filter(|c| c.is_ascii_hexdigit() || *c == '#').take(7).collect();
            match res {
                Some(keep) => {
                    let clean = buf.trim().trim_start_matches('#').to_string();
                    if keep && clean.len() == 6 && u32::from_str_radix(&clean, 16).is_ok() {
                        out.typed = Some(format!("#{}", clean.to_lowercase()));
                    }
                    ctx.data_mut(|d| d.remove::<String>(hx_id));
                }
                None => ctx.data_mut(|d| d.insert_temp(hx_id, buf)),
            }
        }
    }
    out
}
