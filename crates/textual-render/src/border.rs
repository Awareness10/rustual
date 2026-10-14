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
        use BorderKind::*;
        // BORDER_CHARS from textual/_border.py.
        match self {
            None | Hidden | Blank => [[" ", " ", " "], [" ", " ", " "], [" ", " ", " "]],
            Ascii => [["+", "-", "+"], ["|", " ", "|"], ["+", "-", "+"]],
            Round => [["╭", "─", "╮"], ["│", " ", "│"], ["╰", "─", "╯"]],
            Solid => [["┌", "─", "┐"], ["│", " ", "│"], ["└", "─", "┘"]],
            Double => [["╔", "═", "╗"], ["║", " ", "║"], ["╚", "═", "╝"]],
            Dashed => [["┏", "╍", "┓"], ["╏", " ", "╏"], ["┗", "╍", "┛"]],
            Heavy => [["┏", "━", "┓"], ["┃", " ", "┃"], ["┗", "━", "┛"]],
            Inner => [["▗", "▄", "▖"], ["▐", " ", "▌"], ["▝", "▀", "▘"]],
            Outer => [["▛", "▀", "▜"], ["▌", " ", "▐"], ["▙", "▄", "▟"]],
            Thick => [["█", "▀", "█"], ["█", " ", "█"], ["█", "▄", "█"]],
            Block => [["▄", "▄", "▄"], ["█", " ", "█"], ["▀", "▀", "▀"]],
            Hkey => [["▔", "▔", "▔"], [" ", " ", " "], ["▁", "▁", "▁"]],
            Vkey => [["▏", " ", "▕"], ["▏", " ", "▕"], ["▏", " ", "▕"]],
            Tall => [["▊", "▔", "▎"], ["▊", " ", "▎"], ["▊", "▁", "▎"]],
            Panel => [["▊", "█", "▎"], ["▊", " ", "▎"], ["▊", "▁", "▎"]],
            Tab | Wide => [["▁", "▁", "▁"], ["▎", " ", "▊"], ["▔", "▔", "▔"]],
        }
    }

    /// 3x3 coloring locations matching [`BorderKind::chars`].
    ///
    /// Ported from `BORDER_LOCATIONS` in `textual/_border.py`, mapped by what Textual's
    /// `get_box` actually renders for each index (the variant docs describe the result):
    /// 0 → `Normal` (inner style), 1 → `Outer` (outer style),
    /// 2 → `OuterReverse` (`Style(bg=outer.bg, fg=inner.fg, reverse)`),
    /// 3 → `Reverse` (`Style(bg=inner.bg, fg=outer.fg, reverse)`).
    pub fn locations(&self) -> [[BorderCellStyle; 3]; 3] {
        use BorderCellStyle::{Normal as N, Outer as O, OuterReverse as OR, Reverse as R};
        use BorderKind::*;
        match self {
            Inner => [[O, O, O], [O, O, O], [O, O, O]],
            Block => [[O, O, O], [N, N, N], [O, O, O]],
            Tall | Panel => [[OR, N, O], [OR, N, O], [OR, N, O]],
            Tab | Wide => [[O, O, O], [N, O, R], [O, O, O]],
            _ => [[N, N, N], [N, N, N], [N, N, N]],
        }
    }

    pub fn name(&self) -> &'static str {
        use BorderKind::*;
        match self {
            None => "none",
            Hidden => "hidden",
            Ascii => "ascii",
            Blank => "blank",
            Dashed => "dashed",
            Double => "double",
            Heavy => "heavy",
            Hkey => "hkey",
            Inner => "inner",
            Outer => "outer",
            Panel => "panel",
            Round => "round",
            Solid => "solid",
            Tall => "tall",
            Tab => "tab",
            Thick => "thick",
            Block => "block",
            Vkey => "vkey",
            Wide => "wide",
        }
    }

    /// Every border kind, in declaration order.
    pub const ALL: [BorderKind; 19] = [
        BorderKind::None,
        BorderKind::Hidden,
        BorderKind::Ascii,
        BorderKind::Blank,
        BorderKind::Dashed,
        BorderKind::Double,
        BorderKind::Heavy,
        BorderKind::Hkey,
        BorderKind::Inner,
        BorderKind::Outer,
        BorderKind::Panel,
        BorderKind::Round,
        BorderKind::Solid,
        BorderKind::Tall,
        BorderKind::Tab,
        BorderKind::Thick,
        BorderKind::Block,
        BorderKind::Vkey,
        BorderKind::Wide,
    ];

    /// Whether border titles are drawn reversed on the (top, bottom) edge
    /// (`BORDER_TITLE_FLIP` in `textual/_border.py`).
    pub fn title_flip(&self) -> (bool, bool) {
        match self {
            BorderKind::Panel => (true, false),
            BorderKind::Tab => (true, true),
            _ => (false, false),
        }
    }
}

impl FromStr for BorderKind {
    type Err = String;
    /// Parse a CSS border type name (case-insensitive). The empty string means `none`.
    fn from_str(s: &str) -> Result<Self, String> {
        let name = s.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Ok(BorderKind::None);
        }
        BorderKind::ALL
            .iter()
            .copied()
            .find(|k| k.name() == name)
            .ok_or_else(|| format!("invalid border type {s:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for k in BorderKind::ALL {
            assert_eq!(k.name().parse::<BorderKind>().unwrap(), k);
        }
        assert_eq!("ROUND".parse::<BorderKind>().unwrap(), BorderKind::Round);
        assert_eq!(" heavy ".parse::<BorderKind>().unwrap(), BorderKind::Heavy);
        assert_eq!("".parse::<BorderKind>().unwrap(), BorderKind::None);
        assert!("wavy".parse::<BorderKind>().is_err());
    }

    #[test]
    fn has_width() {
        assert!(!BorderKind::None.has_width());
        assert!(!BorderKind::Hidden.has_width());
        assert!(BorderKind::Blank.has_width());
        assert!(BorderKind::Round.has_width());
    }

    #[test]
    fn glyphs() {
        assert_eq!(
            BorderKind::Round.chars(),
            [["╭", "─", "╮"], ["│", " ", "│"], ["╰", "─", "╯"]]
        );
        assert_eq!(BorderKind::Tall.chars()[0], ["▊", "▔", "▎"]);
        assert_eq!(BorderKind::Panel.chars()[0][1], "█");
        assert_eq!(BorderKind::Hkey.chars()[1], [" ", " ", " "]);
        // every glyph is a single cell
        for k in BorderKind::ALL {
            for row in k.chars() {
                for g in row {
                    assert_eq!(crate::cells::cell_len(g), 1, "{k:?} {g:?}");
                }
            }
        }
    }

    #[test]
    fn locations() {
        use BorderCellStyle::*;
        assert_eq!(BorderKind::Round.locations()[0][0], Normal);
        assert_eq!(BorderKind::Inner.locations()[1][1], Outer);
        assert_eq!(BorderKind::Block.locations()[1], [Normal, Normal, Normal]);
        assert_eq!(
            BorderKind::Tall.locations()[1],
            [OuterReverse, Normal, Outer]
        );
        assert_eq!(BorderKind::Tab.locations()[1], [Normal, Outer, Reverse]);
        assert_eq!(BorderKind::Wide.locations(), BorderKind::Tab.locations());
    }
}
