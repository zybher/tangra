//! Flip the selected shapes, lines and paths horizontally or vertically in place (the
//! properties panel's Position section). Everything that rides the shape's box flips with
//! it: its points and curve handles, rubbed-out spots and lasso cuts, and a gradient's handles.
//! Frames, pictures and text don't flip yet.
use crate::*;

/// Mirror `s` across the middle of its own box (`horiz`: left / right, else top / bottom)
fn flip_stroke(s: &mut Stroke, horiz: bool) {
    if s.points.is_empty() {
        return;
    }
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for q in &s.points {
        let v = if horiz { q.x } else { q.y };
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let m = lo + hi;
    let f = |q: &mut Pt| {
        if horiz {
            q.x = m - q.x;
        } else {
            q.y = m - q.y;
        }
    };
    s.points.iter_mut().for_each(f);
    for k in &mut s.knots {
        f(&mut k.p);
        f(&mut k.i);
        f(&mut k.o);
    }
    let a = if horiz { 0 } else { 1 };
    for e in &mut s.erase {
        e[a] = 1.0 - e[a];
    }
    for l in &mut s.erase_loops {
        for q in l.iter_mut() {
            q[a] = 1.0 - q[a];
        }
    }
    if let Some(g) = &mut s.gradient {
        g.from[a] = 1.0 - g.from[a];
        g.to[a] = 1.0 - g.to[a];
    }
}

impl App {
    /// Any selected shape that can flip
    pub(crate) fn can_flip(&self) -> bool {
        self.selected.iter().any(|s| s.0 == Kind::Stroke)
    }

    /// Flip every selected shape / line / path in place (one undo step)
    pub(crate) fn flip_selection(&mut self, horiz: bool) {
        if !self.can_flip() {
            return;
        }
        self.snapshot();
        let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Stroke).map(|s| s.1.clone()).collect();
        let b = &mut self.board;
        for s in b.world_strokes.iter_mut().chain(b.images.iter_mut().flat_map(|i| i.annotations.strokes.iter_mut())) {
            if ids.contains(&s.id) {
                flip_stroke(s, horiz);
            }
        }
        self.mark_dirty();
    }
}
