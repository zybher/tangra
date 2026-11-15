//! Double-click a number on a bar or in a dropdown to type it. No text box: the value stays
//! where it is, an accent ring eases in around it, the digits are washed in accent (all
//! selected, so typing replaces them) and a soft caret breathes at their end. Enter or a click
//! elsewhere sets it, Esc puts it back, Up / Down nudge it (Shift = 10). The new value then
//! counts / morphs to its target instead of jumping.
use crate::*;
use egui::{Event, EventFilter, Key};

#[derive(Clone)]
struct St {
    buf: String,
    all: bool,
    orig: f32,
}

/// Morph time from the old value to the typed one
const TWEEN: f64 = 0.32;

pub(crate) struct NumEdit {
    id: egui::Id,
    /// 0..1 how far into edit mode
    pub e: f32,
    /// Shown instead of the value while editing (no unit suffix)
    pub text: Option<String>,
    sel: f32,
    /// A value to apply this frame (the morph toward a typed / nudged value)
    pub apply: Option<f32>,
    /// `apply` is the first step of a new change (take the undo snapshot)
    pub first: bool,
}

fn act_id() -> egui::Id {
    egui::Id::new("qd_num_edit_active")
}

/// Some number is being typed (bars hold their magnify still meanwhile)
pub(crate) fn active(ctx: &egui::Context) -> bool {
    let last: u64 = ctx.data(|d| d.get_temp(act_id())).unwrap_or(0);
    last + 1 >= ctx.cumulative_pass_nr() && last != 0
}

