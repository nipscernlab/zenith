//! The slash commands: the table `/help` and completion read, and the parser.
//!
//! The first eight commands are the ones the brief names. The other five exist because
//! testing SOLAR by hand needs them, and `docs/DESIGN.md`, section 10, says why each one.

use std::ops::Range;

use zenith_client::schema::closest;

use crate::theme::ThemeName;

/// What a command takes after its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Argument {
    /// Nothing.
    None,
    /// One word, which must be there.
    Word,
    /// One word, which may be absent.
    OptionalWord,
    /// Everything after the name, which must be there.
    Rest,
    /// Everything after the name, which may be absent.
    OptionalRest,
    /// An API name, then optionally the rest.
    ApiThenRest,
}

/// One command, as `/help` and completion show it.
#[derive(Debug, Clone, Copy)]
pub struct Spec {
    /// The name, without its slash.
    pub name: &'static str,
    /// How it is written, for `/help`.
    pub usage: &'static str,
    /// What it does, one line.
    pub summary: &'static str,
    /// What it takes.
    pub argument: Argument,
}

/// Every command, in the order `/help` lists them.
pub const COMMANDS: &[Spec] = &[
    Spec {
        name: "list",
        usage: "/list",
        summary: "every API SOLAR answers to",
        argument: Argument::None,
    },
    Spec {
        name: "describe",
        usage: "/describe <api>",
        summary: "one API: parameters, errors, side effects, examples",
        argument: Argument::Word,
    },
    Spec {
        name: "call",
        usage: "/call <api> [json]",
        summary: "any call, with its parameters as JSON, checked before it is sent",
        argument: Argument::ApiThenRest,
    },
    Spec {
        name: "ping",
        usage: "/ping [message]",
        summary: "the round trip, and SOLAR's own time",
        argument: Argument::OptionalRest,
    },
    Spec {
        name: "version",
        usage: "/version",
        summary: "the versions of SOLAR, its protocol and its manifest, and its build",
        argument: Argument::None,
    },
    Spec {
        name: "theme",
        usage: "/theme [night|light|high-contrast]",
        summary: "the colours; with no name, the next theme",
        argument: Argument::OptionalWord,
    },
    Spec {
        name: "help",
        usage: "/help [command]",
        summary: "the commands, or one of them",
        argument: Argument::OptionalWord,
    },
    Spec {
        name: "quit",
        usage: "/quit",
        summary: "leave ZENITH",
        argument: Argument::None,
    },
    Spec {
        name: "raw",
        usage: "/raw <line>",
        summary: "send a line exactly as typed, unchecked: a batch, a broken envelope",
        argument: Argument::Rest,
    },
    Spec {
        name: "reconnect",
        usage: "/reconnect",
        summary: "restart SOLAR, and pick up a new build of it",
        argument: Argument::None,
    },
    Spec {
        name: "clear",
        usage: "/clear",
        summary: "empty the transcript; the History keeps everything",
        argument: Argument::None,
    },
    Spec {
        name: "forget",
        usage: "/forget",
        summary: "empty the command line history, here and in the file that keeps it",
        argument: Argument::None,
    },
    Spec {
        name: "export",
        usage: "/export [path]",
        summary: "write this connection as a recording for solar replay",
        argument: Argument::OptionalRest,
    },
    Spec {
        name: "report",
        usage: "/report [path]",
        summary: "write one file with everything a bug report needs",
        argument: Argument::OptionalRest,
    },
];

/// The command a name stands for.
#[must_use]
pub fn spec(name: &str) -> Option<&'static Spec> {
    let name = name.strip_prefix('/').unwrap_or(name);
    COMMANDS.iter().find(|spec| spec.name == name)
}

/// A command, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `/list`.
    List,
    /// `/describe <api>`.
    Describe {
        /// The API.
        api: String,
    },
    /// `/call <api> [json]`.
    Call {
        /// The API.
        api: String,
        /// The JSON as typed, empty when there was none.
        json: String,
        /// Where the JSON starts in the line, for underlining an error in it.
        json_start: usize,
    },
    /// `/ping [message]`.
    Ping {
        /// The message.
        message: Option<String>,
    },
    /// `/version`.
    Version,
    /// `/theme [name]`.
    Theme {
        /// The theme, or the next one.
        name: Option<ThemeName>,
    },
    /// `/help [command]`.
    Help {
        /// The command.
        command: Option<&'static Spec>,
    },
    /// `/quit`.
    Quit,
    /// `/raw <line>`.
    Raw {
        /// The line.
        line: String,
    },
    /// `/reconnect`.
    Reconnect,
    /// `/clear`.
    Clear,
    /// `/forget`.
    Forget,
    /// `/export [path]`.
    Export {
        /// The path.
        path: Option<String>,
    },
    /// `/report [path]`.
    Report {
        /// The path.
        path: Option<String>,
    },
}

