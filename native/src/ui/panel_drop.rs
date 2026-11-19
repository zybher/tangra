//! Dropdowns in the properties panel (Blend, a fill's gradient type…). The row shows the
//! current choice; a click opens a floating list over everything (the panel doesn't grow).
//! One choice a row, top to bottom; past 10 the list scrolls. It opens under its row (over it
//! when there's no room below), fades / slides in with its rows cascading like the edit bar's
//! dropdowns, and closes on a pick, a click elsewhere or Esc. Only one is open at a time.

use super::drop_anim::PITCH;
use crate::*;

/// Rows shown before the list scrolls
const MAX_ROWS: usize = 10;
/// How many rows a bar dropdown's list shows before it scrolls
const BAR_ROWS: usize = 5;

fn open_now(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.data(|d| d.get_temp::<Option<egui::Id>>(egui::Id::new("qd_pdrop_open"))).flatten()
}

fn set_open(ctx: &egui::Context, id: Option<egui::Id>) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        if id.is_none() && d.get_temp::<Option<egui::Id>>(egui::Id::new("qd_pdrop_open")).flatten().is_some() {
            d.insert_temp(egui::Id::new("qd_pdrop_shut"), pass);
        }
        d.insert_temp(egui::Id::new("qd_pdrop_open"), id)
    });
}

/// A list is open, or closed this frame or the one before (the click that picked in it
/// mustn't land on what's under it once it's gone)
pub(crate) fn covering(ctx: &egui::Context) -> bool {
    let pass = ctx.cumulative_pass_nr();
    any_open(ctx) || ctx.data(|d| d.get_temp::<u64>(egui::Id::new("qd_pdrop_shut"))).is_some_and(|p| pass <= p + 1)
}

/// A dropdown row in `r` (`label` on the left, the current choice on the right; an empty
/// label puts the choice on the left), with an icon (`ui/svg_icons.rs`) beside each choice in
/// the list and the row. Returns the newly picked index.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dropdown_icons(ui: &mut egui::Ui, id: egui::Id, r: Rect, label: &str, names: &[&str], icons: &[&'static str], cur: usize, a: f32) -> Option<usize> {
    dropdown_ex(ui, id, r, label, names, icons, cur, &[], a).0
}

/// A dropdown that looks and moves exactly like the hover bar's (style_bar.rs): the whole box
/// (icon, choice, chevron) is the button, with the bar's hover and the liquid blue fill while
/// open; its list is the bar's (bar_drop.rs / drop_anim.rs): glass panel fading and sliding
/// in, rows cascading in from the box's side, the wave magnify, as wide as its rows. Opening
/// it closes any other floating panel; a pick, a click elsewhere or Esc closes it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dropdown_bar(ui: &mut egui::Ui, id: egui::Id, r: Rect, names: &[&str], icons: &[&'static str], cur: usize, tip: &str, a: f32) -> Option<usize> {
    dropdown_bar_ex(ui, id, r, names, icons, cur, tip, false, a)
}

