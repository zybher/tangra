//! Custom colour panel (Figma-style): saturation/brightness square, hue slider and a hex field,
//! all with the app's rounded corners (the square's corner points are pulled onto 6 px arcs,
//! the hue strip is a pill).

use crate::*;
use egui::ecolor::Hsva;

const W: f32 = 232.0;

/// Draws the panel. Returns (colour changed, close clicked). `id` keeps the hue stable while the
/// colour is grey/black (where the hue can't be recovered from RGB).
pub(crate) fn color_panel(ui: &mut egui::Ui, id: egui::Id, c: &mut Color32, k: f32) -> (bool, bool) {
    color_panel_ex(ui, id, c, k, true)
}

/// The same, without its "Custom" title and close cross (inside the paint panel, which has its own)
pub(crate) fn color_panel_ex(ui: &mut egui::Ui, id: egui::Id, c: &mut Color32, k: f32, header: bool) -> (bool, bool) {
    ui.set_width(W);
    // Parts appear one after another as `k` goes 0 -> 1 (and leave in reverse)
    let base = ui.opacity();
    let part = |i: f32| {
        let x = (k * 1.6 - i * 0.18).clamp(0.0, 1.0);
        1.0 - (1.0 - x).powi(3)
    };
    let mut changed = false;
    let mut close = false;
    let mut hsv = ui
        .data(|d| d.get_temp::<(Color32, [f32; 3])>(id))
        .filter(|(k, _)| k == c)
        .map(|(_, h)| Hsva::new(h[0], h[1], h[2], 1.0))
        .unwrap_or_else(|| Hsva::from(*c));

    if header {
        // Header: title + a drawn close cross
        ui.set_opacity(base * part(0.0));
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Custom").font(semibold(13.0)).color(txt()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (xr, xresp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                let hv = ease(ui.ctx(), xresp.id, xresp.hovered());
                if hv > 0.01 {
                    ui.painter().rect_filled(xr, 5.0, wa((20.0 * hv) as u8));
                }
                let (cc, d) = (xr.center(), 4.0);
                let st = EStroke::new(1.5_f32, mix(txt_dim(), txt(), hv));
                ui.painter().line_segment([cc + vec2(-d, -d), cc + vec2(d, d)], st);
                ui.painter().line_segment([cc + vec2(-d, d), cc + vec2(d, -d)], st);
                if xresp.clicked() {
                    close = true;
                }
            });
        });
        ui.add_space(6.0);
    }

    // Saturation (x) / brightness (y) square
    ui.set_opacity(base * part(1.0));
    let (r, resp) = ui.allocate_exact_size(vec2(W, 168.0), Sense::click_and_drag());
    let hue = Color32::from(Hsva::new(hsv.h, 1.0, 1.0, 1.0));
    let mut mesh = egui::Mesh::default();
    // a fine grid whose corner points are pulled onto rounded corners (the panel's 6 px)
    const N: usize = 48;
    const RAD: f32 = 6.0;
    for j in 0..=N {
        for i in 0..=N {
            let (s, v) = (i as f32 / N as f32, 1.0 - j as f32 / N as f32);
            let col = mix(Color32::WHITE, hue, s);
            let col = mix(Color32::BLACK, col, v);
            mesh.colored_vertex(round_in(pos2(r.min.x + s * r.width(), r.min.y + (1.0 - v) * r.height()), r, RAD), col);
        }
    }
    for j in 0..N {
        for i in 0..N {
            let a = (j * (N + 1) + i) as u32;
            let b = a + (N + 1) as u32;
            mesh.add_triangle(a, a + 1, b);
            mesh.add_triangle(a + 1, b + 1, b);
        }
    }
    let p = ui.painter().clone();
    p.add(egui::Shape::mesh(mesh));
    p.rect_stroke(r, RAD, EStroke::new(1.0_f32, border()));
    if let Some(q) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        hsv.s = ((q.x - r.min.x) / r.width()).clamp(0.0, 1.0);
        hsv.v = (1.0 - (q.y - r.min.y) / r.height()).clamp(0.0, 1.0);
        changed = true;
    }
    // Knobs glide when the colour jumps (hex typed, swatch picked); follow the pointer exactly when dragged
    let sv_drag = resp.dragged();
    let knob = pos2(
        super::micro::glide(ui.ctx(), id.with("kx"), r.min.x + hsv.s * r.width(), sv_drag),
        super::micro::glide(ui.ctx(), id.with("ky"), r.min.y + (1.0 - hsv.v) * r.height(), sv_drag),
    );
    // Knobs grow 1px on hover, like the grid-dots opacity slider's knob (side_panel.rs)
    let (kh, _) = hover_fx(ui.ctx(), &resp, true);
    p.circle_filled(knob, 7.0 + kh, Color32::from(Hsva::new(hsv.h, hsv.s, hsv.v, 1.0)));
    p.circle_stroke(knob, 7.0 + kh, EStroke::new(2.5_f32, Color32::WHITE));
    p.circle_stroke(knob, 8.5 + kh, EStroke::new(1.0_f32, Color32::from_black_alpha(90)));
    ui.add_space(10.0);

    // Hue slider
    ui.set_opacity(base * part(2.0));
    let p = ui.painter().clone();
    let (hr, hresp) = ui.allocate_exact_size(vec2(W, 14.0), Sense::click_and_drag());
    let mut hm = egui::Mesh::default();
    // a pill: round ends
    // columns spaced by angle round each end cap (so the arcs come out smooth), evenly between
    let hrad = hr.height() / 2.0;
    const CAP: usize = 16;
    const MID: usize = 64;
    let mut xs: Vec<f32> = Vec::with_capacity(2 * CAP + MID + 2);
    for i in 0..=CAP {
        let an = std::f32::consts::FRAC_PI_2 * i as f32 / CAP as f32;
        xs.push(hr.min.x + hrad * (1.0 - an.cos()));
    }
    for i in 1..MID {
        xs.push(hr.min.x + hrad + (hr.width() - 2.0 * hrad) * i as f32 / MID as f32);
    }
    for i in (0..=CAP).rev() {
        let an = std::f32::consts::FRAC_PI_2 * i as f32 / CAP as f32;
        xs.push(hr.max.x - hrad * (1.0 - an.cos()));
    }
    for (i, &x) in xs.iter().enumerate() {
        let t = ((x - hr.min.x) / hr.width()).clamp(0.0, 1.0);
        let col = Color32::from(Hsva::new(t, 1.0, 1.0, 1.0));
        let d = (x - hr.min.x).min(hr.max.x - x).min(hrad);
        let half = (hrad * hrad - (hrad - d) * (hrad - d)).max(0.0).sqrt();
        hm.colored_vertex(pos2(x, hr.center().y - half), col);
        hm.colored_vertex(pos2(x, hr.center().y + half), col);
        if i > 0 {
            let k = (i * 2) as u32;
            hm.add_triangle(k - 2, k - 1, k);
            hm.add_triangle(k - 1, k + 1, k);
        }
    }
    p.with_clip_rect(hr).add(egui::Shape::mesh(hm));
    if let Some(q) = hresp.interact_pointer_pos().filter(|_| hresp.dragged() || hresp.clicked()) {
        hsv.h = ((q.x - hr.min.x) / hr.width()).clamp(0.0, 0.9999);
        changed = true;
    }
    let hk = pos2(super::micro::glide(ui.ctx(), id.with("hx"), hr.min.x + hsv.h * hr.width(), hresp.dragged()), hr.center().y);
    let (hh, _) = hover_fx(ui.ctx(), &hresp, true);
    p.circle_filled(hk, 7.5 + hh, hue);
    p.circle_stroke(hk, 7.5 + hh, EStroke::new(2.5_f32, Color32::WHITE));
    ui.add_space(10.0);

    // Hex field: keeps what you type while focused, applies when it's a full colour
    let cur = format!("{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
    ui.set_opacity(base * part(3.0));
    let hid = id.with("hex");
    let mut txt: String = ui.data(|d| d.get_temp(hid)).unwrap_or_else(|| cur.clone());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Hex").color(txt_dim()));
        // A rounded dark box like the panel's fields; typed in place like a rename (gliding
        // caret and selection, inline_text.rs)
        let (fr, fresp) = ui.allocate_exact_size(vec2(W - 40.0 - 30.0, 24.0), Sense::click());
        let fp = ui.painter().clone();
        let (fh, fpp) = hover_fx(ui.ctx(), &fresp, true);
        fp.rect_filled(fr, 6.0, Color32::from_black_alpha(60));
        fp.rect_filled(fr, 6.0, hover_fill(fh, fpp));
        let tr = fr.shrink2(vec2(5.0, 0.0));
        let font = egui::TextStyle::Body.resolve(ui.style());
        let tcol = ui.visuals().text_color();
        let son = hid.with("on");
        let on: bool = ui.data(|d| d.get_temp(son)).unwrap_or(false);
        if fresp.hovered() && !on {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        let before = txt.clone();
        let mut focused = false;
        if on || fresp.clicked() {
            let ring = super::inline_text::Ring::Fixed(fr, 6.0);
            let res = super::inline_text::edit(ui, hid.with("field"), tr, &mut txt, font, tcol, !on, ring);
            focused = res.is_none();
            ui.data_mut(|d| d.insert_temp(son, focused));
        } else {
            fp.with_clip_rect(fr).text(tr.left_center(), egui::Align2::LEFT_CENTER, txt.as_str(), font, tcol);
        }
        txt = txt.chars().take(7).collect();
        let clean = txt.trim().trim_start_matches('#').to_string();
        if txt != before && clean.len() == 6 {
            if let Ok(v) = u32::from_str_radix(&clean, 16) {
                let nc = Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
                hsv = Hsva::from(nc);
                changed = true;
            }
        }
        if !focused {
            txt = cur.clone();
        }
        // an eyedropper beside it: the next click anywhere in the window takes that pixel's colour
        let (er, _) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::hover());
        // on the same dark box as the hex field beside it
        ui.painter().rect_filled(er, 6.0, Color32::from_black_alpha(60));
        if super::paint_panel::icon_btn(ui, id.with("drop"), er, "eyedropper", "Pick a colour from the screen", 1.0) {
            ui.ctx().data_mut(|d| d.insert_temp(dropper_key(), (id, None::<Pos2>)));
        }
    });
    if let Some(nc) = dropper(ui.ctx(), id) {
        hsv = Hsva::from(nc);
        changed = true;
    }
    ui.data_mut(|d| d.insert_temp(hid, txt));

    if changed {
        *c = Color32::from(Hsva::new(hsv.h, hsv.s, hsv.v, 1.0));
    }
    ui.data_mut(|d| d.insert_temp(id, (*c, [hsv.h, hsv.s, hsv.v])));
    (changed, close)
}

