use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders};
use termcode_theme::theme::PaneFocusStyle;

pub struct AppLayout {
    pub top_bar: Rect,
    pub sidebar: Option<Rect>,
    pub sidebar_title: Option<Rect>,
    pub sidebar_border: Option<Rect>,
    pub sidebar_panel: Option<Rect>,
    pub editor_panel: Option<Rect>,
    pub tab_bar: Rect,
    pub editor_area: Rect,
    pub status_bar: Rect,
}

pub fn compute_layout(
    area: Rect,
    sidebar_visible: bool,
    sidebar_width: u16,
    pane_focus_style: PaneFocusStyle,
    panel_borders: bool,
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

    let border_block = Block::default().borders(Borders::ALL);

    let (sidebar, sidebar_title, sidebar_border, sidebar_panel, editor_panel, right_panel) =
        if sidebar_visible && sidebar_width > 0 {
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
                .split(middle);
            let raw_sidebar = horizontal[0];
            let right = horizontal[1];

            if panel_borders {
                let sidebar_inner = border_block.inner(raw_sidebar);
                let editor_inner = border_block.inner(right);

                if sidebar_inner.width == 0 || sidebar_inner.height == 0 {
                    // Sidebar too small for borders, skip sidebar content
                    (
                        None,
                        None,
                        None,
                        Some(raw_sidebar),
                        Some(right),
                        editor_inner,
                    )
                } else if editor_inner.width == 0 || editor_inner.height == 0 {
                    // Editor too small for borders, skip editor content
                    (None, None, None, Some(raw_sidebar), Some(right), right)
                } else {
                    // Apply pane focus style inside the bordered sidebar
                    let (sb, sb_title, sb_border) = match pane_focus_style {
                        PaneFocusStyle::TitleBar | PaneFocusStyle::AccentLine => {
                            let vsplit = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([Constraint::Length(1), Constraint::Min(1)])
                                .split(sidebar_inner);
                            (Some(vsplit[1]), Some(vsplit[0]), None)
                        }
                        PaneFocusStyle::Border => {
                            // Skip sidebar_border when panel_borders is on (avoid double border)
                            (Some(sidebar_inner), None, None)
                        }
                    };
                    (
                        sb,
                        sb_title,
                        sb_border,
                        Some(raw_sidebar),
                        Some(right),
                        editor_inner,
                    )
                }
            } else {
                let (sb, sb_title, sb_border) = match pane_focus_style {
                    PaneFocusStyle::TitleBar | PaneFocusStyle::AccentLine => {
                        let vsplit = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([Constraint::Length(1), Constraint::Min(1)])
                            .split(raw_sidebar);
                        (Some(vsplit[1]), Some(vsplit[0]), None)
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
                            (Some(hsplit[0]), None, Some(hsplit[1]))
                        } else {
                            (Some(raw_sidebar), None, None)
                        }
                    }
                };
                (sb, sb_title, sb_border, None, None, right)
            }
        } else if panel_borders {
            let editor_inner = border_block.inner(middle);
            if editor_inner.width == 0 || editor_inner.height == 0 {
                (None, None, None, None, Some(middle), middle)
            } else {
                (None, None, None, None, Some(middle), editor_inner)
            }
        } else {
            (None, None, None, None, None, middle)
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
        sidebar_panel,
        editor_panel,
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
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::TitleBar, false);
        let sidebar = layout.sidebar.unwrap();
        let title = layout.sidebar_title.unwrap();
        assert!(layout.sidebar_border.is_none());
        assert_eq!(title.height, 1);
        assert_eq!(title.width, 20);
        assert_eq!(sidebar.y, title.y + 1);
        assert_eq!(sidebar.height, area().height - 3);
    }

    #[test]
    fn border_style_splits_sidebar() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::Border, false);
        let sidebar = layout.sidebar.unwrap();
        let border = layout.sidebar_border.unwrap();
        assert!(layout.sidebar_title.is_none());
        assert_eq!(border.width, 1);
        assert_eq!(sidebar.width, 19);
        assert_eq!(border.x, sidebar.x + sidebar.width);
    }

    #[test]
    fn accent_line_style_same_as_title_bar_layout() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::AccentLine, false);
        let sidebar = layout.sidebar.unwrap();
        let title = layout.sidebar_title.unwrap();
        assert!(layout.sidebar_border.is_none());
        assert_eq!(title.height, 1);
        assert_eq!(sidebar.y, title.y + 1);
    }

    #[test]
    fn sidebar_hidden_all_none() {
        let layout = compute_layout(area(), false, 20, PaneFocusStyle::TitleBar, false);
        assert!(layout.sidebar.is_none());
        assert!(layout.sidebar_title.is_none());
        assert!(layout.sidebar_border.is_none());
    }

    #[test]
    fn sidebar_content_only_height() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::TitleBar, false);
        let sidebar = layout.sidebar.unwrap();
        assert_eq!(sidebar.height, 21);
    }

    #[test]
    fn panel_borders_creates_panel_rects() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::TitleBar, true);
        let sp = layout.sidebar_panel.unwrap();
        let ep = layout.editor_panel.unwrap();
        assert_eq!(sp.width, 20);
        assert_eq!(ep.width, 60);
        // Content areas are inset by 1 on each side
        let sidebar = layout.sidebar.unwrap();
        assert_eq!(sidebar.width, 18 - 0); // 20 - 2 borders, minus 0 for title split
        let title = layout.sidebar_title.unwrap();
        assert_eq!(title.width, 18);
        assert_eq!(layout.tab_bar.width, 58);
        assert_eq!(layout.editor_area.width, 58);
    }

    #[test]
    fn panel_borders_border_style_skips_sidebar_border() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::Border, true);
        assert!(layout.sidebar_border.is_none());
        assert!(layout.sidebar.is_some());
        assert!(layout.sidebar_panel.is_some());
    }

    #[test]
    fn panel_borders_sidebar_hidden() {
        let layout = compute_layout(area(), false, 20, PaneFocusStyle::TitleBar, true);
        assert!(layout.sidebar_panel.is_none());
        assert!(layout.editor_panel.is_some());
        // Editor content inset
        assert_eq!(layout.editor_area.width, 78); // 80 - 2
    }

    #[test]
    fn no_panel_borders_no_panel_rects() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::TitleBar, false);
        assert!(layout.sidebar_panel.is_none());
        assert!(layout.editor_panel.is_none());
    }
}
