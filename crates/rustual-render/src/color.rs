//! RGBA colors with CSS-style parsing and blending (port of `textual/color.py`).

use std::fmt;

/// An RGB color with a floating point alpha in `0.0..=1.0`.
///
/// A color may also be one of the terminal's 16 ANSI colors (`ansi_red`, ...) or its default
/// color (`ansi_default`), as in Textual: [`Color::ansi`] is then `Some(index)` (`-1` for the
/// default) and `r`/`g`/`b` hold a standard approximation. ANSI colors are opaque, ignore alpha
/// changes, win when composited over another color, and are written to the terminal as ANSI
/// codes when the app uses native ANSI colors (otherwise converted with a [`TerminalTheme`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
    /// Terminal ANSI color index (0-15), `-1` for the terminal's default color, or `None`.
    pub ansi: Option<i8>,
}

/// The ANSI color names, in index order (`ansi_<name>`).
pub const ANSI_COLORS: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
];

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
        Color {
            r,
            g,
            b,
            a: 1.0,
            ansi: None,
        }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Color {
            r,
            g,
            b,
            a,
            ansi: None,
        }
    }

    /// Every named color (Textual's `COLOR_NAME_TO_RGB`, including `ansi_*`), sorted by name:
    /// `(name, (r, g, b, a))`.
    pub fn named_colors() -> impl Iterator<Item = (&'static str, (u8, u8, u8, u8))> {
        crate::color_names::NAMED_COLORS.iter().copied()
    }

    /// The terminal's default color (`ansi_default`).
    pub const ANSI_DEFAULT: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 1.0,
        ansi: Some(-1),
    };

    /// ANSI color `index` (0-15; see [`ANSI_COLORS`]), or the default color for other values.
    pub fn from_ansi(index: i8) -> Color {
        let Some(name) = usize::try_from(index).ok().and_then(|i| ANSI_COLORS.get(i)) else {
            return Color::ANSI_DEFAULT;
        };
        let rgb = Color::parse(&format!("ansi_{name}")).unwrap_or(Color::BLACK);
        Color {
            ansi: Some(index),
            ..rgb
        }
    }

    /// `ansi_<name>` / `ansi_default` for ANSI colors.
    pub fn ansi_name(&self) -> Option<String> {
        match self.ansi? {
            -1 => Some("ansi_default".into()),
            i => ANSI_COLORS
                .get(i as usize)
                .map(|name| format!("ansi_{name}")),
        }
    }

    /// This color as a truecolor RGB color (ANSI colors via `theme`; the default color is the
    /// theme's foreground or background).
    pub fn to_truecolor(&self, theme: &TerminalTheme, foreground: bool) -> Color {
        match self.ansi {
            None => *self,
            Some(-1) if foreground => theme.foreground,
            Some(-1) => theme.background,
            Some(i) => theme.ansi[(i as usize) % 16],
        }
    }

    /// Black and white version (Textual's `Color.monochrome`).
    pub fn monochrome(&self) -> Color {
        let gray = (self.r as f32 * 0.2126 + self.g as f32 * 0.7152 + self.b as f32 * 0.0722)
            .round_ties_even() as u8;
        Color::rgba(gray, gray, gray, self.a)
    }

    /// Parse a CSS color: named colors (Textual's table incl. `ansi_*`), `#rgb`, `#rgba`,
    /// `#rrggbb`, `#rrggbbaa`, `rgb(r,g,b)`, `rgba(r,g,b,a)`, `hsl(h,s%,l%)`, `hsla(...)`,
    /// `transparent`. Case-insensitive, surrounding whitespace ignored.
    pub fn parse(s: &str) -> Result<Color, ColorParseError> {
        let text = s.trim();
        let err = || ColorParseError(s.to_string());
        let lower = text.to_ascii_lowercase();
        if let Some(hex) = lower.strip_prefix('#') {
            let digits: Vec<u8> = hex
                .chars()
                .map(|c| c.to_digit(16).map(|d| d as u8))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(err)?;
            let pair = |i: usize| digits[i] * 16 + digits[i + 1];
            let dup = |i: usize| digits[i] * 17;
            return match digits.len() {
                3 => Ok(Color::rgb(dup(0), dup(1), dup(2))),
                4 => Ok(Color::rgba(dup(0), dup(1), dup(2), dup(3) as f32 / 255.0)),
                6 => Ok(Color::rgb(pair(0), pair(2), pair(4))),
                8 => Ok(Color::rgba(
                    pair(0),
                    pair(2),
                    pair(4),
                    pair(6) as f32 / 255.0,
                )),
                _ => Err(err()),
            };
        }
        if let Some((func, rest)) = lower.split_once('(') {
            let args = rest.strip_suffix(')').ok_or_else(err)?;
            let args: Vec<&str> = args.split(',').map(str::trim).collect();
            let num = |a: &str| -> Result<f32, ColorParseError> {
                match a.strip_suffix('%') {
                    Some(p) => p
                        .trim()
                        .parse::<f32>()
                        .map(|v| v / 100.0)
                        .map_err(|_| err()),
                    None => a.parse::<f32>().map_err(|_| err()),
                }
            };
            let channel = |a: &str| -> Result<u8, ColorParseError> {
                let v = if a.ends_with('%') {
                    num(a)? * 255.0
                } else {
                    num(a)?
                };
                Ok(v.round().clamp(0.0, 255.0) as u8)
            };
            let unit = |a: &str| -> Result<f32, ColorParseError> {
                let v = num(a)?;
                Ok(if a.ends_with('%') {
                    v
                } else if v > 1.0 {
                    v / 100.0
                } else {
                    v
                })
            };
            return match (func.trim(), args.len()) {
                ("rgb", 3) => Ok(Color::rgb(
                    channel(args[0])?,
                    channel(args[1])?,
                    channel(args[2])?,
                )),
                ("rgba", 4) => Ok(Color::rgba(
                    channel(args[0])?,
                    channel(args[1])?,
                    channel(args[2])?,
                    num(args[3])?.clamp(0.0, 1.0),
                )),
                ("hsl", 3) => Ok(Color::from_hsl(
                    num(args[0])?,
                    unit(args[1])?,
                    unit(args[2])?,
                )),
                ("hsla", 4) => Ok(
                    Color::from_hsl(num(args[0])?, unit(args[1])?, unit(args[2])?)
                        .with_alpha(num(args[3])?.clamp(0.0, 1.0)),
                ),
                _ => Err(err()),
            };
        }
        let name = lower.replace('-', "_");
        if name == "ansi_default" {
            return Ok(Color::ANSI_DEFAULT);
        }
        if let Some(index) = name
            .strip_prefix("ansi_")
            .and_then(|n| ANSI_COLORS.iter().position(|c| *c == n))
        {
            let (r, g, b, _) = crate::color_names::NAMED_COLORS[crate::color_names::NAMED_COLORS
                .binary_search_by(|(n, _)| (*n).cmp(name.as_str()))
                .map_err(|_| err())?]
            .1;
            return Ok(Color {
                r,
                g,
                b,
                a: 1.0,
                ansi: Some(index as i8),
            });
        }
        crate::color_names::NAMED_COLORS
            .binary_search_by(|(n, _)| (*n).cmp(name.as_str()))
            .map(|i| {
                let (r, g, b, a) = crate::color_names::NAMED_COLORS[i].1;
                Color::rgba(r, g, b, a as f32 / 255.0)
            })
            .map_err(|_| err())
    }

    /// Create from HSL (h in 0..360 degrees, s and l in 0..1).
    pub fn from_hsl(h: f32, s: f32, l: f32) -> Color {
        let h = (h.rem_euclid(360.0)) / 360.0;
        let (s, l) = (s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
        if s == 0.0 {
            let v = (l * 255.0).round() as u8;
            return Color::rgb(v, v, v);
        }
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let hue = |mut t: f32| {
            if t < 0.0 {
                t += 1.0
            }
            if t > 1.0 {
                t -= 1.0
            }
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };
        let c = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
        Color::rgb(c(hue(h + 1.0 / 3.0)), c(hue(h)), c(hue(h - 1.0 / 3.0)))
    }

    /// (h degrees 0..360, s 0..1, l 0..1)
    pub fn to_hsl(&self) -> (f32, f32, f32) {
        let (r, g, b) = (
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        );
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if max == min {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0, s, l)
    }

    /// The RGB color with alpha `a` (an ANSI color becomes its RGB approximation, as in Textual).
    pub fn with_alpha(&self, a: f32) -> Color {
        Color::rgba(self.r, self.g, self.b, a)
    }

    /// Alpha multiplied by `f` (ANSI colors are unchanged).
    pub fn multiply_alpha(&self, f: f32) -> Color {
        if self.ansi.is_some() {
            return *self;
        }
        Color {
            a: self.a * f,
            ..*self
        }
    }

    /// Fully transparent (ANSI colors never are).
    pub fn is_transparent(&self) -> bool {
        self.a <= 0.0 && self.ansi.is_none()
    }

    /// Linear interpolation of all channels (including alpha) towards `dest` by `factor` (0..1).
    /// An ANSI `dest` is returned as is.
    pub fn blend(&self, dest: Color, factor: f32) -> Color {
        if dest.ansi.is_some() {
            return dest;
        }
        if factor <= 0.0 {
            return *self;
        }
        if factor >= 1.0 {
            return dest;
        }
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * factor) as u8;
        Color::rgba(
            lerp(self.r, dest.r),
            lerp(self.g, dest.g),
            lerp(self.b, dest.b),
            self.a + (dest.a - self.a) * factor,
        )
    }

    /// Composite `top` over `self` (self is the background), using `top.a` as the blend factor.
    /// The result keeps `self`'s alpha. Equivalent to Textual's `self + top`.
    pub fn composite(&self, top: Color) -> Color {
        if top.ansi.is_some() {
            return top;
        }
        if top.a <= 0.0 {
            return *self;
        }
        if top.a >= 1.0 {
            return Color { a: self.a, ..top };
        }
        self.blend(top, top.a).with_alpha(self.a)
    }

    /// Tint: blend RGB towards `tint`'s RGB by `tint.a`, keeping own alpha (no effect when
    /// either color is ANSI).
    pub fn tint(&self, tint: Color) -> Color {
        if self.ansi.is_some() || tint.ansi.is_some() {
            return *self;
        }
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * tint.a) as u8;
        Color::rgba(
            lerp(self.r, tint.r),
            lerp(self.g, tint.g),
            lerp(self.b, tint.b),
            self.a,
        )
    }

    /// Lighten by `amount` (0..1) in CIE-L*ab lightness like Textual.
    pub fn lighten(&self, amount: f32) -> Color {
        let (l, a, b) = rgb_to_lab(self);
        lab_to_rgb(l + amount * 100.0, a, b, self.a)
    }

    pub fn darken(&self, amount: f32) -> Color {
        self.lighten(-amount)
    }

    pub fn inverse(&self) -> Color {
        Color::rgba(255 - self.r, 255 - self.g, 255 - self.b, self.a)
    }

    /// Perceived brightness 0..1.
    pub fn brightness(&self) -> f32 {
        (299.0 * self.r as f32 + 587.0 * self.g as f32 + 114.0 * self.b as f32) / (1000.0 * 255.0)
    }

    /// Contrasting text color (white or black) with given alpha, as in Textual.
    pub fn get_contrast_text(&self, alpha: f32) -> Color {
        if self.brightness() < 0.5 {
            Color::WHITE
        } else {
            Color::BLACK
        }
        .with_alpha(alpha)
    }

    /// `#RRGGBB` or `#RRGGBBAA` when alpha < 1.
    pub fn hex(&self) -> String {
        if self.a >= 1.0 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            let a = (self.a.clamp(0.0, 1.0) * 255.0) as u8;
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, a)
        }
    }

    /// CSS string, e.g. `rgb(10,20,30)` or `rgba(10,20,30,0.5)`.
    pub fn css(&self) -> String {
        if self.a >= 1.0 {
            format!("rgb({},{},{})", self.r, self.g, self.b)
        } else {
            format!("rgba({},{},{},{})", self.r, self.g, self.b, self.a)
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rgba({},{},{},{})", self.r, self.g, self.b, self.a)
    }
}

