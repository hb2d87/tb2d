use crate::{
    config::{ColumnConfig, PaneConfig, PaneLayoutMode, WidthPolicy, Workspace},
    input::Direction,
    layout::{ColumnLayout, FocusRef, Layout, ViewportState},
    pty::{PaneId, PtyEvent, PtyManager},
    session::SessionState,
};
use anyhow::Result;
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use tracing::warn;

const SCROLLBACK_LINES: usize = 1_000;
pub const MAX_HORIZONTAL_SCROLL: u16 = 512;
const MIN_COLUMN_WIDTH: u16 = 12;
const COLUMN_RESIZE_STEP: u16 = 4;
const VERTICAL_SCROLL_STEP: usize = 3;
const HORIZONTAL_SCROLL_STEP: u16 = 4;
const PANE_WEIGHT_STEP: u16 = 1;
const MAX_PANE_WEIGHT: u16 = 24;
const TEMP_PANE_ID: PaneId = usize::MAX;

pub struct PaneRuntime {
    pub id: PaneId,
    pub name: String,
    pub command: String,
    pub terminal: vt100::Parser,
    pub exited: bool,
    pub view: PaneViewState,
    pub scrollback_max: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneViewState {
    pub vertical: usize,
    pub horizontal: u16,
    pub presentation: PresentationMode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationMode {
    #[default]
    Symbols,
    Words,
    Horizontal,
}

impl PresentationMode {
    pub fn next(self) -> Self {
        match self {
            Self::Symbols => Self::Words,
            Self::Words => Self::Horizontal,
            Self::Horizontal => Self::Symbols,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Symbols => "symbols",
            Self::Words => "words",
            Self::Horizontal => "horizontal",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AppMode {
    #[default]
    Live,
    Control,
    Resize,
}

pub struct App {
    pub workspace: Workspace,
    pub focus: FocusRef,
    pub viewport: ViewportState,
    pub viewport_target: u16,
    pub session_template: Option<PathBuf>,
    pub column_widths: Vec<Option<u16>>,
    pub pane_selections: Vec<usize>,
    pub pane_layouts: Vec<PaneLayoutMode>,
    pub pane_weights: Vec<Vec<u16>>,
    pub zoomed: Option<FocusRef>,
    pub mode: AppMode,
    pub status_message: Option<String>,
    pub panes: HashMap<PaneId, PaneRuntime>,
    pub should_quit: bool,
    restored_pane_views: Vec<Vec<PaneViewState>>,
    event_routes: HashMap<PaneId, PaneId>,
    ptys: PtyManager,
}

impl App {
    pub fn new(workspace: Workspace, restored: SessionState) -> Result<Self> {
        let viewport_target = restored.viewport.offset;
        let session_template = restored.template.clone();
        let column_widths = normalize_column_widths(&workspace, restored.column_widths);
        let pane_selections = normalize_pane_selections(&workspace, restored.pane_selections);
        let restored_pane_views = normalize_pane_views(&workspace, restored.pane_views);
        let pane_layouts = normalize_pane_layouts(&workspace, restored.pane_layouts);
        let pane_weights = normalize_pane_weights(&workspace, restored.pane_weights);
        let zoomed = normalize_zoomed(&workspace, restored.zoomed);
        let mut app = Self {
            workspace,
            focus: restored.focus,
            viewport: restored.viewport,
            viewport_target,
            session_template,
            column_widths,
            pane_selections,
            pane_layouts,
            pane_weights,
            zoomed,
            mode: AppMode::Live,
            status_message: None,
            panes: HashMap::new(),
            should_quit: false,
            restored_pane_views,
            event_routes: HashMap::new(),
            ptys: PtyManager::new(),
        };
        app.clamp_focus();
        Ok(app)
    }

    pub fn start_panes(&mut self, layout: &Layout) -> Result<()> {
        if !self.panes.is_empty() {
            return Ok(());
        }
        for (column_index, column) in self.workspace.columns.iter().enumerate() {
            let rects = self.pane_rects(layout, column_index);
            for (pane_index, pane) in column.panes.iter().enumerate() {
                let fallback = layout.columns[column_index].panes[pane_index];
                let rect = rects.get(pane_index).copied().flatten().unwrap_or(fallback);
                let cols = rect.width.saturating_sub(2).max(1);
                let rows = rect.height.saturating_sub(2).max(1);
                let id = pane_id(column_index, pane_index);
                self.ptys.spawn(id, &pane.command, cols, rows)?;
                self.event_routes.insert(id, id);
                self.panes.insert(id, PaneRuntime {
                    id,
                    name: pane.name.clone(),
                    command: pane.command.clone(),
                    terminal: vt100::Parser::new(rows, cols, SCROLLBACK_LINES),
                    exited: false,
                    view: self.restored_pane_views[column_index][pane_index],
                    scrollback_max: 0,
                });
            }
        }
        Ok(())
    }

    pub fn session_state(&self) -> SessionState {
        SessionState {
            template: self.session_template.clone(),
            workspace: Some(self.workspace.clone()),
            focus: self.focus,
            viewport: ViewportState { offset: self.viewport_target },
            column_widths: self.column_widths.clone(),
            pane_selections: self.pane_selections.clone(),
            pane_layouts: self.pane_layouts.clone(),
            pane_weights: self.pane_weights.clone(),
            zoomed: self.zoomed,
            pane_views: self
                .workspace
                .columns
                .iter()
                .enumerate()
                .map(|(column_index, column)| {
                    column
                        .panes
                        .iter()
                        .enumerate()
                        .map(|(pane_index, _)| {
                            self.panes
                                .get(&pane_id(column_index, pane_index))
                                .map(|pane| pane.view)
                                .unwrap_or(self.restored_pane_views[column_index][pane_index])
                        })
                        .collect()
                })
                .collect(),
        }
    }

    pub fn resize_status(&self, layout: &Layout) -> String {
        let width = layout.columns[self.focus.column].width;
        match self.column_widths.get(self.focus.column).copied().flatten() {
            Some(_) => format!("resize {} cells (custom)", width),
            None => format!("resize {} cells (default)", width),
        }
    }

    pub fn pane_rects(&self, layout: &Layout, column: usize) -> Vec<Option<Rect>> {
        let Some(column_config) = self.workspace.columns.get(column) else { return Vec::new() };
        if let Some(zoomed) = self.zoomed {
            let mut panes = vec![None; column_config.panes.len()];
            if zoomed.column == column && zoomed.pane < panes.len() {
                panes[zoomed.pane] = Some(Rect::new(
                    self.viewport.offset,
                    0,
                    layout.viewport_width,
                    layout.content_height,
                ));
            }
            return panes;
        }
        if self.pane_layouts[column] == PaneLayoutMode::Fit {
            return weighted_fit_pane_rects(
                &layout.columns[column],
                layout.content_height,
                &self.pane_weights[column],
            );
        }
        layout.pane_rects_with_mode(column, self.pane_selections[column], self.workspace.peek, self.pane_layouts[column])
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
                    let id = self.event_routes.get(&id).copied().unwrap_or(id);
                    if let Some(pane) = self.panes.get_mut(&id) {
                        pane.terminal.process(&bytes);
                        refresh_scrollback(pane);
                    }
                }
                PtyEvent::Exited(id) => {
                    let id = self.event_routes.get(&id).copied().unwrap_or(id);
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
            Direction::Left => {
                self.focus.column = if self.workspace.wrap_columns && self.focus.column == 0 {
                    self.workspace.columns.len() - 1
                } else {
                    self.focus.column.saturating_sub(1)
                };
                self.focus.pane = self.pane_selections[self.focus.column];
            }
            Direction::Right => {
                self.focus.column = if self.workspace.wrap_columns
                    && self.focus.column + 1 == self.workspace.columns.len()
                {
                    0
                } else {
                    (self.focus.column + 1).min(self.workspace.columns.len() - 1)
                };
                self.focus.pane = self.pane_selections[self.focus.column];
            }
            Direction::Up => {
                self.focus.pane = if self.pane_layouts[self.focus.column] == PaneLayoutMode::Carousel {
                    (self.focus.pane + self.workspace.columns[self.focus.column].panes.len() - 1)
                        % self.workspace.columns[self.focus.column].panes.len()
                } else {
                    self.focus.pane.saturating_sub(1)
                };
            }
            Direction::Down => {
                self.focus.pane = if self.pane_layouts[self.focus.column] == PaneLayoutMode::Carousel {
                    (self.focus.pane + 1) % self.workspace.columns[self.focus.column].panes.len()
                } else {
                    (self.focus.pane + 1)
                        .min(self.workspace.columns[self.focus.column].panes.len() - 1)
                };
            }
        }
        self.clamp_focus();
        self.pane_selections[self.focus.column] = self.focus.pane;
        if self.zoomed.is_some() {
            self.zoomed = Some(self.focus);
        }
    }

    pub fn focus_at(&mut self, layout: &Layout, screen_x: u16, screen_y: u16) {
        let Some(column) = layout.column_at_screen_x(self.viewport, screen_x) else { return };
        let canvas_x = self.viewport.offset.saturating_add(screen_x);
        if let Some((pane, _)) = self.pane_rects(layout, column)
            .into_iter()
            .enumerate()
            .find(|(_, rect)| rect.is_some_and(|rect| contains(rect, canvas_x, screen_y)))
        {
            self.focus = FocusRef { column, pane };
            self.pane_selections[column] = pane;
        }
    }

    pub fn reveal_focus(&mut self, layout: &Layout) {
        self.viewport_target = layout.reveal_offset(
            self.viewport_target,
            self.focus.column,
            self.workspace.peek,
        );
    }

    pub fn animate_viewport(&mut self, layout: &Layout) {
        self.viewport_target = self.viewport_target.min(layout.max_offset());
        layout.clamp_viewport(&mut self.viewport);
        self.viewport.offset = animate_towards(self.viewport.offset, self.viewport_target);
    }

    pub fn resize_focused_column(&mut self, layout: &Layout, grow: bool) {
        let current = layout.columns[self.focus.column].width;
        let resized = if grow {
            current.saturating_add(COLUMN_RESIZE_STEP)
        } else {
            current.saturating_sub(COLUMN_RESIZE_STEP).max(MIN_COLUMN_WIDTH)
        };
        self.column_widths[self.focus.column] = Some(resized);
    }

    pub fn reset_focused_column_width(&mut self) {
        self.column_widths[self.focus.column] = None;
        self.set_status("column width reset");
    }

    pub fn resize_focused_pane(&mut self, grow: bool) {
        let column = self.focus.column;
        let pane = self.focus.pane;
        if self.pane_layouts[column] != PaneLayoutMode::Fit {
            self.pane_layouts[column] = PaneLayoutMode::Fit;
        }
        let weights = &mut self.pane_weights[column];
        if weights.is_empty() || pane >= weights.len() {
            return;
        }
        if grow {
            weights[pane] = weights[pane].saturating_add(PANE_WEIGHT_STEP).min(MAX_PANE_WEIGHT);
            self.set_status("focused pane grew");
        } else if weights[pane] > 1 {
            weights[pane] = weights[pane].saturating_sub(PANE_WEIGHT_STEP).max(1);
            self.set_status("focused pane shrank");
        } else {
            for (index, weight) in weights.iter_mut().enumerate() {
                if index != pane {
                    *weight = weight.saturating_add(PANE_WEIGHT_STEP).min(MAX_PANE_WEIGHT);
                }
            }
            self.set_status("neighbor panes grew");
        }
    }

    pub fn reset_focused_space(&mut self) {
        let column = self.focus.column;
        self.column_widths[column] = None;
        self.pane_weights[column].fill(1);
        self.zoomed = None;
        self.set_status("focused column space reset");
    }

    pub fn toggle_focused_zoom(&mut self) {
        let focus = self.focus;
        self.zoomed = if self.zoomed == Some(focus) { None } else { Some(focus) };
        self.set_status(if self.zoomed.is_some() { "pane zoomed" } else { "pane zoom cleared" });
    }

    pub fn enter_control_mode(&mut self) {
        self.mode = AppMode::Control;
        self.status_message = None;
    }

    pub fn enter_resize_mode(&mut self) {
        self.mode = AppMode::Resize;
        self.status_message = None;
    }

    pub fn exit_mode(&mut self) {
        self.mode = AppMode::Live;
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
    }

    pub fn scroll_focused_pane(&mut self, direction: Direction) {
        self.scroll_focused_pane_by(direction, 1);
    }

    pub fn scroll_focused_pane_by(&mut self, direction: Direction, steps: usize) {
        if steps == 0 {
            return;
        }
        let pane = self.panes.get_mut(&self.focused_pane_id());
        let Some(pane) = pane else { return };
        let scrollback_max = terminal_scrollback_max(pane);
        pane.scrollback_max = scrollback_max;
        match direction {
            Direction::Up => {
                pane.view.vertical = pane
                    .view
                    .vertical
                    .saturating_add(VERTICAL_SCROLL_STEP.saturating_mul(steps))
                    .min(scrollback_max);
            }
            Direction::Down => {
                pane.view.vertical = pane
                    .view
                    .vertical
                    .saturating_sub(VERTICAL_SCROLL_STEP.saturating_mul(steps));
            }
            Direction::Left => {
                pane.view.presentation = PresentationMode::Horizontal;
                pane.view.horizontal = pane
                    .view
                    .horizontal
                    .saturating_sub(
                        HORIZONTAL_SCROLL_STEP.saturating_mul(steps.min(u16::MAX as usize) as u16),
                    );
            }
            Direction::Right => {
                pane.view.presentation = PresentationMode::Horizontal;
                pane.view.horizontal = pane
                    .view
                    .horizontal
                    .saturating_add(
                        HORIZONTAL_SCROLL_STEP.saturating_mul(steps.min(u16::MAX as usize) as u16),
                    )
                    .min(MAX_HORIZONTAL_SCROLL);
            }
        }
        pane.view.vertical = pane.view.vertical.min(scrollback_max);
        pane.view.vertical = set_terminal_scrollback(pane, pane.view.vertical);
    }

    pub fn cycle_focused_presentation(&mut self) {
        if let Some(pane) = self.panes.get_mut(&self.focused_pane_id()) {
            pane.view.presentation = pane.view.presentation.next();
            let label = pane.view.presentation.label();
            self.set_status(format!("presentation: {label}"));
        }
    }

    pub fn cycle_focused_layout(&mut self) {
        let layout = &mut self.pane_layouts[self.focus.column];
        *layout = match layout {
            PaneLayoutMode::Fit => PaneLayoutMode::Tabs,
            PaneLayoutMode::Tabs => PaneLayoutMode::Carousel,
            PaneLayoutMode::Carousel => PaneLayoutMode::Fit,
        };
        let label = layout.label();
        self.set_status(format!("layout: {label}"));
    }

    pub fn add_pane_after_focus(&mut self, layout: &Layout) -> Result<()> {
        let column = self.focus.column;
        let insert_at = self.focus.pane.saturating_add(1);
        let old_len = self.workspace.columns[column].panes.len();
        let pane = PaneConfig {
            name: unique_pane_name(&self.workspace.columns[column], "pane"),
            command: default_shell_command(),
        };

        self.workspace.columns[column].panes.insert(insert_at, pane);
        self.restored_pane_views[column].insert(insert_at, PaneViewState::default());
        self.pane_weights[column].insert(insert_at, 1);
        for pane_index in (insert_at..old_len).rev() {
            self.rename_runtime_pane(pane_id(column, pane_index), pane_id(column, pane_index + 1));
        }
        self.focus = FocusRef { column, pane: insert_at };
        self.pane_selections[column] = insert_at;
        if self.zoomed.is_some() {
            self.zoomed = Some(self.focus);
        }

        let updated_layout = self.updated_layout(layout)?;
        self.spawn_runtime_pane(column, insert_at, &updated_layout)?;
        self.set_status("pane added");
        Ok(())
    }

    pub fn add_column_after_focus(&mut self, layout: &Layout) -> Result<()> {
        let insert_at = self.focus.column.saturating_add(1);
        let old_len = self.workspace.columns.len();
        for column_index in (insert_at..old_len).rev() {
            for pane_index in (0..self.workspace.columns[column_index].panes.len()).rev() {
                self.rename_runtime_pane(
                    pane_id(column_index, pane_index),
                    pane_id(column_index + 1, pane_index),
                );
            }
        }

        self.workspace.columns.insert(insert_at, ColumnConfig {
            name: unique_column_name(&self.workspace, "column"),
            layout: PaneLayoutMode::Fit,
            width: WidthPolicy::Preset("medium".to_owned()),
            panes: vec![PaneConfig {
                name: "terminal".to_owned(),
                command: default_shell_command(),
            }],
        });
        self.column_widths.insert(insert_at, None);
        self.pane_selections.insert(insert_at, 0);
        self.pane_layouts.insert(insert_at, PaneLayoutMode::Fit);
        self.pane_weights.insert(insert_at, vec![1]);
        self.restored_pane_views.insert(insert_at, vec![PaneViewState::default()]);
        self.focus = FocusRef { column: insert_at, pane: 0 };
        if self.zoomed.is_some() {
            self.zoomed = Some(self.focus);
        }

        let updated_layout = self.updated_layout(layout)?;
        self.spawn_runtime_pane(insert_at, 0, &updated_layout)?;
        self.set_status("column added");
        Ok(())
    }

    pub fn move_focused_pane_to_column(&mut self, direction: Direction) {
        let target_column = match direction {
            Direction::Left => self.focus.column.checked_sub(1),
            Direction::Right => (self.focus.column + 1 < self.workspace.columns.len())
                .then_some(self.focus.column + 1),
            _ => None,
        };
        let Some(target_column) = target_column else {
            self.set_status("no column in that direction");
            return;
        };
        let source_column = self.focus.column;
        let source_pane = self.focus.pane;
        if self.workspace.columns[source_column].panes.len() <= 1 {
            self.set_status("cannot move the last pane from a column");
            return;
        }

        let target_insert = self.pane_selections[target_column]
            .saturating_add(1)
            .min(self.workspace.columns[target_column].panes.len());
        let moved_pane = self.workspace.columns[source_column].panes.remove(source_pane);
        let moved_view = self.restored_pane_views[source_column].remove(source_pane);
        let moved_weight = self.pane_weights[source_column].remove(source_pane);
        self.rename_runtime_pane(pane_id(source_column, source_pane), TEMP_PANE_ID);

        let source_old_len = self.workspace.columns[source_column].panes.len() + 1;
        for pane_index in source_pane + 1..source_old_len {
            self.rename_runtime_pane(
                pane_id(source_column, pane_index),
                pane_id(source_column, pane_index - 1),
            );
        }
        let target_old_len = self.workspace.columns[target_column].panes.len();
        for pane_index in (target_insert..target_old_len).rev() {
            self.rename_runtime_pane(
                pane_id(target_column, pane_index),
                pane_id(target_column, pane_index + 1),
            );
        }

        self.workspace.columns[target_column].panes.insert(target_insert, moved_pane);
        self.restored_pane_views[target_column].insert(target_insert, moved_view);
        self.pane_weights[target_column].insert(target_insert, moved_weight);
        self.rename_runtime_pane(TEMP_PANE_ID, pane_id(target_column, target_insert));
        self.pane_selections[source_column] = self.pane_selections[source_column]
            .min(self.workspace.columns[source_column].panes.len() - 1);
        self.focus = FocusRef { column: target_column, pane: target_insert };
        self.pane_selections[target_column] = target_insert;
        if self.zoomed.is_some() {
            self.zoomed = Some(self.focus);
        }
        self.set_status("pane moved");
    }

    pub fn move_focused_column(&mut self, direction: Direction) {
        let target = match direction {
            Direction::Left => self.focus.column.checked_sub(1),
            Direction::Right => (self.focus.column + 1 < self.workspace.columns.len())
                .then_some(self.focus.column + 1),
            _ => None,
        };
        let Some(target) = target else {
            self.set_status("no column in that direction");
            return;
        };
        let source = self.focus.column;
        let source_len = self.workspace.columns[source].panes.len();
        let target_len = self.workspace.columns[target].panes.len();

        for pane_index in 0..source_len {
            self.rename_runtime_pane(pane_id(source, pane_index), temp_pane_id(pane_index));
        }
        for pane_index in 0..target_len {
            self.rename_runtime_pane(pane_id(target, pane_index), pane_id(source, pane_index));
        }
        for pane_index in 0..source_len {
            self.rename_runtime_pane(temp_pane_id(pane_index), pane_id(target, pane_index));
        }

        self.workspace.columns.swap(source, target);
        self.column_widths.swap(source, target);
        self.pane_selections.swap(source, target);
        self.pane_layouts.swap(source, target);
        self.pane_weights.swap(source, target);
        self.restored_pane_views.swap(source, target);
        self.focus.column = target;
        self.focus.pane = self.focus.pane.min(self.workspace.columns[target].panes.len() - 1);
        self.pane_selections[target] = self.focus.pane;
        self.zoomed = self.zoomed.map(|mut zoomed| {
            if zoomed.column == source {
                zoomed.column = target;
            } else if zoomed.column == target {
                zoomed.column = source;
            }
            zoomed
        });
        self.set_status("column moved");
    }

    pub fn reorder_focused_pane(&mut self, direction: Direction) {
        let column = self.focus.column;
        let pane = self.focus.pane;
        let target = match direction {
            Direction::Up => pane.saturating_sub(1),
            Direction::Down => (pane + 1).min(self.workspace.columns[column].panes.len() - 1),
            _ => return,
        };
        if pane == target {
            return;
        }
        self.workspace.columns[column].panes.swap(pane, target);
        self.restored_pane_views[column].swap(pane, target);
        self.pane_weights[column].swap(pane, target);
        let left = pane_id(column, pane);
        let right = pane_id(column, target);
        self.ptys.swap(left, right);
        for route in self.event_routes.values_mut() {
            if *route == left {
                *route = right;
            } else if *route == right {
                *route = left;
            }
        }
        let left_pane = self.panes.remove(&left);
        let right_pane = self.panes.remove(&right);
        if let Some(mut runtime) = left_pane {
            runtime.id = right;
            self.panes.insert(right, runtime);
        }
        if let Some(mut runtime) = right_pane {
            runtime.id = left;
            self.panes.insert(left, runtime);
        }
        self.focus.pane = target;
        self.pane_selections[column] = target;
        if self.zoomed.is_some() {
            self.zoomed = Some(self.focus);
        }
    }

    fn updated_layout(&self, layout: &Layout) -> Result<Layout> {
        Layout::calculate_with_widths(
            &self.workspace,
            layout.viewport_width,
            layout.content_height,
            &self.column_widths,
        )
    }

    fn spawn_runtime_pane(&mut self, column: usize, pane: usize, layout: &Layout) -> Result<()> {
        let id = pane_id(column, pane);
        let fallback = layout.columns[column].panes[pane];
        let rect = self
            .pane_rects(layout, column)
            .get(pane)
            .copied()
            .flatten()
            .unwrap_or(fallback);
        let cols = rect.width.saturating_sub(2).max(1);
        let rows = rect.height.saturating_sub(2).max(1);
        let pane_config = &self.workspace.columns[column].panes[pane];
        self.ptys.spawn(id, &pane_config.command, cols, rows)?;
        self.event_routes.insert(id, id);
        self.panes.insert(id, PaneRuntime {
            id,
            name: pane_config.name.clone(),
            command: pane_config.command.clone(),
            terminal: vt100::Parser::new(rows, cols, SCROLLBACK_LINES),
            exited: false,
            view: PaneViewState::default(),
            scrollback_max: 0,
        });
        Ok(())
    }

    fn rename_runtime_pane(&mut self, old: PaneId, new: PaneId) {
        if old == new {
            return;
        }
        if let Some(mut runtime) = self.panes.remove(&old) {
            runtime.id = new;
            self.panes.insert(new, runtime);
        }
        self.ptys.rename(old, new);
        for routed in self.event_routes.values_mut() {
            if *routed == old {
                *routed = new;
            }
        }
    }

    pub fn resize_panes(&mut self, layout: &Layout) {
        for column in &layout.columns {
            for (pane, rect) in self.pane_rects(layout, column.index)
                .into_iter()
                .enumerate()
            {
                let Some(rect) = rect else { continue };
                if self.is_collapsed_carousel_pane(column.index, pane) {
                    continue;
                }
                let id = pane_id(column.index, pane);
                let cols = rect
                    .width
                    .saturating_sub(2)
                    .saturating_add(
                        self.panes
                            .get(&id)
                            .map(|pane| pane.view.horizontal)
                            .unwrap_or_default(),
                    )
                    .max(1);
                let rows = rect.height.saturating_sub(2).max(1);
                if let Err(error) = self.ptys.resize(id, cols, rows) {
                    warn!(%error, "failed to resize PTY");
                }
                if let Some(pane) = self.panes.get_mut(&id) {
                    if pane.terminal.screen().size() != (rows, cols) {
                        pane.terminal.set_size(rows, cols);
                        refresh_scrollback(pane);
                    }
                }
            }
        }
    }

    pub fn is_collapsed_carousel_pane(&self, column: usize, pane: usize) -> bool {
        self.pane_layouts[column] == PaneLayoutMode::Carousel
            && pane != self.pane_selections[column]
    }

    fn clamp_focus(&mut self) {
        self.focus.column = self.focus.column.min(self.workspace.columns.len() - 1);
        self.focus.pane = self.focus.pane.min(self.workspace.columns[self.focus.column].panes.len() - 1);
        if let Some(zoomed) = self.zoomed {
            if zoomed.column >= self.workspace.columns.len()
                || zoomed.pane >= self.workspace.columns[zoomed.column].panes.len()
            {
                self.zoomed = None;
            }
        }
        self.pane_selections[self.focus.column] = self.focus.pane;
    }
}

fn animate_towards(current: u16, target: u16) -> u16 {
    let distance = current.abs_diff(target);
    if distance == 0 {
        return current;
    }
    let step = distance.div_ceil(3).clamp(1, 8);
    if current < target {
        current.saturating_add(step).min(target)
    } else {
        current.saturating_sub(step).max(target)
    }
}

fn refresh_scrollback(pane: &mut PaneRuntime) {
    let vertical = pane.view.vertical;
    pane.scrollback_max = terminal_scrollback_max(pane);
    pane.view.vertical = vertical.min(pane.scrollback_max);
    pane.view.vertical = set_terminal_scrollback(pane, pane.view.vertical);
}

fn terminal_scrollback_max(pane: &mut PaneRuntime) -> usize {
    let vertical = pane.view.vertical;
    pane.terminal.set_scrollback(usize::MAX);
    let max = pane.terminal.screen().scrollback();
    pane.terminal.set_scrollback(vertical.min(max));
    max
}

fn set_terminal_scrollback(pane: &mut PaneRuntime, vertical: usize) -> usize {
    pane.terminal.set_scrollback(vertical);
    pane.terminal.screen().scrollback()
}

fn weighted_fit_pane_rects(
    column: &ColumnLayout,
    height: u16,
    weights: &[u16],
) -> Vec<Option<Rect>> {
    let count = column.panes.len();
    if count == 0 {
        return Vec::new();
    }
    let mut heights = weighted_heights(height, weights, count);
    let mut y = 0;
    heights
        .drain(..)
        .map(|pane_height| {
            let rect = Rect::new(column.x, y, column.width, pane_height);
            y = y.saturating_add(pane_height);
            Some(rect)
        })
        .collect()
}

fn weighted_heights(height: u16, weights: &[u16], count: usize) -> Vec<u16> {
    if count == 0 {
        return Vec::new();
    }
    let count_u16 = count.min(u16::MAX as usize) as u16;
    let base: u16 = if height >= count_u16.saturating_mul(3) {
        3
    } else if height >= count_u16 {
        1
    } else {
        0
    };
    let mut heights = vec![base; count];
    let mut remaining = height.saturating_sub(base.saturating_mul(count_u16));
    let normalized = normalize_weights(weights, count);
    let total_weight = normalized.iter().map(|weight| usize::from(*weight)).sum::<usize>().max(1);
    for (index, weight) in normalized.iter().enumerate() {
        let share = (usize::from(remaining) * usize::from(*weight)) / total_weight;
        heights[index] = heights[index].saturating_add(share.min(u16::MAX as usize) as u16);
    }
    let used = heights.iter().copied().sum::<u16>();
    remaining = height.saturating_sub(used);
    let mut index = 0;
    while remaining > 0 {
        heights[index] = heights[index].saturating_add(1);
        remaining -= 1;
        index = (index + 1) % count;
    }
    heights
}

fn normalize_weights(weights: &[u16], count: usize) -> Vec<u16> {
    let mut normalized = weights
        .iter()
        .copied()
        .take(count)
        .map(|weight| weight.clamp(1, MAX_PANE_WEIGHT))
        .collect::<Vec<_>>();
    normalized.resize(count, 1);
    normalized
}

pub fn pane_id(column: usize, pane: usize) -> PaneId {
    (column << 16) | pane
}

fn temp_pane_id(index: usize) -> PaneId {
    TEMP_PANE_ID.saturating_sub(index)
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x.saturating_add(rect.width)
        && y >= rect.y && y < rect.y.saturating_add(rect.height)
}

fn normalize_column_widths(workspace: &Workspace, restored: Vec<Option<u16>>) -> Vec<Option<u16>> {
    let mut widths = restored
        .into_iter()
        .take(workspace.columns.len())
        .collect::<Vec<_>>();
    widths.resize(workspace.columns.len(), None);
    for width in widths.iter_mut().flatten() {
        *width = (*width).max(MIN_COLUMN_WIDTH);
    }
    widths
}

fn normalize_pane_selections(workspace: &Workspace, restored: Vec<usize>) -> Vec<usize> {
    workspace
        .columns
        .iter()
        .enumerate()
        .map(|(column_index, column)| {
            restored
                .get(column_index)
                .copied()
                .unwrap_or_default()
                .min(column.panes.len() - 1)
        })
        .collect()
}

fn normalize_pane_views(workspace: &Workspace, restored: Vec<Vec<PaneViewState>>) -> Vec<Vec<PaneViewState>> {
    workspace
        .columns
        .iter()
        .enumerate()
        .map(|(column_index, column)| {
            (0..column.panes.len())
                .map(|pane_index| {
                    let mut view = restored
                        .get(column_index)
                        .and_then(|panes| panes.get(pane_index))
                        .copied()
                        .unwrap_or_default();
                    view.horizontal = view.horizontal.min(MAX_HORIZONTAL_SCROLL);
                    view
                })
                .collect()
        })
        .collect()
}

fn normalize_pane_weights(workspace: &Workspace, restored: Vec<Vec<u16>>) -> Vec<Vec<u16>> {
    workspace
        .columns
        .iter()
        .enumerate()
        .map(|(column_index, column)| {
            let mut weights = restored
                .get(column_index)
                .map(|weights| normalize_weights(weights, column.panes.len()))
                .unwrap_or_else(|| vec![1; column.panes.len()]);
            weights.resize(column.panes.len(), 1);
            weights
        })
        .collect()
}

fn normalize_pane_layouts(workspace: &Workspace, restored: Vec<PaneLayoutMode>) -> Vec<PaneLayoutMode> {
    workspace
        .columns
        .iter()
        .enumerate()
        .map(|(column_index, column)| {
            restored.get(column_index).copied().unwrap_or(column.layout)
        })
        .collect()
}

fn normalize_zoomed(workspace: &Workspace, zoomed: Option<FocusRef>) -> Option<FocusRef> {
    let zoomed = zoomed?;
    if zoomed.column < workspace.columns.len()
        && zoomed.pane < workspace.columns[zoomed.column].panes.len()
    {
        Some(zoomed)
    } else {
        None
    }
}

fn unique_pane_name(column: &ColumnConfig, base: &str) -> String {
    let existing = column
        .panes
        .iter()
        .map(|pane| pane.name.as_str())
        .collect::<std::collections::HashSet<_>>();
    unique_name(base, &existing)
}

fn unique_column_name(workspace: &Workspace, base: &str) -> String {
    let existing = workspace
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<std::collections::HashSet<_>>();
    unique_name(base, &existing)
}

fn unique_name(base: &str, existing: &std::collections::HashSet<&str>) -> String {
    if !existing.contains(base) {
        return base.to_owned();
    }
    for index in 2.. {
        let candidate = format!("{base} {index}");
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!("unbounded name search should always find a candidate")
}

fn default_shell_command() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "sh".to_owned())
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

    #[test]
    fn viewport_animation_is_subtle_and_interruptible() {
        assert_eq!(animate_towards(0, 30), 8);
        assert_eq!(animate_towards(8, 0), 5);
        assert_eq!(animate_towards(5, 5), 5);
    }

    #[test]
    fn normalizes_restored_column_width_overrides_to_workspace_shape_and_bounds() {
        let workspace = Workspace::parse(
            "gap: 1\ncolumns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n  - name: two\n    width: 50\n    panes:\n      - name: b\n",
        )
        .unwrap();
        let widths = normalize_column_widths(&workspace, vec![Some(4), Some(24), Some(80)]);
        assert_eq!(widths, vec![Some(MIN_COLUMN_WIDTH), Some(24)]);
    }

    #[test]
    fn resize_status_reflects_custom_and_default_widths() {
        let workspace = Workspace::parse(
            "gap: 1\ncolumns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n  - name: two\n    width: 50\n    panes:\n      - name: b\n",
        )
        .unwrap();
        let layout = Layout::calculate_with_widths(&workspace, 120, 20, &[Some(64), None]).unwrap();
        let custom = App {
            workspace: workspace.clone(),
            focus: FocusRef { column: 0, pane: 0 },
            viewport: ViewportState::default(),
            viewport_target: 0,
            session_template: None,
            column_widths: vec![Some(64), None],
            pane_selections: vec![0, 0],
            pane_layouts: vec![PaneLayoutMode::Fit, PaneLayoutMode::Fit],
            pane_weights: vec![vec![1], vec![1]],
            zoomed: None,
            mode: AppMode::Live,
            status_message: None,
            panes: HashMap::new(),
            should_quit: false,
            restored_pane_views: vec![vec![PaneViewState::default()], vec![PaneViewState::default()]],
            event_routes: HashMap::new(),
            ptys: PtyManager::new(),
        };
        let default = App {
            workspace,
            focus: FocusRef { column: 1, pane: 0 },
            viewport: ViewportState::default(),
            viewport_target: 0,
            session_template: None,
            column_widths: vec![None, None],
            pane_selections: vec![0, 0],
            pane_layouts: vec![PaneLayoutMode::Fit, PaneLayoutMode::Fit],
            pane_weights: vec![vec![1], vec![1]],
            zoomed: None,
            mode: AppMode::Live,
            status_message: None,
            panes: HashMap::new(),
            should_quit: false,
            restored_pane_views: vec![vec![PaneViewState::default()], vec![PaneViewState::default()]],
            event_routes: HashMap::new(),
            ptys: PtyManager::new(),
        };
        assert!(custom.resize_status(&layout).contains("custom"));
        assert!(default.resize_status(&layout).contains("default"));
    }

    #[test]
    fn start_panes_uses_the_first_layout_to_set_the_parser_size() {
        let workspace = Workspace::parse(
            "gap: 1\ncolumns:\n  - name: one\n    width: 40\n    panes:\n      - name: shell\n        command: sleep 1\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        let layout = Layout::calculate(&app.workspace, 120, 20).unwrap();
        app.start_panes(&layout).unwrap();
        let pane = app.panes.get(&pane_id(0, 0)).unwrap();
        assert_eq!(pane.terminal.screen().size(), (18, 38));
    }

    #[test]
    fn moving_between_columns_restores_each_columns_selected_row() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n      - name: b\n  - name: two\n    width: 40\n    panes:\n      - name: c\n      - name: d\n      - name: e\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        app.move_focus(Direction::Down);
        app.move_focus(Direction::Right);
        app.move_focus(Direction::Down);
        assert_eq!(app.focus, FocusRef { column: 1, pane: 1 });
        app.move_focus(Direction::Left);
        assert_eq!(app.focus, FocusRef { column: 0, pane: 1 });
        app.move_focus(Direction::Right);
        assert_eq!(app.focus, FocusRef { column: 1, pane: 1 });
    }

    #[test]
    fn optionally_wraps_horizontal_navigation_only_after_reaching_an_edge() {
        let workspace = Workspace::parse(
            "wrap_columns: true\ncolumns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n  - name: two\n    width: 40\n    panes:\n      - name: b\n  - name: three\n    width: 40\n    panes:\n      - name: c\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        app.move_focus(Direction::Right);
        assert_eq!(app.focus.column, 1);
        app.move_focus(Direction::Right);
        assert_eq!(app.focus.column, 2);
        app.move_focus(Direction::Right);
        assert_eq!(app.focus.column, 0);
        app.move_focus(Direction::Left);
        assert_eq!(app.focus.column, 2);
    }

    #[test]
    fn horizontal_navigation_stops_at_edges_when_wrapping_is_disabled() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n  - name: two\n    width: 40\n    panes:\n      - name: b\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        app.move_focus(Direction::Left);
        assert_eq!(app.focus.column, 0);
        app.move_focus(Direction::Right);
        app.move_focus(Direction::Right);
        assert_eq!(app.focus.column, 1);
    }

    #[test]
    fn carousel_navigation_wraps_between_the_first_and_last_panes() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    layout: carousel\n    width: 40\n    panes:\n      - name: a\n      - name: b\n      - name: c\n",
        )
        .unwrap();
        let mut app = App::new(workspace, Default::default()).unwrap();
        app.move_focus(Direction::Up);
        assert_eq!(app.focus.pane, 2);
        app.move_focus(Direction::Down);
        assert_eq!(app.focus.pane, 0);
    }

    #[test]
    fn cycles_runtime_layout_without_mutating_workspace_default() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        app.cycle_focused_layout();
        assert_eq!(app.pane_layouts[0], PaneLayoutMode::Tabs);
        assert_eq!(app.workspace.columns[0].layout, PaneLayoutMode::Fit);
        app.cycle_focused_layout();
        assert_eq!(app.pane_layouts[0], PaneLayoutMode::Carousel);
    }

