use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct AppLayout {
    pub top_bar: Rect,
    pub sidebar: Option<Rect>,
    pub tab_bar: Rect,
    pub editor_area: Rect,
    pub status_bar: Rect,
}

pub fn compute_layout(area: Rect, sidebar_visible: bool, sidebar_width: u16) -> AppLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let top_bar = vertical[0];
    let middle = vertical[1];
    let status_bar = vertical[2];

    let (sidebar, right_panel) = if sidebar_visible && sidebar_width > 0 {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
            .split(middle);
        (Some(horizontal[0]), horizontal[1])
    } else {
        (None, middle)
    };

    let right_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(right_panel);

    AppLayout {
        top_bar,
        sidebar,
        tab_bar: right_split[0],
        editor_area: right_split[1],
        status_bar,
    }
}
