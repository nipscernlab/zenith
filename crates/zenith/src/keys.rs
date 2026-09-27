//! Every key ZENITH answers to, in one table.
//!
//! The table is the single source for two things: which action a key press means, and
//! what the help overlay lists. A key therefore cannot exist without being listed, and
//! adding one is adding one row here (`docs/ADDING_A_FEATURE.md`). The tests hold the
//! table to ADR 0005: every action has a key with `Ctrl` or a plain key, and `Alt` and the
//! function keys are only ever extra routes.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What a key press asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Open the help overlay.
    Help,
    /// Go to the next tab.
    NextTab,
    /// Go to the previous tab.
    PreviousTab,
    /// Go to a tab by its number, from 1. Only ever an extra route: `Tab` reaches every
    /// tab.
    GoToTab(u8),
    /// Go to the Session tab with `/` on the command line.
    Slash,
    /// Open the envelope viewer.
    Envelope,
    /// Restart SOLAR.
    Reconnect,
    /// Draw the whole screen again.
    Repaint,
    /// Cancel the call in flight, or arm quitting.
    Interrupt,
    /// Close what is open.
    Close,
    /// Run the command line.
    Submit,
    /// Complete, or move to the next candidate.
    Complete,
    /// Move to the previous candidate.
    CompleteBack,
    /// Up: the menu, the history, a list.
    Up,
    /// Down: the menu, the history, a list.
    Down,
    /// Move the cursor left.
    Left,
    /// Move the cursor right.
    Right,
    /// Move the cursor to the start.
    Home,
    /// Move the cursor to the end.
    End,
    /// Move the cursor a word left.
    WordLeft,
    /// Move the cursor a word right.
    WordRight,
    /// Delete the character before the cursor.
    Backspace,
    /// Delete the character at the cursor.
    Delete,
    /// Delete the word before the cursor.
    DeleteWord,
    /// Delete from the start to the cursor.
    DeleteToStart,
    /// Delete from the cursor to the end.
    DeleteToEnd,
    /// Scroll or move up by a page.
    PageUp,
    /// Scroll or move down by a page.
    PageDown,
    /// Go to the top of a list.
    Top,
    /// Go to the bottom of a list, and follow it.
    Bottom,
    /// Run the example with this number, from 1.
    RunExample(u8),
    /// Run every example of the selected API.
    RunAllExamples,
    /// Open the parameter form of the selected API.
    OpenForm,
    /// Put the selected thing on the command line, to edit it there.
    Edit,
    /// Filter the list by a part of a name.
    Filter,
    /// Ask SOLAR for the manifest again.
    ReloadManifest,
    /// The next field of the form.
    NextField,
    /// The previous field of the form.
    PreviousField,
    /// Toggle a boolean field.
    Toggle,
    /// Run what is selected or filled in.
    Run,
    /// Choose the lowest level the Log shows.
    Level(LevelKey),
    /// Show one level fewer.
    LowerLevel,
    /// Show one level more.
    RaiseLevel,
    /// Expand the selected line.
    Expand,
    /// Clear the tab.
    Clear,
    /// Send the selected request again.
    RunAgain,
    /// Export the History.
    Export,
    /// The previous call, in the viewer.
    PreviousCall,
    /// The next call, in the viewer.
    NextCall,
    /// Quit, where it is offered.
    Quit,
    /// Go on to the tabs, from the opening's failure card.
    Continue,
}

/// A level of SOLAR's log, as the keys of the Log tab choose them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LevelKey {
    /// `error`.
    Error,
    /// `warn`.
    Warn,
    /// `info`.
    Info,
    /// `debug`.
    Debug,
    /// `trace`.
    Trace,
}

/// Where a binding works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place {
    /// On every tab, unless a text field has the keys.
    Everywhere,
    /// The command line of the Session tab.
    CommandLine,
    /// Any list: the tabs other than Session, the viewer, the help.
    List,
    /// The APIs tab.
    Apis,
    /// The parameter form.
    Form,
    /// The Log tab.
    Log,
    /// The History tab.
    History,
    /// The envelope viewer.
    Viewer,
    /// The help overlay.
    Help,
    /// The failure card of the opening.
    Failure,
}

