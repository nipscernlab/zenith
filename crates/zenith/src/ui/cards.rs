//! The cards of the transcript: every entry laid out for a person, and the entry of an
//! API, which the APIs tab and `/describe` share.

use std::collections::HashMap;

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use serde_json::{Value, json};
use zenith_client::envelope::{self, Body, Detail, Envelope, Message, Warning};
use zenith_client::manifest::{Api, Catalogue, Matching};
use zenith_client::pointer::Pointer;
use zenith_client::schema_view::View;

use super::text::{self, clean, pad, span, truncate, wrap};
use super::{json, lockup};
use crate::app::App;
use crate::app::apis::ExampleResult;
use crate::app::history::{CallRecord, Summary};
use crate::app::session::{Entry, Layout, Tone};
use crate::brand;
use crate::clock;
use crate::commands::COMMANDS;
use crate::theme::Theme;

/// Where a card's body starts.
const BODY: usize = 3;

/// The lines of one transcript entry at this width.
#[must_use]
pub fn entry_lines(app: &App, entry: &Entry, width: usize) -> Vec<Line<'static>> {
    let theme = &app.theme;
    match entry {
        Entry::Command { text } => {
            let line = Line::from(vec![
                Span::styled(format!(" {} ", app.glyphs.prompt), theme.accent()),
                span(text, theme.accent()),
            ]);
            wrap(&line, width, BODY)
        }
        Entry::Notice { tone, lines } => notice(app, *tone, lines, width),
        Entry::Help { topic } => help(app, *topic, width),
        Entry::Unexpected { line } => {
            let mut out = vec![Line::from(vec![
                Span::raw(" ".repeat(BODY)),
                Span::styled(" unexpected ", theme.badge_error()),
                Span::styled(
                    "  SOLAR wrote a line no call was waiting for:",
                    theme.text(),
                ),
            ])];
            out.push(Line::from(vec![
                Span::raw(" ".repeat(BODY)),
                Span::styled(
                    truncate(
                        &clean(line),
                        width.saturating_sub(BODY),
                        app.glyphs.ellipsis,
                    ),
                    theme.muted(),
                ),
            ]));
            out
        }
        Entry::Mark { version } => solar_lockup(app, Some(version)),
        Entry::Call { record, layout } => match app.history.calls.get(*record) {
            Some(record) => call(app, record, *layout, width),
            None => vec![Line::from(Span::styled(
                format!(
                    "{}(this call is no longer in the History)",
                    " ".repeat(BODY)
                ),
                theme.muted(),
            ))],
        },
    }
}

fn notice(app: &App, tone: Tone, lines: &[String], width: usize) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let mut out = Vec::new();
    for (index, text) in lines.iter().enumerate() {
        let lead = match (tone, index) {
            (Tone::Error, 0) => vec![
                Span::raw(" ".repeat(BODY)),
                Span::styled(" error ", theme.badge_error()),
                Span::raw("  "),
            ],
            (Tone::Error, _) => vec![Span::raw(" ".repeat(BODY + 9))],
            (Tone::Info, 0) => vec![
                Span::raw(" ".repeat(BODY)),
                Span::styled(app.glyphs.dot.to_owned(), theme.accent()),
                Span::raw(" "),
            ],
            (Tone::Info, _) => vec![Span::raw(" ".repeat(BODY + 2))],
        };
        let hanging = lead.iter().map(|span| text::width(&span.content)).sum();
        let style = if tone == Tone::Error && index == 0 {
            theme.error()
        } else {
            theme.text()
        };
        let mut spans = lead;
        spans.push(span(text, style));
        out.extend(wrap(&Line::from(spans), width, hanging));
    }
    out
}

