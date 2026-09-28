//! A single line of text being edited, with a cursor and a history.
//!
//! The cursor is a byte offset that always sits on a character boundary. What is drawn is
//! a window of the line as wide as the box, scrolled so that the cursor is always in it,
//! measured in terminal cells rather than characters, because a wide character takes two.

use unicode_width::UnicodeWidthChar;

use crate::limits;
use crate::ring::Ring;

/// A line being edited.
#[derive(Debug, Clone)]
pub struct LineEditor {
    text: String,
    cursor: usize,
    history: Ring<String>,
    /// Where the history is being walked: the entry shown, and the line as it was before
    /// the walk started, to go back to.
    walking: Option<(u64, String)>,
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl LineEditor {
    /// An empty line, with a history of the declared size.
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            history: Ring::new(
                limits::COMMAND_HISTORY_ENTRIES,
                limits::COMMAND_HISTORY_BYTES,
            ),
            walking: None,
        }
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The cursor, as a byte offset.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether there is no text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The history, oldest first.
    #[must_use]
    pub fn history(&self) -> &Ring<String> {
        &self.history
    }

    /// Replaces the whole line and puts the cursor at its end.
    pub fn set(&mut self, text: &str) {
        self.text = clean(text);
        self.cursor = self.text.len();
        self.walking = None;
    }

    /// Empties the line.
    pub fn clear(&mut self) {
        self.set("");
    }

    /// Inserts text at the cursor. A newline or a tab in pasted text becomes a space,
    /// because the line is one line, and other control characters are dropped.
    pub fn insert(&mut self, text: &str) {
        let text = clean(text);
        self.text.insert_str(self.cursor, &text);
        self.cursor += text.len();
        self.walking = None;
    }

    /// Replaces a byte range with text and puts the cursor after it, which is what
    /// accepting a completion does.
    pub fn replace(&mut self, start: usize, end: usize, with: &str) {
        let start = self.boundary(start.min(self.text.len()));
        let end = self.boundary(end.min(self.text.len())).max(start);
        let with = clean(with);
        self.text.replace_range(start..end, &with);
        self.cursor = start + with.len();
        self.walking = None;
    }

    fn boundary(&self, mut at: usize) -> usize {
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    fn previous_boundary(&self) -> usize {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(at, _)| at)
    }

    fn next_boundary(&self) -> usize {
        self.text[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |character| self.cursor + character.len_utf8())
    }

    /// Moves one character left.
    pub fn left(&mut self) {
        self.cursor = self.previous_boundary();
    }

    /// Moves one character right.
    pub fn right(&mut self) {
        self.cursor = self.next_boundary();
    }

    /// Moves to the start.
    pub fn home(&mut self) {
        self.cursor = 0;
    }

    /// Moves to the end.
    pub fn end(&mut self) {
        self.cursor = self.text.len();
    }

    /// Moves to the start of the word before the cursor, as readline does.
    pub fn word_left(&mut self) {
        let before: Vec<(usize, char)> = self.text[..self.cursor].char_indices().collect();
        let mut index = before.len();
        while index > 0 && !before[index - 1].1.is_alphanumeric() {
            index -= 1;
        }
        while index > 0 && before[index - 1].1.is_alphanumeric() {
            index -= 1;
        }
        self.cursor = before.get(index).map_or(0, |(at, _)| *at);
    }

    /// Moves to the end of the word after the cursor.
    pub fn word_right(&mut self) {
        let mut at = self.cursor;
        let rest = &self.text[self.cursor..];
        let mut characters = rest.chars().peekable();
        while let Some(character) = characters.peek().copied() {
            if character.is_alphanumeric() {
                break;
            }
            at += character.len_utf8();
            characters.next();
        }
        while let Some(character) = characters.peek().copied() {
            if !character.is_alphanumeric() {
                break;
            }
            at += character.len_utf8();
            characters.next();
        }
        self.cursor = at;
    }

    /// Deletes the character before the cursor.
    pub fn backspace(&mut self) {
        let start = self.previous_boundary();
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
        self.walking = None;
    }

    /// Deletes the character at the cursor.
    pub fn delete(&mut self) {
        let end = self.next_boundary();
        self.text.replace_range(self.cursor..end, "");
        self.walking = None;
    }

    /// Deletes back to the previous space, as `Ctrl+W` does in a shell.
    pub fn delete_word(&mut self) {
        let before = &self.text[..self.cursor];
        let trimmed = before.trim_end();
        let start = trimmed
            .char_indices()
            .rev()
            .find(|(_, character)| character.is_whitespace())
            .map_or(0, |(at, character)| at + character.len_utf8());
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
        self.walking = None;
    }

    /// Deletes from the start to the cursor.
    pub fn delete_to_start(&mut self) {
        self.text.replace_range(..self.cursor, "");
        self.cursor = 0;
        self.walking = None;
    }

    /// Deletes from the cursor to the end.
    pub fn delete_to_end(&mut self) {
        self.text.truncate(self.cursor);
        self.walking = None;
    }

    /// Takes the line to run it, empties the editor, and keeps the line in the history
    /// unless it repeats the last one.
    pub fn take(&mut self) -> String {
        let line = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.walking = None;
        let repeats = self.history.last().is_some_and(|(_, last)| *last == line);
        if !line.trim().is_empty() && !repeats {
            self.history.push(line.clone());
        }
        line
    }

    /// Puts a line of an earlier session into the history, as the newest.
    pub fn remember(&mut self, line: &str) {
        if !line.trim().is_empty() {
            self.history.push(clean(line));
        }
    }

    /// Empties the history and says how many lines it held.
    pub fn forget_history(&mut self) -> usize {
        let held = self.history.len();
        self.history.clear();
        self.walking = None;
        held
    }

