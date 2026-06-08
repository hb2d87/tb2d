# Changelog

All notable changes to tb2d will be documented in this file.

## Unreleased

## 0.1.4 - 2026-06-08

- Removed user-facing horizontal pane scrolling. Pane output now wraps at pane
  width, while vertical pane scrolling and horizontal column navigation remain.
- Fixed keyboard handling for curses apps such as `nnn` by advertising
  `TERM=xterm-256color` inside panes and honoring application cursor-key mode.
- Added a more universal pane terminal profile with `COLORTERM=truecolor`,
  `TERM_PROGRAM=tb2d`, `TB2D=1`, and `TB2D_PANE_TERM` override support.

## 0.1.3 - 2026-06-05

- Fixed deep vertical scrolling so panes with more history than one screen can
  scroll through the full terminal buffer.
- Patched the vendored `vt100` 0.15.2 renderer to avoid panics when drawing
  deep scrollback offsets.
- Added `tb2d update` and `tb2d update --version vX.Y.Z` as friendly wrappers
  around the official installer.
- Simplified the README for first-time users by trimming maintainer-focused
  release/development instructions and adding update/uninstall guidance.

## 0.1.2 - 2026-06-04

- Added Zellij-inspired live/control/resize hotkey layers, including direct
  `Alt+n` pane creation, `Alt+c` column creation, `Alt+r` resize mode, and
  `Alt+s` session saving.
- Updated control mode so `h/j/k/l` moves focus while resize commands live in
  the dedicated resize mode.
- Added `tb2d --config` and `tb2d --config-path` for quickly creating and
  editing the user default YAML config.
- Updated the installer to seed starter YAML configs into the user config
  directory without overwriting existing edits.
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
