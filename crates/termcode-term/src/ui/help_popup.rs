use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;
use termcode_view::editor::EditorMode;

use crate::input::InputMapper;

pub struct HelpPopupWidget<'a> {
    theme: &'a Theme,
    sections: Vec<(&'static str, Vec<(String, &'static str)>)>,
}

impl<'a> HelpPopupWidget<'a> {
    /// Build the popup for the keymap that is actually active, so the listed
    /// keys stay correct under any preset or user override.
    pub fn new(theme: &'a Theme, mapper: &InputMapper, mode: EditorMode) -> Self {
        let sections = SECTIONS
            .iter()
            .filter_map(|(title, commands)| {
                let rows: Vec<(String, &'static str)> = commands
                    .iter()
                    .filter_map(|(command_id, label)| {
                        binding_for_help(mapper, mode, command_id).map(|keys| (keys, *label))
                    })
                    .collect();
                (!rows.is_empty()).then_some((*title, rows))
            })
            .collect();
        Self { theme, sections }
    }
}

/// Keys `App::handle_key` services before the keymap ever sees them, so no
/// keymap can bind or remove them. Without this the popup would list no way to
/// quit under any keymap that does not also bind `app.quit`.
const ALWAYS_AVAILABLE: &[(&str, &str)] = &[("app.quit", "Ctrl+Q")];

/// Find a key to advertise for `command_id`.
///
/// The popup is a reference sheet rather than a context-sensitive hint, so it
/// falls back across the editing modes: a keymap that binds `Esc → mode.normal`
/// only in Insert should still show that row when help is opened from Normal.
fn binding_for_help(mapper: &InputMapper, mode: EditorMode, command_id: &str) -> Option<String> {
    let (preferred, fallback) = if mode == EditorMode::Insert {
        (EditorMode::Insert, EditorMode::Normal)
    } else {
        (EditorMode::Normal, EditorMode::Insert)
    };
    mapper
        .binding_for(preferred, command_id)
        .or_else(|| mapper.binding_for(fallback, command_id))
        .or_else(|| {
            ALWAYS_AVAILABLE
                .iter()
                .find(|(id, _)| *id == command_id)
                .map(|(_, keys)| (*keys).to_string())
        })
}

/// Commands worth surfacing in the popup, grouped for display. Entries with no
/// binding in the active keymap are dropped rather than shown as blank.
const SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "General",
        &[
            ("app.quit", "Quit"),
            ("file.save", "Save"),
            ("view.toggle_sidebar", "Toggle Sidebar"),
            ("edit.undo", "Undo"),
            ("edit.redo", "Redo"),
            ("help.toggle", "Toggle Help"),
        ],
    ),
    (
        "Navigation",
        &[
            ("fuzzy.open", "Find File"),
            ("search.open", "Search"),
            ("search.open_replace", "Search & Replace"),
            ("palette.open", "Command Palette"),
            ("tab.next", "Next Tab"),
            ("tab.prev", "Previous Tab"),
            ("tab.close", "Close Tab"),
        ],
    ),
    (
        "Editing",
        &[
            ("mode.insert", "Edit Mode"),
            ("mode.normal", "Normal Mode"),
            ("edit.delete_line", "Delete Line"),
            ("edit.yank_line", "Yank Line"),
            ("edit.paste_after", "Paste"),
            ("clipboard.copy", "Copy"),
            ("clipboard.cut", "Cut"),
            ("clipboard.paste", "Paste (Clipboard)"),
        ],
    ),
    (
        "Code",
        &[
            ("goto.definition", "Go to Definition"),
            ("lsp.hover", "Hover Info"),
            ("diagnostic.next", "Next Diagnostic"),
            ("diagnostic.prev", "Previous Diagnostic"),
        ],
    ),
];

