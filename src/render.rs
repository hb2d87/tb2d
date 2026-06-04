use crate::{
    app::{pane_id, App, AppMode, PresentationMode, MAX_HORIZONTAL_SCROLL},
    layout::Layout,
};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
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
            let border = if focused { ui.selection_bg.to_color() } else { ui.muted.to_color() };
            let title = pane_title(app, column.index, pane_index, pane.exited);
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(if focused { BorderType::Thick } else { BorderType::Plain })
                .border_style(Style::default().fg(border).add_modifier(
                    if focused { Modifier::BOLD } else { Modifier::empty() },
                ));
            let visible_lines = screen_rect.height.saturating_sub(2);
            let clipped_left = app.viewport.offset.saturating_sub(canvas_rect.x);
            let content_start = clipped_left.saturating_sub(1).saturating_add(pane.view.horizontal);
            let show_scrollbars = focused && !app.is_collapsed_carousel_pane(column.index, pane_index);
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
            if show_scrollbars {
                draw_pane_scrollbars(frame, app, pane, screen_rect, visible_lines, content_width);
            }
        }
    }

    draw_footer(frame, app, layout);
    if app.mode == AppMode::Control {
        draw_control_overlay(frame, app);
    } else if app.mode == AppMode::Resize {
        draw_resize_overlay(frame, app);
    }
}

fn pane_title(app: &App, column_index: usize, pane_index: usize, exited: bool) -> Line<'static> {
    let ui = &app.workspace.ui;
    let column = &app.workspace.columns[column_index];
    let pane = &column.panes[pane_index];
    let focused = app.focus.column == column_index && app.focus.pane == pane_index;
    let title = if column.panes.len() == 1 { &column.name } else { &pane.name };
    let title_style = if focused {
        Style::default()
            .fg(ui.selection_fg.to_color())
            .bg(ui.selection_bg.to_color())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(ui.muted.to_color()).add_modifier(Modifier::BOLD)
    };
    let mut spans = vec![Span::styled(format!(" {title} "), title_style)];
    if exited {
        spans.push(Span::styled(" [exited]", Style::default().fg(Color::Yellow)));
    }
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
                    if active { "┃" } else { "│" },
                    Style::default()
                        .fg(if active { ui.selection_bg.to_color() } else { ui.muted.to_color() })
                        .add_modifier(if active { Modifier::BOLD } else { Modifier::empty() }),
                ),
                Span::raw(" "),
            ]
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

