//! A mark with words beside it, laid out as both brands say: the words start three
//! columns after the symbol, the first of them on the row that carries the name and each
//! of the others on the row below the one before.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::text;
use crate::brand::{GAP, Mark};
use crate::glyphs::Charset;
use crate::theme::Theme;

/// The lines of `mark` with `words` beside it, each line starting `indent` columns in.
/// Words beyond the last row of the symbol are left out.
#[must_use]
pub fn lines(
    mark: &Mark,
    charset: Charset,
    indent: usize,
    words: &[(String, Style)],
    theme: &Theme,
) -> Vec<Line<'static>> {
    mark.rows(charset)
        .into_iter()
        .enumerate()
        .map(|(row, drawing)| {
            let mut spans = Vec::with_capacity(4);
            if indent > 0 {
                spans.push(Span::styled(" ".repeat(indent), theme.base()));
            }
            spans.push(Span::styled(drawing, theme.mark()));
            if let Some((words, style)) = row
                .checked_sub(mark.name_row)
                .and_then(|index| words.get(index))
            {
                spans.push(Span::styled(" ".repeat(GAP), theme.base()));
                spans.push(Span::styled(words.clone(), *style));
            }
            Line::from(spans)
        })
        .collect()
}

/// The columns the widest of those lines takes, without the indent.
#[must_use]
pub fn width(mark: &Mark, words: &[(String, Style)]) -> usize {
    let widest = words
        .iter()
        .map(|(words, _)| text::width(words))
        .max()
        .unwrap_or(0);
    mark.width + GAP + widest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brand::{SOLAR, ZENITH};
    use crate::theme::{Depth, ThemeName};

    fn words(texts: &[&str]) -> Vec<(String, Style)> {
        texts
            .iter()
            .map(|text| ((*text).to_owned(), Style::default()))
            .collect()
    }

    fn plain(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn the_words_start_three_columns_after_the_symbol_from_the_name_row() {
        let theme = Theme::new(ThemeName::Night, Depth::None);
        let beside = words(&["NAME", "what", "who", "version"]);
        for mark in [ZENITH, SOLAR] {
            let lines = lines(&mark, Charset::Unicode, 2, &beside, &theme);
            assert_eq!(lines.len(), mark.height);
            for (row, line) in lines.iter().enumerate() {
                let text = plain(line);
                let after = text.chars().skip(2 + mark.width).collect::<String>();
                match row.checked_sub(mark.name_row) {
                    Some(index) if index < beside.len() => {
                        assert_eq!(after, format!("   {}", beside[index].0), "row {row}");
                    }
                    _ => assert_eq!(after, "", "row {row}"),
                }
            }
            assert_eq!(width(&mark, &beside), mark.width + 3 + 7);
        }
    }

    #[test]
    fn words_beyond_the_symbol_are_left_out() {
        let theme = Theme::new(ThemeName::Night, Depth::None);
        let beside = words(&["one", "two", "three", "four", "five"]);
        let lines = lines(&ZENITH, Charset::Ascii, 0, &beside, &theme);
        assert_eq!(lines.len(), 4);
        assert!(plain(&lines[3]).ends_with("   four"));
    }
}
