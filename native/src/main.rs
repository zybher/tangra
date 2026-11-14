#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod model;
mod platform;
mod theme;
mod app;
mod canvas;
mod ui;
mod widgets;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Key, PointerButton, Pos2, Rect, ResizeDirection, Rounding, Sense,
    Shape, Stroke as EStroke, TextureHandle, ViewportCommand,
};
use global_hotkey::hotkey::HotKey;
use global_hotkey::GlobalHotKeyManager;
use model::*;
use platform::{hex_rgb, Bridge, Clip, Msg};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use theme::*;
use widgets::*;
use canvas::*;
use ui::*;

type Tween = (f64, Vec<(bool, String, [f32; 4], [f32; 4])>);
/// Canvas objects copied with Ctrl+C (kept in memory; the system clipboard holds a marker)
#[derive(Clone)]
struct ObjClip {
    nonce: String,
    images: Vec<ImageNode>,
    notes: Vec<Note>,
    links: Vec<LinkCard>,
    groups: Vec<Group>,
    texts: Vec<Label>,
    strokes: Vec<Stroke>,
    center: Pt,
}

/// The last image copied to the system clipboard (its pixel size + the canvas object),
/// so pasting it back inside Tangra keeps its on-canvas size instead of pixel size
static IMG_CLIP: std::sync::Mutex<Option<(u32, u32, ObjClip)>> = std::sync::Mutex::new(None);



#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
    Image,
    Note,
    Link,
    Group,
    Text,
    /// A pen stroke or shape, either free on the canvas or drawn on an image
    Stroke,
}
type Sel = (Kind, String);

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Select,
    Pen,
    Highlighter,
    Pixelate,
    Text,
    Eraser,
    Rect,
    Ellipse,
    Arrow,
    /// Click to drop a frame (chosen preset), or drag one out
    Frame,
}

/// Shapes are stored as ordinary strokes, so the eraser, undo, selection and
/// "copy with annotations" all work on them with no extra code.
fn shape_points(tool: Tool, a: Pt, b: Pt, head: f32) -> Vec<Pt> {
    let pt = |x: f32, y: f32| Pt { x, y };
    match tool {
        Tool::Rect => vec![a, pt(b.x, a.y), b, pt(a.x, b.y), a, pt(b.x, a.y)],
        Tool::Ellipse => {
            let (cx, cy) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            let (rx, ry) = ((b.x - a.x).abs() / 2.0, (b.y - a.y).abs() / 2.0);
            (0..=48)
                .map(|i| {
                    let t = i as f32 / 48.0 * std::f32::consts::TAU;
                    pt(cx + rx * t.cos(), cy + ry * t.sin())
                })
                .collect()
        }
        _ => {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len = (dx * dx + dy * dy).sqrt().max(1e-3);
            let (ux, uy) = (dx / len, dy / len);
            let h = head.min(len * 0.5);
            let wing = |s: f32| pt(b.x - ux * h - uy * h * 0.55 * s, b.y - uy * h + ux * h * 0.55 * s);
            vec![a, b, wing(1.0), b, wing(-1.0)]
        }
    }
}

fn pts_bbox(pts: &[Pt]) -> Option<[f32; 4]> {
    let first = pts.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
    for p in pts {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    Some([x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0)])
}

#[derive(Clone, Copy)]
enum TbIcon {
    Select,
    Frame,
    Note,
    Rect,
    Ellipse,
    Arrow,
    Pen,
    Highlighter,
    Pixelate,
    Text,
    Eraser,
}

