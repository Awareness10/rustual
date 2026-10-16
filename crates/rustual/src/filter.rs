//! Output filters applied to every frame (port of `textual/filter.py`).
//!
//! - Unless the app uses native ANSI colors, ANSI colors (`ansi_red`, `ansi_default`, ...) are
//!   converted to truecolor with a terminal theme (`ANSIToTruecolor`), and `dim` text becomes a
//!   color blended towards its background (terminals render `dim` inconsistently).
//! - With `NO_COLOR` set, colors are removed: the terminal's default colors with native ANSI
//!   colors (`NoColor`), a gray scale otherwise (`Monochrome`).

use rustual_render::{Attrs, Canvas, Color, TerminalTheme};

/// Factor for converting `dim` to a color: 0 is the background, 1 the text color
/// (`TEXTUAL_DIM_FACTOR` default).
pub const DIM_FACTOR: f32 = 0.66;

/// How the frame is written to the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutputMode {
    /// Write ANSI colors as ANSI codes (`App.native_ansi_color`).
    pub native_ansi: bool,
    /// The theme is dark (selects the terminal theme for truecolor conversion).
    pub dark: bool,
    /// `NO_COLOR` is set.
    pub no_color: bool,
}

/// The terminal theme used to show ANSI colors as truecolor (Textual's `ansi_theme_dark` /
/// `ansi_theme_light`).
pub fn terminal_theme(dark: bool) -> &'static TerminalTheme {
    if dark {
        &TerminalTheme::MONOKAI
    } else {
        &TerminalTheme::ALABASTER
    }
}

/// Apply the filters for `mode` to every cell.
pub fn apply(canvas: &mut Canvas, mode: OutputMode) {
    if mode.native_ansi && !mode.no_color {
        return;
    }
    let theme = terminal_theme(mode.dark);
    for_each_cell(canvas, |fg, bg, attrs| {
        if !mode.native_ansi {
            truecolor(fg, bg, attrs, theme);
        }
        if mode.no_color {
            if mode.native_ansi {
                *fg = Color::ANSI_DEFAULT;
                *bg = Color::ANSI_DEFAULT;
            } else {
                *fg = fg.monochrome();
                *bg = bg.monochrome();
            }
        }
    });
}

/// Convert every remaining ANSI color to truecolor for a screenshot, which has no terminal
/// palette: with Rich's SVG export theme, as Textual's `export_screenshot` does.
pub fn to_truecolor(canvas: &mut Canvas) {
    let theme = &TerminalTheme::SVG_EXPORT;
    for_each_cell(canvas, |fg, bg, _| {
        *fg = fg.to_truecolor(theme, true);
        *bg = bg.to_truecolor(theme, false);
    });
}

/// `ANSIToTruecolor.truecolor_style` for one cell.
fn truecolor(fg: &mut Color, bg: &mut Color, attrs: &mut Attrs, theme: &TerminalTheme) {
    *fg = fg.to_truecolor(theme, true);
    *bg = bg.to_truecolor(theme, false);
    if attrs.contains(Attrs::DIM) {
        *fg = dim_color(*bg, *fg);
        attrs.remove(Attrs::DIM);
    }
}

/// `dim_color`: `color` blended towards `background` by [`DIM_FACTOR`].
fn dim_color(background: Color, color: Color) -> Color {
    let mix = |b: u8, c: u8| (b as f32 + (c as f32 - b as f32) * DIM_FACTOR) as u8;
    Color::rgba(
        mix(background.r, color.r),
        mix(background.g, color.g),
        mix(background.b, color.b),
        color.a,
    )
}

fn for_each_cell(canvas: &mut Canvas, mut f: impl FnMut(&mut Color, &mut Color, &mut Attrs)) {
    let size = canvas.size();
    for y in 0..size.height {
        for x in 0..size.width {
            if let Some(cell) = canvas.get_mut(x, y) {
                f(&mut cell.fg, &mut cell.bg, &mut cell.attrs);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustual_render::{Cell, Size};

    fn canvas(fg: Color, bg: Color, attrs: Attrs) -> Canvas {
        let mut c = Canvas::new(Size::new(1, 1), bg);
        *c.get_mut(0, 0).unwrap() = Cell {
            fg,
            bg,
            attrs,
            ..Cell::blank(bg)
        };
        c
    }

    #[test]
    fn ansi_to_truecolor() {
        let mode = OutputMode {
            dark: true,
            ..Default::default()
        };
        let mut c = canvas(Color::from_ansi(1), Color::ANSI_DEFAULT, Attrs::NONE);
        apply(&mut c, mode);
        let cell = c.get(0, 0).unwrap();
        assert_eq!(cell.fg, Color::rgb(244, 0, 95)); // MONOKAI red
        assert_eq!(cell.bg, Color::rgb(12, 12, 12)); // MONOKAI background
                                                     // Light themes use ALABASTER; the default foreground is the theme's.
        let mut c = canvas(Color::ANSI_DEFAULT, Color::from_ansi(4), Attrs::NONE);
        apply(&mut c, OutputMode::default());
        let cell = c.get(0, 0).unwrap();
        assert_eq!(
            (cell.fg, cell.bg),
            (Color::rgb(0, 0, 0), Color::rgb(50, 92, 192))
        );
    }

    #[test]
    fn native_ansi_keeps_colors() {
        let mode = OutputMode {
            native_ansi: true,
            ..Default::default()
        };
        let mut c = canvas(Color::from_ansi(2), Color::ANSI_DEFAULT, Attrs::DIM);
        apply(&mut c, mode);
        let cell = c.get(0, 0).unwrap();
        assert_eq!(cell.fg.ansi, Some(2));
        assert!(cell.attrs.contains(Attrs::DIM));
    }

    #[test]
    fn dim_becomes_a_color() {
        let mut c = canvas(Color::rgb(200, 100, 0), Color::rgb(0, 0, 100), Attrs::DIM);
        apply(&mut c, OutputMode::default());
        let cell = c.get(0, 0).unwrap();
        assert_eq!(cell.fg, Color::rgb(132, 66, 34));
        assert!(!cell.attrs.contains(Attrs::DIM));
    }

    #[test]
    fn no_color() {
        let mut c = canvas(Color::rgb(255, 0, 0), Color::rgb(0, 0, 255), Attrs::NONE);
        apply(
            &mut c,
            OutputMode {
                no_color: true,
                ..Default::default()
            },
        );
        let cell = c.get(0, 0).unwrap();
        assert_eq!(
            (cell.fg, cell.bg),
            (Color::rgb(54, 54, 54), Color::rgb(18, 18, 18))
        );
        let mut c = canvas(Color::rgb(255, 0, 0), Color::from_ansi(4), Attrs::NONE);
        apply(
            &mut c,
            OutputMode {
                no_color: true,
                native_ansi: true,
                ..Default::default()
            },
        );
        let cell = c.get(0, 0).unwrap();
        assert_eq!(
            (cell.fg, cell.bg),
            (Color::ANSI_DEFAULT, Color::ANSI_DEFAULT)
        );
    }
}