fn help(app: &App, topic: Option<&str>, width: usize) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let column = COMMANDS
        .iter()
        .map(|spec| text::width(spec.usage))
        .max()
        .unwrap_or(0)
        .min(36);
    let mut out = Vec::new();
    for spec in COMMANDS
        .iter()
        .filter(|spec| topic.is_none_or(|topic| topic == spec.name))
    {
        let line = Line::from(vec![
            Span::raw(" ".repeat(BODY)),
            Span::styled(pad(spec.usage, column), theme.accent()),
            Span::raw("  "),
            Span::styled(spec.summary, theme.text()),
        ]);
        out.extend(wrap(&line, width, BODY + column + 2));
    }
    if topic.is_none() {
        out.push(Line::from(vec![
            Span::raw(" ".repeat(BODY)),
            Span::styled("Tab", theme.key()),
            Span::styled(
                " completes commands, API names and parameters; ",
                theme.muted(),
            ),
            Span::styled("?", theme.key()),
            Span::styled(" on an empty line lists every key.", theme.muted()),
        ]));
    }
    out
}

/// Where a call stands, and what came back.
#[allow(
    clippy::too_many_lines,
    reason = "every way a call can stand is one case of the same card"
)]
fn call(app: &App, record: &CallRecord, layout: Layout, width: usize) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let method = record
        .method
        .clone()
        .unwrap_or_else(|| "(no method)".to_owned());
    if record.waiting() {
        let elapsed = app.now.saturating_duration_since(record.sent);
        let star = app.glyphs.stars[usize::try_from(elapsed.as_millis() / 250 % 4).unwrap_or(0)];
        let budget = app
            .catalogue()
            .and_then(|catalogue| catalogue.api(&method))
            .and_then(|api| api.timeout_ms);
        let mut spans = vec![
            Span::raw(" ".repeat(BODY)),
            Span::styled(star.to_owned(), theme.accent()),
            Span::raw(" "),
            Span::styled(method, theme.accent()),
            Span::styled(
                format!(", waiting {}", clock::latency(elapsed)),
                theme.muted(),
            ),
        ];
        if let Some(budget) = budget.filter(|budget| elapsed.as_millis() > u128::from(*budget)) {
            spans.push(Span::styled(
                format!(", past the budget of {budget} ms SOLAR declares for it"),
                theme.error(),
            ));
        }
        return wrap(&Line::from(spans), width, BODY + 2);
    }
    if let Some(why) = &record.closed {
        let line = Line::from(vec![
            Span::raw(" ".repeat(BODY)),
            Span::styled(" closed ", theme.badge_error()),
            Span::raw("  "),
            Span::styled(method, theme.accent()),
            Span::styled(format!(": {why}"), theme.text()),
        ]);
        return wrap(&line, width, BODY + 10);
    }
    let Some(outcome) = &record.outcome else {
        return Vec::new();
    };
    let Some(line) = &outcome.line else {
        let bytes = match outcome.summary {
            Summary::TooLong { bytes } => bytes,
            _ => 0,
        };
        let message = Line::from(vec![
            Span::raw(" ".repeat(BODY)),
            Span::styled(" error ", theme.badge_error()),
            Span::raw("  "),
            Span::styled(
                format!(
                    "The response to {method} was {bytes} bytes, longer than the 16 MiB ZENITH \
                     keeps, so it was dropped."
                ),
                theme.text(),
            ),
        ]);
        return wrap(&message, width, BODY + 9);
    };
    let facts = Facts {
        method,
        call: record.call,
        round_trip: record.round_trip(),
        by_order: outcome.by_order,
    };
    match envelope::read(line) {
        Message::Single(envelope) => single(app, &envelope, &facts, layout, width),
        Message::Batch(items) => {
            let mut out = vec![Line::from(vec![
                Span::raw(" ".repeat(BODY)),
                Span::styled(" batch ", theme.badge_ok()),
                Span::styled(
                    format!(
                        "  {} {}{}",
                        items.len(),
                        if items.len() == 1 {
                            "response"
                        } else {
                            "responses"
                        },
                        facts.timing(app.glyphs.dot)
                    ),
                    theme.text(),
                ),
            ])];
            for (index, item) in items.iter().enumerate() {
                out.push(Line::from(Span::styled(
                    format!(
                        "{}element {index}, id {}",
                        " ".repeat(BODY),
                        item.id.to_value()
                    ),
                    theme.muted(),
                )));
                let element = Facts {
                    method: item
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.method.clone())
                        .unwrap_or_else(|| "(no method)".to_owned()),
                    call: facts.call,
                    round_trip: None,
                    by_order: false,
                };
                out.extend(single(app, item, &element, Layout::Generic, width));
            }
            out
        }
        Message::NotJson { error } => {
            let mut out = wrap(
                &Line::from(vec![
                    Span::raw(" ".repeat(BODY)),
                    Span::styled(" error ", theme.badge_error()),
                    Span::raw("  "),
                    Span::styled(
                        format!(
                            "SOLAR answered {} with a line that is not JSON: {error}.",
                            facts.method
                        ),
                        theme.text(),
                    ),
                ]),
                width,
                BODY + 9,
            );
            out.push(Line::from(vec![
                Span::raw(" ".repeat(BODY)),
                Span::styled(
                    truncate(
                        &clean(line),
                        width.saturating_sub(BODY),
                        app.glyphs.ellipsis,
                    ),
                    theme.muted(),
                ),
            ]));
            out
        }
    }
}