fn rgb_to_lab(c: &Color) -> (f32, f32, f32) {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v > 0.04045 {
            ((v + 0.055) / 1.055).powf(2.4)
        } else {
            v / 12.92
        }
    };
    let (r, g, b) = (lin(c.r), lin(c.g), lin(c.b));
    let x = (r * 41.24 + g * 35.76 + b * 18.05) / 95.047;
    let y = (r * 21.26 + g * 71.52 + b * 7.22) / 100.0;
    let z = (r * 1.93 + g * 11.92 + b * 95.05) / 108.883;
    let f = |v: f32| {
        if v > 0.008856 {
            v.powf(1.0 / 3.0)
        } else {
            7.787 * v + 16.0 / 116.0
        }
    };
    let (x, y, z) = (f(x), f(y), f(z));
    (116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z))
}

fn lab_to_rgb(l: f32, a: f32, b: f32, alpha: f32) -> Color {
    let off = 16.0 / 116.0;
    let y = (l + 16.0) / 116.0;
    let x = a / 500.0 + y;
    let z = y - b / 200.0;
    let y = if y > 0.206_893_03 {
        y.powi(3)
    } else {
        (y - off) / 7.787
    };
    let x = if x > 0.206_893_03 {
        0.95047 * x.powi(3)
    } else {
        0.122059 * (x - off)
    };
    let z = if z > 0.206_893_03 {
        1.08883 * z.powi(3)
    } else {
        0.139827 * (z - off)
    };
    let r = x * 3.2406 + y * -1.5372 + z * -0.4986;
    let g = x * -0.9689 + y * 1.8758 + z * 0.0415;
    let bb = x * 0.0557 + y * -0.2040 + z * 1.0570;
    let gamma = |v: f32| {
        if v > 0.003_130_8 {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        } else {
            12.92 * v
        }
    };
    let ch = |v: f32| (gamma(v) * 255.0).clamp(0.0, 255.0) as u8;
    Color::rgba(ch(r), ch(g), ch(bb), alpha)
}