impl Place {
    /// The heading of this place in the help overlay.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Everywhere => "Everywhere",
            Self::CommandLine => "The command line",
            Self::List => "Lists",
            Self::Apis => "APIs",
            Self::Form => "The parameter form",
            Self::Log => "Log",
            Self::History => "History",
            Self::Viewer => "The envelope viewer",
            Self::Help => "This help",
            Self::Failure => "When SOLAR fails at the start",
        }
    }

    /// The order of the help overlay.
    pub const ALL: [Self; 10] = [
        Self::Everywhere,
        Self::CommandLine,
        Self::List,
        Self::Apis,
        Self::Form,
        Self::Log,
        Self::History,
        Self::Viewer,
        Self::Help,
        Self::Failure,
    ];
}

/// When a binding applies, on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    /// Always.
    Always,
    /// Only when the command line is empty.
    LineEmpty,
    /// Only when the command line has text.
    LineHasText,
}

/// One key, as the table writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    /// The key.
    pub code: KeyCode,
    /// `Ctrl` or `Alt`, or neither. Shift is part of the character or the code.
    pub modifiers: KeyModifiers,
}

const fn plain(code: KeyCode) -> Key {
    Key {
        code,
        modifiers: KeyModifiers::NONE,
    }
}

const fn character(character: char) -> Key {
    plain(KeyCode::Char(character))
}

const fn ctrl(character: char) -> Key {
    Key {
        code: KeyCode::Char(character),
        modifiers: KeyModifiers::CONTROL,
    }
}

const fn ctrl_code(code: KeyCode) -> Key {
    Key {
        code,
        modifiers: KeyModifiers::CONTROL,
    }
}

const fn alt(character: char) -> Key {
    Key {
        code: KeyCode::Char(character),
        modifiers: KeyModifiers::ALT,
    }
}

impl Key {
    /// Whether this key depends on something some terminal does not deliver: `Alt`, which
    /// macOS Terminal does not send by default, or a function key, which several
    /// terminals keep for themselves.
    #[must_use]
    pub fn is_extra(&self) -> bool {
        self.modifiers.contains(KeyModifiers::ALT) || matches!(self.code, KeyCode::F(_))
    }

    /// How the help overlay writes this key.
    #[must_use]
    pub fn label(&self) -> String {
        let name = match self.code {
            KeyCode::Char(' ') => "Space".to_owned(),
            KeyCode::Char(character) => {
                if self.modifiers.is_empty() {
                    character.to_string()
                } else {
                    character.to_ascii_uppercase().to_string()
                }
            }
            KeyCode::Enter => "Enter".to_owned(),
            KeyCode::Esc => "Esc".to_owned(),
            KeyCode::Tab => "Tab".to_owned(),
            KeyCode::BackTab => "Shift+Tab".to_owned(),
            KeyCode::Backspace => "Backspace".to_owned(),
            KeyCode::Delete => "Delete".to_owned(),
            KeyCode::Home => "Home".to_owned(),
            KeyCode::End => "End".to_owned(),
            KeyCode::PageUp => "PgUp".to_owned(),
            KeyCode::PageDown => "PgDn".to_owned(),
            KeyCode::Up => "\u{2191}".to_owned(),
            KeyCode::Down => "\u{2193}".to_owned(),
            KeyCode::Left => "\u{2190}".to_owned(),
            KeyCode::Right => "\u{2192}".to_owned(),
            KeyCode::F(number) => format!("F{number}"),
            other => format!("{other:?}"),
        };
        let mut label = String::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            label.push_str("Ctrl+");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            label.push_str("Alt+");
        }
        label.push_str(&name);
        label
    }

    /// Whether a key press is this key. Shift is ignored for characters, since it is
    /// already in the character, and for `Shift+Tab`, which terminals send as its own key.
    #[must_use]
    pub fn matches(&self, event: &KeyEvent) -> bool {
        let wanted = self.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT);
        let pressed = typed_modifiers(event);
        if wanted != pressed {
            return false;
        }
        match (self.code, event.code) {
            (KeyCode::Char(a), KeyCode::Char(b)) if !wanted.is_empty() => {
                a.eq_ignore_ascii_case(&b)
            }
            (a, b) => a == b,
        }
    }
}

