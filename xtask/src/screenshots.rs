//! `cargo xtask screenshots [--check]`: the README's pictures, drawn from the snapshots.
//!
//! The screens of the README are replays of a session recorded against a real SOLAR,
//! drawn by ZENITH's own drawing code with ratatui's test backend, and snapshotted with
//! the colour of every cell. This task turns those snapshots into SVG files in
//! `docs/screenshots/`, cell for cell and colour for colour. With `--check` it writes
//! nothing and fails when a file differs from what the snapshot draws, or when the README
//! does not show one, which is how CI holds the pictures to the code.

use std::fmt::Write as _;
use std::path::Path;

/// The snapshots the README shows, the file each becomes, and what it shows.
pub(crate) const GALLERY: [(&str, &str); 7] = [
    ("screens__readme_session__100x30__night", "session.svg"),
    (
        "screens__readme_session__100x30__light",
        "session-light.svg",
    ),
    ("screens__readme_apis__100x30__night", "apis.svg"),
    ("screens__readme_log__100x30__night", "log.svg"),
    ("screens__readme_history__100x30__night", "history.svg"),
    ("screens__readme_menu__100x30__night", "completion.svg"),
    (
        "screens__readme_opening__100x30__high-contrast",
        "opening.svg",
    ),
];

const SNAPSHOTS: &str = "crates/zenith/tests/snapshots";
const OUTPUT: &str = "docs/screenshots";

/// Draws every picture of the gallery, or checks that they are drawn.
///
/// # Errors
///
/// A snapshot that is missing or cannot be read, a picture that differs from its snapshot
/// with `--check`, or a picture the README does not show.
pub(crate) fn run(root: &Path, check: bool) -> Result<(), String> {
    let readme = std::fs::read_to_string(root.join("README.md"))
        .map_err(|error| format!("README.md could not be read: {error}"))?;
    let mut stale = Vec::new();
    for (snapshot, file) in GALLERY {
        let path = root.join(SNAPSHOTS).join(format!("{snapshot}.snap"));
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
        let svg = render(&text, snapshot)?;
        let target = root.join(OUTPUT).join(file);
        if check {
            let current = std::fs::read_to_string(&target).unwrap_or_default();
            if current != svg {
                stale.push(format!("{OUTPUT}/{file}"));
            }
        } else {
            std::fs::create_dir_all(root.join(OUTPUT))
                .map_err(|error| format!("{OUTPUT} could not be created: {error}"))?;
            std::fs::write(&target, &svg)
                .map_err(|error| format!("{} could not be written: {error}", target.display()))?;
            println!("screenshots: {OUTPUT}/{file}");
        }
        if !readme.contains(&format!("{OUTPUT}/{file}")) {
            stale.push(format!("README.md does not show {OUTPUT}/{file}"));
        }
    }
    if stale.is_empty() {
        if check {
            println!("screenshots: every picture of the README is its snapshot");
        }
        Ok(())
    } else {
        Err(format!(
            "the README's pictures are not the snapshots: {}. Run `cargo xtask screenshots` \
             and commit what it draws.",
            stale.join(", ")
        ))
    }
}

/// One cell of a screen.
#[derive(Clone)]
struct Cell {
    symbol: String,
    style: Style,
}

/// The style of a cell, as a snapshot lists it.
#[derive(Clone, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one flag for each modifier a snapshot lists, as the snapshot lists them"
)]
struct Style {
    foreground: String,
    background: String,
    bold: bool,
    dim: bool,
    underlined: bool,
    reversed: bool,
}

impl Style {
    fn parse(text: &str, default: &(String, String)) -> Self {
        let mut words = text.split_whitespace();
        let foreground = words.next().unwrap_or("-");
        let _on = words.next();
        let background = words.next().unwrap_or("-");
        let rest: Vec<&str> = words.collect();
        Self {
            foreground: colour(foreground, &default.0),
            background: colour(background, &default.1),
            bold: rest.contains(&"bold"),
            dim: rest.contains(&"dim"),
            underlined: rest.contains(&"underlined"),
            reversed: rest.contains(&"reversed"),
        }
    }

    /// The colours as drawn: reverse video swaps them.
    fn drawn(&self) -> (&str, &str) {
        if self.reversed {
            (&self.background, &self.foreground)
        } else {
            (&self.foreground, &self.background)
        }
    }
}