impl PartialEq for Spec {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Spec {}

/// Why a line is not a command, and which part of it is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// One or two sentences.
    pub message: String,
    /// The bytes of the line the message is about.
    pub span: Option<Range<usize>>,
}

fn error(message: impl Into<String>, span: Option<Range<usize>>) -> ParseError {
    ParseError {
        message: message.into(),
        span,
    }
}

/// Parses one line of the command line. An empty line is `Ok(None)`: nothing to do.
///
/// # Errors
///
/// [`ParseError`] saying what is wrong, with the part of the line to underline: a line
/// without a slash, an unknown command with the closest one, a missing or extra argument.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per command reads as the table of commands it parses"
)]
pub fn parse(line: &str, api_names: &[&str]) -> Result<Option<Command>, ParseError> {
    let start = line.len() - line.trim_start().len();
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let Some(after_slash) = trimmed.strip_prefix('/') else {
        let first_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
        let first = &trimmed[..first_end];
        let span = Some(start..start + first_end);
        if api_names.contains(&first) {
            return Err(error(
                format!("Commands start with a slash. Did you mean /call {first}?"),
                span,
            ));
        }
        return Err(error(
            "Commands start with a slash. /help lists them.",
            span,
        ));
    };
    let name_end = after_slash
        .find(char::is_whitespace)
        .unwrap_or(after_slash.len());
    let name = &after_slash[..name_end];
    let name_span = start..start + 1 + name_end;
    let Some(spec) = spec(name) else {
        let names: Vec<String> = COMMANDS.iter().map(|spec| spec.name.to_owned()).collect();
        let message = match closest(name, &names) {
            Some(close) => format!("There is no command /{name}. Did you mean /{close}?"),
            None => format!("There is no command /{name}. /help lists them."),
        };
        return Err(error(message, Some(name_span)));
    };
    let rest_raw = &after_slash[name_end..];
    let rest = rest_raw.trim();
    let rest_start = start + 1 + name_end + (rest_raw.len() - rest_raw.trim_start().len());
    let rest_span = Some(rest_start..rest_start + rest.len());
    let words: Vec<&str> = rest.split_whitespace().collect();
    match spec.argument {
        Argument::None if !rest.is_empty() => {
            return Err(error(
                format!("/{} takes nothing after it.", spec.name),
                rest_span,
            ));
        }
        Argument::Word | Argument::Rest | Argument::ApiThenRest if rest.is_empty() => {
            return Err(error(
                format!("/{} needs more: {}.", spec.name, spec.usage),
                Some(name_span),
            ));
        }
        Argument::Word | Argument::OptionalWord if words.len() > 1 => {
            return Err(error(
                format!("/{} takes one word: {}.", spec.name, spec.usage),
                rest_span,
            ));
        }
        _ => {}
    }
    let optional = |text: &str| (!text.is_empty()).then(|| text.to_owned());
    let command = match spec.name {
        "list" => Command::List,
        "describe" => Command::Describe {
            api: rest.to_owned(),
        },
        "call" => {
            let api_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let json_raw = &rest[api_end..];
            let json = json_raw.trim_start();
            Command::Call {
                api: rest[..api_end].to_owned(),
                json: json.to_owned(),
                json_start: rest_start + api_end + (json_raw.len() - json.len()),
            }
        }
        "ping" => Command::Ping {
            message: optional(rest),
        },
        "version" => Command::Version,
        "theme" => {
            let name = if rest.is_empty() {
                None
            } else {
                Some(ThemeName::parse(rest).ok_or_else(|| {
                    error(
                        format!(
                            "There is no theme {rest}. The themes are {}.",
                            ThemeName::ALL.map(ThemeName::name).join(", ")
                        ),
                        rest_span.clone(),
                    )
                })?)
            };
            Command::Theme { name }
        }
        "help" => {
            let command = if rest.is_empty() {
                None
            } else {
                Some(spec_or_error(rest, rest_span)?)
            };
            Command::Help { command }
        }
        "quit" => Command::Quit,
        "raw" => Command::Raw {
            line: rest.to_owned(),
        },
        "reconnect" => Command::Reconnect,
        "clear" => Command::Clear,
        "forget" => Command::Forget,
        "export" => Command::Export {
            path: optional(rest),
        },
        _ => Command::Report {
            path: optional(rest),
        },
    };
    Ok(Some(command))
}

