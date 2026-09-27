//! Text for the screen: made safe, measured in cells, cut and wrapped.

use std::borrow::Cow;
use std::fmt::Write as _;

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Text from SOLAR with every control character replaced by a visible escape, so that
/// nothing SOLAR sends can drive the terminal. `SECURITY.md` promises this.
#[must_use]
pub fn clean(text: &str) -> Cow<'_, str> {
    if !text.chars().any(char::is_control) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for character in text.chars() {
        match character {
            '\t' => out.push_str("    "),
            character if character.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(character));
            }
            character => out.push(character),
        }
    }
    Cow::Owned(out)
}

/// The width of text in terminal cells.
#[must_use]
pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Text cut to `max` cells, with the ellipsis at the end when something was cut.
#[must_use]
pub fn truncate(text: &str, max: usize, ellipsis: &str) -> String {
    if width(text) <= max {
        return text.to_owned();
    }
    let room = max.saturating_sub(width(ellipsis));
    let mut out = String::new();
    let mut used = 0;
    for character in text.chars() {
        let cells = character.width().unwrap_or(0);
        if used + cells > room {
            break;
        }
        used += cells;
        out.push(character);
    }
    if max >= width(ellipsis) {
        out.push_str(ellipsis);
    }
    out
}

/// Text padded with spaces to `cells` on the right, or cut to it.
#[must_use]
pub fn pad(text: &str, cells: usize) -> String {
    let current = width(text);
    if current >= cells {
        truncate(text, cells, "")
    } else {
        format!("{text}{}", " ".repeat(cells - current))
    }
}

/// A styled line wrapped at word boundaries to `max` cells. The lines after the first
/// start with `hanging` spaces, so that a wrapped value stays in its column. A word
/// longer than a whole line is broken where it has to be.
#[must_use]
pub fn wrap(line: &Line<'_>, max: usize, hanging: usize) -> Vec<Line<'static>> {
    let max = max.max(1);
    let hanging = hanging.min(max.saturating_sub(1));
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    let push_line = |out: &mut Vec<Line<'static>>, current: &mut Vec<Span<'static>>| {
        // Trailing spaces at a break are not drawn.
        if let Some(last) = current.last_mut() {
            let trimmed = last.content.trim_end().to_owned();
            *last = Span::styled(trimmed, last.style);
        }
        out.push(Line::from(std::mem::take(current)));
    };
    for span in &line.spans {
        let style = span.style;
        for piece in pieces(&span.content) {
            let cells = width(piece);
            let is_space = piece.chars().all(char::is_whitespace);
            if used + cells <= max {
                current.push(Span::styled(piece.to_owned(), style));
                used += cells;
                continue;
            }
            if is_space {
                push_line(&mut out, &mut current);
                current.push(Span::raw(" ".repeat(hanging)));
                used = hanging;
                continue;
            }
            if used > hanging && cells <= max - hanging {
                push_line(&mut out, &mut current);
                current.push(Span::raw(" ".repeat(hanging)));
                current.push(Span::styled(piece.to_owned(), style));
                used = hanging + cells;
                continue;
            }
            // A word that does not fit even on a line of its own is broken by cells.
            for character in piece.chars() {
                let cells = character.width().unwrap_or(0);
                if used + cells > max {
                    push_line(&mut out, &mut current);
                    current.push(Span::raw(" ".repeat(hanging)));
                    used = hanging;
                }
                current.push(Span::styled(character.to_string(), style));
                used += cells;
            }
        }
    }
    if !current.is_empty() || out.is_empty() {
        out.push(Line::from(current));
    }
    out.into_iter().map(merge).collect()
}

/// Facts joined by `dot`, each kept whole: a fact that does not fit on the line starts
/// the next one, after `hanging` spaces, so that a line breaks between facts rather than
/// inside one. A fact longer than a whole line is wrapped like any text.
#[must_use]
pub fn facts(
    lead: &str,
    facts: Vec<Vec<Span<'static>>>,
    dot: &Span<'static>,
    max: usize,
    hanging: usize,
) -> Vec<Line<'static>> {
    let dot_cells = width(&dot.content);
    let mut out = Vec::new();
    let mut current = vec![Span::raw(lead.to_owned())];
    let mut used = width(lead);
    let mut first = true;
    for fact in facts {
        let cells: usize = fact.iter().map(|span| width(&span.content)).sum();
        if !first && used + dot_cells + cells > max {
            out.extend(wrap(
                &Line::from(std::mem::take(&mut current)),
                max,
                hanging,
            ));
            current.push(Span::raw(" ".repeat(hanging)));
            used = hanging;
            first = true;
        }
        if !first {
            current.push(dot.clone());
            used += dot_cells;
        }
        used += cells;
        current.extend(fact);
        first = false;
    }
    out.extend(wrap(&Line::from(current), max, hanging));
    out
}

