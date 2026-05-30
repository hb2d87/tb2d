use crate::{
    config::Workspace,
    input::Direction,
    layout::{FocusRef, Layout, ViewportState},
    pty::{PaneId, PtyEvent, PtyManager},
    session::SessionState,
};
use anyhow::Result;
use ratatui::layout::Rect;
use std::collections::HashMap;
use tracing::warn;

const SCROLLBACK_LINES: usize = 1_000;

pub struct PaneRuntime {
    pub id: PaneId,
    pub name: String,
    pub command: String,
    pub terminal: vt100::Parser,
    pub exited: bool,
}

pub struct App {
    pub workspace: Workspace,
    pub focus: FocusRef,
    pub viewport: ViewportState,
    pub panes: HashMap<PaneId, PaneRuntime>,
    pub should_quit: bool,
    ptys: PtyManager,
}

impl App {
    pub fn new(workspace: Workspace, restored: SessionState) -> Result<Self> {
        let mut panes = HashMap::new();
        let mut ptys = PtyManager::new();
        for (column_index, column) in workspace.columns.iter().enumerate() {
            for (pane_index, pane) in column.panes.iter().enumerate() {
                let id = pane_id(column_index, pane_index);
                ptys.spawn(id, &pane.command)?;
                panes.insert(id, PaneRuntime {
                    id,
                    name: pane.name.clone(),
                    command: pane.command.clone(),
                    terminal: vt100::Parser::new(24, 80, SCROLLBACK_LINES),
                    exited: false,
                });
            }
        }
        let mut app = Self {
            workspace,
            focus: restored.focus,
            viewport: restored.viewport,
            panes,
            should_quit: false,
            ptys,
        };
        app.clamp_focus();
        Ok(app)
    }

    pub fn session_state(&self) -> SessionState {
        SessionState { focus: self.focus, viewport: self.viewport }
    }

    pub fn focused_pane_id(&self) -> PaneId {
        pane_id(self.focus.column, self.focus.pane)
    }

    pub fn send_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.ptys.write(self.focused_pane_id(), bytes)
    }

    pub fn drain_pty_events(&mut self) {
        while let Some(event) = self.ptys.try_recv() {
            match event {
                PtyEvent::Output(id, bytes) => {
                    if let Some(pane) = self.panes.get_mut(&id) {
                        pane.terminal.process(&bytes);
                    }
                }
                PtyEvent::Exited(id) => {
                    if let Some(pane) = self.panes.get_mut(&id) {
                        pane.exited = true;
                    }
                }
                PtyEvent::ReadError(id, error) => warn!(pane = id, %error, "PTY reader stopped"),
            }
        }
    }

    pub fn move_focus(&mut self, direction: Direction) {
        match direction {
            Direction::Left => self.focus.column = self.focus.column.saturating_sub(1),
            Direction::Right => {
                self.focus.column = (self.focus.column + 1).min(self.workspace.columns.len() - 1);
            }
            Direction::Up => self.focus.pane = self.focus.pane.saturating_sub(1),
            Direction::Down => {
                self.focus.pane = (self.focus.pane + 1)
                    .min(self.workspace.columns[self.focus.column].panes.len() - 1);
            }
        }
        self.clamp_focus();
    }

    pub fn focus_at(&mut self, layout: &Layout, screen_x: u16, screen_y: u16) {
        let Some(column) = layout.column_at_screen_x(self.viewport, screen_x) else { return };
        let canvas_x = self.viewport.offset.saturating_add(screen_x);
        if let Some((pane, _)) = layout.columns[column]
            .panes
            .iter()
            .enumerate()
            .find(|(_, rect)| contains(**rect, canvas_x, screen_y))
        {
            self.focus = FocusRef { column, pane };
        }
    }

    pub fn reveal_focus(&mut self, layout: &Layout) {
        layout.reveal(&mut self.viewport, self.focus.column, self.workspace.peek);
    }

    pub fn resize_panes(&mut self, layout: &Layout) {
        for column in &layout.columns {
            for (pane, rect) in column.panes.iter().enumerate() {
                let id = pane_id(column.index, pane);
                let cols = rect.width.saturating_sub(2).max(1);
                let rows = rect.height.saturating_sub(2).max(1);
                if let Err(error) = self.ptys.resize(id, cols, rows) {
                    warn!(%error, "failed to resize PTY");
                }

            }
        }
    }

    fn clamp_focus(&mut self) {
        self.focus.column = self.focus.column.min(self.workspace.columns.len() - 1);
        self.focus.pane = self.focus.pane.min(self.workspace.columns[self.focus.column].panes.len() - 1);
    }
}

pub fn pane_id(column: usize, pane: usize) -> PaneId {
    (column << 16) | pane
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x.saturating_add(rect.width)
        && y >= rect.y && y < rect.y.saturating_add(rect.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_parser_applies_cursor_movement_and_ansi() {
        let mut terminal = vt100::Parser::new(4, 20, SCROLLBACK_LINES);
        terminal.process(b"plain\r\n\x1b[31mred\x1b[0m");
        terminal.process(b"\x1b[1D!");
        assert!(terminal.screen().contents().contains("plain"));
        assert!(terminal.screen().contents().contains("re!"));
    }
}