    #[test]
    fn resizing_a_fit_pane_changes_its_rendered_space() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n      - name: b\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        let layout = Layout::calculate(&app.workspace, 80, 20).unwrap();
        assert_eq!(
            app.pane_rects(&layout, 0),
            vec![Some(Rect::new(0, 0, 40, 10)), Some(Rect::new(0, 10, 40, 10))]
        );

        app.move_focus(Direction::Down);
        app.resize_focused_pane(true);

        assert_eq!(app.pane_weights[0], vec![1, 2]);
        assert_eq!(
            app.pane_rects(&layout, 0),
            vec![Some(Rect::new(0, 0, 40, 8)), Some(Rect::new(0, 8, 40, 12))]
        );
    }

    #[test]
    fn zoomed_pane_takes_the_viewport_and_hides_other_panes() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n      - name: b\n  - name: two\n    width: 40\n    panes:\n      - name: c\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        let layout = Layout::calculate(&app.workspace, 80, 20).unwrap();

        app.toggle_focused_zoom();

        assert_eq!(
            app.pane_rects(&layout, 0),
            vec![Some(Rect::new(0, 0, 80, 20)), None]
        );
        assert_eq!(app.pane_rects(&layout, 1), vec![None]);
        assert_eq!(app.session_state().zoomed, Some(FocusRef { column: 0, pane: 0 }));
    }

    #[test]
    fn zoomed_pane_follows_keyboard_focus() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n      - name: b\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        app.toggle_focused_zoom();
        app.move_focus(Direction::Down);
        assert_eq!(app.focus, FocusRef { column: 0, pane: 1 });
        assert_eq!(app.zoomed, Some(FocusRef { column: 0, pane: 1 }));
    }

    #[test]
    fn adds_a_new_pane_after_the_focused_pane_and_persists_it() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n        command: sleep 1\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        let layout = Layout::calculate(&app.workspace, 80, 20).unwrap();

        app.add_pane_after_focus(&layout).unwrap();

        assert_eq!(app.focus, FocusRef { column: 0, pane: 1 });
        assert_eq!(app.workspace.columns[0].panes.len(), 2);
        assert_eq!(app.workspace.columns[0].panes[1].name, "pane");
        assert!(app.panes.contains_key(&pane_id(0, 1)));
        assert_eq!(
            app.session_state().workspace.unwrap().columns[0].panes.len(),
            2
        );
    }

    #[test]
    fn adds_a_new_column_after_the_focused_column_and_persists_it() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n        command: sleep 1\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        let layout = Layout::calculate(&app.workspace, 80, 20).unwrap();

        app.add_column_after_focus(&layout).unwrap();

        assert_eq!(app.focus, FocusRef { column: 1, pane: 0 });
        assert_eq!(app.workspace.columns.len(), 2);
        assert_eq!(app.workspace.columns[1].name, "column");
        assert_eq!(app.workspace.columns[1].panes[0].name, "terminal");
        assert!(app.panes.contains_key(&pane_id(1, 0)));
        assert_eq!(app.session_state().workspace.unwrap().columns.len(), 2);
    }

    #[test]
    fn moves_focused_pane_between_columns_without_emptying_the_source_column() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n      - name: b\n  - name: two\n    width: 40\n    panes:\n      - name: c\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        app.move_focus(Direction::Down);

        app.move_focused_pane_to_column(Direction::Right);

        assert_eq!(app.focus, FocusRef { column: 1, pane: 1 });
        assert_eq!(
            app.workspace.columns[0].panes.iter().map(|pane| pane.name.as_str()).collect::<Vec<_>>(),
            vec!["a"]
        );
        assert_eq!(
            app.workspace.columns[1].panes.iter().map(|pane| pane.name.as_str()).collect::<Vec<_>>(),
            vec!["c", "b"]
        );
        assert_eq!(app.pane_weights[0].len(), 1);
        assert_eq!(app.pane_weights[1].len(), 2);
    }

    #[test]
    fn moves_focused_column_with_its_runtime_space_state() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n  - name: two\n    width: 40\n    panes:\n      - name: b\n      - name: c\n",
        )
        .unwrap();
        let restored = SessionState {
            column_widths: vec![Some(44), Some(55)],
            pane_selections: vec![0, 1],
            pane_weights: vec![vec![1], vec![2, 3]],
            ..SessionState::default()
        };
        let mut app = App::new(workspace, restored).unwrap();
        app.move_focus(Direction::Right);

        app.move_focused_column(Direction::Left);

        assert_eq!(app.focus, FocusRef { column: 0, pane: 1 });
        assert_eq!(
            app.workspace.columns.iter().map(|column| column.name.as_str()).collect::<Vec<_>>(),
            vec!["two", "one"]
        );
        assert_eq!(app.column_widths, vec![Some(55), Some(44)]);
        assert_eq!(app.pane_selections, vec![1, 0]);
        assert_eq!(app.pane_weights, vec![vec![2, 3], vec![1]]);
    }

    #[test]
    fn reorders_panes_and_keeps_focus_on_the_moved_runtime() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n      - name: b\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        let mut terminal = vt100::Parser::new(2, 8, SCROLLBACK_LINES);
        terminal.process(b"buffered");
        app.panes.insert(pane_id(0, 0), PaneRuntime {
            id: pane_id(0, 0),
            name: "a".to_owned(),
            command: "sh".to_owned(),
            terminal,
            exited: false,
            view: PaneViewState::default(),
            scrollback_max: 0,
        });
        app.event_routes.insert(pane_id(0, 0), pane_id(0, 0));
        app.reorder_focused_pane(Direction::Down);
        assert_eq!(app.focus.pane, 1);
        assert_eq!(app.workspace.columns[0].panes[1].name, "a");
        assert!(app.panes[&pane_id(0, 1)].terminal.screen().contents().contains("buffered"));
        assert_eq!(app.event_routes[&pane_id(0, 0)], pane_id(0, 1));
    }

    #[test]
    fn pane_scroll_state_is_local_and_clamped_by_the_terminal_buffer() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        let mut terminal = vt100::Parser::new(2, 8, SCROLLBACK_LINES);
        terminal.process(b"one\r\ntwo\r\nthree\r\nfour");
        terminal.set_scrollback(usize::MAX);
        let scrollback_max = terminal.screen().scrollback();
        terminal.set_scrollback(0);
        app.panes.insert(pane_id(0, 0), PaneRuntime {
            id: pane_id(0, 0),
            name: "a".to_owned(),
            command: "sh".to_owned(),
            terminal,
            exited: false,
            view: PaneViewState::default(),
            scrollback_max,
        });
        app.scroll_focused_pane(Direction::Up);
        assert_eq!(app.panes[&pane_id(0, 0)].view.vertical, 2);
        app.scroll_focused_pane(Direction::Right);
        assert_eq!(app.panes[&pane_id(0, 0)].view.horizontal, 4);
        assert_eq!(app.panes[&pane_id(0, 0)].view.presentation, PresentationMode::Horizontal);
        app.scroll_focused_pane(Direction::Down);
        assert_eq!(app.panes[&pane_id(0, 0)].view.vertical, 0);
    }

    #[test]
    fn repeated_vertical_wheel_scroll_stays_clamped_to_scrollback() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        let mut terminal = vt100::Parser::new(2, 8, SCROLLBACK_LINES);
        terminal.process(b"one\r\ntwo\r\nthree\r\nfour");
        terminal.set_scrollback(usize::MAX);
        let scrollback_max = terminal.screen().scrollback();
        terminal.set_scrollback(0);
        app.panes.insert(pane_id(0, 0), PaneRuntime {
            id: pane_id(0, 0),
            name: "a".to_owned(),
            command: "sh".to_owned(),
            terminal,
            exited: false,
            view: PaneViewState::default(),
            scrollback_max,
        });

        for _ in 0..1_000 {
            app.scroll_focused_pane(Direction::Up);
        }

        let pane = &app.panes[&pane_id(0, 0)];
        assert_eq!(pane.view.vertical, pane.scrollback_max);
        assert_eq!(pane.terminal.screen().scrollback(), pane.scrollback_max);
    }

    #[test]
    fn refreshed_scrollback_keeps_full_terminal_history_reachable() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        let mut terminal = vt100::Parser::new(2, 8, SCROLLBACK_LINES);
        terminal.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix\r\nseven");
        terminal.set_scrollback(usize::MAX);
        let full_scrollback = terminal.screen().scrollback();
        assert!(full_scrollback > usize::from(terminal.screen().size().0));
        terminal.set_scrollback(0);
        let mut pane = PaneRuntime {
            id: pane_id(0, 0),
            name: "a".to_owned(),
            command: "sh".to_owned(),
            terminal,
            exited: false,
            view: PaneViewState::default(),
            scrollback_max: 0,
        };
        refresh_scrollback(&mut pane);
        assert_eq!(pane.scrollback_max, full_scrollback);
        app.panes.insert(pane_id(0, 0), pane);

        for _ in 0..1_000 {
            app.scroll_focused_pane(Direction::Up);
        }

        let pane = &app.panes[&pane_id(0, 0)];
        assert_eq!(pane.view.vertical, full_scrollback);
        assert_eq!(pane.terminal.screen().scrollback(), full_scrollback);
    }

    #[test]
    fn vertical_scroll_keeps_vt100_cell_rendering_in_safe_bounds() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        let mut terminal = vt100::Parser::new(2, 8, SCROLLBACK_LINES);
        terminal.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix\r\nseven");
        terminal.set_scrollback(usize::MAX);
        let full_scrollback = terminal.screen().scrollback();
        assert!(full_scrollback > usize::from(terminal.screen().size().0));
        terminal.set_scrollback(0);
        app.panes.insert(pane_id(0, 0), PaneRuntime {
            id: pane_id(0, 0),
            name: "a".to_owned(),
            command: "sh".to_owned(),
            terminal,
            exited: false,
            view: PaneViewState::default(),
            scrollback_max: full_scrollback,
        });

        for _ in 0..1_000 {
            app.scroll_focused_pane(Direction::Up);
        }

        let pane = &app.panes[&pane_id(0, 0)];
        assert_eq!(pane.view.vertical, full_scrollback);
        assert_eq!(pane.terminal.screen().scrollback(), pane.view.vertical);
        let _ = pane.terminal.screen().cell(0, 0);
    }

    #[test]
    fn horizontal_scroll_is_bounded_to_keep_parser_resize_stable() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        app.panes.insert(pane_id(0, 0), PaneRuntime {
            id: pane_id(0, 0),
            name: "a".to_owned(),
            command: "sh".to_owned(),
            terminal: vt100::Parser::new(2, 8, SCROLLBACK_LINES),
            exited: false,
            view: PaneViewState::default(),
            scrollback_max: 0,
        });
        for _ in 0..1_000 {
            app.scroll_focused_pane(Direction::Right);
        }
        assert_eq!(app.panes[&pane_id(0, 0)].view.horizontal, MAX_HORIZONTAL_SCROLL);
    }

    #[test]
    fn carousel_preview_resize_preserves_terminal_history_dimensions() {
        let workspace = Workspace::parse(
            "peek: 3\ncolumns:\n  - name: one\n    layout: carousel\n    width: 40\n    panes:\n      - name: a\n      - name: b\n",
        )
        .unwrap();
        let mut app = App::new(workspace, SessionState::default()).unwrap();
        for pane in 0..2 {
            app.panes.insert(pane_id(0, pane), PaneRuntime {
                id: pane_id(0, pane),
                name: char::from(b'a' + pane as u8).to_string(),
                command: "sh".to_owned(),
                terminal: vt100::Parser::new(12, 38, SCROLLBACK_LINES),
                exited: false,
                view: PaneViewState::default(),
                scrollback_max: 0,
            });
        }
        let layout = Layout::calculate(&app.workspace, 80, 20).unwrap();
        app.resize_panes(&layout);
        assert_eq!(app.panes[&pane_id(0, 0)].terminal.screen().size(), (15, 38));
        assert_eq!(app.panes[&pane_id(0, 1)].terminal.screen().size(), (12, 38));
    }

    #[test]
    fn restores_and_normalizes_per_column_selection_and_pane_view() {
        let workspace = Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n        command: sleep 1\n      - name: b\n        command: sleep 1\n",
        )
        .unwrap();
        let restored_view = PaneViewState {
            vertical: 4,
            horizontal: u16::MAX,
            presentation: PresentationMode::Horizontal,
        };
        let restored = SessionState {
            focus: FocusRef { column: 0, pane: 1 },
            pane_selections: vec![99],
            pane_layouts: vec![PaneLayoutMode::Tabs],
            pane_weights: vec![vec![0, u16::MAX, 4]],
            pane_views: vec![vec![PaneViewState::default(), restored_view]],
            zoomed: Some(FocusRef { column: 0, pane: 1 }),
            ..SessionState::default()
        };
        let mut app = App::new(workspace, restored).unwrap();
        assert_eq!(app.pane_selections, vec![1]);
        assert_eq!(app.pane_layouts, vec![PaneLayoutMode::Tabs]);
        assert_eq!(app.pane_weights, vec![vec![1, MAX_PANE_WEIGHT]]);
        assert_eq!(app.zoomed, Some(FocusRef { column: 0, pane: 1 }));
        let layout = Layout::calculate(&app.workspace, 80, 20).unwrap();
        app.start_panes(&layout).unwrap();
        let normalized_view = PaneViewState { horizontal: MAX_HORIZONTAL_SCROLL, ..restored_view };
        assert_eq!(app.panes[&pane_id(0, 1)].view, normalized_view);
        assert_eq!(app.session_state().pane_selections, vec![1]);
        assert_eq!(app.session_state().pane_views[0][1], normalized_view);
        assert_eq!(app.session_state().pane_layouts, vec![PaneLayoutMode::Tabs]);
        assert_eq!(app.session_state().pane_weights, vec![vec![1, MAX_PANE_WEIGHT]]);
        assert_eq!(app.session_state().zoomed, Some(FocusRef { column: 0, pane: 1 }));
    }
}
