# Feature: Tables use the full content pane (#64)

**Status:** ✅ Complete
**Branch:** `fix/64-tables-full-width`
**Date:** 2026-08-22
**Lines Changed:** `src/main.rs`, vendored `egui_commonmark` (options + 2 table renderers)

## Summary

In reading mode (`full_width_content = false`) the whole document ui is
allocated at the prose cap (~600px), so wide tables were clipped at that
column with their right side unreachable — the reporter's "window width not
fully used, tables cut on the right". Markdown and HTML tables now escape
the reading column: they span the full content pane (horizontally scrolling
within it when still wider), while prose keeps the reading width.

## Features

- [x] `CommonMarkOptions::table_max_width` + `CommonMarkViewer::table_max_width()` builder setter
- [x] md-viewer passes the content pane's real width; tables bound to it, prose unchanged
- [x] Applies to both markdown tables and HTML tables
- [x] Verified at 1920px (A–G of an 8-col table visible vs cut-at-C before) and narrow widths

## Key Discoveries

### A child Ui cannot exceed its parent allocation — unless you force max_rect

The document renderer wraps everything in
`allocate_ui_with_layout(vec2(max_width, 0.0), …)`. Any nested ScrollArea is
clamped to that 600px parent, so raising `ScrollArea::max_width` alone does
nothing. But egui's `Ui::new_child` uses a builder-supplied `max_rect`
verbatim — **it is not intersected with the parent's rect**. Carving out a
wider scope lets the table escape:

```rust
let mut table_scope_rect = ui.cursor();
table_scope_rect.max.x = table_scope_rect.min.x + table_bound;
table_scope_rect.max.y = ui.max_rect().bottom();
ui.scope_builder(egui::UiBuilder::new().max_rect(table_scope_rect), |ui| { … })
```

### Anchor the carve-out at `ui.cursor()`, not `ui.max_rect()`

First attempt built the scope from `max_rect()` (document top): the table
repainted on top of the heading/paragraph above it while later content
flowed underneath. The cursor position is the correct top-left anchor;
extend only right/down.

### Nested-context overshoot is bounded

Inside lists/blockquotes `cursor().min.x` is indented, so
`min.x + pane_width` can overshoot the pane by the indent amount. At the
top level (the common case for wide tables) the offset is just the frame
margin. Accepted as a known trade-off; no clipping regressions observed at
900px or 1920px.

## Future Improvements

- Make overflow affordance stronger: currently egui's on-hover scrollbar +
  Shift+wheel; consider always-visible thin scrollbar for overflowing tables.
- Root fix upstream: egui could expose per-block width overrides so this
  hack (scope_builder with forced rect) becomes unnecessary.
