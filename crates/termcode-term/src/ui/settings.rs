use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;
use termcode_view::settings::{SettingsCategory, SettingsFocus, SettingsPicker, SettingsState};

use super::overlay::render_overlay_frame;
use crate::display_width::{ui_char_width, ui_str_width};

/// Rows the frame costs on top of the item list: the two borders and the hint
/// line at the bottom. `App` subtracts the same amount when it tells the state
/// how many items fit, so paging moves by exactly one screen.
pub const CHROME_ROWS: usize = 3;

/// How many options the value list can show over a settings screen this tall.
/// `App` feeds the same number back into the picker so paging moves by exactly
/// one screenful and the highlight never scrolls out of sight.
pub fn picker_visible_rows(area_height: u16, option_count: usize) -> usize {
    // The popup floats inside the screen's frame and above its hint line, and
    // spends two more rows on its own borders.
    let max_options = area_height.saturating_sub(CHROME_ROWS as u16 + 3);
    (option_count as u16).min(max_options) as usize
}

/// Width of the category pane, including its divider column.
const CATEGORY_WIDTH: u16 = 16;

/// Which category the pane starts at, so `selected` is one of the `height` it
/// can draw. Scrolls by as little as it takes, and never past the end.
fn category_scroll(selected: usize, count: usize, height: usize) -> usize {
    if height == 0 || count <= height {
        return 0;
    }
    // One row past the selection, minus a screenful: the least that brings it
    // into view from below. Clamped to the last full screen, so the pane never
    // shows blank rows under a list that still has entries above.
    let below = (selected + 1).saturating_sub(height);
    below.min(count - height)
}

/// Share of the frame the popup takes, and the caps that keep it from becoming
/// a wall of empty space on a large terminal.
const WIDTH_PERCENT: u16 = 70;
const MAX_WIDTH: u16 = 96;
const HEIGHT_PERCENT: u16 = 75;
const MAX_HEIGHT: u16 = 24;
/// Below this the two panes cannot both be drawn, and the popup is dropped
/// rather than squeezed into columns it does not have.
const MIN_WIDTH: u16 = 30;
const MIN_HEIGHT: u16 = 5;

/// Where the settings popup lands in `frame`, or `None` when the terminal is
/// too small to draw it.
///
/// The single source of its geometry, shared by the widget below and `App`,
/// which sizes the row budget from it -- paging by a screenful only works if
/// both agree on how tall that screen is.
pub fn popup_area(frame: Rect) -> Option<Rect> {
    let width = (frame.width as u32 * WIDTH_PERCENT as u32 / 100) as u16;
    let width = width.min(MAX_WIDTH).min(frame.width);
    let height = (frame.height as u32 * HEIGHT_PERCENT as u32 / 100) as u16;
    let height = height.min(MAX_HEIGHT).min(frame.height);

    if width < MIN_WIDTH || height < MIN_HEIGHT {
        return None;
    }

    Some(Rect::new(
        frame.x + (frame.width - width) / 2,
        frame.y + (frame.height - height) / 2,
        width,
        height,
    ))
}

/// Where every part of the settings screen lands.
///
/// The single source of the screen's geometry, shared by the widget below and
/// `mouse.rs`. Two matching call sites would be one drift away from a click
/// selecting a row the user is not pointing at -- the same reason
/// `confirm_dialog::layout` and `explorer_toolbar::buttons` exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsLayout {
    pub popup: Rect,
    /// Rows of the category pane, up to but not including the divider.
    pub categories: Rect,
    /// The category drawn on the pane's first row; see [`category_scroll`].
    pub first_category: usize,
    /// The column the divider between the two panes occupies.
    pub divider_x: u16,
    /// Rows of the item pane. `None` where the popup is too narrow to draw
    /// one, which is the case the widget bails out of as well.
    pub items: Option<Rect>,
    /// The item drawn on the item pane's first row.
    pub first_item: usize,
    pub item_count: usize,
    /// The value list, while one is open and there is room to draw it.
    pub picker: Option<PickerLayout>,
}

/// Where the value list lands, when one is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerLayout {
    pub popup: Rect,
    /// Rows of the list inside its frame.
    pub options: Rect,
    pub first_option: usize,
    pub option_count: usize,
}

/// Which entry of a list a row belongs to, or `None` past its end.
fn entry_at(list: Rect, first: usize, count: usize, x: u16, y: u16) -> Option<usize> {
    if !(x >= list.x && x < list.x + list.width && y >= list.y && y < list.y + list.height) {
        return None;
    }
    let index = first + (y - list.y) as usize;
    (index < count).then_some(index)
}

