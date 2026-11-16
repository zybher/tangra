//! A picture's look changing to a new edited copy (lasso cut, restore cuts, adjustments)
//! crossfades from the old one instead of swapping at once (animation audit #27, #97).
use crate::*;
use std::collections::HashMap;
use std::sync::Mutex;

struct Ent {
    cur: egui::TextureHandle,
    cuts: usize,
    /// the look it's leaving: (texture, its cut count, when the change happened)
    old: Option<(egui::TextureHandle, usize, f64)>,
    seen: f64,
}

static ST: Mutex<Option<HashMap<String, Ent>>> = Mutex::new(None);

/// What to paint with the picture's texture `cur`: the old texture, the new texture's weight
/// (0..1), and whether the old one goes underneath (cuts restored: holes fill in) or on top
/// (cut: the old pixels fade off the new holes)
pub(crate) fn fade(img: &str, cur: &egui::TextureHandle, cuts: usize, now: f64) -> Option<(egui::TextureHandle, f32, bool)> {
    let dur = 0.18 * anim_k() as f64;
    let mut g = ST.lock().unwrap();
    let m = g.get_or_insert_with(HashMap::new);
    // forget pictures no longer painted (deleted, other board)
    m.retain(|_, e| now - e.seen < 2.0);
    let e = m.entry(img.to_string()).or_insert_with(|| Ent { cur: cur.clone(), cuts, old: None, seen: now });
    e.seen = now;
    if e.cur.id() != cur.id() {
        let prev = std::mem::replace(&mut e.cur, cur.clone());
        e.old = (dur > 0.0).then(|| (prev, e.cuts, now));
        e.cuts = cuts;
    }
    let (old, old_cuts, t0) = e.old.clone()?;
    let t = ((now - t0) / dur.max(1e-6)) as f32;
    if t >= 1.0 {
        e.old = None;
        return None;
    }
    let w = 1.0 - (1.0 - t).powi(3);
    Some((old, w, cuts < old_cuts))
}
