use crate::{app::{pane_id, App, PresentationMode}, layout::Layout};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame,
};

pub fn draw(frame: &mut Frame, app: &App, layout: &Layout) {
    let ui = &app.workspace.ui;
    for column in &layout.columns {
        for (pane_index, canvas_rect) in app.pane_rects(layout, column.index)
            .into_iter()
            .enumerate()
        {
            let Some(canvas_rect) = canvas_rect else { continue };
            let Some(screen_rect) = layout.screen_rect(canvas_rect, app.viewport) else { continue };
            if screen_rect.width < 2 || screen_rect.height < 2 {
                continue;
            }
            let focused = app.focus.column == column.index && app.focus.pane == pane_index;
            let pane = &app.panes[&pane_id(column.index, pane_index)];
            let border = if focused { ui.accent.to_color() } else { ui.muted.to_color() };
            let title = pane_title(app, column.index, pane_index, pane.exited);
            let block = Block::default()
                .title(title)
                .title_bottom(pane_tabs(app, column.index, app.pane_selections[column.index]))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border).add_modifier(
                    if focused { Modifier::BOLD } else { Modifier::empty() },
                ));
            let visible_lines = screen_rect.height.saturating_sub(2);
            let clipped_left = app.viewport.offset.saturating_sub(canvas_rect.x);
            let content_start = clipped_left.saturating_sub(1).saturating_add(pane.view.horizontal);
            let content_width = screen_rect.width.saturating_sub(2);
            let paragraph = if app.is_collapsed_carousel_pane(column.index, pane_index) {
                Paragraph::new(pane_history(app, column.index)).block(block)
            } else {
                Paragraph::new(terminal_lines(
                    pane.terminal.screen(),
                    content_start,
                    content_width,
                    visible_lines,
                    focused,
                )).block(block)
            };
            let paragraph = if matches!(pane.view.presentation, PresentationMode::Words) {
                paragraph.wrap(Wrap { trim: false })
            } else {
                paragraph
            };
            frame.render_widget(paragraph, screen_rect);
        }
    }

    draw_minimap(frame, app, layout);
}

fn pane_title(app: &App, column_index: usize, pane_index: usize, exited: bool) -> Line<'static> {
    let ui = &app.workspace.ui;
    let column = &app.workspace.columns[column_index];
    let pane = &column.panes[pane_index];
    let runtime = &app.panes[&pane_id(column_index, pane_index)];
    let primary = if app.focus.column == column_index && app.focus.pane == pane_index {
        ui.accent.to_color()
    } else {
        ui.muted.to_color()
    };
    let mut spans = vec![
        Span::styled(" ", Style::default()),
        Span::styled(pane.name.clone(), Style::default().fg(primary).add_modifier(Modifier::BOLD)),
    ];
    if exited {
        spans.push(Span::styled(" [exited]", Style::default().fg(Color::Yellow)));
    }
    spans.push(Span::styled(
        format!(
            "  {}{}{} ",
            runtime.view.presentation.label(),
            if runtime.view.vertical > 0 { " up" } else { "" },
            if runtime.view.horizontal > 0 { " x" } else { "" },
        ),
        Style::default().fg(ui.muted.to_color()),
    ));
    Line::from(spans)
}

