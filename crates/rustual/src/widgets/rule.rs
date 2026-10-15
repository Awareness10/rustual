//! `Rule` (port of `textual/widgets/rule.py`): a horizontal or vertical separator line.

use crate::context::Ctx;
use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::{Strip, Style};

const DEFAULT_CSS: &str = "
Rule {
    color: $secondary;
}
Rule.-horizontal {
    height: 1;
    margin: 1 0;
    width: 1fr;
}
Rule.-vertical {
    width: 1;
    margin: 0 2;
    height: 1fr;
}
";

/// Orientation of a [`Rule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleOrientation {
    #[default]
    Horizontal,
    Vertical,
}

impl RuleOrientation {
    pub fn class(&self) -> &'static str {
        match self {
            RuleOrientation::Horizontal => "-horizontal",
            RuleOrientation::Vertical => "-vertical",
        }
    }
}

/// Line style of a [`Rule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineStyle {
    Ascii,
    Blank,
    Dashed,
    Double,
    Heavy,
    Hidden,
    None,
    #[default]
    Solid,
    Thick,
}

impl LineStyle {
    /// Character used for a horizontal rule.
    pub fn horizontal_char(&self) -> &'static str {
        match self {
            LineStyle::Ascii => "-",
            LineStyle::Blank | LineStyle::Hidden | LineStyle::None => " ",
            LineStyle::Dashed => "╍",
            LineStyle::Double => "═",
            LineStyle::Heavy => "━",
            LineStyle::Solid => "─",
            LineStyle::Thick => "█",
        }
    }

    /// Character used for a vertical rule.
    pub fn vertical_char(&self) -> &'static str {
        match self {
            LineStyle::Ascii => "|",
            LineStyle::Blank | LineStyle::Hidden | LineStyle::None => " ",
            LineStyle::Dashed => "╏",
            LineStyle::Double => "║",
            LineStyle::Heavy => "┃",
            LineStyle::Solid => "│",
            LineStyle::Thick => "█",
        }
    }

    /// Parse a Textual line style name (`"solid"`, `"heavy"`, ...).
    pub fn parse(name: &str) -> Option<LineStyle> {
        Some(match name {
            "ascii" => LineStyle::Ascii,
            "blank" => LineStyle::Blank,
            "dashed" => LineStyle::Dashed,
            "double" => LineStyle::Double,
            "heavy" => LineStyle::Heavy,
            "hidden" => LineStyle::Hidden,
            "none" => LineStyle::None,
            "solid" => LineStyle::Solid,
            "thick" => LineStyle::Thick,
            _ => return None,
        })
    }
}

/// A rule to separate content, like an HTML `<hr>`.
#[derive(Debug, Clone, Default)]
pub struct Rule {
    orientation: RuleOrientation,
    line_style: LineStyle,
}

impl Rule {
    pub fn new(orientation: RuleOrientation, line_style: LineStyle) -> Self {
        Rule {
            orientation,
            line_style,
        }
    }

    pub fn horizontal() -> Self {
        Rule::new(RuleOrientation::Horizontal, LineStyle::Solid)
    }

    pub fn vertical() -> Self {
        Rule::new(RuleOrientation::Vertical, LineStyle::Solid)
    }

    pub fn line_style(mut self, line_style: LineStyle) -> Self {
        self.line_style = line_style;
        self
    }

    pub fn get_orientation(&self) -> RuleOrientation {
        self.orientation
    }

    pub fn get_line_style(&self) -> LineStyle {
        self.line_style
    }

    pub fn set_line_style(&mut self, line_style: LineStyle) {
        self.line_style = line_style;
    }

    pub fn set_orientation(&mut self, orientation: RuleOrientation, ctx: &mut Ctx) {
        ctx.remove_class(self.orientation.class());
        self.orientation = orientation;
        ctx.add_class(orientation.class());
    }
}

impl Widget for Rule {
    fn type_names(&self) -> &'static [&'static str] {
        &["Rule"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.add_class(self.orientation.class());
    }

    fn content_width(&self, ctx: &RenderCtx) -> Option<i32> {
        Some(match self.orientation {
            RuleOrientation::Horizontal => ctx.size.width,
            RuleOrientation::Vertical => 1,
        })
    }

    fn content_height(&self, ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(match self.orientation {
            RuleOrientation::Horizontal => 1,
            RuleOrientation::Vertical => ctx.size.height,
        })
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let size = ctx.size;
        let lines = match self.orientation {
            RuleOrientation::Horizontal => vec![Strip::from_str(
                &self
                    .line_style
                    .horizontal_char()
                    .repeat(size.width.max(0) as usize),
                Style::new(),
            )],
            RuleOrientation::Vertical => (0..size.height)
                .map(|_| Strip::from_str(self.line_style.vertical_char(), Style::new()))
                .collect(),
        };
        Content::Lines(lines)
    }
}