/// `dropdown_bar` shown as just its choice's icon (`icon_only`: the properties card's blend
/// mode): no box at rest, the hover fill on hover, the liquid blue with a white icon while
/// open; its list (with the names) opens under it, lined up on its right edge
#[allow(clippy::too_many_arguments)]
pub(crate) fn dropdown_bar_ex(ui: &mut egui::Ui, id: egui::Id, r: Rect, names: &[&str], icons: &[&'static str], cur: usize, tip: &str, icon_only: bool, a: f32) -> Option<usize> {
    let ctx = ui.ctx().clone();
    let open = open_now(&ctx) == Some(id);
    let resp = ui.interact(r, id, Sense::click()).on_hover_text(tip);
    let (hh, pp) = hover_fx(&ctx, &resp, true);
    let p = ui.painter();
    if !icon_only {
        p.rect_filled(r, 6.0, Color32::from_black_alpha((60.0 * a) as u8));
    }
    p.rect_filled(r, 6.0, hover_fill(hh, pp).gamma_multiply(a));
    // the bar's liquid fill while its list is open
    let lq = ctx.animate_bool_with_time(id.with("liquid"), open, 0.5 * anim_k());
    let lq = lq * lq * (3.0 - 2.0 * lq);
    if lq >= 0.999 {
        p.rect_filled(r, 6.0, blue().gamma_multiply(a));
    } else if lq > 0.001 {
        liquid_fill(p, r, 6.0, lq, ctx.input(|i| i.time) as f32 + 5.0);
    }
    let gc = mix(txt_dim(), hi(), hh.max(lq)).gamma_multiply(a);
    let cy = r.center().y;
    if icon_only {
        if let Some(ic) = icons.get(cur) {
            super::svg_icons::draw(p, ic, r.center(), 15.0, gc);
        }
    } else {
        if let Some(ic) = icons.get(cur) {
            super::svg_icons::draw(p, ic, pos2(r.min.x + 16.0, cy), 16.0, gc);
        }
        p.text(pos2(r.min.x + 30.0, cy), Align2::LEFT_CENTER, names.get(cur).copied().unwrap_or(""), FontId::proportional(12.0), gc);
        super::svg_icons::draw(p, "chevron-down", pos2(r.max.x - 13.0, cy), 12.0, gc);
    }
    // a long list shows BAR_ROWS at a time and scrolls (the same glide as `dropdown_ex`)
    let n = names.len();
    let vis = n.min(BAR_ROWS);
    let max_s = (n - vis) as f32 * PITCH;
    if resp.clicked() {
        set_open(&ctx, if open { None } else { Some(id) });
        if !open {
            // start scrolled so the current choice shows
            let top = cur.saturating_sub(vis / 2).min(n - vis) as f32 * PITCH;
            ctx.data_mut(|d| d.insert_temp(id.with("scroll"), top));
            super::micro::glide(&ctx, id.with("sv"), top, true);
        }
    }
    let da = super::drop_anim::DropAnim::new(&ctx, id.with("drop"), if open { 1 } else { 0 });
    if da.k(1) <= 0.001 {
        return None;
    }
    let mut out = None;
    let mut zone = Rect::NOTHING;
    let w = super::drop_anim::fit_w(&ctx, 32.0, names);
    egui::Area::new(id.with("list")).order(egui::Order::Foreground).fixed_pos(ctx.screen_rect().min).interactable(open).show(&ctx, |ui| {
        // above the floating panel it opened from (same order), for drawing and for clicks
        ui.ctx().move_to_top(ui.layer_id());
        let sw = if max_s > 0.0 { 8.0 } else { 0.0 };
        let dd = super::bar_drop::open_under(ui, &da, id, open, r, 6.0, vis, w + sw, icon_only, super::bar_drop::MENU);
        zone = dd.zone();
        let mut target: f32 = ui.ctx().data(|d| d.get_temp(id.with("scroll"))).unwrap_or(0.0);
        if open && max_s > 0.0 && ui.ctx().pointer_hover_pos().is_some_and(|q| zone.contains(q)) {
            let dy = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 {
                target = (target - dy).clamp(0.0, max_s);
                ui.ctx().data_mut(|d| d.insert_temp(id.with("scroll"), target));
            }
        }
        let sv = super::micro::glide(ui.ctx(), id.with("sv"), target, false);
        for (i, name) in names.iter().enumerate() {
            let f = i as f32 - sv / PITCH;
            if f <= -1.0 || f >= vis as f32 {
                continue;
            }
            let (rr, wm, p) = dd.row_at(&da, f);
            let rr = Rect::from_min_max(rr.min, pos2(rr.max.x - sw, rr.max.y));
            let resp = ui.interact(rr.intersect(p.clip_rect()), id.with(("row", i)), Sense::click());
            wave_tile(&p, rr, wm, mix(popup(), gray_ui(90), 0.35), 1.0);
            let k = 1.0 + 0.14 * wm;
            let ok = ease(ui.ctx(), id.with(("on", i)), i == cur);
            let col = mix(mix(txt_dim(), txt(), wm), blue(), ok);
            if let Some(ic) = icons.get(i) {
                super::svg_icons::draw(&p, ic, pos2(rr.min.x + 16.0, rr.center().y), 16.0 * k, col);
            }
            p.text(pos2(rr.min.x + 32.0, rr.center().y), Align2::LEFT_CENTER, *name, FontId::proportional(12.0 * k), col);
            if open && resp.clicked() {
                if i != cur {
                    out = Some(i);
                }
                set_open(ui.ctx(), None);
            }
        }
        // a thin dark bar shows where in the list it is, its track from the first row's text
        // top to the last row's text bottom
        if max_s > 0.0 {
            let pr = dd.panel_rect();
            let (r0, r1) = (dd.slot(0), dd.slot(vis - 1));
            let tr = Rect::from_min_max(pos2(pr.max.x - 7.0, r0.center().y - 6.0), pos2(pr.max.x - 5.0, r1.center().y + 6.0));
            let th = tr.height() * vis as f32 / n as f32;
            let ty = tr.min.y + (tr.height() - th) * (sv / max_s).clamp(0.0, 1.0);
            let mut sp = ui.painter().clone().with_clip_rect(pr);
            sp.multiply_opacity(dd.k * da.row(0, vis, false));
            sp.rect_filled(tr, 1.0, Color32::from_black_alpha(60));
            sp.rect_filled(Rect::from_min_size(pos2(tr.min.x, ty), vec2(tr.width(), th)), 1.0, border());
        }
    });
    if open {
        ui_block(&ctx, zone);
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let q = ctx.input(|i| i.pointer.interact_pos());
        if esc || (super::drop_anim::closing_press(&ctx) && q.is_some_and(|q| !zone.contains(q) && !r.contains(q))) {
            set_open(&ctx, None);
        }
    }
    out
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
    let k = ease(&ctx, id.with("k"), open);
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

/// `dropdown_bar` (the blend modes' look and wave list) with `dropdown_ex`'s small extra
/// buttons left of the chevron, fading in while the row is hovered; returns (pick, extra)
#[allow(clippy::too_many_arguments)]
pub(crate) fn dropdown_bar_extras(ui: &mut egui::Ui, id: egui::Id, r: Rect, names: &[&str], icons: &[&'static str], cur: usize, tip: &str, extras: &[(&'static str, &str)], a: f32) -> (Option<usize>, Option<usize>) {
    let pick = dropdown_bar_ex(ui, id, r, names, icons, cur, tip, false, a);
    let ctx = ui.ctx().clone();
    let cy = r.center().y;
    let mut extra = None;
    let mut tip_on: Option<(Rect, &str)> = None;
    let row_hov = ctx.pointer_hover_pos().is_some_and(|q| r.contains(q));
    let ek = ease(&ctx, id.with("ex_k"), row_hov && !extras.is_empty() && open_now(&ctx) != Some(id));
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
            if er.clicked() && ek > 0.5 {
                extra = Some(i);
            }
        }
    }
    super::glide_tip::show(&ctx, id.with("ex_tip"), tip_on);
    (pick, extra)
}

/// Is the floating panel `id` (a dropdown or colour pop) the open one
/// Close whichever floating panel is open
pub(crate) fn close_all(ctx: &egui::Context) {
    set_open(ctx, None);
}

/// Is any floating panel (a dropdown list, a colour panel, a hex bubble) open
pub(crate) fn any_open(ctx: &egui::Context) -> bool {
    open_now(ctx).is_some()
}

pub(crate) fn is_open(ctx: &egui::Context, id: egui::Id) -> bool {
    open_now(ctx) == Some(id)
}

/// Open / close the floating panel `id` (closes any other)
pub(crate) fn toggle(ctx: &egui::Context, id: egui::Id) {
    set_open(ctx, if is_open(ctx, id) { None } else { Some(id) });
}

