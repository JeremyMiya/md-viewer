//! Regression tests for inline-code wrapping. The renderer used to lay out a
//! long inline-code token as a single overflowing widget, which clipped or
//! overlapped surrounding text at narrow widths. See pulldown.rs
//! `inline_code_wrap_segments`.

use egui::{Context, Rect, Shape, TextStyle};
use egui_commonmark_extended::{CommonMarkCache, CommonMarkViewer};

#[derive(Debug)]
struct PaintedText {
    text: String,
    rect: Rect,
    underlined: bool,
}

fn collect_painted_text(shape: &Shape, painted: &mut Vec<PaintedText>) {
    // Text can be emitted directly or nested in a grouped Shape::Vec.
    match shape {
        Shape::Text(text) => painted.push(PaintedText {
            text: text.galley.job.text.clone(),
            rect: text.galley.rect.translate(text.pos.to_vec2()),
            underlined: text
                .galley
                .job
                .sections
                .iter()
                .any(|section| section.format.underline.width > 0.0),
        }),
        Shape::Vec(shapes) => {
            for shape in shapes {
                collect_painted_text(shape, painted);
            }
        }
        _ => {}
    }
}

fn render_geometry(markdown: &str, width: f32) -> (Rect, f32, Vec<PaintedText>) {
    render_geometry_with_hooks(markdown, width, &[])
}

fn render_geometry_with_hooks(
    markdown: &str,
    width: f32,
    hooks: &[&str],
) -> (Rect, f32, Vec<PaintedText>) {
    let ctx = Context::default();
    let mut cache = CommonMarkCache::default();
    for hook in hooks {
        cache.add_link_hook(*hook);
    }
    let mut body_rect = Rect::NOTHING;
    let mut painted = Vec::new();

    // Two passes let egui settle font/layout caches before geometry is asserted.
    for pass in 0..2 {
        ctx.begin_pass(Default::default());
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.set_width(width);
            let response = CommonMarkViewer::new()
                .default_width(Some(width as usize))
                .table_max_width(Some(width as usize))
                .show(ui, &mut cache, markdown);
            body_rect = response.response.rect;
        });
        let output = ctx.end_pass();

        // Only final-pass positions represent the settled layout.
        if pass == 1 {
            for clipped in output.shapes {
                collect_painted_text(&clipped.shape, &mut painted);
            }
        }
    }

    let body_id = TextStyle::Body.resolve(&ctx.style());
    let row_height = ctx.fonts_mut(|fonts| fonts.row_height(&body_id));
    (body_rect, row_height, painted)
}

fn render(markdown: &str, width: f32) -> (Rect, f32) {
    let (body_rect, row_height, _) = render_geometry(markdown, width);
    (body_rect, row_height)
}

fn text_rect(painted: &[PaintedText], marker: &str) -> Rect {
    painted
        .iter()
        .find(|entry| entry.text.contains(marker))
        .unwrap_or_else(|| panic!("missing painted marker {marker:?}: {painted:#?}"))
        .rect
}