fn draw_pane_scrollbars(
    frame: &mut Frame,
    app: &App,
    pane: &crate::app::PaneRuntime,
    area: Rect,
    visible_lines: u16,
    content_width: u16,
) {
    if area.width < 4 || area.height < 4 {
        return;
    }
    let ui = &app.workspace.ui;
    let thumb = Style::default()
        .fg(ui.accent.to_color())
        .add_modifier(Modifier::BOLD);
    let vertical_x = area.x + area.width - 1;
    let vertical_y = area.y + 1;
    let vertical_len = area.height.saturating_sub(2);
    draw_thumb(
        frame,
        vertical_x,
        vertical_y,
        0,
        1,
        vertical_len,
        "┃",
        thumb,
        pane.scrollback_max.saturating_add(visible_lines as usize),
        visible_lines as usize,
        pane.scrollback_max.saturating_sub(pane.view.vertical),
    );

    let horizontal_x = area.x + 1;
    let horizontal_y = area.y + area.height - 1;
    let horizontal_len = area.width.saturating_sub(2);
    draw_thumb(
        frame,
        horizontal_x,
        horizontal_y,
        1,
        0,
        horizontal_len,
        "━",
        thumb,
        MAX_HORIZONTAL_SCROLL as usize + content_width as usize,
        content_width as usize,
        pane.view.horizontal as usize,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_thumb(
    frame: &mut Frame,
    x: u16,
    y: u16,
    dx: u16,
    dy: u16,
    track_len: u16,
    symbol: &str,
    style: Style,
    content_len: usize,
    viewport_len: usize,
    position: usize,
) {
    if track_len == 0 || content_len <= viewport_len {
        return;
    }
    let track_len = track_len as usize;
    let max_position = content_len.saturating_sub(viewport_len).max(1);
    let thumb_len = ((track_len * viewport_len) / content_len).clamp(1, track_len);
    let max_start = track_len.saturating_sub(thumb_len);
    let start = (position.min(max_position) * max_start) / max_position;
    for offset in start..start + thumb_len {
        let cell_x = x.saturating_add(dx.saturating_mul(offset as u16));
        let cell_y = y.saturating_add(dy.saturating_mul(offset as u16));
        if let Some(cell) = frame.buffer_mut().cell_mut((cell_x, cell_y)) {
            cell.set_symbol(symbol).set_style(style);
        }
    }
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

fn draw_footer(frame: &mut Frame, app: &App, layout: &Layout) {
    let area = frame.area();
    if area.height == 0 {
        return;
    }
    let ui = &app.workspace.ui;
    if area.height >= 2 {
        frame.render_widget(
            Paragraph::new(column_navigation(app))
                .style(Style::default().fg(ui.status_fg.to_color())),
            Rect::new(0, area.height - 2, area.width, 1),
        );
    }
    let mode = match app.mode {
        AppMode::Control => "CONTROL",
        AppMode::Resize => "RESIZE",
        AppMode::Live if app.zoomed.is_some() => "ZOOM",
        AppMode::Live => "LIVE",
    };
    let hint = app
        .status_message
        .as_deref()
        .unwrap_or("Alt+p control Alt+r resize Alt+s save Alt+z zoom Ctrl-q exit");
    let info = format!(
        " {} | {} | {} / {} | {} | view {}-{}/{} | {} ",
        app.workspace.name,
        mode,
        app.workspace.columns[app.focus.column].name,
        app.workspace.columns[app.focus.column].panes[app.focus.pane].name,
        app.resize_status(layout),
        app.viewport.offset,
        app.viewport.offset.saturating_add(layout.viewport_width).min(layout.canvas_width),
        layout.canvas_width,
        hint,
    );
    frame.render_widget(
        Paragraph::new(info)
            .style(Style::default().fg(ui.status_fg.to_color())),
        Rect::new(0, area.height - 1, area.width, 1),
    );
}

fn draw_control_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(frame.area(), 82, 14);
    let ui = &app.workspace.ui;
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(Line::from(Span::styled(
            " control ",
            Style::default()
                .fg(ui.selection_fg.to_color())
                .bg(ui.selection_bg.to_color())
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(ui.selection_bg.to_color()));
    let lines = vec![
        Line::from("Navigate"),
        Line::from("  h/j/k/l or arrows focus panes and columns"),
        Line::from(""),
        Line::from("Structure"),
        Line::from("  n new pane      c new column       Shift+h/l or [ / ] move pane"),
        Line::from("  { / } move column"),
        Line::from(""),
        Line::from("Column and View"),
        Line::from("  r resize mode   z zoom pane        m layout    w presentation"),
        Line::from("  0 / b reset focused space"),
        Line::from(""),
        Line::from("Session"),
        Line::from("  s save now      Esc or p cancel"),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().fg(ui.status_fg.to_color())),
        area,
    );
}

fn draw_resize_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(frame.area(), 72, 10);
    let ui = &app.workspace.ui;
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(Line::from(Span::styled(
            " resize ",
            Style::default()
                .fg(ui.selection_fg.to_color())
                .bg(ui.selection_bg.to_color())
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(ui.selection_bg.to_color()));
    let lines = vec![
        Line::from("Pane size"),
        Line::from("  j / Down / + grow focused pane     k / Up / - shrink focused pane"),
        Line::from(""),
        Line::from("Column width"),
        Line::from("  h / Left shrink column             l / Right grow column"),
        Line::from(""),
        Line::from("  0 / b reset focused space          Esc, r, or p exit"),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .style(Style::default().fg(ui.status_fg.to_color())),
        area,
    );
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn column_navigation(app: &App) -> Line<'static> {
    let ui = &app.workspace.ui;
    let mut spans = vec![Span::raw(" ")];
    for (index, column) in app.workspace.columns.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" - ", Style::default().fg(ui.muted.to_color())));
        }
        let style = if index == app.focus.column {
            Style::default()
                .fg(ui.selection_bg.to_color())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(ui.status_fg.to_color())
        };
        spans.push(Span::styled(format!(" {} ", column.name), style));
    }
    Line::from(spans)
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
    fn pane_titles_show_only_the_pane_name_when_a_column_has_multiple_panes() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: alpha\n      - name: beta\n",
        )
        .unwrap();
        let app = App::new(workspace, Default::default()).unwrap();
        assert_eq!(pane_title(&app, 0, 1, false).to_string(), " beta ");
    }

    #[test]
    fn pane_titles_use_the_column_name_when_the_pane_is_alone() {
        let workspace = Workspace::parse(
            "columns:\n  - name: editor\n    width: 40\n    panes:\n      - name: shell\n",
        )
        .unwrap();
        let app = App::new(workspace, Default::default()).unwrap();
        assert_eq!(pane_title(&app, 0, 0, false).to_string(), " editor ");
    }

    #[test]
    fn collapsed_carousel_history_uses_a_thicker_active_marker() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: alpha\n      - name: beta\n      - name: gamma\n",
        )
        .unwrap();
        let mut app = App::new(workspace, Default::default()).unwrap();
        app.pane_selections[0] = 1;
        assert_eq!(pane_history(&app, 0).to_string(), "│ ┃ │ ");
    }

    #[test]
    fn footer_navigation_lists_columns_and_highlights_the_selected_one() {
        let workspace = Workspace::parse(
            "columns:\n  - name: editor\n    width: 40\n    panes:\n      - name: shell\n  - name: agent\n    width: 40\n    panes:\n      - name: shell\n",
        )
        .unwrap();
        let app = App::new(workspace, Default::default()).unwrap();
        assert_eq!(column_navigation(&app).to_string(), "  editor  -  agent ");
    }
}
