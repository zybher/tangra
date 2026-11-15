//! Geometry and hit-testing: item rects, strokes, frames, what is under the pointer.

use crate::*;

impl App {
    pub(crate) fn rect_of(&self, k: Kind, id: &str) -> Option<[f32; 4]> {
        macro_rules! g {
            ($v:expr) => {
                $v.iter().find(|i| i.id == id).map(|i| [i.x, i.y, i.width, i.height])
            };
        }
        match k {
            Kind::Image => g!(self.board.images),
            Kind::Note => g!(self.board.sticky_notes),
            Kind::Link => g!(self.board.link_cards),
            Kind::Group => g!(self.board.group_cards),
            Kind::Text => self.board.world_texts.iter().find(|i| i.id == id).map(|i| {
                let (w, h) = text_wh(&i.text, i.font_size);
                [i.x, i.y, w, h]
            }),
            Kind::Stroke => self.stroke_rect(id),
        }
    }

    /// World-space bounding box of a stroke, wherever it lives.
    pub(crate) fn stroke_rect(&self, id: &str) -> Option<[f32; 4]> {
        if let Some(s) = self.board.world_strokes.iter().find(|s| s.id == id) {
            return pts_bbox(&s.points);
        }
        for img in &self.board.images {
            if let Some(s) = img.annotations.strokes.iter().find(|s| s.id == id) {
                let (sx, sy) = img.scale();
                return pts_bbox(&s.points).map(|[x, y, w, h]| [img.x + x * sx, img.y + y * sy, w * sx, h * sy]);
            }
        }
        None
    }

    /// Move and/or stretch a stroke so its bounding box becomes `to` (world space).
    pub(crate) fn fit_stroke(&mut self, id: &str, to: [f32; 4]) {
        let Some([ox, oy, ow, oh]) = self.stroke_rect(id) else { return };
        let (kx, ky) = (to[2] / ow.max(1e-3), to[3] / oh.max(1e-3));
        if let Some(s) = self.board.world_strokes.iter_mut().find(|s| s.id == id) {
            for p in &mut s.points {
                p.x = to[0] + (p.x - ox) * kx;
                p.y = to[1] + (p.y - oy) * ky;
            }
            return;
        }
        for img in &mut self.board.images {
            let (sx, sy) = img.scale();
            let (ix, iy) = (img.x, img.y);
            if let Some(s) = img.annotations.strokes.iter_mut().find(|s| s.id == id) {
                for p in &mut s.points {
                    p.x = (to[0] + (ix + p.x * sx - ox) * kx - ix) / sx;
                    p.y = (to[1] + (iy + p.y * sy - oy) * ky - iy) / sy;
                }
                return;
            }
        }
    }

    pub(crate) fn find_stroke(&self, id: &str) -> Option<&Stroke> {
        let b = &self.board;
        b.world_strokes
            .iter()
            .chain(b.images.iter().flat_map(|i| i.annotations.strokes.iter()))
            .find(|s| s.id == id)
    }

    /// Flip fill / outline on the selected shapes; never leaves a shape with neither.
    pub(crate) fn toggle_style(&mut self, fill: bool) {
        self.snapshot();
        let ids: Vec<String> = self.selected.iter().filter(|s| s.0 == Kind::Stroke).map(|s| s.1.clone()).collect();
        let b = &mut self.board;
        let all = b.world_strokes.iter_mut().chain(b.images.iter_mut().flat_map(|i| i.annotations.strokes.iter_mut()));
        for s in all.filter(|s| ids.contains(&s.id)) {
            if fill {
                s.fill = !s.fill;
                if !s.fill {
                    s.no_stroke = false;
                }
            } else {
                s.no_stroke = !s.no_stroke;
                if s.no_stroke {
                    s.fill = true;
                }
            }
        }
    }

    /// Put a freshly pasted image inside a frame: next to the images already
    /// there, growing the frame to fit.
    pub(crate) fn place_in_frame(&mut self, gid: &str, img_id: &str) {
        let b = &mut self.board;
        let Some(g) = b.group_cards.iter_mut().find(|g| g.id == gid) else { return };
        let pad = 20.0;
        let mut right: Option<f32> = None;
        for i in b.images.iter().filter(|i| i.id != img_id) {
            if i.x >= g.x && i.y >= g.y && i.x + i.width <= g.x + g.width && i.y + i.height <= g.y + g.height {
                right = Some(right.map_or(i.x + i.width, |r| r.max(i.x + i.width)));
            }
        }
        let Some(img) = b.images.iter_mut().find(|i| i.id == img_id) else { return };
        img.x = right.map_or(g.x + pad, |r| r + pad);
        img.y = g.y + pad;
        g.width = g.width.max(img.x + img.width + pad - g.x);
        g.height = g.height.max(img.y + img.height + pad - g.y);
    }