fn paint_icon(p: &egui::Painter, icon: TbIcon, c: Pos2, s: f32, col: Color32) {
    let st = EStroke::new(1.4_f32, col);
    let q = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    let poly = |pts: &[(f32, f32)]| Shape::closed_line(pts.iter().map(|&(x, y)| q(x, y)).collect(), st);
    match icon {
        TbIcon::Select => {
            p.add(poly(&[(-4.0, -6.0), (-4.0, 5.0), (-1.2, 2.4), (0.8, 6.5), (2.6, 5.7), (0.7, 1.7), (4.5, 1.7)]));
        }
        TbIcon::Frame => {
            for d in [-3.0_f32, 3.0] {
                p.line_segment([q(d, -6.5), q(d, 6.5)], st);
                p.line_segment([q(-6.5, d), q(6.5, d)], st);
            }
        }
        TbIcon::Note => {
            p.add(poly(&[(-5.5, -5.5), (5.5, -5.5), (5.5, 1.0), (1.0, 5.5), (-5.5, 5.5)]));
            p.line_segment([q(1.0, 5.5), q(1.0, 1.0)], st);
            p.line_segment([q(1.0, 1.0), q(5.5, 1.0)], st);
        }
        TbIcon::Rect => {
            p.rect_stroke(Rect::from_center_size(c, vec2(12.0 * s, 12.0 * s)), 1.5, st);
        }
        TbIcon::Ellipse => {
            p.circle_stroke(c, 6.2 * s, st);
        }
        TbIcon::Arrow => {
            p.line_segment([q(-5.0, 5.0), q(5.0, -5.0)], st);
            p.line_segment([q(5.0, -5.0), q(-0.5, -5.0)], st);
            p.line_segment([q(5.0, -5.0), q(5.0, 0.5)], st);
        }
        TbIcon::Pen => {
            p.add(poly(&[(-6.0, 6.0), (-5.0, 2.2), (2.3, -5.2), (5.2, -2.3), (-2.2, 5.0)]));
        }
        TbIcon::Highlighter => {
            p.line_segment([q(-3.5, 2.5), q(3.5, -4.5)], EStroke::new(4.5 * s, col.gamma_multiply(0.6)));
            p.line_segment([q(-6.0, 6.5), q(6.0, 6.5)], st);
        }
        TbIcon::Text => {
            p.line_segment([q(-5.0, -5.5), q(5.0, -5.5)], st);
            p.line_segment([q(-5.0, -5.5), q(-5.0, -3.5)], st);
            p.line_segment([q(5.0, -5.5), q(5.0, -3.5)], st);
            p.line_segment([q(0.0, -5.5), q(0.0, 6.0)], st);
            p.line_segment([q(-2.0, 6.0), q(2.0, 6.0)], st);
        }
        TbIcon::Pixelate => {
            p.rect_stroke(Rect::from_min_max(q(-5.5, -5.5), q(5.5, 5.5)), 1.0, st);
            for (x, y) in [(-5.5, -5.5), (0.0, 0.0)] {
                p.rect_filled(Rect::from_min_max(q(x, y), q(x + 5.5, y + 5.5)), 0.0, col);
            }
        }
        TbIcon::Eraser => {
            p.add(poly(&[(0.5, -6.5), (6.0, -1.0), (0.0, 5.0), (-5.5, -0.5)]));
            p.line_segment([q(-2.5, -3.5), q(3.0, 2.0)], st);
            p.line_segment([q(-3.0, 6.5), q(6.0, 6.5)], st);
        }
    }
}

/// Frame titles are drawn at this size on screen, whatever the zoom
const FRAME_TITLE_PX: f32 = 13.0;
/// Frame titles when not hovered/selected (they turn the accent colour then)
const FRAME_TITLE_COL: Color32 = Color32::from_rgb(0x69, 0x69, 0x69);

#[derive(Clone)]
enum Drag {
    None,
    Move { start: Pt, origins: Vec<(Sel, Pt)> },
    Resize { start: Pt, ox: f32, oy: f32, ow: f32, oh: f32, left: bool, top: bool, rx: bool, ry: bool, sel: Sel },
    BoxSelect { start: Pt, current: Pt, initial: HashSet<Sel> },
    Draw,
    Erase,
    Frame { start: Pt, id: String },
    /// Dragging a text label that sits on an image (coords in image pixels)
    MoveLabel { image: String, label: String, grab: Pt },
    /// Dragging a bulk-spacing handle: (sel, start rect) in order along `axis`, plus frame contents
    /// Resizing a multi-selection from its box: start rects of the items and of frame contents
    MultiResize { start: Pt, bx: [f32; 4], left: bool, top: bool, rx: bool, ry: bool, items: Vec<(Sel, [f32; 4])>, follow: Vec<(Sel, [f32; 4], usize)> },
    Gap { idx: usize, axis: usize, start: Pt, gap0: f32, items: Vec<(Sel, [f32; 4])>, follow: Vec<(Sel, [f32; 4], usize)> },
}

enum EditTarget {
    NewLabel { image: Option<String> },
    ImageLabel { image: String, label: String },
    Note(String),
    Group(String),
    WorldText(String),
}

struct Edit {
    target: EditTarget,
    world: Pt,
    text: String,
    fresh: u8,
}

#[derive(Clone)]
enum Prompt {
    NewBoard(String),
    Rename(String),
    ConfirmDelete,
}

struct SettingsDraft {
    summon: String,
    dump: String,
    error: String,
}

#[allow(dead_code)]
enum Act {
    /// Shrink / grow the selected frames to wrap what's inside them
    FitFrame,
    /// Corner radius for the selected pictures
    ImageCorner(f32),
    /// Aim the next "add" action at the middle of the view (toolbar buttons)
    CentreMenu,
    CycleColor,
    ToggleFill,
    ToggleStroke,
    ToggleClip,
    FrameLayout(u8),
    /// Reshape the selected frame to width / height = ratio, keeping its width
    FrameAspect(f32),
    Corner(f32),
    ResetStretch,
    Border(f32),
    ArrangeAuto(String),
    TextFontSize(f32),
    StrokeWidth(f32),
    Paste,
    Tool(Tool),
    Color(String),
    Width(f32),
    /// Right-click on a toolbar button: (which options, button rect)
    ToolMenu(u8, Rect),
    AddNote,
    AddGroup,
    LabelHere,
    Undo,
    Redo,
    ZoomReset,
    ZoomFit,
    SwitchBoard(String),
    NewBoard,
    RenameBoard,
    DeleteBoard,
    Settings,
    Hide,
    Quit,
    CopyBaked,
    CopyClean,
    ClearAnnotations,
    Delete,
    Activate,
    NoteColor(String),
    NoteFontSize(f32),
    NoteOpacity(f32),
    GroupColor(String),
    GroupFontSize(f32),
    Front,
}

