//! Which frame an object belongs to. Stored per object (`board.parents`: id -> frame id, "" for
//! the board) instead of guessed from position, so a frame keeps its children when it grows,
//! and a child can be bigger than its frame (the frame clips it).
use crate::*;
use std::collections::HashMap;
use std::sync::Mutex;

/// Cursor (world) of the canvas drag in progress
static DROP_AT: Mutex<Option<[f32; 2]>> = Mutex::new(None);

fn center_in(r: [f32; 4], f: [f32; 4]) -> bool {
    let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    cx >= f[0] && cx <= f[0] + f[2] && cy >= f[1] && cy <= f[1] + f[3]
}
fn within(r: [f32; 4], f: [f32; 4]) -> bool {
    r[0] >= f[0] && r[1] >= f[1] && r[0] + r[2] <= f[0] + f[2] && r[1] + r[3] <= f[1] + f[3]
}
fn touches(r: [f32; 4], f: [f32; 4]) -> bool {
    r[0] < f[0] + f[2] && r[0] + r[2] > f[0] && r[1] < f[1] + f[3] && r[1] + r[3] > f[1]
}

impl App {
    /// The frame `id` sits in, if any
    pub(crate) fn parent_of(&self, id: &str) -> Option<&str> {
        let p = self.board.parents.get(id)?;
        (!p.is_empty() && p != id && self.board.group_cards.iter().any(|g| &g.id == p)).then_some(p.as_str())
    }

    /// True when `id` is `frame` or somewhere inside it
    fn under(&self, id: &str, frame: &str) -> bool {
        let mut cur = Some(id);
        for _ in 0..64 {
            match cur {
                Some(c) if c == frame => return true,
                Some(c) => cur = self.parent_of(c),
                None => return false,
            }
        }
        false
    }

    /// Everything inside a frame, at any depth
    pub(crate) fn descendants(&self, frame: &str) -> Vec<Sel> {
        self.all_items().into_iter().filter(|s| s.1 != frame && self.under(&s.1, frame)).collect()
    }

    /// The smallest frame whose area holds `r`'s centre (`full`: holds all of `r`), skipping
    /// `id` itself and anything inside it
    fn frame_for(&self, id: &str, r: [f32; 4], full: bool) -> String {
        let is_frame = self.board.group_cards.iter().any(|g| g.id == id);
        let mut best: Option<(&Group, f32)> = None;
        for g in &self.board.group_cards {
            let f = [g.x, g.y, g.width, g.height];
            let a = g.width * g.height;
            // A frame only goes into a bigger frame; other objects may be larger than their frame (clipped)
            let ok = if full { within(r, f) && (a > r[2] * r[3] || g.id.as_str() < id) } else { center_in(r, f) && (!is_frame || a > r[2] * r[3]) };
            if !ok || self.board.hidden.contains(&g.id) || self.under(&g.id, id) {
                continue;
            }
            if best.map_or(true, |(_, ba)| a < ba) {
                best = Some((g, a));
            }
        }
        best.map(|(g, _)| g.id.clone()).unwrap_or_default()
    }

