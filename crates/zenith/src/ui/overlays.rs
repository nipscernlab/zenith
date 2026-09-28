//! What is drawn over the tabs: the help, and the envelope viewer.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use serde_json::Value;

use super::json;
use super::text::{self, pad, wrap};
use crate::app::App;
use crate::app::history::CallRecord;
use crate::clock;
use crate::keys;

/// The help overlay: every key of the key table, by where it works.
pub fn help(frame: &mut Frame<'_>, area: Rect, app: &App, scroll: usize) {
    let theme = &app.theme;
    // The whole tab is covered first, so that nothing of it shows at the sides.
    frame.render_widget(Clear, area);
    frame.render_widget(Block::new().style(theme.base()), area);
    let box_area = help_box(area);
    let lines = help_lines(app, box_area);
    draw_scrolled(
        frame,
        box_area,
        app,
        " keys ",
        lines,
        scroll,
        vec![("Esc", "close"), ("\u{2191} \u{2193}", "scroll")],
    );
}

/// The help's box, centred in the tab.
fn help_box(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(96);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y,
        width,
        area.height,
    )
}

/// How far down the help can be, of the `scroll` lines asked for: all of them, unless
/// that is past its last page.
#[must_use]
pub fn help_scroll(app: &App, scroll: usize) -> usize {
    let box_area = help_box(super::content_area(app));
    limit(scroll, help_lines(app, box_area).len(), box_area)
}

/// The help's lines, for its box.
fn help_lines(app: &App, box_area: Rect) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let inner_width = usize::from(box_area.width.saturating_sub(4));
    let key_width = 22.min(inner_width / 3);
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (place, rows) in keys::help_rows() {
        if rows.is_empty() {
            continue;
        }
        lines.push(Line::from(Span::styled(place.title(), theme.strong())));
        for (keys, help) in rows {
            let line = Line::from(vec![
                Span::raw("  "),
                Span::styled(pad(&app.glyphs.key_name(&keys), key_width), theme.key()),
                Span::raw("  "),
                Span::styled(help, theme.text()),
            ]);
            lines.extend(wrap(&line, inner_width, key_width + 4));
        }
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled("The mouse", theme.strong())));
    for (what, help) in keys::MOUSE {
        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled(pad(what, key_width), theme.key()),
            Span::raw("  "),
            Span::styled(*help, theme.text()),
        ]);
        lines.extend(wrap(&line, inner_width, key_width + 4));
    }
    lines.push(Line::from(""));
    lines.extend(wrap(
        &Line::from(Span::styled(
            "Every action has a plain key or a Ctrl key; Alt and the function keys are extra routes.",
            theme.muted(),
        )),
        inner_width,
        0,
    ));
    lines
}

/// The scroll a box of `lines` lines can have of `scroll`: down to its last page.
fn limit(scroll: usize, lines: usize, box_area: Rect) -> usize {
    scroll.min(lines.saturating_sub(usize::from(box_area.height.saturating_sub(2))))
}

/// The envelope viewer: one call's request and response, whole.
pub fn viewer(frame: &mut Frame<'_>, area: Rect, app: &App, record: u64, scroll: usize) {
    let Some(call) = app.history.calls.get(record) else {
        return;
    };
    let method = call.method.clone().unwrap_or_else(|| "(raw)".to_owned());
    let timing = call.round_trip().map_or_else(String::new, |trip| {
        format!(" {} {}", app.glyphs.dot, clock::latency(trip))
    });
    let title = format!(
        " call {} {} {}{timing} ",
        call.call,
        app.glyphs.dot,
        text::clean(&method)
    );
    let lines = viewer_lines(app, call, area);
    frame.render_widget(Clear, area);
    draw_scrolled(
        frame,
        area,
        app,
        &title,
        lines,
        scroll,
        vec![("\u{2190} \u{2192}", "other calls"), ("Esc", "close")],
    );
}