#[derive(Clone, Copy)]
struct View {
    origin: Pos2,
    cam: Camera,
}

impl View {
    fn s(&self, x: f32, y: f32) -> Pos2 {
        pos2(
            self.origin.x + (x - self.cam.x) * self.cam.zoom,
            self.origin.y + (y - self.cam.y) * self.cam.zoom,
        )
    }
    fn w(&self, p: Pos2) -> Pt {
        Pt {
            x: (p.x - self.origin.x) / self.cam.zoom + self.cam.x,
            y: (p.y - self.origin.y) / self.cam.zoom + self.cam.y,
        }
    }
}

struct App {
    settings: Settings,
    board: Board,
    boards: Vec<(String, String)>,
    undo: Vec<Board>,
    redo: Vec<Board>,
    dirty_since: Option<f64>,
    now: f64,

    tool: Tool,
    pen_color: String,
    pen_width: f32,
    /// Blur (pixelate) brush size, world px
    blur_size: f32,
    sel: Option<Sel>,
    /// Selected text label on an image: (image id, label id)
    sel_label: Option<(String, String)>,
    selected: HashSet<Sel>,
    drag: Drag,
    drag_board: Option<Board>,
    drag_changed: bool,
    /// Guide lines (world x0,y0,x1,y1) from the last snapped move
    snap_guides: Vec<[f32; 4]>,
    /// Ctrl held: move without snapping
    snap_off: bool,
    /// Equal-gap markers while moving: segments [x0, y0, x1, y1] in world space
    gap_marks: Vec<[f32; 4]>,
    /// Alt-drag duplicate: where the original stays (world box), measured against the copy
    dup_origin: Option<[f32; 4]>,
    /// While resizing: objects whose width / height the resized one now matches
    size_match: (Option<[f32; 4]>, Option<[f32; 4]>),
    active: Option<(Option<String>, Stroke)>,
    edit: Option<Edit>,
    menu_world: Pt,
    menu_hit: Option<Sel>,
    /// Camera when the menu opened; a zoom or pan closes the menu
    menu_cam: (f32, f32, f32),
    /// Toolbar options popup: 0 select, 1 frame, 2 shapes, 3 pen-like
    tool_menu: Option<(u8, Rect)>,
    tool_menu_last: (u8, Rect),
    tool_sub: Option<usize>,
    /// Left-drag on empty canvas pans instead of box-selecting
    hand_drag: bool,
    /// Index into FRAME_SIZES for new frames
    frame_size: usize,
    /// 0 outline, 1 fill + outline, 2 fill only (rectangle / ellipse)
    shape_fill: u8,
    frame_square: bool,
    text_size: f32,
    text_style: u8,
    /// When the hovered submenu last changed (for its slide-in)
    tool_sub_since: f64,

    textures: HashMap<String, (TextureHandle, f64)>,
    missing: HashSet<String>,
    /// How far each loaded texture was shrunk (0 = full size, n = 1/2^n).
    tex_lvl: HashMap<String, u8>,
    asleep: bool,
    /// Image decoding happens on a worker thread so panning never waits on the disk.
    jobs: mpsc::Sender<(std::path::PathBuf, String, u8)>,
    done: Receiver<(String, u8, Option<(u32, u32, Vec<u8>)>)>,
    pending: HashSet<String>,
    /// Coarse copies of images that have pixelated areas (a few KB each).
    mosaics: HashMap<String, TextureHandle>,
    // F3 performance read-out
    stats_on: bool,
    frames: std::collections::VecDeque<f64>,
    ui_ms: f32,
    cpu_pct: f32,
    cpu_prev: (f64, f64),
    toasts: Vec<(String, f64)>,
    prompt: Option<Prompt>,
    settings_draft: Option<SettingsDraft>,
    /// Right-hand customize panel
    panel_open: bool,
    /// Which shortcut is listening for keys: 0 = show/hide, 1 = quick-dump
    recording: Option<u8>,
    hk_error: String,
    shortcuts_open: bool,
    corners_dirty: bool,
    last_canvas: Rect,
    noise: Option<egui::TextureHandle>,

    rx: Receiver<Msg>,
    reload: Arc<AtomicBool>,
    hk_ids: Arc<Mutex<(u32, u32)>>,
    hk_mgr: Option<GlobalHotKeyManager>,
    hk_registered: Vec<HotKey>,
    _tray: Option<tray_icon::TrayIcon>,

