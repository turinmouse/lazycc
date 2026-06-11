use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

use super::state::ProfileForm;

pub(crate) const FORM_FIELD_COUNT: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TuiLayout {
    pub(crate) navigation: Rect,
    pub(crate) mcp: Rect,
    pub(crate) details: Rect,
    pub(crate) status: Rect,
}

pub(crate) fn tui_layout(area: Rect) -> TuiLayout {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(36), Constraint::Percentage(64)])
        .split(root[0]);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(columns[0]);
    TuiLayout {
        navigation: left[0],
        mcp: left[1],
        details: columns[1],
        status: root[1],
    }
}

pub(crate) fn form_fields<'a>(
    form: &'a ProfileForm,
) -> [(&'static str, &'a str); FORM_FIELD_COUNT] {
    [
        ("Name", form.name.as_str()),
        ("Base URL", form.base_url.as_str()),
        ("API key", form.api_key.as_str()),
        ("Model", form.model.as_str()),
    ]
}

pub(crate) fn form_layout(area: Rect, form: &ProfileForm) -> (Rect, Vec<Rect>) {
    let content_height = 1 + FORM_FIELD_COUNT as u16 * 3 + u16::from(form.error.is_some()) + 1;
    let area = centered_rect(72, content_height + 2, area);
    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let mut constraints = Vec::with_capacity(FORM_FIELD_COUNT + 3);
    constraints.push(Constraint::Length(1));
    constraints.extend((0..FORM_FIELD_COUNT).map(|_| Constraint::Length(3)));
    if form.error.is_some() {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner)
        .to_vec();

    (area, rows)
}

pub(crate) fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(height.min(area.height)),
            Constraint::Min(0),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(width.min(area.width)),
            Constraint::Min(0),
        ])
        .split(vertical[1]);
    horizontal[1]
}

pub(crate) fn rect_contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

pub(crate) fn navigation_list_area(area: Rect) -> Rect {
    area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    })
}

pub(crate) fn list_index_in_area(
    area: Rect,
    column: u16,
    row: u16,
    item_count: usize,
) -> Option<usize> {
    if !rect_contains(area, column, row) || item_count == 0 {
        return None;
    }

    let index = row.saturating_sub(area.y) as usize;
    (index < item_count).then_some(index)
}
