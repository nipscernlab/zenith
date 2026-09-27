//! The Session tab: the transcript, the completion menu, the band, the command line.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::cards;
use super::text::{self, pad, truncate, wrap};
use crate::app::App;
use crate::app::session::Tone;

/// The most candidates the menu shows at once.
const MENU_ROWS: usize = 8;

/// Draws the tab.
pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = usize::from(area.width);
    let band = band_lines(app, width);
    let band_height = u16::try_from(band.len()).unwrap_or(0);
    let [transcript, band_area, command] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(band_height),
        Constraint::Length(3),
    ])
    .areas(area);
    draw_transcript(frame, transcript, app);
    if band_height > 0 {
        frame.render_widget(Paragraph::new(band).style(app.theme.base()), band_area);
    }
    draw_command_line(frame, command, app);
    if app.session.menu_open() {
        draw_menu(frame, transcript, command, app);
    }
}

fn band_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    let Some(band) = &app.session.band else {
        return Vec::new();
    };
    let theme = &app.theme;
    let mut lines = Vec::new();
    for (index, sentence) in band.lines.iter().enumerate() {
        let lead = if index == 0 && band.tone == Tone::Error {
            vec![
                Span::raw(" "),
                Span::styled(" error ", theme.badge_error()),
                Span::raw(" "),
            ]
        } else {
            vec![Span::raw(" ".repeat(9))]
        };
        let mut spans = lead;
        spans.push(Span::styled(
            text::clean(sentence).into_owned(),
            theme.text(),
        ));
        lines.extend(wrap(&Line::from(spans), width, 9));
    }
    lines.truncate(4);
    lines
}

fn draw_transcript(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let width = usize::from(area.width).saturating_sub(1);
    let height = usize::from(area.height);
    let wanted = height + app.session.scroll;
    // Lay out from the newest entry up, only as far as the view needs.
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut reached_top = true;
    for (_, entry) in app.session.transcript.iter().rev() {
        let mut entry_lines = cards::entry_lines(app, entry, width);
        entry_lines.extend(std::mem::take(&mut lines));
        lines = entry_lines;
        if lines.len() >= wanted {
            reached_top = false;
            break;
        }
    }
    if reached_top && app.session.transcript.dropped() > 0 {
        lines.insert(
            0,
            Line::from(Span::styled(
                format!(
                    " {} older entries were dropped to stay within {} entries.",
                    app.session.transcript.dropped(),
                    app.session.transcript.max_entries()
                ),
                theme.muted(),
            )),
        );
    }
    let scroll = app.session.scroll.min(lines.len().saturating_sub(height));
    let end = lines.len() - scroll;
    let start = end.saturating_sub(height);
    let visible: Vec<Line<'static>> = lines[start..end].to_vec();
    // Short transcripts sit at the bottom, next to the command line.
    let top_padding = height.saturating_sub(visible.len());
    let mut shown: Vec<Line<'static>> = vec![Line::from(""); top_padding];
    shown.extend(visible);
    frame.render_widget(Paragraph::new(shown).style(theme.base()), area);
    if scroll > 0 {
        let marker = if app.session.unseen > 0 {
            format!(" {} {} new ", app.glyphs.down, app.session.unseen)
        } else {
            format!(" {} more below ", app.glyphs.down)
        };
        let marker_width = u16::try_from(text::width(&marker)).unwrap_or(0);
        let at = Rect::new(
            area.x + area.width.saturating_sub(marker_width + 1),
            area.y + area.height.saturating_sub(1),
            marker_width.min(area.width),
            1,
        );
        frame.render_widget(Paragraph::new(Span::styled(marker, theme.badge_ok())), at);
    }
}

