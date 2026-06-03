# Changelog

All notable changes to tb2d will be documented in this file.

## Unreleased

- Added Zellij-inspired live/control/resize hotkey layers, including direct
  `Alt+n` pane creation, `Alt+c` column creation, and `Alt+r` resize mode.
- Updated control mode so `h/j/k/l` moves focus while resize commands live in
  the dedicated resize mode.
- Added a README flow GIF that shows the horizontally scrollable workspace and
  viewport movement.
- Documented the project as draft-stage and developed with heavy LLM
  assistance.
- Normalized user-facing project naming to lowercase `tb2d`.

## 0.1.1 - 2026-06-03

- Updated the built-in default workspace to the intended `2r, 1r, 3rc, 2r`
  layout.
- Improved the installer so it can add the install directory to the user's
  shell profile when `tb2d` would otherwise not be on `PATH`.
- Clarified that `tb2d` works without `--session` or `--template`; both flags
  are optional.

## 0.1.0 - 2026-06-03

Initial shareable release.

- Added PTY-backed terminal panes arranged in a horizontally scrollable column
  workspace.
- Added `fit`, `tabs`, and `carousel` column layouts, pane zoom, runtime pane
  and column editing, and persistent named sessions.
- Added selected-pane borders and title highlighting, column navigation footer,
  scroll indicators, and configurable UI colors.
- Added autosave, diagnostics JSONL, panic logging, bounded scroll handling,
  and regression tests for the scroll crash path.
- Added Linux x86_64 and Apple Silicon macOS release packaging, installer
  script, CI checks, and tag-based GitHub release workflow.