/// The modifiers of a key press that mean something to the table: `Ctrl` and `Alt`, except
/// for a character typed with both at once, which is `AltGr` on Windows and has none.
///
/// On a Brazilian ABNT2 keyboard `/` and `?` are `AltGr+Q` and `AltGr+W`, and on German,
/// French and many other layouts `{`, `}`, `[`, `]` and `@` are typed the same way;
/// Windows reports every one of them as the character with `Ctrl` and `Alt` held. Taking
/// them for shortcuts would make those keyboards unable to type a command.
#[must_use]
pub fn typed_modifiers(event: &KeyEvent) -> KeyModifiers {
    let held = event.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT);
    let altgr = held.contains(KeyModifiers::CONTROL | KeyModifiers::ALT)
        && matches!(event.code, KeyCode::Char(character) if !character.is_control());
    if altgr { KeyModifiers::NONE } else { held }
}

/// Whether a key press is text to type: a character with neither `Ctrl` nor `Alt`, or
/// one typed with `AltGr`.
#[must_use]
pub fn is_text(event: &KeyEvent) -> bool {
    matches!(event.code, KeyCode::Char(character) if !character.is_control())
        && typed_modifiers(event).is_empty()
}

/// One row of the table.
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    /// Where it works.
    pub place: Place,
    /// The keys, the first of which the help lists first.
    pub keys: &'static [Key],
    /// What they do.
    pub action: Action,
    /// When, on the command line.
    pub when: When,
    /// What the help overlay says.
    pub help: &'static str,
}

const fn bind(place: Place, keys: &'static [Key], action: Action, help: &'static str) -> Binding {
    Binding {
        place,
        keys,
        action,
        when: When::Always,
        help,
    }
}

const fn bind_when(
    place: Place,
    keys: &'static [Key],
    action: Action,
    when: When,
    help: &'static str,
) -> Binding {
    Binding {
        place,
        keys,
        action,
        when,
        help,
    }
}

use Place::{
    Apis, CommandLine, Everywhere, Failure, Form, Help as HelpPlace, History, List, Log, Viewer,
};