fn assert_vertical_order(painted: &[PaintedText], markers: &[&str]) {
    // Strict top-to-bottom order detects same-row overlap at either block edge.
    for pair in markers.windows(2) {
        let upper = text_rect(painted, pair[0]);
        let lower = text_rect(painted, pair[1]);
        assert!(
            upper.bottom() <= lower.top(),
            "expected {:?} above {:?}, got upper={upper:?} lower={lower:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn short_inline_code_stays_on_one_row() {
    let (rect, row_height) = render("prefix `short-code` suffix", 540.0);
    assert!(
        rect.height() <= row_height * 1.5,
        "short inline code wrapped unexpectedly: rect={rect:?} row_height={row_height}"
    );
}

#[test]
fn path_like_inline_code_wraps() {
    let md = "`10-19 Infrastructure Core/10-Architecture/10-K3s-Plex-Legacy/10-Ansible-K3s-Plex/10.25-Ansible-K3s-Plex-Runbooks.md`";
    let (rect, row_height) = render(md, 540.0);
    assert!(
        rect.height() > row_height * 1.5,
        "path-like inline code did not wrap: rect={rect:?} row_height={row_height}"
    );
}

#[test]
fn unbreakable_long_inline_code_wraps() {
    let md = format!("`{}`", "A".repeat(180));
    let (rect, row_height) = render(&md, 540.0);
    assert!(
        rect.height() > row_height * 1.5,
        "unbroken long inline code did not wrap: rect={rect:?} row_height={row_height}"
    );
}

#[test]
fn long_inline_code_path_keeps_clickable_link_styling_after_wrapping() {
    let path = "github/research/github50/docs/AMIHUD_HT8D_BASELINE_ERROR_RETROSPECTIVE.md";
    let markdown = format!("Trigger: `{path}`");
    let (_, _, painted) = render_geometry_with_hooks(&markdown, 540.0, &[path]);
    let linked_text: String = painted
        .iter()
        .filter(|entry| entry.underlined)
        .map(|entry| entry.text.as_str())
        .collect();

    assert_eq!(linked_text, path, "painted text: {painted:#?}");
}

#[test]
fn long_markdown_table_text_wraps_and_expands_its_row() {
    let prose = "WRAPPED_MARKDOWN_CELL ".repeat(18);
    let markdown = format!(
        "| Key | Description |\n|---|---|\n| signal | {prose} |\n\nAFTER_MARKDOWN_TABLE"
    );
    let (_, row_height, painted) = render_geometry(&markdown, 360.0);
    let cell = text_rect(&painted, "WRAPPED_MARKDOWN_CELL");
    let after = text_rect(&painted, "AFTER_MARKDOWN_TABLE");

    assert!(cell.height() > row_height * 1.5, "cell did not wrap: {cell:?}");
    assert!(cell.bottom() <= after.top(), "wrapped row clipped/overlapped: {cell:?} {after:?}");
}

#[test]
fn long_html_table_text_wraps_and_expands_its_row() {
    let prose = "WRAPPED_HTML_CELL ".repeat(18);
    let markdown = format!(
        "<table><tr><th>Key</th><th>Description</th></tr><tr><td>signal</td><td>{prose}</td></tr></table>\n\nAFTER_HTML_TABLE"
    );
    let (_, row_height, painted) = render_geometry(&markdown, 360.0);
    let cell = text_rect(&painted, "WRAPPED_HTML_CELL");
    let after = text_rect(&painted, "AFTER_HTML_TABLE");

    assert!(cell.height() > row_height * 1.5, "cell did not wrap: {cell:?}");
    assert!(cell.bottom() <= after.top(), "wrapped row clipped/overlapped: {cell:?} {after:?}");
}

// ---------------------------------------------------------------------------
// Nested-list regression coverage (devlog/027).
//
// Pre-fix bugs:
//   1. `delayed_events_list_item` stopped at the first `TagEnd::Item`, leaking
//      outer-item events back to the outer `show()` loop when an item
//      contained a nested sub-list. The outer loop would eventually call
//      `List::start_item` with an empty stack → `unreachable!()` panic.
//   2. `show_scrollable`'s `sc.events` was parsed without the math option
//      while `show()`'s `cache.cached_events` was parsed with the math
//      option enabled at compile time. The split-point indices diverged from
//      the events Vec actually used by the viewport-skip path, so iteration
//      jumped to an unrelated event — often `Tag::Item` — and panicked the
//      same way.
//   3. Split points were added at every block-end, including ones inside
//      lists / tables / blockquotes — even with bugs 1 & 2 fixed this could
//      land iteration mid-container in the future.
//
// These tests exercise the show() and show_scrollable() paths with
// nested-list markdown. On pre-fix code each reproduced the panic.

fn nested_list_md() -> &'static str {
    "\
- outer-1 has some text
  - inner-1a
  - inner-1b
- outer-2 also has text
  - inner-2a
- outer-3 final item

Trailing paragraph with $0.02 markers and $env_var math-like content.
"
}

#[test]
fn nested_list_renders_via_show() {
    let (rect, row_height) = render(nested_list_md(), 540.0);
    assert!(
        rect.height() > row_height,
        "nested list rendered with zero height: rect={rect:?} row_height={row_height}"
    );
}

fn render_scrollable(
    markdown: &str,
    width: f32,
    height: f32,
    scroll_offset: Option<f32>,
) -> egui::Rect {
    let ctx = Context::default();
    let mut cache = CommonMarkCache::default();
    let mut inner_rect = egui::Rect::NOTHING;
    for pass in 0..3 {
        ctx.begin_pass(Default::default());
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.set_width(width);
            ui.set_height(height);
            let pending = if pass == 1 { scroll_offset } else { None };
            let out = CommonMarkViewer::new()
                .pending_scroll_offset(pending)
                .show_scrollable("scrollable_test", ui, &mut cache, markdown);
            inner_rect = out.inner_rect;
        });
        let _ = ctx.end_pass();
    }
    inner_rect
}

