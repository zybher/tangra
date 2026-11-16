//! Undo/redo history, saving, selection and label-edit bookkeeping.

use crate::*;

impl App {
    pub(crate) fn view(&self, rect: Rect) -> View {
        View { origin: rect.min, cam: self.board.camera }
    }

    pub(crate) fn mark_dirty(&mut self) {
        if self.dirty_since.is_none() {
            self.dirty_since = Some(self.now);
        }
    }

    pub(crate) fn push_undo(&mut self, b: Board) {
        self.undo.push(b);
        if self.undo.len() > 50 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.mark_dirty();
    }

    /// Call right before changing the board.
    pub(crate) fn snapshot(&mut self) {
        // Items still fading out of a delete are gone (and glides landed) before this step is recorded
        self.finish_dying();
        self.land_glides();
        let b = self.board.clone();
        self.push_undo(b);
    }

    pub(crate) fn save_now(&mut self) {
        if self.dirty_since.take().is_some() {
            self.board.updated_at = now_ms();
            super::saver::save_async(self.board.clone());
        }
        if let Some(e) = super::saver::take_error() {
            self.toast(format!("Save failed: {e}"));
        }
    }

    pub(crate) fn select_only(&mut self, s: Sel) {
        self.selected.clear();
        self.selected.insert(s.clone());
        self.sel = Some(s);
        self.sel_label = None;
    }

    /// The text label on an image under a world point, as (image id, label id)
    pub(crate) fn label_at(&self, w: Pt) -> Option<(String, String)> {
        let img = self.image_at(w)?;
        let (sx, sy) = img.scale();
        let lp = Pt { x: (w.x - img.x) / sx, y: (w.y - img.y) / sy };
        img.annotations
            .texts
            .iter()
            .rev()
            .find(|l| label_hit(l, lp, 4.0 / sx.max(0.01)))
            .map(|l| (img.id.clone(), l.id.clone()))
    }

    pub(crate) fn sel_label_mut(&mut self) -> Option<(&mut Label, f32)> {
        let (image, label) = self.sel_label.clone()?;
        let img = self.board.images.iter_mut().find(|i| i.id == image)?;
        let sx = img.scale().0;
        img.annotations.texts.iter_mut().find(|l| l.id == label).map(|l| (l, sx))
    }

    pub(crate) fn edit_label(&mut self, (image, label): (String, String)) {
        let Some(img) = self.board.images.iter().find(|i| i.id == image) else { return };
        let (sx, sy) = img.scale();
        let Some(l) = img.annotations.texts.iter().find(|l| l.id == label) else { return };
        self.edit = Some(Edit {
            world: Pt { x: img.x + l.x * sx, y: img.y + l.y * sy },
            text: l.text.clone(),
            fresh: 2,
            target: EditTarget::ImageLabel { image, label },
        });
    }

    pub(crate) fn deselect_all(&mut self) {
        self.selected.clear();
        self.sel = None;
        self.sel_label = None;
    }

    pub(crate) fn toggle_select(&mut self, s: Sel) {
        if self.selected.contains(&s) {
            self.selected.remove(&s);
            if self.sel.as_ref() == Some(&s) {
                self.sel = self.selected.iter().next().cloned();
            }
        } else {
            self.selected.insert(s.clone());
            self.sel = Some(s);
        }
    }

    pub(crate) fn undo(&mut self) {
        self.finish_dying();
        self.land_glides();
        if let Some(prev) = self.undo.pop() {
            let cam = self.board.camera;
            let rects = self.item_rects();
            let cur = std::mem::replace(&mut self.board, prev);
            self.after_history_jump(&cur, &rects);
            self.redo.push(cur);
            self.board.camera = cam;
            self.deselect_all();
            self.mark_dirty();
        }
    }

    pub(crate) fn redo(&mut self) {
        self.finish_dying();
        self.land_glides();
        if let Some(next) = self.redo.pop() {
            let cam = self.board.camera;
            let rects = self.item_rects();
            let cur = std::mem::replace(&mut self.board, next);
            self.after_history_jump(&cur, &rects);
            self.undo.push(cur);
            self.board.camera = cam;
            self.deselect_all();
            self.mark_dirty();
        }
    }

    /// Undo / redo don't teleport (audit #29): items that come back fade in, items that go
    /// fade out, and moved / resized ones glide from where they were
    fn after_history_jump(&mut self, old: &Board, old_rects: &std::collections::HashMap<Sel, [f32; 4]>) {
        use crate::canvas::presence;
        let (was, now_ids) = (presence::board_ids(old), presence::board_ids(&self.board));
        presence::enter(now_ids.difference(&was).cloned(), self.now, 0.0);
        // Gone items: put back for the length of their fade, then removed (tick_presence)
        let gone: Vec<String> = was.difference(&now_ids).cloned().collect();
        if !gone.is_empty() && anim_k() > 0.0 {
            let mut ghost = old.clone();
            let keep: std::collections::HashSet<String> = gone.iter().cloned().collect();
            // only top-level kinds come back as ghosts (annotation marks just go)
            ghost.images.retain(|i| keep.contains(&i.id));
            ghost.link_cards.retain(|i| keep.contains(&i.id));
            ghost.group_cards.retain(|i| keep.contains(&i.id));
            ghost.world_texts.retain(|i| keep.contains(&i.id));
            ghost.world_strokes.retain(|i| keep.contains(&i.id));
            self.board.images.extend(ghost.images);
            self.board.link_cards.extend(ghost.link_cards);
            // frames go underneath, as they are painted first anyway
            self.board.group_cards.extend(ghost.group_cards);
            self.board.world_texts.extend(ghost.world_texts);
            self.board.world_strokes.extend(ghost.world_strokes);
            presence::kill(gone, self.now);
        }
        // Moved / resized: start at the old box and glide (canvas/glide.rs)
        let mut moves = Vec::new();
        for (k, id) in self.all_items() {
            if let (Some(to), Some(from)) = (self.rect_of(k, &id), old_rects.get(&(k, id.clone())).copied()) {
                if (0..4).any(|i| (to[i] - from[i]).abs() > 0.01) {
                    self.set_rect(k, &id, from);
                    moves.push(((k, id), to));
                }
            }
        }
        if !moves.is_empty() {
            self.glide_to(moves);
        }
    }

    /// Every top-level item's box
    fn item_rects(&self) -> std::collections::HashMap<Sel, [f32; 4]> {
        self.all_items().into_iter().filter_map(|s| self.rect_of(s.0, &s.1).map(|r| (s, r))).collect()
    }
}
