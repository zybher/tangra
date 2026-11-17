//! Canvas items come and go smoothly instead of popping (animation audit #18–35, #97):
//! - entering: pasted, duplicated, added, brought back by undo/redo, or on a board you switch
//!   to; fades in (`enter`). Things you draw yourself never do (they're already on screen).
//! - leaving: deleted or erased items stay on the board a moment, fading out, and are removed
//!   when it ends (`kill` / `App::tick_presence`); undo, redo, a new undo step or a board switch
//!   finish them first so nothing half-deleted is ever saved or restored.
//! - their looks (colour, width, radius, stroke position, fill/outline on and off, font size,
//!   opacity, hiding) glide to new values while painting (`gf` / `gcol`).
use crate::*;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// Fade in / out time (s), scaled by the animation speed
const SECS: f64 = 0.18;

struct St {
    /// id -> (start, starting opacity)
    born: HashMap<String, (f64, f32)>,
    /// id -> when its fade-out began
    dying: HashMap<String, f64>,
}

static ST: Mutex<Option<St>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut St) -> R) -> R {
    let mut g = ST.lock().unwrap();
    f(g.get_or_insert_with(|| St { born: HashMap::new(), dying: HashMap::new() }))
}

fn dur() -> f64 {
    SECS * anim_k().max(1e-3) as f64
}

fn ez(x: f32) -> f32 {
    1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3)
}

/// These items fade in from `a0` (0: from nothing)
pub(crate) fn enter(ids: impl IntoIterator<Item = String>, now: f64, a0: f32) {
    if anim_k() == 0.0 {
        return;
    }
    with(|s| {
        for id in ids {
            s.born.insert(id, (now, a0));
        }
    });
}

/// These items start fading out; `App::tick_presence` removes them when it ends
pub(crate) fn kill(ids: impl IntoIterator<Item = String>, now: f64) {
    with(|s| {
        for id in ids {
            s.dying.entry(id).or_insert(now);
        }
    });
}

pub(crate) fn is_dying(id: &str) -> bool {
    with(|s| s.dying.contains_key(id))
}

pub(crate) fn dying_ids() -> HashSet<String> {
    with(|s| s.dying.keys().cloned().collect())
}

/// How present an item is (0..1) from entering / leaving; 1 for everything else
pub(crate) fn k(id: &str, now: f64) -> f32 {
    with(|s| {
        let mut k = 1.0;
        if let Some(&(t0, a0)) = s.born.get(id) {
            let t = ((now - t0) / dur()) as f32;
            if t >= 1.0 {
                s.born.remove(id);
            } else {
                k *= a0 + (1.0 - a0) * ez(t);
            }
        }
        if let Some(&t0) = s.dying.get(id) {
            k *= 1.0 - ez(((now - t0) / dur()) as f32);
        }
        k
    })
}

/// Anything still fading in or out (keep repainting)
pub(crate) fn busy() -> bool {
    with(|s| !s.born.is_empty() || !s.dying.is_empty())
}

/// Ids whose fade-out has ended (all of them when `all`), taken off the list
fn reap(now: f64, all: bool) -> HashSet<String> {
    with(|s| {
        let d = dur();
        let done: HashSet<String> = s.dying.iter().filter(|(_, t0)| all || now - **t0 >= d).map(|(id, _)| id.clone()).collect();
        for id in &done {
            s.dying.remove(id);
        }
        // finished fade-ins too: only marks on pictures ask `k`, so the rest would otherwise
        // stay listed forever and keep `busy` (a redraw every frame) on
        s.born.retain(|_, (t0, _)| now - *t0 < d);
        done
    })
}

/// A number gliding to `v` (keyed by `id`); snaps when it wasn't painted the frame before
pub(crate) fn gf(ctx: &egui::Context, id: egui::Id, v: f32) -> f32 {
    crate::ui::micro::glide(ctx, id, v, false)
}

/// A colour (unmultiplied rgba, so a fade to clear keeps its hue) gliding to `c`
pub(crate) fn gcol(ctx: &egui::Context, id: egui::Id, [r, g, b, a]: [u8; 4]) -> Color32 {
    let ch = |n: u8, v: u8| gf(ctx, id.with(n), v as f32).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(ch(0, r), ch(1, g), ch(2, b), ch(3, a))
}

/// Every item id on a board (annotation marks included)
pub(crate) fn board_ids(b: &Board) -> HashSet<String> {
    let mut s: HashSet<String> = HashSet::new();
    s.extend(b.images.iter().map(|i| i.id.clone()));
    s.extend(b.sticky_notes.iter().map(|i| i.id.clone()));
    s.extend(b.link_cards.iter().map(|i| i.id.clone()));
    s.extend(b.group_cards.iter().map(|i| i.id.clone()));
    s.extend(b.world_texts.iter().map(|i| i.id.clone()));
    s.extend(b.world_strokes.iter().map(|i| i.id.clone()));
    for img in &b.images {
        s.extend(img.annotations.strokes.iter().map(|i| i.id.clone()));
        s.extend(img.annotations.texts.iter().map(|i| i.id.clone()));
    }
    s
}

impl App {
    /// Takes every item in `ids` off the board (annotation marks included)
    pub(crate) fn remove_ids(&mut self, ids: &HashSet<String>) {
        if ids.is_empty() {
            return;
        }
        let b = &mut self.board;
        b.images.retain(|i| !ids.contains(&i.id));
        b.sticky_notes.retain(|i| !ids.contains(&i.id));
        b.link_cards.retain(|i| !ids.contains(&i.id));
        b.group_cards.retain(|i| !ids.contains(&i.id));
        b.world_texts.retain(|i| !ids.contains(&i.id));
        b.world_strokes.retain(|i| !ids.contains(&i.id));
        for img in &mut b.images {
            img.annotations.strokes.retain(|i| !ids.contains(&i.id));
            img.annotations.texts.retain(|i| !ids.contains(&i.id));
        }
    }

    /// Run every frame: removes items whose fade-out ended
    pub(crate) fn tick_presence(&mut self, ctx: &egui::Context) {
        let done = reap(self.now, false);
        if !done.is_empty() {
            self.remove_ids(&done);
            self.mark_dirty();
        }
        if busy() {
            ctx.request_repaint();
        }
    }

    /// Ends every fade-out now (before undo / redo / a new undo step / a board switch)
    pub(crate) fn finish_dying(&mut self) {
        let done = reap(self.now, true);
        if !done.is_empty() {
            self.remove_ids(&done);
            self.mark_dirty();
        }
    }

    /// Opacity an item paints with: entering / leaving, hide (eye) eased, its opacity glided
    pub(crate) fn presence_k(&self, ctx: &egui::Context, id: &str, hidden: bool, opacity: f32) -> (f32, f32) {
        let vis = ease(ctx, egui::Id::new(("qd_vis", id)), !hidden);
        let op = gf(ctx, egui::Id::new(("qd_op", id)), opacity * 100.0) / 100.0;
        (vis * k(id, self.now), op)
    }
}
