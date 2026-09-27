//! The characters ZENITH draws with: by default only those the fonts of the classic
//! Windows console have, and with `--ascii` only 7-bit ASCII.
//!
//! Rounded corners, check marks and emoji are left out on purpose: a missing glyph is a
//! box of garbage in somebody's terminal. What is here is in WGL4, the character set that
//! Consolas, Lucida Console and Cascadia Mono all cover, and that every terminal font of
//! macOS and Linux covers too. The old raster fonts of the console have less, which is
//! what `--ascii` is for.

use ratatui::symbols::border;

/// Which set of characters to draw with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Charset {
    /// Box drawing, blocks and a few shapes.
    Unicode,
    /// 7-bit ASCII only.
    Ascii,
}

/// The characters of one set.
#[derive(Debug, Clone, Copy)]
pub struct Glyphs {
    /// Which set.
    pub charset: Charset,
    /// The prompt of the command line, and the marker of a selected row.
    pub prompt: &'static str,
    /// SOLAR when connected.
    pub body: &'static str,
    /// SOLAR when not connected.
    pub hollow: &'static str,
    /// The orbit the status bar is drawn as.
    pub orbit: &'static str,
    /// The separator between items on one line.
    pub dot: &'static str,
    /// What stands for text that was cut.
    pub ellipsis: &'static str,
    /// Stars, dim to bright.
    pub stars: [&'static str; 4],
    /// Scroll markers.
    pub up: &'static str,
    /// Scroll markers.
    pub down: &'static str,
    /// The unit of microseconds.
    pub micro: &'static str,
    /// The borders of boxes.
    pub border: border::Set<'static>,
    /// A horizontal rule.
    pub rule: &'static str,
    /// A vertical rule.
    pub bar: &'static str,
    /// The arrow keys, as the help overlay writes them.
    pub arrows: [&'static str; 4],
}

/// Box drawing with straight corners, which WGL4 has and rounded corners are not.
const UNICODE_BORDER: border::Set<'static> = border::PLAIN;

/// Boxes in ASCII.
const ASCII_BORDER: border::Set<'static> = border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

impl Glyphs {
    /// The characters of a set.
    #[must_use]
    pub fn of(charset: Charset) -> Self {
        match charset {
            Charset::Unicode => Self {
                charset,
                prompt: "\u{203a}",
                body: "\u{25cf}",
                hollow: "\u{25cb}",
                orbit: "\u{2500}",
                dot: "\u{b7}",
                ellipsis: "\u{2026}",
                stars: ["\u{b7}", "\u{2219}", "\u{2022}", "*"],
                up: "\u{25b2}",
                down: "\u{25bc}",
                micro: "\u{b5}s",
                border: UNICODE_BORDER,
                rule: "\u{2500}",
                bar: "\u{2502}",
                arrows: ["\u{2190}", "\u{2191}", "\u{2192}", "\u{2193}"],
            },
            Charset::Ascii => Self {
                charset,
                prompt: ">",
                body: "*",
                hollow: "o",
                orbit: "-",
                dot: "-",
                ellipsis: "...",
                stars: [".", "'", "+", "*"],
                up: "^",
                down: "v",
                micro: "us",
                border: ASCII_BORDER,
                rule: "-",
                bar: "|",
                arrows: ["Left", "Up", "Right", "Down"],
            },
        }
    }

    /// Replaces the arrows and other symbols in a key name with this set's.
    #[must_use]
    pub fn key_name(&self, name: &str) -> String {
        if self.charset == Charset::Unicode {
            return name.to_owned();
        }
        name.replace('\u{2190}', "Left")
            .replace('\u{2191}', "Up")
            .replace('\u{2192}', "Right")
            .replace('\u{2193}', "Down")
    }
}

/// The characters beyond ASCII that the Unicode set uses, every one of them in WGL4, which
/// the tests check the set against.
pub const WGL4_USED: &str = "\u{203a}\u{25cf}\u{25cb}\u{2500}\u{b7}\u{2026}\u{2219}\
    \u{2022}\u{25b2}\u{25bc}\u{b5}\u{2502}\u{250c}\u{2510}\u{2514}\u{2518}\u{2190}\u{2191}\
    \u{2192}\u{2193}\u{2588}\u{2580}\u{2584}\u{2551}\u{2550}";

#[cfg(test)]
mod tests {
    use super::*;

    fn every_string(glyphs: &Glyphs) -> Vec<&'static str> {
        let mut all = vec![
            glyphs.prompt,
            glyphs.body,
            glyphs.hollow,
            glyphs.orbit,
            glyphs.dot,
            glyphs.ellipsis,
            glyphs.up,
            glyphs.down,
            glyphs.micro,
            glyphs.rule,
            glyphs.bar,
            glyphs.border.top_left,
            glyphs.border.top_right,
            glyphs.border.bottom_left,
            glyphs.border.bottom_right,
            glyphs.border.vertical_left,
            glyphs.border.horizontal_top,
        ];
        all.extend(glyphs.stars);
        all.extend(glyphs.arrows);
        all
    }

    #[test]
    fn the_ascii_set_is_seven_bit() {
        for text in every_string(&Glyphs::of(Charset::Ascii)) {
            assert!(text.is_ascii(), "{text:?} is not ASCII");
        }
    }

    #[test]
    fn the_unicode_set_uses_only_the_wgl4_characters_listed() {
        for text in every_string(&Glyphs::of(Charset::Unicode)) {
            for character in text.chars() {
                assert!(
                    character.is_ascii() || WGL4_USED.contains(character),
                    "{character:?} (U+{:04X}) is not in the list of WGL4 characters in use",
                    u32::from(character)
                );
            }
        }
    }

    #[test]
    fn key_names_lose_their_arrows_in_ascii() {
        let ascii = Glyphs::of(Charset::Ascii);
        assert_eq!(ascii.key_name("\u{2190} \u{2192}"), "Left Right");
        let unicode = Glyphs::of(Charset::Unicode);
        assert_eq!(unicode.key_name("\u{2191}"), "\u{2191}");
    }
}
