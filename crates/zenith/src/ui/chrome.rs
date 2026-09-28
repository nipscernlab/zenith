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
    let mut stars = String::new();
    for column in 0..gap {
        let value = scatter(column as u64, 0x5A7E_11E5);
        let star = if column > 1 && column + 2 < gap && value.is_multiple_of(7) {
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
    let mut segments: Vec<Vec<Span<'static>>> = Vec::new();
    if let Some((tone, message)) = &app.flash {
        let style = if *tone == Tone::Error {
            theme.error()
        } else {
            theme.text()
        };
        segments.push(vec![Span::styled(text::clean(message).into_owned(), style)]);
    } else {
        match link.phase {
            Phase::Connected => {
                if let Some(info) = &link.info {
                    segments.push(vec![
                        Span::styled(format!("SOLAR {}", info.solar_version), theme.strong()),
                        Span::styled(format!(" {} ", glyphs.dot), theme.muted()),
                        Span::styled(info.protocol.clone(), theme.text()),
                    ]);
                }
                if let Some(trip) = link.last_round_trip {
                    segments.push(vec![
                        Span::styled("last call ", theme.muted()),
                        Span::styled(clock::latency(trip), theme.text()),
                    ]);
                }
                if let Some(connected_at) = link.connected_at {
                    let lasted = app.now.saturating_duration_since(connected_at);
                    segments.push(vec![
                        Span::styled("in orbit ", theme.muted()),
                        Span::styled(clock::lasted(lasted), theme.text()),
                    ]);
                }
            }
            Phase::Starting | Phase::Handshaking => {
                let waited = app.now.saturating_duration_since(link.requested);
                segments.push(vec![
                    Span::styled("SOLAR connecting", body_style),
                    Span::styled(format!(" {} ", glyphs.dot), theme.muted()),
                    Span::styled(clock::latency(waited), theme.muted()),
                ]);
            }
            Phase::Down => {
                let short = link.failure.as_ref().map_or_else(
                    || "not connected".to_owned(),
                    super::super::app::link::Failure::short,
                );
                segments.push(vec![
                    Span::styled("SOLAR disconnected", theme.error()),
                    Span::styled(format!(" {} ", glyphs.dot), theme.muted()),
                    Span::styled(short, theme.text()),
                ]);
                segments.push(vec![
                    Span::styled("Ctrl+R", theme.key()),
                    Span::styled(" reconnect", theme.muted()),
                ]);
            }
        }
    }
    let width = usize::from(area.width);
    let start = vec![
        Span::styled(format!(" {}", orbit(2)), theme.faint()),
        Span::styled(body.to_owned(), body_style),
        Span::styled(format!("{} ", orbit(2)), theme.faint()),
    ];
    // The state is in the words already: `in orbit` when connected, `SOLAR connecting`
    // and `SOLAR disconnected` otherwise, beside a body that is full or hollow.
    let end = vec![Span::styled(format!("{} ", orbit(1)), theme.faint())];
    let cells =
        |spans: &[Span<'_>]| -> usize { spans.iter().map(|span| text::width(&span.content)).sum() };
    let fixed = cells(&start) + cells(&end);
    // Drop segments from the end until what is left fits, then cut the first if needed.
    while segments.len() > 1 {
        let joined: usize =
            segments.iter().map(|segment| cells(segment)).sum::<usize>() + (segments.len() - 1) * 5;
        if fixed + joined < width {
            break;
        }
        segments.pop();
    }
    let mut spans = start;
    for (index, segment) in segments.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(format!(" {} ", orbit(3)), theme.faint()));
        }
        spans.extend(segment);
    }
    let room = width.saturating_sub(cells(&spans) + cells(&end));
    if cells(&spans) + cells(&end) > width {
        let flat: String = spans.iter().map(|span| span.content.as_ref()).collect();
        let cut = text::truncate(&flat, width.saturating_sub(cells(&end)), glyphs.ellipsis);
        spans = vec![Span::styled(cut, theme.text())];
    } else {
        spans.push(Span::styled(
            format!(" {}", orbit(room.saturating_sub(1))),
            theme.faint(),
        ));
    }
    spans.extend(end);
    frame.render_widget(Paragraph::new(Line::from(spans)).style(theme.base()), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scatter_is_the_same_every_time_and_differs_by_position() {
        assert_eq!(scatter(7, 1), scatter(7, 1));
        assert_ne!(scatter(7, 1), scatter(8, 1));
    }
}
