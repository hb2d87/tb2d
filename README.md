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

Use `Alt+h/j/k/l` or `Alt+Arrow` to change focus, click a pane to focus it,
and press `Ctrl+q` to exit. The viewport eases into focus changes instead of
jumping abruptly. Use `Alt+-` and `Alt+=` to resize the focused column, or
`Alt+0` to return it to its configured width. The template, focus, viewport
offset, and column width overrides are saved under the platform state
directory in `tb2d/<session>.json`.

Pane controls:

- `Alt+PageUp` / `Alt+PageDown` or the mouse wheel scroll the focused pane vertically.
- `Alt+Shift+h/l` or `Alt+Shift+Left/Right` scroll it horizontally.
- `Alt+w` cycles `symbols`, `words`, and `horizontal` content presentation.
- `Alt+Shift+k/j` or `Alt+Shift+Up/Down` reorders the focused pane within its column.
- `Alt+m` cycles `fit`, `tabs`, and `carousel` layouts for the focused column.

`fit` is a vertical stack. `tabs` shows one pane with a `1-2-3` style tab row.
`carousel` shows the selected pane with compact neighboring previews. Carousel
selection is remembered independently for each column.

## Release archives

Release archives remain usable without the installer:

```bash
tar -xzf tb2d-vX.Y.Z-linux-x86_64.tar.gz
./tb2d-vX.Y.Z-linux-x86_64/tb2d
```

## Workspace YAML

Each column has a name, width, optional `fit`, `tabs`, or `carousel` layout,
and one or more panes. Widths support cell
counts, the `small`, `medium`, and `big` presets, custom presets, and
percentages with optional clamps such as `"55% min=42 max=72"`.

```yaml
name: demo
ui:
  accent: light-cyan
  muted: dark-gray
  status_fg: black
  status_bg: cyan
gap: 2
peek: 3
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
