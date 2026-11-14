//! Keyboard shortcuts, pointer drags (move/resize/draw/marquee) and input routing.

use crate::*;

impl App {
    pub(crate) fn keys(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.focused);
        let ctrl_v = focused && platform::ctrl_v_down();
        let ctrl_v_edge = ctrl_v && !self.ctrl_v_was;
        self.ctrl_v_was = ctrl_v;
        if self.edit.is_some() || self.prompt.is_some() || self.shortcuts_open || ctx.wants_keyboard_input() {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::L) && i.modifiers.is_none()) {
            crate::ui::outline::toggle_outline(ctx);
        }
        // Arrow keys nudge the selection (Shift: 10 px)
        let (nx, ny) = ctx.input(|i| {
            let s = if i.modifiers.shift { 10.0 } else { 1.0 };
            let k = |a: Key, b: Key| (i.key_pressed(b) as i32 - i.key_pressed(a) as i32) as f32 * s;
            (k(Key::ArrowLeft, Key::ArrowRight), k(Key::ArrowUp, Key::ArrowDown))
        });
        if nx != 0.0 || ny != 0.0 {
            self.nudge(nx, ny);
        }
        let (mut paste, mut copy) = (ctrl_v_edge, false);
        let (m, undo, redo, del, esc, select_all, tool) = ctx.input(|i| {
            for e in &i.events {
                match e {
                    egui::Event::Paste(_) => paste = true,
                    egui::Event::Copy => copy = true,
                    _ => {}
                }
            }
            let m = i.modifiers;
            paste |= m.command && i.key_pressed(Key::V);
            copy |= m.command && i.key_pressed(Key::C);
            let select_all = m.command && i.key_pressed(Key::A);
            let tool = if m.command || m.alt {
                None
            } else if i.key_pressed(Key::V) {
                Some(Tool::Select)
            } else if i.key_pressed(Key::P) {
                Some(Tool::Pen)
            } else if i.key_pressed(Key::B) {
                Some(Tool::Pixelate)
            } else if i.key_pressed(Key::H) {
                Some(Tool::Highlighter)
            } else if i.key_pressed(Key::T) {
                Some(Tool::Text)
            } else if i.key_pressed(Key::E) {
                Some(Tool::Eraser)
            } else if i.key_pressed(Key::R) {
                Some(Tool::Rect)
            } else if i.key_pressed(Key::O) {
                Some(Tool::Ellipse)
            } else if i.key_pressed(Key::A) {
                Some(Tool::Arrow)
            } else if i.key_pressed(Key::F) {
                Some(Tool::Frame)
            } else {
                None
            };
            (
                m,
                m.command && !m.shift && i.key_pressed(Key::Z),
                m.command && (i.key_pressed(Key::Y) || (m.shift && i.key_pressed(Key::Z))),
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                i.key_pressed(Key::Escape),
                select_all,
                tool,
            )
        });
        // Ctrl+D duplicate in place, Ctrl+] / Ctrl+[ front / back, Shift+0 100 %, Shift+1 fit all
        let (dup, front, back, z100, fit) = ctx.input(|i| {
            let m = i.modifiers;
            (
                m.command && i.key_pressed(Key::D),
                m.command && i.key_pressed(Key::CloseBracket),
                m.command && i.key_pressed(Key::OpenBracket),
                m.shift && !m.command && i.key_pressed(Key::Num0),
                m.shift && !m.command && i.key_pressed(Key::Num1),
            )
        });
        if dup && !self.selected.is_empty() {
            self.snapshot();
            let items = self.selected.clone();
            self.duplicate_in_place(&items);
        }
        if front {
            self.bring_to_front();
            self.mark_dirty();
        }
        if back {
            self.send_to_back();
            self.mark_dirty();
        }
        // The zooms need the canvas rect, so they run with the frame's other actions (and glide)
        if z100 || fit {
            let a: u8 = if z100 { 1 } else { 2 };
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_key_zoom"), a));
        }
        if select_all {
            self.selected.clear();
            for img in &self.board.images { self.selected.insert((Kind::Image, img.id.clone())); }
            for n in &self.board.sticky_notes { self.selected.insert((Kind::Note, n.id.clone())); }
            for l in &self.board.link_cards { self.selected.insert((Kind::Link, l.id.clone())); }
            for g in &self.board.group_cards { self.selected.insert((Kind::Group, g.id.clone())); }
            for t in &self.board.world_texts { self.selected.insert((Kind::Text, t.id.clone())); }
            for s in &self.board.world_strokes { self.selected.insert((Kind::Stroke, s.id.clone())); }
            self.sel = self.selected.iter().next().cloned();
        }
        if paste && self.now - self.last_paste > 0.3 {
            self.last_paste = self.now;
            let rect = self.last_rect;
            let p = ctx.pointer_hover_pos().filter(|p| rect.contains(*p)).unwrap_or(rect.center());
            let world = self.view(rect).w(p);
            self.paste_at(ctx, world, false);
        }
        // Image files dropped from Explorer land where they were dropped, fanned out if several
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            let rect = self.last_rect;
            let ppp = ctx.pixels_per_point();
            let p = platform::cursor_client()
                .map(|(x, y)| pos2(x / ppp, y / ppp))
                .filter(|p| rect.contains(*p))
                .unwrap_or(rect.center());
            self.drop_files(ctx, self.view(rect).w(p), &dropped);
        }
        if copy {
            let one_image = self.selected.len() == 1 && matches!(self.sel, Some((Kind::Image, _)));
            if one_image || self.selected.is_empty() {
                self.copy_image(!m.shift);
            } else {
                self.copy_objects(ctx);
            }
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if del {
            self.delete_selected();
        }
        if let Some(t) = tool {
            if t != self.tool {
                self.tool_menu = None;
                self.tool_sub = None;
            }
            self.tool = t;
        }
        if esc {
            if self.tool != Tool::Select {
                self.tool = Tool::Select;
            } else {
                self.deselect_all();
            }
        }
    }

    pub(crate) fn begin_drag(&mut self, w: Pt, shift_or_ctrl: bool) {
        let zoom = self.board.camera.zoom;
        match self.tool {
            Tool::Pen | Tool::Highlighter | Tool::Rect | Tool::Ellipse | Tool::Arrow => {
                let hl = self.tool == Tool::Highlighter;
                let head = 10.0 + self.pen_width * 4.0;
                let (target, first, head) = match self.image_at(w) {
                    Some(img) => {
                        let (sx, sy) = img.scale();
                        (Some(img.id.clone()), Pt { x: (w.x - img.x) / sx, y: (w.y - img.y) / sy }, head / sx)
                    }
                    None => (None, w, head / zoom),
                };
                self.shape_anchor = (first, head);
                self.active = Some((
                    target,
                    Stroke {
                        id: new_id(),
                        points: vec![first],
                        color: self.pen_color.clone(),
                        width: if hl { self.pen_width * 3.5 } else { self.pen_width },
                        opacity: Some(if hl { 0.35 } else { 1.0 }),
                        created_at: now_ms(),
                        fill: matches!(self.tool, Tool::Rect | Tool::Ellipse) && self.shape_fill > 0,
                        no_stroke: matches!(self.tool, Tool::Rect | Tool::Ellipse) && self.shape_fill == 2,
                        pixelate: false,
                        radius: None,
                    },
                ));
                self.drag = Drag::Draw;
            }
            Tool::Frame => {
                self.snapshot();
                let id = self.board.add_group(w);
                // Starts empty at the pointer and grows with the drag (a plain click gets the default size on release)
                if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                    (g.x, g.y, g.width, g.height) = (w.x, w.y, 0.0, 0.0);
                    g.square = self.frame_square;
                }
                self.select_only((Kind::Group, id.clone()));
                self.drag = Drag::Frame { start: w, id };
            }
            Tool::Pixelate => {
                let hit = self.image_at(w).map(|img| {
                    let (sx, sy) = img.scale();
                    (img.id.clone(), img.asset_file_name.clone(), Pt { x: (w.x - img.x) / sx, y: (w.y - img.y) / sy })
                });
                let Some((id, file, first)) = hit else {
                    self.toast("Pixelate works on images: swipe over one");
                    return;
                };
                self.want_mosaic(&file);
                self.active = Some((
                    Some(id),
                    Stroke {
                        id: new_id(),
                        points: vec![first],
                        color: "#808080".into(),
                        width: self.blur_size,
                        opacity: Some(1.0),
                        created_at: now_ms(),
                        pixelate: true,
                        radius: None,
                        ..Default::default()
                    },
                ));
                self.drag = Drag::Draw;
            }
            Tool::Eraser => {
                self.drag_board = Some(self.board.clone());
                self.drag_changed = self.erase_at(w);
                self.drag = Drag::Erase;
            }
            Tool::Text => {}
            Tool::Select => {
                if let Some((run, i)) = self.gap_handle_at(w) {
                    self.begin_gap_drag(run, i, w);
                    return;
                }
                if let Some(h) = self.multi_handle_at(w) {
                    self.begin_multi_resize(w, h);
                    return;
                }
                // Text written on an image moves on its own, inside that image
                self.sel_label = None;
                let lab = self.image_at(w).and_then(|img| {
                    let (sx, sy) = img.scale();
                    let lp = Pt { x: (w.x - img.x) / sx, y: (w.y - img.y) / sy };
                    img.annotations
                        .texts
                        .iter()
                        .rev()
                        .find(|l| label_hit(l, lp, 4.0 / sx.max(0.01)))
                        .map(|l| (img.id.clone(), l.id.clone(), Pt { x: lp.x - l.x, y: lp.y - l.y }))
                });
                if let Some((image, label, grab)) = lab {
                    self.deselect_all();
                    self.sel_label = Some((image.clone(), label.clone()));
                    self.drag_board = Some(self.board.clone());
                    self.drag = Drag::MoveLabel { image, label, grab };
                    return;
                }

                // Resize handle check on single selected item
                if self.selected.len() == 1 {
                    if let Some((k, id)) = self.sel.clone() {
                        if k != Kind::Text {
                            if let Some([x, y, wd, h]) = self.rect_of(k, &id) {
                                let grab = 12.0 / zoom;
                                for (left, top) in [(false, false), (true, false), (false, true), (true, true)] {
                                    let cx = if left { x } else { x + wd };
                                    let cy = if top { y } else { y + h };
                                    if (w.x - cx).abs() < grab && (w.y - cy).abs() < grab {
                                        self.drag_board = Some(self.board.clone());
                                        self.drag = Drag::Resize { start: w, ox: x, oy: y, ow: wd, oh: h, left, top, rx: true, ry: true, sel: (k, id) };
                                        return;
                                    }
                                }
                                // Edges resize one way
                                let (in_x, in_y) = (w.x > x && w.x < x + wd, w.y > y && w.y < y + h);
                                let edge = if in_y && (w.x - x).abs() < grab * 0.7 {
                                    Some((true, false, true, false))
                                } else if in_y && (w.x - x - wd).abs() < grab * 0.7 {
                                    Some((false, false, true, false))
                                } else if in_x && (w.y - y).abs() < grab * 0.7 {
                                    Some((false, true, false, true))
                                } else if in_x && (w.y - y - h).abs() < grab * 0.7 {
                                    Some((false, false, false, true))
                                } else {
                                    None
                                };
                                if let Some((left, top, rx, ry)) = edge {
                                    if k == Kind::Image {
                                        if let Some(i) = self.board.images.iter_mut().find(|i| i.id == id) {
                                            let (sx, sy) = i.scale();
                                            if i.base_scale.is_none() && (sx / sy - 1.0).abs() < 0.005 {
                                                i.base_scale = Some(sx);
                                            }
                                        }
                                    }
                                    self.drag_board = Some(self.board.clone());
                                    self.drag = Drag::Resize { start: w, ox: x, oy: y, ow: wd, oh: h, left, top, rx, ry, sel: (k, id) };
                                    return;
                                }
                            }
                        }
                    }
                }

                let hit = self.hit_test_drag(w);
                if let Some(target) = hit {
                    if self.selected.contains(&target) {
                        // Already in selection, keep current selection
                    } else if shift_or_ctrl {
                        self.selected.insert(target.clone());
                        self.sel = Some(target);
                    } else {
                        self.select_only(target);
                    }

                    // Collect origins of all items being moved.
                    // Also if a frame is moved, move items fully inside the frame with it!
                    let mut moving_sels = self.selected.clone();
                    for (k, id) in &self.selected {
                        if *k == Kind::Group {
                            if let Some([gx, gy, gw, gh]) = self.rect_of(Kind::Group, id) {
                                let is_inside = |rx: f32, ry: f32, rw: f32, rh: f32| {
                                    // Anything still touching the frame belongs to it
                                    rx < gx + gw && rx + rw > gx && ry < gy + gh && ry + rh > gy
                                };
                                for img in &self.board.images {
                                    if is_inside(img.x, img.y, img.width, img.height) {
                                        moving_sels.insert((Kind::Image, img.id.clone()));
                                    }
                                }
                                for n in &self.board.sticky_notes {
                                    if is_inside(n.x, n.y, n.width, n.height) {
                                        moving_sels.insert((Kind::Note, n.id.clone()));
                                    }
                                }
                                for l in &self.board.link_cards {
                                    if is_inside(l.x, l.y, l.width, l.height) {
                                        moving_sels.insert((Kind::Link, l.id.clone()));
                                    }
                                }
                                for s in &self.board.world_strokes {
                                    if pts_bbox(&s.points).is_some_and(|[x, y, w, h]| is_inside(x, y, w, h)) {
                                        moving_sels.insert((Kind::Stroke, s.id.clone()));
                                    }
                                }
                                for t in &self.board.world_texts {
                                    if is_inside(t.x, t.y, 20.0, 14.0) {
                                        moving_sels.insert((Kind::Text, t.id.clone()));
                                    }
                                }
                            }
                        }
                    }

                    let mut origins = Vec::new();
                    for s in &moving_sels {
                        if let Some([x, y, _, _]) = self.rect_of(s.0, &s.1) {
                            origins.push((s.clone(), Pt { x, y }));
                        }
                    }
                    self.drag_board = Some(self.board.clone());
                    self.dup_origin = None;
                    if ALT_DOWN.load(std::sync::atomic::Ordering::Relaxed) {
                        // Remember the original's box so the drag can show the distance to it
                        let (mut a, mut b) = (pos2(f32::MAX, f32::MAX), pos2(f32::MIN, f32::MIN));
                        for s in &moving_sels {
                            if let Some([x, y, ww, hh]) = self.rect_of(s.0, &s.1) {
                                (a, b) = (a.min(pos2(x, y)), b.max(pos2(x + ww, y + hh)));
                            }
                        }
                        self.dup_origin = (b.x > a.x).then(|| [a.x, a.y, b.x - a.x, b.y - a.y]);
                        self.duplicate_in_place(&moving_sels);
                    }
                    self.drag = Drag::Move { start: w, origins };
                } else {
                    let initial = if shift_or_ctrl {
                        self.selected.clone()
                    } else {
                        self.deselect_all();
                        HashSet::new()
                    };
                    if !self.hand_drag {
                        self.drag = Drag::BoxSelect { start: w, current: w, initial };
                    }
                }
            }
        }
    }

    pub(crate) fn continue_drag(&mut self, w: Pt) {
        match self.drag.clone() {
            Drag::Draw => {
                let mut local = w;
                if let Some((Some(id), _)) = &self.active {
                    if let Some(img) = self.board.images.iter().find(|i| &i.id == id) {
                        let (sx, sy) = img.scale();
                        local = Pt { x: (w.x - img.x) / sx, y: (w.y - img.y) / sy };
                    }
                }
                let (tool, (anchor, head), shift) = (self.tool, self.shape_anchor, self.shift);
                if let Some((_, s)) = &mut self.active {
                    if matches!(tool, Tool::Rect | Tool::Ellipse | Tool::Arrow) {
                        let end = if shift { constrain(tool, anchor, local) } else { local };
                        s.points = shape_points(tool, anchor, end, head);
                    } else if s.points.last() != Some(&local) {
                        s.points.push(local);
                    }
                }
            }
            Drag::Erase => {
                self.drag_changed |= self.erase_at(w);
            }
            Drag::Frame { start, id } => {
                // A tiny drag is a click: keep the preset size centred on it
                let (_, fw, fh) = FRAME_SIZES[0];
                let (dx, dy) = (w.x - start.x, w.y - start.y);
                let _ = (fw, fh);
                let rect = {
                    let ww = dx.abs();
                    let hh = dy.abs();
                    let x = if dx < 0.0 { start.x - ww } else { start.x };
                    let y = if dy < 0.0 { start.y - hh } else { start.y };
                    [x, y, ww, hh]
                };
                if let Some(g) = self.board.group_cards.iter_mut().find(|g| g.id == id) {
                    g.x = rect[0];
                    g.y = rect[1];
                    g.width = rect[2];
                    g.height = rect[3];
                }
                self.mark_dirty();
            }
            Drag::Move { start, origins } => {
                let (dx, dy) = self.smart_move(&origins, start, w);
                for (s, origin) in &origins {
                    if let Some([_, _, wd, h]) = self.rect_of(s.0, &s.1) {
                        self.set_rect(s.0, &s.1, [origin.x + dx, origin.y + dy, wd, h]);
                    }
                }
                self.drag_changed = true;
            }
            Drag::Resize { start, ox, oy, ow, oh, left, top, rx, ry, sel: (k, id) } => {
                // The opposite corner stays put; Shift keeps proportions; Alt resizes from the centre
                // (both sides move). Sizes and positions land on whole pixels.
                let alt = ALT_DOWN.load(std::sync::atomic::Ordering::Relaxed);
                let grow = if alt { 2.0 } else { 1.0 };
                let min = if k == Kind::Stroke { 8.0 } else { 40.0 };
                let (sx, sy) = (if left { -1.0 } else { 1.0 }, if top { -1.0 } else { 1.0 });
                let mut nw = if rx { (ow + grow * sx * (w.x - start.x)).max(min).round() } else { ow };
                let mut nh = if ry { (oh + grow * sy * (w.y - start.y)).max(min).round() } else { oh };
                if (k == Kind::Image && rx && ry) || self.shift {
                    if rx {
                        nh = nw * oh / ow.max(1.0);
                    } else {
                        nw = nh * ow / oh.max(1.0);
                    }
                }
                let keep = (k == Kind::Image && rx && ry) || self.shift;
                let (nw, nh, mw, mh) = self.size_snap(&(k, id.clone()), nw, nh, rx, ry, keep, ow / oh.max(1.0));
                let (nx, ny) = if alt {
                    ((ox + (ow - nw) / 2.0).round(), (oy + (oh - nh) / 2.0).round())
                } else {
                    (if left { ox + ow - nw } else { ox }, if top { oy + oh - nh } else { oy })
                };
                self.set_rect(k, &id, [nx, ny, nw, nh]);
                self.size_guides([nx, ny, nw, nh], mw, mh);
                if k == Kind::Image && rx && ry {
                    let b0 = self.drag_board.as_ref().and_then(|b| b.images.iter().find(|i| i.id == id)).and_then(|i| i.base_scale);
                    if let (Some(b0), Some(i)) = (b0, self.board.images.iter_mut().find(|i| i.id == id)) {
                        i.base_scale = Some(b0 * nw / ow.max(1e-3));
                    }
                }
                self.drag_changed = true;
            }
            Drag::BoxSelect { start, initial, .. } => {
                let min_x = start.x.min(w.x);
                let min_y = start.y.min(w.y);
                let max_x = start.x.max(w.x);
                let max_y = start.y.max(w.y);
                let box_r = [min_x, min_y, (max_x - min_x).max(1e-3), (max_y - min_y).max(1e-3)];
                let mut new_sel = initial.clone();
                for hit in self.items_in_rect(box_r) {
                    new_sel.insert(hit);
                }
                self.selected = new_sel;
                self.sel = self.selected.iter().next().cloned();
                self.drag = Drag::BoxSelect { start, current: w, initial };
            }
            Drag::MoveLabel { image, label, grab } => {
                if let Some(img) = self.board.images.iter_mut().find(|i| i.id == image) {
                    let (sx, sy) = img.scale();
                    let (ox, oy) = (img.x, img.y);
                    if let Some(l) = img.annotations.texts.iter_mut().find(|l| l.id == label) {
                        l.x = (w.x - ox) / sx - grab.x;
                        l.y = (w.y - oy) / sy - grab.y;
                        self.drag_changed = true;
                    }
                }
            }
            Drag::Gap { .. } => self.continue_gap_drag(w),
            Drag::MultiResize { .. } => self.continue_multi_resize(w),
            Drag::None => {}
        }
    }

    pub(crate) fn end_drag(&mut self) {
        if let Drag::Frame { start, id } = &self.drag {
            // A click (or a tiny drag) drops a default-size frame centred on it
            let (_, fw, fh) = FRAME_SIZES[0];
            if let Some(g) = self.board.group_cards.iter_mut().find(|g| &g.id == id) {
                if g.width.max(g.height) * self.board.camera.zoom < 6.0 {
                    (g.x, g.y, g.width, g.height) = (start.x - fw / 2.0, start.y - fh / 2.0, fw, fh);
                } else {
                    g.width = g.width.max(20.0);
                    g.height = g.height.max(20.0);
                }
            }
            // One frame per use, then back to select so it can be moved straight away
            self.tool = Tool::Select;
        }
        if let Drag::Draw = self.drag {
            if let Some((target, stroke)) = self.active.take() {
                if stroke.points.len() >= 2 {
                    self.snapshot();
                    match target.and_then(|id| self.board.images.iter_mut().find(|i| i.id == id)) {
                        Some(img) => img.annotations.strokes.push(stroke),
                        None => self.board.world_strokes.push(stroke),
                    }
                }
            }
        } else if self.drag_changed {
            if let Some(b) = self.drag_board.take() {
                self.push_undo(b);
            }
        }
        self.active = None;
        self.drag = Drag::None;
        self.drag_board = None;
        self.drag_changed = false;
    }

    pub(crate) fn input(&mut self, ctx: &egui::Context, resp: &egui::Response, rect: Rect) {
        ALT_DOWN.store(ctx.input(|i| i.modifiers.alt), std::sync::atomic::Ordering::Relaxed);
        PTR_SPEED.store(ctx.input(|i| i.pointer.velocity().length()).to_bits(), std::sync::atomic::Ordering::Relaxed);
        let (press, pos, hover, scroll_delta, zoom_delta, ctrl, space) = ctx.input(|i| {
            (
                i.pointer.press_origin(),
                i.pointer.interact_pos(),
                i.pointer.hover_pos(),
                i.raw_scroll_delta,
                i.zoom_delta(),
                i.modifiers.ctrl || i.modifiers.command,
                i.key_down(Key::Space),
            )
        });
        let space = space && self.edit.is_none() && !ctx.wants_keyboard_input();

        // Mouse wheel: Ctrl+Wheel (or pinch) zooms; normal wheel scrolls/pans.
        // Locked while a right-click menu is open, so it stays on its spot.
        let cm_open: bool = ctx.data(|d| d.get_temp(egui::Id::new("qd_cm_open"))).unwrap_or(false);
        if resp.contains_pointer() && !cm_open {
            if ctrl || (zoom_delta - 1.0).abs() > 1e-3 {
                let f = if (zoom_delta - 1.0).abs() > 1e-3 {
                    zoom_delta
                } else if scroll_delta.y != 0.0 {
                    1.12f32.powf((scroll_delta.y / 50.0).clamp(-3.0, 3.0))
                } else {
                    1.0
                };
                if let (true, Some(hp)) = (f != 1.0, hover) {
                    let zid = egui::Id::new("qd_zoom_glide");
                    let (mut tgt, _, last) = ctx.data(|d| d.get_temp::<(f32, Pos2, f32)>(zid)).unwrap_or((self.board.camera.zoom, hp, self.board.camera.zoom));
                    if (last / self.board.camera.zoom - 1.0).abs() > 1e-5 {
                        tgt = self.board.camera.zoom;
                    }
                    tgt = (tgt * f).clamp(0.02, 5.0);
                    ctx.data_mut(|d| d.insert_temp(zid, (tgt, hp, self.board.camera.zoom)));
                }
            } else if scroll_delta != egui::Vec2::ZERO {
                let cam = &mut self.board.camera;
                cam.x -= scroll_delta.x / cam.zoom;
                cam.y -= scroll_delta.y / cam.zoom;
                self.mark_dirty();
            }
        }

        // Zoom glide toward the wheel target
        {
            let zid = egui::Id::new("qd_zoom_glide");
            if let Some((tgt, hp, last)) = ctx.data(|d| d.get_temp::<(f32, Pos2, f32)>(zid)) {
                let cur = self.board.camera.zoom;
                // Thresholds are relative: an absolute 1e-4 is 0.5% at 2% zoom, enough to visibly shift
                let close = (tgt / cur - 1.0).abs() < 1e-4;
                if (last / cur - 1.0).abs() > 1e-5 || close {
                    ctx.data_mut(|d| d.remove::<(f32, Pos2, f32)>(zid));
                    if close && (last / cur - 1.0).abs() <= 1e-5 && tgt != cur {
                        // Final snap keeps the point under the cursor in place
                        let before = self.view(rect).w(hp);
                        let cam = &mut self.board.camera;
                        cam.zoom = tgt;
                        cam.x = before.x - (hp.x - rect.min.x) / cam.zoom;
                        cam.y = before.y - (hp.y - rect.min.y) / cam.zoom;
                    }
                } else {
                    let dt = ctx.input(|i| i.stable_dt).min(0.05);
                    let k = 1.0 - (-dt * 16.0).exp();
                    let nz = if (tgt / cur - 1.0).abs() < 0.002 { tgt } else { cur * (tgt / cur).powf(k) };
                    let before = self.view(rect).w(hp);
                    let cam = &mut self.board.camera;
                    cam.zoom = nz;
                    cam.x = before.x - (hp.x - rect.min.x) / cam.zoom;
                    cam.y = before.y - (hp.y - rect.min.y) / cam.zoom;
                    ctx.data_mut(|d| d.insert_temp(zid, (tgt, hp, nz)));
                    self.mark_dirty();
                    ctx.request_repaint();
                }
            }
        }

        // Middle-mouse drag pans (Space + left drag also works)
        let hand = self.hand_drag && self.tool == Tool::Select && matches!(self.drag, Drag::None);
        if resp.dragged_by(PointerButton::Middle) || ((space || hand) && resp.dragged_by(PointerButton::Primary)) {
            let d = resp.drag_delta();
            let cam = &mut self.board.camera;
            cam.x -= d.x / cam.zoom;
            cam.y -= d.y / cam.zoom;
            self.mark_dirty();
            ctx.set_cursor_icon(egui::CursorIcon::None);
            if let Some(hp) = hover {
                hand_cursor(ctx, hp, true);
            }
        } else if resp.hovered() && (space || hand) {
            ctx.set_cursor_icon(egui::CursorIcon::None);
            if let Some(hp) = hover {
                hand_cursor(ctx, hp, false);
            }
        } else if resp.hovered() {
            ctx.set_cursor_icon(match self.tool {
                Tool::Pen | Tool::Highlighter | Tool::Pixelate | Tool::Rect | Tool::Ellipse | Tool::Arrow | Tool::Frame => egui::CursorIcon::Crosshair,
                Tool::Text => egui::CursorIcon::Text,
                Tool::Eraser => egui::CursorIcon::None,
                Tool::Select => egui::CursorIcon::Default,
            });
        }

        let v = self.view(rect);
        let shift_or_ctrl = ctx.input(|i| i.modifiers.shift || i.modifiers.command || i.modifiers.ctrl);
        self.shift = ctx.input(|i| i.modifiers.shift);
        self.snap_off = ctx.input(|i| i.modifiers.ctrl);

        // A click on the canvas while editing text only ends the edit (one step back: the
        // object stays selected); the next click acts normally
        let sid = egui::Id::new("qd_click_ends_edit");
        if ctx.input(|i| i.pointer.primary_pressed()) {
            let n = ctx.cumulative_pass_nr();
            let ended = ctx.data(|d| d.get_temp::<u64>(egui::Id::new("qd_edit_ended"))).is_some_and(|e| n <= e + 1);
            let swallow = self.edit.is_some() || ended;
            ctx.data_mut(|d| d.insert_temp(sid, swallow));
        }
        let swallow = ctx.data(|d| d.get_temp::<bool>(sid)).unwrap_or(false);
        let space = space || swallow;
        if !space {
            if resp.drag_started_by(PointerButton::Primary) {
                if let Some(p) = press.or(pos).filter(|p| !ui_blocks(ctx, *p)) {
                    self.begin_drag(v.w(p), shift_or_ctrl);
                }
            }
            if resp.dragged_by(PointerButton::Primary) {
                if let Some(p) = pos {
                    self.continue_drag(v.w(p));
                }
            }
        }
        if resp.drag_stopped() {
            self.end_drag();
        }

        // Clicks on the floating style bar / its dropdowns never reach the canvas underneath
        let click_pos = resp.interact_pointer_pos().or(pos).filter(|p| !ui_blocks(ctx, *p));
        if !space && resp.clicked() {
            if let Some(p) = click_pos {
                let w = v.w(p);
                match self.tool {
                    Tool::Select => {
                        let hit = self.hit_test(w);
                        if let Some(l) = self.label_at(w).filter(|_| !shift_or_ctrl) {
                            // Text on an image wins over the image under it
                            self.deselect_all();
                            self.sel_label = Some(l);
                        } else if let Some(target) = hit {
                            if shift_or_ctrl {
                                self.toggle_select(target);
                            } else {
                                self.select_only(target);
                            }
                        } else if !shift_or_ctrl {
                            self.deselect_all();
                        }
                    }
                    Tool::Text => self.begin_label(w),
                    Tool::Eraser => {
                        let before = self.board.clone();
                        if self.erase_at(w) {
                            self.push_undo(before);
                        }
                    }
                    _ => {}
                }
            }
        }
        if resp.double_clicked() && self.tool == Tool::Select {
            // A frame is renamed by double-clicking its title (which sits above it),
            // not its body.
            let wp = click_pos.map(|p| v.w(p));
            if let Some(l) = wp.and_then(|p| self.label_at(p)) {
                self.sel_label = Some(l.clone());
                self.edit_label(l);
                return;
            }
            // Same on-screen title band as selecting/dragging (fixed px, so it stays easy to hit
            // at any zoom), plus a little slack around it
            let title_hit = wp.and_then(|p| {
                let z = v.cam.zoom.max(0.01);
                self.board.group_cards.iter().rev().find(|g| {
                    let th = (FRAME_TITLE_PX + 14.0 + g.border.unwrap_or(1.0) / 2.0) / z;
                    let pad = 4.0 / z;
                    p.x >= g.x - pad && p.x <= g.x + g.width + pad && p.y >= g.y - th && p.y <= g.y + pad
                })
            });
            if let Some(g) = title_hit {
                let hit = (Kind::Group, g.id.clone());
                // Same as right-click > Rename: the frame stays selected, so its pills and style
                // bar play their usual exit instead of vanishing
                self.select_only(hit.clone());
                self.activate(hit);
            } else if let Some(hit) = wp.and_then(|p| self.hit_test(p)) {
                self.select_only(hit.clone());
                if hit.0 != Kind::Group {
                    self.activate(hit);
                }
            }
        }
        if resp.secondary_clicked() {
            if let Some(p) = click_pos {
                self.menu_world = v.w(p);
                self.menu_hit = self.hit_test(self.menu_world);
                let c = self.board.camera;
                self.menu_cam = (c.x, c.y, c.zoom);
                let lab = self.label_at(self.menu_world);
                if lab.is_some() {
                    self.deselect_all();
                    self.menu_hit = None;
                }
                self.sel_label = lab;
                if let Some(hit) = &self.menu_hit {
                    if !self.selected.contains(hit) {
                        self.select_only(hit.clone());
                    }
                }
            }
        }
    }

    // ---------- painting ----------
}
