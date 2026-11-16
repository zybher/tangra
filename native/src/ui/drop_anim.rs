//! Motion shared by the hover bars' dropdowns, the same as the toolbar's right-click menu
//! (tool_menu.rs): one panel per bar that fades and slides in (`ease`, 8 px), whose rows rise
//! in one after another from the edge nearest the bar (0.45 s cascade, 16 px), and that, when
//! another dropdown of the same bar is opened, glides and resizes to it while the new rows
//! cascade in again. Closing plays the cascade backwards.
use crate::*;

/// Width of panels whose rows need the room (picture sliders), and row pitch (tool_menu.rs)
pub(crate) const W: f32 = 190.0;
pub(crate) const PITCH: f32 = 30.0;

/// Narrowest list panel that holds its rows: labels start at `text_x`, the longest one is
/// measured at its hovered (wave) size, then the same small margin on the right
pub(crate) fn fit_w(ctx: &egui::Context, text_x: f32, labels: &[&str]) -> f32 {
    let font = FontId::proportional(12.0 * 1.14);
    let tw = labels.iter().map(|l| ctx.fonts(|f| f.layout_no_wrap(l.to_string(), font.clone(), Color32::WHITE).size().x)).fold(0.0_f32, f32::max);
    (text_x + tw + 14.0).ceil()
}

/// Which way a bar's dropdown of height `ph` opens: away from the object being edited (its
/// screen box, published by the place / style bars as "qd_edit_obj") so it never covers
/// it when there's room; with room neither or both ways, the side that covers less of it
pub(crate) fn opens_up(ctx: &egui::Context, bar: Rect, ph: f32, rect: Rect) -> bool {
    let down_room = bar.max.y + 14.0 + ph <= rect.max.y - 70.0;
    let up_room = bar.min.y - 14.0 - ph >= rect.min.y + 8.0;
    let Some(obj) = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("qd_edit_obj"))) else { return !down_room };
    let cover = |y0: f32, y1: f32| (y1.min(obj.max.y) - y0.max(obj.min.y)).max(0.0);
    let down = cover(bar.max.y + 14.0, bar.max.y + 14.0 + ph);
    let up = cover(bar.min.y - 14.0 - ph, bar.min.y - 14.0);
    match (up_room, down_room) {
        (true, true) => up < down,
        (true, false) => true,
        (false, true) => false,
        (false, false) => up < down,
    }
}

/// Left edge of a `w`-wide panel centred over a button at `cx`, kept inside `rect`
pub(crate) fn left_for(cx: f32, rect: Rect, w: f32) -> f32 {
    (cx - w / 2.0).clamp(rect.min.x + 8.0, (rect.max.x - 8.0 - w).max(rect.min.x + 8.0))
}

/// A press that should close open dropdowns and popovers: any press except one that starts
/// moving the canvas (middle button, or with space held), so they stay open while panning
pub(crate) fn closing_press(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.pointer.any_pressed() && !i.pointer.button_pressed(egui::PointerButton::Middle) && !(i.key_down(egui::Key::Space) && !ctx.wants_keyboard_input()))
}

/// Left edge for a dropdown at `left`..+`w`, `top`..+`ph`: when it would still cover the
/// object being edited, it moves beside the object instead (the nearer side that fits)
pub(crate) fn aside_x(ctx: &egui::Context, left: f32, w: f32, top: f32, ph: f32, rect: Rect) -> f32 {
    let Some(obj) = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("qd_edit_obj"))) else { return left };
    if !Rect::from_min_size(pos2(left, top), vec2(w, ph)).intersects(obj) {
        return left;
    }
    let l = obj.min.x - 12.0 - w;
    let r = obj.max.x + 12.0;
    let fl = l >= rect.min.x + 8.0;
    let fr = r + w <= rect.max.x - 8.0;
    match (fl, fr) {
        (true, true) => if (left - l).abs() <= (left - r).abs() { l } else { r },
        (true, false) => l,
        (false, true) => r,
        _ => left,
    }
}

