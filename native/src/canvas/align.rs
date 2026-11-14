//! Align, distribute, fill and fade the selection. One object is placed inside the frame it
//! belongs to; several objects are placed against their combined box (like Figma).
use crate::*;

/// Left, centre, right (x) and top, middle, bottom (y)
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Align {
    Left,
    HCenter,
    Right,
    Top,
    VCenter,
    Bottom,
}

impl App {
    /// The selected objects that move on their own (a child whose frame is also selected rides along)
    pub(crate) fn place_targets(&self) -> Vec<Sel> {
        // Everything inside a selected frame, worked out once
        let inner: std::collections::HashSet<String> = self
            .selected
            .iter()
            .filter(|o| o.0 == Kind::Group)
            .flat_map(|o| self.descendants(&o.1).into_iter().map(|s| s.1))
            .collect();
        self.selected.iter().filter(|s| !inner.contains(&s.1)).cloned().collect()
    }

    /// The box to align against: the parent frame for one object, else the selection's box
    pub(crate) fn place_bounds(&self, targets: &[Sel]) -> Option<[f32; 4]> {
        if let [(k, id)] = targets {
            let _ = k;
            return self.parent_of(id).and_then(|p| self.rect_of(Kind::Group, p));
        }
        let mut b: Option<[f32; 4]> = None;
        for (k, id) in targets {
            let Some(r) = self.rect_of(*k, id) else { continue };
            b = Some(match b {
                None => r,
                Some(a) => {
                    let (x0, y0) = (a[0].min(r[0]), a[1].min(r[1]));
                    let (x1, y1) = ((a[0] + a[2]).max(r[0] + r[2]), (a[1] + a[3]).max(r[1] + r[3]));
                    [x0, y0, x1 - x0, y1 - y0]
                }
            });
        }
        b
    }

    /// True when one object sits in an auto-arranging frame (the frame decides where it goes)
    pub(crate) fn place_locked_by_layout(&self, targets: &[Sel]) -> bool {
        matches!(targets, [(Kind::Image, id)] if self.parent_of(id).is_some_and(|p| self.board.group_cards.iter().any(|g| g.id == p && g.layout > 0)))
    }

    /// Move an object by (dx, dy); a frame brings everything inside it
    fn nudge_tree(&mut self, k: Kind, id: &str, dx: f32, dy: f32) {
        let mut all = vec![(k, id.to_string())];
        if k == Kind::Group {
            all.extend(self.descendants(id));
        }
        for (kd, i) in all {
            if let Some([x, y, w, h]) = self.rect_of(kd, &i) {
                self.set_rect(kd, &i, [x + dx, y + dy, w, h]);
            }
        }
    }

    pub(crate) fn align_selection(&mut self, how: Align) {
        let targets = self.place_targets();
        if self.place_locked_by_layout(&targets) {
            return;
        }
        let Some(f) = self.place_bounds(&targets) else { return };
        let moves: Vec<(Sel, f32, f32)> = targets
            .iter()
            .filter_map(|s| {
                let r = self.rect_of(s.0, &s.1)?;
                let (dx, dy) = match how {
                    Align::Left => (f[0] - r[0], 0.0),
                    Align::HCenter => (f[0] + (f[2] - r[2]) / 2.0 - r[0], 0.0),
                    Align::Right => (f[0] + f[2] - r[2] - r[0], 0.0),
                    Align::Top => (0.0, f[1] - r[1]),
                    Align::VCenter => (0.0, f[1] + (f[3] - r[3]) / 2.0 - r[1]),
                    Align::Bottom => (0.0, f[1] + f[3] - r[3] - r[1]),
                };
                (dx.abs() + dy.abs() > 0.01).then(|| (s.clone(), dx, dy))
            })
            .collect();
        if moves.is_empty() {
            return;
        }
        self.snapshot();
        for ((k, id), dx, dy) in moves {
            self.nudge_tree(k, &id, dx, dy);
        }
        self.mark_dirty();
    }

    /// Even gaps between 3+ objects, keeping the first and last where they are
    pub(crate) fn distribute_selection(&mut self, horizontal: bool) {
        let targets = self.place_targets();
        let mut rs: Vec<(Sel, [f32; 4])> = targets.into_iter().filter_map(|s| self.rect_of(s.0, &s.1).map(|r| (s, r))).collect();
        if rs.len() < 3 {
            return;
        }
        let a = if horizontal { 0 } else { 1 };
        rs.sort_by(|x, y| x.1[a].total_cmp(&y.1[a]));
        let start = rs[0].1[a];
        let end = rs.iter().map(|(_, r)| r[a] + r[a + 2]).fold(f32::MIN, f32::max);
        let total: f32 = rs.iter().map(|(_, r)| r[a + 2]).sum();
        let gap = (end - start - total) / (rs.len() - 1) as f32;
        self.snapshot();
        let mut at = start;
        for ((k, id), r) in rs {
            let d = at - r[a];
            if horizontal {
                self.nudge_tree(k, &id, d, 0.0);
            } else {
                self.nudge_tree(k, &id, 0.0, d);
            }
            at += r[a + 2] + gap;
        }
        self.mark_dirty();
    }

    /// One image: scale it to cover its frame (keeping its shape), centred; the frame crops the rest
    pub(crate) fn fill_frame(&mut self, cover: bool) {
        let targets = self.place_targets();
        let [(Kind::Image, id)] = targets.as_slice() else { return };
        let (Some(f), Some(r)) = (self.place_bounds(&targets), self.rect_of(Kind::Image, id)) else { return };
        if r[2] <= 0.0 || r[3] <= 0.0 {
            return;
        }
        let (kx, ky) = (f[2] / r[2], f[3] / r[3]);
        let k = if cover { kx.max(ky) } else { kx.min(ky) };
        let (w, h) = (r[2] * k, r[3] * k);
        let nr = [f[0] + (f[2] - w) / 2.0, f[1] + (f[3] - h) / 2.0, w, h];
        if (0..4).all(|i| (nr[i] - r[i]).abs() < 0.01) {
            return;
        }
        self.snapshot();
        let id = id.clone();
        self.set_rect(Kind::Image, &id, nr);
        self.mark_dirty();
    }

    /// Set the opacity of every selected object (no undo step: the caller snapshots once per drag)
    pub(crate) fn set_selection_opacity(&mut self, v: f32) {
        let v = v.clamp(0.0, 1.0);
        for (_, id) in self.selected.clone() {
            if v >= 0.999 {
                self.board.opacity.remove(&id);
            } else {
                self.board.opacity.insert(id, v);
            }
        }
        self.mark_dirty();
    }

    /// Selected frames (not auto-arranging) shrink / grow to wrap everything inside them
    pub(crate) fn fit_frames_to_content(&mut self) {
        let frames: Vec<String> = self
            .selected
            .iter()
            .filter(|s| s.0 == Kind::Group && self.board.group_cards.iter().any(|g| g.id == s.1 && g.layout == 0))
            .map(|s| s.1.clone())
            .collect();
        let mut fits = Vec::new();
        for f in frames {
            let kids = self.descendants(&f);
            if let Some(b) = self.place_bounds(&kids).filter(|_| kids.len() > 1).or_else(|| kids.first().and_then(|k| self.rect_of(k.0, &k.1))) {
                if b[2] > 1.0 && b[3] > 1.0 {
                    fits.push((f, b));
                }
            }
        }
        if fits.is_empty() {
            return;
        }
        self.snapshot();
        for (f, b) in fits {
            self.set_rect(Kind::Group, &f, b);
        }
        self.mark_dirty();
    }
}
