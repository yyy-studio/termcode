use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders};
use termcode_theme::theme::PaneFocusStyle;

/// The narrowest the sidebar may be made, by drag or by the settings screen.
/// Both paths share the bound so neither can produce a width the other rejects.
pub const MIN_SIDEBAR_WIDTH: u16 = 10;
/// The widest, likewise.
pub const MAX_SIDEBAR_WIDTH: u16 = 80;
/// Columns the editor keeps whatever the frame's width, so the sidebar cannot
/// be dragged over the whole terminal.
pub const MIN_EDITOR_WIDTH: u16 = 20;

/// The width the sidebar may be dragged to in a frame this wide.
pub fn clamp_sidebar_width(width: u16, frame_width: u16) -> u16 {
    let max = MAX_SIDEBAR_WIDTH
        .min(frame_width.saturating_sub(MIN_EDITOR_WIDTH))
        .max(MIN_SIDEBAR_WIDTH);
    width.clamp(MIN_SIDEBAR_WIDTH, max)
}

pub struct AppLayout {
    /// The whole frame. Overlays centre themselves in it rather than in any of
    /// the regions below, so hit-testing one needs it too.
    pub frame: Rect,
    pub top_bar: Rect,
    pub sidebar: Option<Rect>,
    /// One row above the tree holding the project name and the explorer's
    /// action buttons.
    pub sidebar_toolbar: Option<Rect>,
    pub sidebar_title: Option<Rect>,
    pub sidebar_border: Option<Rect>,
    /// The one column the sidebar and the editor meet on, dragged to resize the
    /// sidebar. It is always the sidebar's last column, whatever is drawn
    /// there: the panel border, the focus border, or plain tree padding.
    pub sidebar_divider: Option<Rect>,
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

    let mut sidebar_divider = None;

    let (sidebar, sidebar_title, sidebar_border, sidebar_panel, editor_panel, right_panel) =
        if sidebar_visible && sidebar_width > 0 {
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
                .split(middle);
            let raw_sidebar = horizontal[0];
            let right = horizontal[1];

            // `sidebar_width > 0` does not guarantee the split gave it any
            // columns -- a frame narrower than the requested width does not.
            if raw_sidebar.width > 0 {
                sidebar_divider = Some(Rect::new(
                    raw_sidebar.x + raw_sidebar.width - 1,
                    raw_sidebar.y,
                    1,
                    raw_sidebar.height,
                ));
            }

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

    // The toolbar takes the title row when there is one to take -- it carries
    // the same focus styling, so nothing is lost by replacing " EXPLORER" with
    // the project name and the buttons. The other focus styles have no such row
    // and give up the tree's first line instead.
    let (sidebar, sidebar_title, sidebar_toolbar) = match (sidebar, sidebar_title) {
        (Some(sb), Some(title)) if pane_focus_style == PaneFocusStyle::TitleBar => {
            (Some(sb), None, Some(title))
        }
        (Some(sb), title) if sb.height >= 2 => {
            let toolbar = Rect::new(sb.x, sb.y, sb.width, 1);
            let tree = Rect::new(sb.x, sb.y + 1, sb.width, sb.height - 1);
            (Some(tree), title, Some(toolbar))
        }
        (sb, title) => (sb, title, None),
    };

    let right_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(right_panel);

    AppLayout {
        frame: area,
        top_bar,
        sidebar,
        sidebar_toolbar,
        sidebar_title,
        sidebar_border,
        sidebar_divider,
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
    fn the_divider_is_the_sidebars_last_column_in_every_style() {
        for style in [
            PaneFocusStyle::TitleBar,
            PaneFocusStyle::AccentLine,
            PaneFocusStyle::Border,
        ] {
            for panel_borders in [false, true] {
                let layout = compute_layout(area(), true, 20, style, panel_borders);
                let divider = layout
                    .sidebar_divider
                    .unwrap_or_else(|| panic!("{style:?} borders={panel_borders} has a divider"));
                assert_eq!(divider.x, 19, "{style:?} borders={panel_borders}");
                assert_eq!(divider.width, 1);
                // The whole seam is grabbable, top bar and status bar aside.
                assert_eq!(divider.y, 1);
                assert_eq!(divider.height, area().height - 2);
            }
        }
    }

    #[test]
    fn a_hidden_sidebar_has_no_divider() {
        assert!(
            compute_layout(area(), false, 20, PaneFocusStyle::TitleBar, false)
                .sidebar_divider
                .is_none()
        );
        assert!(
            compute_layout(area(), true, 0, PaneFocusStyle::TitleBar, false)
                .sidebar_divider
                .is_none()
        );
    }

    #[test]
    fn clamp_keeps_the_editor_usable_on_a_narrow_frame() {
        // Room for both: the requested width stands.
        assert_eq!(clamp_sidebar_width(30, 120), 30);
        // The bounds hold whatever is asked for.
        assert_eq!(clamp_sidebar_width(1, 120), MIN_SIDEBAR_WIDTH);
        assert_eq!(clamp_sidebar_width(200, 300), MAX_SIDEBAR_WIDTH);
        // 60 columns leaves the sidebar 40 once the editor has its 20.
        assert_eq!(clamp_sidebar_width(200, 60), 40);
        // Too narrow for both: the minimum wins rather than collapsing.
        assert_eq!(clamp_sidebar_width(200, 25), MIN_SIDEBAR_WIDTH);
    }

    #[test]
    fn title_bar_style_splits_sidebar() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::TitleBar, false);
        let sidebar = layout.sidebar.unwrap();
        // The title row is where the explorer toolbar goes under this style.
        let toolbar = layout.sidebar_toolbar.unwrap();
        assert!(layout.sidebar_title.is_none());
        assert!(layout.sidebar_border.is_none());
        assert_eq!(toolbar.height, 1);
        assert_eq!(toolbar.width, 20);
        assert_eq!(sidebar.y, toolbar.y + 1);
        assert_eq!(sidebar.height, area().height - 3);
    }

    #[test]
    fn styles_without_a_title_row_give_the_toolbar_the_first_tree_line() {
        for style in [PaneFocusStyle::Border, PaneFocusStyle::AccentLine] {
            let layout = compute_layout(area(), true, 20, style, false);
            let sidebar = layout.sidebar.unwrap();
            let toolbar = layout.sidebar_toolbar.unwrap();
            assert_eq!(toolbar.height, 1);
            assert_eq!(sidebar.y, toolbar.y + 1);
            assert_eq!(toolbar.width, sidebar.width);
        }
        // The accent line survives: it is not what the toolbar took.
        let accent = compute_layout(area(), true, 20, PaneFocusStyle::AccentLine, false);
        assert!(accent.sidebar_title.is_some());
    }

    #[test]
    fn a_hidden_sidebar_has_no_toolbar() {
        let layout = compute_layout(area(), false, 20, PaneFocusStyle::TitleBar, false);
        assert!(layout.sidebar_toolbar.is_none());
    }

    #[test]
    fn border_style_splits_sidebar() {
        let layout = compute_layout(area(), true, 20, PaneFocusStyle::Border, false);
        let sidebar = layout.sidebar.unwrap();
        let _ = layout.sidebar_toolbar.expect("toolbar row");
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
        // Title row, then the toolbar, then the tree.
        assert_eq!(sidebar.y, title.y + 2);
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
        assert_eq!(sidebar.width, 18); // 20 - 2 borders
        let toolbar = layout.sidebar_toolbar.unwrap();
        assert_eq!(toolbar.width, 18);
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
