use crate::config::Workspace;
use anyhow::Result;
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusRef {
    pub column: usize,
    pub pane: usize,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewportState {
    pub offset: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnLayout {
    pub index: usize,
    pub x: u16,
    pub width: u16,
    pub panes: Vec<Rect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub columns: Vec<ColumnLayout>,
    pub canvas_width: u16,
    pub viewport_width: u16,
    pub content_height: u16,
}

impl Layout {
    pub fn calculate(workspace: &Workspace, viewport_width: u16, content_height: u16) -> Result<Self> {
        let mut x = 0_u16;
        let mut columns = Vec::with_capacity(workspace.columns.len());
        for (index, column) in workspace.columns.iter().enumerate() {
            let width = column.width.resolve(viewport_width, &workspace.presets)?;
            let panes = stack_rects(x, width, content_height, column.panes.len());
            columns.push(ColumnLayout { index, x, width, panes });
            x = x.saturating_add(width);
            if index + 1 < workspace.columns.len() {
                x = x.saturating_add(workspace.gap);
            }
        }
        Ok(Self { columns, canvas_width: x, viewport_width, content_height })
    }

    pub fn max_offset(&self) -> u16 {
        self.canvas_width.saturating_sub(self.viewport_width)
    }

    pub fn reveal(&self, viewport: &mut ViewportState, column: usize, peek: u16) {
        let Some(column) = self.columns.get(column) else { return };
        let usable_peek = peek.min(self.viewport_width / 3);
        let left = viewport.offset.saturating_add(usable_peek);
        let right = viewport.offset.saturating_add(self.viewport_width.saturating_sub(usable_peek));
        let column_right = column.x.saturating_add(column.width);
        if column.x < left {
            viewport.offset = column.x.saturating_sub(usable_peek);
        } else if column_right > right {
            viewport.offset = column_right.saturating_add(usable_peek).saturating_sub(self.viewport_width);
        }
        viewport.offset = viewport.offset.min(self.max_offset());
    }

    pub fn screen_rect(&self, canvas_rect: Rect, viewport: ViewportState) -> Option<Rect> {
        let left = canvas_rect.x.max(viewport.offset);
        let right = canvas_rect
            .x
            .saturating_add(canvas_rect.width)
            .min(viewport.offset.saturating_add(self.viewport_width));
        (left < right).then_some(Rect {
            x: left.saturating_sub(viewport.offset),
            y: canvas_rect.y,
            width: right.saturating_sub(left),
            height: canvas_rect.height,
        })
    }

    pub fn column_at_screen_x(&self, viewport: ViewportState, screen_x: u16) -> Option<usize> {
        let canvas_x = viewport.offset.saturating_add(screen_x);
        self.columns
            .iter()
            .find(|column| canvas_x >= column.x && canvas_x < column.x.saturating_add(column.width))
            .map(|column| column.index)
    }
}

fn stack_rects(x: u16, width: u16, height: u16, pane_count: usize) -> Vec<Rect> {
    let count = pane_count.max(1) as u16;
    let base = height / count;
    let remainder = height % count;
    let mut y = 0;
    (0..count)
        .map(|index| {
            let pane_height = base + u16::from(index < remainder);
            let rect = Rect::new(x, y, width, pane_height);
            y += pane_height;
            rect
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Workspace;

    fn workspace() -> Workspace {
        Workspace::parse(
            "gap: 2\npeek: 3\ncolumns:\n  - name: one\n    width: 40\n    panes:\n      - name: a\n  - name: two\n    width: 50\n    panes:\n      - name: b\n      - name: c\n  - name: three\n    width: 30\n    panes:\n      - name: d\n",
        ).unwrap()
    }

    #[test]
    fn calculates_strip_and_vertical_stack() {
        let layout = Layout::calculate(&workspace(), 80, 21).unwrap();
        assert_eq!(layout.canvas_width, 124);
        assert_eq!(layout.columns[1].x, 42);
        assert_eq!(layout.columns[1].panes, vec![Rect::new(42, 0, 50, 11), Rect::new(42, 11, 50, 10)]);
    }

    #[test]
    fn reveals_columns_and_clamps_to_canvas() {
        let layout = Layout::calculate(&workspace(), 80, 20).unwrap();
        let mut viewport = ViewportState::default();
        layout.reveal(&mut viewport, 1, 3);
        assert_eq!(viewport.offset, 15);
        layout.reveal(&mut viewport, 2, 3);
        assert_eq!(viewport.offset, 44);
        layout.reveal(&mut viewport, 0, 3);
        assert_eq!(viewport.offset, 0);
    }

    #[test]
    fn clips_canvas_rect_to_screen() {
        let layout = Layout::calculate(&workspace(), 80, 20).unwrap();
        assert_eq!(
            layout.screen_rect(Rect::new(42, 0, 50, 10), ViewportState { offset: 50 }),
            Some(Rect::new(0, 0, 42, 10))
        );
    }
}
