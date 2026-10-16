//! Box-drawing character sets (port of `rich/box.py`).

/// The characters of a box, as eight rows of four characters (`rich.box.Box`):
///
/// ```text
/// ┌─┬┐ top
/// │ ││ head
/// ├─┼┤ head_row
/// │ ││ mid
/// ├─┼┤ row
/// ├─┼┤ foot_row
/// │ ││ foot
/// └─┴┘ bottom
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxChars {
    rows: [[char; 4]; 8],
    /// Uses ASCII characters only.
    pub ascii: bool,
}

/// Which horizontal row of a table to draw ([`BoxChars::row`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowLevel {
    Head,
    Row,
    Mid,
    Foot,
}

impl BoxChars {
    /// Build from eight lines of four characters (the layout of `rich.box` definitions).
    pub const fn new(rows: [[char; 4]; 8], ascii: bool) -> Self {
        BoxChars { rows, ascii }
    }

    pub fn top_left(&self) -> char {
        self.rows[0][0]
    }
    pub fn top(&self) -> char {
        self.rows[0][1]
    }
    pub fn top_right(&self) -> char {
        self.rows[0][3]
    }
    pub fn bottom_left(&self) -> char {
        self.rows[7][0]
    }
    pub fn bottom(&self) -> char {
        self.rows[7][1]
    }
    pub fn bottom_right(&self) -> char {
        self.rows[7][3]
    }
    /// Left / right edge and column divider of header lines.
    pub fn head_vertical(&self) -> (char, char, char) {
        (self.rows[1][0], self.rows[1][3], self.rows[1][2])
    }
    /// Left / right edge and column divider of body lines.
    pub fn mid_vertical(&self) -> (char, char, char) {
        (self.rows[3][0], self.rows[3][3], self.rows[3][2])
    }
    /// Left / right edge and column divider of footer lines.
    pub fn foot_vertical(&self) -> (char, char, char) {
        (self.rows[6][0], self.rows[6][3], self.rows[6][2])
    }

    /// The top edge for columns of `widths` (`Box.get_top`).
    pub fn get_top(&self, widths: &[i32]) -> String {
        let [l, h, d, r] = self.rows[0];
        line(l, h, d, r, widths, true)
    }

    /// The bottom edge for columns of `widths` (`Box.get_bottom`).
    pub fn get_bottom(&self, widths: &[i32]) -> String {
        let [l, h, d, r] = self.rows[7];
        line(l, h, d, r, widths, true)
    }

    /// A horizontal rule between rows (`Box.get_row`).
    pub(crate) fn row(&self, widths: &[i32], level: RowLevel, edge: bool) -> String {
        let [l, h, d, r] = match level {
            RowLevel::Head => self.rows[2],
            RowLevel::Row => self.rows[4],
            RowLevel::Foot => self.rows[5],
            RowLevel::Mid => {
                let [l, _, d, r] = self.rows[3];
                [l, ' ', d, r]
            }
        };
        line(l, h, d, r, widths, edge)
    }

    /// The box to use when the header is hidden (`Box.get_plain_headed_box`): boxes with a
    /// distinct header separator fall back to their plain equivalent.
    pub fn plain_headed(&self) -> BoxChars {
        match *self {
            b if b == HEAVY_HEAD || b == SQUARE_DOUBLE_HEAD => SQUARE,
            b if b == MINIMAL_DOUBLE_HEAD || b == MINIMAL_HEAVY_HEAD => MINIMAL,
            b if b == ASCII_DOUBLE_HEAD => ASCII2,
            b => b,
        }
    }
}

fn line(
    left: char,
    horizontal: char,
    divider: char,
    right: char,
    widths: &[i32],
    edge: bool,
) -> String {
    let mut s = String::new();
    if edge {
        s.push(left);
    }
    for (i, w) in widths.iter().enumerate() {
        s.extend(std::iter::repeat_n(horizontal, (*w).max(0) as usize));
        if i + 1 < widths.len() {
            s.push(divider);
        }
    }
    if edge {
        s.push(right);
    }
    s
}

/// `const` helper: eight rows from eight 4-char strings.
const fn parse(rows: [&str; 8], ascii: bool) -> BoxChars {
    let mut out = [[' '; 4]; 8];
    let mut r = 0;
    while r < 8 {
        let bytes = rows[r].as_bytes();
        let mut i = 0;
        let mut c = 0;
        while c < 4 {
            // Decode one UTF-8 scalar.
            let b0 = bytes[i] as u32;
            let (ch, len) = if b0 < 0x80 {
                (b0, 1)
            } else if b0 < 0xE0 {
                (((b0 & 0x1F) << 6) | (bytes[i + 1] as u32 & 0x3F), 2)
            } else if b0 < 0xF0 {
                (
                    ((b0 & 0x0F) << 12)
                        | ((bytes[i + 1] as u32 & 0x3F) << 6)
                        | (bytes[i + 2] as u32 & 0x3F),
                    3,
                )
            } else {
                (
                    ((b0 & 0x07) << 18)
                        | ((bytes[i + 1] as u32 & 0x3F) << 12)
                        | ((bytes[i + 2] as u32 & 0x3F) << 6)
                        | (bytes[i + 3] as u32 & 0x3F),
                    4,
                )
            };
            out[r][c] = match char::from_u32(ch) {
                Some(ch) => ch,
                None => panic!("invalid box character"),
            };
            i += len;
            c += 1;
        }
        assert!(i == bytes.len(), "box rows have four characters");
        r += 1;
    }
    BoxChars::new(out, ascii)
}

