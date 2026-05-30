use crate::{app::{pane_id, App}, layout::Layout};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

pub fn draw(frame: &mut Frame, app: &App, layout: &Layout) {
    for column in &layout.columns {
        for (pane_index, canvas_rect) in column.panes.iter().enumerate() {
            let Some(screen_rect) = layout.screen_rect(*canvas_rect, app.viewport) else { continue };
            if screen_rect.width < 2 || screen_rect.height < 2 {
                continue;
            }
            let focused = app.focus.column == column.index && app.focus.pane == pane_index;
            let pane = &app.panes[&pane_id(column.index, pane_index)];
            let border = if focused { Color::Cyan } else { Color::DarkGray };
            let title = if pane.exited {
                format!(" {} [exited] ", pane.name)
            } else {
                format!(" {} ", pane.name)
            };
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border).add_modifier(
                    if focused { Modifier::BOLD } else { Modifier::empty() },
                ));
            let visible_lines = screen_rect.height.saturating_sub(2);
            let clipped_left = app.viewport.offset.saturating_sub(canvas_rect.x);
            let content_start = clipped_left.saturating_sub(1);
            let text = pane.terminal.screen()
                .rows(content_start, screen_rect.width.saturating_sub(2))
                .take(visible_lines as usize)
                .map(Line::raw)
                .collect::<Vec<_>>();
            frame.render_widget(Paragraph::new(text).block(block), screen_rect);
        }
    }

    draw_minimap(frame, app, layout);
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
    let label = format!(
        " {}  col {}/{}  offset {}/{}  Ctrl-q exit ",
        app.workspace.name,
        app.focus.column + 1,
        app.workspace.columns.len(),
        app.viewport.offset,
        layout.max_offset(),
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Cyan))
            .ratio(ratio)
            .label(label),
        Rect::new(0, area.height - 1, area.width, 1),
    );
}