#[test]
fn nested_list_does_not_panic_in_show_scrollable() {
    // Three passes: bootstrap, jump via `pending_scroll_offset`, then settle.
    // Forces the viewport-clipped branch to pick a split-point landing near
    // the nested list — pre-fix this reproduced the SIGABRT seen on T470.
    let rect = render_scrollable(nested_list_md(), 540.0, 200.0, Some(80.0));
    assert!(
        rect.height() > 0.0,
        "show_scrollable produced empty content rect: {rect:?}"
    );
}

#[test]
fn deep_scroll_keeps_content_extent_and_paints_visible_text() {
    let mut markdown = (0..80)
        .map(|index| {
            format!("## Section {index}\n\nParagraph {index} with enough text to render.\n\n")
        })
        .collect::<String>();
    markdown.push_str("| Signal | Type | Result | Notes |\n|---|---|---|---|\n");
    for index in 0..80 {
        markdown.push_str(&format!(
            "| signal_{index} | generated | {index}.123 | a long table-cell note for row {index} |\n"
        ));
    }
    markdown.extend((80..400).map(|index| {
        format!("## Section {index}\n\nParagraph {index} with enough text to render.\n\n")
    }));
    let ctx = Context::default();
    let mut cache = CommonMarkCache::default();
    let mut initial_content_height = 0.0;
    let mut viewport_content_heights = Vec::new();
    let mut viewport_visible_text = Vec::new();
    let offsets = [0.05, 0.20, 0.45, 0.70, 0.90, 0.60, 0.30, 0.10];

    for pass in 0..=offsets.len() {
        ctx.begin_pass(Default::default());
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.set_width(540.0);
            ui.set_height(220.0);
            let out = CommonMarkViewer::new().show_scrollable(
                "deep_scroll_extent",
                ui,
                &mut cache,
                &markdown,
            );
            if pass == 0 {
                initial_content_height = out.content_size.y;
            } else {
                viewport_content_heights.push(out.content_size.y);
            }
            if let Some(fraction) = offsets.get(pass) {
                let mut state = out.state;
                state.offset.y = initial_content_height * fraction;
                state.store(ui.ctx(), out.id);
            }
        });
        let output = ctx.end_pass();

        if pass > 0 {
            let mut visible_text = 0;
            for clipped in output.shapes {
                let mut painted = Vec::new();
                collect_painted_text(&clipped.shape, &mut painted);
                visible_text += painted
                    .iter()
                    .filter(|text| text.rect.intersects(clipped.clip_rect))
                    .count();
            }
            viewport_visible_text.push(visible_text);
        }
    }

    for (index, content_height) in viewport_content_heights.iter().enumerate() {
        let extent_drift = (content_height - initial_content_height).abs();
        // The extent is dominated by the bootstrap's `page_size`, but a slice is
        // laid out live and its trailing block can settle a few pixels past that
        // measurement, so the total is not bit-stable. The bound stays tight
        // enough to catch a slice laying out at the wrong column or overflowing
        // its rect, which moved this by thousands of pixels.
        assert!(
            extent_drift <= 32.0,
            "viewport {index} changed document height by {extent_drift}px: initial={initial_content_height}, settled={content_height}"
        );
    }
    for (index, visible_text) in viewport_visible_text.iter().enumerate() {
        assert!(*visible_text > 0, "viewport {index} painted no visible text");
    }
}

