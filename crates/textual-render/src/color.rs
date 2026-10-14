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

    pub fn with_alpha(&self, a: f32) -> Color {
        Color { a, ..*self }
    }

    pub fn multiply_alpha(&self, f: f32) -> Color {
        Color {
            a: self.a * f,
            ..*self
        }
    }

    pub fn is_transparent(&self) -> bool {
        self.a <= 0.0
    }

    /// Linear interpolation of all channels (including alpha) towards `dest` by `factor` (0..1).
    pub fn blend(&self, dest: Color, factor: f32) -> Color {
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
        if top.a >= 1.0 {
            return Color { a: self.a, ..top };
        }
        self.blend(top, top.a).with_alpha(self.a)
    }

    /// Tint: blend RGB towards `tint`'s RGB by `tint.a`, keeping own alpha.
    pub fn tint(&self, tint: Color) -> Color {
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
