use crate::config::{PaneLayoutMode, Workspace};
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
    pub mode: PaneLayoutMode,
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
        Self::calculate_with_widths(workspace, viewport_width, content_height, &[])
    }

    pub fn calculate_with_widths(
        workspace: &Workspace,
        viewport_width: u16,
        content_height: u16,
        column_widths: &[Option<u16>],
    ) -> Result<Self> {
        let mut x = 0_u16;
        let mut columns = Vec::with_capacity(workspace.columns.len());
        for (index, column) in workspace.columns.iter().enumerate() {
            let width = column_widths
                .get(index)
                .copied()
                .flatten()
                .unwrap_or(column.width.resolve(viewport_width, &workspace.presets)?)
                .max(1);
            let panes = stack_rects(x, width, content_height, column.panes.len());
            columns.push(ColumnLayout {
                index,
                x,
                width,
                mode: column.layout,
                panes,
            });
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
        viewport.offset = self.reveal_offset(viewport.offset, column, peek);
    }

    pub fn reveal_offset(&self, offset: u16, column: usize, peek: u16) -> u16 {
        let Some(column) = self.columns.get(column) else { return offset };
        let usable_peek = peek.min(self.viewport_width / 3);
        let left = offset.saturating_add(usable_peek);
        let right = offset.saturating_add(self.viewport_width.saturating_sub(usable_peek));
        let column_right = column.x.saturating_add(column.width);
        let mut revealed = offset;
        if column.x < left {
            revealed = column.x.saturating_sub(usable_peek);
        } else if column_right > right {
            revealed = column_right.saturating_add(usable_peek).saturating_sub(self.viewport_width);
        }
        revealed.min(self.max_offset())
    }

    pub fn clamp_viewport(&self, viewport: &mut ViewportState) {
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

    pub fn pane_rects(&self, column: usize, focus_pane: usize, peek: u16) -> Vec<Option<Rect>> {
        let Some(column_layout) = self.columns.get(column) else { return Vec::new() };
        self.pane_rects_with_mode(column, focus_pane, peek, column_layout.mode)
    }

    pub fn pane_rects_with_mode(
        &self,
        column: usize,
        focus_pane: usize,
        peek: u16,
        mode: PaneLayoutMode,
    ) -> Vec<Option<Rect>> {
        let Some(column_layout) = self.columns.get(column) else { return Vec::new() };
        match mode {
            PaneLayoutMode::Fit => column_layout.panes.iter().copied().map(Some).collect(),
            PaneLayoutMode::Tabs => {
                let mut panes = vec![None; column_layout.panes.len()];
                if !panes.is_empty() {
                    let selected = focus_pane.min(panes.len() - 1);
                    panes[selected] = Some(Rect::new(
                        column_layout.x,
                        0,
                        column_layout.width,
                        self.content_height,
                    ));
                }
                panes
            }
            PaneLayoutMode::Carousel => self.carousel_pane_rects(column, focus_pane, peek),
        }
    }

    pub fn carousel_pane_rects(&self, column: usize, focus_pane: usize, peek: u16) -> Vec<Option<Rect>> {
        let Some(column_layout) = self.columns.get(column) else { return Vec::new() };
        let pane_count = column_layout.panes.len();
        if pane_count == 0 {
            return Vec::new();
        }

        let clamped_focus = focus_pane.min(pane_count - 1);
        if pane_count >= 3 {
            let preview_height = peek
                .max(2)
                .min((self.content_height.saturating_sub(1) / 3).max(1));
            let focus_height = self.content_height.saturating_sub(preview_height * 2);
            let previous = (clamped_focus + pane_count - 1) % pane_count;
            let next = (clamped_focus + 1) % pane_count;
            let mut panes = vec![None; pane_count];
            panes[previous] = Some(Rect::new(column_layout.x, 0, column_layout.width, preview_height));
            panes[clamped_focus] = Some(Rect::new(
                column_layout.x,
                preview_height,
                column_layout.width,
                focus_height,
            ));
            panes[next] = Some(Rect::new(
                column_layout.x,
                preview_height.saturating_add(focus_height),
                column_layout.width,
                preview_height,
            ));
            return panes;
        }

        let has_previous = clamped_focus > 0;
        let has_next = clamped_focus + 1 < pane_count;
        let visible_neighbors = u16::from(has_previous) + u16::from(has_next);
        let preview_height = if visible_neighbors == 0 {
            0
        } else {
            let max_preview = self.content_height.saturating_sub(1) / (visible_neighbors + 1);
            peek.max(2).min(max_preview.max(1))
        };

        let mut y = 0;
        let mut panes = vec![None; pane_count];
        if has_previous {
            panes[clamped_focus - 1] = Some(Rect::new(column_layout.x, y, column_layout.width, preview_height));
            y = y.saturating_add(preview_height);
        }

        let focus_height = self.content_height.saturating_sub(preview_height * visible_neighbors);
        panes[clamped_focus] = Some(Rect::new(column_layout.x, y, column_layout.width, focus_height));
        y = y.saturating_add(focus_height);

        if has_next {
            panes[clamped_focus + 1] = Some(Rect::new(column_layout.x, y, column_layout.width, preview_height));
        }

        panes
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
        )
        .unwrap()
    }

    fn carousel_workspace() -> Workspace {
        Workspace::parse(
            "gap: 2\npeek: 3\ncolumns:\n  - name: one\n    layout: carousel\n    width: 40\n    panes:\n      - name: a\n      - name: b\n      - name: c\n      - name: d\n",
        )
        .unwrap()
    }

    #[test]
    fn calculates_strip_and_vertical_stack() {
        let layout = Layout::calculate(&workspace(), 80, 21).unwrap();
        assert_eq!(layout.canvas_width, 124);
        assert_eq!(layout.columns[1].x, 42);
        assert_eq!(layout.columns[1].mode, PaneLayoutMode::Fit);
        assert_eq!(layout.columns[1].panes, vec![Rect::new(42, 0, 50, 11), Rect::new(42, 11, 50, 10)]);
    }

    #[test]
    fn honors_explicit_carousel_mode_for_two_panes() {
        let workspace = Workspace::parse(
            "gap: 2\npeek: 3\ncolumns:\n  - name: one\n    layout: carousel\n    width: 40\n    panes:\n      - name: a\n      - name: b\n",
        )
        .unwrap();
        let layout = Layout::calculate(&workspace, 80, 20).unwrap();
        let rects = layout.pane_rects(0, 0, 3);
        assert_eq!(layout.columns[0].mode, PaneLayoutMode::Carousel);
        assert_eq!(rects[0], Some(Rect::new(0, 0, 40, 17)));
        assert_eq!(rects[1], Some(Rect::new(0, 17, 40, 3)));
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

    #[test]
    fn applies_runtime_column_width_overrides() {
        let layout = Layout::calculate_with_widths(
            &workspace(),
            80,
            20,
            &[None, Some(64), None],
        )
        .unwrap();
        assert_eq!(layout.columns[1].width, 64);
        assert_eq!(layout.columns[2].x, 108);
    }

    #[test]
    fn carousel_focus_keeps_the_selected_pane_prominent_and_hides_distant_panes() {
        let layout = Layout::calculate(&carousel_workspace(), 80, 20).unwrap();
        let rects = layout.carousel_pane_rects(0, 1, 3);
        assert_eq!(rects[0], Some(Rect::new(0, 0, 40, 3)));
        assert_eq!(rects[1], Some(Rect::new(0, 3, 40, 14)));
        assert_eq!(rects[2], Some(Rect::new(0, 17, 40, 3)));
        assert_eq!(rects[3], None);
    }

    #[test]
    fn carousel_focus_at_column_edges_wraps_and_stays_centered() {
        let layout = Layout::calculate(&carousel_workspace(), 80, 20).unwrap();
        let rects = layout.carousel_pane_rects(0, 0, 3);
        assert_eq!(rects[0], Some(Rect::new(0, 3, 40, 14)));
        assert_eq!(rects[1], Some(Rect::new(0, 17, 40, 3)));
        assert_eq!(rects[2], None);
        assert_eq!(rects[3], Some(Rect::new(0, 0, 40, 3)));
    }

    #[test]
    fn tabs_mode_only_encloses_the_selected_pane() {
        let layout = Layout::calculate(&carousel_workspace(), 80, 20).unwrap();
        let rects = layout.pane_rects_with_mode(0, 2, 3, PaneLayoutMode::Tabs);
        assert_eq!(rects[0], None);
        assert_eq!(rects[1], None);
        assert_eq!(rects[2], Some(Rect::new(0, 0, 40, 20)));
        assert_eq!(rects[3], None);
    }

    #[test]
    fn clamps_restored_viewport_after_terminal_growth() {
        let layout = Layout::calculate(&workspace(), 120, 20).unwrap();
        let mut viewport = ViewportState { offset: 80 };
        layout.clamp_viewport(&mut viewport);
        assert_eq!(viewport.offset, 4);
    }
}
