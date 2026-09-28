//! The APIs tab: the catalogue on the left, the entry or the form on the right.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use zenith_client::schema_view::FieldKind;

use super::text::{self, pad, span, truncate, wrap};
use super::{Scrolls, cards};
use crate::app::App;
use crate::app::apis::Form;

/// Draws the tab.
#[allow(
    clippy::too_many_lines,
    reason = "the tab is one picture: list, rule, entry and hints, laid out together"
)]
pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let [body, hints] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let Some(catalogue) = app.catalogue() else {
        let message = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No manifest yet: the catalogue is built from SOLAR's, and SOLAR is not connected.",
                theme.muted(),
            )),
        ])
        .style(theme.base());
        frame.render_widget(message, body);
        return;
    };
    let list_width = list_width(area.width);
    let [list, bar, detail] = columns(body);
    let visible = app.apis.visible(catalogue);
    let selected = app.apis.selected.min(visible.len().saturating_sub(1));

    // The list.
    let mut lines = Vec::new();
    let header = if app.apis.filtering || !app.apis.filter.is_empty() {
        let cursor = if app.apis.filtering { "_" } else { "" };
        Line::from(vec![
            Span::styled(" filter ", theme.muted()),
            span(app.apis.filter.text(), theme.accent()),
            Span::styled(cursor, theme.accent()),
        ])
    } else {
        Line::from(Span::styled(
            format!(
                " {} {} {} manifest {}",
                catalogue.apis.len(),
                if catalogue.apis.len() == 1 {
                    "API"
                } else {
                    "APIs"
                },
                app.glyphs.dot,
                catalogue.schema_version
            ),
            theme.muted(),
        ))
    };
    lines.push(header);
    let rows = usize::from(list.height).saturating_sub(1);
    let first = selected.saturating_sub(rows.saturating_sub(1));
    let name_width = usize::from(list_width).saturating_sub(12);
    for (index, api) in visible.iter().enumerate().skip(first).take(rows) {
        let chosen = index == selected;
        let marker = if chosen { app.glyphs.prompt } else { " " };
        let version = api.version.clone().unwrap_or_default();
        let line = Line::from(vec![
            Span::styled(format!(" {marker} "), theme.accent()),
            Span::styled(
                pad(
                    &truncate(&text::clean(&api.name), name_width, app.glyphs.ellipsis),
                    name_width,
                ),
                if chosen { theme.strong() } else { theme.text() },
            ),
            Span::styled(format!(" {}", pad(&version, 8)), theme.muted()),
        ]);
        lines.push(if chosen {
            line.style(theme.selection())
        } else {
            line
        });
    }
    if visible.is_empty() {
        lines.push(Line::from(Span::styled(
            "   nothing matches",
            theme.muted(),
        )));
    }
    frame.render_widget(Paragraph::new(lines).style(theme.base()), list);
    let rule: Vec<Line<'static>> = (0..bar.height)
        .map(|_| Line::from(Span::styled(app.glyphs.bar, theme.faint())))
        .collect();
    frame.render_widget(Paragraph::new(rule).style(theme.base()), bar);

    // The entry, or the form.
    let width = usize::from(detail.width).saturating_sub(2);
    let content = match (&app.apis.form, visible.get(selected)) {
        (Some(form), _) => form_lines(app, form, width),
        (None, Some(api)) => cards::api_entry(app, api, width, 1, Some(&app.apis.results)),
        (None, None) => Vec::new(),
    };
    let scroll = usize::from(entry_scroll(app.apis.scroll, content.len()));
    let shown: Vec<Line<'static>> = content.into_iter().skip(scroll).collect();
    frame.render_widget(Paragraph::new(shown).style(theme.base()), detail);

    let hint = if app.apis.form.is_some() {
        vec![
            ("Enter", "run"),
            ("Tab", "next field"),
            ("Space", "toggle"),
            ("Esc", "close"),
        ]
    } else {
        vec![
            ("1 to 9", "run an example"),
            ("a", "all"),
            ("Enter", "form"),
            ("e", "edit"),
            ("f", "filter"),
            ("R", "reload"),
        ]
    };
    frame.render_widget(super::hint_bar(app, &hint), hints);
}

/// The list's width, for a tab `width` cells wide.
fn list_width(width: u16) -> u16 {
    (width / 3).clamp(28, 40)
}

