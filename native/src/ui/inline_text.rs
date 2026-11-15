//! In-place renaming, in the same style as typing a number (num_edit.rs): no text box. The
//! name stays where it is drawn, an accent ring eases in, the text starts selected under an
//! accent wash, and a soft caret breathes where you type. The caret and the selection glide
//! between positions instead of jumping. Enter / Tab / a click elsewhere keeps it, Esc cancels.
//! Arrows, Home / End, Shift to select, Ctrl to jump words, Ctrl+A / C / X / V, click and drag
//! to place the caret or select.
use crate::*;
use egui::text::CCursor;
use egui::{Event, EventFilter, Key};

#[derive(Clone, Copy)]
struct St {
    caret: usize,
    anchor: usize,
}

/// The accent outline around the field
pub(crate) enum Ring {
    /// A fixed box (e.g. an outline row), corner radius
    Fixed(Rect, f32),
    /// Hugs the text: from `left` to just past the text, never past `max_right`; tinted inside
    Hug { left: f32, max_right: f32, rounding: f32 },
}

fn char_x(g: &egui::Galley, i: usize) -> f32 {
    g.pos_from_ccursor(CCursor::new(i)).min.x
}

/// Previous / next word boundary from `i`
fn word(cs: &[char], i: usize, back: bool) -> usize {
    let mut j = i;
    if back {
        while j > 0 && cs[j - 1].is_whitespace() {
            j -= 1;
        }
        while j > 0 && !cs[j - 1].is_whitespace() {
            j -= 1;
        }
    } else {
        while j < cs.len() && cs[j].is_whitespace() {
            j += 1;
        }
        while j < cs.len() && !cs[j].is_whitespace() {
            j += 1;
        }
    }
    j
}