/// Error creating a [`Gradient`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradientError(pub String);

impl fmt::Display for GradientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GradientError {}

/// A color gradient through `(position, color)` stops (port of `textual.color.Gradient`).
///
/// `quality` colors are pre-computed along the spectrum; [`Gradient::get_color`] blends between
/// the two nearest ones.
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    stops: Vec<(f64, Color)>,
    quality: usize,
    colors: Vec<Color>,
}

impl Gradient {
    /// A gradient with the default quality (50 steps). Stops are sorted by position; there must
    /// be at least two, starting at 0.0 and ending at 1.0.
    pub fn new(stops: &[(f64, Color)]) -> Result<Gradient, GradientError> {
        Self::with_quality(stops, 50)
    }

    pub fn with_quality(stops: &[(f64, Color)], quality: usize) -> Result<Gradient, GradientError> {
        let mut stops = stops.to_vec();
        stops.sort_by(|a, b| a.0.total_cmp(&b.0));
        if stops.len() < 2 {
            return Err(GradientError("At least 2 stops required.".into()));
        }
        if stops[0].0 != 0.0 {
            return Err(GradientError("First stop must be 0.".into()));
        }
        if stops[stops.len() - 1].0 != 1.0 {
            return Err(GradientError("Last stop must be 1.".into()));
        }
        let quality = quality.max(2);
        let mut colors = Vec::with_capacity(quality);
        let mut position = 0;
        for step_position in 0..quality {
            let step = step_position as f64 / (quality - 1) as f64;
            while step > stops[position + 1].0 {
                position += 1;
            }
            let (stop1, color1) = stops[position];
            let (stop2, color2) = stops[position + 1];
            let span = stop2 - stop1;
            let factor = if span > 0.0 {
                (step - stop1) / span
            } else {
                1.0
            };
            colors.push(color1.blend(color2, factor as f32));
        }
        Ok(Gradient {
            stops,
            quality,
            colors,
        })
    }

