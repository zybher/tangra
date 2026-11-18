//! Dropdowns in the properties panel (Blend, a fill's gradient type…). The row shows the
//! current choice; a click opens a floating list over everything (the panel doesn't grow).
//! One choice a row, top to bottom; past 10 the list scrolls. It opens under its row (over it
//! when there's no room below), fades / slides in with its rows cascading like the edit bar's
//! dropdowns, and closes on a pick, a click elsewhere or Esc. Only one is open at a time.

use super::drop_anim::PITCH;
use crate::*;

/// Rows shown before the list scrolls
const MAX_ROWS: usize = 10;

fn open_now(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.data(|d| d.get_temp::<Option<egui::Id>>(egui::Id::new("qd_pdrop_open"))).flatten()
}

fn set_open(ctx: &egui::Context, id: Option<egui::Id>) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_pdrop_open"), id));
}

/// A dropdown row in `r` (`label` on the left, the current choice on the right; an empty
/// label puts the choice on the left), with an icon (`ui/svg_icons.rs`) beside each choice in
/// the list and the row. Returns the newly picked index.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dropdown_icons(ui: &mut egui::Ui, id: egui::Id, r: Rect, label: &str, names: &[&str], icons: &[&'static str], cur: usize, a: f32) -> Option<usize> {
    dropdown_ex(ui, id, r, label, names, icons, cur, &[], a).0
}

