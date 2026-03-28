use ratatui::layout::{Constraint, Direction, Layout, Rect};
use termcode_theme::theme::PaneFocusStyle;

pub struct AppLayout {
    pub top_bar: Rect,
    pub sidebar: Option<Rect>,
    pub sidebar_title: Option<Rect>,
    pub sidebar_border: Option<Rect>,
    pub tab_bar: Rect,
    pub editor_area: Rect,
    pub status_bar: Rect,
}

pub fn compute_layout(
    area: Rect,
    sidebar_visible: bool,
    sidebar_width: u16,
    pane_focus_style: PaneFocusStyle,
) -> AppLayout {
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

    let (sidebar, sidebar_title, sidebar_border, right_panel) =
        if sidebar_visible && sidebar_width > 0 {
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
                .split(middle);
            let raw_sidebar = horizontal[0];
            let right = horizontal[1];

            match pane_focus_style {
                PaneFocusStyle::TitleBar | PaneFocusStyle::AccentLine => {
                    let vsplit = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(1), Constraint::Min(1)])
                        .split(raw_sidebar);
                    (Some(vsplit[1]), Some(vsplit[0]), None, right)
                }
                PaneFocusStyle::Border => {
                    if raw_sidebar.width > 1 {
                        let hsplit = Layout::default()
                            .direction(Direction::Horizontal)
                            .constraints([
                                Constraint::Length(raw_sidebar.width - 1),
                                Constraint::Length(1),
                            ])
                            .split(raw_sidebar);
                        (Some(hsplit[0]), None, Some(hsplit[1]), right)
                    } else {
                        (Some(raw_sidebar), None, None, right)
                    }
                }
            }
        } else {
            (None, None, None, middle)
        };

    let right_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(right_panel);

    AppLayout {
        top_bar,
        sidebar,
        sidebar_title,
        sidebar_border,
        tab_bar: right_split[0],
        editor_area: right_split[1],
        status_bar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::new(0, 0, 80, 24)
    }

    #[test]
    fn title_bar_style_splits_sidebar() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::TitleBar);
        let sidebar = layout.sidebar.unwrap();
        let title = layout.sidebar_title.unwrap();
        assert!(layout.sidebar_border.is_none());
        assert_eq!(title.height, 1);
        assert_eq!(title.width, 20);
        // sidebar is content-only (excludes title row)
        assert_eq!(sidebar.y, title.y + 1);
        assert_eq!(sidebar.height, area().height - 3); // minus top_bar, title, status_bar
    }

    #[test]
    fn border_style_splits_sidebar() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::Border);
        let sidebar = layout.sidebar.unwrap();
        let border = layout.sidebar_border.unwrap();
        assert!(layout.sidebar_title.is_none());
        assert_eq!(border.width, 1);
        assert_eq!(sidebar.width, 19);
        assert_eq!(border.x, sidebar.x + sidebar.width);
    }

    #[test]
    fn accent_line_style_same_as_title_bar_layout() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::AccentLine);
        let sidebar = layout.sidebar.unwrap();
        let title = layout.sidebar_title.unwrap();
        assert!(layout.sidebar_border.is_none());
        assert_eq!(title.height, 1);
        assert_eq!(sidebar.y, title.y + 1);
    }

    #[test]
    fn sidebar_hidden_all_none() {
        let layout = compute_layout(area(), false, 20, PaneFocusStyle::TitleBar);
        assert!(layout.sidebar.is_none());
        assert!(layout.sidebar_title.is_none());
        assert!(layout.sidebar_border.is_none());
    }

    #[test]
    fn sidebar_content_only_height() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::TitleBar);
        let sidebar = layout.sidebar.unwrap();
        // middle = 24 - 2 (top_bar + status_bar) = 22
        // sidebar content = 22 - 1 (title) = 21
        assert_eq!(sidebar.height, 21);
    }
}
