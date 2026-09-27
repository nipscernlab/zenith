//! The palette, the three themes, and the four colour depths.
//!
//! Every colour ZENITH draws is one of the five colours of SOLAR's `docs/brand/README.md`,
//! or a mix of two of them, written as the mix. A theme gives each role a colour, and the
//! code draws roles, never colours. The contrast of every role against its background is
//! computed by the tests with the formula of WCAG 2.2, and `docs/DESIGN.md`, section 14,
//! gives the table.
//!
//! When there is no colour at all, because `NO_COLOR` is set, every role is drawn in the
//! terminal's own colours and told apart by bold, underline, dim and reverse, so that
//! colour is never the only carrier of meaning.

use ratatui::style::{Color, Modifier, Style};

/// A colour as red, green and blue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// Gold, the symbol on dark; here, what matters.
pub const GOLD: Rgb = Rgb(0xFF, 0xC2, 0x3D);
/// Copper, the symbol on light.
pub const COPPER: Rgb = Rgb(0xB3, 0x5A, 0x00);
/// Night, the dark background.
pub const NIGHT: Rgb = Rgb(0x0B, 0x0C, 0x14);
/// Mist, the light background and the name on dark.
pub const MIST: Rgb = Rgb(0xF5, 0xF1, 0xE8);
/// Ink, the name on light, and text.
pub const INK: Rgb = Rgb(0x17, 0x16, 0x1C);

impl Rgb {
    /// `self` laid over `under` at `weight`, from 0 to 1: `MIST.over(NIGHT, 0.62)` is
    /// mist at 62% over night.
    #[must_use]
    pub fn over(self, under: Self, weight: f64) -> Self {
        let channel = |a: u8, b: u8| {
            let mixed = f64::from(a) * weight + f64::from(b) * (1.0 - weight);
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the value is rounded and clamped to 0..=255 first"
            )]
            let byte = mixed.round().clamp(0.0, 255.0) as u8;
            byte
        };
        Self(
            channel(self.0, under.0),
            channel(self.1, under.1),
            channel(self.2, under.2),
        )
    }

    /// The relative luminance of WCAG 2.2.
    #[must_use]
    pub fn luminance(self) -> f64 {
        let linear = |channel: u8| {
            let value = f64::from(channel) / 255.0;
            if value <= 0.040_45 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.0) + 0.7152 * linear(self.1) + 0.0722 * linear(self.2)
    }

    /// The contrast ratio of WCAG 2.2 between two colours, from 1 to 21.
    #[must_use]
    pub fn contrast(self, other: Self) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// The nearest colour of the xterm 256-colour palette, from its cube and its grey
    /// ramp. The first sixteen entries are left out, because every terminal draws them
    /// differently.
    #[must_use]
    pub fn nearest_256(self) -> u8 {
        const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        let distance = |a: Self, b: Self| {
            let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).pow(2);
            d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
        };
        let mut best = (u8::MAX, i32::MAX);
        for index in 16..=255_u8 {
            let candidate = if index < 232 {
                let offset = index - 16;
                Self(
                    STEPS[usize::from(offset / 36)],
                    STEPS[usize::from(offset % 36 / 6)],
                    STEPS[usize::from(offset % 6)],
                )
            } else {
                let grey = 8 + (index - 232) * 10;
                Self(grey, grey, grey)
            };
            let score = distance(self, candidate);
            if score < best.1 {
                best = (index, score);
            }
        }
        best.0
    }
}

/// The three themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeName {
    /// The night sky, the default.
    Night,
    /// Ink on mist.
    Light,
    /// The night sky with nothing muted.
    HighContrast,
}

impl ThemeName {
    /// Every theme, in the order `/theme` cycles through them.
    pub const ALL: [Self; 3] = [Self::Night, Self::Light, Self::HighContrast];

    /// The name a person types.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Night => "night",
            Self::Light => "light",
            Self::HighContrast => "high-contrast",
        }
    }

    /// The theme a name stands for.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|theme| theme.name().eq_ignore_ascii_case(name.trim()))
    }

    /// The theme after this one.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Night => Self::Light,
            Self::Light => Self::HighContrast,
            Self::HighContrast => Self::Night,
        }
    }
}

/// How many colours the terminal draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Depth {
    /// Any colour, as red, green and blue.
    TrueColor,
    /// The xterm 256-colour palette.
    Indexed,
    /// The sixteen colours every terminal has, as the terminal draws them.
    Sixteen,
    /// No colour at all: `NO_COLOR`.
    None,
}

impl Depth {
    /// Every depth, in the order the tests draw them.
    pub const ALL: [Self; 4] = [Self::TrueColor, Self::Indexed, Self::Sixteen, Self::None];

