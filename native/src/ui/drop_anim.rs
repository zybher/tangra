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

/// A bar's own side of the object: above if there's room, else below, or the other side once
/// a dropdown needed it (`drop_fit`) while that side still has room for the bar
pub(crate) fn bar_side(ctx: &egui::Context, bid: egui::Id, fr: Rect, h: f32, rect: Rect) -> bool {
    let top_ok = fr.min.y - 32.0 - h >= rect.min.y + 50.0;
    let bottom_ok = fr.max.y + 32.0 + h <= rect.max.y - 70.0;
    if ctx.data(|d| d.get_temp::<bool>(bid.with("flip"))).unwrap_or(false) {
        // only ever flips from above to below (above was the natural pick, so it had room)
        if top_ok && bottom_ok {
            return false;
        }
        ctx.data_mut(|d| d.remove::<bool>(bid.with("flip")));
    }
    top_ok
}

/// Where a bar is heading (before its glide), and whether it may move to the object's other side
pub(crate) fn publish_target(ctx: &egui::Context, bid: egui::Id, target: Rect, can_flip: bool) {
    ctx.data_mut(|d| d.insert_temp(bid.with("target"), (target, can_flip)));
}

/// The far-collapse waits after a flip until the pointer has reached the moved bar / dropdown
pub(crate) fn flip_grace(ctx: &egui::Context, bid: egui::Id) -> bool {
    ctx.data(|d| d.get_temp::<bool>(bid.with("grace"))).unwrap_or(false)
}

/// How bar `bid`'s `n`-row list opens: (columns, panel height, opens up). Off the object being
/// edited if anyhow possible: one column, else wrapped into 2 then 3 (shorter, wider, but not
/// much wider than the bar: `WIDE`, `w` being one column's width); else the
/// whole bar moves to the object's other side and opens away from it there; only when nothing
/// clears it does it stay one column on the side that covers less (`opens_up`)
/// Columns are only used while the panel stays within `WIDE` x the bar's width (a single
/// column always may), so a dropdown never sprawls far past the bar it hangs from
pub(crate) const WIDE: f32 = 1.15;

pub(crate) fn drop_fit(ctx: &egui::Context, bid: egui::Id, bar: Rect, n: usize, rect: Rect, w: f32) -> (usize, f32, bool) {
    let ph1 = n as f32 * PITCH + 8.0;
    let Some(obj) = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("qd_edit_obj"))) else {
        return (1, ph1, opens_up(ctx, bar, ph1, rect));
    };
    // decided where the bar is heading, so a glide in progress doesn't flip it back
    let (tgt, can_flip) = ctx.data(|d| d.get_temp::<(Rect, bool)>(bid.with("target"))).unwrap_or((bar, false));
    let cover = |y0: f32, y1: f32| (y1.min(obj.max.y) - y0.max(obj.min.y)).max(0.0);
    let fits = |b: Rect| -> Option<(usize, f32, bool)> {
        let mut last = 0;
        for c in 1..=3usize {
            let per = n.div_ceil(c);
            if c > 1 && (per < 2 || per == last || grid_w(w, c) > bar.width().max(grid_w(w, 2)) * WIDE) {
                break;
            }
            last = per;
            let ph = per as f32 * PITCH + 8.0;
            let down = b.max.y + 14.0 + ph <= rect.max.y - 70.0 && cover(b.max.y + 14.0, b.max.y + 14.0 + ph) <= 0.0;
            let up = b.min.y - 14.0 - ph >= rect.min.y + 8.0 && cover(b.min.y - 14.0 - ph, b.min.y - 14.0) <= 0.0;
            if down || up {
                return Some((c, ph, !down));
            }
        }
        None
    };
    if let Some(f) = fits(tgt) {
        return f;
    }
    if can_flip {
        let above = tgt.center().y < obj.center().y;
        let alt = if above { Rect::from_min_size(pos2(tgt.min.x, obj.max.y + 32.0), tgt.size()) } else { Rect::from_min_size(pos2(tgt.min.x, obj.min.y - 32.0 - tgt.height()), tgt.size()) };
        let room = if above { alt.max.y <= rect.max.y - 70.0 } else { alt.min.y >= rect.min.y + 50.0 };
        if let (true, Some(f)) = (room, fits(alt)) {
            let was = ctx.data(|d| d.get_temp::<bool>(bid.with("flip"))).unwrap_or(false);
            ctx.data_mut(|d| {
                d.insert_temp(bid.with("flip"), !was);
                d.insert_temp(bid.with("grace"), true);
            });
            ctx.request_repaint();
            return f;
        }
    }
    (1, ph1, opens_up(ctx, bar, ph1, rect))
}

