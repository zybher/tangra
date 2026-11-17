//! Board data model + on-disk storage.
//! File format and location are identical to the old Tauri build, so boards carry over.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Stroke {
    pub id: String,
    pub points: Vec<Pt>,
    pub color: String,
    pub width: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    pub created_at: f64,
    /// Closed shapes only: paint the inside
    #[serde(skip_serializing_if = "is_false")]
    pub fill: bool,
    /// Closed shapes only: hide the outline (fill only)
    #[serde(skip_serializing_if = "is_false")]
    pub no_stroke: bool,
    /// On images: this swipe pixelates what is under it instead of drawing a colour
    #[serde(skip_serializing_if = "is_false")]
    pub pixelate: bool,
    /// Rectangles only: corner radius in world units
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    /// Closed shapes only: where the outline sits on the edge, like a frame's (0 centre, 1 inside,
    /// 2 outside); new shapes start inside
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub stroke_pos: u8,
    /// Spots rubbed out by the eraser, as (x, y, rx, ry) fractions of its box so they move
    /// and stretch with it (canvas/holes.rs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub erase: Vec<[f32; 4]>,
    /// Loops cut out with the lasso, as fractions of its box (canvas/holes.rs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub erase_loops: Vec<Vec<[f32; 2]>>,
    /// Arrows only: head style (canvas/arrow_mesh.rs: 0 line, 1 triangle, 2 reversed, 3 circle, 4 diamond)
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub tip: u8,
    /// Arrows only: line ends (0 round, 1 square)
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub cap: u8,
    /// Pen paths (and shapes turned into one to edit their points): the points with their
    /// curve handles. `points` is always this path flattened (`reflatten`), so drawing,
    /// hit-testing, the eraser and moving / resizing work on paths with no extra code
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub knots: Vec<Knot>,
    /// Pen paths only: the last point joins back to the first
    #[serde(default, skip_serializing_if = "is_false")]
    pub closed: bool,
}

/// One point of a pen path: where it is and its two curve handles (`i` toward the point
/// before, `o` toward the one after; a handle on the point itself = a sharp corner that side)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Knot {
    pub p: Pt,
    pub i: Pt,
    pub o: Pt,
    /// Rounding of a sharp corner (world units; drawn as an arc, the point itself stays put)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub r: f32,
    /// How its handles follow each other: 0 same angle, 1 each on its own, 2 same angle and length
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub m: u8,
}

impl Knot {
    pub fn corner(p: Pt) -> Self {
        Knot { p, i: p, o: p, ..Default::default() }
    }
    pub fn sharp(&self) -> bool {
        self.i == self.p && self.o == self.p
    }
}

/// Point `t` (0..1) along the curve a -> b -> c -> d
pub fn cubic(a: Pt, b: Pt, c: Pt, d: Pt, t: f32) -> Pt {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    Pt { x: a.x * w0 + b.x * w1 + c.x * w2 + d.x * w3, y: a.y * w0 + b.y * w1 + c.y * w2 + d.y * w3 }
}

/// Samples per path segment when flattened
pub const SEG_STEPS: usize = 24;

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn is_false(b: &bool) -> bool {
    !*b
}