    /// The name `--color` takes.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::TrueColor => "truecolor",
            Self::Indexed => "256",
            Self::Sixteen => "16",
            Self::None => "none",
        }
    }

    /// The depth a name stands for.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "truecolor" | "24bit" | "true" => Some(Self::TrueColor),
            "256" => Some(Self::Indexed),
            "16" => Some(Self::Sixteen),
            "none" | "no" | "off" => Some(Self::None),
            _ => None,
        }
    }

    /// The depth this terminal most likely draws, from the variables terminals set.
    ///
    /// `NO_COLOR` is handled by the caller, because an explicit `--color` beats it and
    /// `ZENITH_COLOR` does not (`docs/OPEN_QUESTIONS.md`).
    #[must_use]
    pub fn guess(variable: impl Fn(&str) -> Option<String>) -> Self {
        let has = |name: &str| variable(name).is_some_and(|value| !value.is_empty());
        let colorterm = variable("COLORTERM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let program = variable("TERM_PROGRAM").unwrap_or_default();
        let term = variable("TERM").unwrap_or_default().to_ascii_lowercase();
        if colorterm == "truecolor"
            || colorterm == "24bit"
            || has("WT_SESSION")
            || program == "iTerm.app"
            || program == "vscode"
            || program == "WezTerm"
        {
            Self::TrueColor
        } else if term.contains("256color") || cfg!(windows) {
            Self::Indexed
        } else {
            Self::Sixteen
        }
    }
}

/// The colours of one theme, by role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Behind everything.
    pub background: Rgb,
    /// Text.
    pub text: Rgb,
    /// Secondary text: labels, units, the parts of a line that are not the point.
    pub muted: Rgb,
    /// Borders, separators, stars.
    pub faint: Rgb,
    /// What matters: the active tab, the prompt, headings, names.
    pub accent: Rgb,
    /// Errors.
    pub error: Rgb,
    /// The background of a selected row.
    pub selection: Rgb,
    /// The SOLAR mark: gold on dark, copper on light, as the brand requires.
    pub mark: Rgb,
}

impl Palette {
    /// The palette of a theme.
    #[must_use]
    pub fn of(theme: ThemeName) -> Self {
        match theme {
            ThemeName::Night => Self {
                background: NIGHT,
                text: MIST,
                muted: MIST.over(NIGHT, 0.62),
                faint: MIST.over(NIGHT, 0.38),
                accent: GOLD,
                error: COPPER.over(GOLD, 0.5),
                selection: MIST.over(NIGHT, 0.14),
                mark: GOLD,
            },
            ThemeName::Light => Self {
                background: MIST,
                text: INK,
                muted: INK.over(MIST, 0.72),
                faint: INK.over(MIST, 0.5),
                accent: COPPER.over(INK, 0.75),
                error: COPPER.over(INK, 0.75),
                selection: INK.over(MIST, 0.12),
                mark: COPPER,
            },
            ThemeName::HighContrast => Self {
                background: NIGHT,
                text: MIST,
                muted: MIST,
                faint: MIST,
                accent: GOLD,
                error: GOLD,
                selection: MIST,
                mark: GOLD,
            },
        }
    }
}

/// A theme drawn at a colour depth: the styles every part of the screen is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Which theme.
    pub name: ThemeName,
    /// At which depth.
    pub depth: Depth,
    palette: Palette,
}

/// The sixteen colours each role takes, by theme, when the terminal has only those.
struct Sixteen {
    background: Color,
    text: Color,
    muted: Color,
    accent: Color,
    mark: Color,
}

impl Theme {
    /// A theme at a depth.
    #[must_use]
    pub fn new(name: ThemeName, depth: Depth) -> Self {
        Self {
            name,
            depth,
            palette: Palette::of(name),
        }
    }

    /// The colours, in true colour.
    #[must_use]
    pub fn palette(&self) -> Palette {
        self.palette
    }

    fn sixteen(&self) -> Sixteen {
        match self.name {
            // Gold is yellow, ESC[33m, as SOLAR's brand says for the mark.
            ThemeName::Night => Sixteen {
                background: Color::Black,
                text: Color::Gray,
                muted: Color::DarkGray,
                accent: Color::Yellow,
                mark: Color::Yellow,
            },
            // Yellow on a light background is unreadable in most palettes, and the brand
            // gives no sixteen-colour copper, so the accent is red there; recorded in
            // docs/OPEN_QUESTIONS.md.
            ThemeName::Light => Sixteen {
                background: Color::White,
                text: Color::Black,
                muted: Color::DarkGray,
                accent: Color::Red,
                mark: Color::Red,
            },
            ThemeName::HighContrast => Sixteen {
                background: Color::Black,
                text: Color::White,
                muted: Color::White,
                accent: Color::LightYellow,
                mark: Color::LightYellow,
            },
        }
    }