    last_rect: Rect,
    hide_in: u8,
    quitting: bool,
    mem_mb: f32,
    mem_at: f64,
    last_paste: f64,
    ctrl_v_was: bool,
    trimmed: bool,
    was_focused: bool,
    /// Smooth camera move for zoom-to-fit / reset: (from, to, start time)
    cam_anim: Option<(Camera, Camera, f64)>,
    /// Start corner and arrow-head length of the shape being dragged out
    shape_anchor: (Pt, f32),
    shift: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_style(&cc.egui_ctx);
        let _ = CTX.set(cc.egui_ctx.clone());
        let mut settings = load_settings();
        THEME.store(settings.canvas_theme, std::sync::atomic::Ordering::Relaxed);
        ACCENT.store(settings.accent, std::sync::atomic::Ordering::Relaxed);
        GRADIENTS.store(settings.gradients, std::sync::atomic::Ordering::Relaxed);
        GRAD_DIR.store(settings.gradient_dir as usize, std::sync::atomic::Ordering::Relaxed);
        AURA_PAL.store(settings.aura_palette as usize, std::sync::atomic::Ordering::Relaxed);
        AURA_MOT.store(settings.aura_motion as usize, std::sync::atomic::Ordering::Relaxed);
        ANIM.store(settings.anim, std::sync::atomic::Ordering::Relaxed);
        if settings.outline_width > 0.0 {
            ui::outline::set_width(settings.outline_width);
        }
        retint(&cc.egui_ctx);
        let boards = list_boards();
        if !boards.iter().any(|b| b.0 == settings.active_board_id) {
            settings.active_board_id = boards[0].0.clone();
            save_settings(&settings);
        }
        let board = load_board(&settings.active_board_id);

