//! The settings screen draws with manual buffer writes, so its column
//! arithmetic has to stay inside the area it was handed. A cell written past
//! the edge panics, which in a TUI means aborting with the terminal still in
//! raw mode.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use termcode_term::ui::settings::SettingsWidget;
use termcode_theme::theme::Theme;
use termcode_view::settings::{
    SettingItem, SettingTarget, SettingValue, SettingsFocus, SettingsPicker, SettingsState,
};

fn picker(option_count: usize, selected: usize) -> SettingsPicker {
    SettingsPicker {
        item_index: 0,
        title: "Theme".to_string(),
        options: (0..option_count).map(|i| format!("option-{i}")).collect(),
        selected,
        original: 0,
        scroll_offset: 0,
        visible_height: 8,
        preview: true,
    }
}

fn state() -> SettingsState {
    let mut state = SettingsState::new();
    state.load_items(vec![
        SettingItem::new(
            "Theme",
            SettingValue::Choice {
                options: vec!["one-dark".into(), "gruvbox-dark".into()],
                selected: 0,
            },
            SettingTarget::Config(vec!["theme".into()]),
        ),
        SettingItem::new(
            "Sidebar Width",
            SettingValue::Int {
                value: 30,
                min: 10,
                max: 80,
                step: 1,
            },
            SettingTarget::Config(vec!["ui".into(), "sidebar_width".into()]),
        ),
        SettingItem::new(
            "Mouse Support",
            SettingValue::Bool(true),
            SettingTarget::Config(vec!["editor".into(), "mouse_enabled".into()]),
        )
        .needing_restart(),
        SettingItem::new(
            "Save File",
            SettingValue::KeyBinding(Some("Ctrl+S".into())),
            SettingTarget::Keybinding {
                mode: None,
                command: "file.save".into(),
            },
        )
        .with_detail("file.save  ·  [global]"),
    ]);
    state
}

fn draw(state: &SettingsState, area: Rect) {
    let theme = Theme::default();
    let mut buf = Buffer::empty(area);
    SettingsWidget::new(state, &theme).render(area, &mut buf);
}

#[test]
fn renders_at_a_range_of_sizes() {
    let state = state();
    for (width, height) in [(80, 24), (120, 40), (40, 10), (31, 6), (200, 60)] {
        draw(&state, Rect::new(0, 0, width, height));
    }
}

#[test]
fn refuses_to_draw_where_it_cannot_fit() {
    // Too small for the two panes: the widget must bail out rather than write
    // negative-width columns.
    let state = state();
    for (width, height) in [(29, 20), (80, 4), (1, 1)] {
        draw(&state, Rect::new(0, 0, width, height));
    }
}

#[test]
fn renders_a_capture_in_progress() {
    let mut state = state();
    state.selected = 3;
    state.capturing = Some("g g".to_string());
    draw(&state, Rect::new(0, 0, 80, 24));

    state.capturing = Some(String::new());
    draw(&state, Rect::new(0, 0, 80, 24));
}

#[test]
fn renders_values_too_long_for_their_column() {
    let mut state = state();
    state.load_items(vec![
        SettingItem::new(
            "A label far longer than the pane it has to fit inside of, by a lot",
            SettingValue::Info(
                "/an/extremely/long/path/that/will/not/fit/in/the/value/column/at/all".to_string(),
            ),
            SettingTarget::ReadOnly,
        )
        .with_detail("a detail line that also runs well past the right-hand edge of the screen"),
    ]);
    state.message = Some("a status message long enough to overrun the frame it sits in".into());
    draw(&state, Rect::new(0, 0, 42, 12));
}

#[test]
fn renders_the_value_picker_over_the_screen() {
    let mut state = state();
    for (width, height) in [(80, 24), (120, 40), (40, 10), (31, 6), (34, 7)] {
        state.picker = Some(picker(4, 2));
        draw(&state, Rect::new(0, 0, width, height));
    }
}

#[test]
fn renders_a_picker_with_more_options_than_rows() {
    let mut state = state();
    let mut p = picker(200, 150);
    p.scroll_offset = 145;
    state.picker = Some(p);
    // The popup has to clip to the screen rather than run off the bottom.
    draw(&state, Rect::new(0, 0, 80, 12));
    draw(&state, Rect::new(0, 0, 80, 6));
}

#[test]
fn renders_a_picker_with_labels_too_long_for_the_popup() {
    let mut state = state();
    let mut p = picker(3, 1);
    p.title = "A setting whose name is far wider than the popup".to_string();
    p.options[1] = "/an/absurdly/long/option/value/that/cannot/possibly/fit".to_string();
    state.picker = Some(p);
    draw(&state, Rect::new(0, 0, 44, 14));
}

#[test]
fn renders_with_either_pane_focused() {
    let mut state = state();
    state.set_focus(SettingsFocus::Categories);
    draw(&state, Rect::new(0, 0, 80, 24));
    state.set_focus(SettingsFocus::Items);
    draw(&state, Rect::new(0, 0, 80, 24));
}

#[test]
fn renders_a_scrolled_list_of_many_items() {
    let mut state = SettingsState::new();
    state.visible_height = 5;
    state.load_items(
        (0..120)
            .map(|i| {
                SettingItem::new(
                    format!("Command {i}"),
                    SettingValue::KeyBinding(None),
                    SettingTarget::ReadOnly,
                )
            })
            .collect(),
    );
    state.move_selection(119);
    draw(&state, Rect::new(0, 0, 80, 10));
}
