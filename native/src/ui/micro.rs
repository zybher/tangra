//! Small shared motion helpers for micro-interactions that used to snap: a value that glides
//! to where it is going (tooltips sliding between buttons, knobs, the active tab, markers).
//! Same exp-glide as the pointer and the rename caret; off with the app's animations.
use crate::*;

/// How fast a glided value catches up (1/s), as inline_text's caret
const RATE: f32 = 16.0;

/// Glides the value stored at `id` toward `target` and returns it. `snap` jumps straight there
/// (first show, or when it was hidden so it shouldn't fly in from somewhere old). A value
/// that wasn't glided last frame (its thing wasn't drawn) also starts where it is.
pub(crate) fn glide(ctx: &egui::Context, id: egui::Id, target: f32, snap: bool) -> f32 {
    let ak = anim_k();
    let pass = ctx.cumulative_pass_nr();
    let cur: Option<(f32, u64)> = ctx.data(|d| d.get_temp(id));
    let v = match cur {
        Some((c, last)) if !snap && ak > 0.0 && pass.saturating_sub(last) <= 1 => {
            let dt = ctx.input(|i| i.stable_dt).min(0.05);
            c + (target - c) * (1.0 - (-dt * RATE / ak).exp())
        }
        _ => target,
    };
    let v = if (v - target).abs() < 0.05 { target } else { v };
    ctx.data_mut(|d| d.insert_temp(id, (v, pass)));
    if v != target {
        ctx.request_repaint();
    }
    v
}
