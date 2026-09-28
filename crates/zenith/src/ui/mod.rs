//! Drawing: the whole screen from the application's state, and nothing else.
//!
//! Drawing reads the state and never changes it, and it reads no clock: every time on
//! screen is computed from `app.now`, which is what makes a screen a function of the state
//! and every screen snapshot-testable.

pub mod apis;
pub mod cards;
pub mod chrome;
pub mod history;
pub mod json;
pub mod lockup;
pub mod log;
pub mod opening;
pub mod overlays;
pub mod session;
pub mod text;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{App, Overlay, Tab};
use crate::glyphs::Charset;

/// The smallest terminal ZENITH draws its screen in.
pub const MIN_WIDTH: u16 = 80;

/// The smallest terminal ZENITH draws its screen in.
pub const MIN_HEIGHT: u16 = 24;

/// Draws the whole screen.
pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    frame.render_widget(Block::new().style(app.theme.base()), area);
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        too_small(frame, area, app);
        return;
    }
    if app.opening.is_some() {
        opening::draw(frame, area, app);
        return;
    }
    let [header, content, status] = frame_areas(area);
    chrome::header(frame, header, app);
    chrome::status(frame, status, app);
    match app.tab {
        Tab::Session => session::draw(frame, content, app),
        Tab::Apis => apis::draw(frame, content, app),
        Tab::Log => log::draw(frame, content, app),
        Tab::History => history::draw(frame, content, app),
    }
    match app.overlay {
        Some(Overlay::Help { scroll }) => overlays::help(frame, content, app, scroll),
        Some(Overlay::Viewer { record, scroll }) => {
            overlays::viewer(frame, content, app, record, scroll);
        }
        None => {}
    }
}

/// The header, the tab and the status bar of a screen.
fn frame_areas(area: Rect) -> [Rect; 3] {
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(area)
}

/// The area of the tab, at the size the terminal last said it was.
fn content_area(app: &App) -> Rect {
    let [_, content, _] = frame_areas(Rect::new(0, 0, app.size.0, app.size.1));
    content
}

/// What a notch of the wheel scrolls, by where the pointer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scrolls {
    /// Nothing: the header, the status bar, the opening, the form, a terminal too small.
    Nothing,
    /// The help or the viewer, wherever the pointer is, since it covers the tab and has
    /// the keys.
    Overlay,
    /// The Session tab's transcript, from anywhere on the tab but the menu: it is the
    /// tab's one view that scrolls.
    Transcript,
    /// The completion menu, under the pointer.
    Menu,
    /// The list of APIs.
    ApiList,
    /// The entry of the API selected.
    ApiEntry,
    /// The Log's lines.
    Log,
    /// The History's calls.
    History,
}

/// What the wheel scrolls with the pointer on `position`, laid out by the code that draws
/// the screen, at the size the terminal last said it was.
#[must_use]
pub fn scrolls_at(app: &App, position: Position) -> Scrolls {
    if app.size.0 < MIN_WIDTH || app.size.1 < MIN_HEIGHT || app.opening.is_some() {
        return Scrolls::Nothing;
    }
    if app.overlay.is_some() {
        return Scrolls::Overlay;
    }
    let content = content_area(app);
    if !content.contains(position) {
        return Scrolls::Nothing;
    }
    match app.tab {
        Tab::Session => session::scrolls_at(app, content, position),
        Tab::Apis => apis::scrolls_at(app, content, position),
        Tab::Log => Scrolls::Log,
        Tab::History => Scrolls::History,
    }
}

fn too_small(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let times = if app.glyphs.charset == Charset::Ascii {
        "x"
    } else {
        "\u{d7}"
    };
    let sentence = format!(
        "ZENITH needs {MIN_WIDTH} {times} {MIN_HEIGHT}, and this terminal is {} {times} {}.",
        area.width, area.height
    );
    let lines = text::wrap(
        &Line::from(Span::styled(sentence, app.theme.text())),
        usize::from(area.width.max(1)),
        0,
    );
    let height = u16::try_from(lines.len()).unwrap_or(1).min(area.height);
    let at = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(height) / 2,
        area.width,
        height,
    );
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(app.theme.base()),
        at,
    );
}

/// A line of key hints, `key what  key what`, for the bottom of a tab.
#[must_use]
pub fn hint_bar(app: &App, hints: &[(&str, &str)]) -> Paragraph<'static> {
    let theme = &app.theme;
    let mut spans = vec![Span::raw(" ")];
    for (key, words) in hints {
        spans.push(Span::styled(app.glyphs.key_name(key), theme.key()));
        spans.push(Span::styled(format!(" {words}   "), theme.muted()));
    }
    Paragraph::new(Line::from(spans)).style(theme.base())
}

/// Draws a line of key hints.
pub fn draw_hints(frame: &mut Frame<'_>, area: Rect, app: &App, hints: &[(&str, &str)]) {
    frame.render_widget(hint_bar(app, hints), area);
}
