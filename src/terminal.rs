use crate::{
    app::App,
    input::{map_key, Action},
    layout::Layout,
    render,
    session::SessionStore,
};
use anyhow::Result;
use crossterm::{
    Command,
    event::{
        self, DisableMouseCapture, Event, KeyEventKind, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use serde_json::json;
use std::fmt;
use std::{
    io::{self, Stdout},
    time::{Duration, Instant},
};
use tracing::warn;

const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(5);
const MAX_EVENTS_PER_FRAME: usize = 128;

pub fn run(app: &mut App, store: &SessionStore) -> Result<()> {
    let mut terminal = TerminalGuard::new()?;
    let mut started = false;
    let mut last_autosave = Instant::now();
    let mut saved_state = app.session_state();
    while !app.should_quit {
        let size = terminal.terminal.size()?;
        let layout = Layout::calculate_with_widths(
            &app.workspace,
            size.width,
            size.height.saturating_sub(2),
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
        if last_autosave.elapsed() >= AUTOSAVE_INTERVAL {
            let state = app.session_state();
            if state != saved_state {
                match store.save(&state) {
                    Ok(()) => saved_state = state,
                    Err(error) => {
                        warn!(%error, "failed to autosave session");
                        note(store, "autosave-failed", &[("error", json!(error.to_string()))]);
                    }
                }
            }
            last_autosave = Instant::now();
        }

        match event::poll(Duration::from_millis(40)) {
            Ok(true) => handle_pending_events(app, &layout, store)?,
            Ok(false) => {}
            Err(error) => {
                warn!(%error, "failed to poll terminal event");
                note(store, "terminal-event-poll-error", &[("error", json!(error.to_string()))]);
            }
        }
    }
    Ok(())
}

fn handle_pending_events(app: &mut App, layout: &Layout, store: &SessionStore) -> Result<()> {
    let mut pending_scroll = PendingScroll::default();
    let mut events_read = 0_usize;
    let mut reached_frame_cap = false;
    for index in 0..MAX_EVENTS_PER_FRAME {
        let event = match event::read() {
            Ok(event) => event,
            Err(error) => {
                warn!(%error, "failed to read terminal event");
                note(store, "terminal-event-read-error", &[("error", json!(error.to_string()))]);
                break;
            }
        };
        events_read += 1;
        handle_event(app, layout, event, &mut pending_scroll)?;
        if app.should_quit {
            break;
        }
        match event::poll(Duration::ZERO) {
            Ok(true) if index + 1 == MAX_EVENTS_PER_FRAME => {
                reached_frame_cap = true;
                break;
            }
            Ok(true) => {}
            Ok(false) => break,
            Err(error) => {
                warn!(%error, "failed to poll terminal event");
                note(store, "terminal-event-poll-error", &[("error", json!(error.to_string()))]);
                break;
            }
        }
    }
    if reached_frame_cap {
        note(store, "event-frame-cap-hit", &[("events_read", json!(events_read))]);
    }
    pending_scroll.apply(app, store, events_read);
    Ok(())
}

fn handle_event(
    app: &mut App,
    layout: &Layout,
    event: Event,
    pending_scroll: &mut PendingScroll,
) -> Result<()> {
    match event {
        Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
            match map_key(key) {
                Action::Quit => app.should_quit = true,
                Action::Move(direction) => app.move_focus(direction),
                Action::ResizeColumn(grow) => app.resize_focused_column(layout, grow),
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
            app.focus_at(layout, mouse.column, mouse.row);
        }
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::ScrollUp => {
            pending_scroll.vertical += 1;
        }
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::ScrollDown => {
            pending_scroll.vertical -= 1;
        }
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::ScrollLeft => {
            pending_scroll.horizontal -= 1;
        }
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::ScrollRight => {
            pending_scroll.horizontal += 1;
        }
        Event::Resize(_, _) => {}
        _ => {}
    }
    Ok(())
}

#[derive(Debug, Default)]
struct PendingScroll {
    vertical: i32,
    horizontal: i32,
}

impl PendingScroll {
    fn apply(self, app: &mut App, store: &SessionStore, events_read: usize) {
        // Touchpads can emit a burst of wheel events in one frame; coalesce them
        // into one app scroll update so rendering and diagnostics stay stable.
        if self.vertical != 0 || self.horizontal != 0 {
            note(
                store,
                "scroll-burst",
                &[
                    ("vertical", json!(self.vertical)),
                    ("horizontal", json!(self.horizontal)),
                    ("events_read", json!(events_read)),
                    ("focus_column", json!(app.focus.column)),
                    ("focus_pane", json!(app.focus.pane)),
                ],
            );
        }
        if self.vertical > 0 {
            app.scroll_focused_pane_by(crate::input::Direction::Up, self.vertical as usize);
        } else if self.vertical < 0 {
            app.scroll_focused_pane_by(
                crate::input::Direction::Down,
                self.vertical.unsigned_abs() as usize,
            );
        }
        if self.horizontal > 0 {
            app.scroll_focused_pane_by(crate::input::Direction::Right, self.horizontal as usize);
        } else if self.horizontal < 0 {
            app.scroll_focused_pane_by(
                crate::input::Direction::Left,
                self.horizontal.unsigned_abs() as usize,
            );
        }
    }
}

fn note(store: &SessionStore, event: &str, fields: &[(&str, serde_json::Value)]) {
    if let Err(error) = store.append_diagnostic(event, fields) {
        warn!(%error, event, "failed to append diagnostics event");
    }
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnableBasicMouseCapture;

impl Command for EnableBasicMouseCapture {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        // Crossterm's broad mouse capture enables motion tracking too. TB2D only
        // needs click and wheel events, so use basic SGR mouse reporting.
        f.write_str(concat!(
            "\x1b[?1000h",
            "\x1b[?1006h",
        ))
    }
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableBasicMouseCapture) {
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