/// The list, the rule and the entry, left to right.
fn columns(body: Rect) -> [Rect; 3] {
    Layout::horizontal([
        Constraint::Length(list_width(body.width)),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(body)
}

/// The scroll an entry of `lines` lines can have of `scroll`: down to its last line.
fn entry_scroll(scroll: u16, lines: usize) -> u16 {
    scroll.min(u16::try_from(lines.saturating_sub(1)).unwrap_or(u16::MAX))
}

/// What the wheel scrolls at `position` of the tab: the list left of the rule and the
/// entry right of it, and nothing while the form is open, whose fields `Tab` moves
/// between.
#[must_use]
pub fn scrolls_at(app: &App, area: Rect, position: Position) -> Scrolls {
    if app.apis.form.is_some() || app.catalogue().is_none() {
        return Scrolls::Nothing;
    }
    let [list, _, _] = columns(area);
    if position.x < list.right() {
        Scrolls::ApiList
    } else {
        Scrolls::ApiEntry
    }
}

/// How far down the entry of the API selected can be, of the `scroll` lines asked for:
/// all of them, unless that is past its last line.
#[must_use]
pub fn scroll_of_entry(app: &App, scroll: u16) -> u16 {
    let Some(catalogue) = app.catalogue() else {
        return 0;
    };
    let visible = app.apis.visible(catalogue);
    let Some(api) = visible.get(app.apis.selected.min(visible.len().saturating_sub(1))) else {
        return 0;
    };
    let area = super::content_area(app);
    let [body, _] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let [_, _, detail] = columns(body);
    let width = usize::from(detail.width).saturating_sub(2);
    let lines = cards::api_entry(app, api, width, 1, Some(&app.apis.results)).len();
    entry_scroll(scroll, lines)
}

#[allow(
    clippy::too_many_lines,
    reason = "the form is drawn field by field, top to bottom, as it is read"
)]
fn form_lines(app: &App, form: &Form, width: usize) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let mut out = vec![
        Line::from(vec![
            Span::raw(" "),
            Span::styled(form.api.clone(), theme.strong()),
            Span::styled(" parameters", theme.muted()),
        ]),
        Line::from(""),
    ];
    if form.fields.is_empty() {
        out.push(Line::from(Span::styled(
            " This API takes no parameters. Enter runs it.",
            theme.muted(),
        )));
    }
    for (index, field) in form.fields.iter().enumerate() {
        let focused = index == form.focus;
        let marker = if focused { app.glyphs.prompt } else { " " };
        out.extend(wrap(
            &Line::from(vec![
                Span::styled(format!(" {marker} "), theme.accent()),
                Span::styled(
                    field.name.clone(),
                    if focused {
                        theme.strong()
                    } else {
                        theme.accent()
                    },
                ),
                Span::styled(format!("  {}", field.summary), theme.text()),
                Span::styled(
                    if field.required {
                        "  required"
                    } else {
                        "  optional"
                    },
                    theme.muted(),
                ),
            ]),
            width,
            5,
        ));
        let shown = field.shown();
        let placeholder = match &field.kind {
            FieldKind::Boolean => "Space toggles",
            FieldKind::Choice(_) => "\u{2190} \u{2192} choose",
            FieldKind::Json => "JSON",
            FieldKind::Integer | FieldKind::Number => "a number",
            FieldKind::Text => "text",
        };
        let value = if shown.is_empty() {
            Span::styled(app.glyphs.key_name(placeholder), theme.faint())
        } else {
            span(&shown, theme.text())
        };
        let cursor = if focused && field.takes_text() {
            "_"
        } else {
            ""
        };
        let line = Line::from(vec![
            Span::raw("     "),
            Span::styled("[ ", theme.faint()),
            value,
            Span::styled(cursor, theme.accent()),
            Span::styled(" ]", theme.faint()),
        ]);
        out.push(if focused {
            line.style(theme.selection())
        } else {
            line
        });
        if let Some(description) = &field.description {
            out.extend(wrap(
                &Line::from(vec![Span::raw("     "), span(description, theme.muted())]),
                width,
                5,
            ));
        }
        for (_, message) in form.errors.iter().filter(|(at, _)| *at == Some(index)) {
            out.extend(wrap(
                &Line::from(vec![
                    Span::raw("     "),
                    Span::styled(" error ", theme.badge_error()),
                    Span::raw(" "),
                    span(message, theme.text()),
                ]),
                width,
                13,
            ));
        }
        out.push(Line::from(""));
    }
    for (_, message) in form.errors.iter().filter(|(at, _)| at.is_none()) {
        out.extend(wrap(
            &Line::from(vec![
                Span::raw(" "),
                Span::styled(" error ", theme.badge_error()),
                Span::raw(" "),
                span(message, theme.text()),
            ]),
            width,
            9,
        ));
    }
    if !form.unchecked.is_empty() {
        out.push(Line::from(Span::styled(
            format!(
                " ZENITH did not check {}; SOLAR will.",
                form.unchecked.join(", ")
            ),
            theme.muted(),
        )));
    }
    if let Some(command) = form.command() {
        out.extend(wrap(
            &Line::from(vec![
                Span::styled(" the same as ", theme.muted()),
                span(&command, theme.accent()),
            ]),
            width,
            13,
        ));
    }
    out
}