/// `dropdown` with small icon buttons (icon, tooltip) that show on the row's right while it's
/// hovered; also returns which of them was clicked
#[allow(clippy::too_many_arguments)]
pub(crate) fn dropdown_ex(ui: &mut egui::Ui, id: egui::Id, r: Rect, label: &str, names: &[&str], icons: &[&'static str], cur: usize, extras: &[(&'static str, &str)], a: f32) -> (Option<usize>, Option<usize>) {
    let ctx = ui.ctx().clone();
    let open = open_now(&ctx) == Some(id);
    // only the chevron opens it (the row itself stays still under the pointer)
    let cb = Rect::from_center_size(pos2(r.max.x - 13.0, r.center().y), vec2(22.0, r.height() - 4.0));
    let resp = ui.interact(cb, id, Sense::click());
    let (hh, pp) = hover_fx(&ctx, &resp, true);
    let p = ui.painter();
    p.rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
    p.rect_filled(cb, 5.0, hover_fill(hh.max(if open { 0.6 } else { 0.0 }), pp).gamma_multiply(a));
    let name = names.get(cur).copied().unwrap_or("");
    let cy = r.center().y;
    let icon = icons.get(cur).copied();
    let ncol = mix(gray_ui(225), hi(), hh).gamma_multiply(a);
    if label.is_empty() {
        let tx = r.min.x + if icon.is_some() { 30.0 } else { 8.0 };
        if let Some(ic) = icon {
            super::svg_icons::draw(p, ic, pos2(r.min.x + 16.0, cy), 15.0, ncol);
        }
        p.text(pos2(tx, cy), Align2::LEFT_CENTER, name, FontId::proportional(12.0), ncol);
    } else {
        p.text(pos2(r.min.x + 8.0, cy), Align2::LEFT_CENTER, label, FontId::proportional(11.5), txt_dim().gamma_multiply(a));
        let nr = p.text(pos2(r.max.x - 24.0, cy), Align2::RIGHT_CENTER, name, FontId::proportional(12.0), ncol);
        if let Some(ic) = icon {
            super::svg_icons::draw(p, ic, pos2(nr.min.x - 12.0, cy), 15.0, ncol);
        }
    }
    let k = ease(&ctx, id.with("k"), open);
    // the chevron turns over as it opens
    super::svg_icons::draw(p, if k > 0.5 { "chevron-up" } else { "chevron-down" }, cb.center(), 12.0, mix(txt_dim(), hi(), hh.max(k)).gamma_multiply(a));
    let now = ctx.input(|i| i.time);
    // the extra buttons, left of the chevron, fading in while the row is hovered
    let mut extra = None;
    let mut tip_on: Option<(Rect, &str)> = None;
    let row_hov = ctx.pointer_hover_pos().is_some_and(|q| r.contains(q));
    let ek = ease(&ctx, id.with("ex_k"), row_hov && !extras.is_empty());
    if ek > 0.001 {
        for (i, (icon, tip)) in extras.iter().enumerate() {
            let br = Rect::from_center_size(pos2(r.max.x - 26.0 - 12.0 - i as f32 * 24.0, cy), vec2(22.0, r.height() - 4.0));
            let br = br.translate(vec2((1.0 - ek) * 6.0, 0.0));
            let er = ui.interact(br, id.with(("ex", i)), Sense::click());
            if er.hovered() {
                tip_on = Some((br, tip));
            }
            let (eh, ep) = hover_fx(&ctx, &er, true);
            let p = ui.painter();
            p.rect_filled(br, 5.0, hover_fill(eh, ep).gamma_multiply(a * ek));
            super::svg_icons::draw(p, icon, br.center(), 14.0, mix(txt_dim(), hi(), eh).gamma_multiply(a * ek));
            if er.clicked() {
                extra = Some(i);
            }
        }
    }
    // one tip for the extras, gliding between them (ui/glide_tip.rs)
    super::glide_tip::show(&ctx, id.with("ex_tip"), tip_on);
    if resp.clicked() {
        if open {
            set_open(&ctx, None);
        } else {
            set_open(&ctx, Some(id));
            // start scrolled so the current choice shows
            let n = names.len();
            let top = cur.saturating_sub(MAX_ROWS / 2).min(n.saturating_sub(MAX_ROWS)) as f32 * PITCH;
            ctx.data_mut(|d| {
                d.insert_temp(id.with("t0"), now);
                d.insert_temp(id.with("scroll"), top);
            });
            super::micro::glide(&ctx, id.with("sv"), top, true);
        }
    }
    if k <= 0.001 {
        return (None, extra);
    }

    // where it goes: under the row, or over it when the window ends first
    let n = names.len();
    let vis = n.min(MAX_ROWS);
    let h = vis as f32 * PITCH + 8.0;
    let screen = ctx.screen_rect();
    let up = r.max.y + 6.0 + h > screen.max.y - 8.0 && r.min.y - 6.0 - h > screen.min.y + 8.0;
    let sg = if up { -1.0 } else { 1.0 };
    let top = if up { r.min.y - 6.0 - h } else { r.max.y + 6.0 } - (1.0 - k) * 8.0 * sg;
    let tw = names.iter().map(|n| ctx.fonts(|f| f.layout_no_wrap(n.to_string(), FontId::proportional(12.0), txt()).size().x)).fold(0.0_f32, f32::max);
    let lw = (tw + if icons.is_empty() { 10.0 } else { 32.0 } + 18.0 + 8.0).max(120.0).min(r.width());
    let lx = if label.is_empty() { r.min.x } else { r.max.x - lw };
    let list = Rect::from_min_size(pos2(lx, top), vec2(lw, h));
    if open {
        ui_block(&ctx, list);
    }

    // scrolling (glides), only when it doesn't all fit
    let max_s = (n - vis) as f32 * PITCH;
    let mut target: f32 = ctx.data(|d| d.get_temp(id.with("scroll"))).unwrap_or(0.0);
    let hover = ctx.pointer_hover_pos().is_some_and(|q| list.contains(q));
    if open && hover && max_s > 0.0 {
        let dy = ctx.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            target = (target - dy).clamp(0.0, max_s);
            ctx.data_mut(|d| d.insert_temp(id.with("scroll"), target));
        }
    }
    // the same exp-glide as everywhere else, so it settles softly at the ends instead of
    // restarting a timed ease on every wheel tick
    let sv = super::micro::glide(&ctx, id.with("sv"), target, false);

    // a click anywhere else, or Esc, closes it
    let (pressed, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.key_pressed(egui::Key::Escape)));
    if open && (esc || (pressed && !hover && !resp.hovered())) {
        set_open(&ctx, None);
    }

    let t0: f64 = ctx.data(|d| d.get_temp(id.with("t0"))).unwrap_or(now);
    let kf = ((now - t0) / 0.45).clamp(0.0, 1.0) as f32;
    if kf < 1.0 {
        ctx.request_repaint();
    }
    let first = (sv / PITCH).floor() as usize;
    let mut out = None;
    egui::Area::new(id.with("list")).order(egui::Order::Foreground).fixed_pos(list.min).interactable(open).show(&ctx, |ui| {
        ui.allocate_exact_size(list.size(), Sense::click());
        let base = ui.painter().clone().with_clip_rect(screen);
        let mut p = base.clone();
        p.multiply_opacity(k);
        p.rect_filled(list.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
        // a touch greyer than the plain popup, like the panel's own fields
        p.rect_filled(list, 10.0, mix(popup(), gray_ui(90), 0.12));
        p.rect_stroke(list, 10.0, EStroke::new(1.0_f32, border()));
        let inner = list.shrink(4.0);
        for (i, name) in names.iter().enumerate() {
            let rr = Rect::from_min_size(pos2(inner.min.x, inner.min.y + i as f32 * PITCH - sv), vec2(inner.width(), PITCH));
            if !rr.intersects(inner) {
                continue;
            }
            // rows cascade in from the row that opened it, as the edit bar's dropdowns
            let from = if up { (first + vis).saturating_sub(1 + i) } else { i.saturating_sub(first) } as f32;
            let e = (kf * 1.5 - from * 0.15).clamp(0.0, 1.0);
            let rk = 1.0 - (1.0 - e).powi(3);
            let rr = rr.translate(vec2(0.0, -sg * (1.0 - rk) * 16.0));
            let mut rp = base.with_clip_rect(inner);
            rp.multiply_opacity(k * rk);
            let resp = ui.interact(rr.intersect(inner), id.with(("row", i)), Sense::click());
            let (rh, _) = hover_fx(ui.ctx(), &resp, open);
            wave_tile(&rp, rr.shrink2(vec2(0.0, 1.0)), rh, mix(popup(), gray_ui(90), 0.35), 1.0);
            let on = ease(ui.ctx(), id.with(("on", i)), i == cur);
            let col = mix(mix(txt_dim(), txt(), rh), blue(), on);
            let ic = icons.get(i).copied();
            if let Some(ic) = ic {
                super::svg_icons::draw(&rp, ic, pos2(rr.min.x + 18.0, rr.center().y), 15.0, col);
            }
            rp.text(pos2(rr.min.x + if ic.is_some() { 32.0 } else { 10.0 }, rr.center().y), Align2::LEFT_CENTER, *name, FontId::proportional(12.0), col);
            if open && resp.clicked() {
                if i != cur {
                    out = Some(i);
                }
                set_open(ui.ctx(), None);
            }
        }
        // a thin bar shows where in the list it is
        if max_s > 0.0 {
            let tr = Rect::from_min_max(pos2(list.max.x - 5.0, inner.min.y + 2.0), pos2(list.max.x - 3.0, inner.max.y - 2.0));
            let th = tr.height() * vis as f32 / n as f32;
            let ty = tr.min.y + (tr.height() - th) * (sv / max_s);
            p.rect_filled(Rect::from_min_size(pos2(tr.min.x, ty), vec2(tr.width(), th)), 1.0, wa(70));
        }
    });
    (out, extra)
}

