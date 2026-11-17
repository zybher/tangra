//! Named icon helpers for the UI, drawn from the icon pack (svg_icons.rs). `size` is the
//! old 16-grid size the callers pass; the pack's 24-unit art is scaled to match it.
use crate::Kind;
use eframe::egui::{self, Color32, Pos2};

fn svg(p: &egui::Painter, name: &'static str, c: Pos2, size: f32, col: Color32) {
    super::svg_icons::draw(p, name, c, size * 1.1, col);
}

/// Eye: almond outline + round pupil; a diagonal slash through it when `off`
pub(crate) fn eye(p: &egui::Painter, c: Pos2, size: f32, col: Color32, off: bool) {
    svg(p, if off { "eye-off" } else { "eye" }, c, size, col);
}

/// Padlock: rounded body + shackle; open (right leg lifted off the body) when `!locked`
pub(crate) fn lock(p: &egui::Painter, c: Pos2, size: f32, col: Color32, locked: bool) {
    svg(p, if locked { "lock" } else { "unlock" }, c, size, col);
}

/// The small per-kind glyph used in outline rows (and the drag ghost)
pub(crate) fn kind_icon(p: &egui::Painter, k: Kind, ic: Pos2, icol: Color32) {
    kind_icon_named(p, k, None, ic, icol)
}

/// Same, with drawings shown by the tool that made them (`Stroke::tool_kind`)
pub(crate) fn kind_icon_named(p: &egui::Painter, k: Kind, tool: Option<&'static str>, ic: Pos2, icol: Color32) {
    let name = match k {
        Kind::Stroke if tool.is_some() => tool.unwrap_or("pencil"),
        Kind::Group => "frame",
        Kind::Image => "image",
        Kind::Text => "text",
        Kind::Note => "sticky-note",
        Kind::Link => "link",
        Kind::Stroke => "pencil",
    };
    super::svg_icons::draw(p, name, ic, 14.0, icol);
}

/// Align icons: an edge (or centre) line with a long and a short bar against it.
/// `how`: 0 left, 1 centre, 2 right, 3 top, 4 middle, 5 bottom
pub(crate) fn align(p: &egui::Painter, c: Pos2, size: f32, col: Color32, how: u8) {
    let name = ["align-left", "align-center-h", "align-right", "align-top", "align-center-v", "align-bottom"][how.min(5) as usize];
    svg(p, name, c, size, col);
}

/// Distribute: two edge lines with an object between them
pub(crate) fn distribute(p: &egui::Painter, c: Pos2, size: f32, col: Color32, horizontal: bool) {
    svg(p, if horizontal { "distribute-h" } else { "distribute-v" }, c, size, col);
}

/// Opacity: a small checkerboard square
pub(crate) fn opacity(p: &egui::Painter, c: Pos2, size: f32, col: Color32) {
    svg(p, "opacity", c, size, col);
}

/// Adjust: two slider lines with knobs
pub(crate) fn adjust(p: &egui::Painter, c: Pos2, size: f32, col: Color32) {
    svg(p, "adjustments", c, size, col);
}

/// Restore cut areas: the picture's dashed edge with a turn-back arrow
pub(crate) fn restore_cuts(p: &egui::Painter, c: Pos2, size: f32, col: Color32) {
    svg(p, "restore-cuts", c, size, col);
}

/// Reset: a turn-back arrow
pub(crate) fn reset(p: &egui::Painter, c: Pos2, size: f32, col: Color32) {
    svg(p, "rotate-ccw", c, size, col);
}

/// 3D: a cube in outline
pub(crate) fn cube(p: &egui::Painter, c: Pos2, size: f32, col: Color32) {
    svg(p, "cube-3d", c, size, col);
}