    /// A gradient through evenly spaced colors (at least two).
    pub fn from_colors(colors: &[Color]) -> Result<Gradient, GradientError> {
        if colors.len() < 2 {
            return Err(GradientError("Two or more colors required.".into()));
        }
        let n = (colors.len() - 1) as f64;
        let stops: Vec<(f64, Color)> = colors
            .iter()
            .enumerate()
            .map(|(i, c)| (i as f64 / n, *c))
            .collect();
        Self::new(&stops)
    }

    pub fn stops(&self) -> &[(f64, Color)] {
        &self.stops
    }

    /// The pre-computed colors (`quality` of them).
    pub fn colors(&self) -> &[Color] {
        &self.colors
    }

    /// The color at `position` (clamped to 0..=1).
    pub fn get_color(&self, position: f64) -> Color {
        if position <= 0.0 || position.is_nan() {
            return self.colors[0];
        }
        if position >= 1.0 {
            return self.colors[self.quality - 1];
        }
        let color_position = position * (self.quality - 1) as f64;
        let index = color_position as usize;
        let (c1, c2) = (self.colors[index], self.colors[index + 1]);
        c1.blend(c2, (color_position % 1.0) as f32)
    }
}

#[cfg(test)]
mod gradient_tests {
    use super::*;

    #[test]
    fn gradient_stops_and_blend() {
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        let g = Gradient::new(&[(1.0, blue), (0.0, red)]).unwrap();
        assert_eq!(g.colors().len(), 50);
        assert_eq!(g.get_color(0.0), red);
        assert_eq!(g.get_color(1.0), blue);
        assert_eq!(g.get_color(-3.0), red);
        let mid = g.get_color(0.5);
        assert!((mid.r as i32 - 127).abs() <= 2 && (mid.b as i32 - 127).abs() <= 2);
        let g3 = Gradient::from_colors(&[red, Color::rgb(0, 255, 0), blue]).unwrap();
        assert_eq!(g3.stops()[1].0, 0.5);
        assert!(g3.get_color(0.5).g > 240);
        assert!(Gradient::new(&[(0.0, red)]).is_err());
        assert!(Gradient::new(&[(0.1, red), (1.0, blue)]).is_err());
        assert!(Gradient::new(&[(0.0, red), (0.9, blue)]).is_err());
        assert!(Gradient::from_colors(&[red]).is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_formats() {
        assert_eq!(Color::parse("#fff").unwrap(), Color::WHITE);
        assert_eq!(
            Color::parse("#FF000080").unwrap(),
            Color::rgba(255, 0, 0, 128.0 / 255.0)
        );
        assert_eq!(Color::parse("rgb(1, 2, 3)").unwrap(), Color::rgb(1, 2, 3));
        assert_eq!(
            Color::parse("rgba(1,2,3,0.5)").unwrap(),
            Color::rgba(1, 2, 3, 0.5)
        );
        assert_eq!(
            Color::parse("hsl(0, 100%, 50%)").unwrap(),
            Color::rgb(255, 0, 0)
        );
        assert_eq!(Color::parse(" Red ").unwrap(), Color::parse("red").unwrap());
        assert_eq!(Color::parse("ansi_bright_red").unwrap().a, 1.0);
        assert!(Color::parse("transparent").unwrap().is_transparent());
        assert!(Color::parse("nonsense").is_err());
        assert!(Color::parse("#12").is_err());
    }

    #[test]
    fn hsl_roundtrip() {
        let c = Color::rgb(70, 130, 180);
        let (h, s, l) = c.to_hsl();
        let back = Color::from_hsl(h, s, l);
        assert!((back.r as i32 - c.r as i32).abs() <= 1);
        assert!((back.b as i32 - c.b as i32).abs() <= 1);
    }

    #[test]
    fn blending() {
        let bg = Color::BLACK;
        assert_eq!(
            bg.composite(Color::WHITE.with_alpha(0.5)),
            Color::rgb(127, 127, 127)
        );
        assert_eq!(bg.composite(Color::rgb(9, 9, 9)), Color::rgb(9, 9, 9));
        assert_eq!(bg.composite(Color::TRANSPARENT), bg);
        assert_eq!(
            Color::rgb(0, 0, 0).blend(Color::rgb(100, 200, 50), 0.5),
            Color::rgb(50, 100, 25)
        );
    }

    #[test]
    fn lighten_darken() {
        let c = Color::rgb(100, 100, 100);
        assert!(c.lighten(0.1).r > c.r);
        assert!(c.darken(0.1).r < c.r);
        assert_eq!(Color::BLACK.get_contrast_text(1.0), Color::WHITE);
        assert_eq!(Color::rgb(1, 2, 3).hex(), "#010203");
    }
}

/// RGB values for the terminal's colors, used to show ANSI colors as truecolor (Rich's
/// `TerminalTheme`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalTheme {
    pub background: Color,
    pub foreground: Color,
    /// The 16 ANSI colors.
    pub ansi: [Color; 16],
}

impl TerminalTheme {
    /// Textual's terminal theme for dark themes (`textual._ansi_theme.MONOKAI`).
    pub const MONOKAI: TerminalTheme = TerminalTheme {
        background: Color::rgb(12, 12, 12),
        foreground: Color::rgb(217, 217, 217),
        ansi: [
            Color::rgb(26, 26, 26),
            Color::rgb(244, 0, 95),
            Color::rgb(152, 224, 36),
            Color::rgb(253, 151, 31),
            Color::rgb(157, 101, 255),
            Color::rgb(244, 0, 95),
            Color::rgb(88, 209, 235),
            Color::rgb(196, 197, 181),
            Color::rgb(98, 94, 76),
            Color::rgb(244, 0, 95),
            Color::rgb(152, 224, 36),
            Color::rgb(224, 213, 97),
            Color::rgb(157, 101, 255),
            Color::rgb(244, 0, 95),
            Color::rgb(88, 209, 235),
            Color::rgb(246, 246, 239),
        ],
    };

