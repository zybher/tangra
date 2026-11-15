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

    /// New boxes for an object moved by (dx, dy); a frame brings everything inside it
    fn tree_moves(&self, k: Kind, id: &str, dx: f32, dy: f32, out: &mut Vec<(Sel, [f32; 4])>) {
        let mut all = vec![(k, id.to_string())];
        if k == Kind::Group {
            all.extend(self.descendants(id));
        }
        for s in all {
            if let Some([x, y, w, h]) = self.rect_of(s.0, &s.1) {
                out.push((s, [x + dx, y + dy, w, h]));
            }
        }
    }

    pub(crate) fn align_selection(&mut self, how: Align) {
        let targets = self.place_targets();
        if self.place_locked_by_layout(&targets) {
            return;
        }
        let Some(f) = self.place_bounds(&targets) else { return };
        let mut moves = Vec::new();
        for s in &targets {
            let Some(r) = self.rect_of(s.0, &s.1) else { continue };
            let (dx, dy) = match how {
                Align::Left => (f[0] - r[0], 0.0),
                Align::HCenter => (f[0] + (f[2] - r[2]) / 2.0 - r[0], 0.0),
                Align::Right => (f[0] + f[2] - r[2] - r[0], 0.0),
                Align::Top => (0.0, f[1] - r[1]),
                Align::VCenter => (0.0, f[1] + (f[3] - r[3]) / 2.0 - r[1]),
                Align::Bottom => (0.0, f[1] + f[3] - r[3] - r[1]),
            };
            if dx.abs() + dy.abs() > 0.01 {
                self.tree_moves(s.0, &s.1, dx, dy, &mut moves);
            }
        }
        if moves.is_empty() {
            return;
        }
        self.snapshot();
        self.glide_to(moves);
    }

    /// Even gaps between 3+ objects, keeping the outer ones where they are. Objects are split
    /// into lanes first (rows for spacing across, columns for spacing down), so a grid gets
    /// even gaps in every row instead of piling up.
    pub(crate) fn distribute_selection(&mut self, horizontal: bool) {
        let targets = self.place_targets();
        let mut rs: Vec<(Sel, [f32; 4])> = targets.into_iter().filter_map(|s| self.rect_of(s.0, &s.1).map(|r| (s, r))).collect();
        if rs.len() < 3 {
            return;
        }
        let a = if horizontal { 0 } else { 1 };
        let c = 1 - a;
        // Outer edges along the spacing axis (shared by every lane, so columns line up)
        let start = rs.iter().map(|(_, r)| r[a]).fold(f32::MAX, f32::min);
        let end = rs.iter().map(|(_, r)| r[a] + r[a + 2]).fold(f32::MIN, f32::max);
        // Lanes: objects whose cross-axis centre falls inside the lane's span so far
        rs.sort_by(|x, y| x.1[c].total_cmp(&y.1[c]));
        let mut lanes: Vec<(f32, Vec<(Sel, [f32; 4])>)> = Vec::new();
        for it in rs {
            let mid = it.1[c] + it.1[c + 2] / 2.0;
            match lanes.last_mut() {
                Some((lane_hi, l)) if mid < *lane_hi => {
                    *lane_hi = lane_hi.max(it.1[c] + it.1[c + 2]);
                    l.push(it);
                }
                _ => lanes.push((it.1[c] + it.1[c + 2], vec![it])),
            }
        }
        let single_lane = lanes.len() == 1;
        let mut moves = Vec::new();
        for (_, mut lane) in lanes {
            if lane.len() < 2 || (single_lane && lane.len() < 3) {
                continue;
            }
            lane.sort_by(|x, y| x.1[a].total_cmp(&y.1[a]));
            let total: f32 = lane.iter().map(|(_, r)| r[a + 2]).sum();
            let gap = (end - start - total) / (lane.len() - 1) as f32;
            let mut at = start;
            for ((k, id), r) in lane {
                let d = at - r[a];
                if d.abs() > 0.01 {
                    if horizontal {
                        self.tree_moves(k, &id, d, 0.0, &mut moves);
                    } else {
                        self.tree_moves(k, &id, 0.0, d, &mut moves);
                    }
                }
                at += r[a + 2] + gap;
            }
        }
        if moves.is_empty() {
            return;
        }
        self.snapshot();
        self.glide_to(moves);
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
        self.glide_to(vec![((Kind::Image, id.clone()), nr)]);
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
        self.glide_to(fits.into_iter().map(|(f, b)| ((Kind::Group, f), b)).collect());
    }

    /// Wrap the selection in a new frame (Figma's Ctrl+Alt+G). The new frame takes the place
    /// of the selection in whatever frame it was in.
    pub(crate) fn frame_selection(&mut self) {
        let targets = self.place_targets();
        let mut b: Option<[f32; 4]> = None;
        for (k, id) in &targets {
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
        let Some(b) = b.filter(|b| b[2] > 1.0 && b[3] > 1.0) else { return };
        // Shared frame around the selection, else the board
        let home = match targets.first().and_then(|t| self.parent_of(&t.1)) {
            Some(p) if targets.iter().all(|t| self.parent_of(&t.1) == Some(p)) => p.to_string(),
            _ => String::new(),
        };
        self.snapshot();
        let id = self.board.add_group(Pt { x: 0.0, y: 0.0 });
        // Paint under any frame being wrapped (frames paint in list order)
        let first = self.board.group_cards.iter().position(|g| targets.iter().any(|t| t.0 == Kind::Group && t.1 == g.id));
        if let (Some(at), Some(g)) = (first, self.board.group_cards.pop()) {
            self.board.group_cards.insert(at, g);
        }
        if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
            (g.x, g.y, g.width, g.height) = (b[0], b[1], b[2], b[3]);
            g.title = "Frame".into();
            g.clip = true;
        }
        self.board.parents.insert(id.clone(), home);
        for (_, t) in &targets {
            self.board.parents.insert(t.clone(), id.clone());
        }
        let n = targets.len();
        self.select_only((Kind::Group, id));
        self.mark_dirty();
        self.toast(format!("Framed {n} object{}", if n == 1 { "" } else { "s" }));
    }
}
