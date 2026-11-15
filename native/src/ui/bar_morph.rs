//! The style bar and the placement bar are the same pill: when the selection swaps from one
//! to the other (an image ↔ a frame), the new bar starts where the old one was and glides
//! (position + width) instead of the old one shrinking out and the new one springing in.
use crate::*;

type Glide = (f32, f32, f32);

/// Every frame a bar is up: where it is (centre x, centre y, width)
pub(crate) fn publish(ctx: &egui::Context, bid: egui::Id, g: Glide) {
    let nr = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(bid.with("morph_out"), (g, nr)));
}

/// On a fresh entrance: start from the other bar's spot if it was up a moment ago.
/// Returns true when it took over (the entrance spring is skipped; the pill just glides)
pub(crate) fn take(ctx: &egui::Context, bid: egui::Id, other: egui::Id, now: f64) -> bool {
    let nr = ctx.cumulative_pass_nr();
    let Some((g, at)) = ctx.data(|d| d.get_temp::<(Glide, u64)>(other.with("morph_out"))) else { return false };
    if nr.saturating_sub(at) > 1 {
        return false;
    }
    ctx.data_mut(|d| {
        d.insert_temp(bid.with("glide"), g);
        d.insert_temp(bid.with("t0"), now - 10.0);
        d.insert_temp(other.with("taken"), now);
        d.remove::<(Glide, u64)>(other.with("morph_out"));
    });
    true
}

/// The other bar took this one's place: skip the exit animation
pub(crate) fn taken(ctx: &egui::Context, bid: egui::Id, now: f64) -> bool {
    ctx.data(|d| d.get_temp::<f64>(bid.with("taken"))).is_some_and(|t| now - t < 0.6)
}
