//! Frame-first picking (Figma): a click on anything inside a frame picks the outermost frame
//! that isn't "open" yet. A frame counts as open once something inside it is selected, so
//! after you've stepped in, clicks pick siblings at that level. Double-click steps one level
//! deeper; holding Ctrl picks the deepest object straight away.
use crate::*;
use std::collections::HashSet;

impl App {
    /// `hit` is the deepest object under the pointer; returns what a click should select.
    /// `deeper`: also treat the selected frames themselves as open (double-click)
    pub(crate) fn pick(&self, hit: Sel, deeper: bool) -> Sel {
        if CTRL_DOWN.load(std::sync::atomic::Ordering::Relaxed) {
            return hit;
        }
        let mut open: HashSet<String> = HashSet::new();
        for (k, id) in &self.selected {
            if deeper && *k == Kind::Group {
                open.insert(id.clone());
            }
            let mut cur = self.parent_of(id).map(str::to_string);
            for _ in 0..64 {
                let Some(p) = cur else { break };
                cur = self.parent_of(&p).map(str::to_string);
                open.insert(p);
            }
        }
        let mut at = hit;
        for _ in 0..64 {
            match self.parent_of(&at.1) {
                Some(p) if !open.contains(p) => at = (Kind::Group, p.to_string()),
                _ => break,
            }
        }
        at
    }
}
