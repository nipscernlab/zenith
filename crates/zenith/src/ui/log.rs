//! The Log tab: SOLAR's standard error, at the level chosen here.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::fmt::Write as _;

use super::text::{self, pad, span, truncate, wrap};
use crate::app::App;
use crate::app::log::{Level, LogEntry};
use crate::clock;
use crate::keys::LevelKey;
use crate::limits;

/// Draws the tab.
#[allow(
    clippy::too_many_lines,
    reason = "the tab is one picture: levels, lines, expansion and hints"
)]
pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let tab = &app.log;
    let visible = tab.visible();
    let current = tab.current(&visible);
    let expanded = tab
        .expanded
        .then(|| current.and_then(|number| tab.entries.get(number)))
        .flatten();
    let width = usize::from(area.width);
    let expansion: Vec<Line<'static>> = expanded.map_or_else(Vec::new, |entry| {
        let mut lines = vec![Line::from(Span::styled(
            format!(" {}", app.glyphs.rule.repeat(width.saturating_sub(2))),
            theme.faint(),
        ))];
        lines.extend(wrap(
            &Line::from(vec![Span::raw(" "), span(&entry.message, theme.text())]),
            width.saturating_sub(1),
            1,
        ));
        lines.truncate(usize::from(area.height / 2));
        lines
    });
    let expansion_height = u16::try_from(expansion.len()).unwrap_or(0);
    let [header, list, below, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(expansion_height),
        Constraint::Length(1),
    ])
    .areas(area);

    let mut levels = vec![Span::styled(" level ", theme.muted())];
    for level in LevelKey::ALL {
        let chosen = level == tab.minimum;
        let label = if chosen {
            format!("[{}]", level.name())
        } else {
            format!(" {} ", level.name())
        };
        levels.push(Span::styled(
            label,
            if chosen {
                theme.tab_active()
            } else {
                theme.muted()
            },
        ));
    }
    let mut parts = Vec::new();
    if tab.solar_is_quieter() {
        parts.push(format!("SOLAR was started at {}", tab.solar_level));
    }
    parts.push(format!(
        "{} {}",
        group(tab.entries.len()),
        if tab.entries.len() == 1 {
            "line"
        } else {
            "lines"
        }
    ));
    parts.push(format!(
        "{} dropped",
        group(usize::try_from(tab.entries.dropped()).unwrap_or(usize::MAX))
    ));
    parts.push(format!(
        "keeps {} lines or {} MiB",
        group(limits::LOG_ENTRIES),
        limits::LOG_BYTES / limits::MIB
    ));
    let used: usize = levels.iter().map(|span| text::width(&span.content)).sum();
    let room = width.saturating_sub(used + 2);
    let separator = format!(" {} ", app.glyphs.dot);
    // The least important count goes first when the line is too narrow for all of them.
    while parts.len() > 1 && text::width(&parts.join(&separator)) > room {
        parts.pop();
    }
    let counts = truncate(&parts.join(&separator), room, app.glyphs.ellipsis);
    levels.push(Span::styled(format!("{counts:>room$} "), theme.muted()));
    frame.render_widget(
        Paragraph::new(Line::from(levels)).style(theme.base()),
        header,
    );

    let rows = usize::from(list.height);
    let position = current.and_then(|number| visible.iter().position(|n| *n == number));
    let end = position.map_or(visible.len(), |position| {
        (position + 1).max(rows.min(visible.len()))
    });
    let start = end.saturating_sub(rows);
    let mut lines = Vec::new();
    for number in &visible[start..end.min(visible.len())] {
        let Some(entry) = tab.entries.get(*number) else {
            continue;
        };
        let chosen = tab.selected.is_some() && Some(*number) == current;
        let line = entry_line(app, entry, width);
        lines.push(if chosen {
            line.style(theme.selection())
        } else {
            line
        });
    }
    if visible.is_empty() {
        let message = if tab.entries.is_empty() {
            "  SOLAR has written nothing to its standard error yet."
        } else {
            "  Nothing at this level. t shows everything."
        };
        lines.push(Line::from(Span::styled(message, theme.muted())));
    }
    frame.render_widget(Paragraph::new(lines).style(theme.base()), list);
    if expansion_height > 0 {
        frame.render_widget(Paragraph::new(expansion).style(theme.base()), below);
    }
    super::draw_hints(
        frame,
        hints,
        app,
        &[
            ("e w i d t", "level"),
            ("\u{2190} \u{2192}", "fewer, more"),
            ("Enter", "whole line"),
            ("G", "follow"),
            ("c", "clear"),
        ],
    );
}

fn entry_line(app: &App, entry: &LogEntry, width: usize) -> Line<'static> {
    let theme = &app.theme;
    let time = entry
        .time
        .as_deref()
        .and_then(|time| time.split('T').nth(1))
        .map_or_else(
            || clock::Utc::of(entry.received_wall).time_of_day(),
            |clock_part| {
                clock_part
                    .trim_end_matches('Z')
                    .chars()
                    .take(12)
                    .collect::<String>()
            },
        );
    let level_style = match entry.level {
        Level::Known(LevelKey::Error) | Level::Raw => theme.error(),
        Level::Known(LevelKey::Warn) => theme.accent(),
        Level::Known(LevelKey::Info) => theme.text(),
        Level::Known(LevelKey::Debug | LevelKey::Trace) => theme.muted(),
    };
    let mut spans = vec![
        Span::styled(format!(" {} ", pad(&time, 12)), theme.muted()),
        Span::styled(pad(entry.level.word(), 6), level_style),
    ];
    let mut context = String::new();
    if let Some(id) = &entry.request_id {
        let _ = write!(context, "#{id} ");
    }
    if let Some(method) = &entry.method {
        context.push_str(method);
        context.push(' ');
    }
    if let Some(micros) = entry.duration_us {
        let _ = write!(context, "{} ", clock::micros(micros));
    }
    if !context.is_empty() {
        spans.push(Span::styled(
            text::clean(&context).into_owned(),
            theme.accent(),
        ));
    }
    let used: usize = spans.iter().map(|span| text::width(&span.content)).sum();
    let mut message = text::clean(&entry.message).into_owned();
    if entry.cut > 0 {
        let _ = write!(message, " (and {} more bytes, cut)", entry.cut);
    }
    spans.push(Span::styled(
        truncate(&message, width.saturating_sub(used), app.glyphs.ellipsis),
        theme.text(),
    ));
    Line::from(spans)
}

/// A number with a space every three digits, as the design writes limits: `20 000`.
#[must_use]
pub fn group(number: usize) -> String {
    let digits = number.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(digit);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn numbers_are_grouped_by_thousands() {
        assert_eq!(super::group(20_000), "20 000");
        assert_eq!(super::group(512), "512");
        assert_eq!(super::group(1_000_000), "1 000 000");
    }
}