#[test]
fn deeply_nested_list_renders() {
    let md = "\
- L1
  - L2
    - L3 first
    - L3 second
  - L2 second
- L1 second
";
    let (rect, _) = render(md, 540.0);
    assert!(rect.height() > 0.0, "deeply nested list rect was empty: {rect:?}");
    let rect2 = render_scrollable(md, 540.0, 200.0, None);
    assert!(rect2.height() > 0.0, "deeply nested via scrollable empty: {rect2:?}");
}

#[test]
fn list_code_block_uses_separate_rows() {
    let markdown = "- ISSUE44_BEFORE\n  ```sh\n  ISSUE44_CODE\n  ```\n  ISSUE44_AFTER";
    let (_, _, painted) = render_geometry(markdown, 540.0);

    // The issue #44 block and trailing text must each occupy later rows.
    assert_vertical_order(
        &painted,
        &["ISSUE44_BEFORE", "ISSUE44_CODE", "ISSUE44_AFTER"],
    );
}

#[test]
fn ordered_list_code_block_uses_separate_rows() {
    let markdown = "1. ORDERED_BEFORE\n   ```text\n   ORDERED_CODE\n   ```\n   ORDERED_AFTER";
    let (_, _, painted) = render_geometry(markdown, 540.0);

    // Ordered-list markers use the same list runtime state as bullets.
    assert_vertical_order(
        &painted,
        &["ORDERED_BEFORE", "ORDERED_CODE", "ORDERED_AFTER"],
    );
}

#[test]
fn code_only_list_item_renders_without_overlap_or_panic() {
    let markdown = "- ```text\n  CODE_ONLY_MARKER\n  ```";
    let (_, _, painted) = render_geometry(markdown, 540.0);

    // Successful rendering and a visible marker cover the no-panic contract.
    let code = text_rect(&painted, "CODE_ONLY_MARKER");
    assert!(
        code.is_positive(),
        "code-only block has no painted area: {code:?}"
    );
}

#[test]
fn multiple_code_blocks_in_one_item_keep_row_order() {
    let markdown = "- MULTI_BEFORE\n  ```text\n  MULTI_CODE_ONE\n  ```\n  MULTI_BETWEEN\n  ```text\n  MULTI_CODE_TWO\n  ```\n  MULTI_AFTER";
    let (_, _, painted) = render_geometry(markdown, 540.0);

    // Each block has independent before/after boundaries inside one item.
    assert_vertical_order(
        &painted,
        &[
            "MULTI_BEFORE",
            "MULTI_CODE_ONE",
            "MULTI_BETWEEN",
            "MULTI_CODE_TWO",
            "MULTI_AFTER",
        ],
    );
}

#[test]
fn nested_list_code_blocks_keep_order_and_deeper_indentation() {
    let markdown = "- OUTER_BEFORE\n  ```text\n  OUTER_CODE\n  ```\n  - NESTED_BEFORE\n    ```text\n    NESTED_CODE\n    ```\n    NESTED_AFTER\n- OUTER_AFTER";
    let (_, _, painted) = render_geometry(markdown, 540.0);

    // Nested list processing must remain balanced and vertically ordered.
    assert_vertical_order(
        &painted,
        &[
            "OUTER_BEFORE",
            "OUTER_CODE",
            "NESTED_BEFORE",
            "NESTED_CODE",
            "NESTED_AFTER",
            "OUTER_AFTER",
        ],
    );

    let outer_code = text_rect(&painted, "OUTER_CODE");
    let nested_code = text_rect(&painted, "NESTED_CODE");
    assert!(
        nested_code.left() > outer_code.left(),
        "nested code lost list indentation: outer={outer_code:?} nested={nested_code:?}"
    );
}

#[test]
fn top_level_code_block_layout_remains_separate() {
    let markdown = "TOP_BEFORE\n\n```text\nTOP_CODE\n```\n\nTOP_AFTER";
    let (_, _, painted) = render_geometry(markdown, 540.0);

    // This control is already green before the fix and protects the list-only gate.
    assert_vertical_order(&painted, &["TOP_BEFORE", "TOP_CODE", "TOP_AFTER"]);
}
