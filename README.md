# TB2D

TB2D is a Rust terminal workspace manager with a horizontally scrollable strip
of columns and PTY-backed panes. Each column can choose a pane layout mode:
`fit` for stacked panes or `carousel` for a focused, zellij-like vertical view.

## Install

Install the latest Linux x86_64 or Apple Silicon macOS release to
`~/.local/bin/tb2d`:

```bash
curl -fsSL https://raw.githubusercontent.com/hb2d/tb2d/master/scripts/install.sh | sh
```

The installer accepts `--version`, `--install-dir`, and `--repo` options. For
example, install a specific release into a custom directory:

```bash
curl -fsSL https://raw.githubusercontent.com/hb2d/tb2d/master/scripts/install.sh |
  sh -s -- --version v0.1.0 --install-dir "$HOME/bin"
```

For local development, build and install the same `tb2d` command with Cargo:

```bash
cargo install --path .
```

## Run

Launch the built-in four-column workspace:

```bash
tb2d
```

Start or replace a named session with a YAML workspace template:

```bash
tb2d --template ./examples/web-reader.yaml --session main
```

Later, restore that session and its remembered template with:

```bash
tb2d --session main
```

## Controls

Use `Alt+h/j/k/l` or `Alt+Arrow` to change focus, and click a pane to focus it.
The viewport eases into focus changes instead of jumping abruptly. Press
`Ctrl+q` to exit. Press `Alt+p` to open control mode, a small in-app cheat
sheet for space, layout, and session actions.

Column controls:

- `Alt+h/l` or `Alt+Left/Right` moves between columns.
- `Alt+-` and `Alt+=` resize the focused column.
- `Alt+0` returns the focused column to its configured width.
- `Alt+m` cycles `fit`, `tabs`, and `carousel` layouts for the focused column.

Pane controls:

- `Alt+j/k` or `Alt+Down/Up` moves between panes in the focused column.
- `Alt+z` zooms the focused pane to the full viewport; press it again to
  restore the layout.
- `Alt+PageUp` / `Alt+PageDown` or the mouse wheel scrolls the focused pane
  vertically.
- `Alt+Shift+h/l`, `Alt+Shift+Left/Right`, or horizontal wheel events scroll
  it horizontally.
- `Alt+w` cycles `symbols`, `words`, and `horizontal` content presentation.
- `Alt+Shift+k/j` or `Alt+Shift+Up/Down` reorders the focused pane within its column.

Control mode:

- `z` toggles pane zoom.
- `n` creates a pane after the focused pane.
- `c` creates a column after the focused column.
- `[` / `]` or `,` / `.` moves the focused pane to the previous or next column.
- `{` / `}` moves the focused column left or right.
- `j` or `+` grows the focused pane in `fit` layout.
- `k` or `-` shrinks the focused pane in `fit` layout.
- `h` and `l` resize the focused column.
- `m` cycles layout mode, and `w` cycles content presentation.
- `0` or `b` resets the focused column's space: column width, pane weights,
  and zoom.
- `s` saves the current session immediately.
- `Esc` or `p` exits control mode without applying another action.

`fit` is a vertical stack. `tabs` shows only the selected pane. `carousel`
shows the selected pane with compact neighboring previews. Pane selection is
remembered independently for each column.

## Sessions and diagnostics

When you run with `--session`, TB2D autosaves every 5 seconds and once more on
exit. The saved session remembers the template path, focus, viewport offset,
runtime workspace shape, column width overrides, selected pane per column,
runtime layout modes, fit pane weights, zoomed pane, and pane scroll positions.
Runtime workspace shape includes columns and panes created from control mode.
Passing a new `--template` starts from that YAML again and replaces the saved
runtime workspace snapshot on the next save.

Session state is written under the platform state directory as
`tb2d/<session>.json`. Runtime diagnostics are written next to it as
`tb2d/<session>.diagnostics.jsonl`. On most Linux systems, the default session
diagnostics file is `~/.local/state/tb2d/main.diagnostics.jsonl`.

Diagnostics are newline-delimited JSON records. They include session
start/stop breadcrumbs, workspace load failures, terminal event read/poll
errors, autosave failures, scroll bursts, frame event caps, and panic
backtraces. If the UI disappears without an obvious terminal error, check this
file first.

## Release archives

Release archives remain usable without the installer:

```bash
tar -xzf tb2d-vX.Y.Z-linux-x86_64.tar.gz
./tb2d-vX.Y.Z-linux-x86_64/tb2d
```

## Development checks

Before opening a PR, run the same checks used by CI:

```bash
cargo test --locked --lib
cargo build --locked --release
sh -n scripts/install.sh
python3 -m py_compile scripts/package-release.py
python3 scripts/package-release.py \
  --binary target/release/tb2d \
  --out-dir dist \
  --version ci \
  --platform linux-x86_64
```

## Workspace YAML

Each column has a name, width, optional `fit`, `tabs`, or `carousel` layout,
and one or more panes. Widths support cell counts, the `small`, `medium`, and
`big` presets, custom presets, and percentages with optional clamps such as
`"55% min=42 max=72"`.

Set `wrap_columns: true` to let an additional horizontal move at the first or
last column wrap to the opposite edge. Without it, horizontal navigation stops
at the edge.

The `ui.selection_bg` color is used for the selected pane border and selected
pane title background. The `ui.selection_fg` color is used for selected pane
title text. This keeps the focused pane visible without changing terminal
content colors inside the pane.

```yaml
name: demo
ui:
  accent: light-cyan
  muted: dark-gray
  selection_fg: black
  selection_bg: white
  status_fg: black
  status_bg: cyan
gap: 2
peek: 3
wrap_columns: true
columns:
  - name: editor
    layout: carousel
    width: big
    panes:
      - name: shell
        command: "${SHELL:-sh}"
```

TB2D uses PTYs with a `vt100` parser. It resizes pane terminals with the
workspace, renders ANSI colors and common text attributes, preserves wide
character layout, and handles common full-screen terminal applications. It is
still intentionally lighter than a complete terminal emulator: application
mouse forwarding, application cursor-key mode, and terminal reply plumbing are
future improvements.