pub const ASCII: BoxChars = parse(
    [
        "+--+", "| ||", "|-+|", "| ||", "|-+|", "|-+|", "| ||", "+--+",
    ],
    true,
);
pub const ASCII2: BoxChars = parse(
    [
        "+-++", "| ||", "+-++", "| ||", "+-++", "+-++", "| ||", "+-++",
    ],
    true,
);
pub const ASCII_DOUBLE_HEAD: BoxChars = parse(
    [
        "+-++", "| ||", "+=++", "| ||", "+-++", "+-++", "| ||", "+-++",
    ],
    true,
);
pub const SQUARE: BoxChars = parse(
    [
        "┌─┬┐",
        "│ ││",
        "├─┼┤",
        "│ ││",
        "├─┼┤",
        "├─┼┤",
        "│ ││",
        "└─┴┘",
    ],
    false,
);
pub const SQUARE_DOUBLE_HEAD: BoxChars = parse(
    [
        "┌─┬┐",
        "│ ││",
        "╞═╪╡",
        "│ ││",
        "├─┼┤",
        "├─┼┤",
        "│ ││",
        "└─┴┘",
    ],
    false,
);
pub const MINIMAL: BoxChars = parse(
    [
        "  ╷ ",
        "  │ ",
        "╶─┼╴",
        "  │ ",
        "╶─┼╴",
        "╶─┼╴",
        "  │ ",
        "  ╵ ",
    ],
    false,
);
pub const MINIMAL_HEAVY_HEAD: BoxChars = parse(
    [
        "  ╷ ",
        "  │ ",
        "╺━┿╸",
        "  │ ",
        "╶─┼╴",
        "╶─┼╴",
        "  │ ",
        "  ╵ ",
    ],
    false,
);
pub const MINIMAL_DOUBLE_HEAD: BoxChars = parse(
    [
        "  ╷ ", "  │ ", " ═╪ ", "  │ ", " ─┼ ", " ─┼ ", "  │ ", "  ╵ ",
    ],
    false,
);
pub const SIMPLE: BoxChars = parse(
    [
        "    ", "    ", " ── ", "    ", "    ", " ── ", "    ", "    ",
    ],
    false,
);
pub const SIMPLE_HEAD: BoxChars = parse(
    [
        "    ", "    ", " ── ", "    ", "    ", "    ", "    ", "    ",
    ],
    false,
);
pub const SIMPLE_HEAVY: BoxChars = parse(
    [
        "    ", "    ", " ━━ ", "    ", "    ", " ━━ ", "    ", "    ",
    ],
    false,
);
pub const HORIZONTALS: BoxChars = parse(
    [
        " ── ", "    ", " ── ", "    ", " ── ", " ── ", "    ", " ── ",
    ],
    false,
);
pub const ROUNDED: BoxChars = parse(
    [
        "╭─┬╮",
        "│ ││",
        "├─┼┤",
        "│ ││",
        "├─┼┤",
        "├─┼┤",
        "│ ││",
        "╰─┴╯",
    ],
    false,
);
pub const HEAVY: BoxChars = parse(
    [
        "┏━┳┓",
        "┃ ┃┃",
        "┣━╋┫",
        "┃ ┃┃",
        "┣━╋┫",
        "┣━╋┫",
        "┃ ┃┃",
        "┗━┻┛",
    ],
    false,
);
pub const HEAVY_EDGE: BoxChars = parse(
    [
        "┏━┯┓",
        "┃ │┃",
        "┠─┼┨",
        "┃ │┃",
        "┠─┼┨",
        "┠─┼┨",
        "┃ │┃",
        "┗━┷┛",
    ],
    false,
);
pub const HEAVY_HEAD: BoxChars = parse(
    [
        "┏━┳┓",
        "┃ ┃┃",
        "┡━╇┩",
        "│ ││",
        "├─┼┤",
        "├─┼┤",
        "│ ││",
        "└─┴┘",
    ],
    false,
);
pub const DOUBLE: BoxChars = parse(
    [
        "╔═╦╗",
        "║ ║║",
        "╠═╬╣",
        "║ ║║",
        "╠═╬╣",
        "╠═╬╣",
        "║ ║║",
        "╚═╩╝",
    ],
    false,
);
pub const DOUBLE_EDGE: BoxChars = parse(
    [
        "╔═╤╗",
        "║ │║",
        "╟─┼╢",
        "║ │║",
        "╟─┼╢",
        "╟─┼╢",
        "║ │║",
        "╚═╧╝",
    ],
    false,
);
pub const MARKDOWN: BoxChars = parse(
    [
        "    ", "| ||", "|-||", "| ||", "|-||", "|-||", "| ||", "    ",
    ],
    true,
);