impl Widget for HelpPopupWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate content size
        let mut content_lines: Vec<(Style, String)> = Vec::new();
        let title = "Keyboard Shortcuts";
        let max_key_width = self
            .sections
            .iter()
            .flat_map(|(_, items)| items.iter())
            .map(|(k, _)| k.chars().count())
            .max()
            .unwrap_or(0);

        let content_width = max_key_width + 4 + 20; // key + separator + description
        let popup_width = (content_width + 4) as u16; // padding
        let popup_width = popup_width.min(area.width.saturating_sub(4));

        let bg = self.theme.ui.background.to_ratatui();
        let fg = self.theme.ui.foreground.to_ratatui();
        let border_color = self.theme.ui.border.to_ratatui();
        let title_style = Style::default()
            .fg(self.theme.ui.line_number_active.to_ratatui())
            .bg(bg);
        let section_style = Style::default().fg(self.theme.ui.info.to_ratatui()).bg(bg);
        let key_style = Style::default()
            .fg(self.theme.ui.line_number_active.to_ratatui())
            .bg(bg);
        let desc_style = Style::default().fg(fg).bg(bg);
        let border_style = Style::default().fg(border_color).bg(bg);
        let bg_style = Style::default().fg(fg).bg(bg);

        // Build content lines
        content_lines.push((title_style, title.to_string()));
        content_lines.push((bg_style, String::new())); // blank line

        for (section_name, items) in &self.sections {
            content_lines.push((section_style, format!("  {section_name}")));
            for (key, desc) in items {
                content_lines.push((
                    bg_style,
                    format!("    {key:<width$}  {desc}", width = max_key_width),
                ));
            }
            content_lines.push((bg_style, String::new())); // blank after section
        }

        // Add footer
        content_lines.push((desc_style, "  Press any key to close".to_string()));

        let popup_height = (content_lines.len() as u16 + 2).min(area.height.saturating_sub(2)); // +2 for border
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        let popup_rect = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Fill background (reset to clear any inherited modifiers like REVERSED cursor)
        for y in popup_rect.y..popup_rect.y + popup_rect.height {
            for x in popup_rect.x..popup_rect.x + popup_rect.width {
                if x < buf.area().width && y < buf.area().height {
                    buf[(x, y)].reset();
                    buf[(x, y)].set_char(' ').set_style(bg_style);
                }
            }
        }

        // Draw border
        let right = popup_rect.x + popup_rect.width - 1;
        let bottom = popup_rect.y + popup_rect.height - 1;

        // Top and bottom borders
        for x in popup_rect.x..=right {
            if x < buf.area().width {
                if popup_rect.y < buf.area().height {
                    buf[(x, popup_rect.y)]
                        .set_char(if x == popup_rect.x {
                            '╭'
                        } else if x == right {
                            '╮'
                        } else {
                            '─'
                        })
                        .set_style(border_style);
                }
                if bottom < buf.area().height {
                    buf[(x, bottom)]
                        .set_char(if x == popup_rect.x {
                            '╰'
                        } else if x == right {
                            '╯'
                        } else {
                            '─'
                        })
                        .set_style(border_style);
                }
            }
        }

        // Left and right borders
        for y in (popup_rect.y + 1)..bottom {
            if y < buf.area().height {
                if popup_rect.x < buf.area().width {
                    buf[(popup_rect.x, y)].set_char('│').set_style(border_style);
                }
                if right < buf.area().width {
                    buf[(right, y)].set_char('│').set_style(border_style);
                }
            }
        }

        // Render content lines
        let inner_x = popup_rect.x + 1;
        let inner_width = popup_rect.width.saturating_sub(2) as usize;
        for (i, (style, line)) in content_lines.iter().enumerate() {
            let y = popup_rect.y + 1 + i as u16;
            if y >= bottom {
                break;
            }
            if y >= buf.area().height {
                break;
            }

            // Determine style for each character based on position
            let is_shortcut_line = line.starts_with("    ") && !line.trim().is_empty();

            for (j, ch) in line.chars().enumerate() {
                if j >= inner_width {
                    break;
                }
                let x = inner_x + j as u16;
                if x < buf.area().width {
                    if is_shortcut_line {
                        // Key portion vs description
                        let trimmed_start = 4; // "    " prefix
                        let key_end = trimmed_start + max_key_width;
                        let char_style = if j < key_end { key_style } else { desc_style };
                        buf[(x, y)].set_char(ch).set_style(char_style);
                    } else {
                        buf[(x, y)].set_char(ch).set_style(*style);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use termcode_config::keymap::KeymapPreset;

    use crate::command::{CommandRegistry, register_builtin_commands};

    fn registry() -> CommandRegistry {
        let mut r = CommandRegistry::new();
        register_builtin_commands(&mut r);
        r
    }

    fn rows(widget: &HelpPopupWidget<'_>) -> Vec<(String, &'static str)> {
        widget
            .sections
            .iter()
            .flat_map(|(_, items)| items.iter().cloned())
            .collect()
    }

    #[test]
    fn default_keymap_lists_its_own_keys() {
        let theme = Theme::default();
        let mapper = InputMapper::new();
        let widget = HelpPopupWidget::new(&theme, &mapper, EditorMode::Normal);
        let rows = rows(&widget);
        assert!(rows.contains(&("Ctrl+S".to_string(), "Save")));
        assert!(rows.contains(&("Ctrl+P".to_string(), "Find File")));
    }

    #[test]
    fn quit_is_always_listed_even_though_no_keymap_binds_it() {
        let theme = Theme::default();
        // Built-in keymap: Ctrl+Q is hardcoded in App, not bound in any table.
        let listed = rows(&HelpPopupWidget::new(
            &theme,
            &InputMapper::new(),
            EditorMode::Normal,
        ));
        assert!(listed.contains(&("Ctrl+Q".to_string(), "Quit")));

        // Same for a preset that binds nothing at all.
        let empty: KeymapPreset = toml::from_str("").unwrap();
        let mapper = InputMapper::from_preset(&empty, &registry());
        let listed = rows(&HelpPopupWidget::new(&theme, &mapper, EditorMode::Normal));
        assert!(listed.contains(&("Ctrl+Q".to_string(), "Quit")));
    }

    #[test]
    fn a_preset_binding_for_quit_wins_over_the_hardcoded_key() {
        let preset: KeymapPreset = toml::from_str(
            r#"
[mode.normal]
"shift+z shift+q" = "app.quit"
"#,
        )
        .unwrap();
        let theme = Theme::default();
        let mapper = InputMapper::from_preset(&preset, &registry());
        let listed = rows(&HelpPopupWidget::new(&theme, &mapper, EditorMode::Normal));
        assert!(listed.contains(&("Shift+Z Shift+Q".to_string(), "Quit")));
    }

    #[test]
    fn insert_only_bindings_are_still_listed_from_normal_mode() {
        let preset: KeymapPreset = toml::from_str(
            r#"
[mode.insert]
"esc" = "mode.normal"
"#,
        )
        .unwrap();
        let theme = Theme::default();
        let mapper = InputMapper::from_preset(&preset, &registry());
        let listed = rows(&HelpPopupWidget::new(&theme, &mapper, EditorMode::Normal));
        assert!(listed.contains(&("Esc".to_string(), "Normal Mode")));
    }

    #[test]
    fn preset_keys_replace_the_defaults_in_the_popup() {
        let preset: KeymapPreset = toml::from_str(
            r#"
[global]
"ctrl+s" = "file.save"

[mode.normal]
"space f" = "fuzzy.open"
"g d" = "goto.definition"
"#,
        )
        .unwrap();
        let theme = Theme::default();
        let mapper = InputMapper::from_preset(&preset, &registry());
        let rows = rows(&HelpPopupWidget::new(&theme, &mapper, EditorMode::Normal));
        assert!(rows.contains(&("Space f".to_string(), "Find File")));
        assert!(rows.contains(&("g d".to_string(), "Go to Definition")));
        // Nothing is bound to these, so they are omitted rather than blank.
        assert!(!rows.iter().any(|(_, label)| *label == "Search"));
        assert!(!rows.iter().any(|(_, label)| *label == "Hover Info"));
    }
}