/// Left edge of a `w`-wide panel centred over a button at `cx`, kept inside `rect`
pub(crate) fn left_for(cx: f32, rect: Rect, w: f32) -> f32 {
    (cx - w / 2.0).clamp(rect.min.x + 8.0, (rect.max.x - 8.0 - w).max(rect.min.x + 8.0))
}

/// A press that should close open dropdowns and popovers: any press except one that starts
/// moving the canvas (middle button, or with space held), so they stay open while panning
pub(crate) fn closing_press(ctx: &egui::Context) -> bool {
    // The light / dark toggle doesn't count: the reveal freezes the screen as it starts, which
    // would hold a half-closed dropdown on the old side (ui/theme_toggle.rs)
    let tog: Rect = ctx.data(|d| d.get_temp(egui::Id::new("qd_theme_btn"))).unwrap_or(Rect::NOTHING);
    ctx.input(|i| {
        i.pointer.any_pressed()
            && !i.pointer.button_pressed(egui::PointerButton::Middle)
            && !(i.key_down(egui::Key::Space) && !ctx.wants_keyboard_input())
            && !i.pointer.interact_pos().is_some_and(|q| tog.contains(q))
    })
}

/// True while bar `bar`'s dropdown is open or still playing its exit (`bar`: the bar's id).
/// A bar going away first closes its dropdown, waits on this, then plays its own exit
pub(crate) fn hold_bar(ctx: &egui::Context, bar: egui::Id) -> bool {
    let pop: u8 = ctx.data(|d| d.get_temp(bar.with("pop"))).unwrap_or(0);
    if pop != 0 {
        ctx.data_mut(|d| d.insert_temp(bar.with("pop"), 0u8));
        ctx.request_repaint();
    }
    pop != 0 || ctx.data(|d| d.get_temp::<f32>(bar.with("drop").with("t_now"))).unwrap_or(0.0) > 0.0
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
            d.insert_temp(id.with("t_now"), t);
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

/// How far (px) the pointer may wander from a bar and its open dropdown before it collapses
pub(crate) const FAR: f32 = 440.0;

/// Auto-collapse shared by the style and placement bars: the pointer stays more than `FAR`
/// clear of the bar and its dropdown (`zone`) for a moment (leaving the window counts too).
/// Waits after the bar moved to the object's other side until the pointer has reached it, and
/// while `hold` (a row being dragged). Sets `set_pop` to 0 when it fires.
pub(crate) fn far_collapse(ctx: &egui::Context, bid: egui::Id, zone: Rect, pop: u8, set_pop: &mut u8, hold: bool) {
    if pop != 0 && *set_pop == pop {
        let far = ctx.input(|i| i.pointer.hover_pos()).map_or(true, |q| {
            let d = vec2((zone.min.x - q.x).max(q.x - zone.max.x).max(0.0), (zone.min.y - q.y).max(q.y - zone.max.y).max(0.0));
            d.length() > FAR
        });
        let grace = flip_grace(ctx, bid);
        if grace && ctx.input(|i| i.pointer.hover_pos()).is_some_and(|q| zone.expand(4.0).contains(q)) {
            ctx.data_mut(|d| d.remove::<bool>(bid.with("grace")));
        }
        let far = far && !grace && !hold;
        let aid = bid.with("away");
        let now = ctx.input(|i| i.time);
        if far {
            let t = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(aid, || now));
            ctx.request_repaint();
            if now - t > 0.35 {
                *set_pop = 0;
            }
        } else {
            ctx.data_mut(|d| d.remove::<f64>(aid));
        }
    }
    if *set_pop == 0 {
        ctx.data_mut(|d| {
            d.remove::<f64>(bid.with("away"));
            d.remove::<bool>(bid.with("grace"));
        });
    }
}