        let (tx, rx) = mpsc::channel();
        let (jobs, job_rx) = mpsc::channel::<(std::path::PathBuf, String, u8)>();
        let (done_tx, done) = mpsc::channel();
        let wctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            while let Ok((path, file, l)) = job_rx.recv() {
                let out = platform::load_rgba(&path).ok().map(|(w, h, rgba)| {
                    if l == MOSAIC {
                        if let Some(img) = image::RgbaImage::from_raw(w, h, rgba.clone()) {
                            let (nx, ny) = platform::mosaic_dims(w, h);
                            let t = image::imageops::thumbnail(&img, nx, ny);
                            return (t.width(), t.height(), t.into_raw());
                        }
                    }
                    if l > 0 && (w.min(h) >> l) >= 32 {
                        if let Some(img) = image::RgbaImage::from_raw(w, h, rgba.clone()) {
                            let t = image::imageops::thumbnail(&img, w >> l, h >> l);
                            return (t.width(), t.height(), t.into_raw());
                        }
                    }
                    (w, h, rgba)
                });
                if done_tx.send((file, l, out)).is_err() {
                    break;
                }
                wctx.request_repaint();
            }
        });
        let hk_ids = Arc::new(Mutex::new((0u32, 0u32)));
        let reload = Arc::new(AtomicBool::new(false));
        platform::install_handlers(Bridge {
            tx,
            ctx: cc.egui_ctx.clone(),
            ids: hk_ids.clone(),
            reload: reload.clone(),
        });
        let tray = image::load_from_memory(include_bytes!("../assets/icon32.png"))
            .ok()
            .map(|i| i.into_rgba8())
            .and_then(|i| {
                let (w, h) = (i.width(), i.height());
                platform::build_tray(i.into_raw(), w, h)
            });

        let mut app = Self {
            settings,
            board,
            boards,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty_since: None,
            now: 0.0,
            tool: Tool::Select,
            pen_color: "#ef4444".into(),
            pen_width: 4.0,
            blur_size: 28.0,
            sel: None,
            sel_label: None,
            selected: HashSet::new(),
            drag: Drag::None,
            drag_board: None,
            drag_changed: false,
            snap_guides: Vec::new(),
            snap_off: false,
            gap_marks: Vec::new(),
            dup_origin: None,
            size_match: (None, None),
            active: None,
            edit: None,
            menu_world: Pt::default(),
            menu_hit: None,
            menu_cam: (0.0, 0.0, 0.0),
            tool_menu: None,
            tool_menu_last: (0, Rect::NOTHING),
            tool_sub: None,
            hand_drag: false,
            frame_size: 0,
            shape_fill: 0,
            frame_square: false,
            text_size: 16.0,
            text_style: 0,
            tool_sub_since: 0.0,
            textures: HashMap::new(),
            missing: HashSet::new(),
            tex_lvl: HashMap::new(),
            asleep: false,
            jobs,
            done,
            pending: HashSet::new(),
            mosaics: HashMap::new(),
            stats_on: false,
            frames: Default::default(),
            ui_ms: 0.0,
            cpu_pct: 0.0,
            cpu_prev: (0.0, 0.0),
            toasts: Vec::new(),
            prompt: None,
            settings_draft: None,
            panel_open: false,
            recording: None,
            hk_error: String::new(),
            shortcuts_open: false,
            corners_dirty: true,
            last_canvas: Rect::NOTHING,
            noise: {
                // Additive grain: colour 0-3, alpha 0, so it only ever lifts by a hair
                let mut seed: u32 = 0x9e37_79b9;
                let px: Vec<Color32> = (0..128 * 128)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        let v = (seed % 4) as u8;
                        Color32::from_rgba_premultiplied(v, v, v, 0)
                    })
                    .collect();
                let img = egui::ColorImage { size: [128, 128], pixels: px };
                let opts = egui::TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    minification: egui::TextureFilter::Nearest,
                    wrap_mode: egui::TextureWrapMode::Repeat,
                    ..Default::default()
                };
                Some(cc.egui_ctx.load_texture("qd_noise", img, opts))
            },
            rx,
            reload,
            hk_ids,
            hk_mgr: GlobalHotKeyManager::new().ok(),
            hk_registered: Vec::new(),
            _tray: tray,
            last_rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0)),
            hide_in: 0,
            quitting: false,
            mem_mb: 0.0,
            mem_at: -10.0,
            last_paste: -10.0,
            ctrl_v_was: false,
            trimmed: false,
            was_focused: false,
            cam_anim: None,
            shape_anchor: (Pt::default(), 0.0),
            shift: false,
        };
        if let Err(e) = app.register_hotkeys() {
            app.toast(e);
        }
        app
    }

    // ---------- small helpers ----------

    fn toast(&mut self, msg: impl Into<String>) {
        self.toasts.push((msg.into(), self.now));
    }

    fn handle_window_resize(&self, ctx: &egui::Context) {
        let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if is_max {
            return;
        }
        let screen = ctx.screen_rect();
        let w = screen.width();
        let h = screen.height();
        if w < 100.0 || h < 100.0 {
            return;
        }

        let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
        let m = 5.0_f32;
        let c = 12.0_f32;

        let left = pos.x <= m;
        let right = pos.x >= w - m;
        let top = pos.y <= m;
        let bottom = pos.y >= h - m;

        let left_c = pos.x <= c;
        let right_c = pos.x >= w - c;
        let top_c = pos.y <= c;
        let bottom_c = pos.y >= h - c;

        let (dir, cursor) = if top_c && left_c {
            (Some(ResizeDirection::NorthWest), CursorIcon::ResizeNorthWest)
        } else if top_c && right_c {
            (Some(ResizeDirection::NorthEast), CursorIcon::ResizeNorthEast)
        } else if bottom_c && left_c {
            (Some(ResizeDirection::SouthWest), CursorIcon::ResizeSouthWest)
        } else if bottom_c && right_c {
            (Some(ResizeDirection::SouthEast), CursorIcon::ResizeSouthEast)
        } else if top {
            (Some(ResizeDirection::North), CursorIcon::ResizeVertical)
        } else if bottom {
            (Some(ResizeDirection::South), CursorIcon::ResizeVertical)
        } else if left {
            (Some(ResizeDirection::West), CursorIcon::ResizeHorizontal)
        } else if right {
            (Some(ResizeDirection::East), CursorIcon::ResizeHorizontal)
        } else {
            (None, CursorIcon::Default)
        };

        if let Some(dir) = dir {
            ctx.set_cursor_icon(cursor);
            if ctx.input(|i| i.pointer.button_pressed(PointerButton::Primary)) {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
            }
        }
    }

    fn register_hotkeys(&mut self) -> Result<(), String> {
        let Some(mgr) = &self.hk_mgr else {
            return Err("Global hotkeys are unavailable".into());
        };
        let _ = mgr.unregister_all(&self.hk_registered);
        self.hk_registered.clear();
        let summon: HotKey = self
            .settings
            .summon_hotkey
            .parse()
            .map_err(|e| format!("Summon hotkey: {e}"))?;
        let dump: HotKey = self
            .settings
            .quick_dump_hotkey
            .parse()
            .map_err(|e| format!("Quick-dump hotkey: {e}"))?;
        let mut errors = Vec::new();
        for (name, hk) in [("Summon", summon), ("Quick-dump", dump)] {
            match mgr.register(hk) {
                Ok(()) => self.hk_registered.push(hk),
                Err(_) => errors.push(format!("{name} hotkey is already used by another app")),
            }
        }
        *self.hk_ids.lock().unwrap() = (summon.id(), dump.id());
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join(" · "))
        }
    }

}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = chrome();
        [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ui::scale::update_ui_scale(ctx);
        self.now = ctx.input(|i| i.time);
        self.stamp_parents();
        if matches!(self.drag, Drag::None) {
            let mut auto = Vec::new();
            for g in self.board.group_cards.iter().filter(|g| g.layout > 0) {
                let ids: Vec<String> = self
                    .board
                    .images
                    .iter()
                    .filter(|im| {
                        let (cx, cy) = (im.x + im.width / 2.0, im.y + im.height / 2.0);
                        // Stored frame membership decides; centre position only for objects not stamped yet
                        self.board.parents.get(&im.id).map_or(cx > g.x && cx < g.x + g.width && cy > g.y && cy < g.y + g.height, |p| p == &g.id)
                    })
                    .map(|im| im.id.clone())
                    .collect();
                let kid = egui::Id::new(("qd_arr_ids", g.id.as_str()));
                let prev: Option<Vec<String>> = ctx.data(|d| d.get_temp(kid));
                if prev.as_ref().is_some_and(|p| ids.iter().any(|i| !p.contains(i))) {
                    auto.push(g.id.clone());
                }
                ctx.data_mut(|d| d.insert_temp(kid, ids));
            }
            for id in auto {
                self.apply(Act::ArrangeAuto(id), ctx, ctx.screen_rect());
            }
        }
        // Frame arrange animation: ease every image (and the frame) from its old rect to the new one
        if let Some((t0, tw)) = ctx.data(|d| d.get_temp::<Tween>(egui::Id::new("qd_layout_tw"))) {
            let k = (((self.now - t0) / 0.45) as f32).clamp(0.0, 1.0);
            let e = 1.0 - (1.0 - k).powi(4);
            for (frame, id, a, b) in &tw {
                let v: Vec<f32> = (0..4).map(|j| a[j] + (b[j] - a[j]) * e).collect();
                if *frame {
                    if let Some(g) = self.board.group_cards.iter_mut().find(|g| &g.id == id) {
                        (g.x, g.y, g.width, g.height) = (v[0], v[1], v[2], v[3]);
                    }
                } else if let Some(im) = self.board.images.iter_mut().find(|i| &i.id == id) {
                    (im.x, im.y, im.width, im.height) = (v[0], v[1], v[2], v[3]);
                }
            }
            if k >= 1.0 {
                ctx.data_mut(|d| d.remove::<Tween>(egui::Id::new("qd_layout_tw")));
            } else {
                ctx.request_repaint();
            }
        }
        let t0 = std::time::Instant::now();
        if self.corners_dirty {
            self.corners_dirty = false;
            platform::set_round_corners(self.settings.round_corners);
        }
        if aurora() && !self.asleep {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        if ctx.input(|i| i.key_pressed(Key::F3)) {
            self.stats_on = !self.stats_on;
        }

        // A quick-dump landed on disk while we were hidden
        if self.reload.swap(false, Ordering::SeqCst) {
            self.board = load_board(&self.settings.active_board_id);
            self.undo.clear();
            self.redo.clear();
            self.dirty_since = None;
            self.deselect_all();
            self.toast("Quick-dump added while hidden");
        }

        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Tangra => {
                    let rect = self.last_rect;
                    let n = (self.board.images.len() % 10) as f32 * 24.0;
                    let c = self.view(rect).w(rect.center() + vec2(n, n));
                    self.paste_at(ctx, c, true);
                }
                Msg::Settings => self.open_settings(),
                Msg::Quit => self.quit(ctx),
                Msg::Hide => self.begin_hide(ctx),
            }
        }

        // The window X hides to tray instead of quitting
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.hide_in == 0 {
                self.begin_hide(ctx);
            }
        }

        // Hidden to tray: draw nothing and ask for nothing until the window is back
        if self.asleep && self.hide_in == 0 {
            if platform::window_live() {
                self.asleep = false;
            } else {
                return;
            }
        }
        if self.hide_in > 0 {
            self.hide_in -= 1;
            egui::CentralPanel::default().frame(egui::Frame::none().fill(chrome())).show(ctx, |_| {});
            if self.hide_in == 0 {
                platform::hide_window();
                self.asleep = true;
                platform::trim_memory();
            } else {
                ctx.request_repaint();
            }
            return;
        }

        self.handle_window_resize(ctx);
        self.keys(ctx);

        // Glide the camera for zoom-to-fit / reset; any pan or scroll cancels it
        if let Some((from, to, start)) = self.cam_anim {
            let interrupted = ctx.input(|i| i.pointer.any_down() || i.raw_scroll_delta != egui::Vec2::ZERO || i.zoom_delta() != 1.0);
            let k = (((self.now - start) / 0.3) as f32).clamp(0.0, 1.0);
            if interrupted {
                self.cam_anim = None;
            } else if k >= 1.0 {
                self.board.camera = to;
                self.cam_anim = None;
            } else {
                let e = egui::emath::easing::cubic_out(k);
                let size = self.last_rect.size();
                let centre = |c: Camera| (c.x + size.x / (2.0 * c.zoom), c.y + size.y / (2.0 * c.zoom));
                let ((fx, fy), (tx, ty)) = (centre(from), centre(to));
                let zoom = from.zoom * (to.zoom / from.zoom).powf(e);
                self.board.camera = Camera {
                    x: fx + (tx - fx) * e - size.x / (2.0 * zoom),
                    y: fy + (ty - fy) * e - size.y / (2.0 * zoom),
                    zoom,
                    ..to
                };
                ctx.request_repaint();
            }
        }

        let mut view = self.view(self.last_rect);
        egui::CentralPanel::default().frame(egui::Frame::none().fill(chrome())).show(ctx, |ui| {
            let full_rect = ui.max_rect();
            let titlebar_h = 44.0_f32;
            let titlebar_rect = Rect::from_min_max(full_rect.min, pos2(full_rect.max.x, full_rect.min.y + titlebar_h));
            let canvas_rect = Rect::from_min_max(pos2(full_rect.min.x, full_rect.min.y + titlebar_h), full_rect.max);
            self.last_rect = canvas_rect;

            // 1. Canvas interaction & drawing
            let resp = ui.allocate_rect(canvas_rect, Sense::click_and_drag());
            self.input(ctx, &resp, canvas_rect);
            view = self.view(canvas_rect);
            self.ensure_textures(ctx, view, canvas_rect);

            // Canvas painter clipped strictly to canvas_rect so zoomed/panned items never bleed into titlebar
            let canvas_painter = ui.painter().with_clip_rect(canvas_rect);
            self.paint(&canvas_painter, canvas_rect, view, ctx.pointer_hover_pos());

            // 2. Titlebar UI drawn on top of the canvas junction with the unclipped window painter
            self.titlebar_ui(ui, ctx, titlebar_rect);

            let mut acts = Vec::new();
            // 3. Interactive bottom HUD overlay (Undo, Redo, Zoom reset/percentage, Zoom fit, Status)
            self.last_canvas = canvas_rect;
            self.bottom_bar_ui(ui, &mut acts, canvas_rect);
            self.toolbar_ui(ui, &mut acts, canvas_rect);
            self.style_bar_ui(ui, &mut acts, canvas_rect);
            self.place_bar_ui(ui, canvas_rect);
            let pctx = ui.ctx().clone();
            self.side_panel_ui(&pctx, canvas_rect);
            self.outline_ui(&pctx, canvas_rect);
            self.tool_menu_ui(&pctx);

            resp.context_menu(|ui| self.menu_ui(ui, &mut acts));
            let cm_open = resp.context_menu_opened();
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("qd_cm_open"), cm_open));
            match ctx.data_mut(|d| d.remove_temp::<u8>(egui::Id::new("qd_key_zoom"))) {
                Some(1) => acts.push(Act::ZoomReset),
                Some(2) => acts.push(Act::ZoomFit),
                _ => {}
            }
            for a in acts {
                let glide = matches!(a, Act::ZoomFit | Act::ZoomReset);
                let from = self.board.camera;
                self.apply(a, ctx, canvas_rect);
                if glide {
                    self.cam_anim = Some((from, self.board.camera, self.now));
                    self.board.camera = from;
                    ctx.request_repaint();
                }
            }
        });

        // Dialogs dim the canvas behind them
        let dim = ease(ctx, egui::Id::new("qd_dim"), self.prompt.is_some() || self.shortcuts_open);
        if dim > 0.0 {
            ctx.layer_painter(egui::LayerId::new(egui::Order::PanelResizeLine, egui::Id::new("qd_dim")))
                .rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha((dim * 110.0) as u8));
        }

        self.edit_ui(ctx, view);
        self.prompt_ui(ctx);

        // ---- housekeeping ----
        let now = self.now;
        // Real culling: textures that have been off-screen for a while are freed
        self.textures.retain(|_, t| now - t.1 < 15.0);

        let focused = ctx.input(|i| i.focused);
        // Hand startup-only pages (shader compile, font build) back to Windows once,
        // and again whenever the window loses focus.
        if (!self.trimmed && now > 2.0) || (self.was_focused && !focused) {
            self.trimmed = true;
            platform::trim_memory();
        } else if !self.trimmed {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        self.was_focused = focused;
        if let Some(t) = self.dirty_since {
            if !focused || now - t > 0.6 {
                self.save_now();
            } else {
                ctx.request_repaint_after(Duration::from_millis(300));
            }
        }
        let keep = self.toasts.len().saturating_sub(2);
        self.toasts.drain(..keep);
        self.toasts.retain(|t| now - t.1 < 3.2);
        if !self.toasts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if now - self.mem_at > 2.0 {
            self.mem_mb = platform::memory_mb();
            self.mem_at = now;
            // Forget everything kept for images that are no longer on the board
            let live: HashSet<&str> = self.board.images.iter().map(|i| i.asset_file_name.as_str()).collect();
            self.textures.retain(|f, _| live.contains(f.as_str()));
            self.tex_lvl.retain(|f, _| live.contains(f.as_str()));
            self.mosaics.retain(|f, _| live.contains(f.as_str()));
            self.missing.retain(|f| live.contains(f.as_str()));
        }
        if focused {
            ctx.request_repaint_after(Duration::from_secs(3));
        }
        if self.stats_on {
            self.frames.push_back(now);
            while self.frames.front().is_some_and(|t| now - t > 1.0) {
                self.frames.pop_front();
            }
            if now - self.cpu_prev.1 >= 1.0 {
                let cpu = platform::cpu_ms();
                let cores = std::thread::available_parallelism().map_or(1.0, |n| n.get() as f64);
                self.cpu_pct = ((cpu - self.cpu_prev.0) / ((now - self.cpu_prev.1) * 1000.0 * cores) * 100.0) as f32;
                self.cpu_prev = (cpu, now);
            }
            self.ui_ms = t0.elapsed().as_secs_f32() * 1000.0;
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        cm_ghost(ctx);
    }
}

