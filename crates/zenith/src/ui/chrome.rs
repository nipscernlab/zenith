//! The header, with the tabs, and the status bar, which draws SOLAR as a body on its
//! orbit.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::text;
use crate::app::link::Phase;
use crate::app::session::Tone;
use crate::app::{App, Tab};
use crate::clock;

/// A number from a position and a seed that looks random and is always the same, so that
/// the stars of the header are the same on every run and in every snapshot.
#[must_use]
pub fn scatter(position: u64, seed: u64) -> u64 {
    // SplitMix64.
    let mut z = position
        .wrapping_add(seed)
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// One star in how many cells of the sky.
const DENSITY: u64 = 29;

/// The sky: stars in the cells of `area` that no rect of `keep_out` covers. Where each star
/// is depends only on its cell and on the screen's width, so the Session tab's sky has the
/// stars the opening had, in the same cells. The stars twinkle with `frame`, which only the
/// opening advances; everywhere else they are still, and an idle ZENITH stays idle.
pub fn sky(
    buffer: &mut Buffer,
    screen_width: u16,
    area: Rect,
    keep_out: &[Rect],
    frame: u64,
    app: &App,
) {
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if keep_out
                .iter()
                .any(|rect| rect.height > 0 && rect.contains(Position::new(x, y)))
            {
                continue;
            }
            let position = u64::from(y) * u64::from(screen_width) + u64::from(x);
            let value = scatter(position, 0x00C0_FFEE);
            if !value.is_multiple_of(DENSITY) {
                continue;
            }
            let twinkles = (value >> 24).is_multiple_of(3);
            let phase = (value >> 16).wrapping_add(if twinkles { frame } else { 0 });
            let bright = phase % 6 < 2;
            let glyph = app.glyphs.stars[usize::try_from((value >> 8) % 4).unwrap_or(0)];
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_symbol(glyph).set_style(app.theme.star(bright));
            }
        }
    }
}

/// The header: the name, the tabs with their numbers, stars in the space left, `? keys`.
pub fn header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let mut spans = vec![Span::styled(" ZENITH ", theme.strong())];
    for (index, tab) in Tab::ALL.iter().enumerate() {
        spans.push(Span::styled("  ", theme.base()));
        let label = format!("{} {}", index + 1, tab.title());
        let style = if *tab == app.tab {
            theme.tab_active()
        } else {
            theme.tab_inactive()
        };
        spans.push(Span::styled(label, style));
    }
    let hint = vec![
        Span::styled("?", theme.key()),
        Span::styled(" keys ", theme.muted()),
    ];
    let used: usize = spans
        .iter()
        .chain(hint.iter())
        .map(|span| text::width(&span.content))
        .sum();
    let gap = usize::from(area.width).saturating_sub(used);
    // Two cells at each end of the gap are always blank, so that no star touches a tab or
    // the hint.
    let sky = 2..gap.saturating_sub(2);
    let mut stars = String::new();
    for column in 0..gap {
        let value = scatter(column as u64, 0x5A7E_11E5);
        let star = if sky.contains(&column) && value.is_multiple_of(7) {
            app.glyphs.stars[usize::try_from(value >> 8).unwrap_or(0) % 3]
        } else {
            " "
        };
        stars.push_str(star);
    }
    spans.push(Span::styled(stars, theme.faint()));
    spans.extend(hint);
    frame.render_widget(Paragraph::new(Line::from(spans)).style(theme.base()), area);
}

/// What stands between two stretches of the status bar: a space, three cells of orbit and a
/// space.
const BETWEEN: usize = 5;

