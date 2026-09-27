//! What `Tab` offers, from where the cursor is and what the manifest says.
//!
//! Commands come from the command table; API names, parameter names and parameter values
//! come from the catalogue, so a new SOLAR API completes the day it appears. Inside the
//! JSON of a `/call`, the place of the cursor is found by the tolerant scanner of
//! `zenith_client::json_text`, and the schema is followed to that place.

use std::ops::Range;

use zenith_client::json_text::{Position, position_at};
use zenith_client::manifest::Catalogue;
use zenith_client::pointer::Pointer;
use zenith_client::schema_view::View;

use crate::commands::{Argument, COMMANDS, spec};
use crate::theme::ThemeName;

/// One thing `Tab` can insert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// What replaces the range.
    pub insert: String,
    /// The name the menu shows.
    pub label: String,
    /// What the menu says beside it.
    pub detail: String,
}

/// The candidates at the cursor, and the part of the line a candidate replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// The bytes of the line that a candidate replaces.
    pub replace: Range<usize>,
    /// The candidates, best first.
    pub candidates: Vec<Candidate>,
}

/// The completion at `cursor`, a byte offset into `line`, if there is anything to offer.
#[must_use]
pub fn complete(line: &str, cursor: usize, catalogue: Option<&Catalogue>) -> Option<Completion> {
    let cursor = cursor.min(line.len());
    let before = &line[..cursor];
    let lead = before.len() - before.trim_start().len();
    let typed = &before[lead..];
    let rest = typed.strip_prefix('/')?;
    let completion = match rest.find(char::is_whitespace) {
        None => commands(line, lead, cursor),
        Some(name_end) => {
            let name = &rest[..name_end];
            let argument_start = lead + 1 + name_end;
            arguments(line, name, argument_start, cursor, catalogue)?
        }
    };
    // Nothing to offer when the one candidate is what is already there.
    let already = &line[completion.replace.clone()];
    let useful: Vec<Candidate> = completion
        .candidates
        .into_iter()
        .filter(|candidate| candidate.insert.trim_end() != already.trim_end())
        .collect();
    (!useful.is_empty()).then_some(Completion {
        replace: completion.replace,
        candidates: useful,
    })
}

fn word_end(line: &str, from: usize) -> usize {
    line[from..]
        .find(char::is_whitespace)
        .map_or(line.len(), |offset| from + offset)
}

fn commands(line: &str, slash: usize, cursor: usize) -> Completion {
    let prefix = &line[slash..cursor];
    let candidates = COMMANDS
        .iter()
        .filter(|spec| format!("/{}", spec.name).starts_with(prefix))
        .map(|spec| Candidate {
            insert: if spec.argument == Argument::None {
                format!("/{}", spec.name)
            } else {
                format!("/{} ", spec.name)
            },
            label: spec.usage.to_owned(),
            detail: spec.summary.to_owned(),
        })
        .collect();
    Completion {
        replace: slash..word_end(line, cursor),
        candidates,
    }
}

fn arguments(
    line: &str,
    name: &str,
    argument_start: usize,
    cursor: usize,
    catalogue: Option<&Catalogue>,
) -> Option<Completion> {
    let spec = spec(name)?;
    let argument = &line[argument_start..cursor];
    let word_start = argument_start + (argument.len() - argument.trim_start().len());
    let first_word = &line[word_start..cursor];
    let in_first_word = !first_word.contains(char::is_whitespace);
    match spec.name {
        "describe" | "call" if in_first_word => {
            let catalogue = catalogue?;
            let trailing = if spec.name == "call" { " " } else { "" };
            Some(Completion {
                replace: word_start..word_end(line, cursor),
                candidates: api_names(catalogue, first_word, trailing),
            })
        }
        "call" => {
            let catalogue = catalogue?;
            let api_end = word_end(line, word_start);
            let api = catalogue.api(&line[word_start..api_end])?;
            let json_start = api_end + (line[api_end..].len() - line[api_end..].trim_start().len());
            if cursor < json_start {
                return None;
            }
            let json = &line[json_start..];
            parameters(api, json, cursor - json_start, json_start)
        }
        "theme" if in_first_word => Some(Completion {
            replace: word_start..word_end(line, cursor),
            candidates: ThemeName::ALL
                .iter()
                .filter(|theme| theme.name().starts_with(first_word))
                .map(|theme| Candidate {
                    insert: theme.name().to_owned(),
                    label: theme.name().to_owned(),
                    detail: String::new(),
                })
                .collect(),
        }),
        "help" if in_first_word => {
            let prefix = first_word.trim_start_matches('/');
            Some(Completion {
                replace: word_start..word_end(line, cursor),
                candidates: COMMANDS
                    .iter()
                    .filter(|spec| spec.name.starts_with(prefix))
                    .map(|spec| Candidate {
                        insert: spec.name.to_owned(),
                        label: spec.usage.to_owned(),
                        detail: spec.summary.to_owned(),
                    })
                    .collect(),
            })
        }
        _ => None,
    }
}

/// The API names that start with what was typed, then those that contain it, so that
/// `ping` finds `solar.ping`.
fn api_names(catalogue: &Catalogue, typed: &str, trailing: &str) -> Vec<Candidate> {
    let candidate = |api: &zenith_client::manifest::Api| Candidate {
        insert: format!("{}{trailing}", api.name),
        label: api.name.clone(),
        detail: api.summary.clone().unwrap_or_default(),
    };
    let mut found: Vec<Candidate> = catalogue
        .apis
        .iter()
        .filter(|api| api.name.starts_with(typed))
        .map(candidate)
        .collect();
    found.extend(
        catalogue
            .apis
            .iter()
            .filter(|api| !api.name.starts_with(typed) && api.name.contains(typed))
            .map(candidate),
    );
    found
}

