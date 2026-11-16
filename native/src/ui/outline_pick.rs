//! Clicking rows in the outline: a plain click selects just that row (and brings it into
//! view), Shift selects every listed row from the last plain-clicked one to here, Ctrl / Cmd
//! adds or removes one row at a time (Ctrl + Shift adds the range to what's selected)
use crate::*;

impl App {
    pub(crate) fn outline_pick(&mut self, ctx: &egui::Context, s: Sel, shift: bool, ctrl: bool, order: &[Sel], rect_of: impl Fn(&Sel) -> Option<[f32; 4]>, canvas: egui::Rect) {
        let aid = egui::Id::new("qd_outline_anchor");
        let anchor: Option<Sel> = ctx.data(|d| d.get_temp(aid)).filter(|a| order.contains(a)).or_else(|| self.sel.clone().filter(|a| order.contains(a)));
        if shift {
            let to = order.iter().position(|x| *x == s);
            let from = anchor.as_ref().and_then(|a| order.iter().position(|x| x == a)).or(to);
            if let (Some(a), Some(b)) = (from, to) {
                if !ctrl {
                    self.selected.clear();
                }
                for x in &order[a.min(b)..=a.max(b)] {
                    self.selected.insert(x.clone());
                }
                self.sel = Some(s);
                self.sel_label = None;
                // the anchor stays put, so the next Shift-click re-spans from it
                if let Some(a) = anchor {
                    ctx.data_mut(|d| d.insert_temp(aid, a));
                }
                return;
            }
        }
        if ctrl {
            if self.selected.remove(&s) {
                if self.sel.as_ref() == Some(&s) {
                    self.sel = order.iter().rev().find(|x| self.selected.contains(*x)).cloned().or_else(|| self.selected.iter().next().cloned());
                }
            } else {
                self.selected.insert(s.clone());
                self.sel = Some(s.clone());
            }
            self.sel_label = None;
            ctx.data_mut(|d| d.insert_temp(aid, s));
            return;
        }
        if let Some(r) = rect_of(&s) {
            self.outline_focus(r, canvas);
        }
        ctx.data_mut(|d| d.insert_temp(aid, s.clone()));
        self.select_only(s);
    }
}