pub(crate) struct DropAnim {
    /// The dropdown shown (stays the last one while it closes); 0 = none yet
    pub kind: u8,
    /// Panel fade / slide, eased
    pub t: f32,
    ot: f32,
    kf: f32,
    id: egui::Id,
}

impl DropAnim {
    /// `id`: one per bar. `pop`: the bar's open dropdown (0 = closed).
    pub(crate) fn new(ctx: &egui::Context, id: egui::Id, pop: u8) -> Self {
        let mut kind: u8 = ctx.data(|d| d.get_temp(id.with("kind"))).unwrap_or(0);
        let t = ease(ctx, id.with("t"), pop != 0);
        let ot = ctx.animate_bool_with_time(id.with("ot"), pop != 0, 0.45 * anim_k());
        let now = ctx.input(|i| i.time);
        let mut since: f64 = ctx.data(|d| d.get_temp(id.with("since"))).unwrap_or(0.0);
        if pop != 0 && pop != kind {
            // Switching while open replays the cascade (a fresh open uses `ot`)
            if kind != 0 && t > 0.5 {
                since = now;
            }
            kind = pop;
        }
        ctx.data_mut(|d| {
            d.insert_temp(id.with("kind"), kind);
            d.insert_temp(id.with("since"), since);
        });
        let kf = ((now - since) / 0.45).clamp(0.0, 1.0) as f32;
        if kf < 1.0 || (t > 0.0 && t < 1.0) {
            ctx.request_repaint();
        }
        Self { kind, t, ot, kf, id }
    }

    /// Panel opacity for dropdown `kind` (0 unless it is the one shown)
    pub(crate) fn k(&self, kind: u8) -> f32 {
        if self.kind == kind {
            self.t
        } else {
            0.0
        }
    }

    /// Row `i` of `n`: how far it has risen in (0..1). `up`: the panel sits above the bar.
    pub(crate) fn row(&self, i: usize, n: usize, up: bool) -> f32 {
        let from_bar = if up { n.saturating_sub(1 + i) } else { i } as f32;
        let cascade = |x: f32| {
            let e = (x * 1.5 - from_bar * 0.15).clamp(0.0, 1.0);
            1.0 - (1.0 - e).powi(3)
        };
        cascade(self.kf).min(cascade(self.ot))
    }

    /// The panel's background for target `pr`, gliding (left, width and the far edge) after a
    /// switch; also returns how far the rows shift sideways to stay inside it.
    pub(crate) fn panel(&self, ctx: &egui::Context, pr: Rect, up: bool) -> (Rect, f32) {
        let far = if up { pr.min.y } else { pr.max.y };
        let target = [pr.min.x, pr.width(), far];
        let gid = self.id.with("glide");
        let pass = ctx.cumulative_pass_nr();
        let (mut cur, last): ([f32; 3], u64) = ctx.data(|d| d.get_temp(gid)).unwrap_or((target, pass.wrapping_sub(1)));
        if last != pass {
            let dt = ctx.input(|i| i.stable_dt).min(0.05);
            // A fresh open starts in place; only a switch glides (as the toolbar menu)
            if self.t < 0.95 && self.kf >= 1.0 {
                cur = target;
            } else {
                let f = 1.0 - (-dt * 16.0).exp();
                for j in 0..3 {
                    cur[j] += (target[j] - cur[j]) * f;
                }
            }
            if (0..3).any(|j| (cur[j] - target[j]).abs() > 0.3) {
                ctx.request_repaint();
            } else {
                cur = target;
            }
            ctx.data_mut(|d| d.insert_temp(gid, (cur, pass)));
        }
        let bg = if up {
            Rect::from_min_max(pos2(cur[0], cur[2]), pos2(cur[0] + cur[1], pr.max.y))
        } else {
            Rect::from_min_max(pos2(cur[0], pr.min.y), pos2(cur[0] + cur[1], cur[2]))
        };
        (bg, cur[0] - pr.min.x)
    }
}
