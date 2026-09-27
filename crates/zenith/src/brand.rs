//! The two marks ZENITH draws, in their terminal forms.
//!
//! ZENITH's own mark is the dome of an observatory with its slit open at the zenith, and
//! it is read from `docs/brand` of this repository, whose `README.md` gives its rules.
//! SOLAR's mark is copied byte for byte from SOLAR's `docs/brand` into `assets/solar`,
//! and a test compares the copies with SOLAR's when SOLAR's checkout is available.
//!
//! ZENITH's mark stands for ZENITH, in the opening, and SOLAR's stands for SOLAR, where
//! ZENITH shows SOLAR itself; neither is drawn in the other's place. Both brands pad every
//! line to the symbol's width rather than trusting the file, and start text three columns
//! after the symbol. A mark is never stretched, mirrored or recoloured outside the
//! palette, so this module only hands out the drawings as they are.

use crate::glyphs::Charset;

/// A symbol in its two terminal forms, and where the text beside it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    /// The symbol drawn with half blocks.
    pub banner: &'static str,
    /// The same drawing in 7-bit ASCII.
    pub banner_ascii: &'static str,
    /// The width of the symbol, in cells.
    pub width: usize,
    /// The height of the symbol, in cells.
    pub height: usize,
    /// The row, from zero, that carries the name when text is beside the symbol.
    pub name_row: usize,
}

/// ZENITH's mark, 16 × 4 cells, from `docs/brand` of this repository. The name is level
/// with the top of the dome, where the slit opens.
pub const ZENITH: Mark = Mark {
    banner: include_str!("../../../docs/brand/banner.txt"),
    banner_ascii: include_str!("../../../docs/brand/banner-ascii.txt"),
    width: 16,
    height: 4,
    name_row: 0,
};

/// SOLAR's mark, 16 × 8 cells, from SOLAR's `docs/brand`. The name is on the row whose
/// slot opens to the right.
pub const SOLAR: Mark = Mark {
    banner: include_str!("../assets/solar/banner.txt"),
    banner_ascii: include_str!("../assets/solar/banner-ascii.txt"),
    width: 16,
    height: 8,
    name_row: 2,
};

/// The columns between a symbol and the text beside it, in both brands.
pub const GAP: usize = 3;

