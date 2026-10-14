//! CSS scalar values: `10`, `50%`, `1fr`, `auto`, `25w`, `25h`, `50vw`, `50vh`.

use std::str::FromStr;
use textual_render::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Cells,
    Fraction,
    Percent,
    /// Percentage of the container width.
    Width,
    /// Percentage of the container height.
    Height,
    ViewWidth,
    ViewHeight,
    Auto,
}

/// Which axis a percentage resolves against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Width,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scalar {
    pub value: f32,
    pub unit: Unit,
}

impl Scalar {
    pub const AUTO: Scalar = Scalar {
        value: 1.0,
        unit: Unit::Auto,
    };

    pub const fn cells(v: f32) -> Self {
        Scalar {
            value: v,
            unit: Unit::Cells,
        }
    }
    pub const fn fr(v: f32) -> Self {
        Scalar {
            value: v,
            unit: Unit::Fraction,
        }
    }
    pub const fn percent(v: f32) -> Self {
        Scalar {
            value: v,
            unit: Unit::Percent,
        }
    }

    pub fn is_auto(&self) -> bool {
        self.unit == Unit::Auto
    }
    pub fn is_fraction(&self) -> bool {
        self.unit == Unit::Fraction
    }
    pub fn is_cells(&self) -> bool {
        self.unit == Unit::Cells
    }
    pub fn is_percent(&self) -> bool {
        matches!(self.unit, Unit::Percent | Unit::Width | Unit::Height)
    }

    /// Resolve to a (fractional) number of cells.
    /// - `Percent` resolves against `container` along `dim`.
    /// - `Fraction` multiplies by `fraction_unit` (cells per 1fr).
    /// - `Auto` resolves to 0 — callers must handle auto themselves.
    pub fn resolve(
        &self,
        dim: Dimension,
        container: Size,
        viewport: Size,
        fraction_unit: f32,
    ) -> f32 {
        let along = |s: Size| match dim {
            Dimension::Width => s.width as f32,
            Dimension::Height => s.height as f32,
        };
        match self.unit {
            Unit::Cells => self.value,
            Unit::Fraction => self.value * fraction_unit,
            Unit::Percent => self.value * along(container) / 100.0,
            Unit::Width => self.value * container.width as f32 / 100.0,
            Unit::Height => self.value * container.height as f32 / 100.0,
            Unit::ViewWidth => self.value * viewport.width as f32 / 100.0,
            Unit::ViewHeight => self.value * viewport.height as f32 / 100.0,
            Unit::Auto => 0.0,
        }
    }
}

impl FromStr for Scalar {
    type Err = String;
    /// Parse `"10"`, `"1.5fr"`, `"50%"`, `"auto"`, `"20w"`, `"20h"`, `"10vw"`, `"10vh"`.
    fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("auto") {
            return Ok(Scalar::AUTO);
        }
        let (num, unit) = [
            ("fr", Unit::Fraction),
            ("%", Unit::Percent),
            ("vw", Unit::ViewWidth),
            ("vh", Unit::ViewHeight),
            ("w", Unit::Width),
            ("h", Unit::Height),
        ]
        .iter()
        .find_map(|(suffix, unit)| s.strip_suffix(suffix).map(|n| (n, *unit)))
        .unwrap_or((s, Unit::Cells));
        let value: f32 = num
            .trim()
            .parse()
            .map_err(|_| format!("invalid scalar: {s}"))?;
        Ok(Scalar { value, unit })
    }
}
