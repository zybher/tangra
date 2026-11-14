//! Custom colour panel (Figma-style): saturation/brightness square, hue slider and a hex field.

use crate::*;
use egui::ecolor::Hsva;

const W: f32 = 232.0;

/// Draws the panel. Returns (colour changed, close clicked). `id` keeps the hue stable while the
/// colour is grey/black (where the hue can't be recovered from RGB).
pub(crate) fn color_panel(ui: &mut egui::Ui, id: egui::Id, c: &mut Color32, k: f32) -> (bool, bool) {
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

    // Header: title + a drawn close cross
    ui.set_opacity(base * part(0.0));
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Custom").font(semibold(13.0)).color(TEXT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (xr, xresp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
            let hv = ease(ui.ctx(), xresp.id, xresp.hovered());
            if hv > 0.01 {
                ui.painter().rect_filled(xr, 5.0, Color32::from_white_alpha((20.0 * hv) as u8));
            }
            let (cc, d) = (xr.center(), 4.0);
            let st = EStroke::new(1.5_f32, mix(TEXT_DIM, TEXT, hv));
            ui.painter().line_segment([cc + vec2(-d, -d), cc + vec2(d, d)], st);
            ui.painter().line_segment([cc + vec2(-d, d), cc + vec2(d, -d)], st);
            if xresp.clicked() {
                close = true;
            }
        });
    });
    ui.add_space(6.0);

    // Saturation (x) / brightness (y) square
    ui.set_opacity(base * part(1.0));
    let (r, resp) = ui.allocate_exact_size(vec2(W, 168.0), Sense::click_and_drag());
    let hue = Color32::from(Hsva::new(hsv.h, 1.0, 1.0, 1.0));
    let mut mesh = egui::Mesh::default();
    const N: usize = 16;
    for j in 0..=N {
        for i in 0..=N {
            let (s, v) = (i as f32 / N as f32, 1.0 - j as f32 / N as f32);
            let col = mix(Color32::WHITE, hue, s);
            let col = mix(Color32::BLACK, col, v);
            mesh.colored_vertex(pos2(r.min.x + s * r.width(), r.min.y + (1.0 - v) * r.height()), col);
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
    p.rect_stroke(r, 0.0, EStroke::new(1.0_f32, border()));
    if let Some(q) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        hsv.s = ((q.x - r.min.x) / r.width()).clamp(0.0, 1.0);
        hsv.v = (1.0 - (q.y - r.min.y) / r.height()).clamp(0.0, 1.0);
        changed = true;
    }
    let knob = pos2(r.min.x + hsv.s * r.width(), r.min.y + (1.0 - hsv.v) * r.height());
    p.circle_filled(knob, 7.0, Color32::from(Hsva::new(hsv.h, hsv.s, hsv.v, 1.0)));
    p.circle_stroke(knob, 7.0, EStroke::new(2.5_f32, Color32::WHITE));
    p.circle_stroke(knob, 8.5, EStroke::new(1.0_f32, Color32::from_black_alpha(90)));
    ui.add_space(10.0);

    // Hue slider
    ui.set_opacity(base * part(2.0));
    let p = ui.painter().clone();
    let (hr, hresp) = ui.allocate_exact_size(vec2(W, 14.0), Sense::click_and_drag());
    let mut hm = egui::Mesh::default();
    const H: usize = 24;
    for i in 0..=H {
        let t = i as f32 / H as f32;
        let col = Color32::from(Hsva::new(t, 1.0, 1.0, 1.0));
        let x = hr.min.x + t * hr.width();
        hm.colored_vertex(pos2(x, hr.min.y), col);
        hm.colored_vertex(pos2(x, hr.max.y), col);
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
    let hk = pos2(hr.min.x + hsv.h * hr.width(), hr.center().y);
    p.circle_filled(hk, 7.5, hue);
    p.circle_stroke(hk, 7.5, EStroke::new(2.5_f32, Color32::WHITE));
    ui.add_space(10.0);

    // Hex field: keeps what you type while focused, applies when it's a full colour
    let cur = format!("{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
    ui.set_opacity(base * part(3.0));
    let hid = id.with("hex");
    let mut txt: String = ui.data(|d| d.get_temp(hid)).unwrap_or_else(|| cur.clone());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Hex").color(TEXT_DIM));
        // Square corners, like the colour square and hue strip
        let ws = &mut ui.visuals_mut().widgets;
        for w in [&mut ws.inactive, &mut ws.hovered, &mut ws.active, &mut ws.noninteractive, &mut ws.open] {
            w.rounding = egui::Rounding::ZERO;
        }
        let te = ui.add(egui::TextEdit::singleline(&mut txt).desired_width(W - 40.0).char_limit(7));
        let clean = txt.trim().trim_start_matches('#').to_string();
        if te.changed() && clean.len() == 6 {
            if let Ok(v) = u32::from_str_radix(&clean, 16) {
                let nc = Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
                hsv = Hsva::from(nc);
                changed = true;
            }
        }
        if !te.has_focus() {
            txt = cur.clone();
        }
    });
    ui.data_mut(|d| d.insert_temp(hid, txt));

    if changed {
        *c = Color32::from(Hsva::new(hsv.h, hsv.s, hsv.v, 1.0));
    }
    ui.data_mut(|d| d.insert_temp(id, (*c, [hsv.h, hsv.s, hsv.v])));
    (changed, close)
}
