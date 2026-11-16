//! One stacking order for the whole board, shared by the outline, painting and clicking:
//! the outline list is the canvas, top row in front. Among the objects in one frame (or
//! loose on the board):
//! - ones placed by hand (outline drag, bring to front / send to back) keep that place
//!   (`board.layer_order`, smaller is further in front);
//! - newer objects nobody placed yet go on top of those, frames nobody placed underneath,
//!   so a fresh board stacks like it always did (text, drawings, notes, links, pictures,
//!   then frames, newer above older).
//!
//! A frame's contents always sit on top of the frame itself.
use crate::*;
use std::collections::{HashMap, HashSet};

/// One step of painting, back to front
pub(crate) enum Step {
    Item(Sel),
    /// Everything inside this frame is painted: its outline goes on now
    End(String),
}

fn kind_rank(k: Kind) -> u8 {
    match k {
        Kind::Text => 0,
        Kind::Stroke => 1,
        Kind::Note => 2,
        Kind::Link => 3,
        Kind::Image => 4,
        Kind::Group => 5,
    }
}

impl App {
    /// Sort key among siblings, front first. `idx` is the object's place in `all_items`
    /// (later in its list means newer, so further in front)
    pub(crate) fn z_key(&self, s: &Sel, idx: usize) -> (u8, f32, u8, i64) {
        match self.board.layer_order.get(&s.1) {
            Some(o) => (1, *o, kind_rank(s.0), -(idx as i64)),
            None if s.0 == Kind::Group => (2, 0.0, 0, -(idx as i64)),
            None => (0, 0.0, kind_rank(s.0), -(idx as i64)),
        }
    }

    /// Top-level objects and each frame's contents, front first
    pub(crate) fn z_lists(&self) -> (Vec<Sel>, HashMap<String, Vec<Sel>>) {
        let items = self.all_items();
        let idx: HashMap<String, usize> = items.iter().enumerate().map(|(i, s)| (s.1.clone(), i)).collect();
        let mut roots: Vec<Sel> = Vec::new();
        let mut kids: HashMap<String, Vec<Sel>> = HashMap::new();
        for s in &items {
            match self.parent_of(&s.1).filter(|p| idx.contains_key(*p)) {
                Some(p) => kids.entry(p.to_string()).or_default().push(s.clone()),
                None => roots.push(s.clone()),
            }
        }
        let sort = |v: &mut Vec<Sel>| {
            v.sort_by(|a, b| self.z_key(a, idx[&a.1]).partial_cmp(&self.z_key(b, idx[&b.1])).unwrap_or(std::cmp::Ordering::Equal))
        };
        sort(&mut roots);
        for v in kids.values_mut() {
            sort(v);
        }
        (roots, kids)
    }

    /// Everything in paint order, back to front
    pub(crate) fn paint_steps(&self) -> Vec<Step> {
        let (roots, kids) = self.z_lists();
        let mut out = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        fn walk(list: &[Sel], kids: &HashMap<String, Vec<Sel>>, seen: &mut HashSet<String>, out: &mut Vec<Step>) {
            for s in list.iter().rev() {
                if !seen.insert(s.1.clone()) {
                    continue;
                }
                out.push(Step::Item(s.clone()));
                if let Some(k) = kids.get(&s.1) {
                    walk(k, kids, seen, out);
                }
                if s.0 == Kind::Group {
                    out.push(Step::End(s.1.clone()));
                }
            }
        }
        walk(&roots, &kids, &mut seen, &mut out);
        // Anything in a parent loop still paints
        for (_, k) in &kids {
            walk(k, &kids, &mut seen, &mut out);
        }
        out
    }

    /// Everything, front first (for picking what's under the pointer)
    pub(crate) fn front_to_back(&self) -> Vec<Sel> {
        let mut v: Vec<Sel> = self.paint_steps().into_iter().filter_map(|s| if let Step::Item(s) = s { Some(s) } else { None }).collect();
        v.reverse();
        v
    }

    /// Bring to front (`front`) / send to back: the objects move to the top / bottom of
    /// their own frame's list, and that list is placed by hand from then on
    pub(crate) fn restack(&mut self, ids: &HashSet<String>, front: bool) {
        let (roots, kids) = self.z_lists();
        for list in std::iter::once(&roots).chain(kids.values()) {
            if !list.iter().any(|s| ids.contains(&s.1)) {
                continue;
            }
            let (moved, rest): (Vec<&Sel>, Vec<&Sel>) = list.iter().partition(|s| ids.contains(&s.1));
            let order: Vec<&Sel> = if front { moved.into_iter().chain(rest).collect() } else { rest.into_iter().chain(moved).collect() };
            for (n, s) in order.into_iter().enumerate() {
                self.board.layer_order.insert(s.1.clone(), n as f32);
            }
        }
        self.mark_dirty();
    }

    /// The object under `p` that's in front, in the shared order: what you see is what you
    /// click. `drag`: frames only catch their header / edge (so content inside stays easy
    /// to grab and a press inside empty space can start a selection box)
    pub(crate) fn hit_ordered(&self, p: Pt, drag: bool) -> Option<Sel> {
        let bl = self.blocked_at(p);
        let b = &self.board;
        let r = 6.0 / b.camera.zoom.max(0.01);
        let filled = |s: &Stroke, q: Pt| s.fill && s.closed_len().is_some() && pts_bbox(&s.points).is_some_and(|[x, y, w, h]| inside(q, x, y, w, h));
        let in_rect = |k: Kind, id: &str| self.rect_of(k, id).is_some_and(|[x, y, w, h]| inside(p, x, y, w, h));
        for (k, id) in self.front_to_back() {
            if bl.contains(&id) {
                continue;
            }
            let hit = match k {
                Kind::Stroke => b.world_strokes.iter().find(|s| s.id == id).is_some_and(|s| stroke_hit(s, p, r + s.width / 2.0) || filled(s, p)),
                Kind::Text => b.world_texts.iter().find(|l| l.id == id).is_some_and(|l| label_hit(l, p, 4.0)),
                Kind::Note | Kind::Link => in_rect(k, &id),
                Kind::Image => {
                    let Some(img) = b.images.iter().find(|i| i.id == id).filter(|i| inside(p, i.x, i.y, i.width, i.height)) else { continue };
                    // a drawing on the picture is picked before the picture
                    let (sx, sy) = img.scale();
                    let lp = Pt { x: (p.x - img.x) / sx, y: (p.y - img.y) / sy };
                    if let Some(s) = img.annotations.strokes.iter().rev().find(|s| stroke_hit(s, lp, (r + s.width / 2.0) / sx) || filled(s, lp)) {
                        return Some((Kind::Stroke, s.id.clone()));
                    }
                    true
                }
                Kind::Group => b.group_cards.iter().find(|g| g.id == id).is_some_and(|g| if drag { self.frame_hit_drag(g, p) } else { inside(p, g.x, g.y, g.width, g.height) }),
            };
            if hit {
                return Some((k, id));
            }
        }
        None
    }
}