/// `q` pulled inside `r`'s rounded corners (radius `rad`), for meshes with rounded edges
pub(crate) fn round_in(q: Pos2, r: Rect, rad: f32) -> Pos2 {
    let cx = q.x.clamp(r.min.x + rad, r.max.x - rad);
    let cy = q.y.clamp(r.min.y + rad, r.max.y - rad);
    let d = q - pos2(cx, cy);
    if d.length() > rad {
        pos2(cx, cy) + d.normalized() * rad
    } else {
        q
    }
}

fn dropper_key() -> egui::Id {
    egui::Id::new("qd_eyedropper")
}

/// Is the eyedropper waiting for its click (other panels shouldn't close on it)
pub(crate) fn picking(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<(egui::Id, Option<Pos2>)>(dropper_key())).is_some()
}

/// The eyedropper of panel `id`: while on, a see-through layer over everything takes the next
/// click (crosshair; Esc cancels). The window is captured every frame, so a card by the pointer
/// shows the pixels round it magnified (the one under it ringed), its colour and hex; the click
/// takes that pixel's colour
fn dropper(ctx: &egui::Context, id: egui::Id) -> Option<Color32> {
    let (pid, _): (egui::Id, Option<Pos2>) = ctx.data(|d| d.get_temp(dropper_key()))?;
    let img_key = dropper_key().with("img");
    let stop = |ctx: &egui::Context| {
        ctx.data_mut(|d| {
            d.remove::<(egui::Id, Option<Pos2>)>(dropper_key());
            d.remove::<std::sync::Arc<egui::ColorImage>>(img_key);
            d.remove::<u64>(dropper_key().with("asked"));
            d.remove::<u64>(dropper_key().with("since"));
        })
    };
    if pid != id {
        return None;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        stop(ctx);
        return None;
    }
    // One capture, taken while neither the pointer nor the card is drawn (the pointer is painted
    // by the app, so a capture with it would sample the pointer itself); the board holds still
    // while picking, so that one picture is what's under the pointer
    // (the capture itself is caught every frame by `dropper_tick`, wherever it lands)
    let img: Option<std::sync::Arc<egui::ColorImage>> = ctx.data(|d| d.get_temp(img_key));
    if img.is_none() {
        let asked = dropper_key().with("asked");
        let pass = ctx.cumulative_pass_nr();
        let first = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(dropper_key().with("since"), || pass));
        if pass > first + 120 {
            // no capture came back: give up rather than leave the window unusable
            stop(ctx);
            ctx.data_mut(|d| d.remove::<u64>(dropper_key().with("since")));
            return None;
        }
        if ctx.data(|d| d.get_temp::<u64>(asked)).is_none_or(|n| pass > n + 10) {
            ctx.data_mut(|d| d.insert_temp(asked, pass));
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
        ctx.set_cursor_icon(egui::CursorIcon::None);
        ctx.request_repaint();
        return None;
    }
    let ppp = ctx.pixels_per_point();
    let px = |q: Pos2, dx: i32, dy: i32| -> Option<Color32> {
        let im = img.as_ref()?;
        let [w, h] = im.size;
        let (x, y) = ((q.x * ppp) as i32 + dx, (q.y * ppp) as i32 + dy);
        (x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h).then(|| im.pixels[y as usize * w + x as usize].to_opaque())
    };
    let screen = ctx.screen_rect();
    let mut out = None;
    egui::Area::new(id.with("dropper_layer")).order(egui::Order::Tooltip).fixed_pos(screen.min).show(ctx, |ui| {
        let resp = ui.allocate_rect(screen, Sense::click());
        if img.is_none() {
            return;
        }
        // the crosshair with an eyedropper badge, like the drawing tools' cursors
        crate::ui::cursor::set_badge(ui.ctx(), "eyedropper");
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        let Some(q) = ui.ctx().pointer_hover_pos() else { return };
        let here = px(q, 0, 0);
        if resp.clicked() {
            out = here;
            stop(ui.ctx());
            return;
        }
        // the card: a magnified patch, the colour's square and hex
        const N: i32 = 9;
        const CELL: f32 = 7.0;
        let patch = N as f32 * CELL;
        let card_w = 12.0 + patch + 12.0 + 120.0;
        let card_h = patch + 16.0;
        let mut at = q + vec2(18.0, 18.0);
        if at.x + card_w > screen.max.x - 8.0 {
            at.x = q.x - 18.0 - card_w;
        }
        if at.y + card_h > screen.max.y - 8.0 {
            at.y = q.y - 18.0 - card_h;
        }
        let card = Rect::from_min_size(at, vec2(card_w, card_h));
        let p = ui.painter();
        p.rect_filled(card.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(70));
        p.rect_filled(card, 10.0, popup());
        p.rect_stroke(card, 10.0, EStroke::new(1.0_f32, border()));
        let pr = Rect::from_min_size(card.min + vec2(8.0, 8.0), vec2(patch, patch));
        let pp = p.with_clip_rect(pr);
        // the cells as one mesh pulled into the patch's rounded corners (`round_in`, like the
        // gradient swatch), so the corner cells curve with the frame instead of poking out
        let mut m = egui::Mesh::default();
        for gy in 0..N {
            for gx in 0..N {
                let c = px(q, gx - N / 2, gy - N / 2).unwrap_or(Color32::BLACK);
                let r = Rect::from_min_size(pr.min + vec2(gx as f32 * CELL, gy as f32 * CELL), vec2(CELL, CELL));
                const S: usize = 4;
                let b = m.vertices.len() as u32;
                for yy in 0..=S {
                    for xx in 0..=S {
                        let v = pos2(r.min.x + r.width() * xx as f32 / S as f32, r.min.y + r.height() * yy as f32 / S as f32);
                        m.colored_vertex(round_in(v, pr, 6.0), c);
                    }
                }
                for yy in 0..S as u32 {
                    for xx in 0..S as u32 {
                        let i = b + yy * (S as u32 + 1) + xx;
                        m.add_triangle(i, i + 1, i + S as u32 + 1);
                        m.add_triangle(i + 1, i + S as u32 + 2, i + S as u32 + 1);
                    }
                }
            }
        }
        pp.add(m);
        for k in 1..N {
            let o = k as f32 * CELL;
            let lc = EStroke::new(0.5_f32, Color32::from_black_alpha(40));
            pp.line_segment([round_in(pos2(pr.min.x + o, pr.min.y), pr, 6.0), round_in(pos2(pr.min.x + o, pr.max.y), pr, 6.0)], lc);
            pp.line_segment([round_in(pos2(pr.min.x, pr.min.y + o), pr, 6.0), round_in(pos2(pr.max.x, pr.min.y + o), pr, 6.0)], lc);
        }
        let mid = Rect::from_min_size(pr.min + vec2((N / 2) as f32 * CELL, (N / 2) as f32 * CELL), vec2(CELL, CELL));
        pp.rect_stroke(mid.expand(1.0), 1.0, EStroke::new(1.5_f32, Color32::WHITE));
        p.rect_stroke(pr, 6.0, EStroke::new(1.0_f32, border()));
        let c = here.unwrap_or(Color32::BLACK);
        let sw = Rect::from_min_size(pos2(pr.max.x + 12.0, pr.min.y + 6.0), vec2(18.0, 18.0));
        p.rect_filled(sw, 5.0, c);
        p.rect_stroke(sw, 5.0, EStroke::new(1.0_f32, wa(60)));
        p.text(pos2(sw.max.x + 8.0, sw.center().y), Align2::LEFT_CENTER, format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b()), FontId::proportional(13.0), txt());
        let ty = pr.max.y - 14.0;
        super::svg_icons::draw(p, "eyedropper", pos2(sw.center().x, ty), 14.0, txt_dim());
        p.text(pos2(sw.max.x + 8.0, ty), Align2::LEFT_CENTER, "Click to sample", FontId::proportional(12.0), txt_dim());
    });
    out
}

/// Run every frame: keeps the eyedropper's capture whichever frame it arrives in
pub(crate) fn dropper_tick(ctx: &egui::Context) {
    if !picking(ctx) {
        return;
    }
    let shot = ctx.input(|i| {
        i.raw.events.iter().rev().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    if let Some(img) = shot {
        ctx.data_mut(|d| d.insert_temp(dropper_key().with("img"), img));
    }
}