impl SettingsLayout {
    /// The category under the pointer, if it is over the category pane.
    pub fn category_at(&self, x: u16, y: u16) -> Option<usize> {
        entry_at(
            self.categories,
            self.first_category,
            SettingsCategory::ALL.len(),
            x,
            y,
        )
    }

    /// The setting under the pointer, if it is over the item pane.
    pub fn item_at(&self, x: u16, y: u16) -> Option<usize> {
        entry_at(self.items?, self.first_item, self.item_count, x, y)
    }
}

impl PickerLayout {
    /// The option under the pointer, if it is over the list.
    pub fn option_at(&self, x: u16, y: u16) -> Option<usize> {
        entry_at(self.options, self.first_option, self.option_count, x, y)
    }
}

/// Lay the screen out over `frame`, or `None` where it cannot be drawn at all.
pub fn layout(state: &SettingsState, frame: Rect) -> Option<SettingsLayout> {
    let popup = popup_area(frame)?;

    let inner_x = popup.x + 1;
    let inner_y = popup.y + 1;
    let inner_width = popup.width.saturating_sub(2);
    let list_height = popup.height.saturating_sub(CHROME_ROWS as u16);

    let divider_x = inner_x + CATEGORY_WIDTH - 1;
    let items_width = inner_width.saturating_sub(CATEGORY_WIDTH);
    // The widget stops here too rather than squeezing the rows into columns it
    // does not have, so there is nothing to click.
    let items =
        (items_width >= 12).then(|| Rect::new(divider_x + 1, inner_y, items_width, list_height));

    Some(SettingsLayout {
        popup,
        categories: Rect::new(inner_x, inner_y, CATEGORY_WIDTH - 1, list_height),
        first_category: category_scroll(
            state.category_index,
            SettingsCategory::ALL.len(),
            list_height as usize,
        ),
        divider_x,
        items,
        first_item: state.scroll_offset,
        item_count: state.items.len(),
        picker: state
            .picker
            .as_ref()
            .and_then(|picker| picker_layout(picker, popup)),
    })
}

/// The value list, centred over the screen it belongs to and above its hint
/// line -- covering that would hide the way back out.
fn picker_layout(picker: &SettingsPicker, popup: Rect) -> Option<PickerLayout> {
    let width = (popup.width / 2)
        .clamp(20, 46)
        .min(popup.width.saturating_sub(4));
    let rows = picker_visible_rows(popup.height, picker.options.len()) as u16 + 2;
    if rows < 3 || width < 12 {
        return None;
    }
    let region_height = popup.height.saturating_sub(2);
    let frame = Rect::new(
        popup.x + (popup.width.saturating_sub(width)) / 2,
        popup.y + (region_height.saturating_sub(rows)) / 2,
        width,
        rows,
    );
    Some(PickerLayout {
        options: Rect::new(
            frame.x + 1,
            frame.y + 1,
            frame.width.saturating_sub(2),
            frame.height.saturating_sub(2),
        ),
        popup: frame,
        first_option: picker.scroll_offset,
        option_count: picker.options.len(),
    })
}

pub struct SettingsWidget<'a> {
    state: &'a SettingsState,
    theme: &'a Theme,
}