    /// The stroke under the cursor (drawn on top of everything, so checked first).
    pub(crate) fn stroke_at(&self, p: Pt) -> Option<Sel> {
        let z = self.board.camera.zoom;
        let r = 6.0 / z;
        let filled = |s: &Stroke, q: Pt| {
            s.fill && s.closed_len().is_some() && pts_bbox(&s.points).is_some_and(|[x, y, w, h]| inside(q, x, y, w, h))
        };
        let bl = self.blocked_at(p);
        if let Some(s) = self.board.world_strokes.iter().rev().find(|s| !bl.contains(&s.id) && (stroke_hit(s, p, r + s.width / 2.0) || filled(s, p))) {
            return Some((Kind::Stroke, s.id.clone()));
        }
        let img = self.image_at(p)?;
        let (sx, sy) = img.scale();
        let lp = Pt { x: (p.x - img.x) / sx, y: (p.y - img.y) / sy };
        img.annotations
            .strokes
            .iter()
            .rev()
            .find(|s| stroke_hit(s, lp, (r + s.width / 2.0) / sx) || filled(s, lp))
            .map(|s| (Kind::Stroke, s.id.clone()))
    }

    pub(crate) fn set_rect(&mut self, k: Kind, id: &str, r: [f32; 4]) {
        macro_rules! s {
            ($v:expr) => {
                if let Some(i) = $v.iter_mut().find(|i| i.id == id) {
                    i.x = r[0];
                    i.y = r[1];
                    i.width = r[2];
                    i.height = r[3];
                }
            };
        }
        match k {
            Kind::Image => s!(self.board.images),
            Kind::Note => s!(self.board.sticky_notes),
            Kind::Link => s!(self.board.link_cards),
            Kind::Group => s!(self.board.group_cards),
            Kind::Text => {
                if let Some(i) = self.board.world_texts.iter_mut().find(|i| i.id == id) {
                    i.x = r[0];
                    i.y = r[1];
                }
            }
            Kind::Stroke => self.fit_stroke(id, r),
        }
    }

    pub(crate) fn image_at(&self, p: Pt) -> Option<&ImageNode> {
        let bl = self.blocked_at(p);
        self.board.images.iter().rev().find(|i| !bl.contains(&i.id) && inside(p, i.x, i.y, i.width, i.height))
    }

    pub(crate) fn hit_test(&self, p: Pt) -> Option<Sel> {
        let bl = self.blocked_at(p);
        let b = &self.board;
        // A frame's title (above it) selects the whole frame, like Figma; double-click still renames
        if let Some(t) = self.frame_title_at(p) {
            return Some(t);
        }
        if let Some(s) = self.stroke_at(p) {
            return Some(s);
        }
        if let Some(t) = b.world_texts.iter().rev().find(|l| !bl.contains(&l.id) && label_hit(l, p, 4.0)) {
            return Some((Kind::Text, t.id.clone()));
        }
        if let Some(n) = b.sticky_notes.iter().rev().find(|i| !bl.contains(&i.id) && inside(p, i.x, i.y, i.width, i.height)) {
            return Some((Kind::Note, n.id.clone()));
        }
        if let Some(n) = b.link_cards.iter().rev().find(|i| !bl.contains(&i.id) && inside(p, i.x, i.y, i.width, i.height)) {
            return Some((Kind::Link, n.id.clone()));
        }
        if let Some(n) = self.image_at(p) {
            return Some((Kind::Image, n.id.clone()));
        }
        // Topmost first: the same order frames paint in
        self.frames_paint_order()
            .into_iter()
            .rev()
            .find(|i| !bl.contains(&i.id) && inside(p, i.x, i.y, i.width, i.height))
            .map(|n| (Kind::Group, n.id.clone()))
    }