/// What a card's first line says about a call.
struct Facts {
    method: String,
    call: u64,
    round_trip: Option<std::time::Duration>,
    by_order: bool,
}

impl Facts {
    fn timing(&self, dot: &str) -> String {
        self.round_trip.map_or_else(String::new, |trip| {
            format!(" {dot} {}", clock::latency(trip))
        })
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "a response card is read top to bottom, and so it is written"
)]
fn single(
    app: &App,
    envelope: &Envelope,
    facts: &Facts,
    layout: Layout,
    width: usize,
) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let dot = format!(" {} ", app.glyphs.dot);
    let in_solar = envelope
        .meta
        .as_ref()
        .and_then(|meta| meta.duration_us)
        .map(|micros| format!("{}{} in SOLAR", dot, clock::micros(micros)));
    let api_version = envelope
        .meta
        .as_ref()
        .and_then(|meta| meta.api_version.clone())
        .map_or_else(String::new, |version| format!(" {version}"));
    let mut out = Vec::new();
    match &envelope.body {
        Body::Success { data, warnings } => {
            let mut spans = vec![
                Span::raw(" ".repeat(BODY)),
                Span::styled(" ok ", theme.badge_ok()),
                Span::raw("  "),
            ];
            if layout == Layout::Ping {
                spans.push(Span::styled("pong", theme.strong()));
                if let Some(trip) = facts.round_trip {
                    spans.push(Span::styled(
                        format!(" in {}", clock::latency(trip)),
                        theme.text(),
                    ));
                }
                spans.push(Span::styled(
                    in_solar.clone().unwrap_or_default(),
                    theme.muted(),
                ));
                if let Some(echo) = data.get("echo").filter(|echo| !echo.is_null()) {
                    spans.push(Span::styled(format!("{dot}echo "), theme.muted()));
                    spans.push(json::scalar(echo, theme));
                }
                spans.push(Span::styled(
                    format!("{dot}{}{api_version}{dot}#{}", facts.method, facts.call),
                    theme.muted(),
                ));
            } else {
                spans.push(Span::styled(facts.method.clone(), theme.accent()));
                spans.push(Span::styled(api_version.clone(), theme.muted()));
                spans.push(Span::styled(facts.timing(app.glyphs.dot), theme.text()));
                spans.push(Span::styled(in_solar.unwrap_or_default(), theme.muted()));
                spans.push(Span::styled(format!("{dot}#{}", facts.call), theme.muted()));
            }
            out.extend(wrap(&Line::from(spans), width, BODY + 6));
            match layout {
                Layout::Ping => {}
                Layout::List => out.extend(list(app, data, width)),
                Layout::Describe => out.extend(describe(app, data, width)),
                Layout::Version => {
                    let version = envelope
                        .meta
                        .as_ref()
                        .and_then(|meta| meta.solar_version.as_deref());
                    out.extend(solar_lockup(app, version));
                    out.extend(json::human(data, BODY, width, theme));
                }
                Layout::Generic => out.extend(json::human(data, BODY, width, theme)),
            }
            for warning in warnings {
                out.extend(warning_lines(theme, warning, width));
            }
        }
        Body::Failure(failure) => {
            let status = failure.status.clone().unwrap_or_else(|| "?".to_owned());
            let reason = failure.reason.clone().unwrap_or_else(|| "?".to_owned());
            let spans = vec![
                Span::raw(" ".repeat(BODY)),
                Span::styled(" error ", theme.badge_error()),
                Span::raw("  "),
                Span::styled(status, theme.error()),
                Span::styled(dot.clone(), theme.muted()),
                Span::styled(reason, theme.error()),
                Span::styled(format!("{dot}{}", facts.method), theme.accent()),
                Span::styled(facts.timing(app.glyphs.dot), theme.text()),
                Span::styled(format!("{dot}#{}", facts.call), theme.muted()),
            ];
            out.extend(wrap(&Line::from(spans), width, BODY + 9));
            if let Some(message) = &failure.message {
                let line = Line::from(vec![
                    Span::raw(" ".repeat(BODY)),
                    span(message, theme.text()),
                ]);
                out.extend(wrap(&line, width, BODY));
            }
            for detail in &failure.details {
                out.extend(detail_lines(theme, detail, width));
            }
        }
        Body::Unrecognised => {
            out.push(Line::from(vec![
                Span::raw(" ".repeat(BODY)),
                Span::styled(" error ", theme.badge_error()),
                Span::styled(
                    "  a response that is neither a result nor an error",
                    theme.text(),
                ),
            ]));
        }
    }
    for violation in &envelope.violations {
        let line = Line::from(vec![
            Span::raw(" ".repeat(BODY)),
            Span::styled("contract", theme.error()),
            Span::raw("  "),
            Span::styled(
                format!(
                    "{} Section {} of SOLAR's contract.",
                    violation.what, violation.section
                ),
                theme.text(),
            ),
        ]);
        out.extend(wrap(&line, width, BODY + 10));
    }
    if facts.by_order {
        out.push(Line::from(Span::styled(
            format!(
                "{}matched to this call by order, because its id was null",
                " ".repeat(BODY)
            ),
            theme.muted(),
        )));
    }
    out
}

