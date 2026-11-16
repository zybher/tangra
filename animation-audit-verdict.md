# Animation audit: executor verdicts

The audit is in `animation-audit.md`. Every row was checked against the code. Most were confirmed by reading the cited lines; the rest are judgement calls, explained below.

- ✅ Confirmed: it really snaps, and fixing it is a small, local change with an existing helper.
- 🟡 Real but big: it really snaps, but fixing it is a feature of its own (tombstones, history diffing, a theme crossfade, list FLIP), not a micro-tweak. These are parked until you pick them.
- ❌ Not it: a false positive, a duplicate, or a snap that should stay instant.

## ❌ Not it

| # | Why |
|---|---|
| 17 | Shift+Arrow nudge should stay instant. A tween would fight held-key repeats and make precise nudging feel laggy. |
| 38 | Quick-dump reload happens while the window is hidden, so nobody sees it. |
| 89 | The maximize/restore icon follows an OS window-state change. It's invisible in practice. |
| 94 | UI scale is stepped in 0.05 on purpose. Smoothing it would re-layout every glyph each frame while resizing (blurry, slow). |
| 100 | Duplicate of #37. |
| 101 | Lasso closing on release is meant to be instant. The loop is already drawn up to that point. |
| 104 | The two blank frames happen while the window is hiding (it frees textures). They aren't visible. |
| 1 | Half-true. The outline appears at the new object; it doesn't "slide". Gliding a selection between objects reads as lag (Figma doesn't either). The right fix is a short fade-in, and that's folded into #2. |

## ✅ Confirmed (fix candidates)

| Group | Rows | Fix |
|---|---|---|
| Canvas overlays fade in/out | 2, 4, 5, 6, 7, 8, 9, 10, 11 | `animate_bool_with_time(…, 0.12·anim_k)` on alpha (guides and marquee keep their last geometry while fading out) |
| Multi-select handles | 3 | Same spring scale-in as the single-selection handles (`qd_handle_show`) |
| Instant rect writes → glide | 12, 13, 14, 15, 16 | `self.glide_to(moves)`, as align / fit frame already do |
| Props "Reset" bypasses the reset morph | 96 | Call `reset_morph_start`, as image_sliders already does (props.rs still snaps) |
| Props typed value commits with no tween | 44 | Route through `num_edit` TWEEN, like the bar's number fields |
| Selection-state colours and rings | 43, 58, 60, 61, 62, 63, 82, 83, 84, 85, 87, 98, 99 | `ease(ctx, id, on)` → `mix(off, on, k)` |
| Menu radio / checkbox | 64, 65 | `ease` the fill, tick and dot radius |
| Outline row details | 39 (chevron rotates), 46, 47, 49, 50, 53, 54, 55 | `ease` on rotation / alpha |
| Rename ring exit | 45 | Keep drawing until `it_e` has eased out |
| Drop marker | 48 | Exp-glide the marker y |
| Tooltip slides between buttons | 56, 57 | Exp-glide `tx` (style bar and place bar) |
| Knobs jump on click | 68, 69, 77 | Exp-glide the knob position |
| Picker handle hover | 70 | `hover_fx` on the handle radius |
| Rolling numbers | 74, 75, 76, 103, 78, 79 | `roll_text` (zoom %, opacity, stroke/corner values, "N selected", Animations label, submenu value) |
| Active tab slides | 81 | Exp-glide the active tab's x / width |
| Submenu exit | 66 | Keep the leaving submenu for its fade-out (the entrance already eases) |
| Note editor popup | 67 | Same fade / scale entrance as the other popups |
| Small rows popping | 51, 71, 72, 88, 102 | `animate_bool` on row height / alpha |
| Text-message swap | 73, 80 | Crossfade (low priority) |

## 🟡 Real but big (parked)

| Rows | What it would take |
|---|---|
| 18, 19, 20, 22, 23, 24, 25 | Per-object render-time tweens of colour / opacity / fill / corner / width / stroke position / font size: a "property glide" next to `canvas/glide.rs`. Doable as one module; covers all seven rows. |
| 26 | Visibility fade: hidden items must keep painting while they fade. |
| 29, 36 | Undo/redo glide: diff the two boards and glide the moved rects. Risky. |
| 30, 35, 28, 27, 97 | Exit animations for deleted / erased / cleared / cut content: needs tombstones (a fading ghost after removal). |
| 31, 32, 34 | Entrance pop for pasted / new / duplicated objects. Medium; one shared "spawn" ease. |
| 33 | Board switch crossfade. |
| 37 | Z-order lift. |
| 40, 41, 42, 52, 86 | List / layout reflow (outline rows, props sections, tab widths): FLIP-style y/x glides. Medium-big. |
| 21 | Clip toggle wipe. |
| 90, 91, 92, 95 | Theme / accent / gradient / pattern crossfade: a global colour lerp in theme.rs. Medium. |
| 93 | Grid tier crossfade on zoom. Medium. |

## Proposed batch 1 (cheap, highly visible, low risk)

96, 56/57, 64/65, 39, 43, 58/60/61/62, 2/3/4/9, 81, 83/84, 74, 68, 12/13/14-16.