    /// Rich's theme for SVG exports (`rich.terminal_theme.SVG_EXPORT_THEME`), which Textual's
    /// screenshots use for ANSI colors.
    pub const SVG_EXPORT: TerminalTheme = TerminalTheme {
        background: Color::rgb(41, 41, 41),
        foreground: Color::rgb(197, 200, 198),
        ansi: [
            Color::rgb(75, 78, 85),
            Color::rgb(204, 85, 90),
            Color::rgb(152, 168, 75),
            Color::rgb(208, 179, 68),
            Color::rgb(96, 138, 177),
            Color::rgb(152, 114, 159),
            Color::rgb(104, 160, 179),
            Color::rgb(197, 200, 198),
            Color::rgb(154, 155, 153),
            Color::rgb(255, 38, 39),
            Color::rgb(0, 130, 61),
            Color::rgb(208, 132, 66),
            Color::rgb(25, 132, 233),
            Color::rgb(255, 44, 122),
            Color::rgb(57, 130, 128),
            Color::rgb(253, 253, 197),
        ],
    };

    /// Textual's terminal theme for light themes (`textual._ansi_theme.ALABASTER`).
    pub const ALABASTER: TerminalTheme = TerminalTheme {
        background: Color::rgb(247, 247, 247),
        foreground: Color::rgb(0, 0, 0),
        ansi: [
            Color::rgb(0, 0, 0),
            Color::rgb(170, 55, 49),
            Color::rgb(68, 140, 39),
            Color::rgb(203, 144, 0),
            Color::rgb(50, 92, 192),
            Color::rgb(122, 62, 157),
            Color::rgb(0, 131, 178),
            Color::rgb(247, 247, 247),
            Color::rgb(119, 119, 119),
            Color::rgb(240, 80, 80),
            Color::rgb(96, 203, 0),
            Color::rgb(255, 188, 93),
            Color::rgb(0, 122, 204),
            Color::rgb(230, 76, 230),
            Color::rgb(0, 170, 203),
            Color::rgb(247, 247, 247),
        ],
    };
}
