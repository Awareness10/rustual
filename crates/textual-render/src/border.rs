//! Border glyph tables (port of `textual/_border.py`).

use std::str::FromStr;

/// CSS border types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BorderKind {
    #[default]
    None,
    Hidden,
    Ascii,
    Blank,
    Dashed,
    Double,
    Heavy,
    Hkey,
    Inner,
    Outer,
    Panel,
    Round,
    Solid,
    Tall,
    Tab,
    Thick,
    Block,
    Vkey,
    Wide,
}

/// How a border cell is colored, from Textual's `BORDER_LOCATIONS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderCellStyle {
    /// fg = border color, bg = widget background (0 in Textual).
    Normal,
    /// fg = widget background, bg = border color (1, "reverse").
    Reverse,
    /// fg = border color, bg = background *outside* the widget (2).
    Outer,
    /// fg = background outside, bg = border color (3).
    OuterReverse,
}

impl BorderKind {
    /// True for kinds that take up space (everything except `None`/`Hidden`).
    /// Note: Textual treats `hidden` like `none`; `blank` takes space.
    pub fn has_width(&self) -> bool {
        !matches!(self, BorderKind::None | BorderKind::Hidden)
    }

    /// 3x3 glyphs: rows top/middle/bottom, columns left/middle/right. Middle-middle unused.
    pub fn chars(&self) -> [[&'static str; 3]; 3] {
        todo!()
    }

    /// 3x3 coloring locations matching [`BorderKind::chars`].
    pub fn locations(&self) -> [[BorderCellStyle; 3]; 3] {
        todo!()
    }

    pub fn name(&self) -> &'static str {
        todo!()
    }
}

impl FromStr for BorderKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        todo!("{s}")
    }
}