/// Every key ZENITH answers to.
pub const BINDINGS: &[Binding] = &[
    bind(
        Everywhere,
        &[character('?')],
        Action::Help,
        "every key, by where it works",
    ),
    bind(
        Everywhere,
        &[plain(KeyCode::Tab)],
        Action::NextTab,
        "the next tab",
    ),
    bind(
        Everywhere,
        &[plain(KeyCode::BackTab)],
        Action::PreviousTab,
        "the previous tab",
    ),
    bind(
        Everywhere,
        &[plain(KeyCode::F(1)), alt('1')],
        Action::GoToTab(1),
        "Session, where the terminal delivers the key",
    ),
    bind(
        Everywhere,
        &[plain(KeyCode::F(2)), alt('2')],
        Action::GoToTab(2),
        "APIs, where the terminal delivers the key",
    ),
    bind(
        Everywhere,
        &[plain(KeyCode::F(3)), alt('3')],
        Action::GoToTab(3),
        "Log, where the terminal delivers the key",
    ),
    bind(
        Everywhere,
        &[plain(KeyCode::F(4)), alt('4')],
        Action::GoToTab(4),
        "History, where the terminal delivers the key",
    ),
    bind(
        Everywhere,
        &[character('/')],
        Action::Slash,
        "a command, on the Session tab",
    ),
    bind(
        Everywhere,
        &[ctrl('o')],
        Action::Envelope,
        "the whole envelope of the latest call",
    ),
    bind(
        Everywhere,
        &[ctrl('r')],
        Action::Reconnect,
        "restart SOLAR, and pick up a new build",
    ),
    bind(
        Everywhere,
        &[ctrl('l')],
        Action::Repaint,
        "draw the whole screen again",
    ),
    bind(
        Everywhere,
        &[ctrl('c')],
        Action::Interrupt,
        "cancel the call in flight when SOLAR offers solar.cancel; again, quit",
    ),
    bind(
        Everywhere,
        &[plain(KeyCode::Esc)],
        Action::Close,
        "close the overlay, menu or form",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::Enter)],
        Action::Submit,
        "run the line as typed",
    ),
    bind_when(
        CommandLine,
        &[plain(KeyCode::Tab)],
        Action::Complete,
        When::LineHasText,
        "complete, and the next candidate",
    ),
    bind_when(
        CommandLine,
        &[plain(KeyCode::BackTab)],
        Action::CompleteBack,
        When::LineHasText,
        "the previous candidate",
    ),
    bind_when(
        CommandLine,
        &[plain(KeyCode::Tab)],
        Action::NextTab,
        When::LineEmpty,
        "on an empty line, the next tab",
    ),
    bind_when(
        CommandLine,
        &[plain(KeyCode::BackTab)],
        Action::PreviousTab,
        When::LineEmpty,
        "on an empty line, the previous tab",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::Up)],
        Action::Up,
        "the menu when it is open, the history otherwise",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::Down)],
        Action::Down,
        "the menu when it is open, the history otherwise",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::Left)],
        Action::Left,
        "move left",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::Right)],
        Action::Right,
        "move right",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::Home), ctrl('a')],
        Action::Home,
        "the start of the line",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::End), ctrl('e')],
        Action::End,
        "the end of the line",
    ),
    bind(
        CommandLine,
        &[ctrl_code(KeyCode::Left), alt('b')],
        Action::WordLeft,
        "a word left",
    ),
    bind(
        CommandLine,
        &[ctrl_code(KeyCode::Right), alt('f')],
        Action::WordRight,
        "a word right",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::Backspace)],
        Action::Backspace,
        "delete back",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::Delete)],
        Action::Delete,
        "delete forward",
    ),
    bind(
        CommandLine,
        &[ctrl('w')],
        Action::DeleteWord,
        "delete the word before",
    ),
    bind(
        CommandLine,
        &[ctrl('u')],
        Action::DeleteToStart,
        "delete to the start",
    ),
    bind(
        CommandLine,
        &[ctrl('k')],
        Action::DeleteToEnd,
        "delete to the end",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::PageUp), ctrl('b')],
        Action::PageUp,
        "scroll the transcript up",
    ),
    bind(
        CommandLine,
        &[plain(KeyCode::PageDown), ctrl('f')],
        Action::PageDown,
        "scroll the transcript down, and follow it at the bottom",
    ),
    bind_when(
        CommandLine,
        &[character('?')],
        Action::Help,
        When::LineEmpty,
        "on an empty line, this help",
    ),
    bind(
        List,
        &[plain(KeyCode::Up), character('k')],
        Action::Up,
        "up one",
    ),
    bind(
        List,
        &[plain(KeyCode::Down), character('j')],
        Action::Down,
        "down one",
    ),
    bind(
        List,
        &[plain(KeyCode::PageUp), ctrl('b')],
        Action::PageUp,
        "up a page",
    ),
    bind(
        List,
        &[plain(KeyCode::PageDown), ctrl('f')],
        Action::PageDown,
        "down a page",
    ),
    bind(
        List,
        &[plain(KeyCode::Home), character('g')],
        Action::Top,
        "the top",
    ),
    bind(
        List,
        &[plain(KeyCode::End), character('G')],
        Action::Bottom,
        "the bottom, and follow",
    ),
    bind(
        Apis,
        &[
            character('1'),
            character('2'),
            character('3'),
            character('4'),
            character('5'),
            character('6'),
            character('7'),
            character('8'),
            character('9'),
        ],
        Action::RunExample(0),
        "run the example with that number",
    ),
    bind(
        Apis,
        &[character('a')],
        Action::RunAllExamples,
        "run every example",
    ),
    bind(
        Apis,
        &[plain(KeyCode::Enter)],
        Action::OpenForm,
        "the parameter form",
    ),
    bind(
        Apis,
        &[character('e')],
        Action::Edit,
        "the first example, on the command line",
    ),
    bind(
        Apis,
        &[character('f')],
        Action::Filter,
        "filter by a part of the name",
    ),
    bind(
        Apis,
        &[character('R')],
        Action::ReloadManifest,
        "ask SOLAR for the manifest again",
    ),
    bind(
        Form,
        &[plain(KeyCode::Tab), plain(KeyCode::Down)],
        Action::NextField,
        "the next field",
    ),
    bind(
        Form,
        &[plain(KeyCode::BackTab), plain(KeyCode::Up)],
        Action::PreviousField,
        "the previous field",
    ),
    bind(Form, &[character(' ')], Action::Toggle, "toggle a boolean"),
    bind(
        Form,
        &[plain(KeyCode::Left)],
        Action::Left,
        "the previous fixed value, or the cursor left in text",
    ),
    bind(
        Form,
        &[plain(KeyCode::Right)],
        Action::Right,
        "the next fixed value, or the cursor right in text",
    ),
    bind(
        Form,
        &[plain(KeyCode::Enter)],
        Action::Run,
        "validate and run",
    ),
    bind(
        Log,
        &[character('e')],
        Action::Level(LevelKey::Error),
        "show errors only",
    ),
    bind(
        Log,
        &[character('w')],
        Action::Level(LevelKey::Warn),
        "warnings and up",
    ),
    bind(
        Log,
        &[character('i')],
        Action::Level(LevelKey::Info),
        "information and up",
    ),
    bind(
        Log,
        &[character('d')],
        Action::Level(LevelKey::Debug),
        "debugging and up",
    ),
    bind(
        Log,
        &[character('t')],
        Action::Level(LevelKey::Trace),
        "everything",
    ),
    bind(
        Log,
        &[plain(KeyCode::Left)],
        Action::LowerLevel,
        "one level fewer",
    ),
    bind(
        Log,
        &[plain(KeyCode::Right)],
        Action::RaiseLevel,
        "one level more",
    ),
    bind(
        Log,
        &[plain(KeyCode::Enter)],
        Action::Expand,
        "the whole line",
    ),
    bind(Log, &[character('c')], Action::Clear, "clear the tab"),
    bind(
        History,
        &[plain(KeyCode::Enter)],
        Action::Run,
        "view the call",
    ),
    bind(
        History,
        &[character('r')],
        Action::RunAgain,
        "send the request again",
    ),
    bind(
        History,
        &[character('e')],
        Action::Edit,
        "the request, on the command line",
    ),
    bind(
        History,
        &[character('x')],
        Action::Export,
        "write this connection as a recording for solar replay",
    ),
    bind(
        Viewer,
        &[plain(KeyCode::Left)],
        Action::PreviousCall,
        "the previous call",
    ),
    bind(
        Viewer,
        &[plain(KeyCode::Right)],
        Action::NextCall,
        "the next call",
    ),
    bind(Viewer, &[character('q'), ctrl('o')], Action::Close, "close"),
    bind(
        HelpPlace,
        &[character('?'), character('q')],
        Action::Close,
        "close",
    ),
    bind(Failure, &[character('r')], Action::Reconnect, "try again"),
    bind(
        Failure,
        &[plain(KeyCode::Enter)],
        Action::Continue,
        "go to the tabs, to read the Log",
    ),
    bind(Failure, &[character('q')], Action::Quit, "quit"),
];