    fn color(&self, rgb: Rgb, sixteen: Color) -> Color {
        match self.depth {
            Depth::TrueColor => Color::Rgb(rgb.0, rgb.1, rgb.2),
            // The brand gives 215 for gold at this depth, and it is also the nearest.
            Depth::Indexed if rgb == GOLD => Color::Indexed(215),
            Depth::Indexed => Color::Indexed(rgb.nearest_256()),
            Depth::Sixteen => sixteen,
            Depth::None => Color::Reset,
        }
    }

    fn with(&self, rgb: Rgb, sixteen: Color) -> Style {
        Style::new()
            .fg(self.color(rgb, sixteen))
            .bg(self.color(self.palette.background, self.sixteen().background))
    }

    /// Whether this is drawn with no colour at all.
    #[must_use]
    pub fn colourless(&self) -> bool {
        self.depth == Depth::None
    }

    /// Text on the background: what every area is filled with first.
    #[must_use]
    pub fn base(&self) -> Style {
        self.with(self.palette.text, self.sixteen().text)
    }

    /// Text.
    #[must_use]
    pub fn text(&self) -> Style {
        self.base()
    }

    /// Secondary text.
    #[must_use]
    pub fn muted(&self) -> Style {
        self.with(self.palette.muted, self.sixteen().muted)
    }

    /// Borders, separators, dim stars.
    #[must_use]
    pub fn faint(&self) -> Style {
        let style = self.with(self.palette.faint, self.sixteen().muted);
        if self.colourless() {
            style.add_modifier(Modifier::DIM)
        } else {
            style
        }
    }