/// A span's text split into words and the spaces between them.
fn pieces(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut in_space = None;
    for (at, character) in text.char_indices() {
        let space = character == ' ';
        match in_space {
            Some(previous) if previous != space => {
                pieces.push(&text[start..at]);
                start = at;
            }
            _ => {}
        }
        in_space = Some(space);
    }
    if start < text.len() {
        pieces.push(&text[start..]);
    }
    pieces
}

/// A line with neighbouring spans of the same style joined, which keeps the buffer small
/// and the snapshots readable.
fn merge(line: Line<'static>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for span in line.spans {
        if span.content.is_empty() {
            continue;
        }
        match spans.last_mut() {
            Some(last) if last.style == span.style => {
                let joined = format!("{}{}", last.content, span.content);
                *last = Span::styled(joined, last.style);
            }
            _ => spans.push(span),
        }
    }
    Line::from(spans)
}

/// A span of owned text, cleaned of control characters.
#[must_use]
pub fn span(text: &str, style: Style) -> Span<'static> {
    Span::styled(clean(text).into_owned(), style)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn control_characters_become_visible_escapes() {
        assert_eq!(clean("a\u{1b}[31mb"), "a\\u{1b}[31mb");
        assert_eq!(clean("plain"), "plain");
        assert!(matches!(clean("plain"), Cow::Borrowed(_)));
    }

    #[test]
    fn wide_characters_are_two_cells() {
        assert_eq!(width("日本"), 4);
        assert_eq!(truncate("日本語です", 5, "…"), "日本…");
        assert_eq!(pad("ab", 4), "ab  ");
        assert_eq!(pad("abcdef", 4), "abcd");
    }

    #[test]
    fn lines_wrap_at_words_with_a_hanging_indent() {
        let line = Line::from("the quick brown fox jumps");
        assert_eq!(
            texts(&wrap(&line, 10, 2)),
            vec!["the quick", "  brown", "  fox", "  jumps"]
        );
    }

    #[test]
    fn a_word_longer_than_the_line_is_broken() {
        let line = Line::from("abcdefghij");
        assert_eq!(texts(&wrap(&line, 4, 0)), vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn styles_survive_wrapping() {
        let style = Style::new().bold();
        let line = Line::from(vec![
            Span::raw("key "),
            Span::styled("value that wraps", style),
        ]);
        let wrapped = wrap(&line, 10, 4);
        assert_eq!(texts(&wrapped), vec!["key value", "    that", "    wraps"]);
        assert_eq!(wrapped[1].spans.last().unwrap().style, style);
    }

    #[test]
    fn an_empty_line_stays_one_line() {
        assert_eq!(wrap(&Line::from(""), 10, 0).len(), 1);
    }

    #[test]
    fn facts_break_between_them_and_never_inside_one() {
        let fact = |text: &str| vec![Span::raw(text.to_owned())];
        let dot = Span::raw(" · ");
        let facts_of = |max| {
            texts(&facts(
                " ",
                vec![
                    fact("idempotent"),
                    fact("budget 1000 ms"),
                    fact("responses up to 8 MiB"),
                ],
                &dot,
                max,
                1,
            ))
        };
        assert_eq!(
            facts_of(80),
            vec![" idempotent · budget 1000 ms · responses up to 8 MiB"]
        );
        assert_eq!(
            facts_of(40),
            vec![" idempotent · budget 1000 ms", " responses up to 8 MiB"]
        );
        assert_eq!(
            facts_of(16),
            vec![
                " idempotent",
                " budget 1000 ms",
                " responses up to",
                " 8 MiB"
            ]
        );
    }
}