fn spec_or_error(name: &str, span: Option<Range<usize>>) -> Result<&'static Spec, ParseError> {
    spec(name).ok_or_else(|| {
        error(
            format!("There is no command {name}. /help lists them."),
            span,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const APIS: [&str; 2] = ["solar.ping", "system.info"];

    fn ok(line: &str) -> Command {
        parse(line, &APIS).unwrap().unwrap()
    }

    fn fails(line: &str) -> ParseError {
        parse(line, &APIS).unwrap_err()
    }

    #[test]
    fn an_empty_line_does_nothing() {
        assert_eq!(parse("   ", &APIS), Ok(None));
    }

    #[test]
    fn a_call_keeps_its_json_and_where_it_starts() {
        let line = "/call solar.ping   {\"message\": \"hi there\"}";
        let Command::Call {
            api,
            json,
            json_start,
        } = ok(line)
        else {
            panic!("not a call");
        };
        assert_eq!(api, "solar.ping");
        assert_eq!(json, "{\"message\": \"hi there\"}");
        assert_eq!(&line[json_start..], json);
        assert_eq!(
            ok("/call system.info"),
            Command::Call {
                api: "system.info".to_owned(),
                json: String::new(),
                json_start: 17
            }
        );
    }

    #[test]
    fn a_line_without_a_slash_is_pointed_at_the_command_it_may_have_meant() {
        let failure = fails("solar.ping {}");
        assert_eq!(
            failure.message,
            "Commands start with a slash. Did you mean /call solar.ping?"
        );
        assert_eq!(failure.span, Some(0..10));
        assert_eq!(
            fails("hello").message,
            "Commands start with a slash. /help lists them."
        );
    }

    #[test]
    fn an_unknown_command_suggests_the_closest_one() {
        let failure = fails("/pign");
        assert_eq!(
            failure.message,
            "There is no command /pign. Did you mean /ping?"
        );
        assert_eq!(failure.span, Some(0..5));
        assert_eq!(
            fails("/zzzzzz").message,
            "There is no command /zzzzzz. /help lists them."
        );
    }

    #[test]
    fn missing_and_extra_arguments_are_refused_with_the_usage() {
        assert_eq!(
            fails("/describe").message,
            "/describe needs more: /describe <api>."
        );
        assert_eq!(fails("/list now").message, "/list takes nothing after it.");
        assert_eq!(fails("/list now").span, Some(6..9));
        assert_eq!(
            fails("/describe a b").message,
            "/describe takes one word: /describe <api>."
        );
    }

    #[test]
    fn the_theme_and_the_help_are_checked_against_what_exists() {
        assert_eq!(
            ok("/theme high-contrast"),
            Command::Theme {
                name: Some(ThemeName::HighContrast)
            }
        );
        assert_eq!(ok("/theme"), Command::Theme { name: None });
        assert!(
            fails("/theme sunny")
                .message
                .contains("night, light, high-contrast")
        );
        assert_eq!(
            ok("/help /call"),
            Command::Help {
                command: spec("call")
            }
        );
    }

    #[test]
    fn the_rest_of_a_line_is_one_argument_where_a_command_takes_the_rest() {
        assert_eq!(
            ok("/ping hello there"),
            Command::Ping {
                message: Some("hello there".to_owned())
            }
        );
        assert_eq!(
            ok("/raw [1, 2]"),
            Command::Raw {
                line: "[1, 2]".to_owned()
            }
        );
        assert_eq!(
            ok("/export my file.ndjson"),
            Command::Export {
                path: Some("my file.ndjson".to_owned())
            }
        );
    }

    #[test]
    fn every_command_of_the_table_parses_from_its_usage() {
        for spec in COMMANDS {
            let line = match spec.argument {
                Argument::None | Argument::OptionalWord | Argument::OptionalRest => {
                    format!("/{}", spec.name)
                }
                Argument::Word | Argument::ApiThenRest | Argument::Rest => {
                    format!("/{} solar.ping", spec.name)
                }
            };
            assert!(parse(&line, &APIS).is_ok(), "{line}");
        }
    }
}
