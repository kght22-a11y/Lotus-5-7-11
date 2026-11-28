# CHECKPOINT — Markdown Section Anchors & Workbook Orphan Audit

**Date:** 2026-11-10  
**Previous checkpoint:** `CHECKPOINT-BIDIRECTIONAL-DOCUMENT-LINKS.md` (45 tests)  
**This checkpoint:** 51 tests (14 CLI, 35 core, 2 edge)

---

## What Changed

### `crates/lotus-core/src/doc/mod.rs`

- **`SectionHeading` struct** — `level: u8`, `title: String`, `anchor: String`, `line_index: usize`.
- **`heading_to_slug(title)`** — slugifies a heading title: lowercased, non-alphanumeric runs collapsed to `-`, leading/trailing dashes stripped.
- **`parse_heading_line(line)`** — returns `Option<SectionHeading>` for Markdown heading lines (`#`–`######`).
- **`Document::headings()`** — returns `Vec<SectionHeading>` for all headings found in `body`.
- **`Document::find_section(fragment)`** — resolves a fragment string to a `SectionHeading`; matches by slug, case-insensitive title, or bare `#`-prefixed anchor.
- **`Document::has_section(fragment)`** — boolean convenience wrapper.
- **`Document::section_content(fragment)`** — extracts raw body text from the matched heading up to (not including) the next heading of equal or higher rank.
- **`Document::render_section(fragment, sheet, workbook)`** — renders section content with live `{{cell:name}}` substitutions.
- **`render_template_str(body, sheet, workbook)`** — extracted standalone helper (previously inlined in `render_live`); drives all `{{cell:...}}` substitution.
- **`DocumentRefStatus` enum** — `Valid`, `ValidWithSection(String)`, `UnresolvedDocument`, `UnresolvedSection(String)`.
- **`BrokenDocRef`, `BrokenSectionRef`, `BrokenDocCellRef`, `DanglingFormulaRef`** — typed diagnostic structs for the orphan report.
- **`OrphanReport` struct** — aggregates all four issue categories; exposes `is_clean()` and `total_issues()`.
- **New tests:** `markdown_headings_are_parsed_and_slugified`, `markdown_section_content_and_live_rendering`.

### `crates/lotus-core/src/model/workbook.rs`

- **`Workbook::validate_document_ref(target)`** — resolves a bare document id or `id#fragment` target; returns `DocumentRefStatus`.
- **`Workbook::check_orphans()`** — comprehensive audit returning `OrphanReport` covering:
  1. Broken cell→document refs (documents that do not exist).
  2. Broken cell→section refs (documents that exist but do not contain the named section).
  3. Broken document `{{cell:...}}` templates (cells named in templates that do not exist).
  4. Unreferenced embedded documents (zero inbound cell `documentRefs` links).
  5. Dangling formula dependencies (references to empty or nonexistent cells via `Parser::parse_str`).
- **New tests:** `test_document_ref_validation`, `test_orphan_check_identifies_broken_refs`.

### `crates/lotus-core/src/lib.rs`

Re-exported: `heading_to_slug`, `BrokenDocCellRef`, `BrokenDocRef`, `BrokenSectionRef`, `DanglingFormulaRef`, `DocumentRefStatus`, `OrphanReport`, `SectionHeading`.

### `crates/lotus-core/src/tui/menu.rs`

- Added `MenuAction::OrphanCheck` variant.
- `/WO` menu item: **Worksheet → Orphans → "Audit workbook for broken document links and dangling formulas"**.
- Updated menu test to assert `/WO` → `OrphanCheck`.

### `crates/lotus-core/src/tui/editor.rs`

- Cell info panel (line 1 of the control panel) now appends `[Links: doc_id, ...]` when the cursor cell carries `document_refs`.

### `crates/lotus-cli/src/main.rs`

- **`lotus orphans <file> [sheet_id]`** — new subcommand; calls `check_orphans()` and prints a formatted report grouped by category.
- **`lotus doc <file> id#fragment`** — enhanced; renders the named section only, or lists available anchors when the fragment does not resolve.
- **`lotus links <file> <sheet_id> <cell>`** — enhanced; now prints `DocumentRefStatus` per outgoing reference.
- **`MenuAction::OrphanCheck` handler** — wired into `handle_menu_action()`; runs `check_orphans()` and shows a summary in the status line.
- **`run_demo()`** — `revenue` cell (A1) now carries `documentRefs=["briefing"]`, closing the orphan the previous demo had.
- Usage text updated with the `orphans` subcommand and `#fragment` doc syntax.
- **New tests:** `markdown_section_anchors_and_document_navigation`, `orphan_check_reports_broken_links_and_dangling_references`.

---

## Current Limits

- **Interactive cell-to-document navigation** — pressing Enter on a cell whose `document_refs` contains a document id does not yet open a document viewer pane inside the workspace TUI. Only CLI-side `lotus doc` and `lotus links` inspection is implemented.
- **Multi-section rendering** — `render_section` returns a single contiguous block; there is no way to render all sections as a concatenated result without calling `render_live` on the whole document.
- **Orphan false-positives** — the dangling-formula check walks only direct cell name references in formula ASTs; indirect references via nested formula calls are not traced.

---

## Backup

Pre-change snapshot: `checkpoint-backups/2026-11-10-pre-markdown-section-anchors/`  
(Created before this session's changes were applied.)

---

## Verification

```bash
cargo test --workspace
# test result: ok. 51 passed; 0 failed (14 CLI, 35 core, 2 edge)

cargo run -p lotus-cli -- demo
# writes demo.lotus with revenue.documentRefs = ["briefing"]

cargo run -p lotus-cli -- orphans demo.lotus
# Orphan audit: demo-01
# All clear — no broken references or dangling formulas found.

cargo run -p lotus-cli -- doc demo.lotus briefing
# renders live Executive Briefing with substituted cell values

cargo run -p lotus-cli -- doc demo.lotus "briefing#lotus-5-7-11-executive-briefing"
# renders only the top-level section content
```
