//! The SOLAR mark in its terminal forms, exactly as SOLAR's `docs/brand/README.md` says.
//!
//! `assets/brand/banner.txt` and `banner-ascii.txt` are copied byte for byte from SOLAR's
//! `docs/brand`, and a test compares them with SOLAR's when SOLAR's checkout is available.
//! The rules followed here are the brand's: the symbol is 16 × 8 cells, every line is
//! padded to 16 on the right rather than trusting the file, text beside it starts three
//! columns after it, and the line whose slot opens to the right carries the name. The
//! mark is never stretched, mirrored or recoloured outside the palette, so this module
//! only ever hands out the drawing as it is.

use crate::glyphs::Charset;

/// The symbol drawn with half blocks, from SOLAR's `docs/brand/banner.txt`.
pub const BANNER: &str = include_str!("../assets/brand/banner.txt");

/// The same drawing in 7-bit ASCII, from SOLAR's `docs/brand/banner-ascii.txt`.
pub const BANNER_ASCII: &str = include_str!("../assets/brand/banner-ascii.txt");

/// The width of the symbol, in cells.
pub const WIDTH: usize = 16;

/// The height of the symbol, in cells.
pub const HEIGHT: usize = 8;

/// The columns between the symbol and the text beside it.
pub const GAP: usize = 3;

/// The row, from zero, whose slot opens to the right and carries the name.
pub const NAME_ROW: usize = 2;

/// The eight rows of the symbol, each padded to sixteen cells.
#[must_use]
pub fn rows(charset: Charset) -> [String; HEIGHT] {
    let source = match charset {
        Charset::Unicode => BANNER,
        Charset::Ascii => BANNER_ASCII,
    };
    let mut rows: [String; HEIGHT] = Default::default();
    for (row, line) in rows.iter_mut().zip(source.lines()) {
        let line = line.trim_end_matches('\r');
        let width = line.chars().count();
        *row = format!("{line}{}", " ".repeat(WIDTH.saturating_sub(width)));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_symbol_is_sixteen_by_eight_in_both_forms() {
        for charset in [Charset::Unicode, Charset::Ascii] {
            let rows = rows(charset);
            assert_eq!(rows.len(), HEIGHT);
            for row in &rows {
                assert_eq!(row.chars().count(), WIDTH, "{row:?}");
            }
        }
    }

    #[test]
    fn the_symbol_uses_only_the_characters_the_brand_allows() {
        for row in rows(Charset::Unicode) {
            assert!(
                row.chars()
                    .all(|character| "\u{2588}\u{2580}\u{2584} ".contains(character))
            );
        }
        for row in rows(Charset::Ascii) {
            assert!(row.is_ascii());
        }
    }

    #[test]
    fn the_name_row_is_the_one_whose_slot_opens_to_the_right() {
        let rows = rows(Charset::Unicode);
        let name_row = &rows[NAME_ROW];
        assert!(name_row.starts_with('\u{2584}'));
        assert!(name_row.ends_with("           "));
    }

    #[test]
    fn a_rotation_by_half_a_turn_gives_the_same_drawing() {
        // docs/brand/README.md: a 180 degree rotation gives the same drawing. In cells,
        // that is the rows reversed, each reversed, with the half blocks swapped.
        let rows = rows(Charset::Unicode);
        let rotated: Vec<String> = rows
            .iter()
            .rev()
            .map(|row| {
                row.chars()
                    .rev()
                    .map(|character| match character {
                        '\u{2580}' => '\u{2584}',
                        '\u{2584}' => '\u{2580}',
                        other => other,
                    })
                    .collect()
            })
            .collect();
        assert_eq!(rotated, rows.to_vec());
    }

    #[test]
    fn the_copies_are_byte_for_byte_those_of_solar_when_solar_is_here() {
        let Some(solar) = std::env::var_os("ZENITH_SOLAR_CHECKOUT") else {
            return;
        };
        let brand = std::path::Path::new(&solar).join("docs").join("brand");
        for (name, ours) in [("banner.txt", BANNER), ("banner-ascii.txt", BANNER_ASCII)] {
            let theirs = std::fs::read(brand.join(name)).unwrap();
            assert_eq!(
                ours.as_bytes(),
                theirs.as_slice(),
                "{name} differs from SOLAR's"
            );
        }
    }
}