fn setup_style(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
        fonts.font_data.insert("segoe".into(), egui::FontData::from_owned(bytes));
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            f.insert(0, "segoe".into());
        }
    }
    // Bold, italic and bold italic: Segoe's own faces, falling back to the regular list
    let base = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    for (name, file) in [("b", "segoeuib.ttf"), ("i", "segoeuii.ttf"), ("bi", "segoeuiz.ttf"), ("sb", "seguisb.ttf")] {
        let mut list = base.clone();
        if let Ok(bytes) = std::fs::read(format!("C:\\Windows\\Fonts\\{file}")) {
            let key = format!("segoe_{name}");
            fonts.font_data.insert(key.clone(), egui::FontData::from_owned(bytes));
            list.insert(0, key);
        }
        fonts.families.insert(egui::FontFamily::Name(name.into()), list);
    }
    ctx.set_fonts(fonts);

    let mut st = (*ctx.style()).clone();
    st.visuals = egui::Visuals::dark();
    let v = &mut st.visuals;
    v.panel_fill = frame_bg();
    v.window_fill = popup();
    v.window_stroke = EStroke::new(1.0_f32, border());
    v.window_rounding = Rounding::same(10.0);
    v.menu_rounding = Rounding::same(10.0);
    let shadow = egui::epaint::Shadow {
        // Kept nearly centred: a shadow pushed far down darkens only the area under
        // a popup, which makes its bottom border read as a bright line.
        // The blur must stay well under the height of the smallest popup (a one-line
        // tooltip, ~26 px): egui draws the blur as edge feathering, and when it is as
        // big as the popup itself the shape folds over and leaves a pale line.
        offset: vec2(0.0, 3.0),
        blur: 8.0,
        spread: 1.0,
        color: Color32::from_black_alpha(90),
    };
    v.window_stroke = EStroke::new(1.0_f32, border());
    v.window_shadow = shadow;
    v.popup_shadow = shadow;
    v.extreme_bg_color = Color32::from_rgb(24, 24, 24);
    v.selection.bg_fill = blue();
    v.selection.stroke = EStroke::new(1.0_f32, Color32::from_rgb(147, 197, 253));
    v.override_text_color = Some(TEXT);
    v.widgets.noninteractive.bg_stroke = EStroke::new(1.0_f32, border());
    v.widgets.inactive.bg_fill = Color32::from_gray(52);
    v.widgets.inactive.weak_bg_fill = Color32::from_gray(52);
    v.widgets.hovered.bg_fill = Color32::from_gray(66);
    v.widgets.hovered.weak_bg_fill = Color32::from_gray(66);
    v.widgets.hovered.bg_stroke = EStroke::NONE;
    v.widgets.active.bg_fill = Color32::from_gray(80);
    v.widgets.active.weak_bg_fill = Color32::from_gray(80);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.rounding = Rounding::same(8.0);
    }
    st.spacing.button_padding = vec2(10.0, 5.0);
    st.spacing.item_spacing = vec2(8.0, 3.0);
    st.spacing.menu_margin = egui::Margin::same(6.0);
    st.spacing.window_margin = egui::Margin::same(14.0);
    for (style, size) in [
        (egui::TextStyle::Body, 13.5),
        (egui::TextStyle::Button, 13.5),
        (egui::TextStyle::Small, 11.0),
        (egui::TextStyle::Heading, 16.0),
    ] {
        st.text_styles.insert(style, FontId::proportional(size));
    }
    st.animation_time = 0.14;
    // Force our dark style even when Windows is in light mode (otherwise egui
    // swaps to its stock light theme and popups turn white).
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|s| *s = st.clone());
}

fn main() -> eframe::Result<()> {
    // Single instance: if Tangra is already running, just bring it forward
    #[cfg(not(debug_assertions))]
    if platform::window_exists() {
        platform::show_window();
        return Ok(());
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(platform::TITLE)
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([480.0, 320.0])
        .with_decorations(false)
        .with_resizable(true)
        .with_drag_and_drop(true);
    if let Ok(icon) = image::load_from_memory(include_bytes!("../assets/icon128.png")) {
        let icon = icon.into_rgba8();
        viewport = viewport.with_icon(egui::IconData {
            width: icon.width(),
            height: icon.height(),
            rgba: icon.into_raw(),
        });
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };
    eframe::run_native("Tangra", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
