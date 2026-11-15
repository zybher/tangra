<p align="center"><img src="native/assets/logo.svg" width="96" alt="Tangra logo"></p>

<h1 align="center">Tangra</h1>

<p align="center">An open-source design tool: an infinite canvas for frames, shapes, screenshots and notes.</p>

> **Work in progress.** This is an early version; expect rough edges and breaking changes.

## Features

- **Infinite canvas** with smooth pan and zoom (Shift+0 for 100%, Shift+1 to fit)
- **Frames** that nest: an object belongs to the frame that fully contains it
- **Outline panel** showing the layer tree of the frame you're working in (`L` to collapse)
- **Shapes and drawing:** rectangle, ellipse, arrow, pen, highlighter, blur, text, eraser
- **Images and screenshots:** paste or drop them in, then annotate
- **Notes and link cards**
- **Smart snapping:** alignment guides, equal-gap snapping, Alt to measure distances
- **Style bar** for fill, stroke, corner radius and more on the selected object
- **Multiple boards** as tabs, with background autosave
- **Themes, accent colours, background patterns** and adjustable animation strength
- System tray icon, a global hotkey, and single-instance behaviour (Windows)

## Install and run

Tangra currently targets **Windows**. You need [Rust](https://rustup.rs) (stable).

```bash
git clone https://github.com/zybher/tangra.git
cd tangra/native
cargo build --release
```

Then run `native/target/release/tangra.exe`, or use `cargo run --release` from `native/`.

Boards are saved in `%APPDATA%\com.tangra.app`.

## Where things live

All source is in `native/src/`. Most features live in one file; start from the table below.

| Area | File | What's in it |
| --- | --- | --- |
| **Startup & app loop** | `main.rs` | `App` struct and fields, `App::new`, the per-frame `update` loop, egui style setup, `main()`; shared enums (`Kind`, `Tool`, `Drag`, `Act`, `Prompt`), `View` (screen↔world), toolbar icons, stroke painting |
| **Data model** | `model.rs` | Board, images, notes, links, frames (`Group`), strokes, labels, `Settings` (saved to disk) |
| **Windows / OS** | `platform.rs` | Win32 calls, tray icon, global hotkeys, clipboard, image load/save (PNG/JPEG/WebP/GIF/BMP/TIFF/ICO), cursor position for drops |
| **Colours & canvas looks** | `theme.rs` | Canvas themes, accent colour (`blue()`), surface tones (`frame_bg()` for bars, `popup()` for things that open), gradients, Aurora palettes/motion, background patterns |
| **Shared animation helpers** | `widgets.rs` | `spring`, `ease`, `hover_fx`, `wave_rows`/`wave_tile` (magnify lists), `liquid_fill`, `roll_text` (rolling digits), smooth zoom text, hand cursor, active-tab shape |
| **Actions** | `app/actions.rs` | `apply`: every menu, toolbar and shortcut action (`Act`) is handled here |
| **Clipboard & drops** | `app/clipboard.rs` | Copy/paste objects and images, drag-and-drop files from Explorer, duplicate |
| **Background autosave** | `app/saver.rs` | Writes boards on a worker thread (newest copy per board), flushed before hide/quit |
| **Undo & selection state** | `app/state.rs` | Undo/redo, saving, select/deselect, label editing bookkeeping |
| **Board operations** | `app/board_ops.rs` | Delete, bring to front / send to back, text editing commit, eraser, switch board, zoom-to-fit, hide/quit |
| **Input** | `canvas/input.rs` | Keyboard shortcuts, pointer drags (move, resize, draw, marquee), input routing |
| **Drawing the board** | `canvas/paint.rs` | Background, items, annotations, hover/selection outlines, multi-select box, marquee |
| **Size numbers** | `canvas/size_pills.rs` | Width/height pills around the selection (glide, merge, rolling digits) |
| **Smart move & measuring** | `canvas/snap.rs` | Snapping to other on-screen objects' edges/centres with red guide lines (closest wins), equal-gap spacing in rows/columns, size matching while resizing, Shift axis lock, Ctrl to move freely, Alt-hover gap/edge measurement, Alt-drag duplicate distance to the original with round-number gap snapping (100/50/10) |
| **Bulk spacing handles** | `canvas/gap_handles.rs` | When a selection is an evenly spaced row/column, a handle sits in each gap; dragging one changes every gap at once, Shift-drag changes just that gap (frames bring their contents) |
| **Multi-select editing** | `canvas/multi_edit.rs` | Resize a multi-selection from its box (everything scales, frames bring contents), arrow-key nudge (Shift: 10 px) |
| **Hit-testing** | `canvas/hit_test.rs` | Item rects, what's under the pointer, items in a marquee |
| **Geometry** | `canvas/geometry.rs` | Pure math helpers: rect/segment tests, stroke hits, shape constraints |
| **Textures** | `canvas/textures.rs` | Uploading images to the GPU, pixelate/mosaic, lazy loading |
| **Title bar & tabs** | `ui/titlebar.rs` | Tabs, board switching, window buttons |
| **Bottom toolbar** | `ui/toolbar.rs` | Tool buttons with magnify, zoom/undo pill |
| **Tool option menus** | `ui/tool_menu.rs` | Per-tool menus (pen colours, shapes, sizes…); closes on click-out, Esc, or when the pointer moves well away |
| **Frame style bar** | `ui/style_bar.rs` | Floating bar above a selected frame/shape (or several frames at once): colour, corner radius, border, layout/aspect dropdowns; inside a frame it also gets an Align dropdown (incl. even spacing for 3+) and an opacity scrub, so there's one bar. Opacity shows on every frame (hovered, it fills with accent liquid up to its value); Stroke has an inside/centre/outside dropdown; Arrange only shows for frames with no frames inside |
| **Colour picker** | `ui/color_picker.rs` | Custom colour panel (saturation/brightness square, hue slider, hex field) used by the style bar's Custom colour |
| `native/src/ui/scale.rs` | Responsive UI size: scales all chrome with the window width (1:1 on the small default window, up to 1.45x when maximised on big screens) |
| **Canvas panel** | `ui/side_panel.rs` | Theme, accent, gradient, Aurora, patterns, animation strength, settings |
| **Outline panel** | `ui/outline.rs` | Layer tree of the frame you are in (or whole board), search with kind synonyms (photo, sticky, ...), collapses to a logo bar, fold, click to select, hover to highlight; pushes the bottom-left HUD aside; `L` toggles; double-click/Enter renames; duplicates get numbered |
| **Outline drag & drop** | `ui/outline_drag.rs` | Drag rows to reorder (saved per object) or drop on a frame to move the object into it |
| **Frame nesting** | `canvas/nesting.rs` | Which frame each object belongs to (stored, not guessed from position); the cursor decides which frame a dragged object joins (target frame lights up; hold Ctrl to skip); frames clip their contents at the inner edge of their stroke (strokes are world-sized, they zoom with the frame; only titles stay one screen size) (the cropped-off part of an object is not clickable); nested frames paint and hit-test after their parent |
| **Placement bar** | `ui/place_bar.rs` | Pops up when the selection is inside a frame: align left/centre/right/top/middle/bottom, opacity scrub, corner radius / fill / fit for a picture, even spacing for 3+; steps aside while the style bar is up (that bar carries align + opacity), otherwise sits above the object. A picture outside any frame gets a short opacity + corners bar. Keys: 0-9 opacity, Alt+A/D/W/S/H/V align, Shift+Enter selects the frame; closes like the style bar: the entrance played backwards (both replay the controls they had, so nothing jumps on close) |
| **Bar morph** | `ui/bar_morph.rs` | Style bar ↔ placement bar hand-off: the new bar starts where the old one was and glides instead of popping out/in |
| **Frame-first picking** | `canvas/scope_pick.rs` | A click inside a frame picks the frame; double-click steps one level in, Ctrl+click picks the deepest object, Shift adds |
| **Align & opacity** | `canvas/align.rs` | Align / distribute (row by row on grids) / fill / fit / opacity operations, Ctrl+F (or Ctrl+Alt+G) wraps the selection in a new frame (no fill, square corners; toast names the kind: "Framed 3 images", "objects" when mixed); pressing an align again on objects already lined up moves them as a block to that edge of their frame; opacity is stored per object (`board.opacity`) and a frame fades what's inside it (not its title) |
| **Glides** | `canvas/glide.rs` | Align, distribute, fill/fit and resize-to-fit ease objects into place instead of snapping |
| **Rounded clipping** | `canvas/round_clip.rs` | Images and nested frames inside a rounded clipping frame are cut to its rounded corners (convex polygon clip, textured mesh) |
| **Line icons** | `ui/icons.rs` | Eye, lock, align, distribute, fill / fit and opacity icons drawn on a 16-unit grid, pixel-snapped |
| **Hide / lock** | `canvas/visibility.rs` | Hidden objects skip painting; hidden or locked ones skip picking; a frame passes its state to everything inside |
| **App icon** | `native/build.rs`, `native/assets/icon.ico` | Embeds the icon into `tangra.exe` at build time (regenerate the .ico from `icon128.png`) |
| **Right-click menu** | `ui/menus.rs` | Context menu contents |
| **Menu building blocks** | `ui/menu_kit.rs` | Menu open/close animation, rows, radios, checks, swatches, dividers |
| **Dialogs** | `ui/dialogs.rs` | Label editing, board name prompts, hotkey recording |
| **Shortcuts overlay** | `ui/shortcuts.rs` | The shortcut list in the corner |

### Conventions

- Feature code is an `impl App { … }` block in its own file; each file starts with `use crate::*;`.
- Short-lived UI state (hover, animation, "is this open") is kept in egui temp data under keys
  starting with `qd_`, e.g. `egui::Id::new("qd_style_bar").with("pop")`. Some keys are shared
  between files (the size pills read the style bar's `scrub`/`pop` state), so search for the key
  before renaming one.
- Colours always come from `theme.rs` helpers, never hard-coded, so themes and accents apply everywhere.

## License

MIT. See [LICENSE](LICENSE).