impl Stroke {
    /// Number of points forming the closed outline, if this is a closed shape
    /// (rectangle / ellipse made by the shape tools).
    pub fn closed_len(&self) -> Option<usize> {
        let n = self.points.len();
        if n < 5 {
            return None;
        }
        let near = |a: &Pt, b: &Pt| (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01;
        if near(&self.points[0], &self.points[n - 1]) {
            Some(n - 1)
        } else if near(&self.points[0], &self.points[n - 2]) {
            Some(n - 2)
        } else {
            None
        }
    }
    /// Which tool made it: "rectangle", "ellipse", "arrow", "highlighter", "pencil" or "pen" (a pen path) (also the
    /// toolbar icon's name)
    pub fn tool_kind(&self) -> &'static str {
        if !self.knots.is_empty() {
            return "pen";
        }
        let n = self.points.len();
        match self.closed_len() {
            Some(4) => "rectangle",
            Some(_) => "ellipse",
            None if n == 5 && self.points[1].x == self.points[3].x && self.points[1].y == self.points[3].y => "arrow",
            None if self.opacity.unwrap_or(1.0) < 0.5 => "highlighter",
            None => "pencil",
        }
    }
    /// Segment `j` of a path: its two ends and the handles between them
    pub fn segment(&self, j: usize) -> (Pt, Pt, Pt, Pt) {
        let (a, b) = (self.knots[j], self.knots[(j + 1) % self.knots.len()]);
        (a.p, a.o, b.i, b.p)
    }
    pub fn segments(&self) -> usize {
        let n = self.knots.len();
        if n < 2 {
            0
        } else if self.closed {
            n
        } else {
            n - 1
        }
    }
    /// Rebuilds `points` from the knots (a closed path ends on its first point, so it reads as
    /// a closed shape everywhere)
    pub fn reflatten(&mut self) {
        let mut pts = Vec::new();
        let segs = self.drawn_segments();
        for &(a, b, c, d) in &segs {
            for s in 0..SEG_STEPS {
                pts.push(cubic(a, b, c, d, s as f32 / SEG_STEPS as f32));
            }
        }
        match (self.closed, segs.first(), segs.last()) {
            (true, Some(f), _) => pts.push(f.0),
            (false, _, Some(l)) => pts.push(l.3),
            _ => {}
        }
        self.points = pts;
    }
    /// The segments as drawn: a rounded sharp corner becomes two points joined by an arc
    pub fn drawn_segments(&self) -> Vec<(Pt, Pt, Pt, Pt)> {
        let n = self.knots.len();
        if !self.knots.iter().any(|k| k.r > 0.0) {
            return (0..self.segments()).map(|j| self.segment(j)).collect();
        }
        let mut ks: Vec<Knot> = Vec::with_capacity(n * 2);
        let len = |a: Pt, b: Pt| ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        for (j, k) in self.knots.iter().enumerate() {
            let ends = !self.closed && (j == 0 || j + 1 == n);
            if k.r <= 0.0 || !k.sharp() || ends || n < 3 {
                ks.push(*k);
                continue;
            }
            let (pv, nx) = (self.knots[(j + n - 1) % n], self.knots[(j + 1) % n]);
            // toward the neighbours along the line as it leaves the corner
            let a = if pv.o != pv.p { pv.o } else { pv.p };
            let b = if nx.i != nx.p { nx.i } else { nx.p };
            let (la, lb) = (len(k.p, a), len(k.p, b));
            if la < 1e-3 || lb < 1e-3 {
                ks.push(*k);
                continue;
            }
            let u1 = Pt { x: (a.x - k.p.x) / la, y: (a.y - k.p.y) / la };
            let u2 = Pt { x: (b.x - k.p.x) / lb, y: (b.y - k.p.y) / lb };
            let th = (u1.x * u2.x + u1.y * u2.y).clamp(-1.0, 1.0).acos();
            if th < 0.01 || th > std::f32::consts::PI - 0.01 {
                ks.push(*k);
                continue;
            }
            // how far along each side the arc starts, at most half the side
            let t = (k.r / (th / 2.0).tan()).min(len(k.p, pv.p) / 2.0).min(len(k.p, nx.p) / 2.0);
            let rr = t * (th / 2.0).tan();
            let h = 4.0 / 3.0 * ((std::f32::consts::PI - th) / 4.0).tan() * rr;
            let at = |u: Pt, d: f32| Pt { x: k.p.x + u.x * d, y: k.p.y + u.y * d };
            let (pa, pb) = (at(u1, t), at(u2, t));
            ks.push(Knot { p: pa, i: pa, o: at(u1, t - h), ..Default::default() });
            ks.push(Knot { p: pb, i: at(u2, t - h), o: pb, ..Default::default() });
        }
        let m = ks.len();
        let segs = if self.closed { m } else { m.saturating_sub(1) };
        (0..segs).map(|j| { let (a, b) = (ks[j], ks[(j + 1) % m]); (a.p, a.o, b.i, b.p) }).collect()
    }
    /// A rectangle or ellipse becomes a closed path that looks the same, so its points can be
    /// edited (false for anything else: arrows, pencil lines)
    pub fn to_path(&mut self) -> bool {
        if !self.knots.is_empty() {
            return true;
        }
        let Some(n) = self.closed_len() else { return false };
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for q in &self.points[..n] {
            (x0, y0, x1, y1) = (x0.min(q.x), y0.min(q.y), x1.max(q.x), y1.max(q.y));
        }
        let (w, h) = (x1 - x0, y1 - y0);
        let pt = |x: f32, y: f32| Pt { x, y };
        let kn = |p: Pt, i: Pt, o: Pt| Knot { p, i, o, ..Default::default() };
        // handle length that makes a quarter curve a circle's
        const K: f32 = 0.5523;
        self.knots = if n == 4 {
            let r = self.radius.unwrap_or(0.0).clamp(0.0, w.min(h) / 2.0);
            if r <= 0.0 {
                vec![Knot::corner(pt(x0, y0)), Knot::corner(pt(x1, y0)), Knot::corner(pt(x1, y1)), Knot::corner(pt(x0, y1))]
            } else {
                let k = K * r;
                vec![
                    kn(pt(x0 + r, y0), pt(x0 + r - k, y0), pt(x0 + r, y0)),
                    kn(pt(x1 - r, y0), pt(x1 - r, y0), pt(x1 - r + k, y0)),
                    kn(pt(x1, y0 + r), pt(x1, y0 + r - k), pt(x1, y0 + r)),
                    kn(pt(x1, y1 - r), pt(x1, y1 - r), pt(x1, y1 - r + k)),
                    kn(pt(x1 - r, y1), pt(x1 - r + k, y1), pt(x1 - r, y1)),
                    kn(pt(x0 + r, y1), pt(x0 + r, y1), pt(x0 + r - k, y1)),
                    kn(pt(x0, y1 - r), pt(x0, y1 - r + k), pt(x0, y1 - r)),
                    kn(pt(x0, y0 + r), pt(x0, y0 + r), pt(x0, y0 + r - k)),
                ]
            }
        } else {
            let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, w / 2.0 * K, h / 2.0 * K);
            vec![
                kn(pt(cx, y0), pt(cx - rx, y0), pt(cx + rx, y0)),
                kn(pt(x1, cy), pt(x1, cy - ry), pt(x1, cy + ry)),
                kn(pt(cx, y1), pt(cx + rx, y1), pt(cx - rx, y1)),
                kn(pt(x0, cy), pt(x0, cy + ry), pt(x0, cy - ry)),
            ]
        };
        self.radius = None;
        self.closed = true;
        self.reflatten();
        true
    }
    pub fn fill_alpha(&self) -> f32 {
        self.opacity.unwrap_or(1.0).clamp(0.0, 1.0) * if self.no_stroke { 1.0 } else { 0.3 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Label {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub color: String,
    pub font_size: f32,
    /// 0 regular, 1 bold, 2 italic, 3 bold italic
    pub style: u8,
    /// Spots rubbed out by the eraser, as (x, y, rx, ry) fractions of its box so they move
    /// and stretch with it (canvas/holes.rs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub erase: Vec<[f32; 4]>,
    /// Loops cut out with the lasso, as fractions of its box (canvas/holes.rs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub erase_loops: Vec<Vec<[f32; 2]>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Annotations {
    pub strokes: Vec<Stroke>,
    pub texts: Vec<Label>,
}

impl Annotations {
    /// New ids for every drawing and label, so a copied picture's marks are its own
    pub fn fresh_ids(&mut self) {
        for s in &mut self.strokes {
            s.id = new_id();
        }
        for t in &mut self.texts {
            t.id = new_id();
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageNode {
    pub id: String,
    pub asset_file_name: String,
    pub original_width: f32,
    pub original_height: f32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub annotations: Annotations,
    pub created_at: f64,
    pub z_index: f64,
    /// Uniform scale before the image was first stretched (for Reset stretch)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_scale: Option<f32>,
    /// Corner radius in world units (0 = square)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub corner: f32,
    /// Colour adjustments, cut-out areas and 3D tilt (see image_edit.rs)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit: Option<crate::image_edit::ImageEdit>,
    /// Spots rubbed out by the eraser, as (x, y, rx, ry) fractions of its box so they move
    /// and stretch with it (canvas/holes.rs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub erase: Vec<[f32; 4]>,
    /// Loops cut out with the lasso, as fractions of its box (canvas/holes.rs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub erase_loops: Vec<Vec<[f32; 2]>>,
}

impl ImageNode {
    /// Scale from image-local (original pixel) coordinates to world units.
    pub fn scale(&self) -> (f32, f32) {
        let ow = if self.original_width > 0.0 { self.original_width } else { self.width };
        let oh = if self.original_height > 0.0 { self.original_height } else { self.height };
        ((self.width / ow).max(1e-4), (self.height / oh).max(1e-4))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LinkCard {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub notes: String,
    pub z_index: f64,
}

fn default_opacity() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Note {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text: String,
    pub color: String,
    pub font_size: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    pub z_index: f64,
}

impl Default for Note {
    fn default() -> Self {
        Self {
            id: String::new(),
            x: 0.0,
            y: 0.0,
            width: 260.0,
            height: 180.0,
            text: String::new(),
            color: "#303030".into(),
            font_size: 13.5,
            opacity: 1.0,
            z_index: 0.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Group {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub title: String,
    pub color: String,
    pub font_size: f32,
    pub z_index: f64,
    /// Crop images that stick out past the frame edge
    pub clip: bool,
    /// Sharp corners instead of rounded
    #[serde(default)]
    pub square: bool,
    /// Arrange mode kept on the frame (0 none, 1 row, 2 column, 3 grid, 4 gallery)
    #[serde(default)]
    pub layout: u8,
    /// Size unit an arranged frame keeps across layout switches
    #[serde(default)]
    pub layout_unit: f32,
    /// Corner radius in world units (None: from `square`)
    #[serde(default)]
    pub corner: Option<f32>,
    /// Border width in screen pixels (None: 1)
    #[serde(default)]
    pub border: Option<f32>,
    /// Where the border sits on the edge, like Figma: 0 centre, 1 inside, 2 outside
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub stroke_pos: u8,
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, zoom: 1.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Board {
    pub id: String,
    pub name: String,
    pub camera: Camera,
    pub images: Vec<ImageNode>,
    pub world_strokes: Vec<Stroke>,
    pub world_texts: Vec<Label>,
    pub link_cards: Vec<LinkCard>,
    pub sticky_notes: Vec<Note>,
    pub group_cards: Vec<Group>,
    /// Custom layer names set from the outline (object id -> name); frames use their title instead
    pub layer_names: std::collections::HashMap<String, String>,
    /// Object ids hidden / locked from the outline (frames pass it to what's inside)
    pub hidden: std::collections::HashSet<String>,
    pub locked: std::collections::HashSet<String>,
    /// Which frame each object belongs to (object id -> frame id, "" = the board itself)
    pub parents: std::collections::HashMap<String, String>,
    /// Per-object opacity 0..1 (missing = 1). A frame's opacity also fades what's inside it
    pub opacity: std::collections::HashMap<String, f32>,
    /// Order set by dragging rows in the outline (object id -> position among its siblings)
    pub layer_order: std::collections::HashMap<String, f32>,
    pub updated_at: f64,
    pub created_at: f64,
}

impl Board {
    pub fn add_image(&mut self, file: String, w: u32, h: u32, center: Pt) -> String {
        let node_w = (w as f32).min(850.0).max(1.0);
        let node_h = h as f32 * node_w / (w.max(1) as f32);
        let id = new_id();
        self.images.push(ImageNode {
            id: id.clone(),
            asset_file_name: file,
            original_width: w as f32,
            original_height: h as f32,
            x: center.x - node_w / 2.0,
            y: center.y - node_h / 2.0,
            width: node_w,
            height: node_h,
            annotations: Annotations::default(),
            created_at: now_ms(),
            z_index: self.images.len() as f64 + 1.0,
            base_scale: None,
            corner: 0.0,
            edit: None,
            ..Default::default()
        });
        self.name_image(&id, None);
        id
    }

    pub fn add_link(&mut self, url: &str, center: Pt) -> String {
        let domain = url.split('/').nth(2).unwrap_or(url).to_string();
        let id = new_id();
        self.link_cards.push(LinkCard {
            id: id.clone(),
            x: center.x - 160.0,
            y: center.y - 65.0,
            width: 320.0,
            height: 130.0,
            url: url.to_string(),
            title: domain.clone(),
            domain,
            notes: String::new(),
            z_index: self.link_cards.len() as f64 + 1.0,
        });
        id
    }

    pub fn add_note(&mut self, text: &str, center: Pt) -> String {
        let id = new_id();
        self.sticky_notes.push(Note {
            id: id.clone(),
            x: center.x - 130.0,
            y: center.y - 90.0,
            width: 260.0,
            height: 180.0,
            text: text.to_string(),
            color: "#303030".into(),
            font_size: 13.5,
            opacity: 1.0,
            z_index: self.sticky_notes.len() as f64 + 1.0,
        });
        id
    }

    pub fn add_group(&mut self, center: Pt) -> String {
        let id = new_id();
        self.group_cards.push(Group {
            id: id.clone(),
            x: center.x - 190.0,
            y: center.y - 140.0,
            width: 380.0,
            height: 280.0,
            title: "New Frame".into(),
            color: "#1e293b".into(),
            font_size: 13.0,
            z_index: self.group_cards.len() as f64 + 1.0,
            clip: false,
            square: true,
            layout: 0,
            layout_unit: 0.0,
            corner: None,
            border: None,
            stroke_pos: 1,
        });
        id
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
            && self.world_strokes.is_empty()
            && self.world_texts.is_empty()
            && self.link_cards.is_empty()
            && self.sticky_notes.is_empty()
            && self.group_cards.is_empty()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub summon_hotkey: String,
    pub quick_dump_hotkey: String,
    pub active_board_id: String,
    pub dark_theme: bool,
    /// Index into the canvas colour presets
    pub canvas_theme: usize,
    /// Dot grid strength, 0-100
    pub grid_dots: f32,
    pub aura: bool,
    pub round_corners: bool,
    /// One bit per canvas colour: gradient on/off
    pub gradients: u32,
    /// Gradient direction preset
    pub gradient_dir: u8,
    /// Animation strength: 0 full, 1 subtle, 2 off
    pub anim: u8,
    /// Accent colour, 0xRRGGBB
    pub accent: u32,
    /// Background pattern shape
    pub pattern: u8,
    /// Aurora colour combo
    pub aura_palette: u8,
    /// Aurora motion preset
    pub aura_motion: u8,
    /// Outline panel width in points (0 = default)
    pub outline_width: f32,
    /// Height of the properties section under the outline (0 = default)
    pub props_height: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            summon_hotkey: "CommandOrControl+Shift+Space".into(),
            quick_dump_hotkey: "CommandOrControl+Shift+D".into(),
            active_board_id: "default".into(),
            dark_theme: true,
            canvas_theme: 0,
            grid_dots: 50.0,
            aura: false,
            round_corners: false,
            gradients: 0,
            gradient_dir: 0,
            anim: 0,
            accent: 0x3b82f6,
            pattern: 0,
            aura_palette: 0,
            aura_motion: 0,
            outline_width: 0.0,
            props_height: 0.0,
        }
    }
}

pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

pub fn new_id() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}-{:x}", nanos, COUNTER.fetch_add(1, Ordering::Relaxed))
}

// ---------- storage ----------

pub fn data_dir() -> PathBuf {
    // Portable mode: a `portable.txt` next to the exe keeps all data in a `data` folder beside it,
    // so deleting that one folder removes everything
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        if exe_dir.join("portable.txt").exists() {
            let dir = exe_dir.join("data");
            let _ = fs::create_dir_all(&dir);
            return dir;
        }
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("com.tangra.app");
    // One-time move from the pre-rename folder so existing boards carry over
    let old = base.join("com.quickdump.app");
    if !dir.exists() && old.exists() {
        let _ = fs::rename(&old, &dir);
    }
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn board_dir(id: &str) -> PathBuf {
    let dir = data_dir().join("boards").join(id);
    let _ = fs::create_dir_all(dir.join("assets"));
    dir
}

pub fn asset_path(board_id: &str, file: &str) -> PathBuf {
    board_dir(board_id).join("assets").join(file)
}

pub fn load_board(id: &str) -> Board {
    let path = board_dir(id).join("board.json");
    if let Ok(content) = fs::read_to_string(&path) {
        match serde_json::from_str::<Board>(&content) {
            Ok(mut b) => {
                b.id = id.to_string();
                b.name_unnamed_images();
                // Pictures copied before annotations got their own ids shared them: split them up
                let mut seen = std::collections::HashSet::new();
                for img in &mut b.images {
                    let a = &mut img.annotations;
                    for id in a.strokes.iter_mut().map(|s| &mut s.id).chain(a.texts.iter_mut().map(|t| &mut t.id)) {
                        if !seen.insert(id.clone()) {
                            *id = new_id();
                        }
                    }
                }
                if b.camera.zoom <= 0.0 || !b.camera.zoom.is_finite() {
                    b.camera.zoom = 1.0;
                }
                // Migrate any old navy blue note colors to #303030, and ensure valid opacity
                for n in &mut b.sticky_notes {
                    if n.color.is_empty() || n.color == "#1e293b" {
                        n.color = "#303030".into();
                    }
                    if n.opacity <= 0.0 || n.opacity > 1.0 {
                        n.opacity = 1.0;
                    }
                }
                return b;
            }
            Err(_) => {
                // Never silently overwrite a board we could not read
                let _ = fs::copy(&path, board_dir(id).join("board.json.bak"));
            }
        }
    }
    let b = Board {
        id: id.to_string(),
        name: if id == "default" { "General Dump".into() } else { "Untitled Board".into() },
        created_at: now_ms(),
        updated_at: now_ms(),
        ..Default::default()
    };
    let _ = save_board(&b);
    b
}

pub fn save_board(b: &Board) -> Result<(), String> {
    let dir = board_dir(&b.id);
    let json = serde_json::to_string(b).map_err(|e| e.to_string())?;
    // Write to a temp file first so a crash can never leave a half-written board
    let tmp = dir.join("board.json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir.join("board.json")).map_err(|e| e.to_string())
}

/// Returns (id, name) of every board, sorted by name.
pub fn list_boards() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let boards = data_dir().join("boards");
    let _ = fs::create_dir_all(&boards);
    if let Ok(entries) = fs::read_dir(&boards) {
        for e in entries.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let id = e.file_name().to_string_lossy().to_string();
            let name = fs::read_to_string(e.path().join("board.json"))
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .unwrap_or_else(|| id.clone());
            out.push((id, name));
        }
    }
    if out.is_empty() {
        let b = load_board("default");
        out.push((b.id, b.name));
    }
    out.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    out
}

pub fn create_board(name: &str) -> String {
    let id = new_id();
    let b = Board {
        id: id.clone(),
        name: name.to_string(),
        created_at: now_ms(),
        updated_at: now_ms(),
        ..Default::default()
    };
    let _ = save_board(&b);
    id
}

pub fn delete_board(id: &str) {
    let dir = data_dir().join("boards").join(id);
    if dir.exists() {
        let _ = fs::remove_dir_all(dir);
    }
}

pub fn load_settings() -> Settings {
    fs::read_to_string(data_dir().join("settings.json"))
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub fn save_settings(s: &Settings) {
    if let Ok(json) = serde_json::to_string_pretty(s) {
        let _ = fs::write(data_dir().join("settings.json"), json);
    }
}
