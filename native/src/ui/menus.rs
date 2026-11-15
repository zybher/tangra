//! Right-click context menu and its body.

use crate::*;

impl App {
    pub(crate) fn tools_ui(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        for (tool, label, key) in [
            (Tool::Select, "Select / move", "V"),
            (Tool::Pen, "Pen", "P"),
            (Tool::Highlighter, "Highlighter", "H"),
            (Tool::Pixelate, "Pixelate", "B"),
            (Tool::Eraser, "Eraser", "E"),
        ] {
            if radio_item(ui, label, key, self.tool == tool) {
                acts.push(Act::Tool(tool));
            }
        }
        if item(ui, "Add text here", "T", true) {
            acts.push(Act::LabelHere);
        }
        header(ui, "THICKNESS");
        if let Some(w) = choice_row(ui, &[("S", 2.0), ("M", 4.0), ("L", 8.0), ("XL", 14.0)], self.pen_width, 1.0) {
            acts.push(Act::Width(w));
        }
        header(ui, "COLOR");
        if let Some(c) = swatches(ui, &PEN_COLORS, &self.pen_color) {
            acts.push(Act::Color(c));
        }
    }

    pub(crate) fn menu_ui(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        cm_begin(ui);
        self.menu_body(ui, acts);
        cm_end(ui);
    }