fn parameters(
    api: &zenith_client::manifest::Api,
    json: &str,
    cursor: usize,
    offset: usize,
) -> Option<Completion> {
    let view = View::new(api.params());
    match position_at(json, cursor) {
        Position::Nothing => {
            let candidates = keys(view, &Pointer::root(), "", &[], true);
            Some(Completion {
                replace: offset + cursor..offset + cursor,
                candidates,
            })
        }
        Position::Key {
            object,
            prefix,
            replace,
            present,
        } => Some(Completion {
            replace: offset + replace.start..offset + replace.end,
            candidates: keys(view, &object, &prefix, &present, false),
        }),
        Position::Value {
            at,
            prefix,
            replace,
        } => {
            let place = view.at(&at);
            let candidates = view
                .value_candidates(&place)
                .into_iter()
                .filter(|candidate| candidate.text.starts_with(&prefix))
                .map(|candidate| Candidate {
                    insert: candidate.text.clone(),
                    label: candidate.text,
                    detail: candidate.description.unwrap_or_default(),
                })
                .collect();
            Some(Completion {
                replace: offset + replace.start..offset + replace.end,
                candidates,
            })
        }
        Position::Elsewhere => None,
    }
}

fn keys(
    view: View<'_>,
    object: &Pointer,
    prefix: &str,
    present: &[String],
    open_object: bool,
) -> Vec<Candidate> {
    let place = view.at(object);
    view.properties(&place)
        .into_iter()
        .filter(|property| property.name.starts_with(prefix) && !present.contains(&property.name))
        .map(|property| {
            let description = view.describe(&property.schemas);
            let mut detail = description.summary();
            detail.push_str(if property.required {
                ", required"
            } else {
                ", optional"
            });
            if let Some(text) = &description.description {
                detail.push_str(". ");
                detail.push_str(text.lines().next().unwrap_or_default());
            }
            let key = serde_json::Value::String(property.name.clone()).to_string();
            Candidate {
                insert: if open_object {
                    format!("{{{key}: ")
                } else {
                    format!("{key}: ")
                },
                label: property.name,
                detail,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> Catalogue {
        let text = include_str!("../../zenith-client/tests/fixtures/solar-0.1.0-manifest.json");
        Catalogue::from_data(&serde_json::from_str(text).unwrap()).unwrap()
    }

    fn inserts(line: &str) -> Vec<String> {
        complete(line, line.len(), Some(&catalogue()))
            .map(|completion| {
                completion
                    .candidates
                    .into_iter()
                    .map(|candidate| candidate.insert)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_slash_offers_every_command_and_a_prefix_narrows_them() {
        assert_eq!(inserts("/").len(), COMMANDS.len());
        assert_eq!(inserts("/re"), vec!["/reconnect", "/report "]);
        assert_eq!(inserts("/desc"), vec!["/describe "]);
    }

    #[test]
    fn a_command_typed_in_full_offers_nothing() {
        assert!(inserts("/list").is_empty());
    }

    #[test]
    fn the_argument_of_describe_and_call_is_an_api_name_from_the_manifest() {
        assert_eq!(inserts("/describe solar.p"), vec!["solar.ping"]);
        assert_eq!(inserts("/call sys"), vec!["system.info "]);
        assert_eq!(inserts("/describe ping"), vec!["solar.ping"]);
    }

    #[test]
    fn inside_the_json_the_parameters_of_the_schema_are_offered() {
        assert_eq!(inserts("/call solar.ping "), vec!["{\"message\": "]);
        assert_eq!(inserts("/call solar.ping {"), vec!["\"message\": "]);
        assert_eq!(inserts("/call solar.ping {\"me"), vec!["\"message\": "]);
        assert!(inserts("/call solar.ping {\"message\": \"x\", ").is_empty());
        assert_eq!(inserts("/call solar.describe {"), vec!["\"api\": "]);
    }

    #[test]
    fn the_detail_says_the_type_whether_it_is_required_and_what_it_is_for() {
        let line = "/call solar.describe {";
        let completion = complete(line, line.len(), Some(&catalogue())).unwrap();
        let detail = &completion.candidates[0].detail;
        assert!(detail.starts_with("string, required."), "{detail}");
    }

    #[test]
    fn at_a_value_the_values_the_schema_allows_are_offered() {
        assert_eq!(inserts("/call solar.ping {\"message\": "), vec!["null"]);
        assert_eq!(inserts("/call solar.ping {\"message\": n"), vec!["null"]);
    }

    #[test]
    fn the_replaced_range_covers_the_partial_word() {
        let line = "/call solar.ping {\"mes";
        let completion = complete(line, line.len(), Some(&catalogue())).unwrap();
        assert_eq!(&line[completion.replace], "\"mes");
    }

    #[test]
    fn themes_and_help_topics_complete() {
        assert_eq!(inserts("/theme h"), vec!["high-contrast"]);
        assert_eq!(inserts("/help ca"), vec!["call"]);
    }

    #[test]
    fn with_no_catalogue_only_commands_complete() {
        assert!(complete("/describe s", 11, None).is_none());
        assert!(complete("/pi", 3, None).is_some());
    }

    #[test]
    fn a_line_without_a_slash_offers_nothing() {
        assert!(inserts("solar.ping").is_empty());
    }
}