/// Edits `buf` in `rect` (text left-aligned, vertically centred). `start`: the first frame
/// (selects everything, takes focus). Returns Some(true) to keep, Some(false) to cancel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn edit(ui: &mut egui::Ui, id: egui::Id, rect: Rect, buf: &mut String, font: FontId, col: Color32, start: bool, ring: Ring) -> Option<bool> {
    let ctx = ui.ctx().clone();
    let resp = ui.interact(rect, id, egui::Sense::click_and_drag());
    let sid = id.with("it");
    let n0 = buf.chars().count();
    let mut st: St = ctx.data(|d| d.get_temp(sid)).unwrap_or(St { caret: n0, anchor: 0 });
    if start {
        st = St { caret: n0, anchor: 0 };
        ctx.memory_mut(|m| m.request_focus(id));
    }
    let mut done: Option<bool> = None;
    let focused = ctx.memory(|m| m.has_focus(id));
    if !start && !focused {
        done = Some(true);
    }

    let galley = |s: &str| ctx.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), col));
    let clip_w = rect.width();
    // Horizontal scroll so the caret stays in view (long names)
    let scid = id.with("it_scroll");
    let scroll: f32 = ctx.data(|d| d.get_temp(scid)).unwrap_or(0.0);
    let origin = |g: &egui::Galley, scroll: f32| pos2(rect.min.x - scroll, rect.center().y - g.size().y / 2.0);

    if done.is_none() {
        ctx.memory_mut(|m| m.set_focus_lock_filter(id, EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true }));
        // Pointer: click places the caret, drag selects, double-click selects a word
        let g = galley(buf);
        let at = |q: Pos2| g.cursor_from_pos(q - origin(&g, scroll)).ccursor.index;
        if let Some(q) = resp.interact_pointer_pos() {
            if resp.double_clicked() {
                let cs: Vec<char> = buf.chars().collect();
                let i = at(q);
                st = St { anchor: word(&cs, i.min(cs.len()), true), caret: word(&cs, i.min(cs.len()), false) };
            } else if resp.is_pointer_button_down_on() && ctx.input(|i| i.pointer.any_pressed()) {
                let i = at(q);
                st = St { caret: i, anchor: if ctx.input(|i| i.modifiers.shift) { st.anchor } else { i } };
            } else if resp.dragged() {
                st.caret = at(q);
            }
        }
        // Keyboard
        let evs = ctx.input(|i| i.events.clone());
        for ev in evs {
            let mut cs: Vec<char> = buf.chars().collect();
            let (a, b) = (st.anchor.min(st.caret).min(cs.len()), st.anchor.max(st.caret).min(cs.len()));
            let insert = |cs: &mut Vec<char>, t: &str, st: &mut St| {
                cs.splice(a..b, t.chars().filter(|c| !c.is_control()));
                let p = a + t.chars().filter(|c| !c.is_control()).count();
                *st = St { caret: p, anchor: p };
            };
            match ev {
                Event::Text(t) => insert(&mut cs, &t, &mut st),
                Event::Paste(t) => insert(&mut cs, t.lines().next().unwrap_or(""), &mut st),
                Event::Copy => {
                    if a < b {
                        ctx.copy_text(cs[a..b].iter().collect());
                    }
                }
                Event::Cut => {
                    if a < b {
                        ctx.copy_text(cs[a..b].iter().collect());
                        insert(&mut cs, "", &mut st);
                    }
                }
                Event::Key { key, pressed: true, modifiers: m, .. } => {
                    let c = st.caret.min(cs.len());
                    let mv = |to: usize, st: &mut St| {
                        st.caret = to;
                        if !m.shift {
                            st.anchor = to;
                        }
                    };
                    match key {
                        Key::Enter | Key::Tab => done = Some(true),
                        Key::Escape => done = Some(false),
                        Key::A if m.command => st = St { anchor: 0, caret: cs.len() },
                        Key::Backspace | Key::Delete => {
                            if a < b {
                                insert(&mut cs, "", &mut st);
                            } else {
                                let back = key == Key::Backspace;
                                let to = if m.command { word(&cs, c, back) } else if back { c.saturating_sub(1) } else { (c + 1).min(cs.len()) };
                                let (x, y) = (c.min(to), c.max(to));
                                cs.drain(x..y);
                                st = St { caret: x, anchor: x };
                            }
                        }
                        Key::ArrowLeft | Key::ArrowRight => {
                            let back = key == Key::ArrowLeft;
                            let to = if m.command {
                                word(&cs, c, back)
                            } else if a < b && !m.shift {
                                if back { a } else { b }
                            } else if back {
                                c.saturating_sub(1)
                            } else {
                                (c + 1).min(cs.len())
                            };
                            mv(to, &mut st);
                        }
                        Key::Home | Key::ArrowUp => mv(0, &mut st),
                        Key::End | Key::ArrowDown => mv(cs.len(), &mut st),
                        _ => {}
                    }
                }
                _ => {}
            }
            *buf = cs.into_iter().collect();
        }
        // The canvas doesn't see these keys
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Key { .. } | Event::Text(_) | Event::Paste(_) | Event::Copy | Event::Cut)));
    }
    if done.is_some() {
        ctx.memory_mut(|m| m.surrender_focus(id));
        ctx.data_mut(|d| d.remove::<St>(sid));
    } else {
        ctx.data_mut(|d| d.insert_temp(sid, st));
        ctx.request_repaint();
    }

    // ---- drawing
    let g = galley(buf);
    let n = buf.chars().count();
    let (ci, ai) = (st.caret.min(n), st.anchor.min(n));
    let dt = ctx.input(|i| i.stable_dt).min(0.05);
    let f = 1.0 - (-dt * 16.0).exp();
    let glide = |key: &str, target: f32, snap: bool| -> f32 {
        let gid = id.with(key);
        let cur: f32 = if snap { target } else { ctx.data(|d| d.get_temp(gid)).unwrap_or(target) };
        let v = cur + (target - cur) * f;
        ctx.data_mut(|d| d.insert_temp(gid, v));
        if (v - target).abs() > 0.3 {
            ctx.request_repaint();
        }
        v
    };
    // Keep the caret in view
    let cx_raw = char_x(&g, ci);
    let mut sc_t = scroll;
    if cx_raw - sc_t > clip_w - 4.0 {
        sc_t = cx_raw - clip_w + 4.0;
    }
    if cx_raw - sc_t < 0.0 {
        sc_t = cx_raw;
    }
    sc_t = sc_t.clamp(0.0, (g.size().x - clip_w + 6.0).max(0.0));
    let scroll = glide("it_scroll_g", sc_t, start);
    ctx.data_mut(|d| d.insert_temp(scid, sc_t));
    let o = origin(&g, scroll);

    let e = ease(&ctx, id.with("it_e"), done.is_none());
    let p = ui.painter().clone();
    let tw = glide("it_w", g.size().x, start);
    match ring {
        Ring::Fixed(r, rd) => {
            p.rect_stroke(r.shrink(0.5), rd, egui::Stroke::new(1.0, blue().gamma_multiply(e)));
        }
        Ring::Hug { left, max_right, rounding } => {
            let right = (rect.min.x + tw.max(24.0) + 6.0).min(max_right);
            let b = Rect::from_min_max(pos2(left, o.y - 2.0), pos2(right, o.y + g.size().y + 2.0));
            p.rect_filled(b, rounding, blue().gamma_multiply(0.12 * e));
            p.rect_stroke(b, rounding, egui::Stroke::new(1.0, blue().gamma_multiply(e)));
        }
    }
    let tp = p.with_clip_rect(rect.expand2(vec2(2.0, 4.0)).intersect(p.clip_rect()));
    // Selection wash (glides as it grows / moves)
    let (sa, sb) = (char_x(&g, ai.min(ci)), char_x(&g, ai.max(ci)));
    let has_sel = ai != ci;
    let sel = ease(&ctx, id.with("it_sel"), has_sel);
    let ga = glide("it_sa", sa, start);
    let gb = glide("it_sb", if has_sel { sb } else { sa.max(ga) }, start);
    if sel > 0.01 && gb > ga {
        let h = g.size().y / 2.0;
        let r = Rect::from_min_max(pos2(o.x + ga - 1.0, rect.center().y - h), pos2(o.x + gb + 1.0, rect.center().y + h));
        tp.rect_filled(r, 3.0, blue().gamma_multiply(0.45 * sel * e));
    }
    tp.galley(o, g.clone(), col);
    // Breathing caret, gliding to where it goes
    let cx = glide("it_cx", cx_raw, start);
    let t = ctx.input(|i| i.time) as f32;
    let a = e * (1.0 - sel) * (0.62 + 0.38 * (t * 4.2).cos());
    if a > 0.01 {
        let x = o.x + cx + 0.5;
        let h = g.size().y * 0.42;
        tp.line_segment([pos2(x, rect.center().y - h), pos2(x, rect.center().y + h)], egui::Stroke::new(1.5, Color32::WHITE.gamma_multiply(a)));
    }
    if done.is_none() && resp.hovered() {
        ctx.set_cursor_icon(egui::CursorIcon::Text);
    }
    done
}
