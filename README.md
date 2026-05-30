# TB2D

TB2D is a Rust terminal workspace manager with a horizontally scrollable strip
of columns and vertically stacked PTY-backed panes.

## Run

```bash
cargo run -- ./examples/web-reader.yaml --session main
```

Use `Alt+h/j/k/l` or `Alt+Arrow` to change focus, click a pane to focus it,
and press `Ctrl+q` to exit. Focus and viewport offset are saved under the
platform state directory in `tb2d/<session>.json`.

## Workspace YAML

Each column has a name, width, and one or more panes. Widths support cell
counts, the `small`, `medium`, and `big` presets, custom presets, and
percentages with optional clamps such as `"55% min=42 max=72"`.

```yaml
name: demo
gap: 2
peek: 3
columns:
  - name: editor
    width: big
    panes:
      - name: shell
        command: "${SHELL:-sh}"
```

TB2D intentionally starts with a lightweight pane output renderer rather than
a full terminal emulator. It is suitable for shell-oriented dogfooding while
terminal emulation remains an explicit future improvement.
