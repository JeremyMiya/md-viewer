# Feature: LaTeX `\(...\)` / `\[...\]` math delimiters (#60)

**Status:** ✅ Complete
**Branch:** `feat/60-latex-delimiters`
**Date:** 2026-08-22
**Lines Changed:** new vendored module `latex_delimiters.rs` (~430 lines with tests), 2 call sites in `pulldown.rs`

## Summary

md-viewer rendered only `$...$` / `$$...$$`; LaTeX/Pandoc-style `\(...\)` and
`\[...\]` stayed literal. Delimiters are now rewritten pre-parse on an
in-memory copy and every event range is mapped back to original coordinates,
so search highlighting and selection offsets stay exact.

## Features

- [x] Paired `\(...\)` → `$...$`, `\[...\]` → `$$...$$` in prose contexts
- [x] Protected: inline code, fenced + indented code blocks, raw HTML
- [x] Pairing restricted to a single paragraph/heading/table block
- [x] Backslash-parity escape handling (`\\(` stays literal)
- [x] Unmatched/mismatched delimiters stay literal (mismatched closer ignored like stray bracket)
- [x] `$...$` behavior untouched; currency filter still applies downstream
- [x] Source files never modified; CRLF preserved
- [x] 15 unit tests covering all acceptance cases; E2E screenshot verified

## Key Discoveries

### pulldown-cmark strips the backslash before you can look at it

CommonMark treats `\(` as an escaped paren, so by the time events exist the
delimiter is indistinguishable from prose punctuation — post-parse rewriting
is impossible. The reporter's prototype (raw-source scanner) was the right
shape; the missing piece was keeping byte ranges valid.

### Range remapping instead of equal-length tricks

`$` is shorter than `\(`, so naive splicing shifts every later offset.
Instead each replacement records a checkpoint `(norm_pos, delta)`; remapping
a parsed range is two binary searches. The math event for `\(x\)` maps back
to exactly the full `\(...\)` span, so slices round-trip.

### Contexts come free from a second parse

Rather than hand-scanning for code fences/HTML, parse the ORIGINAL text once
with plain options and use its event ranges: Code/Html/InlineHtml events and
CodeBlock containers become protected ranges; Paragraph/Heading/Table
containers bound pairing. Nested-context correctness falls out of the real
parser — no duplicated CommonMark logic.

### Both parse sites must transform identically

The split-points bootstrap re-parses independently (devlog 027 divergence:
cache events vs re-parsed stream must match or viewport-skip panics). Both
sites now route through `parse_events()`.

## Future Improvements

- Expose an option to disable LaTeX delimiters if a document uses them as literal text heavily.
- Upstream: a pulldown-cmark option for these delimiters would remove the rewrite layer entirely.
