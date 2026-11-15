//! Motion shared by the hover bars' dropdowns, the same as the toolbar's right-click menu
//! (tool_menu.rs): one panel per bar that fades and slides in (`ease`, 8 px), whose rows rise
//! in one after another from the edge nearest the bar (0.45 s cascade, 16 px), and that, when
//! another dropdown of the same bar is opened, glides and resizes to it while the new rows
//! cascade in again. Closing plays the cascade backwards.
use crate::*;

/// Panel width and row pitch, the toolbar menu's (tool_menu.rs)
pub(crate) const W: f32 = 190.0;
pub(crate) const PITCH: f32 = 30.0;

/// Left edge of a panel centred over a button at `cx`, kept inside `rect`
pub(crate) fn left_for(cx: f32, rect: Rect) -> f32 {
    (cx - W / 2.0).clamp(rect.min.x + 8.0, (rect.max.x - 8.0 - W).max(rect.min.x + 8.0))
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
        let (mut cur, last): ([f32; 3], u64) = ctx.data(|d| d.get_temp(gid)).unwrap_or((target, pass));
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