/// The action a key press means in these places, searched in order, with whether the
/// command line is empty for the bindings that depend on it.
#[must_use]
pub fn lookup(places: &[Place], event: &KeyEvent, line_empty: bool) -> Option<Action> {
    for place in places {
        for binding in BINDINGS.iter().filter(|binding| binding.place == *place) {
            let applies = match binding.when {
                When::Always => true,
                When::LineEmpty => line_empty,
                When::LineHasText => !line_empty,
            };
            if !applies {
                continue;
            }
            if let Some(position) = binding.keys.iter().position(|key| key.matches(event)) {
                return Some(match binding.action {
                    // The example keys share one row; the digit says which example.
                    Action::RunExample(_) => {
                        Action::RunExample(u8::try_from(position + 1).unwrap_or(1))
                    }
                    other => other,
                });
            }
        }
    }
    None
}

/// The rows of the help overlay: each place with its keys and what they do.
#[must_use]
pub fn help_rows() -> Vec<(Place, Vec<(String, &'static str)>)> {
    Place::ALL
        .iter()
        .map(|place| {
            let rows = BINDINGS
                .iter()
                .filter(|binding| binding.place == *place)
                .map(|binding| {
                    let keys = if matches!(binding.action, Action::RunExample(_)) {
                        "1 to 9".to_owned()
                    } else {
                        binding
                            .keys
                            .iter()
                            .map(Key::label)
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    (keys, binding.help)
                })
                .collect();
            (*place, rows)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventKind;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: ratatui::crossterm::event::KeyEventState::NONE,
        }
    }

    #[test]
    fn every_action_has_a_plain_or_ctrl_key_and_alt_is_only_ever_extra() {
        for binding in BINDINGS {
            let has_ordinary = binding.keys.iter().any(|key| !key.is_extra());
            // Going to a tab by number is the one action whose keys are all extra, and
            // Tab reaches every tab, which the next test checks.
            if !has_ordinary {
                assert!(
                    matches!(binding.action, Action::GoToTab(_)),
                    "{:?} has only keys some terminal does not deliver",
                    binding.action
                );
            }
        }
    }

    #[test]
    fn every_tab_is_reachable_with_tab_alone() {
        let event = press(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(
            lookup(&[Place::List, Place::Everywhere], &event, true),
            Some(Action::NextTab)
        );
        let back = press(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert_eq!(
            lookup(&[Place::Everywhere], &back, true),
            Some(Action::PreviousTab)
        );
    }

    #[test]
    fn tab_completes_a_line_with_text_and_switches_tabs_on_an_empty_one() {
        let tab = press(KeyCode::Tab, KeyModifiers::NONE);
        let places = [Place::CommandLine, Place::Everywhere];
        assert_eq!(lookup(&places, &tab, false), Some(Action::Complete));
        assert_eq!(lookup(&places, &tab, true), Some(Action::NextTab));
    }

    #[test]
    fn a_question_mark_is_help_on_an_empty_line_and_text_otherwise() {
        let question = press(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(
            lookup(&[Place::CommandLine], &question, true),
            Some(Action::Help)
        );
        assert_eq!(lookup(&[Place::CommandLine], &question, false), None);
    }

    #[test]
    fn control_keys_ignore_the_case_and_plain_keys_do_not() {
        let upper = press(
            KeyCode::Char('O'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(
            lookup(&[Place::Everywhere], &upper, true),
            Some(Action::Envelope)
        );
        let reload = press(KeyCode::Char('R'), KeyModifiers::SHIFT);
        assert_eq!(
            lookup(&[Place::Apis], &reload, true),
            Some(Action::ReloadManifest)
        );
        let lower = press(KeyCode::Char('r'), KeyModifiers::NONE);
        assert_eq!(lookup(&[Place::Apis], &lower, true), None);
    }

    #[test]
    fn a_character_typed_with_altgr_is_text_and_matches_its_plain_key() {
        // What Windows sends for AltGr+W on a Brazilian ABNT2 keyboard.
        let question = press(
            KeyCode::Char('?'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        assert!(is_text(&question));
        assert_eq!(
            lookup(&[Place::CommandLine], &question, true),
            Some(Action::Help)
        );
        let slash = press(
            KeyCode::Char('/'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        assert_eq!(
            lookup(&[Place::Everywhere], &slash, true),
            Some(Action::Slash)
        );
        let control_o = press(KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert!(!is_text(&control_o));
    }

    #[test]
    fn a_digit_on_the_apis_tab_names_its_example() {
        let seven = press(KeyCode::Char('7'), KeyModifiers::NONE);
        assert_eq!(
            lookup(&[Place::Apis], &seven, true),
            Some(Action::RunExample(7))
        );
    }

    #[test]
    fn the_design_document_names_every_key_of_the_table() {
        let design = include_str!("../../../docs/DESIGN.md");
        for binding in BINDINGS {
            for key in binding.keys {
                // The keys that come in runs are written as the run.
                let written = match (key.code, key.modifiers) {
                    (KeyCode::F(_), _) => "`F1` to `F4`".to_owned(),
                    (KeyCode::Char('1'..='9'), KeyModifiers::ALT) => {
                        "`Alt+1` to `Alt+4`".to_owned()
                    }
                    (KeyCode::Char('1'..='9'), _) => "`1` to `9`".to_owned(),
                    _ => format!("`{}`", key.label()),
                };
                assert!(
                    design.contains(&written),
                    "docs/DESIGN.md does not mention {written} of {:?}",
                    binding.action
                );
            }
        }
    }

    #[test]
    fn the_help_lists_every_place_and_every_binding() {
        let rows = help_rows();
        assert_eq!(rows.len(), Place::ALL.len());
        let listed: usize = rows.iter().map(|(_, rows)| rows.len()).sum();
        assert_eq!(listed, BINDINGS.len());
    }
}