    /// What matters.
    #[must_use]
    pub fn accent(&self) -> Style {
        let style = self.with(self.palette.accent, self.sixteen().accent);
        if self.colourless() || self.depth == Depth::Sixteen {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }

    /// What matters most: a heading, a name being looked at.
    #[must_use]
    pub fn strong(&self) -> Style {
        self.accent().add_modifier(Modifier::BOLD)
    }

    /// An error's words. Always bold, so that an error is told from the accent without
    /// colour.
    #[must_use]
    pub fn error(&self) -> Style {
        self.with(self.palette.error, self.sixteen().accent)
            .add_modifier(Modifier::BOLD)
    }

    /// A selected row. In the high-contrast theme and without colour it is reverse video;
    /// otherwise a lighter band of the background.
    #[must_use]
    pub fn selection(&self) -> Style {
        match (self.name, self.depth) {
            (ThemeName::HighContrast, _) | (_, Depth::None | Depth::Sixteen) => {
                self.base().add_modifier(Modifier::REVERSED)
            }
            _ => Style::new()
                .fg(self.color(self.palette.text, self.sixteen().text))
                .bg(self.color(self.palette.selection, self.sixteen().muted)),
        }
    }

    /// The `ok` badge: the word in reverse, so that it reads with no colour.
    #[must_use]
    pub fn badge_ok(&self) -> Style {
        self.accent()
            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
    }

    /// The `error` badge.
    #[must_use]
    pub fn badge_error(&self) -> Style {
        self.error().add_modifier(Modifier::REVERSED)
    }

    /// The SOLAR mark: gold on dark, copper on light, the terminal's foreground without
    /// colour, never anything else.
    #[must_use]
    pub fn mark(&self) -> Style {
        self.with(self.palette.mark, self.sixteen().mark)
    }

    /// A star, bright or dim.
    #[must_use]
    pub fn star(&self, bright: bool) -> Style {
        if bright { self.muted() } else { self.faint() }
    }

    /// A key name in a hint, such as `Ctrl+O`.
    #[must_use]
    pub fn key(&self) -> Style {
        self.accent().add_modifier(Modifier::BOLD)
    }

    /// The tab being shown: bold and underlined as well as coloured, so that it stands
    /// out with no colour at all.
    #[must_use]
    pub fn tab_active(&self) -> Style {
        self.accent()
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }

    /// A tab not being shown.
    #[must_use]
    pub fn tab_inactive(&self) -> Style {
        self.muted()
    }

    /// A member name in JSON.
    #[must_use]
    pub fn json_key(&self) -> Style {
        self.accent()
    }

    /// A string in JSON.
    #[must_use]
    pub fn json_string(&self) -> Style {
        self.text()
    }

    /// A number, `true` or `false` in JSON.
    #[must_use]
    pub fn json_scalar(&self) -> Style {
        self.text().add_modifier(Modifier::BOLD)
    }

    /// `null`, and punctuation, in JSON.
    #[must_use]
    pub fn json_quiet(&self) -> Style {
        self.muted()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_roles(palette: &Palette) -> [(&'static str, Rgb); 4] {
        [
            ("text", palette.text),
            ("muted", palette.muted),
            ("accent", palette.accent),
            ("error", palette.error),
        ]
    }

    #[test]
    fn the_brand_contrasts_are_reproduced() {
        // docs/brand/README.md of SOLAR, section 2.
        assert!((GOLD.contrast(NIGHT) - 12.10).abs() < 0.01);
        assert!((MIST.contrast(NIGHT) - 17.30).abs() < 0.01);
        assert!((COPPER.contrast(MIST) - 4.25).abs() < 0.01);
        assert!((INK.contrast(MIST) - 15.95).abs() < 0.01);
    }

    #[test]
    fn every_text_role_of_night_and_light_reaches_4_5_to_1() {
        for theme in [ThemeName::Night, ThemeName::Light] {
            let palette = Palette::of(theme);
            for (role, colour) in text_roles(&palette) {
                let ratio = colour.contrast(palette.background);
                assert!(ratio >= 4.5, "{} {role}: {ratio:.2}", theme.name());
                let on_selection = colour.contrast(palette.selection);
                assert!(
                    on_selection >= 4.5,
                    "{} {role} on selection: {on_selection:.2}",
                    theme.name()
                );
            }
        }
    }

    #[test]
    fn every_text_role_of_high_contrast_reaches_7_to_1() {
        let palette = Palette::of(ThemeName::HighContrast);
        for (role, colour) in text_roles(&palette) {
            let ratio = colour.contrast(palette.background);
            assert!(ratio >= 7.0, "high-contrast {role}: {ratio:.2}");
        }
    }

    #[test]
    fn borders_and_stars_reach_3_to_1_as_components_do() {
        for theme in ThemeName::ALL {
            let palette = Palette::of(theme);
            let ratio = palette.faint.contrast(palette.background);
            assert!(ratio >= 3.0, "{} faint: {ratio:.2}", theme.name());
        }
    }

    #[test]
    fn gold_is_never_drawn_on_the_light_background() {
        let palette = Palette::of(ThemeName::Light);
        assert!(
            !text_roles(&palette)
                .iter()
                .any(|(_, colour)| *colour == GOLD)
        );
        assert_ne!(palette.mark, GOLD);
    }

    #[test]
    fn the_mark_is_gold_on_dark_and_copper_on_light() {
        assert_eq!(Palette::of(ThemeName::Night).mark, GOLD);
        assert_eq!(Palette::of(ThemeName::HighContrast).mark, GOLD);
        assert_eq!(Palette::of(ThemeName::Light).mark, COPPER);
    }

    #[test]
    fn gold_is_215_in_256_colours_and_yellow_in_16_as_the_brand_says() {
        assert_eq!(GOLD.nearest_256(), 215);
        let night = Theme::new(ThemeName::Night, Depth::Indexed);
        assert_eq!(night.mark().fg, Some(Color::Indexed(215)));
        let sixteen = Theme::new(ThemeName::Night, Depth::Sixteen);
        assert_eq!(sixteen.mark().fg, Some(Color::Yellow));
    }

    #[test]
    fn with_no_colour_nothing_is_coloured_and_emphasis_is_kept() {
        let theme = Theme::new(ThemeName::Night, Depth::None);
        for style in [
            theme.base(),
            theme.accent(),
            theme.error(),
            theme.mark(),
            theme.selection(),
        ] {
            assert!(matches!(style.fg, None | Some(Color::Reset)));
            assert!(matches!(style.bg, None | Some(Color::Reset)));
        }
        assert!(theme.accent().add_modifier.contains(Modifier::BOLD));
        assert!(theme.selection().add_modifier.contains(Modifier::REVERSED));
        assert!(
            theme
                .tab_active()
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }

    #[test]
    fn names_are_read_back_and_cycle() {
        for theme in ThemeName::ALL {
            assert_eq!(ThemeName::parse(theme.name()), Some(theme));
        }
        assert_eq!(ThemeName::Night.next().next().next(), ThemeName::Night);
        assert_eq!(ThemeName::parse("sunny"), None);
        for depth in Depth::ALL {
            assert_eq!(Depth::parse(depth.name()), Some(depth));
        }
    }

    #[test]
    fn the_depth_is_guessed_from_what_the_terminal_says() {
        let only = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            }
        };
        assert_eq!(
            Depth::guess(only(&[("COLORTERM", "truecolor")])),
            Depth::TrueColor
        );
        assert_eq!(Depth::guess(only(&[("WT_SESSION", "x")])), Depth::TrueColor);
        assert_eq!(
            Depth::guess(only(&[("TERM_PROGRAM", "iTerm.app")])),
            Depth::TrueColor
        );
        assert_eq!(
            Depth::guess(only(&[("TERM", "xterm-256color")])),
            Depth::Indexed
        );
        let bare = Depth::guess(only(&[("TERM", "xterm")]));
        assert_eq!(
            bare,
            if cfg!(windows) {
                Depth::Indexed
            } else {
                Depth::Sixteen
            }
        );
    }
}