    /// Run every frame: new objects join the frame they were made in (new frames clip), and an
    /// object that ended up completely outside its frame is re-homed so it can't get lost
    pub(crate) fn stamp_parents(&mut self) {
        if !matches!(self.drag, Drag::None | Drag::Frame { .. } | Drag::Draw) {
            return;
        }
        for (k, id) in self.all_items() {
            let Some(r) = self.rect_of(k, &id) else { continue };
            match self.board.parents.get(&id) {
                None => {
                    // New frames join by their whole box (a frame only nests in a bigger one); anything else
                    // joins the frame under its centre, like a canvas drop
                    let p = self.frame_for(&id, r, k == Kind::Group);
                    self.board.parents.insert(id.clone(), p);
                    if k == Kind::Group {
                        if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                            g.clip = true;
                        }
                    }
                }
                Some(p) if !p.is_empty() && matches!(self.drag, Drag::None) => {
                    let gone = match self.rect_of(Kind::Group, p) {
                        Some(f) => !touches(r, f),
                        None => true,
                    };
                    if gone {
                        let np = self.frame_for(&id, r, false);
                        self.board.parents.insert(id, np);
                    }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn set_drop_at(c: Option<[f32; 2]>) {
        *DROP_AT.lock().unwrap() = c;
    }

    /// The frame under the cursor that the dragged objects go into ("" = the board). Like
    /// Figma, the cursor decides, not the object's box.
    fn drop_frame(&self, c: [f32; 2], moved: &[Sel]) -> String {
        // A dragged frame only goes into a bigger frame
        let big = moved.iter().filter(|s| s.0 == Kind::Group).filter_map(|s| self.rect_of(s.0, &s.1)).map(|r| r[2] * r[3]).fold(0.0, f32::max);
        let mut best: Option<(&Group, f32)> = None;
        for g in &self.board.group_cards {
            let a = g.width * g.height;
            let inside = c[0] >= g.x && c[0] <= g.x + g.width && c[1] >= g.y && c[1] <= g.y + g.height;
            if !inside || a <= big || self.board.hidden.contains(&g.id) || moved.iter().any(|m| self.under(&g.id, &m.1)) {
                continue;
            }
            if best.map_or(true, |(_, ba)| a < ba) {
                best = Some((g, a));
            }
        }
        best.map(|(g, _)| g.id.clone()).unwrap_or_default()
    }

    /// While dragging on the canvas: the dragged objects join the frame the cursor is in (or
    /// leave to the board). Objects inside a moved frame stay with it.
    pub(crate) fn rehome(&mut self, c: [f32; 2], moved: &[Sel]) {
        Self::set_drop_at(Some(c));
        let p = self.drop_frame(c, moved);
        for (_, id) in moved {
            if self.parent_of(id).is_some_and(|q| moved.iter().any(|m| m.1 == q)) {
                continue;
            }
            self.board.parents.insert(id.clone(), p.clone());
        }
    }

    /// The frame to highlight while a canvas drag is over it
    pub(crate) fn drop_target(&self) -> Option<String> {
        if !matches!(self.drag, Drag::Move { .. }) {
            return None;
        }
        let c = (*DROP_AT.lock().unwrap())?;
        let moved: Vec<Sel> = self.selected.iter().cloned().collect();
        Some(self.drop_frame(c, &moved)).filter(|f| !f.is_empty())
    }

    /// A frame that was just drawn takes in the objects it fully surrounds
    pub(crate) fn adopt_enclosed(&mut self, frame: &str) {
        let Some(f) = self.rect_of(Kind::Group, frame) else { return };
        let home = self.board.parents.get(frame).cloned().unwrap_or_default();
        for (k, id) in self.all_items() {
            if id == frame || self.board.parents.get(&id).cloned().unwrap_or_default() != home {
                continue;
            }
            if self.rect_of(k, &id).is_some_and(|r| within(r, f)) {
                self.board.parents.insert(id, frame.to_string());
            }
        }
    }

    /// Frames in paint order: list order, but every frame right after the frame it sits in
    pub(crate) fn frames_paint_order(&self) -> Vec<&Group> {
        let gs = &self.board.group_cards;
        let mut out: Vec<&Group> = Vec::with_capacity(gs.len());
        let mut stack: Vec<&Group> = gs.iter().filter(|g| self.parent_of(&g.id).is_none()).rev().collect();
        while let Some(g) = stack.pop() {
            out.push(g);
            stack.extend(gs.iter().filter(|c| self.parent_of(&c.id) == Some(g.id.as_str())).rev());
        }
        // Anything left over (a parent loop) still paints
        if out.len() < gs.len() {
            for g in gs {
                if !out.iter().any(|o| o.id == g.id) {
                    out.push(g);
                }
            }
        }
        out
    }

    /// Opacity of one object on its own (1 = solid)
    pub(crate) fn opacity_of(&self, id: &str) -> f32 {
        self.board.opacity.get(id).copied().unwrap_or(1.0).clamp(0.0, 1.0)
    }

    /// Objects drawn faded: their own opacity times every frame's around them
    pub(crate) fn fades(&self) -> HashMap<String, f32> {
        let mut out = HashMap::new();
        if self.board.opacity.is_empty() {
            return out;
        }
        for (_, id) in self.all_items() {
            let mut o = self.opacity_of(&id);
            let mut cur = self.parent_of(&id);
            for _ in 0..64 {
                let Some(pid) = cur else { break };
                o *= self.opacity_of(pid);
                cur = self.parent_of(pid);
            }
            if o < 0.999 {
                out.insert(id, o);
            }
        }
        out
    }

    /// World-space clip box per object: the overlap of every clipping frame around it
    /// How far in (world units) a frame crops its contents: to the inner edge of its stroke,
    /// tucked half a pixel under it so no seam shows
    pub(crate) fn clip_inset(&self, g: &Group) -> f32 {
        let bw = g.border.unwrap_or(1.0).max(0.0);
        let px = match g.stroke_pos {
            1 => bw,
            2 => 0.0,
            _ => bw / 2.0,
        };
        (px - 0.5).max(0.0) / self.board.camera.zoom.max(0.0001)
    }

    /// A clipping frame's crop box (world): its rect pulled in to the stroke's inner edge
    pub(crate) fn crop_box(&self, g: &Group) -> [f32; 4] {
        let i = self.clip_inset(g).min(g.width / 2.0).min(g.height / 2.0);
        [g.x + i, g.y + i, g.width - 2.0 * i, g.height - 2.0 * i]
    }

    pub(crate) fn clip_boxes(&self) -> HashMap<String, [f32; 4]> {
        let frames: HashMap<&str, &Group> = self.board.group_cards.iter().map(|g| (g.id.as_str(), g)).collect();
        let mut out = HashMap::new();
        for (id, _) in &self.board.parents {
            let mut c: Option<[f32; 4]> = None;
            let mut cur = self.parent_of(id);
            for _ in 0..64 {
                let Some(pid) = cur else { break };
                if let Some(g) = frames.get(pid).filter(|g| g.clip) {
                    let b = self.crop_box(g);
                    c = Some(match c {
                        None => b,
                        Some(a) => {
                            let (x0, y0) = (a[0].max(b[0]), a[1].max(b[1]));
                            let (x1, y1) = ((a[0] + a[2]).min(b[0] + b[2]), (a[1] + a[3]).min(b[1] + b[3]));
                            [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
                        }
                    });
                }
                cur = self.parent_of(pid);
            }
            if let Some(c) = c {
                out.insert(id.clone(), c);
            }
        }
        out
    }
}