/// A colour of the snapshot as a hex colour: true colour as it is, the 256-colour palette
/// and the sixteen named colours as xterm draws them, the terminal's own as `default`.
fn colour(text: &str, default: &str) -> String {
    if text.starts_with('#') {
        return text.to_owned();
    }
    if let Some(index) = text
        .strip_prefix('@')
        .and_then(|index| index.parse::<u8>().ok())
    {
        return indexed(index);
    }
    let named = match text {
        "Black" => "#000000",
        "Red" => "#CD0000",
        "Green" => "#00CD00",
        "Yellow" => "#CDCD00",
        "Blue" => "#0000EE",
        "Magenta" => "#CD00CD",
        "Cyan" => "#00CDCD",
        "Gray" => "#E5E5E5",
        "DarkGray" => "#7F7F7F",
        "LightRed" => "#FF0000",
        "LightGreen" => "#00FF00",
        "LightYellow" => "#FFFF00",
        "LightBlue" => "#5C5CFF",
        "LightMagenta" => "#FF00FF",
        "LightCyan" => "#00FFFF",
        "White" => "#FFFFFF",
        _ => default,
    };
    named.to_owned()
}

fn indexed(index: u8) -> String {
    const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let (r, g, b) = match index {
        0..=15 => {
            let base = [
                (0, 0, 0),
                (205, 0, 0),
                (0, 205, 0),
                (205, 205, 0),
                (0, 0, 238),
                (205, 0, 205),
                (0, 205, 205),
                (229, 229, 229),
                (127, 127, 127),
                (255, 0, 0),
                (0, 255, 0),
                (255, 255, 0),
                (92, 92, 255),
                (255, 0, 255),
                (0, 255, 255),
                (255, 255, 255),
            ];
            base[usize::from(index)]
        }
        16..=231 => {
            let offset = index - 16;
            (
                STEPS[usize::from(offset / 36)],
                STEPS[usize::from(offset % 36 / 6)],
                STEPS[usize::from(offset % 6)],
            )
        }
        _ => {
            let grey = 8 + (index - 232) * 10;
            (grey, grey, grey)
        }
    };
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// The width and height a snapshot's name gives, as in `__100x30__`.
fn size(name: &str) -> Option<(usize, usize)> {
    let part = name.split("__").find(|part| {
        part.split_once('x')
            .is_some_and(|(a, b)| a.parse::<usize>().is_ok() && b.parse::<usize>().is_ok())
    })?;
    let (width, height) = part.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn cell_width(symbol: &str) -> usize {
    // East Asian wide characters take two cells; nothing ZENITH draws is wider.
    symbol
        .chars()
        .map(|character| {
            let code = u32::from(character);
            if (0x1100..=0x115F).contains(&code)
                || (0x2E80..=0xA4CF).contains(&code)
                || (0xAC00..=0xD7A3).contains(&code)
                || (0xF900..=0xFAFF).contains(&code)
                || (0xFE30..=0xFE4F).contains(&code)
                || (0xFF00..=0xFF60).contains(&code)
                || (0xFFE0..=0xFFE6).contains(&code)
                || (0x1F300..=0x1FAFF).contains(&code)
            {
                2
            } else {
                1
            }
        })
        .sum()
}

/// A snapshot as the cells of its screen.
fn cells(text: &str, name: &str) -> Result<Vec<Vec<Cell>>, String> {
    let (width, height) =
        size(name).ok_or_else(|| format!("{name} does not say its size, as in __100x30__"))?;
    // insta's header is the lines between the first two `---`.
    let body = text
        .strip_prefix("---")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map_or(text, |(_, body)| body);
    let (screen, styles) = body
        .split_once("--- every cell not listed is ")
        .ok_or_else(|| format!("{name} is not a screen snapshot"))?;
    let mut style_lines = styles.lines();
    let common_text = style_lines.next().unwrap_or("- on -");
    let default = if name.contains("light") {
        ("#17161C".to_owned(), "#F5F1E8".to_owned())
    } else {
        ("#F5F1E8".to_owned(), "#0B0C14".to_owned())
    };
    let common = Style::parse(common_text, &default);
    let blank = Cell {
        symbol: " ".to_owned(),
        style: common.clone(),
    };
    let mut grid = vec![vec![blank; width]; height];
    for (y, row) in screen.lines().take(height).enumerate() {
        let mut x = 0;
        for character in row.chars() {
            if x >= width {
                break;
            }
            let symbol = character.to_string();
            let cells = cell_width(&symbol);
            grid[y][x].symbol = symbol;
            for continuation in 1..cells {
                if x + continuation < width {
                    grid[y][x + continuation].symbol = String::new();
                }
            }
            x += cells;
        }
    }
    for line in style_lines {
        // `{y}:{start}-{end} {style}`, with the numbers padded by spaces.
        let parse = || -> Option<(usize, usize, usize, &str)> {
            let (y, rest) = line.split_once(':')?;
            let (start, rest) = rest.split_once('-')?;
            let rest = rest.trim_start();
            let digits = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let end = rest[..digits].parse().ok()?;
            Some((
                y.trim().parse().ok()?,
                start.trim().parse().ok()?,
                end,
                rest[digits..].trim(),
            ))
        };
        let Some((y, start, end, style)) = parse() else {
            continue;
        };
        let style = Style::parse(style, &default);
        for x in start..=end.min(width.saturating_sub(1)) {
            if let Some(cell) = grid.get_mut(y).and_then(|row| row.get_mut(x)) {
                cell.style = style.clone();
            }
        }
    }
    Ok(grid)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Cells in pixels.
const CELL_WIDTH: f64 = 8.4;
const CELL_HEIGHT: f64 = 17.0;
const PADDING: f64 = 16.0;

/// A count of cells as a float, exactly: a screen is never wider or taller than
/// `u16::MAX` cells.
fn length(count: usize) -> f64 {
    f64::from(u16::try_from(count).unwrap_or(u16::MAX))
}

/// A snapshot as an SVG picture.
///
/// # Errors
///
/// When the snapshot is not a screen snapshot of a known size.
fn render(text: &str, name: &str) -> Result<String, String> {
    let grid = cells(text, name)?;
    let height = grid.len();
    let width = grid.first().map_or(0, Vec::len);
    let page = grid.first().and_then(|row| row.first()).map_or_else(
        || "#0B0C14".to_owned(),
        |cell| cell.style.drawn().1.to_owned(),
    );
    let total_width = PADDING * 2.0 + CELL_WIDTH * length(width);
    let total_height = PADDING * 2.0 + CELL_HEIGHT * length(height);
    let mut svg = String::new();
    let _ = writeln!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{total_width:.0}\" \
         height=\"{total_height:.0}\" viewBox=\"0 0 {total_width:.1} {total_height:.1}\" \
         font-family=\"'Cascadia Mono', Consolas, 'DejaVu Sans Mono', Menlo, monospace\" \
         font-size=\"14\">"
    );
    let _ = writeln!(
        svg,
        "<rect width=\"100%\" height=\"100%\" rx=\"10\" fill=\"{page}\"/>"
    );
    for (y, row) in grid.iter().enumerate() {
        let top = PADDING + CELL_HEIGHT * length(y);
        let mut x = 0;
        while x < width {
            let style = &row[x].style;
            let start = x;
            let mut symbols = String::new();
            let mut positions = Vec::new();
            while x < width && row[x].style == *style {
                for character in row[x].symbol.chars() {
                    symbols.push(character);
                    positions.push(format!("{:.1}", PADDING + CELL_WIDTH * length(x)));
                }
                x += 1;
            }
            let (foreground, background) = style.drawn();
            let left = PADDING + CELL_WIDTH * length(start);
            let span = CELL_WIDTH * length(x - start);
            if background != page {
                let _ = writeln!(
                    svg,
                    "<rect x=\"{left:.1}\" y=\"{top:.1}\" width=\"{span:.1}\" height=\"{CELL_HEIGHT:.1}\" fill=\"{background}\"/>"
                );
            }
            if symbols.trim().is_empty() {
                continue;
            }
            let mut attributes = format!("fill=\"{foreground}\"");
            if style.bold {
                attributes.push_str(" font-weight=\"700\"");
            }
            if style.dim {
                attributes.push_str(" opacity=\"0.6\"");
            }
            if style.underlined {
                attributes.push_str(" text-decoration=\"underline\"");
            }
            let baseline = top + 13.0;
            // Every character is placed at its own cell, so that the box drawing lines up
            // whatever font the reader has and whatever program draws the picture.
            let _ = writeln!(
                svg,
                "<text x=\"{}\" y=\"{baseline:.1}\" xml:space=\"preserve\" {attributes}>{}</text>",
                positions.join(" "),
                escape(&symbols)
            );
        }
    }
    svg.push_str("</svg>\n");
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT: &str = "---\nsource: x\nexpression: y\n---\n ab\n--- every cell not listed is #F5F1E8 on #0B0C14\n 0:  1-1   #FFC23D on #0B0C14 bold\n";

    #[test]
    fn a_snapshot_becomes_cells_with_their_styles() {
        let grid = cells(SNAPSHOT, "x__3x1__night").unwrap();
        assert_eq!(grid[0][1].symbol, "a");
        assert_eq!(grid[0][1].style.foreground, "#FFC23D");
        assert!(grid[0][1].style.bold);
        assert!(!grid[0][2].style.bold);
    }

    #[test]
    fn the_picture_places_every_run_of_text_in_its_cells() {
        let svg = render(SNAPSHOT, "x__3x1__night").unwrap();
        assert!(svg.contains(">a</text>"));
        assert!(svg.contains("font-weight=\"700\""));
        assert!(svg.contains(">b</text>"));
    }

    #[test]
    fn colours_of_every_depth_are_turned_into_hex() {
        assert_eq!(colour("@215", "-"), "#FFAF5F");
        assert_eq!(colour("Yellow", "-"), "#CDCD00");
        assert_eq!(colour("-", "#123456"), "#123456");
        assert_eq!(indexed(232), "#080808");
    }

    #[test]
    fn the_size_comes_from_the_name() {
        assert_eq!(
            size("screens__readme_session__100x30__night"),
            Some((100, 30))
        );
        assert_eq!(size("nothing"), None);
    }
}
