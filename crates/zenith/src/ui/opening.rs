//! The opening: a starfield with ZENITH's mark, while the connection is made.
//!
//! The mark is laid out as `docs/brand/README.md`, section 3, says: the symbol of 16 × 4
//! cells, and beside it, three columns after it, the name on the row level with the top of
//! the dome, what ZENITH is, the laboratory and the version on the rows below. The state of
//! the connection is the line under it. No star is drawn within two cells of the mark,
//! which is the clear space the brand asks for.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::chrome::sky;
use super::{lockup, text};
use crate::app::App;
use crate::app::link::Phase;
use crate::brand;

/// What ZENITH is, on the row under its name.
const WHAT: &str = "The terminal of Constellation";

/// The laboratory, on the row under that.
const WHO: &str = "NIPS-CERN";

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
    let (footer, footer_style) = match app.link.phase {
        Phase::Down => ("Not connected to SOLAR", theme.error()),
        Phase::Connected => ("Connected to SOLAR", theme.accent()),
        Phase::Starting | Phase::Handshaking => ("Connecting to SOLAR", theme.muted()),
    };
    let beside = [
        ("ZENITH".to_owned(), theme.strong()),
        (WHAT.to_owned(), theme.text()),
        (WHO.to_owned(), theme.muted()),
        (crate::VERSION.to_owned(), theme.muted()),
    ];
    let mark_width = u16::try_from(lockup::width(&brand::ZENITH, &beside)).unwrap_or(area.width);

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
    let mark_height = u16::try_from(brand::ZENITH.height).unwrap_or(4);
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
        grow(footer_area_text(footer_area, footer), 2),
        grow(card, 1),
    ];
    let frame_number = app.opening.map_or(0, |opening| opening.frame);
    sky(
        frame.buffer_mut(),
        area.width,
        area,
        &keep_out,
        frame_number,
        app,
    );

    // The mark, then the words beside it.
    let lines = lockup::lines(&brand::ZENITH, app.glyphs.charset, 0, &beside, theme);
    for (offset, line) in lines.into_iter().enumerate() {
        let y = top + u16::try_from(offset).unwrap_or(0);
        if y >= area.y + area.height {
            break;
        }
        let line_area = Rect::new(left, y, mark_width.min(area.width), 1);
        frame.render_widget(Paragraph::new(line), line_area);
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(footer, footer_style)))
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
