//! Subtle glides: align, distribute, fill / fit and "resize to fit" ease objects into place
//! instead of snapping. The undo step is taken before the glide starts; the board only
//! counts as changed once everything has landed.
use crate::*;
use std::sync::Mutex;

struct Glide {
    sel: Sel,
    from: [f32; 4],
    to: [f32; 4],
    /// What we wrote last frame: anything else means someone else moved it (undo, drag), so stop
    last: [f32; 4],
}

static GLIDES: Mutex<(f64, Vec<Glide>)> = Mutex::new((0.0, Vec::new()));

const SECS: f32 = 0.22;

fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t)
}

impl App {
    /// Move each object to its new box, gliding there
    pub(crate) fn glide_to(&mut self, moves: Vec<(Sel, [f32; 4])>) {
        if anim_k() == 0.0 {
            for ((k, id), r) in moves {
                self.set_rect(k, &id, r);
            }
            self.mark_dirty();
            return;
        }
        let mut g = GLIDES.lock().unwrap();
        // Anything still mid-glide lands first
        for x in g.1.drain(..) {
            self.set_rect(x.sel.0, &x.sel.1, x.to);
        }
        g.0 = self.now;
        for (s, to) in moves {
            if let Some(from) = self.rect_of(s.0, &s.1) {
                g.1.push(Glide { sel: s, from, to, last: from });
            }
        }
    }

    /// Lands every glide at its end now (before a new undo step, undo or redo, so a
    /// half-way box is never recorded)
    pub(crate) fn land_glides(&mut self) {
        let gl = std::mem::take(&mut GLIDES.lock().unwrap().1);
        if gl.is_empty() {
            return;
        }
        for x in gl {
            self.set_rect(x.sel.0, &x.sel.1, x.to);
        }
        self.mark_dirty();
    }

    /// Run every frame
    pub(crate) fn tick_glides(&mut self, ctx: &egui::Context) {
        let mut g = GLIDES.lock().unwrap();
        if g.1.is_empty() {
            return;
        }
        let t = ((self.now - g.0) as f32 / (SECS * anim_k().max(1e-3))).clamp(0.0, 1.0);
        let e = 1.0 - (1.0 - t).powi(3);
        let mut gl = std::mem::take(&mut g.1);
        gl.retain_mut(|x| {
            let Some(cur) = self.rect_of(x.sel.0, &x.sel.1) else { return false };
            if (0..4).any(|i| (cur[i] - x.last[i]).abs() > 0.01) {
                return false;
            }
            let r = if t >= 1.0 { x.to } else { lerp4(x.from, x.to, e) };
            self.set_rect(x.sel.0, &x.sel.1, r);
            x.last = self.rect_of(x.sel.0, &x.sel.1).unwrap_or(r);
            t < 1.0
        });
        g.1 = gl;
        if g.1.is_empty() {
            self.mark_dirty();
        } else {
            ctx.request_repaint();
        }
    }
}