    /// Shows the previous line of the history.
    pub fn history_previous(&mut self) {
        let target = match &self.walking {
            None => self.history.last().map(|(number, _)| number),
            Some((number, _)) => number
                .checked_sub(1)
                .filter(|previous| *previous >= self.history.first_number()),
        };
        let Some(target) = target else { return };
        let Some(line) = self.history.get(target).cloned() else {
            return;
        };
        let draft = match self.walking.take() {
            Some((_, draft)) => draft,
            None => self.text.clone(),
        };
        self.text = line;
        self.cursor = self.text.len();
        self.walking = Some((target, draft));
    }

    /// Shows the next line of the history, and the line as it was after the last one.
    pub fn history_next(&mut self) {
        let Some((number, draft)) = self.walking.take() else {
            return;
        };
        if let Some(line) = self.history.get(number + 1).cloned() {
            self.text = line;
            self.walking = Some((number + 1, draft));
        } else {
            self.text = draft;
        }
        self.cursor = self.text.len();
    }

    /// The part of the line that fits in `width` cells, as the byte offset it starts at,
    /// and the column of the cursor inside it.
    #[must_use]
    pub fn window(&self, width: usize) -> (usize, usize) {
        let width = width.max(1);
        let cells = |text: &str| -> usize { text.chars().map(|c| c.width().unwrap_or(0)).sum() };
        let before = cells(&self.text[..self.cursor]);
        if before < width {
            return (0, before);
        }
        // Scroll so the cursor sits on the last column of the window.
        let mut start = 0;
        let mut dropped = 0;
        for (at, character) in self.text.char_indices() {
            if before - dropped < width {
                start = at;
                break;
            }
            dropped += character.width().unwrap_or(0);
            start = at + character.len_utf8();
        }
        (start, before - dropped)
    }
}

/// The text with newlines and tabs as spaces and other control characters removed.
fn clean(text: &str) -> String {
    text.chars()
        .filter_map(|character| match character {
            '\n' | '\r' | '\t' => Some(' '),
            other if other.is_control() => None,
            other => Some(other),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(text: &str) -> LineEditor {
        let mut editor = LineEditor::new();
        editor.set(text);
        editor
    }

    #[test]
    fn typing_inserts_at_the_cursor_and_pasted_newlines_become_spaces() {
        let mut line = editor("/pig");
        line.left();
        line.insert("n");
        assert_eq!(line.text(), "/ping");
        line.end();
        line.insert(" one\ntwo\u{1b}");
        assert_eq!(line.text(), "/ping one two");
    }

    #[test]
    fn the_cursor_moves_by_characters_not_bytes() {
        let mut line = editor("aé");
        line.left();
        assert_eq!(line.cursor(), 1);
        line.backspace();
        assert_eq!(line.text(), "é");
        line.delete();
        assert_eq!(line.text(), "");
    }

    #[test]
    fn words_are_moved_over_and_deleted_as_a_shell_does() {
        let mut line = editor("/call solar.ping {\"a\": 1}");
        line.word_left();
        assert_eq!(&line.text()[line.cursor()..], "1}");
        line.end();
        line.delete_word();
        assert_eq!(line.text(), "/call solar.ping {\"a\": ");
        line.delete_word();
        assert_eq!(line.text(), "/call solar.ping ");
        line.home();
        line.word_right();
        assert_eq!(line.cursor(), 5);
        line.delete_to_end();
        assert_eq!(line.text(), "/call");
        line.delete_to_start();
        assert!(line.is_empty());
    }

    #[test]
    fn a_replacement_puts_the_cursor_after_what_was_inserted() {
        let mut line = editor("/des sol");
        line.replace(0, 4, "/describe");
        assert_eq!(line.text(), "/describe sol");
        assert_eq!(line.cursor(), 9);
    }

    #[test]
    fn the_history_is_walked_back_and_forth_and_keeps_the_draft() {
        let mut line = LineEditor::new();
        for text in ["/ping", "/version", "/version", "  "] {
            line.set(text);
            line.take();
        }
        assert_eq!(line.history().len(), 2);
        line.set("/li");
        line.history_previous();
        assert_eq!(line.text(), "/version");
        line.history_previous();
        assert_eq!(line.text(), "/ping");
        line.history_previous();
        assert_eq!(line.text(), "/ping");
        line.history_next();
        line.history_next();
        assert_eq!(line.text(), "/li");
    }

    #[test]
    fn the_window_keeps_the_cursor_in_view() {
        let line = editor("0123456789");
        assert_eq!(line.window(20), (0, 10));
        let (start, column) = line.window(4);
        assert_eq!(&line.text()[start..], "789");
        assert_eq!(column, 3);
    }

    #[test]
    fn a_wide_character_counts_as_two_cells() {
        let line = editor("日本");
        assert_eq!(line.window(10), (0, 4));
    }

    fn with(text: &str) -> LineEditor {
        let mut editor = LineEditor::default();
        editor.set(text);
        editor
    }

    #[test]
    fn a_line_exactly_as_wide_as_the_window_scrolls_so_the_cursor_stays_inside() {
        // Five cells before the cursor in a window of five: the cursor would be on the
        // sixth column, so the line scrolls by one.
        assert_eq!(with("abcde").window(5), (1, 4));
        assert_eq!(with("abcd").window(5), (0, 4));
        // In a window one cell wide, only the cursor fits, after the whole line.
        assert_eq!(with("ab").window(1), (2, 0));
        // Wide characters are counted by their cells and cut at their bytes.
        assert_eq!(with("日本").window(2), (6, 0));
        assert_eq!(with("日本").window(3), (3, 2));
    }
}
