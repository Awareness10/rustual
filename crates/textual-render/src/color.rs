//! RGBA colors with CSS-style parsing and blending (port of `textual/color.py`).

use std::fmt;

/// An RGB color with a floating point alpha in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorParseError(pub String);

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid color: {}", self.0)
    }
}

impl std::error::Error for ColorParseError {}

impl Default for Color {
    fn default() -> Self {
        Color::TRANSPARENT
    }
}

impl Color {
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0.0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Color { r, g, b, a }
    }

    /// Parse a CSS color: named colors (Textual's table incl. `ansi_*`), `#rgb`, `#rgba`,
    /// `#rrggbb`, `#rrggbbaa`, `rgb(r,g,b)`, `rgba(r,g,b,a)`, `hsl(h,s%,l%)`, `hsla(...)`,
    /// `transparent`. Case-insensitive, surrounding whitespace ignored.
    pub fn parse(s: &str) -> Result<Color, ColorParseError> {
        todo!("color parse: {s}")
    }

    /// Create from HSL (h in 0..360 degrees, s and l in 0..1).
    pub fn from_hsl(h: f32, s: f32, l: f32) -> Color {
        todo!("{h}{s}{l}")
    }

    /// (h degrees 0..360, s 0..1, l 0..1)
    pub fn to_hsl(&self) -> (f32, f32, f32) {
        todo!()
    }

    pub fn with_alpha(&self, a: f32) -> Color {
        Color { a, ..*self }
    }

    pub fn multiply_alpha(&self, f: f32) -> Color {
        Color { a: self.a * f, ..*self }
    }

    pub fn is_transparent(&self) -> bool {
        self.a <= 0.0
    }

    /// Linear interpolation of all channels (including alpha) towards `dest` by `factor` (0..1).
    pub fn blend(&self, dest: Color, factor: f32) -> Color {
        todo!("{dest}{factor}")
    }

    /// Composite `top` over `self` (self is the background), using `top.a` as the blend factor.
    /// The result keeps `self`'s alpha. Equivalent to Textual's `self + top`.
    pub fn composite(&self, top: Color) -> Color {
        todo!("{top}")
    }

    /// Tint: blend RGB towards `tint`'s RGB by `tint.a`, keeping own alpha.
    pub fn tint(&self, tint: Color) -> Color {
        todo!("{tint}")
    }

    /// Lighten by `amount` (0..1) in CIE-L*ab lightness like Textual.
    pub fn lighten(&self, amount: f32) -> Color {
        todo!("{amount}")
    }

    pub fn darken(&self, amount: f32) -> Color {
        self.lighten(-amount)
    }

    pub fn inverse(&self) -> Color {
        Color::rgba(255 - self.r, 255 - self.g, 255 - self.b, self.a)
    }

    /// Perceived brightness 0..1.
    pub fn brightness(&self) -> f32 {
        todo!()
    }

    /// Contrasting text color (white or black) with given alpha, as in Textual.
    pub fn get_contrast_text(&self, alpha: f32) -> Color {
        todo!("{alpha}")
    }

    /// `#RRGGBB` or `#RRGGBBAA` when alpha < 1.
    pub fn hex(&self) -> String {
        todo!()
    }

    /// CSS string, e.g. `rgb(10,20,30)` or `rgba(10,20,30,0.5)`.
    pub fn css(&self) -> String {
        todo!()
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rgba({},{},{},{})", self.r, self.g, self.b, self.a)
    }
}
