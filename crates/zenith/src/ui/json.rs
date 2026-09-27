//! JSON on screen, two ways: laid out for a person in the transcript, and indented
//! exactly in the envelope viewer.

use ratatui::text::{Line, Span};
use serde_json::Value;

use super::text::{self, clean};
use crate::theme::Theme;

/// A value as indented JSON, one line per member, coloured by kind. Strings keep their
/// JSON escapes, so what is drawn is what crossed the pipe.
#[must_use]
pub fn pretty(value: &Value, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    pretty_into(value, theme, 0, &mut Vec::new(), &mut lines, false);
    lines
}

fn pretty_into(
    value: &Value,
    theme: &Theme,
    indent: usize,
    prefix: &mut Vec<Span<'static>>,
    lines: &mut Vec<Line<'static>>,
    comma: bool,
) {
    let tail = if comma { "," } else { "" };
    let pad = " ".repeat(indent);
    match value {
        Value::Object(map) if !map.is_empty() => {
            let mut first = std::mem::take(prefix);
            first.insert(0, Span::raw(pad.clone()));
            first.push(Span::styled("{", theme.json_quiet()));
            lines.push(Line::from(first));
            let count = map.len();
            for (index, (key, member)) in map.iter().enumerate() {
                let mut key_prefix = vec![
                    Span::styled(Value::String(key.clone()).to_string(), theme.json_key()),
                    Span::styled(": ", theme.json_quiet()),
                ];
                pretty_into(
                    member,
                    theme,
                    indent + 2,
                    &mut key_prefix,
                    lines,
                    index + 1 < count,
                );
            }
            lines.push(Line::from(vec![
                Span::raw(pad),
                Span::styled(format!("}}{tail}"), theme.json_quiet()),
            ]));
        }
        Value::Array(items) if !items.is_empty() => {
            let mut first = std::mem::take(prefix);
            first.insert(0, Span::raw(pad.clone()));
            first.push(Span::styled("[", theme.json_quiet()));
            lines.push(Line::from(first));
            let count = items.len();
            for (index, item) in items.iter().enumerate() {
                pretty_into(
                    item,
                    theme,
                    indent + 2,
                    &mut Vec::new(),
                    lines,
                    index + 1 < count,
                );
            }
            lines.push(Line::from(vec![
                Span::raw(pad),
                Span::styled(format!("]{tail}"), theme.json_quiet()),
            ]));
        }
        scalar => {
            let mut line = std::mem::take(prefix);
            line.insert(0, Span::raw(pad));
            line.push(Span::styled(
                scalar.to_string(),
                scalar_style(scalar, theme),
            ));
            if comma {
                line.push(Span::styled(",", theme.json_quiet()));
            }
            lines.push(Line::from(line));
        }
    }
}

fn scalar_style(value: &Value, theme: &Theme) -> ratatui::style::Style {
    match value {
        Value::String(_) => theme.json_string(),
        Value::Null => theme.json_quiet(),
        _ => theme.json_scalar(),
    }
}

/// A scalar as the transcript writes it: a string as it is, unless it would read as
/// another kind of value, `null` dimmed, the rest as JSON.
#[must_use]
pub fn scalar(value: &Value, theme: &Theme) -> Span<'static> {
    match value {
        Value::String(text) => {
            let ambiguous = text.is_empty()
                || text.trim() != text
                || matches!(text.as_str(), "null" | "true" | "false")
                || text.parse::<f64>().is_ok();
            if ambiguous {
                Span::styled(Value::String(text.clone()).to_string(), theme.json_string())
            } else {
                Span::styled(clean(text).into_owned(), theme.json_string())
            }
        }
        Value::Null => Span::styled("null", theme.json_quiet()),
        Value::Array(items) if items.is_empty() => Span::styled("[]", theme.json_quiet()),
        Value::Object(map) if map.is_empty() => Span::styled("{}", theme.json_quiet()),
        other => Span::styled(clean(&other.to_string()).into_owned(), theme.json_scalar()),
    }
}

/// Whether a value is drawn on one line: a scalar, an empty container, or an array of
/// scalars short enough to fit.
fn inline(value: &Value, room: usize) -> bool {
    match value {
        Value::Array(items) => {
            items
                .iter()
                .all(|item| !item.is_object() && !item.is_array())
                && text::width(&value.to_string()) <= room
        }
        Value::Object(map) => map.is_empty(),
        _ => true,
    }
}

/// A value laid out for a person: members as aligned keys and values, nested objects
/// indented under their key, arrays of scalars on one line when they fit, long strings
/// wrapped in their column.
#[must_use]
pub fn human(value: &Value, indent: usize, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    human_into(value, indent, width, theme, &mut lines);
    lines
}

