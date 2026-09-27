# The ZENITH mark

**Status:** drawn on 27 September 2026, without a designer, on the rules of SOLAR's brand;
[`docs/OPEN_QUESTIONS.md`](../OPEN_QUESTIONS.md) records it among what was decided
without asking. **Source:** this folder.

This folder is the single source for the ZENITH symbol, its colours and its terminal
form. Code that draws the mark reads from here: `crates/zenith/src/brand.rs` includes
`banner.txt` and `banner-ascii.txt`, and its tests read every file in `svg/`. Nothing here
is generated at build time.

## 1. Symbol

The dome of an observatory standing on the horizon, with its slit open straight up, at
the zenith: the highest point of the sky, which ZENITH is named after.

Geometry, in the 48 × 48 viewBox of every file in `svg/`, on the module of SOLAR's mark:

| Rule | Value |
| --- | --- |
| Module | m = 5.5, SOLAR's |
| Dome | a half disc standing on the horizon y = 35, centre (24, 35), radius 4m = 22: from (2, 35) to (46, 35), apex at (24, 13) |
| Slit | 1m wide, centred on the vertical axis, from the apex down 2m, to y = 24, the centre of the box |
| Clear space in the box | 13 above the apex and 13 below the horizon, so the dome is centred |
| Symmetry | a mirror image about the vertical axis is the same drawing. The dome is never turned: its slit points at the zenith, and on its side or upside down it points nowhere |

The path of every symbol file is therefore
`M2 35A22 22 0 0 1 21.25 13.173V24H26.75V13.173A22 22 0 0 1 46 35Z`.

The mark is a single flat colour. It has no gradient, outline, glow or shadow, and it is
never stretched, turned or enclosed in a ring.

| File | Use |
| --- | --- |
| `svg/symbol-gold.svg` | on dark backgrounds, 24 px and up |
| `svg/symbol-copper.svg` | on light backgrounds, 24 px and up |
| `svg/symbol-ink.svg`, `svg/symbol-white.svg` | one-colour print and engraving |
| `svg/symbol-16px-gold.svg`, `svg/symbol-16px-ink.svg` | below 24 px: a separate pixel drawing, m = 2 px, `shape-rendering="crispEdges"` |
| `svg/lockup-dark.svg`, `svg/lockup-light.svg` | symbol with the name |

