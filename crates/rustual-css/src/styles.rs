//! Typed style properties. Every field is `Option`: `None` = not set by any rule.
//! Getter methods return the effective value with Textual's defaults applied.

use crate::properties::{inherit_text_style, split_declarations, strip_important};
use crate::scalar::Scalar;
use crate::transition::Transition;
use rustual_render::{BorderKind, Color, Spacing, Style};

/// `if other.f.is_some() { self.f = other.f.clone() }` for each field (fields are bound by
/// the destructuring in `merge`).
macro_rules! overlay {
    ($self:ident; $($f:ident),* $(,)?) => {
        $( if $f.is_some() { $self.$f = $f.clone(); } )*
    };
}

/// Fill unset fields from the parent.
macro_rules! fill {
    ($self:ident, $parent:ident; $($f:ident),* $(,)?) => {
        $( if $self.$f.is_none() { $self.$f = $parent.$f.clone(); } )*
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Block,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Visible,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutKind {
    #[default]
    Vertical,
    Horizontal,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoxSizing {
    #[default]
    BorderBox,
    ContentBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Hidden,
    Auto,
    Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignH {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignV {
    #[default]
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Start,
    End,
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dock {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarGutter {
    #[default]
    Auto,
    Stable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextWrap {
    #[default]
    Wrap,
    NoWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextOverflowKind {
    #[default]
    Fold,
    Clip,
    Ellipsis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

/// One border edge: kind + color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderEdge {
    pub kind: BorderKind,
    pub color: Color,
}

/// Text attributes from `text-style: bold italic` etc. Only `Some` flags are applied;
/// `none` sets all to `Some(false)`; `not bold` sets bold `Some(false)`.
pub type TextStyle = Style; // only attribute fields are used, never colors

/// A color value that may be `auto` (contrast text color with given alpha).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorValue {
    Color(Color),
    /// `color: auto 87%` → pick contrasting white/black over the background, alpha 0.87.
    Auto(f32),
}

/// `hatch: <left|right|cross|horizontal|vertical|"c"> <color> [<alpha>%]`: blank cells inside
/// the padding box are drawn as `character` in `color` over the background (Textual `hatch`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hatch {
    pub character: char,
    pub color: Color,
}

impl Hatch {
    /// `hatch: none`: stored like any value (so it overrides a less specific rule) but inactive.
    pub const NONE: Hatch = Hatch {
        character: ' ',
        color: Color::TRANSPARENT,
    };

    /// Keyword → character (Textual `HATCHES`).
    pub const KEYWORDS: &'static [(&'static str, char)] = &[
        ("left", '╲'),
        ("right", '╱'),
        ("cross", '╳'),
        ("horizontal", '─'),
        ("vertical", '│'),
    ];

    /// Whether the hatch draws anything (Textual skips a space character or a transparent color).
    pub fn is_active(&self) -> bool {
        self.character != ' ' && !self.color.is_transparent()
    }
}

/// All supported style properties. Field names map to CSS names with `_` → `-`
/// (e.g. `content_align_horizontal` ← `content-align-horizontal`, also set by `content-align: center middle`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Styles {
    pub display: Option<Display>,
    pub visibility: Option<Visibility>,
    pub layout: Option<LayoutKind>,
    pub position: Option<Position>,

    pub width: Option<Scalar>,
    pub height: Option<Scalar>,
    pub min_width: Option<Scalar>,
    pub min_height: Option<Scalar>,
    pub max_width: Option<Scalar>,
    pub max_height: Option<Scalar>,
    pub box_sizing: Option<BoxSizing>,

    pub margin: Option<Spacing>,
    pub padding: Option<Spacing>,

    /// `border: <kind> [<color>] [<alpha>%]`, `border-top: ...` etc. Missing color: the stylesheet parser
    /// uses the `$foreground` variable when defined, otherwise WHITE (Textual itself uses green).
    pub border_top: Option<BorderEdge>,
    pub border_right: Option<BorderEdge>,
    pub border_bottom: Option<BorderEdge>,
    pub border_left: Option<BorderEdge>,
    /// Outline: drawn over the content area edge, takes no space.
    pub outline_top: Option<BorderEdge>,
    pub outline_right: Option<BorderEdge>,
    pub outline_bottom: Option<BorderEdge>,
    pub outline_left: Option<BorderEdge>,
    pub border_title_align: Option<AlignH>,
    pub border_subtitle_align: Option<AlignH>,
    pub border_title_color: Option<Color>,
    pub border_title_background: Option<Color>,
    pub border_title_style: Option<TextStyle>,
    pub border_subtitle_color: Option<Color>,
    pub border_subtitle_background: Option<Color>,
    pub border_subtitle_style: Option<TextStyle>,

    pub color: Option<ColorValue>,
    pub background: Option<Color>,
    pub background_tint: Option<Color>,
    pub tint: Option<Color>,
    pub hatch: Option<Hatch>,
    pub opacity: Option<f32>,
    pub text_opacity: Option<f32>,
    pub text_style: Option<TextStyle>,
    pub text_align: Option<TextAlign>,
    pub text_wrap: Option<TextWrap>,
    pub text_overflow: Option<TextOverflowKind>,

    pub content_align_horizontal: Option<AlignH>,
    pub content_align_vertical: Option<AlignV>,
    pub align_horizontal: Option<AlignH>,
    pub align_vertical: Option<AlignV>,

    pub dock: Option<Dock>,
    pub offset_x: Option<Scalar>,
    pub offset_y: Option<Scalar>,
    pub layer: Option<String>,
    pub layers: Option<Vec<String>>,

    pub overflow_x: Option<Overflow>,
    pub overflow_y: Option<Overflow>,
    pub scrollbar_gutter: Option<ScrollbarGutter>,
    /// Width of the vertical scrollbar. `scrollbar-size: <horizontal> <vertical>` (Textual order).
    pub scrollbar_size_vertical: Option<i32>,
    /// Height of the horizontal scrollbar (first value of `scrollbar-size`).
    pub scrollbar_size_horizontal: Option<i32>,
    pub scrollbar_color: Option<Color>,
    pub scrollbar_color_hover: Option<Color>,
    pub scrollbar_color_active: Option<Color>,
    pub scrollbar_background: Option<Color>,
    pub scrollbar_background_hover: Option<Color>,
    pub scrollbar_background_active: Option<Color>,
    pub scrollbar_corner_color: Option<Color>,

    pub grid_size_columns: Option<i32>,
    pub grid_size_rows: Option<i32>,
    pub grid_columns: Option<Vec<Scalar>>,
    pub grid_rows: Option<Vec<Scalar>>,
    /// Textual semantics: the gutter *between rows* (a horizontal band). Set by the FIRST
    /// value of `grid-gutter: <horizontal> [<vertical>]` (see `_styles_builder.py`
    /// `process_grid_gutter` and `layouts/grid.py`, which resolves rows with this value).
    pub grid_gutter_horizontal: Option<i32>,
    /// Textual semantics: the gutter *between columns* (a vertical band). Second value of
    /// `grid-gutter` (defaults to the first when only one value is given).
    pub grid_gutter_vertical: Option<i32>,
    pub column_span: Option<i32>,
    pub row_span: Option<i32>,

    pub link_color: Option<Color>,
    pub link_background: Option<Color>,
    pub link_style: Option<TextStyle>,
    pub link_color_hover: Option<Color>,
    pub link_background_hover: Option<Color>,
    pub link_style_hover: Option<TextStyle>,

    /// `transition: <property> <duration> [<easing>] [<delay>], ...` (not inherited).
    pub transitions: Option<Vec<Transition>>,
}

impl Styles {
    pub fn new() -> Self {
        Self::default()
    }

    /// Overlay: every `Some` field in `other` replaces the field in `self`.
    pub fn merge(&mut self, other: &Styles) {
        // Exhaustive destructuring: adding a field to `Styles` without handling it here
        // is a compile error.
        let Styles {
            display,
            visibility,
            layout,
            position,
            width,
            height,
            min_width,
            min_height,
            max_width,
            max_height,
            box_sizing,
            margin,
            padding,
            border_top,
            border_right,
            border_bottom,
            border_left,
            outline_top,
            outline_right,
            outline_bottom,
            outline_left,
            border_title_align,
            border_subtitle_align,
            border_title_color,
            border_title_background,
            border_title_style,
            border_subtitle_color,
            border_subtitle_background,
            border_subtitle_style,
            color,
            background,
            background_tint,
            tint,
            hatch,
            opacity,
            text_opacity,
            text_style,
            text_align,
            text_wrap,
            text_overflow,
            content_align_horizontal,
            content_align_vertical,
            align_horizontal,
            align_vertical,
            dock,
            offset_x,
            offset_y,
            layer,
            layers,
            overflow_x,
            overflow_y,
            scrollbar_gutter,
            scrollbar_size_vertical,
            scrollbar_size_horizontal,
            scrollbar_color,
            scrollbar_color_hover,
            scrollbar_color_active,
            scrollbar_background,
            scrollbar_background_hover,
            scrollbar_background_active,
            scrollbar_corner_color,
            grid_size_columns,
            grid_size_rows,
            grid_columns,
            grid_rows,
            grid_gutter_horizontal,
            grid_gutter_vertical,
            column_span,
            row_span,
            link_color,
            link_background,
            link_style,
            link_color_hover,
            link_background_hover,
            link_style_hover,
            transitions,
        } = other;
        overlay!(self; display, visibility, layout, position, width, height, min_width, min_height, max_width, max_height, box_sizing, margin, padding, border_top, border_right, border_bottom, border_left, outline_top, outline_right, outline_bottom, outline_left, border_title_align, border_subtitle_align, border_title_color, border_title_background, border_title_style, border_subtitle_color, border_subtitle_background, border_subtitle_style, color, background, background_tint, tint, hatch, opacity, text_opacity, text_style, text_align, text_wrap, text_overflow, content_align_horizontal, content_align_vertical, align_horizontal, align_vertical, dock, offset_x, offset_y, layer, layers, overflow_x, overflow_y, scrollbar_gutter, scrollbar_size_vertical, scrollbar_size_horizontal, scrollbar_color, scrollbar_color_hover, scrollbar_color_active, scrollbar_background, scrollbar_background_hover, scrollbar_background_active, scrollbar_corner_color, grid_size_columns, grid_size_rows, grid_columns, grid_rows, grid_gutter_horizontal, grid_gutter_vertical, column_span, row_span, link_color, link_background, link_style, link_color_hover, link_background_hover, link_style_hover, transitions);
    }

    /// Fill unset *inherited* properties from the parent's computed styles. Inherited:
    /// color, text_style, text_align, text_wrap, text_overflow, text_opacity? (no — not inherited),
    /// link_* properties. Everything else is not inherited.
    pub fn inherit_from(&mut self, parent: &Styles) {
        fill!(self, parent; color, text_align, text_wrap, text_overflow, link_color,
            link_background, link_color_hover, link_background_hover);
        self.text_style = inherit_text_style(self.text_style.take(), &parent.text_style);
        self.link_style = inherit_text_style(self.link_style.take(), &parent.link_style);
        self.link_style_hover =
            inherit_text_style(self.link_style_hover.take(), &parent.link_style_hover);
    }

    /// The transition for CSS property `property` (`"offset"`, `"text-opacity"`; `_` is
    /// accepted for `-`), if any.
    pub fn get_transition(&self, property: &str) -> Option<&Transition> {
        let property = property.replace('_', "-");
        self.transitions
            .as_ref()?
            .iter()
            .rev()
            .find(|t| t.property == property)
    }

    // ---- effective-value getters (Textual defaults) ----
    pub fn get_display(&self) -> Display {
        self.display.unwrap_or_default()
    }
    pub fn get_visibility(&self) -> Visibility {
        self.visibility.unwrap_or_default()
    }
    pub fn get_layout(&self) -> LayoutKind {
        self.layout.unwrap_or_default()
    }
    pub fn get_box_sizing(&self) -> BoxSizing {
        self.box_sizing.unwrap_or_default()
    }
    pub fn get_margin(&self) -> Spacing {
        self.margin.unwrap_or_default()
    }
    pub fn get_padding(&self) -> Spacing {
        self.padding.unwrap_or_default()
    }
    pub fn get_background(&self) -> Color {
        self.background.unwrap_or(Color::TRANSPARENT)
    }
    /// The hatch to draw, if any (`None` when unset, `none`, or invisible).
    pub fn get_hatch(&self) -> Option<Hatch> {
        self.hatch.filter(Hatch::is_active)
    }
    pub fn get_opacity(&self) -> f32 {
        self.opacity.unwrap_or(1.0)
    }
    pub fn get_text_opacity(&self) -> f32 {
        self.text_opacity.unwrap_or(1.0)
    }
    pub fn get_overflow_x(&self) -> Overflow {
        self.overflow_x.unwrap_or_default()
    }
    pub fn get_overflow_y(&self) -> Overflow {
        self.overflow_y.unwrap_or_default()
    }
    pub fn get_scrollbar_size_vertical(&self) -> i32 {
        self.scrollbar_size_vertical.unwrap_or(2)
    }
    pub fn get_scrollbar_size_horizontal(&self) -> i32 {
        self.scrollbar_size_horizontal.unwrap_or(1)
    }
    pub fn get_text_align(&self) -> TextAlign {
        self.text_align.unwrap_or_default()
    }
    pub fn get_content_align(&self) -> (AlignH, AlignV) {
        (
            self.content_align_horizontal.unwrap_or_default(),
            self.content_align_vertical.unwrap_or_default(),
        )
    }
    pub fn get_align(&self) -> (AlignH, AlignV) {
        (
            self.align_horizontal.unwrap_or_default(),
            self.align_vertical.unwrap_or_default(),
        )
    }
    pub fn get_grid_size(&self) -> (i32, i32) {
        (
            self.grid_size_columns.unwrap_or(1),
            self.grid_size_rows.unwrap_or(0),
        )
    }
    pub fn get_grid_gutter(&self) -> (i32, i32) {
        (
            self.grid_gutter_horizontal.unwrap_or(0),
            self.grid_gutter_vertical.unwrap_or(0),
        )
    }

    /// Border edges as (top, right, bottom, left); unset edges are `BorderKind::None`.
    pub fn get_border(&self) -> [BorderEdge; 4] {
        let none = BorderEdge {
            kind: BorderKind::None,
            color: Color::TRANSPARENT,
        };
        [
            self.border_top.unwrap_or(none),
            self.border_right.unwrap_or(none),
            self.border_bottom.unwrap_or(none),
            self.border_left.unwrap_or(none),
        ]
    }

    pub fn get_outline(&self) -> [BorderEdge; 4] {
        let none = BorderEdge {
            kind: BorderKind::None,
            color: Color::TRANSPARENT,
        };
        [
            self.outline_top.unwrap_or(none),
            self.outline_right.unwrap_or(none),
            self.outline_bottom.unwrap_or(none),
            self.outline_left.unwrap_or(none),
        ]
    }

    /// Cells taken by borders: 1 per edge whose kind has width.
    pub fn border_spacing(&self) -> Spacing {
        let [t, r, b, l] = self.get_border();
        let w = |e: BorderEdge| if e.kind.has_width() { 1 } else { 0 };
        Spacing::new(w(t), w(r), w(b), w(l))
    }

    /// Padding + border (the "gutter" between outer box and content).
    pub fn gutter(&self) -> Spacing {
        self.get_padding() + self.border_spacing()
    }

    /// Parse a single declaration (`name`, raw value text with variables already substituted)
    /// and set the corresponding field(s). Shorthands (`border`, `margin`, `padding`, `align`,
    /// `content-align`, `overflow`, `offset`, `grid-size`, `grid-gutter`, `scrollbar-size`,
    /// `outline`) set several fields.
    pub fn set_declaration(&mut self, name: &str, value: &str) -> Result<(), String> {
        self.set_declaration_with(name, value, Color::WHITE)
    }

    /// Parse inline-style text like `"width: 10; color: red"`.
    pub fn parse_inline(css: &str) -> Result<Styles, String> {
        let mut styles = Styles::new();
        for part in split_declarations(css) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (name, value) = part
                .split_once(':')
                .ok_or_else(|| format!("expected 'name: value', found {part:?}"))?;
            let (value, _important) = strip_important(value);
            styles.set_declaration(name.trim(), value)?;
        }
        Ok(styles)
    }
}
