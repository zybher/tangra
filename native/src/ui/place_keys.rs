//! Placement keys for the selection, whatever it is: 0-9 opacity, Alt+A/D/W/S/H/V align,
//! Ctrl+Alt+Shift+R fit frames, Ctrl+Alt+G / Ctrl+F frame the selection, Shift+Enter parent.
use crate::*;

impl App {
    /// Figma's keys: 1..9 = 10..90 % opacity, 0 = 100 %; Alt+A/D/W/S/H/V align; Shift+Enter parent
    pub(crate) fn place_keys(&mut self, ctx: &egui::Context) {
        if self.selected.is_empty() || self.edit.is_some() || self.prompt.is_some() || self.shortcuts_open || ctx.wants_keyboard_input() || ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        const DIGITS: [Key; 10] = [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
        let (digit, al) = ctx.input(|i| {
            let m = i.modifiers;
            let digit = if m.is_none() { DIGITS.iter().position(|k| i.key_pressed(*k)) } else { None };
            let al = if m.alt && !m.ctrl && !m.shift && !m.command {
                [(Key::A, Align::Left), (Key::D, Align::Right), (Key::W, Align::Top), (Key::S, Align::Bottom), (Key::H, Align::HCenter), (Key::V, Align::VCenter)]
                    .iter()
                    .find(|(k, _)| i.key_pressed(*k))
                    .map(|x| x.1)
            } else {
                None
            };
            (digit, al)
        });
        if let Some(d) = digit {
            let v = if d == 0 { 1.0 } else { d as f32 / 10.0 };
            self.snapshot();
            self.set_selection_opacity(v);
            self.toast(format!("Opacity {}%", (v * 100.0).round() as i32));
        }
        if let Some(a) = al {
            self.align_selection(a);
        }
        // Ctrl+Alt+Shift+R: resize the selected frames to fit their contents (Figma)
        if ctx.input(|i| i.modifiers.command && i.modifiers.alt && i.modifiers.shift && i.key_pressed(Key::R)) {
            self.fit_frames_to_content();
        }
        // Ctrl+Alt+G: wrap the selection in a new frame (Figma)
        if ctx.input(|i| i.modifiers.command && !i.modifiers.shift && ((i.modifiers.alt && i.key_pressed(Key::G)) || (!i.modifiers.alt && i.key_pressed(Key::F)))) {
            self.frame_selection();
        }
        // Shift+Enter: select the frame around the selection (Figma)
        let up = ctx.input(|i| i.modifiers.shift && !i.modifiers.command && !i.modifiers.alt && i.key_pressed(Key::Enter));
        if up {
            if let Some(f) = self.sel.clone().and_then(|s| self.parent_of(&s.1).map(str::to_string)) {
                self.select_only((Kind::Group, f));
            }
        }
    }
}