/// SOLAR's mark, where ZENITH shows SOLAR itself: laid out as SOLAR's `docs/brand`
/// lays it out, with the version of the SOLAR that answered where the brand puts the
/// version, and a blank line above and below.
fn solar_lockup(app: &App, version: Option<&str>) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let mut beside = vec![
        ("SOLAR".to_owned(), theme.strong()),
        (
            "The central API of the Constellation".to_owned(),
            theme.text(),
        ),
        ("NIPS-CERN".to_owned(), theme.muted()),
    ];
    if let Some(version) = version {
        beside.push((clean(version).into_owned(), theme.muted()));
    }
    let mut out = vec![Line::from("")];
    out.extend(lockup::lines(
        &brand::SOLAR,
        app.glyphs.charset,
        BODY,
        &beside,
        theme,
    ));
    out.push(Line::from(""));
    out
}

fn warning_lines(theme: &Theme, warning: &Warning, width: usize) -> Vec<Line<'static>> {
    let line = Line::from(vec![
        Span::raw(" ".repeat(BODY)),
        Span::styled("warning", theme.error()),
        Span::raw("  "),
        span(warning.code.as_deref().unwrap_or("?"), theme.strong()),
        Span::raw("  "),
        span(warning.message.as_deref().unwrap_or_default(), theme.text()),
    ]);
    wrap(&line, width, BODY + 9)
}

fn detail_lines(theme: &Theme, detail: &Detail, width: usize) -> Vec<Line<'static>> {
    let field = detail.field.clone().unwrap_or_else(|| "-".to_owned());
    let field_width = text::width(&field).clamp(6, 14);
    let label_width = 8;
    let mut rows: Vec<(&str, Span<'static>)> = Vec::new();
    if let Some(expected) = &detail.expected {
        rows.push(("expected", span(expected, theme.text())));
    }
    if !(detail.received.is_null() && detail.expected.is_none()) {
        rows.push(("received", json::scalar(&detail.received, theme)));
    }
    if let Some(hint) = &detail.hint {
        rows.push(("hint", span(hint, theme.text())));
    }
    if let Some(docs) = &detail.docs {
        rows.push(("docs", span(docs, theme.muted())));
    }
    let mut out = Vec::new();
    for (index, (label, value)) in rows.into_iter().enumerate() {
        let first = if index == 0 {
            Span::styled(pad(&clean(&field), field_width), theme.accent())
        } else {
            Span::raw(" ".repeat(field_width))
        };
        let line = Line::from(vec![
            Span::raw(" ".repeat(BODY)),
            first,
            Span::raw("  "),
            Span::styled(pad(label, label_width), theme.muted()),
            Span::raw("  "),
            value,
        ]);
        out.extend(wrap(&line, width, BODY + field_width + 2 + label_width + 2));
    }
    out
}