fn pane_tabs(app: &App, column_index: usize, selected: usize) -> Line<'static> {
    let ui = &app.workspace.ui;
    let spans = app.workspace.columns[column_index]
        .panes
        .iter()
        .enumerate()
        .map(|(index, pane)| {
            let active = index == selected;
            Span::styled(
                format!(" --{}:{}-- ", index + 1, pane.name),
                if active {
                    Style::default()
                        .fg(ui.status_fg.to_color())
                        .bg(ui.accent.to_color())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(ui.muted.to_color())
                },
            )
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

fn pane_history(app: &App, column_index: usize) -> Line<'static> {
    let ui = &app.workspace.ui;
    let spans = app.workspace.columns[column_index]
        .panes
        .iter()
        .enumerate()
        .flat_map(|(index, _)| {
            let active = index == app.pane_selections[column_index];
            [
                Span::styled(
                    if active { "❙" } else { "|" },
                    Style::default()
                        .fg(if active { ui.accent.to_color() } else { ui.muted.to_color() })
                        .add_modifier(if active { Modifier::BOLD } else { Modifier::empty() }),
                ),
                Span::raw(" "),
            ]
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

fn terminal_lines(
    screen: &vt100::Screen,
    start_col: u16,
    width: u16,
    height: u16,
    focused: bool,
) -> Vec<Line<'static>> {
    let cursor = focused
        .then(|| screen.cursor_position())
        .filter(|_| !screen.hide_cursor());
    (0..height)
        .map(|row| {
            let mut spans = Vec::with_capacity(width as usize);
            let mut skip_wide_continuation = false;
            for col in start_col..start_col.saturating_add(width) {
                if skip_wide_continuation {
                    skip_wide_continuation = false;
                    continue;
                }
                let Some(cell) = screen.cell(row, col) else {
                    spans.push(Span::raw(" "));
                    continue;
                };
                let mut style = cell_style(cell);
                if cursor == Some((row, col)) {
                    style = style.add_modifier(Modifier::REVERSED);
                }
                let contents = if cell.has_contents() { cell.contents().to_owned() } else { " ".to_owned() };
                spans.push(Span::styled(contents, style));
                skip_wide_continuation = cell.is_wide();
            }
            Line::from(spans)
        })
        .collect()
}

fn cell_style(cell: &vt100::Cell) -> Style {
    let mut style = Style::default()
        .fg(terminal_color(cell.fgcolor()))
        .bg(terminal_color(cell.bgcolor()));
    for (enabled, modifier) in [
        (cell.bold(), Modifier::BOLD),
        (cell.italic(), Modifier::ITALIC),
        (cell.underline(), Modifier::UNDERLINED),
        (cell.inverse(), Modifier::REVERSED),
    ] {
        if enabled {
            style = style.add_modifier(modifier);
        }
    }
    style
}

fn terminal_color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(index) => Color::Indexed(index),
        vt100::Color::Rgb(red, green, blue) => Color::Rgb(red, green, blue),
    }
}

fn draw_minimap(frame: &mut Frame, app: &App, layout: &Layout) {
    let area = frame.area();
    if area.height == 0 {
        return;
    }
    let ratio = if layout.canvas_width == 0 {
        1.0
    } else {
        layout.viewport_width.min(layout.canvas_width) as f64 / layout.canvas_width as f64
    };
    let ui = &app.workspace.ui;
    let label = format!(
        " {} | {} / {} | {} | view {}-{}/{} | Alt+m layout Alt+w wrap Ctrl-q exit ",
        app.workspace.name,
        app.workspace.columns[app.focus.column].name,
        app.workspace.columns[app.focus.column].panes[app.focus.pane].name,
        app.resize_status(layout),
        app.viewport.offset,
        app.viewport.offset.saturating_add(layout.viewport_width).min(layout.canvas_width),
        layout.canvas_width,
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(ui.status_fg.to_color()).bg(ui.status_bg.to_color()))
            .ratio(ratio)
            .label(Span::styled(label, Style::default().fg(ui.status_fg.to_color()).bg(ui.status_bg.to_color()))),
        Rect::new(0, area.height - 1, area.width, 1),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Workspace;

    #[test]
    fn terminal_lines_preserve_ansi_styles_and_wide_character_width() {
        let mut terminal = vt100::Parser::new(2, 8, 0);
        terminal.process("a\x1b[31m界b\x1b[0m".as_bytes());
        let lines = terminal_lines(terminal.screen(), 0, 8, 1, false);
        assert_eq!(lines[0].width(), 8);
        assert_eq!(lines[0].spans[1].content, "界");
        assert_eq!(lines[0].spans[1].style.fg, Some(Color::Indexed(1)));
        assert_eq!(lines[0].spans[2].content, "b");
    }

    #[test]
    fn focused_terminal_cursor_is_reversed() {
        let mut terminal = vt100::Parser::new(1, 4, 0);
        terminal.process(b"a");
        let lines = terminal_lines(terminal.screen(), 0, 4, 1, true);
        assert!(lines[0].spans[1].style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn renders_fitted_panes_without_carousel_trimming() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n      - name: b\n",
        )
        .unwrap();
        let layout = Layout::calculate(&workspace, 80, 20).unwrap();
        let rects = layout.pane_rects(0, 1, 3);
        assert_eq!(rects[0], Some(Rect::new(0, 0, 40, 10)));
        assert_eq!(rects[1], Some(Rect::new(0, 10, 40, 10)));
    }

    #[test]
    fn pane_tabs_have_compact_delimiters_and_character_indexes() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: alpha\n      - name: beta\n",
        )
        .unwrap();
        let app = App::new(workspace, Default::default()).unwrap();
        assert_eq!(pane_tabs(&app, 0, 1).to_string(), " --1:alpha--  --2:beta-- ");
    }

    #[test]
    fn collapsed_carousel_history_uses_a_thicker_active_marker() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: alpha\n      - name: beta\n      - name: gamma\n",
        )
        .unwrap();
        let mut app = App::new(workspace, Default::default()).unwrap();
        app.pane_selections[0] = 1;
        assert_eq!(pane_history(&app, 0).to_string(), "| ❙ | ");
    }
}