/// A stretch of the status bar, and whether it says the state, which is never dropped.
type Stretch = (Vec<Span<'static>>, bool);

/// The status bar: a line across the whole width with SOLAR on it, and what matters about
/// the connection between the stretches of orbit.
#[allow(
    clippy::too_many_lines,
    reason = "the orbit is one line built from both ends, and splitting it hides the arithmetic"
)]
pub fn status(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let glyphs = &app.glyphs;
    let orbit = |count: usize| glyphs.orbit.repeat(count);
    let link = &app.link;
    let connected = link.phase == Phase::Connected;
    let (body, body_style) = if connected {
        (glyphs.body, theme.accent())
    } else if link.phase == Phase::Down {
        (glyphs.hollow, theme.error())
    } else {
        (glyphs.hollow, theme.muted())
    };
    let mut stretches: Vec<Stretch> = Vec::new();
    if let Some((tone, message)) = &app.flash {
        let style = if *tone == Tone::Error {
            theme.error()
        } else {
            theme.text()
        };
        stretches.push((
            vec![Span::styled(text::clean(message).into_owned(), style)],
            true,
        ));
    } else {
        match link.phase {
            Phase::Connected => {
                if let Some(info) = &link.info {
                    stretches.push((
                        vec![
                            Span::styled(format!("SOLAR {}", info.solar_version), theme.strong()),
                            Span::styled(format!(" {} ", glyphs.dot), theme.muted()),
                            Span::styled(info.protocol.clone(), theme.text()),
                        ],
                        false,
                    ));
                }
                if let Some(trip) = link.last_round_trip {
                    stretches.push((
                        vec![
                            Span::styled("last call ", theme.muted()),
                            Span::styled(clock::latency(trip), theme.text()),
                        ],
                        false,
                    ));
                }
                if let Some(connected_at) = link.connected_at {
                    let lasted = app.now.saturating_duration_since(connected_at);
                    stretches.push((
                        vec![
                            Span::styled("in orbit ", theme.muted()),
                            Span::styled(clock::lasted(lasted), theme.text()),
                        ],
                        true,
                    ));
                }
            }
            Phase::Starting | Phase::Handshaking => {
                let waited = app.now.saturating_duration_since(link.requested);
                stretches.push((
                    vec![
                        Span::styled("SOLAR connecting", body_style),
                        Span::styled(format!(" {} ", glyphs.dot), theme.muted()),
                        Span::styled(clock::latency(waited), theme.muted()),
                    ],
                    true,
                ));
            }
            Phase::Down => {
                let short = link.failure.as_ref().map_or_else(
                    || "not connected".to_owned(),
                    super::super::app::link::Failure::short,
                );
                stretches.push((
                    vec![
                        Span::styled("SOLAR disconnected", theme.error()),
                        Span::styled(format!(" {} ", glyphs.dot), theme.muted()),
                        Span::styled(short, theme.text()),
                    ],
                    true,
                ));
                stretches.push((
                    vec![
                        Span::styled("Ctrl+R", theme.key()),
                        Span::styled(" reconnect", theme.muted()),
                    ],
                    false,
                ));
            }
        }
    }
    let start = vec![
        Span::styled(format!(" {}", orbit(2)), theme.faint()),
        Span::styled(body.to_owned(), body_style),
        Span::styled(format!("{} ", orbit(2)), theme.faint()),
    ];
    // The state is in the words already: `in orbit` when connected, `SOLAR connecting`
    // and `SOLAR disconnected` otherwise, beside a body that is full or hollow.
    let end = Span::styled(format!("{} ", orbit(1)), theme.faint());
    let cells =
        |spans: &[Span<'_>]| -> usize { spans.iter().map(|span| text::width(&span.content)).sum() };
    // What the stretches may take: the width less the orbit at both ends, and the space
    // that always comes before the rest of the orbit.
    let room =
        usize::from(area.width).saturating_sub(cells(&start) + text::width(&end.content) + 1);
    let taken = |stretches: &[Stretch]| -> usize {
        stretches
            .iter()
            .map(|(spans, _)| cells(spans) + BETWEEN)
            .sum::<usize>()
            .saturating_sub(BETWEEN)
    };
    // Stretches drop from the end until what is left fits, all but the one that says the
    // state.
    while taken(&stretches) > room {
        let Some(last) = stretches.iter().rposition(|(_, state)| !state) else {
            break;
        };
        stretches.remove(last);
    }
    let mut middle: Vec<Span<'static>> = Vec::new();
    for (index, (spans, _)) in stretches.into_iter().enumerate() {
        if index > 0 {
            middle.push(Span::styled(format!(" {} ", orbit(3)), theme.faint()));
        }
        middle.extend(spans);
    }
    // What still does not fit is cut, in the colour of text.
    if cells(&middle) > room {
        let flat: String = middle.iter().map(|span| span.content.as_ref()).collect();
        middle = vec![Span::styled(
            text::truncate(&flat, room, glyphs.ellipsis),
            theme.text(),
        )];
    }
    let rest = room.saturating_sub(cells(&middle));
    let mut spans = start;
    spans.extend(middle);
    spans.push(Span::styled(format!(" {}", orbit(rest)), theme.faint()));
    spans.push(end);
    frame.render_widget(Paragraph::new(Line::from(spans)).style(theme.base()), area);
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant, SystemTime};

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Style;
    use zenith_client::handshake::ServerInfo;

    use super::*;
    use crate::app::Options;
    use crate::app::link::Failure;
    use crate::theme::Theme;

    #[test]
    fn the_scatter_is_the_same_every_time_and_differs_by_position() {
        assert_eq!(scatter(7, 1), scatter(7, 1));
        assert_ne!(scatter(7, 1), scatter(8, 1));
    }

    fn started() -> App {
        let options = Options {
            opening: false,
            ..Options::default()
        };
        App::new(options, Instant::now(), SystemTime::UNIX_EPOCH).0
    }

    fn connected() -> App {
        let mut app = started();
        app.link.phase = Phase::Connected;
        app.link.info = Some(ServerInfo {
            solar_version: "0.1.0".to_owned(),
            protocol: "solar/1".to_owned(),
            manifest_schema_version: "2.0.0".to_owned(),
            build: serde_json::Value::Null,
        });
        app.link.last_round_trip = Some(Duration::from_micros(490));
        app.link.connected_at = Some(app.now);
        app.now += Duration::from_secs(42);
        app
    }

    /// One row, `width` cells wide, drawn by `draw`.
    fn row(width: u16, draw: impl FnOnce(&mut Frame<'_>, Rect)) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        terminal.draw(|frame| draw(frame, frame.area())).unwrap();
        terminal.backend().buffer().clone()
    }

    fn symbols(buffer: &Buffer) -> String {
        buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    /// A state of the status bar: the body, and what the bar may show, the first choice
    /// first, each with the style it starts in.
    struct Bar {
        app: App,
        body: &'static str,
        choices: Vec<(&'static str, Style)>,
    }

    fn bars() -> Vec<Bar> {
        let theme = Theme::new(
            crate::theme::ThemeName::Night,
            crate::theme::Depth::TrueColor,
        );
        let mut down = started();
        down.link.phase = Phase::Down;
        down.link.failure = Some(Failure::Timeout);
        let mut error = connected();
        error.flash = Some((Tone::Error, "The history could not be written.".to_owned()));
        let mut info = connected();
        info.flash = Some((Tone::Info, "Press Ctrl+C again to quit".to_owned()));
        vec![
            // Connected, the stretches before `in orbit` drop from the end, and `in orbit`,
            // which says the state, is the one that stays.
            Bar {
                app: connected(),
                body: "\u{25cf}",
                choices: vec![
                    (
                        "SOLAR 0.1.0 \u{b7} solar/1 \u{2500}\u{2500}\u{2500} last call 0.49 ms \u{2500}\u{2500}\u{2500} in orbit 42s",
                        theme.strong(),
                    ),
                    (
                        "SOLAR 0.1.0 \u{b7} solar/1 \u{2500}\u{2500}\u{2500} in orbit 42s",
                        theme.strong(),
                    ),
                    ("in orbit 42s", theme.muted()),
                ],
            },
            Bar {
                app: started(),
                body: "\u{25cb}",
                choices: vec![("SOLAR connecting \u{b7} 0.00 ms", theme.muted())],
            },
            Bar {
                app: down,
                body: "\u{25cb}",
                choices: vec![
                    (
                        "SOLAR disconnected \u{b7} no answer to the handshake \u{2500}\u{2500}\u{2500} Ctrl+R reconnect",
                        theme.error(),
                    ),
                    (
                        "SOLAR disconnected \u{b7} no answer to the handshake",
                        theme.error(),
                    ),
                ],
            },
            // A message for the person takes the place of the stretches, in its tone.
            Bar {
                app: error,
                body: "\u{25cf}",
                choices: vec![("The history could not be written.", theme.error())],
            },
            Bar {
                app: info,
                body: "\u{25cf}",
                choices: vec![("Press Ctrl+C again to quit", theme.text())],
            },
        ]
    }

    /// At every width the bar is the orbit from edge to edge: its start with SOLAR on it,
    /// the first choice that fits, or the last one cut, a space, and orbit to the edge
    /// with one space after it. What is cut is in the colour of text; what is not keeps
    /// the colours of its stretches.
    #[test]
    fn the_status_bar_shows_what_fits_at_every_width_and_never_drops_the_state() {
        let orbit = "\u{2500}";
        for bar in bars() {
            let text = bar.app.theme.text();
            for width in 1..=90_u16 {
                let buffer = row(width, |frame, area| status(frame, area, &bar.app));
                let room = usize::from(width).saturating_sub(10);
                let (middle, style) = bar
                    .choices
                    .iter()
                    .find(|(choice, _)| text::width(choice) <= room)
                    .map_or_else(
                        || {
                            let (last, _) = bar.choices.last().unwrap();
                            (text::truncate(last, room, "\u{2026}"), text)
                        },
                        |(choice, style)| ((*choice).to_owned(), *style),
                    );
                let rest = room - text::width(&middle);
                let whole = format!(
                    " {orbit}{orbit}{}{orbit}{orbit} {middle} {}{orbit} ",
                    bar.body,
                    orbit.repeat(rest)
                );
                let expected: String = whole.chars().take(usize::from(width)).collect();
                assert_eq!(symbols(&buffer), expected, "at {width} cells");
                if !middle.is_empty() {
                    assert_eq!(
                        buffer[(7, 0)].fg,
                        style.fg.unwrap(),
                        "at {width} cells: {middle}"
                    );
                }
            }
        }
    }

    /// The stars of the header fill the gap between the tabs and the hint, and the two
    /// cells at each end of it are always blank.
    #[test]
    fn the_stars_of_the_header_never_touch_a_tab_or_the_hint() {
        let app = started();
        let tabs: usize = Tab::ALL
            .iter()
            .enumerate()
            .map(|(index, tab)| 2 + text::width(&format!("{} {}", index + 1, tab.title())))
            .sum();
        let before = " ZENITH ".len() + tabs;
        let hint = "? keys ".len();
        let mut stars = 0;
        for width in 60..=220_u16 {
            let buffer = row(width, |frame, area| header(frame, area, &app));
            let line = symbols(&buffer);
            let cells: Vec<char> = line.chars().collect();
            let gap = usize::from(width) - before - hint;
            assert_eq!(cells.len(), usize::from(width));
            assert!(line.ends_with("? keys "), "{line}");
            for (column, cell) in cells[before..before + gap].iter().enumerate() {
                if column < 2 || column + 2 >= gap {
                    assert_eq!(*cell, ' ', "column {column} of a gap of {gap}: {line}");
                } else if *cell != ' ' {
                    stars += 1;
                }
            }
        }
        assert!(stars > 100, "{stars}");
    }
}