/// The table of `/list`.
fn list(app: &App, data: &Value, width: usize) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let Some(apis) = data.get("apis").and_then(Value::as_array) else {
        return json::human(data, BODY, width, theme);
    };
    let name = |api: &Value| {
        api.get("name")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_owned()
    };
    let column = apis
        .iter()
        .map(|api| text::width(&name(api)))
        .max()
        .unwrap_or(0)
        .min(30);
    let mut out = vec![Line::from(Span::styled(
        format!(
            "{}{} {} {} SOLAR {} {} {} {} manifest {}",
            " ".repeat(BODY),
            apis.len(),
            if apis.len() == 1 { "API" } else { "APIs" },
            app.glyphs.dot,
            data.get("solar_version")
                .and_then(Value::as_str)
                .unwrap_or("?"),
            app.glyphs.dot,
            data.get("protocol").and_then(Value::as_str).unwrap_or("?"),
            app.glyphs.dot,
            data.get("schema_version")
                .and_then(Value::as_str)
                .unwrap_or("?"),
        ),
        theme.muted(),
    ))];
    for api in apis {
        let version = api.get("version").and_then(Value::as_str).unwrap_or("?");
        let summary = api
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let line = Line::from(vec![
            Span::raw(" ".repeat(BODY)),
            Span::styled(pad(&clean(&name(api)), column), theme.accent()),
            Span::raw("  "),
            Span::styled(pad(&clean(version), 8), theme.muted()),
            span(summary, theme.text()),
        ]);
        out.extend(wrap(&line, width, BODY + column + 10));
    }
    out
}

/// The entry of `/describe`, laid out like the APIs tab.
fn describe(app: &App, data: &Value, width: usize) -> Vec<Line<'static>> {
    let definitions = app
        .catalogue()
        .map_or_else(|| json!({}), |catalogue| catalogue.definitions.clone());
    let document = json!({"schema_version": "2.0.0", "$defs": definitions, "apis": [data]});
    match Catalogue::from_data(&document) {
        Ok(catalogue) => match catalogue.apis.first() {
            Some(api) => api_entry(app, api, width, BODY, None::<&HashMap<_, _>>),
            None => json::human(data, BODY, width, &app.theme),
        },
        Err(_) => json::human(data, BODY, width, &app.theme),
    }
}

