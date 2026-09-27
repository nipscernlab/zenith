//! The opening: a starfield with the SOLAR mark, while the connection is made.
//!
//! The mark is laid out as SOLAR's `docs/brand/README.md`, section 3, says: the symbol of
//! 16 × 8 cells, the text three columns after it, the name on the row whose slot opens to
//! the right. No star is drawn within two cells of the mark, which is the clear space the
//! brand asks for.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::chrome::scatter;
use super::text;
use crate::app::App;
use crate::app::link::Phase;
use crate::brand;

/// One star in how many cells.
const DENSITY: u64 = 29;

/// The words beside the mark, from the brand's example; the fourth is the state of the
/// connection, where the brand puts the version.
const BESIDE: [&str; 3] = ["SOLAR", "The central API of the Constellation", "NIPS-CERN"];

/// Draws the opening over the whole screen.
#[allow(
    clippy::too_many_lines,
    reason = "the layout of the opening is one picture, and splitting it scatters its geometry"
)]
pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let failure = (app.link.phase == Phase::Down)
        .then_some(app.link.failure.as_ref())
        .flatten();
    let state = match app.link.phase {
        Phase::Down => ("not connected", theme.error()),
        Phase::Connected => ("connected", theme.accent()),
        Phase::Starting | Phase::Handshaking => ("connecting", theme.muted()),
    };
    let text_width = BESIDE
        .iter()
        .map(|line| text::width(line))
        .chain(std::iter::once(text::width(state.0)))
        .max()
        .unwrap_or(0);
    let mark_width = u16::try_from(brand::WIDTH + brand::GAP + text_width).unwrap_or(area.width);
    let footer = format!("ZENITH {}, the terminal of Constellation", crate::VERSION);

    // The failure card, when there is one, is laid out first, to know how tall it is.
    let card_width = area.width.saturating_sub(8).min(76);
    let card_lines: Vec<Line<'static>> = failure.map_or_else(Vec::new, |failure| {
        let inner = usize::from(card_width.saturating_sub(4));
        let mut lines = Vec::new();
        lines.extend(text::wrap(
            &Line::from(Span::styled(failure.what(), theme.error())),
            inner,
            0,
        ));
        lines.extend(text::wrap(
            &Line::from(Span::styled(failure.advice(), theme.text())),
            inner,
            0,
        ));
        for words in failure.last_words() {
            lines.push(Line::from(Span::styled(
                text::truncate(&text::clean(words), inner, app.glyphs.ellipsis),
                theme.muted(),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("r", theme.key()),
            Span::styled(" try again   ", theme.muted()),
            Span::styled("Enter", theme.key()),
            Span::styled(" go to the tabs   ", theme.muted()),
            Span::styled("q", theme.key()),
            Span::styled(" quit", theme.muted()),
        ]));
        lines
    });
    let card_height = if card_lines.is_empty() {
        0
    } else {
        u16::try_from(card_lines.len()).unwrap_or(0) + 2
    };
    let mark_height = u16::try_from(brand::HEIGHT).unwrap_or(8);
    let total = mark_height + 2 + 1 + if card_height > 0 { 2 + card_height } else { 0 };
    let top = area.y + area.height.saturating_sub(total) / 2;
    let left = area.x + area.width.saturating_sub(mark_width) / 2;
    let mark = Rect::new(left, top, mark_width.min(area.width), mark_height);
    let footer_area = Rect::new(area.x, top + mark_height + 2, area.width, 1);
    let card = Rect::new(
        area.x + area.width.saturating_sub(card_width) / 2,
        footer_area.y + 2,
        card_width,
        card_height,
    );

    // The stars, outside the clear space around everything that carries words.
    let keep_out = [
        grow(mark, 2),
        grow(footer_area_text(footer_area, &footer), 2),
        grow(card, 1),
    ];
    let frame_number = app.opening.map_or(0, |opening| opening.frame);
    let buffer = frame.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if keep_out
                .iter()
                .any(|rect| rect.height > 0 && contains(*rect, x, y))
            {
                continue;
            }
            let position = u64::from(y) * u64::from(area.width) + u64::from(x);
            let value = scatter(position, 0x00C0_FFEE);
            if !value.is_multiple_of(DENSITY) {
                continue;
            }
            let twinkles = (value >> 24).is_multiple_of(3);
            let phase = (value >> 16).wrapping_add(if twinkles { frame_number } else { 0 });
            let bright = phase % 6 < 2;
            let glyph = app.glyphs.stars[usize::try_from((value >> 8) % 4).unwrap_or(0)];
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_symbol(glyph).set_style(theme.star(bright));
            }
        }
    }

    // The mark, then the words beside it.
    let rows = brand::rows(app.glyphs.charset);
    for (offset, row) in rows.iter().enumerate() {
        let y = top + u16::try_from(offset).unwrap_or(0);
        if y >= area.y + area.height {
            break;
        }
        let beside = offset
            .checked_sub(brand::NAME_ROW)
            .and_then(|index| match index {
                0 => Some((BESIDE[0], theme.strong())),
                1 => Some((BESIDE[1], theme.text())),
                2 => Some((BESIDE[2], theme.muted())),
                3 => Some(state),
                _ => None,
            });
        let mut spans = vec![Span::styled(row.clone(), theme.mark())];
        if let Some((words, style)) = beside {
            spans.push(Span::styled(" ".repeat(brand::GAP), theme.base()));
            spans.push(Span::styled(words.to_owned(), style));
        }
        let line_area = Rect::new(left, y, mark_width.min(area.width), 1);
        frame.render_widget(Paragraph::new(Line::from(spans)), line_area);
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(footer, theme.muted())))
            .alignment(ratatui::layout::Alignment::Center),
        footer_area,
    );
    if card_height > 0 {
        let block = Block::new()
            .borders(Borders::ALL)
            .border_set(app.glyphs.border)
            .border_style(theme.faint())
            .style(theme.base());
        let inner = block.inner(card);
        frame.render_widget(block, card);
        let padded = Rect::new(
            inner.x + 1,
            inner.y,
            inner.width.saturating_sub(2),
            inner.height,
        );
        frame.render_widget(Paragraph::new(card_lines).style(Style::default()), padded);
    }
}

fn footer_area_text(area: Rect, footer: &str) -> Rect {
    let width = u16::try_from(text::width(footer)).unwrap_or(area.width);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y,
        width.min(area.width),
        1,
    )
}

fn grow(rect: Rect, by: u16) -> Rect {
    Rect::new(
        rect.x.saturating_sub(by),
        rect.y.saturating_sub(by),
        rect.width + 2 * by,
        rect.height + 2 * by,
    )
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}