impl Mark {
    /// The rows of the symbol, each padded to its width.
    #[must_use]
    pub fn rows(&self, charset: Charset) -> Vec<String> {
        let source = match charset {
            Charset::Unicode => self.banner,
            Charset::Ascii => self.banner_ascii,
        };
        source
            .lines()
            .take(self.height)
            .map(|line| {
                let line = line.trim_end_matches('\r');
                let width = line.chars().count();
                format!("{line}{}", " ".repeat(self.width.saturating_sub(width)))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{COPPER, GOLD, INK, MIST, Rgb};

    const FULL: char = '\u{2588}';
    const UPPER: char = '\u{2580}';
    const LOWER: char = '\u{2584}';

    const SYMBOLS: [(&str, &str); 4] = [
        (
            "gold",
            include_str!("../../../docs/brand/svg/symbol-gold.svg"),
        ),
        (
            "copper",
            include_str!("../../../docs/brand/svg/symbol-copper.svg"),
        ),
        (
            "ink",
            include_str!("../../../docs/brand/svg/symbol-ink.svg"),
        ),
        (
            "white",
            include_str!("../../../docs/brand/svg/symbol-white.svg"),
        ),
    ];
    const SMALL: [(&str, &str); 2] = [
        (
            "gold",
            include_str!("../../../docs/brand/svg/symbol-16px-gold.svg"),
        ),
        (
            "ink",
            include_str!("../../../docs/brand/svg/symbol-16px-ink.svg"),
        ),
    ];
    const LOCKUPS: [(&str, &str); 2] = [
        (
            "dark",
            include_str!("../../../docs/brand/svg/lockup-dark.svg"),
        ),
        (
            "light",
            include_str!("../../../docs/brand/svg/lockup-light.svg"),
        ),
    ];

    fn hex(colour: Rgb) -> String {
        format!("#{:02X}{:02X}{:02X}", colour.0, colour.1, colour.2)
    }

    /// Every `d` and `fill` of an SVG, in order.
    fn attributes<'a>(svg: &'a str, name: &str) -> Vec<&'a str> {
        let marker = format!(" {name}=\"");
        svg.split(marker.as_str())
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .collect()
    }

    /// The pixels of the 16 px drawing, from its path of rectangles `Mx yHx'Vy'Hx Z`.
    fn pixels_of(path: &str) -> [[bool; 16]; 16] {
        let mut grid = [[false; 16]; 16];
        for rectangle in path.split('Z').filter(|part| !part.is_empty()) {
            let numbers: Vec<usize> = rectangle
                .split(|character: char| !character.is_ascii_digit())
                .filter(|part| !part.is_empty())
                .map(|part| part.parse().unwrap())
                .collect();
            let [left, top, right, bottom, again] = numbers[..] else {
                panic!("{rectangle:?} is not a rectangle");
            };
            assert_eq!(left, again, "{rectangle:?} does not close on itself");
            for row in &mut grid[top..bottom] {
                for pixel in &mut row[left..right] {
                    *pixel = true;
                }
            }
        }
        grid
    }

    /// The rule of `docs/brand/README.md`, section 3: the pixels whose centres fall in a
    /// half disc of radius 8 standing on y = 12, less the slit, columns 7 and 8 of rows 4
    /// to 7.
    fn pixels_by_the_rule() -> [[bool; 16]; 16] {
        let mut grid = [[false; 16]; 16];
        for (y, row) in grid.iter_mut().enumerate() {
            for (x, pixel) in row.iter_mut().enumerate() {
                #[allow(clippy::cast_precision_loss, reason = "the coordinates are below 16")]
                let (dx, dy) = (x as f64 + 0.5 - 8.0, y as f64 + 0.5 - 12.0);
                let inside = dy <= 0.0 && dx * dx + dy * dy <= 64.0;
                let slit = (7..=8).contains(&x) && (4..=7).contains(&y);
                *pixel = inside && !slit;
            }
        }
        grid
    }

    fn short(value: f64) -> String {
        let text = format!("{value:.3}");
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    }

    #[test]
    fn each_mark_has_its_size_in_both_forms() {
        for mark in [ZENITH, SOLAR] {
            for charset in [Charset::Unicode, Charset::Ascii] {
                let rows = mark.rows(charset);
                assert_eq!(rows.len(), mark.height);
                for row in &rows {
                    assert_eq!(row.chars().count(), mark.width, "{row:?}");
                }
            }
        }
    }

    #[test]
    fn each_mark_uses_only_the_characters_its_brand_allows() {
        for mark in [ZENITH, SOLAR] {
            for row in mark.rows(Charset::Unicode) {
                assert!(
                    row.chars()
                        .all(|character| [FULL, UPPER, LOWER, ' '].contains(&character)),
                    "{row:?}"
                );
            }
            for row in mark.rows(Charset::Ascii) {
                assert!(row.chars().all(|character| "#'. ".contains(character)));
            }
        }
    }

    #[test]
    fn the_ascii_form_is_the_same_drawing() {
        for mark in [ZENITH, SOLAR] {
            let unicode = mark.rows(Charset::Unicode);
            let ascii = mark.rows(Charset::Ascii);
            for (drawn, written) in unicode.iter().zip(&ascii) {
                let expected: String = drawn
                    .chars()
                    .map(|character| match character {
                        FULL => '#',
                        UPPER => '\'',
                        LOWER => '.',
                        other => other,
                    })
                    .collect();
                assert_eq!(&expected, written);
            }
        }
    }

    #[test]
    fn zeniths_terminal_form_is_its_16_px_drawing_and_both_follow_the_rule() {
        let rule = pixels_by_the_rule();
        for (colour, svg) in SMALL {
            let paths = attributes(svg, "d");
            assert_eq!(paths.len(), 1, "{colour}");
            assert_eq!(pixels_of(paths[0]), rule, "{colour} is not the rule");
        }
        // Every half block is one pixel: row r of the banner is pixel rows 4 + 2r and
        // 5 + 2r, the rows the dome stands in, and nothing is drawn outside them.
        assert!(
            rule[..4]
                .iter()
                .chain(&rule[12..])
                .flatten()
                .all(|pixel| !pixel)
        );
        for (row, drawn) in ZENITH.rows(Charset::Unicode).iter().enumerate() {
            let expected: String = (0..16)
                .map(|x| match (rule[4 + 2 * row][x], rule[5 + 2 * row][x]) {
                    (true, true) => FULL,
                    (true, false) => UPPER,
                    (false, true) => LOWER,
                    (false, false) => ' ',
                })
                .collect();
            assert_eq!(drawn, &expected, "row {row}");
        }
    }

    #[test]
    fn zeniths_slit_opens_at_the_top_on_the_row_that_carries_the_name() {
        let rows = ZENITH.rows(Charset::Unicode);
        let slit = |row: &str| row.chars().skip(7).take(2).collect::<String>();
        assert_eq!(ZENITH.name_row, 0);
        assert_eq!(slit(&rows[0]), "  ");
        assert_eq!(slit(&rows[1]), "  ");
        assert_eq!(slit(&rows[2]), format!("{FULL}{FULL}"));
        // Mirrored about the vertical axis, the dome is the same drawing.
        for row in &rows {
            assert_eq!(&row.chars().rev().collect::<String>(), row);
        }
    }

    #[test]
    fn zeniths_symbol_is_the_geometry_its_readme_gives() {
        // docs/brand/README.md, section 1: m = 5.5; a half disc of radius 4m centred on
        // (24, 35); a slit 1m wide on the vertical axis, from the apex down 2m.
        let m = 5.5_f64;
        let (centre, horizon, radius) = (24.0, 35.0, 4.0 * m);
        let (slit_left, slit_right) = (centre - m / 2.0, centre + m / 2.0);
        let meet = horizon - (radius * radius - (m / 2.0) * (m / 2.0)).sqrt();
        let bottom = horizon - radius + 2.0 * m;
        let r = short(radius);
        let expected = format!(
            "M{} {}A{r} {r} 0 0 1 {} {}V{}H{}V{}A{r} {r} 0 0 1 {} {}Z",
            short(centre - radius),
            short(horizon),
            short(slit_left),
            short(meet),
            short(bottom),
            short(slit_right),
            short(meet),
            short(centre + radius),
            short(horizon),
        );
        for (colour, svg) in SYMBOLS.iter().chain(&LOCKUPS) {
            assert!(svg.contains("viewBox=\"0 0 "), "{colour}");
            assert_eq!(attributes(svg, "d")[0], expected, "{colour}");
        }
    }

    #[test]
    fn zeniths_files_are_in_the_palette_and_one_flat_colour_each() {
        let white = Rgb(0xFF, 0xFF, 0xFF);
        let expected = [
            ("symbol gold", SYMBOLS[0].1, vec![hex(GOLD)]),
            ("symbol copper", SYMBOLS[1].1, vec![hex(COPPER)]),
            ("symbol ink", SYMBOLS[2].1, vec![hex(INK)]),
            ("symbol white", SYMBOLS[3].1, vec![hex(white)]),
            ("16 px gold", SMALL[0].1, vec![hex(GOLD)]),
            ("16 px ink", SMALL[1].1, vec![hex(INK)]),
            ("lockup dark", LOCKUPS[0].1, vec![hex(GOLD), hex(MIST)]),
            ("lockup light", LOCKUPS[1].1, vec![hex(COPPER), hex(INK)]),
        ];
        for (name, svg, fills) in expected {
            assert_eq!(attributes(svg, "fill"), fills, "{name}");
            for forbidden in ["Gradient", "stroke", "filter", "opacity", "<text"] {
                assert!(!svg.contains(forbidden), "{name} has {forbidden}");
            }
        }
    }

    #[test]
    fn solars_name_row_is_the_one_whose_slot_opens_to_the_right() {
        let rows = SOLAR.rows(Charset::Unicode);
        let name_row = &rows[SOLAR.name_row];
        assert!(name_row.starts_with(LOWER));
        assert!(name_row.ends_with("           "));
    }

    #[test]
    fn solars_mark_turned_by_half_a_turn_is_the_same_drawing() {
        // SOLAR's docs/brand/README.md: a 180 degree rotation gives the same drawing. In
        // cells, that is the rows reversed, each reversed, with the half blocks swapped.
        let rows = SOLAR.rows(Charset::Unicode);
        let rotated: Vec<String> = rows
            .iter()
            .rev()
            .map(|row| {
                row.chars()
                    .rev()
                    .map(|character| match character {
                        UPPER => LOWER,
                        LOWER => UPPER,
                        other => other,
                    })
                    .collect()
            })
            .collect();
        assert_eq!(rotated, rows);
    }

    #[test]
    fn the_two_marks_are_not_the_same_drawing() {
        assert_ne!(ZENITH.banner, SOLAR.banner);
        assert_ne!(ZENITH.height, SOLAR.height);
    }

    #[test]
    fn solars_copies_are_byte_for_byte_those_of_solar_when_solar_is_here() {
        let Some(solar) = std::env::var_os("ZENITH_SOLAR_CHECKOUT") else {
            return;
        };
        let brand = std::path::Path::new(&solar).join("docs").join("brand");
        for (name, ours) in [
            ("banner.txt", SOLAR.banner),
            ("banner-ascii.txt", SOLAR.banner_ascii),
        ] {
            let theirs = std::fs::read(brand.join(name)).unwrap();
            assert_eq!(
                ours.as_bytes(),
                theirs.as_slice(),
                "{name} differs from SOLAR's"
            );
        }
    }
}