impl NumEdit {
    /// `resp`: the number's widget (its id takes keyboard focus). `live`: it can be edited now.
    pub(crate) fn run(ctx: &egui::Context, resp: &egui::Response, live: bool, val: f32, lo: f32, hi: f32) -> Self {
        let id = resp.id;
        let sid = id.with("ne");
        let tid = id.with("ne_tw");
        let now = ctx.input(|i| i.time);
        let mut st: Option<St> = ctx.data(|d| d.get_temp(sid));
        let mut target: Option<f32> = None;
        if live && st.is_none() && resp.double_clicked() {
            st = Some(St { buf: format!("{}", val.round() as i32), all: true, orig: val });
            ctx.memory_mut(|m| m.request_focus(id));
            ctx.data_mut(|d| d.remove::<(f32, f32, f64)>(tid));
        }
        let parse = |s: &St| s.buf.parse::<f32>().ok().map(|v| v.clamp(lo, hi));
        let mut end = false;
        if let Some(s) = st.as_mut() {
            if !live || !ctx.memory(|m| m.has_focus(id)) || resp.drag_started() {
                // Clicked elsewhere / went away: keep what was typed (a drag takes over instead)
                if !resp.drag_started() {
                    target = parse(s);
                }
                end = true;
            } else {
                ctx.memory_mut(|m| m.set_focus_lock_filter(id, EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true }));
                let evs = ctx.input(|i| i.events.clone());
                for ev in evs {
                    match ev {
                        Event::Text(t) => {
                            for ch in t.chars() {
                                let ok = ch.is_ascii_digit() || (ch == '-' && lo < 0.0 && (s.all || s.buf.is_empty()));
                                if !ok {
                                    continue;
                                }
                                if s.all {
                                    s.buf.clear();
                                    s.all = false;
                                }
                                if s.buf.len() < 5 {
                                    s.buf.push(ch);
                                }
                            }
                        }
                        Event::Key { key, pressed: true, modifiers, .. } => match key {
                            Key::Backspace | Key::Delete => {
                                if s.all {
                                    s.buf.clear();
                                    s.all = false;
                                } else {
                                    s.buf.pop();
                                }
                            }
                            Key::Enter | Key::Tab => {
                                target = parse(s);
                                end = true;
                            }
                            Key::Escape => {
                                target = Some(s.orig);
                                end = true;
                            }
                            Key::ArrowUp | Key::ArrowDown => {
                                let step = if modifiers.shift { 10.0 } else { 1.0 } * if key == Key::ArrowUp { 1.0 } else { -1.0 };
                                let v = (parse(s).unwrap_or(val).round() + step).clamp(lo, hi);
                                s.buf = format!("{}", v as i32);
                                s.all = false;
                                target = Some(v);
                            }
                            Key::A if modifiers.command => s.all = true,
                            _ => {}
                        },
                        _ => {}
                    }
                }
                // The canvas doesn't see these keys
                ctx.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Key { .. } | Event::Text(_))));
            }
        }
        if end {
            st = None;
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        match &st {
            Some(s) => {
                ctx.data_mut(|d| {
                    d.insert_temp(sid, s.clone());
                    d.insert_temp(act_id(), ctx.cumulative_pass_nr());
                });
                ctx.request_repaint();
            }
            None => ctx.data_mut(|d| d.remove::<St>(sid)),
        }

        // Morph from the current value to the new one
        let mut first = false;
        if let Some(v) = target {
            if (v - val).abs() > 1e-3 {
                ctx.data_mut(|d| d.insert_temp(tid, (val, v, now)));
                first = true;
            }
        }
        if resp.dragged() {
            ctx.data_mut(|d| d.remove::<(f32, f32, f64)>(tid));
        }
        let mut apply = None;
        if let Some((a, b, t0)) = ctx.data(|d| d.get_temp::<(f32, f32, f64)>(tid)) {
            let x = (((now - t0) / (TWEEN * anim_k().max(1e-3) as f64)) as f32).clamp(0.0, 1.0);
            let k = 1.0 - (1.0 - x).powi(3);
            apply = Some(if x >= 1.0 { b } else { a + (b - a) * k });
            if x >= 1.0 {
                ctx.data_mut(|d| d.remove::<(f32, f32, f64)>(tid));
            } else {
                ctx.request_repaint();
            }
        }

        let e = ease(ctx, id.with("ne_e"), st.is_some());
        let sel = ease(ctx, id.with("ne_sel"), st.as_ref().is_some_and(|s| s.all));
        let text = st.as_ref().map(|s| s.buf.clone());
        Self { id, e, text, sel, apply, first }
    }

    /// Ring, selection wash and caret. `right`: the number's right-centre (it is right-aligned).
    /// `ring`: the pill to outline, or None to ring just the number.
    pub(crate) fn draw(&self, p: &egui::Painter, ring: Option<Rect>, right: Pos2, size: f32) {
        if self.e < 0.002 {
            return;
        }
        let ctx = p.ctx();
        let txt = self.text.clone().unwrap_or_default();
        let w = ctx.fonts(|f| f.layout_no_wrap(txt, FontId::proportional(size), Color32::WHITE).size().x);
        // The wash glides to the new width as digits are typed
        let wid = self.id.with("ne_w");
        let dt = ctx.input(|i| i.stable_dt).min(0.05);
        let cur: f32 = ctx.data(|d| d.get_temp(wid)).unwrap_or(w);
        let gw = cur + (w - cur) * (1.0 - (-dt * 16.0).exp());
        ctx.data_mut(|d| d.insert_temp(wid, gw));
        let h = size * 0.75;
        let ring = ring.unwrap_or_else(|| Rect::from_min_max(pos2(right.x - gw.max(size) - 8.0, right.y - h - 3.0), pos2(right.x + 6.0, right.y + h + 3.0)));
        p.rect_stroke(ring.shrink(0.75), 8.0, egui::Stroke::new(1.25, blue().gamma_multiply(self.e)));
        if self.sel > 0.01 && gw > 0.5 {
            let r = Rect::from_min_max(pos2(right.x - gw - 3.0, right.y - h), pos2(right.x + 3.0, right.y + h));
            p.rect_filled(r, 4.0, blue().gamma_multiply(0.45 * self.sel * self.e));
        }
        // A caret that breathes rather than blinks
        let t = ctx.input(|i| i.time) as f32;
        let a = self.e * (1.0 - self.sel) * (0.62 + 0.38 * (t * 4.2).cos());
        if a > 0.01 {
            let x = right.x + 1.5;
            p.line_segment([pos2(x, right.y - h + 1.0), pos2(x, right.y + h - 1.0)], egui::Stroke::new(1.5, Color32::WHITE.gamma_multiply(a)));
        }
    }
}
