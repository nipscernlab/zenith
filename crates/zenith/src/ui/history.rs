//! The History tab: every call with its timing, and the selected one in more detail.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::cards::outcome_style;
use super::log::group;
use super::text::{self, pad, truncate};
use crate::app::App;
use crate::app::history::{CallRecord, Summary};
use crate::clock;
use crate::limits;

/// Draws the tab.
#[allow(
    clippy::too_many_lines,
    reason = "the tab is one picture: counts, columns, rows, detail and hints"
)]
pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let calls = &app.history.calls;
    let [header, columns, list, detail, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .areas(area);
    let width = usize::from(area.width);
    let summary = format!(
        " {} {} {} {} dropped {} kept {} calls or {} MiB",
        group(calls.len()),
        if calls.len() == 1 { "call" } else { "calls" },
        app.glyphs.dot,
        group(usize::try_from(calls.dropped()).unwrap_or(usize::MAX)),
        app.glyphs.dot,
        group(limits::HISTORY_ENTRIES),
        limits::HISTORY_BYTES / limits::MIB,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(summary, theme.muted())).style(theme.base()),
        header,
    );
    // The columns other than the method take 63 cells; the method gets what is left.
    let method_width = width.saturating_sub(63).clamp(12, 32);
    let heading = format!(
        "   {}  {}  {}  {}  {}  {}",
        pad("#", 4),
        pad("sent, UTC", 12),
        pad("method", method_width),
        pad("outcome", 16),
        pad("round trip", 10),
        "in SOLAR"
    );
    frame.render_widget(
        Paragraph::new(Span::styled(truncate(&heading, width, ""), theme.muted()))
            .style(theme.base()),
        columns,
    );
    let current = app.history.current();
    let rows = usize::from(list.height);
    let numbers: Vec<u64> = calls.iter().map(|(number, _)| number).collect();
    let position = current.and_then(|number| numbers.iter().position(|n| *n == number));
    let end = position.map_or(numbers.len(), |position| {
        (position + 1).max(rows.min(numbers.len()))
    });
    let start = end.saturating_sub(rows);
    let mut lines = Vec::new();
    for number in &numbers[start..end.min(numbers.len())] {
        let Some(record) = calls.get(*number) else {
            continue;
        };
        let chosen = Some(*number) == current;
        let marker = if chosen { app.glyphs.prompt } else { " " };
        let (outcome, style) = outcome_words(app, record);
        let round_trip = record.round_trip().map_or_else(String::new, clock::latency);
        let in_solar = record
            .outcome
            .as_ref()
            .and_then(|outcome| outcome.duration_us)
            .map_or_else(String::new, clock::micros);
        let method = record.method.clone().unwrap_or_else(|| "(raw)".to_owned());
        let line = Line::from(vec![
            Span::styled(format!(" {marker} "), theme.accent()),
            Span::styled(pad(&record.call.to_string(), 4), theme.muted()),
            Span::raw("  "),
            Span::styled(
                pad(&clock::Utc::of(record.sent_wall).time_of_day(), 12),
                theme.muted(),
            ),
            Span::raw("  "),
            Span::styled(
                pad(
                    &truncate(&text::clean(&method), method_width, app.glyphs.ellipsis),
                    method_width,
                ),
                theme.accent(),
            ),
            Span::raw("  "),
            Span::styled(pad(&truncate(&outcome, 16, app.glyphs.ellipsis), 16), style),
            Span::raw("  "),
            Span::styled(format!("{round_trip:>10}"), theme.text()),
            Span::raw("  "),
            Span::styled(format!("{in_solar:>8}"), theme.muted()),
        ]);
        lines.push(if chosen && app.history.selected.is_some() {
            line.style(theme.selection())
        } else {
            line
        });
    }
    if numbers.is_empty() {
        lines.push(Line::from(Span::styled("   No calls yet.", theme.muted())));
    }
    frame.render_widget(Paragraph::new(lines).style(theme.base()), list);

    let mut about = vec![Line::from(Span::styled(
        format!(" {}", app.glyphs.rule.repeat(width.saturating_sub(2))),
        theme.faint(),
    ))];
    if let Some(record) = current.and_then(|number| calls.get(number)) {
        about.push(Line::from(vec![
            Span::styled(" request   ", theme.muted()),
            Span::styled(
                truncate(
                    &text::clean(&record.request),
                    width.saturating_sub(12),
                    app.glyphs.ellipsis,
                ),
                theme.text(),
            ),
        ]));
        let response = match (&record.outcome, &record.closed) {
            (Some(outcome), _) => outcome
                .line
                .clone()
                .unwrap_or_else(|| "(too long to keep)".to_owned()),
            (None, Some(why)) => format!("(none: {why})"),
            (None, None) => "(waiting)".to_owned(),
        };
        about.push(Line::from(vec![
            Span::styled(" response  ", theme.muted()),
            Span::styled(
                truncate(
                    &text::clean(&response),
                    width.saturating_sub(12),
                    app.glyphs.ellipsis,
                ),
                theme.text(),
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(about).style(theme.base()), detail);
    super::draw_hints(
        frame,
        hints,
        app,
        &[
            ("Enter", "view"),
            ("r", "send again"),
            ("e", "edit"),
            ("x", "export"),
        ],
    );
}

fn outcome_words(app: &App, record: &CallRecord) -> (String, ratatui::style::Style) {
    let theme = &app.theme;
    if let Some(why) = &record.closed {
        return (format!("closed: {why}"), theme.error());
    }
    let Some(outcome) = &record.outcome else {
        return ("waiting".to_owned(), theme.muted());
    };
    let words = match &outcome.summary {
        Summary::Ok => "ok".to_owned(),
        Summary::Error { status, .. } => format!("error {status}"),
        Summary::Batch { count } => format!("batch of {count}"),
        Summary::NotJson => "not JSON".to_owned(),
        Summary::TooLong { bytes } => format!("too long, {bytes} B"),
    };
    let words = if outcome.violations > 0 {
        format!("{words}, contract")
    } else {
        words
    };
    (words, outcome_style(theme, &outcome.summary))
}