impl<'a> SettingsWidget<'a> {
    pub fn new(state: &'a SettingsState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for SettingsWidget<'_> {
    /// `area` is the whole frame: the popup floats over the sidebar and the
    /// editor alike, since what it edits belongs to neither.
    fn render(self, area: Rect, buf: &mut Buffer) {
        let Some(placed) = layout(self.state, area) else {
            return;
        };
        let area = placed.popup;
        render_overlay_frame(area, buf, self.theme);

        let bg = self.theme.ui.sidebar_bg.to_ratatui();
        let fg = self.theme.ui.foreground.to_ratatui();
        let dim = self.theme.ui.line_number.to_ratatui();
        let accent = self.theme.ui.info.to_ratatui();
        let selection = self.theme.ui.selection.to_ratatui();
        let warn = self.theme.ui.warning.to_ratatui();

        let inner_x = placed.categories.x;
        let inner_y = placed.categories.y;
        let inner_width = area.width.saturating_sub(2);
        let list_height = placed.categories.height;

        write_text(
            buf,
            area.x + 2,
            area.y,
            area.x + area.width - 1,
            " Settings ",
            Style::default()
                .fg(accent)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        );

        // Category pane. The categories outnumber the rows on a short
        // terminal, and the selected one has to be among those drawn: a
        // highlight nobody can see is a screen with no way to tell where you
        // are. Derived here rather than kept in the state -- the list is fixed
        // and short, so where it starts is a function of the selection and the
        // height, and nothing has to be kept in step with it.
        let categories_focused = self.state.focus == SettingsFocus::Categories;
        let first_category = placed.first_category;
        for (row, (i, category)) in SettingsCategory::ALL
            .iter()
            .enumerate()
            .skip(first_category)
            .enumerate()
        {
            let y = inner_y + row as u16;
            if y >= inner_y + list_height {
                break;
            }
            let is_current = i == self.state.category_index;
            let style = match (is_current, categories_focused) {
                (true, true) => Style::default()
                    .fg(fg)
                    .bg(selection)
                    .add_modifier(Modifier::BOLD),
                (true, false) => Style::default().fg(accent).bg(bg),
                _ => Style::default().fg(dim).bg(bg),
            };
            let label = format!(
                " {:<width$}",
                category.title(),
                width = CATEGORY_WIDTH as usize - 2
            );
            write_text(buf, inner_x, y, inner_x + CATEGORY_WIDTH - 1, &label, style);
        }

        // Divider between the panes.
        let divider_x = placed.divider_x;
        for y in inner_y..inner_y + list_height {
            buf[(divider_x, y)].set_char('\u{2502}').set_style(
                Style::default()
                    .fg(self.theme.ui.border.to_ratatui())
                    .bg(bg),
            );
        }

        // Item pane. `None` is the popup being too narrow to hold one, which
        // is also why nothing there can be clicked.
        let Some(item_pane) = placed.items else {
            return;
        };
        let items_x = item_pane.x;
        let items_width = item_pane.width;
        let items_end = items_x + items_width;
        let value_width = items_width.min(28) / 2;
        let label_width = items_width.saturating_sub(value_width + 2);

        let items_focused = self.state.focus == SettingsFocus::Items;
        for row in 0..list_height {
            let index = self.state.scroll_offset + row as usize;
            let Some(item) = self.state.items.get(index) else {
                break;
            };
            let y = inner_y + row;
            let is_selected = index == self.state.selected;
            let row_style = if is_selected && items_focused {
                Style::default().fg(fg).bg(selection)
            } else {
                Style::default().fg(fg).bg(bg)
            };

            // Paint the whole row first so the selection bar is unbroken.
            for x in items_x..items_end {
                buf[(x, y)].set_char(' ').set_style(row_style);
            }

            let label = truncate_to_width(&item.label, label_width.saturating_sub(1) as usize);
            write_text(buf, items_x + 1, y, items_end, &label, row_style);

            let mut value = item.value.display();
            // While a rebinding is being typed, the keys pressed so far stand in
            // for the value, so the user can see the chord taking shape.
            if is_selected {
                if let Some(captured) = &self.state.capturing {
                    value = if captured.is_empty() {
                        "press keys...".to_string()
                    } else {
                        format!("{captured} _")
                    };
                }
            }
            let value_style = if is_selected && self.state.capturing.is_some() {
                row_style.fg(warn).add_modifier(Modifier::BOLD)
            } else if item.value.is_editable() {
                row_style
            } else {
                row_style.fg(dim)
            };
            let value = truncate_to_width(&value, value_width as usize);
            let value_x = items_end.saturating_sub(value.chars().count() as u16 + 1);
            write_text(
                buf,
                value_x.max(items_x + 1),
                y,
                items_end,
                &value,
                value_style,
            );

            if item.restart_required {
                let marker = "*";
                write_text(
                    buf,
                    items_end.saturating_sub(1),
                    y,
                    items_end,
                    marker,
                    row_style.fg(warn),
                );
            }
        }

        // Hint line: the message from the last save if there is one, otherwise
        // the detail of the selected item, otherwise the keys that work here.
        let hint_y = area.y + area.height - 2;
        let (hint, hint_color) = match (&self.state.picker, &self.state.message) {
            (Some(picker), _) => (
                if picker.preview {
                    "\u{2191}\u{2193} preview   Enter/Space keep   \u{2190}/Esc cancel".to_string()
                } else {
                    "\u{2191}\u{2193} choose   Enter/Space apply   \u{2190}/Esc cancel".to_string()
                },
                accent,
            ),
            (None, Some(message)) => (message.clone(), accent),
            _ => match self.state.selected_item() {
                Some(item) if item.detail.is_some() => {
                    (item.detail.clone().unwrap_or_default(), dim)
                }
                _ => (
                    "\u{2191}\u{2193} move   \u{2190}\u{2192} pane   Enter/Space change   Esc close   * needs restart"
                        .to_string(),
                    dim,
                ),
            },
        };
        let hint = truncate_to_width(&hint, inner_width.saturating_sub(2) as usize);
        write_text(
            buf,
            inner_x + 1,
            hint_y,
            area.x + area.width - 1,
            &hint,
            Style::default().fg(hint_color).bg(bg),
        );

        if let (Some(picker), Some(picker_placed)) = (&self.state.picker, &placed.picker) {
            self.render_picker(picker, picker_placed, buf);
        }
    }
}

impl SettingsWidget<'_> {
    /// The value list. Where it lands is [`picker_layout`]'s answer, not a
    /// second copy of the arithmetic: `mouse.rs` picks options out of the same
    /// rows this draws them on.
    fn render_picker(&self, picker: &SettingsPicker, placed: &PickerLayout, buf: &mut Buffer) {
        let bg = self.theme.ui.sidebar_bg.to_ratatui();
        let fg = self.theme.ui.foreground.to_ratatui();
        let dim = self.theme.ui.line_number.to_ratatui();
        let accent = self.theme.ui.info.to_ratatui();
        let selection = self.theme.ui.selection.to_ratatui();

        let popup = placed.popup;
        render_overlay_frame(popup, buf, self.theme);

        let title = truncate_to_width(
            &format!(" {} ", picker.title),
            popup.width.saturating_sub(4) as usize,
        );
        write_text(
            buf,
            popup.x + 2,
            popup.y,
            popup.x + popup.width - 1,
            &title,
            Style::default()
                .fg(accent)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        );

        let list_x = placed.options.x;
        let list_end = placed.options.x + placed.options.width;
        let visible = placed.options.height;
        for row in 0..visible {
            let index = placed.first_option + row as usize;
            let Some(option) = picker.options.get(index) else {
                break;
            };
            let y = placed.options.y + row;
            let is_highlighted = index == picker.selected;
            let style = if is_highlighted {
                Style::default().fg(fg).bg(selection)
            } else {
                Style::default().fg(fg).bg(bg)
            };
            for x in list_x..list_end {
                buf[(x, y)].set_char(' ').set_style(style);
            }
            // Mark where the setting stood when the list opened, so the way
            // back is visible even after previewing something else.
            let marker = if index == picker.original { "*" } else { " " };
            write_text(buf, list_x, y, list_end, marker, style.fg(dim));
            let label = truncate_to_width(option, list_end.saturating_sub(list_x + 2) as usize);
            write_text(buf, list_x + 2, y, list_end, &label, style);
        }
    }
}

/// Draw `text` from `x`, stopping before `end_x`.
fn write_text(buf: &mut Buffer, x: u16, y: u16, end_x: u16, text: &str, style: Style) {
    let mut cursor = x;
    for ch in text.chars() {
        let width = ui_char_width(ch).max(1) as u16;
        if cursor + width > end_x {
            break;
        }
        buf[(cursor, y)].set_char(ch).set_style(style);
        cursor += width;
    }
}

/// Cut `text` down to `width` display columns, so a long path or message
/// cannot run past the pane it belongs to.
fn truncate_to_width(text: &str, width: usize) -> String {
    if ui_str_width(text) <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = ui_char_width(ch).max(1);
        if used + w > width.saturating_sub(1) {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('\u{2026}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_with_room_for_every_category_starts_at_the_first() {
        assert_eq!(category_scroll(0, 5, 5), 0);
        assert_eq!(category_scroll(4, 5, 5), 0);
        assert_eq!(category_scroll(4, 5, 9), 0);
    }

    #[test]
    fn a_short_pane_scrolls_just_far_enough_to_show_the_selection() {
        // Three rows for five categories: the first three need no scroll.
        assert_eq!(category_scroll(0, 5, 3), 0);
        assert_eq!(category_scroll(2, 5, 3), 0);
        assert_eq!(category_scroll(3, 5, 3), 1);
        assert_eq!(category_scroll(4, 5, 3), 2);
    }

    #[test]
    fn the_pane_never_scrolls_past_the_last_full_screen() {
        // Whatever is asked for, the last row of the list is the last drawn.
        for selected in 0..5 {
            assert!(category_scroll(selected, 5, 3) <= 2);
        }
        assert_eq!(category_scroll(4, 5, 1), 4);
        assert_eq!(
            category_scroll(0, 5, 0),
            0,
            "a pane with no rows cannot scroll"
        );
    }
}
