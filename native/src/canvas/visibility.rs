//! Hide / lock from the outline. Frames pass their state to everything nested inside them.
use crate::*;
use std::collections::HashSet;

impl App {
    /// Ids in `own` plus everything geometrically inside a frame in `own`
    fn spread(&self, own: &HashSet<String>) -> HashSet<String> {
        let mut out = own.clone();
        if own.is_empty() {
            return out;
        }
        for g in self.board.group_cards.iter().filter(|g| own.contains(&g.id)) {
            out.extend(self.descendants(&g.id).into_iter().map(|s| s.1));
        }
        out
    }

    /// Everything not drawn on the canvas
    pub(crate) fn hidden_ids(&self) -> HashSet<String> {
        self.spread(&self.board.hidden)
    }

    /// Everything the canvas can't pick (hidden or locked)
    pub(crate) fn blocked_ids(&self) -> HashSet<String> {
        let mut s = self.spread(&self.board.locked);
        s.extend(self.hidden_ids());
        s
    }

    /// Flip hide (`lock == false`) or lock on one object; whatever it covers drops out of the selection
    pub(crate) fn toggle_vis(&mut self, k: Kind, id: &str, lock: bool) {
        self.snapshot();
        let set = if lock { &mut self.board.locked } else { &mut self.board.hidden };
        if !set.remove(id) {
            set.insert(id.to_string());
            let gone = self.blocked_ids();
            self.selected.retain(|s| !gone.contains(&s.1));
            if self.sel.as_ref().is_some_and(|s| gone.contains(&s.1)) {
                self.sel = self.selected.iter().next().cloned();
            }
        }
        let _ = k;
        self.mark_dirty();
    }
}