    pub(crate) fn menu_body(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        // The menu belongs to a spot on the board; once the view moves, it closes
        let c = self.board.camera;
        let (x, y, z) = self.menu_cam;
        let _ = (c, x, y, z);
        ui.set_min_width(160.0);
        // egui menus reuse last frame's width as available width, so cap it or it never shrinks
        ui.set_max_width(200.0);
        if self.selected.len() > 1 && self.menu_hit.as_ref().map_or(false, |h| self.selected.contains(h)) {
            header(ui, &format!("   SELECTION ({} items)", self.selected.len()));
            if item(ui, "Bring to front", "", true) {
                acts.push(Act::Front);
            }
            if item(ui, "Delete all selected", "Del", true) {
                acts.push(Act::Delete);
            }
            return;
        }
        if let Some((image, label)) = &self.sel_label {
            let img = self.board.images.iter().find(|i| &i.id == image);
            let sx = img.map_or(1.0, |i| i.scale().0);
            let l = img.and_then(|i| i.annotations.texts.iter().find(|l| &l.id == label));
            header(ui, "   TEXT ON IMAGE");
            if item(ui, "Edit text", "Double-click", true) {
                acts.push(Act::Activate);
            }
            header(ui, "   TEXT SIZE");
            let cur_sz = l.map_or(16.0, |t| (if t.font_size > 0.0 { t.font_size } else { 16.0 }) * sx);
            if let Some(sz) = choice_row(ui, &[("S", 12.0), ("M", 16.0), ("L", 24.0), ("XL", 36.0), ("XXL", 56.0)], cur_sz, 1.5) {
                acts.push(Act::TextFontSize(sz));
            }
            header(ui, "   COLOR");
            if let Some(c) = swatches(ui, &PEN_COLORS, l.map_or("", |t| t.color.as_str())) {
                acts.push(Act::Color(c));
            }
            divider(ui);
            if item(ui, "Delete text", "Del", true) {
                acts.push(Act::Delete);
            }
            return;
        }
        match self.menu_hit.as_ref().map(|h| h.0) {
            Some(Kind::Image) => {
                header(ui, "   IMAGE");
                if item(ui, "Copy with annotations", "Ctrl+C", true) {
                    acts.push(Act::CopyBaked);
                }
                if item(ui, "Copy original", "Ctrl+Shift+C", true) {
                    acts.push(Act::CopyClean);
                }
                divider(ui);
                if item(ui, "Clear annotations", "", true) {
                    acts.push(Act::ClearAnnotations);
                }
                let stretched = self.menu_hit.as_ref().and_then(|h| self.board.images.iter().find(|i| i.id == h.1)).is_some_and(|i| {
                    let (sx, sy) = i.scale();
                    (sx / sy - 1.0).abs() > 0.005
                });
                if item(ui, "Reset stretch", "", stretched) {
                    acts.push(Act::ResetStretch);
                }
                divider(ui);
                if item(ui, "Bring to front", "", true) {
                    acts.push(Act::Front);
                }
                if item(ui, "Delete image", "Del", true) {
                    acts.push(Act::Delete);
                }
            }
            Some(Kind::Note) => {
                header(ui, "   NOTE");
                if item(ui, "Edit text", "Double-click", true) {
                    acts.push(Act::Activate);
                }
                ui.add_space(0.0);
                header(ui, "   TEXT SIZE");
                let current_sz = match &self.sel {
                    Some((Kind::Note, id)) => self.board.sticky_notes.iter().find(|n| &n.id == id).map(|n| if n.font_size > 0.0 { n.font_size } else { 13.5 }),
                    _ => None,
                }.unwrap_or(13.5);
                if let Some(sz) = choice_row(ui, &[("S", 11.0), ("M", 14.0), ("L", 18.0), ("XL", 24.0)], current_sz, 1.0) {
                    acts.push(Act::NoteFontSize(sz));
                }
                ui.add_space(0.0);
                header(ui, "   OPACITY");
                let current_op = match &self.sel {
                    Some((Kind::Note, id)) => self.board.sticky_notes.iter().find(|n| &n.id == id).map(|n| n.opacity),
                    _ => None,
                }.unwrap_or(1.0);
                if let Some(val) = choice_row(ui, &[("25%", 0.25), ("50%", 0.5), ("75%", 0.75), ("100%", 1.0)], current_op, 0.12) {
                    acts.push(Act::NoteOpacity(val));
                }
                ui.add_space(0.0);
                header(ui, "   COLOR");
                let current = match &self.sel {
                    Some((Kind::Note, id)) => self.board.sticky_notes.iter().find(|n| &n.id == id).map(|n| n.color.as_str()),
                    _ => None,
                };
                if let Some(c) = swatches(ui, &NOTE_COLORS, current.unwrap_or("")) {
                    acts.push(Act::NoteColor(c));
                }
                divider(ui);
                if item(ui, "Bring to front", "", true) {
                    acts.push(Act::Front);
                }
                if item(ui, "Delete note", "Del", true) {
                    acts.push(Act::Delete);
                }
            }
            Some(Kind::Link) => {
                header(ui, "   LINK");
                if item(ui, "Open in browser", "Double-click", true) {
                    acts.push(Act::Activate);
                }
                if item(ui, "Bring to front", "", true) {
                    acts.push(Act::Front);
                }
                if item(ui, "Delete link", "Del", true) {
                    acts.push(Act::Delete);
                }
            }
            Some(Kind::Group) => {
                header(ui, "   FRAME");
                if item(ui, "Rename title", "Double-click", true) {
                    acts.push(Act::Activate);
                }
                divider(ui);
                let clip = match &self.sel {
                    Some((Kind::Group, id)) => self.board.group_cards.iter().any(|g| &g.id == id && g.clip),
                    _ => false,
                };
                if check_item(ui, "Clip content", clip) {
                    acts.push(Act::ToggleClip);
                }
                let can_fit = matches!(&self.sel, Some((Kind::Group, id)) if self.board.group_cards.iter().any(|g| &g.id == id && g.layout == 0) && !self.descendants(id).is_empty());
                if item(ui, "Resize to fit content", "Ctrl+Alt+Shift+R", can_fit) {
                    acts.push(Act::FitFrame);
                }
                // Border position, like Figma's stroke Inside / Center / Outside
                header(ui, "   STROKE");
                let sp = match &self.sel {
                    Some((Kind::Group, id)) => self.board.group_cards.iter().find(|g| &g.id == id).map_or(0, |g| g.stroke_pos),
                    _ => 0,
                };
                if let Some(v) = choice_row(ui, &[("Inside", 1.0), ("Center", 0.0), ("Outside", 2.0)], sp as f32, 0.5) {
                    acts.push(Act::StrokePos(v as u8));
                }
                divider(ui);
                if item(ui, "Bring to front", "", true) {
                    acts.push(Act::Front);
                }
                if item(ui, "Delete frame", "Del", true) {
                    acts.push(Act::Delete);
                }
            }
            Some(Kind::Stroke) => {
                let stroke = self.sel.as_ref().and_then(|(_, id)| self.find_stroke(id));
                let shape = stroke.is_some_and(|s| s.closed_len().is_some());
                header(ui, if shape { "   SHAPE" } else { "   DRAWING" });
                if let (Some(s), true) = (stroke, shape) {
                    if check_item(ui, "Stroke", !s.no_stroke) {
                        acts.push(Act::ToggleStroke);
                    }
                    if check_item(ui, "Fill", s.fill) {
                        acts.push(Act::ToggleFill);
                    }
                    ui.add_space(0.0);
                }
                divider(ui);
                if item(ui, "Bring to front", "", true) {
                    acts.push(Act::Front);
                }
                if item(ui, "Delete drawing", "Del", true) {
                    acts.push(Act::Delete);
                }
            }
            Some(Kind::Text) => {
                header(ui, "   TEXT");
                if item(ui, "Edit text", "Double-click", true) {
                    acts.push(Act::Activate);
                }
                let label = match &self.sel {
                    Some((Kind::Text, id)) => self.board.world_texts.iter().find(|t| &t.id == id),
                    _ => None,
                };
                divider(ui);
                if item(ui, "Bring to front", "", true) {
                    acts.push(Act::Front);
                }
                if item(ui, "Delete text", "Del", true) {
                    acts.push(Act::Delete);
                }
            }
            None => {
                if item(ui, "Paste here", "Ctrl+V", true) {
                    acts.push(Act::Paste);
                }
                divider(ui);
                header(ui, "   DRAW");
                self.tools_ui(ui, acts);
            }
        }
    }
}
