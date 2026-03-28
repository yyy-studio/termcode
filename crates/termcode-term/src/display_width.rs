use unicode_width::UnicodeWidthChar;

/// Returns the terminal display width of a single character.
/// CJK/fullwidth characters return 2, most others return 1.
/// Control characters return 0.
pub fn char_display_width(ch: char) -> usize {
    ch.width().unwrap_or(0)
}

/// Converts a character index within a line to a display column.
/// Sums the display widths of all characters before `char_idx`.
pub fn char_index_to_display_col(line: &str, char_idx: usize) -> usize {
    line.chars()
        .take(char_idx)
        .map(|ch| ch.width().unwrap_or(0))
        .sum()
}

/// Converts a display column to the corresponding character index.
/// Returns the index of the character that occupies the given display column.
/// If `display_col` falls on the second cell of a wide character, returns that character's index.
/// If `display_col` exceeds the line width, returns the total character count.
pub fn display_col_to_char_index(line: &str, display_col: usize) -> usize {
    let mut col = 0usize;
    for (i, ch) in line.chars().enumerate() {
        let w = ch.width().unwrap_or(0);
        if col + w > display_col {
            return i;
        }
        col += w;
    }
    line.chars().count()
}

/// Returns the total display width of a string.
pub fn str_display_width(s: &str) -> usize {
    s.chars().map(|ch| ch.width().unwrap_or(0)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_display_width_ascii() {
        assert_eq!(char_display_width('a'), 1);
        assert_eq!(char_display_width(' '), 1);
    }

    #[test]
    fn test_char_display_width_cjk() {
        assert_eq!(char_display_width('한'), 2);
        assert_eq!(char_display_width('글'), 2);
        assert_eq!(char_display_width('中'), 2);
        assert_eq!(char_display_width('あ'), 2);
    }

    #[test]
    fn test_char_index_to_display_col() {
        let line = "ab한글cd";
        assert_eq!(char_index_to_display_col(line, 0), 0);
        assert_eq!(char_index_to_display_col(line, 1), 1);
        assert_eq!(char_index_to_display_col(line, 2), 2); // before '한'
        assert_eq!(char_index_to_display_col(line, 3), 4); // before '글'
        assert_eq!(char_index_to_display_col(line, 4), 6); // before 'c'
        assert_eq!(char_index_to_display_col(line, 5), 7); // before 'd'
    }

    #[test]
    fn test_display_col_to_char_index() {
        let line = "ab한글cd";
        assert_eq!(display_col_to_char_index(line, 0), 0); // 'a'
        assert_eq!(display_col_to_char_index(line, 1), 1); // 'b'
        assert_eq!(display_col_to_char_index(line, 2), 2); // '한'
        assert_eq!(display_col_to_char_index(line, 3), 2); // second cell of '한' -> still char 2
        assert_eq!(display_col_to_char_index(line, 4), 3); // '글'
        assert_eq!(display_col_to_char_index(line, 6), 4); // 'c'
        assert_eq!(display_col_to_char_index(line, 8), 6); // past end
    }

    #[test]
    fn test_str_display_width() {
        assert_eq!(str_display_width("hello"), 5);
        assert_eq!(str_display_width("한글"), 4);
        assert_eq!(str_display_width("ab한글cd"), 8);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(char_index_to_display_col("", 0), 0);
        assert_eq!(display_col_to_char_index("", 0), 0);
        assert_eq!(str_display_width(""), 0);
    }
}