In the lockups the name is Martian Mono SemiBold, the named instance of the variable font
at weight 600 and width 100 (version 1.000, SIL OFL 1.1, from
[evilmartians/mono](https://github.com/evilmartians/mono)), converted to outlines, so no
font is needed to render them. The cap height runs from the apex of the dome to the
horizon, 4m, and the name starts at x = 58, where SOLAR's lockup starts its name. Keep 2m
clear around the lockup.

### It is never SOLAR's mark

SOLAR's mark is a whole disc cut by two horizontal slots. ZENITH's is half a disc cut by
one vertical slit. They share the module and the palette, so that they belong together,
and nothing else, so that one is never taken for the other.

- ZENITH's mark stands for ZENITH, and SOLAR's for SOLAR. Neither is drawn in the other's
  place (section 4).
- They are never combined into one drawing: not one inside, over or under the other, and
  not a dome on SOLAR's disc.
- Neither is altered to look like the other: no slots in the dome, no slit in the disc.
- SOLAR's mark is SOLAR's. ZENITH copies it byte for byte into
  `crates/zenith/assets/solar/`, a test compares the copies with SOLAR's, and nothing in
  this repository changes it or draws a variant of it.

## 2. Colours

The five colours of SOLAR's brand, in the same roles. ZENITH has no colour of its own.

| Name | Hex | Role |
| --- | --- | --- |
| Gold | `#FFC23D` | the symbol on dark |
| Copper | `#B35A00` | the symbol on light, one-colour print |
| Night | `#0B0C14` | dark background |
| Mist | `#F5F1E8` | light background, the name on dark |
| Ink | `#17161C` | the name on light, text, one-colour print |

WCAG contrast, computed: gold on night 12.10:1, mist on night 17.30:1, copper on mist
4.25:1, copper on white 4.80:1, ink on mist 15.95:1. Gold on a light background is 1.43:1
on mist and 1.61:1 on white, and is never used. The tests of `crates/zenith/src/theme.rs`
compute gold on night, mist on night, copper on mist and ink on mist again from the hex
values.

## 3. Terminal

`banner.txt` is the symbol in 16 × 4 terminal cells. It uses only `█`, `▀`, `▄` and the
space, and every half block is one pixel of the 16 px icon: row r of the banner is pixel
rows 4 + 2r and 5 + 2r, the rows the dome stands in. Every line is 16 characters wide,
trailing spaces included. Editors tend to strip those spaces, so code that reads the file
pads each line to 16 on the right instead of trusting it. `banner-ascii.txt` is the same
drawing in 7-bit ASCII, with `#` for a whole cell, `.` for a lower half and `'` for an
upper half, the characters of SOLAR's ASCII banner.

The 16 px drawing is its own, not the large one scaled: the pixels whose centres fall
inside a half disc of radius 8 standing on y = 12 and centred on x = 8, less the slit,
columns 7 and 8 of rows 4 to 7.

With text beside it, leave three spaces after the symbol, so text starts at column 19
(0-based). The four lines of text sit level with the four rows of the symbol: the name on
the first, level with the top of the dome where the slit opens, then what ZENITH is, the
laboratory and the version.

```text
   ▄▄██  ██▄▄      ZENITH
 ▄█████  █████▄    The terminal of Constellation
▄██████████████▄   NIPS-CERN
████████████████   0.2.0
```

Rules for the terminal:

1. **Only in output meant for a person.** ZENITH draws the mark on its full screen, which
   it opens only on a terminal. `zenith --version` prints one line of text and no mark.
2. **One colour for the whole symbol:** gold on a dark background, copper on a light one,
   and the terminal's own foreground when there is no colour, with `NO_COLOR` set or
   `--color none` ([no-color.org](https://no-color.org)).
3. **At fewer colours.** Gold is `215` at 256 colours and yellow at 16, as SOLAR's brand
   says, and bright yellow at 16 in the high-contrast theme. Copper is `130` at 256
   colours, the nearest entry of the xterm cube, and red at 16, because SOLAR's brand
   gives copper no 16-colour value and yellow on a light background is unreadable in most
   palettes.
4. **Version.** The version line comes from `env!("CARGO_PKG_VERSION")`, never from a
   literal.

## 4. Where ZENITH draws each mark

| Where | Mark |
| --- | --- |
| The opening, while SOLAR starts | ZENITH's, with the four lines of section 3, in a starfield that keeps two cells clear around it |
| The start of every connection, in the Session tab, where ZENITH shows SOLAR itself | SOLAR's, laid out as SOLAR's `docs/brand/README.md` lays it out, with the version that answered the handshake |
| The card of `/version`, where ZENITH shows SOLAR itself | SOLAR's, laid out the same way, with the version of the SOLAR that answered |

## 5. How the files were made, and what checks them

The symbol files carry the path of section 1, computed from its geometry. The 16 px files
are the rule of section 3, drawn as rectangles of whole pixels. The name in the lockups is
Martian Mono's outlines, instanced at SemiBold with fontTools, the y axis flipped about
the cap height of 800 units and scaled by 0.0275, which is 22 ÷ 800.

The tests of `crates/zenith/src/brand.rs` hold the files to these rules: the path of every
symbol and lockup file is computed again from the numbers of section 1; the pixels of the
16 px files are the rule, and `banner.txt` is those pixels; `banner-ascii.txt` is
`banner.txt` with the characters of section 3; every file is filled with the colours of
section 2 and nothing else, with no gradient, stroke, filter, opacity or text.

## 6. Open items

- No trademark search has been made. Search INPI and the WIPO Global Brand Database
  before registering the mark or printing it on hardware.
- The mark was drawn without a designer. A designer may redraw it on these rules, and the
  tests of section 5 say whether a new drawing keeps them.
