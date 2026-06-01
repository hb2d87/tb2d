use crate::{
    app::App,
    input::{map_key, Action},
    layout::Layout,
    render,
};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io::{self, Stdout}, time::Duration};

pub fn run(app: &mut App) -> Result<()> {
    let mut terminal = TerminalGuard::new()?;
    let mut started = false;
    while !app.should_quit {
        let size = terminal.terminal.size()?;
        let layout = Layout::calculate_with_widths(
            &app.workspace,
            size.width,
            size.height.saturating_sub(1),
            &app.column_widths,
        )?;
        if !started {
            app.start_panes(&layout)?;
            started = true;
        }
        app.reveal_focus(&layout);
        app.animate_viewport(&layout);
        app.resize_panes(&layout);
        app.drain_pty_events();
        terminal.terminal.draw(|frame| render::draw(frame, app, &layout))?;

        if !event::poll(Duration::from_millis(40))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                match map_key(key) {
                    Action::Quit => app.should_quit = true,
                    Action::Move(direction) => app.move_focus(direction),
                    Action::ResizeColumn(grow) => app.resize_focused_column(&layout, grow),
                    Action::ResetColumnWidth => app.reset_focused_column_width(),
                    Action::ScrollPane(direction) => app.scroll_focused_pane(direction),
                    Action::ReorderPane(direction) => app.reorder_focused_pane(direction),
                    Action::CyclePresentation => app.cycle_focused_presentation(),
                    Action::CycleLayout => app.cycle_focused_layout(),
                    Action::Send(bytes) => app.send_input(&bytes)?,
                    Action::Ignore => {}
                }
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                app.focus_at(&layout, mouse.column, mouse.row);
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::ScrollUp => {
                app.scroll_focused_pane(crate::input::Direction::Up);
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::ScrollDown => {
                app.scroll_focused_pane(crate::input::Direction::Down);
            }
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
    Ok(())
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        let terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => terminal,
            Err(error) => {
                let mut stdout = io::stdout();
                let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
                let _ = disable_raw_mode();
                return Err(error.into());
            }
        };
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
        let _ = self.terminal.show_cursor();
    }
}