fn human_into(
    value: &Value,
    indent: usize,
    width: usize,
    theme: &Theme,
    lines: &mut Vec<Line<'static>>,
) {
    let pad = " ".repeat(indent);
    match value {
        Value::Object(map) if !map.is_empty() => {
            // The keys' column is as wide as the longest key, up to a third of the line.
            // A key longer than that has a line of its own, with its value under the
            // column, and is never cut.
            let most = (width.saturating_sub(indent) / 3).max(22);
            let key_width = map
                .keys()
                .map(|key| text::width(key))
                .max()
                .unwrap_or(0)
                .min(most);
            for (key, member) in map {
                let room = width.saturating_sub(indent + key_width + 2);
                if inline(member, room) {
                    let rendered = match member {
                        Value::Array(_) => Span::styled(member.to_string(), theme.json_scalar()),
                        other => scalar(other, theme),
                    };
                    let key = clean(key);
                    let line = if text::width(&key) > key_width {
                        lines.push(Line::from(vec![
                            Span::raw(pad.clone()),
                            Span::styled(key.into_owned(), theme.json_key()),
                        ]));
                        Line::from(vec![
                            Span::raw(" ".repeat(indent + key_width + 2)),
                            rendered,
                        ])
                    } else {
                        Line::from(vec![
                            Span::raw(pad.clone()),
                            Span::styled(text::pad(&key, key_width), theme.json_key()),
                            Span::raw("  "),
                            rendered,
                        ])
                    };
                    lines.extend(text::wrap(&line, width, indent + key_width + 2));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw(pad.clone()),
                        Span::styled(clean(key).into_owned(), theme.json_key()),
                    ]));
                    human_into(member, indent + 2, width, theme, lines);
                }
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (index, item) in items.iter().enumerate() {
                if inline(item, width.saturating_sub(indent + 2)) {
                    let line = Line::from(vec![
                        Span::raw(pad.clone()),
                        Span::styled("- ", theme.json_quiet()),
                        scalar(item, theme),
                    ]);
                    lines.extend(text::wrap(&line, width, indent + 2));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw(pad.clone()),
                        Span::styled(format!("[{index}]"), theme.json_quiet()),
                    ]));
                    human_into(item, indent + 2, width, theme, lines);
                }
            }
        }
        scalar_value => {
            let line = Line::from(vec![Span::raw(pad), scalar(scalar_value, theme)]);
            lines.extend(text::wrap(&line, width, indent));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Depth, ThemeName};
    use serde_json::json;

    fn theme() -> Theme {
        Theme::new(ThemeName::Night, Depth::None)
    }

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
    fn pretty_json_is_indented_with_its_commas() {
        let value = json!({"a": [1, {"b": null}], "c": "x"});
        assert_eq!(
            texts(&pretty(&value, &theme())),
            vec![
                "{",
                "  \"a\": [",
                "    1,",
                "    {",
                "      \"b\": null",
                "    }",
                "  ],",
                "  \"c\": \"x\"",
                "}",
            ]
        );
    }

    #[test]
    fn a_person_reads_aligned_keys_and_values() {
        let value = json!({"echo": "hi", "pong": true, "received_at": "2026-09-27T15:47:00Z"});
        assert_eq!(
            texts(&human(&value, 3, 80, &theme())),
            vec![
                "   echo         hi",
                "   pong         true",
                "   received_at  2026-09-27T15:47:00Z",
            ]
        );
    }

    #[test]
    fn nested_objects_are_indented_under_their_key_and_short_arrays_stay_inline() {
        let value = json!({"build": {"profile": "release"}, "tags": ["a", "b"], "empty": {}});
        assert_eq!(
            texts(&human(&value, 0, 80, &theme())),
            vec![
                "build",
                "  profile  release",
                "tags   [\"a\",\"b\"]",
                "empty  {}"
            ]
        );
    }

    #[test]
    fn a_string_that_would_read_as_another_kind_keeps_its_quotes() {
        let value = json!({"a": "null", "b": "", "c": "12", "d": null});
        assert_eq!(
            texts(&human(&value, 0, 80, &theme())),
            vec!["a  \"null\"", "b  \"\"", "c  \"12\"", "d  null"]
        );
    }

    #[test]
    fn a_long_string_wraps_in_its_column() {
        let value = json!({"description": "one two three four five six"});
        let lines = texts(&human(&value, 0, 24, &theme()));
        assert_eq!(lines[0], "description  one two");
        assert!(lines[1].starts_with("             three"));
    }

    #[test]
    fn no_key_is_ever_cut() {
        // Found by the walkthrough: `/version` once showed manifest_schema_versio.
        let value = json!({"manifest_schema_version": "2.0.0", "protocol": "solar/1"});
        assert_eq!(
            texts(&human(&value, 3, 100, &theme())),
            vec![
                "   manifest_schema_version  2.0.0",
                "   protocol                 solar/1"
            ]
        );
        let long = "a_key_longer_than_a_third_of_the_line";
        let lines = texts(&human(&json!({ long: 1, "b": 2 }), 0, 60, &theme()));
        assert_eq!(lines[0], long);
        assert_eq!(lines[1], format!("{}1", " ".repeat(24)));
        assert_eq!(lines[2], format!("b{}2", " ".repeat(23)));
    }
}