/// Is the floating panel `id` (a dropdown or colour pop) the open one
pub(crate) fn is_open(ctx: &egui::Context, id: egui::Id) -> bool {
    open_now(ctx) == Some(id)
}

/// Open / close the floating panel `id` (closes any other)
pub(crate) fn toggle(ctx: &egui::Context, id: egui::Id) {
    set_open(ctx, if is_open(ctx, id) { None } else { Some(id) });
}

/// The colour panel (`ui/color_picker.rs`) floating left of the properties card, level with
/// `anchor`, while `id` is the open floating panel; fades / slides 8 px in like the edit bar's.
/// Returns the new colour as "#rrggbb" when it's changed.
pub(crate) fn color_pop(ctx: &egui::Context, id: egui::Id, anchor: Rect, card: Rect, color: &str) -> Option<String> {
    let open = is_open(ctx, id);
    let kc = ease(ctx, id.with("k"), open);
    if kc <= 0.001 {
        return None;
    }
    let ez = |x: f32| 1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3);
    let szid = id.with("sz");
    let sz: egui::Vec2 = ctx.data(|d| d.get_temp(szid)).unwrap_or(vec2(260.0, 300.0));
    let screen = ctx.screen_rect();
    let y = anchor.min.y.min(screen.max.y - sz.y - 8.0).max(screen.min.y + 8.0);
    let at = pos2(card.min.x - 10.0 - sz.x + (1.0 - ez(kc)) * 8.0, y);
    let mut c = if color.is_empty() { Color32::GRAY } else { hex(color) };
    let ir = egui::Area::new(id.with("area")).order(egui::Order::Foreground).fixed_pos(at).interactable(open).show(ctx, |ui| {
        ui.set_opacity(ez(kc));
        let bg = Rect::from_min_size(at, sz);
        let bp = ui.painter().clone().with_clip_rect(screen);
        bp.rect_filled(bg.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
        glass(&bp, bg, 10.0, 1.0);
        bp.rect_stroke(bg, 10.0, EStroke::new(1.0_f32, border()));
        egui::Frame::none().inner_margin(egui::Margin::same(14.0)).show(ui, |ui| super::color_picker::color_panel(ui, id.with("hsv"), &mut c, kc)).inner
    });
    let r = ir.response.rect;
    ctx.data_mut(|d| d.insert_temp(szid, r.size()));
    if kc < 1.0 {
        ctx.request_repaint();
    }
    if open {
        ui_block(ctx, r);
        let (pressed, esc, q) = ctx.input(|i| (i.pointer.any_pressed(), i.key_pressed(egui::Key::Escape), i.pointer.interact_pos()));
        // clicks on the card itself (the stop rows) leave it to them
        if ir.inner.1 || esc || (pressed && q.is_some_and(|q| !r.contains(q) && !card.contains(q))) {
            set_open(ctx, None);
        }
    }
    (open && ir.inner.0).then(|| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()))
}