/// Everything the manifest says about one API, as the APIs tab and `/describe` show it.
#[allow(
    clippy::too_many_lines,
    reason = "the sections of an entry are read top to bottom, and so they are written"
)]
#[must_use]
pub fn api_entry(
    app: &App,
    api: &Api,
    width: usize,
    indent: usize,
    results: Option<&HashMap<(String, usize), ExampleResult, impl std::hash::BuildHasher>>,
) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let dot = format!(" {} ", app.glyphs.dot);
    let lead = " ".repeat(indent);
    let heading = |title: &str| {
        Line::from(vec![
            Span::raw(lead.clone()),
            Span::styled(title.to_owned(), theme.strong()),
        ])
    };
    let mut out = Vec::new();
    let mut first = vec![
        Span::raw(lead.clone()),
        Span::styled(api.name.clone(), theme.strong()),
    ];
    if let Some(version) = &api.version {
        first.push(Span::styled(format!(" {version}"), theme.text()));
    }
    if let Some(stability) = &api.stability {
        first.push(Span::styled(format!("{dot}{stability}"), theme.muted()));
    }
    if let Some(since) = &api.since {
        first.push(Span::styled(format!("{dot}since {since}"), theme.muted()));
    }
    out.extend(wrap(&Line::from(first), width, indent));
    if let Some(summary) = &api.summary {
        out.extend(wrap(
            &Line::from(vec![Span::raw(lead.clone()), span(summary, theme.text())]),
            width,
            indent,
        ));
    }
    let effects = if api.side_effects.is_empty() {
        "not stated".to_owned()
    } else {
        api.side_effects.join(", ")
    };
    let idempotent = match api.idempotent {
        Some(true) => "idempotent",
        Some(false) => "not idempotent",
        None => "idempotence not stated",
    };
    let budget = api.timeout_ms.map_or_else(
        || "no budget stated".to_owned(),
        |budget| format!("budget {budget} ms"),
    );
    let mut declared = vec![
        vec![
            Span::styled("side effects ", theme.muted()),
            span(&effects, theme.text()),
        ],
        vec![Span::styled(idempotent, theme.muted())],
        vec![Span::styled(budget, theme.muted())],
    ];
    if let Some(bytes) = api.max_output_bytes {
        let largest = format!("responses up to {}", size(bytes));
        declared.push(vec![Span::styled(largest, theme.muted())]);
    }
    let separator = Span::styled(dot.clone(), theme.muted());
    out.extend(text::facts(&lead, declared, &separator, width, indent));

    out.push(Line::from(""));
    out.push(heading("parameters"));
    let view = View::new(api.params());
    let properties = view.properties(&view.at(&Pointer::root()));
    if properties.is_empty() {
        out.push(Line::from(Span::styled(
            format!("{lead}  none"),
            theme.muted(),
        )));
    }
    let column = properties
        .iter()
        .map(|property| text::width(&property.name))
        .max()
        .unwrap_or(0)
        .min(20);
    for property in &properties {
        let description = view.describe(&property.schemas);
        out.extend(wrap(
            &Line::from(vec![
                Span::raw(format!("{lead}  ")),
                Span::styled(pad(&clean(&property.name), column), theme.accent()),
                Span::raw("  "),
                span(&description.summary(), theme.text()),
                Span::styled(
                    if property.required {
                        "  required"
                    } else {
                        "  optional"
                    },
                    theme.muted(),
                ),
            ]),
            width,
            indent + column + 4,
        ));
        if let Some(text) = &description.description {
            out.extend(wrap(
                &Line::from(vec![
                    Span::raw(" ".repeat(indent + column + 4)),
                    span(text, theme.muted()),
                ]),
                width,
                indent + column + 4,
            ));
        }
    }

    out.push(heading("errors"));
    if api.errors.is_empty() {
        out.push(Line::from(Span::styled(
            format!("{lead}  none beyond those of dispatch"),
            theme.muted(),
        )));
    }
    for error in &api.errors {
        out.push(Line::from(vec![
            Span::raw(format!("{lead}  ")),
            span(&error.status, theme.text()),
            Span::styled(" / ", theme.muted()),
            span(&error.reason, theme.text()),
        ]));
    }

    out.push(heading("examples"));
    if api.examples.is_empty() {
        out.push(Line::from(Span::styled(
            format!("{lead}  none"),
            theme.error(),
        )));
    }
    let name_column = api
        .examples
        .iter()
        .map(|example| text::width(&example.name))
        .max()
        .unwrap_or(0)
        .min(20);
    for (index, example) in api.examples.iter().enumerate() {
        let key = if index < 9 {
            format!("{}", index + 1)
        } else {
            " ".to_owned()
        };
        out.extend(wrap(
            &Line::from(vec![
                Span::raw(format!("{lead}  ")),
                Span::styled(key, theme.key()),
                Span::raw(" "),
                Span::styled(pad(&clean(&example.name), name_column), theme.accent()),
                Span::raw("  "),
                span(&example.params.to_string(), theme.text()),
            ]),
            width,
            indent + name_column + 6,
        ));
        let rule = match &example.matching {
            Matching::Exact => "exact".to_owned(),
            Matching::Subset => "subset".to_owned(),
            Matching::Unknown(rule) => format!("{rule:?}"),
        };
        let mut about = vec![Span::raw(" ".repeat(indent + name_column + 6))];
        if let Some(description) = &example.description {
            about.push(span(description, theme.muted()));
            about.push(Span::styled(
                format!("{dot}compared by {rule}"),
                theme.muted(),
            ));
        } else {
            about.push(Span::styled(format!("compared by {rule}"), theme.muted()));
        }
        out.extend(wrap(&Line::from(about), width, indent + name_column + 6));
        if let Some(result) = results.and_then(|results| results.get(&(api.name.clone(), index))) {
            let (words, style) = match result {
                ExampleResult::Running => ("running".to_owned(), theme.muted()),
                ExampleResult::Matches { round_trip } => (
                    format!("matches the example{dot}{}", clock::latency(*round_trip)),
                    theme.accent(),
                ),
                ExampleResult::Differs { sentence } => (sentence.clone(), theme.error()),
                ExampleResult::Failed { status, reason } => {
                    (format!("SOLAR answered {status} / {reason}"), theme.error())
                }
                ExampleResult::Closed { why } => (why.clone(), theme.error()),
            };
            out.extend(wrap(
                &Line::from(vec![
                    Span::raw(" ".repeat(indent + name_column + 6)),
                    Span::styled(clean(&words).into_owned(), style),
                ]),
                width,
                indent + name_column + 6,
            ));
        }
    }

    let output = View::new(api.output());
    let returned = output.properties(&output.at(&Pointer::root()));
    if !returned.is_empty() {
        out.push(heading("returns"));
        let column = returned
            .iter()
            .map(|property| text::width(&property.name))
            .max()
            .unwrap_or(0)
            .min(20);
        for property in &returned {
            let description = output.describe(&property.schemas);
            out.extend(wrap(
                &Line::from(vec![
                    Span::raw(format!("{lead}  ")),
                    Span::styled(pad(&clean(&property.name), column), theme.accent()),
                    Span::raw("  "),
                    span(&description.summary(), theme.text()),
                ]),
                width,
                indent + column + 4,
            ));
        }
    }

    if let Some(description) = &api.description {
        out.push(heading("description"));
        for paragraph in description.split("\n\n") {
            let flowing = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
            out.extend(wrap(
                &Line::from(vec![
                    Span::raw(format!("{lead}  ")),
                    span(&flowing, theme.text()),
                ]),
                width,
                indent + 2,
            ));
        }
    }
    if !api.other.is_empty() {
        out.push(heading("more, which this ZENITH does not know"));
        for (key, value) in &api.other {
            out.extend(wrap(
                &Line::from(vec![
                    Span::raw(format!("{lead}  ")),
                    span(key, theme.accent()),
                    Span::raw("  "),
                    span(&value.to_string(), theme.text()),
                ]),
                width,
                indent + 4,
            ));
        }
    }
    if !api.missing.is_empty() {
        out.extend(wrap(
            &Line::from(vec![
                Span::raw(lead.clone()),
                Span::styled("contract", theme.error()),
                Span::styled(
                    format!(
                        "  the entry has no {}; section 8 of SOLAR's contract lists them all.",
                        api.missing.join(", ")
                    ),
                    theme.text(),
                ),
            ]),
            width,
            indent + 10,
        ));
    }
    out
}

/// A size in bytes as a person reads it: in whole mebibytes or kibibytes when it is one.
fn size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    match bytes {
        1 => "1 byte".to_owned(),
        _ if bytes >= MIB && bytes.is_multiple_of(MIB) => format!("{} MiB", bytes / MIB),
        _ if bytes >= KIB && bytes.is_multiple_of(KIB) => format!("{} KiB", bytes / KIB),
        _ => format!("{bytes} bytes"),
    }
}

/// The style of an outcome in the lists.
#[must_use]
pub fn outcome_style(theme: &Theme, summary: &Summary) -> Style {
    if summary.is_ok() {
        theme.text()
    } else {
        theme.error()
    }
}

#[cfg(test)]
mod tests {
    use super::size;

    #[test]
    fn a_size_is_said_in_the_largest_whole_unit() {
        assert_eq!(size(8_388_608), "8 MiB");
        assert_eq!(size(65_536), "64 KiB");
        assert_eq!(size(1_000_000), "1000000 bytes");
        assert_eq!(size(1), "1 byte");
        assert_eq!(size(0), "0 bytes");
    }
}