/// How far down the viewer on call `record` can be, of the `scroll` lines asked for: all
/// of them, unless that is past its last page.
#[must_use]
pub fn viewer_scroll(app: &App, record: u64, scroll: usize) -> usize {
    let area = super::content_area(app);
    app.history.calls.get(record).map_or(0, |call| {
        limit(scroll, viewer_lines(app, call, area).len(), area)
    })
}

/// The viewer's lines: the request and the response, as indented JSON when they are JSON.
fn viewer_lines(app: &App, call: &CallRecord, area: Rect) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let inner_width = usize::from(area.width.saturating_sub(4));
    let mut lines: Vec<Line<'static>> = vec![Line::from(vec![
        Span::styled("request", theme.strong()),
        Span::styled(
            format!(
                "  sent {} UTC",
                clock::Utc::of(call.sent_wall).time_of_day()
            ),
            theme.muted(),
        ),
    ])];
    lines.extend(json_block(app, &call.request, inner_width));
    lines.push(Line::from(""));
    let mut response_heading = vec![Span::styled("response", theme.strong())];
    match (&call.outcome, &call.closed) {
        (Some(outcome), _) => {
            response_heading.push(Span::styled(
                format!(
                    "  received {} UTC",
                    clock::Utc::of(outcome.received_wall).time_of_day()
                ),
                theme.muted(),
            ));
            lines.push(Line::from(response_heading));
            match &outcome.line {
                Some(line) => lines.extend(json_block(app, line, inner_width)),
                None => lines.push(Line::from(Span::styled(
                    "The response was longer than the 16 MiB ZENITH keeps.",
                    theme.error(),
                ))),
            }
        }
        (None, Some(why)) => {
            lines.push(Line::from(response_heading));
            lines.push(Line::from(Span::styled(
                format!("None: {why}."),
                theme.error(),
            )));
        }
        (None, None) => {
            lines.push(Line::from(response_heading));
            lines.push(Line::from(Span::styled("Waiting.", theme.muted())));
        }
    }
    lines
}

/// A line as indented JSON when it is JSON, and as it is otherwise.
fn json_block(app: &App, line: &str, width: usize) -> Vec<Line<'static>> {
    match serde_json::from_str::<Value>(line) {
        Ok(value) => json::pretty(&value, &app.theme)
            .into_iter()
            .flat_map(|line| {
                let indent = line.spans.first().map_or(0, |span| {
                    span.content.len() - span.content.trim_start().len()
                });
                wrap(&line, width, indent + 2)
            })
            .collect(),
        Err(_) => wrap(
            &Line::from(Span::styled(
                text::clean(line).into_owned(),
                app.theme.text(),
            )),
            width,
            0,
        ),
    }
}

fn draw_scrolled(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    title: &str,
    lines: Vec<Line<'static>>,
    scroll: usize,
    hints: Vec<(&str, &str)>,
) {
    let theme = &app.theme;
    let mut footer = vec![Span::raw(" ")];
    for (key, words) in hints {
        footer.push(Span::styled(app.glyphs.key_name(key), theme.key()));
        footer.push(Span::styled(format!(" {words}  "), theme.muted()));
    }
    let block = Block::new()
        .borders(Borders::ALL)
        .border_set(app.glyphs.border)
        .border_style(theme.faint())
        .style(theme.base())
        .title_top(Line::from(Span::styled(
            text::clean(title).into_owned(),
            theme.strong(),
        )))
        .title_bottom(Line::from(footer));
    frame.render_widget(Clear, area);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let height = usize::from(inner.height);
    let scroll = scroll.min(lines.len().saturating_sub(height));
    let shown: Vec<Line<'static>> = lines.into_iter().skip(scroll).take(height).collect();
    let padded = Rect::new(
        inner.x + 1,
        inner.y,
        inner.width.saturating_sub(2),
        inner.height,
    );
    frame.render_widget(Paragraph::new(shown), padded);
}