fn draw_command_line(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = Block::new()
        .borders(Borders::ALL)
        .border_set(app.glyphs.border)
        .border_style(theme.faint())
        .style(theme.base());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let editor = &app.session.editor;
    let prompt = format!(" {} ", app.glyphs.prompt);
    let prompt_width = text::width(&prompt);
    let room = usize::from(inner.width).saturating_sub(prompt_width + 1);
    let mut spans = vec![Span::styled(prompt, theme.accent())];
    if editor.is_empty() {
        let placeholder = "Type / for the commands, or ? for every key.";
        spans.push(Span::styled(
            truncate(placeholder, room, app.glyphs.ellipsis),
            theme.muted(),
        ));
    } else {
        let (start, column) = editor.window(room);
        let visible = window_text(&editor.text()[start..], room);
        let underline = app
            .session
            .band
            .as_ref()
            .and_then(|band| band.underline.clone());
        let end = start + visible.len();
        match underline {
            Some(range) if range.start < end && range.end > start => {
                let from = range.start.max(start) - start;
                let to = range.end.min(end) - start;
                let (from, to) = (boundary(&visible, from), boundary(&visible, to));
                spans.push(Span::styled(visible[..from].to_owned(), theme.text()));
                spans.push(Span::styled(
                    visible[from..to].to_owned(),
                    theme.error().add_modifier(Modifier::UNDERLINED),
                ));
                spans.push(Span::styled(visible[to..].to_owned(), theme.text()));
            }
            _ => spans.push(Span::styled(visible, theme.text())),
        }
        let x = inner.x + u16::try_from(prompt_width + column).unwrap_or(0);
        frame.set_cursor_position(Position::new(
            x.min(inner.x + inner.width.saturating_sub(1)),
            inner.y,
        ));
    }
    if editor.is_empty() {
        let x = inner.x + u16::try_from(prompt_width).unwrap_or(0);
        frame.set_cursor_position(Position::new(x, inner.y));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

/// The text from the window's start that fits in `room` cells.
fn window_text(text: &str, room: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for character in text.chars() {
        let cells = unicode_width::UnicodeWidthChar::width(character).unwrap_or(0);
        if used + cells > room {
            break;
        }
        used += cells;
        out.push(character);
    }
    out
}

fn boundary(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn draw_menu(frame: &mut Frame<'_>, transcript: Rect, command: Rect, app: &App) {
    let Some(completion) = &app.session.completion else {
        return;
    };
    let theme = &app.theme;
    let count = completion.candidates.len();
    let rows = count.min(MENU_ROWS);
    let height = u16::try_from(rows + 2).unwrap_or(3).min(transcript.height);
    let width = transcript.width.saturating_sub(2).min(78);
    let area = Rect::new(
        command.x + 1,
        command.y.saturating_sub(height),
        width,
        height,
    );
    let first = app
        .session
        .highlighted
        .saturating_sub(rows.saturating_sub(1))
        .min(count.saturating_sub(rows));
    let label_width = completion
        .candidates
        .iter()
        .map(|candidate| text::width(&candidate.label))
        .max()
        .unwrap_or(0)
        .min(34);
    let inner_width = usize::from(width.saturating_sub(2));
    let lines: Vec<Line<'static>> = completion
        .candidates
        .iter()
        .enumerate()
        .skip(first)
        .take(rows)
        .map(|(index, candidate)| {
            let highlighted = index == app.session.highlighted;
            let marker = if highlighted { app.glyphs.prompt } else { " " };
            let detail_room = inner_width.saturating_sub(label_width + 5);
            let line = Line::from(vec![
                Span::styled(format!("{marker} "), theme.accent()),
                Span::styled(
                    pad(&text::clean(&candidate.label), label_width),
                    theme.accent(),
                ),
                Span::raw("  "),
                Span::styled(
                    truncate(
                        &text::clean(&candidate.detail),
                        detail_room,
                        app.glyphs.ellipsis,
                    ),
                    theme.muted(),
                ),
            ]);
            if highlighted {
                line.style(theme.selection())
            } else {
                line
            }
        })
        .collect();
    let position = if count > rows {
        format!(" {} of {count} ", app.session.highlighted + 1)
    } else {
        String::new()
    };
    let block = Block::new()
        .borders(Borders::ALL)
        .border_set(app.glyphs.border)
        .border_style(theme.faint())
        .style(theme.base())
        .title_bottom(Line::from(vec![
            Span::styled(" Tab", theme.key()),
            Span::styled(" inserts  ", theme.muted()),
            Span::styled(app.glyphs.key_name("\u{2191} \u{2193}"), theme.key()),
            Span::styled(" choose  ", theme.muted()),
            Span::styled("Esc", theme.key()),
            Span::styled(" closes ", theme.muted()),
        ]))
        .title_top(Line::from(Span::styled(position, theme.muted())).right_aligned());
    frame.render_widget(Clear, area);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines), inner);
}