    pub(crate) fn frame_hit_drag(&self, g: &Group, p: Pt) -> bool {
        if self.selected.contains(&(Kind::Group, g.id.clone())) {
            return inside(p, g.x, g.y, g.width, g.height);
        }
        // The title sits just above the frame
        // Grab zones are in screen pixels so they feel the same at every zoom
        let z = self.board.camera.zoom.max(0.01);
        let in_header = p.x >= g.x && p.x <= g.x + g.width && p.y >= g.y - (FRAME_TITLE_PX + 14.0) / z && p.y <= g.y + 8.0 / z;
        let border_pad = 8.0 / z;
        let near_border = inside(p, g.x - border_pad, g.y - border_pad, g.width + border_pad * 2.0, g.height + border_pad * 2.0)
            && if g.width > border_pad * 2.0 && g.height > border_pad * 2.0 {
                !inside(p, g.x + border_pad, g.y + border_pad, g.width - border_pad * 2.0, g.height - border_pad * 2.0)
            } else {
                true
            };
        in_header || near_border
    }

    pub(crate) fn frame_title_at(&self, p: Pt) -> Option<Sel> {
        let z = self.board.camera.zoom.max(0.01);
        self.frames_paint_order()
            .into_iter()
            .rev()
            .find(|g| {
                if self.board.hidden.contains(&g.id) || self.board.locked.contains(&g.id) {
                    return false;
                }
                let th = (FRAME_TITLE_PX + 14.0 + g.border.unwrap_or(1.0) / 2.0) / z;
                p.x >= g.x && p.x <= g.x + g.width && p.y >= g.y - th && p.y <= g.y
            })
            .map(|g| (Kind::Group, g.id.clone()))
    }

    pub(crate) fn hit_test_drag(&self, p: Pt) -> Option<Sel> {
        let bl = self.blocked_at(p);
        let b = &self.board;
        // Dragging a frame by its title moves the whole frame
        if let Some(t) = self.frame_title_at(p) {
            return Some(t);
        }
        if let Some(s) = self.stroke_at(p) {
            return Some(s);
        }
        if let Some(t) = b.world_texts.iter().rev().find(|l| !bl.contains(&l.id) && label_hit(l, p, 4.0)) {
            return Some((Kind::Text, t.id.clone()));
        }
        if let Some(n) = b.sticky_notes.iter().rev().find(|i| !bl.contains(&i.id) && inside(p, i.x, i.y, i.width, i.height)) {
            return Some((Kind::Note, n.id.clone()));
        }
        if let Some(n) = b.link_cards.iter().rev().find(|i| !bl.contains(&i.id) && inside(p, i.x, i.y, i.width, i.height)) {
            return Some((Kind::Link, n.id.clone()));
        }
        if let Some(n) = self.image_at(p) {
            return Some((Kind::Image, n.id.clone()));
        }
        self.frames_paint_order()
            .into_iter()
            .rev()
            .find(|g| !bl.contains(&g.id) && self.frame_hit_drag(g, p))
            .map(|n| (Kind::Group, n.id.clone()))
    }

    pub(crate) fn items_in_rect(&self, box_r: [f32; 4]) -> Vec<Sel> {
        let mut hits = Vec::new();
        let b = &self.board;
        for img in &b.images {
            if rects_intersect(box_r, [img.x, img.y, img.width, img.height]) {
                hits.push((Kind::Image, img.id.clone()));
            }
        }
        for n in &b.sticky_notes {
            if rects_intersect(box_r, [n.x, n.y, n.width, n.height]) {
                hits.push((Kind::Note, n.id.clone()));
            }
        }
        for l in &b.link_cards {
            if rects_intersect(box_r, [l.x, l.y, l.width, l.height]) {
                hits.push((Kind::Link, l.id.clone()));
            }
        }
        for t in &b.world_texts {
            let (tw, th) = text_wh(&t.text, t.font_size);
            if rects_intersect(box_r, [t.x, t.y, tw, th]) {
                hits.push((Kind::Text, t.id.clone()));
            }
        }
        for s in &b.world_strokes {
            if pts_bbox(&s.points).is_some_and(|r| rects_intersect(box_r, r)) {
                hits.push((Kind::Stroke, s.id.clone()));
            }
        }
        for g in &b.group_cards {
            let box_inside_frame = box_r[0] >= g.x && box_r[0] + box_r[2] <= g.x + g.width &&
                                   box_r[1] >= g.y && box_r[1] + box_r[3] <= g.y + g.height;
            if !box_inside_frame && rects_intersect(box_r, [g.x, g.y, g.width, g.height]) {
                hits.push((Kind::Group, g.id.clone()));
            }
        }
        let bl = self.blocked_ids();
        hits.retain(|s| !bl.contains(&s.1));
        hits
    }

    // ---------- textures ----------
}
