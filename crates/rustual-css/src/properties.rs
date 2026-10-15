//! Declaration value parsing: maps `name: value` onto [`Styles`] fields
//! (port of `textual/css/_styles_builder.py`).

use crate::scalar::{Scalar, Unit};
use crate::styles::*;
use rustual_render::{BorderKind, Color, Spacing, Style};

/// Properties Textual supports that rustual does not model. Declarations of these are
/// accepted and ignored so real Textual CSS loads without errors.
pub const IGNORED_PROPERTIES: &[&str] = &[
    "hatch",
    "keyline",
    "pointer",
    "constrain",
    "constrain-x",
    "constrain-y",
    "overlay",
    "expand",
    "line-pad",
    "split",
    "scrollbar-visibility",
];

const UNKNOWN: &str = "unknown property";

impl Styles {
    /// Like [`Styles::set_declaration`], but `border`/`outline` declarations without a color
    /// use `default_border_color` (the parser passes the `$foreground` theme variable when
    /// available; Textual itself uses `Color(0, 255, 0)`).
    ///
    /// The special value `initial` is accepted for every property and leaves it unchanged.
    /// A trailing `!important` is ignored (the parser strips it before calling this).
    pub fn set_declaration_with(
        &mut self,
        name: &str,
        value: &str,
        default_border_color: Color,
    ) -> Result<(), String> {
        let name = name.trim();
        let (value, _important) = strip_important(value);
        let tokens = split_values(value);
        if IGNORED_PROPERTIES.contains(&name) {
            return Ok(());
        }
        if tokens.is_empty() {
            return if is_known_property(name) {
                Err(format!("missing value for '{name}'"))
            } else {
                Err(format!("{UNKNOWN} '{name}'"))
            };
        }
        if tokens.len() == 1 && tokens[0] == "initial" {
            return if is_known_property(name) {
                Ok(())
            } else {
                Err(format!("{UNKNOWN} '{name}'"))
            };
        }
        let t: Vec<&str> = tokens.iter().map(String::as_str).collect();
        self.apply(name, &t, default_border_color)
    }

    fn apply(&mut self, name: &str, t: &[&str], border_color: Color) -> Result<(), String> {
        let edge = |t: &[&str]| border_edge(name, t, border_color);
        match name {
            "display" => self.display = Some(enum_value(name, t, DISPLAY)?),
            "visibility" => self.visibility = Some(enum_value(name, t, VISIBILITY)?),
            "layout" => self.layout = Some(enum_value(name, t, LAYOUT)?),
            "position" => self.position = Some(enum_value(name, t, POSITION)?),
            "box-sizing" => self.box_sizing = Some(enum_value(name, t, BOX_SIZING)?),

            "width" => self.width = Some(scalar(name, t)?),
            "height" => self.height = Some(scalar(name, t)?),
            "min-width" => self.min_width = Some(scalar(name, t)?),
            "min-height" => self.min_height = Some(scalar(name, t)?),
            "max-width" => self.max_width = Some(scalar(name, t)?),
            "max-height" => self.max_height = Some(scalar(name, t)?),

            "margin" => self.margin = Some(spacing(name, t)?),
            "padding" => self.padding = Some(spacing(name, t)?),
            "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
                let v = int(name, one(name, t)?)?;
                self.margin = Some(set_edge(self.margin.unwrap_or_default(), name, v));
            }
            "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
                let v = int(name, one(name, t)?)?;
                self.padding = Some(set_edge(self.padding.unwrap_or_default(), name, v));
            }

            "border" => {
                let e = Some(edge(t)?);
                self.border_top = e;
                self.border_right = e;
                self.border_bottom = e;
                self.border_left = e;
            }
            "border-top" => self.border_top = Some(edge(t)?),
            "border-right" => self.border_right = Some(edge(t)?),
            "border-bottom" => self.border_bottom = Some(edge(t)?),
            "border-left" => self.border_left = Some(edge(t)?),
            "outline" => {
                let e = Some(edge(t)?);
                self.outline_top = e;
                self.outline_right = e;
                self.outline_bottom = e;
                self.outline_left = e;
            }
            "outline-top" => self.outline_top = Some(edge(t)?),
            "outline-right" => self.outline_right = Some(edge(t)?),
            "outline-bottom" => self.outline_bottom = Some(edge(t)?),
            "outline-left" => self.outline_left = Some(edge(t)?),

            "border-title-align" => self.border_title_align = Some(enum_value(name, t, ALIGN_H)?),
            "border-subtitle-align" => {
                self.border_subtitle_align = Some(enum_value(name, t, ALIGN_H)?)
            }
            "border-title-color" => self.border_title_color = Some(plain_color(name, t)?),
            "border-title-background" => self.border_title_background = Some(plain_color(name, t)?),
            "border-title-style" => self.border_title_style = Some(text_style(t)?),
            "border-subtitle-color" => self.border_subtitle_color = Some(plain_color(name, t)?),
            "border-subtitle-background" => {
                self.border_subtitle_background = Some(plain_color(name, t)?)
            }
            "border-subtitle-style" => self.border_subtitle_style = Some(text_style(t)?),

            "color" => self.color = Some(color_value(name, t, true)?),
            "background" => self.background = Some(plain_color(name, t)?),
            "background-tint" => self.background_tint = Some(plain_color(name, t)?),
            "tint" => self.tint = Some(plain_color(name, t)?),
            "opacity" => self.opacity = Some(fraction(name, t)?),
            "text-opacity" => self.text_opacity = Some(fraction(name, t)?),
            "text-style" => self.text_style = Some(text_style(t)?),
            "text-align" => self.text_align = Some(enum_value(name, t, TEXT_ALIGN)?),
            "text-wrap" => self.text_wrap = Some(enum_value(name, t, TEXT_WRAP)?),
            "text-overflow" => self.text_overflow = Some(enum_value(name, t, TEXT_OVERFLOW)?),

            "content-align" => {
                let (h, v) = align_pair(name, t)?;
                self.content_align_horizontal = Some(h);
                self.content_align_vertical = Some(v);
            }
            "content-align-horizontal" => {
                self.content_align_horizontal = Some(enum_value(name, t, ALIGN_H)?)
            }
            "content-align-vertical" => {
                self.content_align_vertical = Some(enum_value(name, t, ALIGN_V)?)
            }
            "align" => {
                let (h, v) = align_pair(name, t)?;
                self.align_horizontal = Some(h);
                self.align_vertical = Some(v);
            }
            "align-horizontal" => self.align_horizontal = Some(enum_value(name, t, ALIGN_H)?),
            "align-vertical" => self.align_vertical = Some(enum_value(name, t, ALIGN_V)?),

            "dock" => {
                // `dock: none` is valid in Textual; with Option fields it means "not docked".
                self.dock = if t == ["none"] {
                    None
                } else {
                    Some(enum_value(name, t, DOCK)?)
                };
            }
            "offset" => {
                let [x, y] = t else {
                    return Err(format!("'{name}' expects two values (x y)"));
                };
                self.offset_x = Some(axis_scalar(name, x, Unit::Width)?);
                self.offset_y = Some(axis_scalar(name, y, Unit::Height)?);
            }
            "offset-x" => self.offset_x = Some(axis_scalar(name, one(name, t)?, Unit::Width)?),
            "offset-y" => self.offset_y = Some(axis_scalar(name, one(name, t)?, Unit::Height)?),
            "layer" => self.layer = Some(one(name, t)?.trim_matches('"').to_string()),
            "layers" => {
                self.layers = Some(t.iter().map(|s| s.trim_matches('"').to_string()).collect())
            }

            "overflow" => {
                let (x, y) = match t {
                    [v] => (*v, *v),
                    [x, y] => (*x, *y),
                    _ => return Err(format!("'{name}' expects one or two values (x y)")),
                };
                self.overflow_x = Some(enum_value(name, &[x], OVERFLOW)?);
                self.overflow_y = Some(enum_value(name, &[y], OVERFLOW)?);
            }
            "overflow-x" => self.overflow_x = Some(enum_value(name, t, OVERFLOW)?),
            "overflow-y" => self.overflow_y = Some(enum_value(name, t, OVERFLOW)?),
            "scrollbar-gutter" => self.scrollbar_gutter = Some(enum_value(name, t, GUTTER)?),
            "scrollbar-size" => {
                // Textual order: `scrollbar-size: <horizontal> <vertical>`.
                let [h, v] = t else {
                    return Err(format!(
                        "'{name}' expects two integers (horizontal vertical)"
                    ));
                };
                self.scrollbar_size_horizontal = Some(non_negative(name, h)?);
                self.scrollbar_size_vertical = Some(non_negative(name, v)?);
            }
            "scrollbar-size-vertical" => {
                self.scrollbar_size_vertical = Some(non_negative(name, one(name, t)?)?)
            }
            "scrollbar-size-horizontal" => {
                self.scrollbar_size_horizontal = Some(non_negative(name, one(name, t)?)?)
            }
            "scrollbar-color" => self.scrollbar_color = Some(plain_color(name, t)?),
            "scrollbar-color-hover" => self.scrollbar_color_hover = Some(plain_color(name, t)?),
            "scrollbar-color-active" => self.scrollbar_color_active = Some(plain_color(name, t)?),
            "scrollbar-background" => self.scrollbar_background = Some(plain_color(name, t)?),
            "scrollbar-background-hover" => {
                self.scrollbar_background_hover = Some(plain_color(name, t)?)
            }
            "scrollbar-background-active" => {
                self.scrollbar_background_active = Some(plain_color(name, t)?)
            }
            "scrollbar-corner-color" => self.scrollbar_corner_color = Some(plain_color(name, t)?),

            "grid-size" => match t {
                [c] => {
                    self.grid_size_columns = Some(int(name, c)?.max(0));
                    self.grid_size_rows = Some(0);
                }
                [c, r] => {
                    self.grid_size_columns = Some(int(name, c)?.max(0));
                    self.grid_size_rows = Some(int(name, r)?.max(0));
                }
                _ => return Err(format!("'{name}' expects one or two integers")),
            },
            "grid-size-columns" => self.grid_size_columns = Some(positive(name, one(name, t)?)?),
            "grid-size-rows" => self.grid_size_rows = Some(positive(name, one(name, t)?)?),
            "grid-columns" => self.grid_columns = Some(scalar_list(name, t, Unit::Width)?),
            "grid-rows" => self.grid_rows = Some(scalar_list(name, t, Unit::Height)?),
            "grid-gutter" => match t {
                // Textual: first value → grid_gutter_horizontal, second → grid_gutter_vertical.
                [v] => {
                    let v = int(name, v)?.max(0);
                    self.grid_gutter_horizontal = Some(v);
                    self.grid_gutter_vertical = Some(v);
                }
                [h, v] => {
                    self.grid_gutter_horizontal = Some(int(name, h)?.max(0));
                    self.grid_gutter_vertical = Some(int(name, v)?.max(0));
                }
                _ => return Err(format!("'{name}' expects one or two integers")),
            },
            "grid-gutter-horizontal" => {
                self.grid_gutter_horizontal = Some(non_negative(name, one(name, t)?)?)
            }
            "grid-gutter-vertical" => {
                self.grid_gutter_vertical = Some(non_negative(name, one(name, t)?)?)
            }
            "column-span" => self.column_span = Some(positive(name, one(name, t)?)?),
            "row-span" => self.row_span = Some(positive(name, one(name, t)?)?),

            "link-color" => self.link_color = Some(plain_color(name, t)?),
            "link-background" => self.link_background = Some(plain_color(name, t)?),
            "link-style" => self.link_style = Some(text_style(t)?),
            "link-color-hover" => self.link_color_hover = Some(plain_color(name, t)?),
            "link-background-hover" => self.link_background_hover = Some(plain_color(name, t)?),
            "link-style-hover" => self.link_style_hover = Some(text_style(t)?),

            "transition" => {
                self.transitions = Some(crate::transition::parse_transitions(&t.join(" "))?)
            }

            _ => return Err(format!("{UNKNOWN} '{name}'")),
        }
        Ok(())
    }
}

/// True for every property name [`Styles::set_declaration`] understands (including the
/// [`IGNORED_PROPERTIES`]).
pub fn is_known_property(name: &str) -> bool {
    if IGNORED_PROPERTIES.contains(&name) {
        return true;
    }
    // Probe with a value every property rejects for a *value* reason, not a name reason.
    let mut probe = Styles::new();
    match probe.apply(name, &["\u{1}"], Color::WHITE) {
        Ok(()) => true,
        Err(e) => !e.starts_with(UNKNOWN),
    }
}

/// Split declaration text on `;`, ignoring `;` inside parentheses or quotes.
pub(crate) fn split_declarations(css: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in css.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth = depth.saturating_sub(1),
            (None, ';') if depth == 0 => {
                out.push(&css[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&css[start..]);
    out
}

/// Remove a trailing `!important`; returns the remaining value and whether it was present.
pub(crate) fn strip_important(value: &str) -> (&str, bool) {
    let v = value.trim();
    match v.strip_suffix("!important") {
        Some(rest) => (rest.trim_end(), true),
        None => (v, false),
    }
}

/// Split a value into whitespace-separated tokens, keeping `rgb(1, 2, 3)` and quoted strings
/// together.
pub(crate) fn split_values(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for c in value.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => {
                quote = None;
                cur.push(c);
            }
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                cur.push(c);
            }
            (None, '(') => {
                depth += 1;
                cur.push(c);
            }
            (None, ')') => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            (None, c) if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

const DISPLAY: &[(&str, Display)] = &[("block", Display::Block), ("none", Display::None)];
const VISIBILITY: &[(&str, Visibility)] = &[
    ("visible", Visibility::Visible),
    ("hidden", Visibility::Hidden),
];
/// `stream` (a Textual layout we don't implement) falls back to vertical.
const LAYOUT: &[(&str, LayoutKind)] = &[
    ("vertical", LayoutKind::Vertical),
    ("horizontal", LayoutKind::Horizontal),
    ("grid", LayoutKind::Grid),
    ("stream", LayoutKind::Vertical),
];
const POSITION: &[(&str, Position)] = &[
    ("relative", Position::Relative),
    ("absolute", Position::Absolute),
];
const BOX_SIZING: &[(&str, BoxSizing)] = &[
    ("border-box", BoxSizing::BorderBox),
    ("content-box", BoxSizing::ContentBox),
];
const ALIGN_H: &[(&str, AlignH)] = &[
    ("left", AlignH::Left),
    ("center", AlignH::Center),
    ("right", AlignH::Right),
];
const ALIGN_V: &[(&str, AlignV)] = &[
    ("top", AlignV::Top),
    ("middle", AlignV::Middle),
    ("bottom", AlignV::Bottom),
];
const TEXT_ALIGN: &[(&str, TextAlign)] = &[
    ("start", TextAlign::Start),
    ("end", TextAlign::End),
    ("left", TextAlign::Left),
    ("center", TextAlign::Center),
    ("right", TextAlign::Right),
    ("justify", TextAlign::Justify),
];
const TEXT_WRAP: &[(&str, TextWrap)] = &[("wrap", TextWrap::Wrap), ("nowrap", TextWrap::NoWrap)];
const TEXT_OVERFLOW: &[(&str, TextOverflowKind)] = &[
    ("fold", TextOverflowKind::Fold),
    ("clip", TextOverflowKind::Clip),
    ("ellipsis", TextOverflowKind::Ellipsis),
];
const DOCK: &[(&str, Dock)] = &[
    ("top", Dock::Top),
    ("right", Dock::Right),
    ("bottom", Dock::Bottom),
    ("left", Dock::Left),
];
const OVERFLOW: &[(&str, Overflow)] = &[
    ("hidden", Overflow::Hidden),
    ("auto", Overflow::Auto),
    ("scroll", Overflow::Scroll),
];
const GUTTER: &[(&str, ScrollbarGutter)] = &[
    ("auto", ScrollbarGutter::Auto),
    ("stable", ScrollbarGutter::Stable),
];

/// Border type names (Textual `VALID_BORDER`), parsed locally rather than via
/// `BorderKind::from_str`.
pub fn parse_border_kind(name: &str) -> Option<BorderKind> {
    Some(match name.to_ascii_lowercase().as_str() {
        "none" => BorderKind::None,
        "hidden" => BorderKind::Hidden,
        "ascii" => BorderKind::Ascii,
        "blank" => BorderKind::Blank,
        "dashed" => BorderKind::Dashed,
        "double" => BorderKind::Double,
        "heavy" => BorderKind::Heavy,
        "hkey" => BorderKind::Hkey,
        "inner" => BorderKind::Inner,
        "outer" => BorderKind::Outer,
        "panel" => BorderKind::Panel,
        "round" => BorderKind::Round,
        "solid" => BorderKind::Solid,
        "tall" => BorderKind::Tall,
        "tab" => BorderKind::Tab,
        "thick" => BorderKind::Thick,
        "block" => BorderKind::Block,
        "vkey" => BorderKind::Vkey,
        "wide" => BorderKind::Wide,
        _ => return None,
    })
}

fn one<'a>(name: &str, t: &[&'a str]) -> Result<&'a str, String> {
    match t {
        [v] => Ok(v),
        _ => Err(format!(
            "'{name}' expects a single value, found {:?}",
            t.join(" ")
        )),
    }
}

fn enum_value<T: Copy>(name: &str, t: &[&str], table: &[(&str, T)]) -> Result<T, String> {
    let v = one(name, t)?.to_ascii_lowercase();
    table
        .iter()
        .find(|(k, _)| *k == v)
        .map(|(_, val)| *val)
        .ok_or_else(|| {
            let valid: Vec<&str> = table.iter().map(|(k, _)| *k).collect();
            format!(
                "invalid value {v:?} for '{name}'; expected one of {}",
                valid.join(", ")
            )
        })
}

fn int(name: &str, s: &str) -> Result<i32, String> {
    s.parse::<i32>()
        .map_err(|_| format!("invalid integer {s:?} for '{name}'"))
}

fn non_negative(name: &str, s: &str) -> Result<i32, String> {
    let v = int(name, s)?;
    if v < 0 {
        return Err(format!("'{name}' must not be negative"));
    }
    Ok(v)
}

fn positive(name: &str, s: &str) -> Result<i32, String> {
    let v = int(name, s)?;
    if v <= 0 {
        return Err(format!("'{name}' must be a positive integer"));
    }
    Ok(v)
}

fn scalar(name: &str, t: &[&str]) -> Result<Scalar, String> {
    one(name, t)?
        .parse::<Scalar>()
        .map_err(|e| format!("'{name}': {e}"))
}

/// Scalars where `%` resolves against a specific axis (Textual's `percent_unit`).
fn axis_scalar(name: &str, s: &str, percent_unit: Unit) -> Result<Scalar, String> {
    let mut v: Scalar = s.parse().map_err(|e| format!("'{name}': {e}"))?;
    if v.unit == Unit::Percent {
        v.unit = percent_unit;
    }
    Ok(v)
}

fn scalar_list(name: &str, t: &[&str], percent_unit: Unit) -> Result<Vec<Scalar>, String> {
    t.iter()
        .map(|s| axis_scalar(name, s, percent_unit))
        .collect()
}

fn spacing(name: &str, t: &[&str]) -> Result<Spacing, String> {
    let values = t
        .iter()
        .map(|s| int(name, s))
        .collect::<Result<Vec<_>, _>>()?;
    Spacing::unpack(&values).ok_or_else(|| {
        format!(
            "'{name}' expects 1, 2 or 4 integers, found {}",
            values.len()
        )
    })
}

fn set_edge(mut s: Spacing, name: &str, v: i32) -> Spacing {
    match name.rsplit('-').next() {
        Some("top") => s.top = v,
        Some("right") => s.right = v,
        Some("bottom") => s.bottom = v,
        _ => s.left = v,
    }
    s
}

fn percent(s: &str) -> Option<f32> {
    s.strip_suffix('%')?.parse::<f32>().ok().map(|v| v / 100.0)
}

/// `0.5` or `50%`, clamped to 0..1.
fn fraction(name: &str, t: &[&str]) -> Result<f32, String> {
    let s = one(name, t)?;
    let v = match percent(s) {
        Some(v) => v,
        None => s.parse::<f32>().map_err(|_| {
            format!("invalid value {s:?} for '{name}'; expected a number or percentage")
        })?,
    };
    Ok(v.clamp(0.0, 1.0))
}

/// Parse a single color word with [`Color::parse`]. `ansi_default` (the terminal's default
/// color, which we can't express) maps to transparent.
pub(crate) fn parse_color_word(s: &str) -> Result<Color, String> {
    if s.eq_ignore_ascii_case("ansi_default") || s.eq_ignore_ascii_case("ansi-default") {
        return Ok(Color::TRANSPARENT);
    }
    Color::parse(s).map_err(|e| e.to_string())
}

/// `<color> [<alpha>%]`, `auto [<alpha>%]`, or `<alpha>%` (alpha applied to white), as in
/// Textual's `process_color`. The last alpha wins (`$text 90%` → `auto 87% 90%` → 0.9).
/// Plain `auto` is `Auto(1.0)`.
pub fn parse_color_value(value: &str) -> Result<ColorValue, String> {
    let words = split_values(value);
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    if words.is_empty() {
        return Err("missing color".into());
    }
    color_value("color", &words, true)
}

fn color_value(name: &str, t: &[&str], allow_auto: bool) -> Result<ColorValue, String> {
    let mut color = None;
    let mut auto = false;
    let mut alpha = None;
    for tok in t {
        if allow_auto && tok.eq_ignore_ascii_case("auto") {
            auto = true;
        } else if let Some(a) = percent(tok) {
            alpha = Some(a);
        } else {
            color = Some(parse_color_word(tok).map_err(|e| format!("'{name}': {e}"))?);
        }
    }
    if auto {
        return Ok(ColorValue::Auto(alpha.unwrap_or(1.0)));
    }
    let mut c = color.unwrap_or(Color::WHITE);
    if let Some(a) = alpha {
        c = c.multiply_alpha(a);
    }
    Ok(ColorValue::Color(c))
}

/// Color for fields that can't express `auto`. Background colors reject `auto` (as Textual
/// does); other color properties (e.g. `link-color: $link-color` where `$link-color` is
/// `auto 87%`) map `auto` to transparent, i.e. "defer to the underlying text color" when
/// composited.
fn plain_color(name: &str, t: &[&str]) -> Result<Color, String> {
    let allow_auto = !name.contains("background");
    match color_value(name, t, allow_auto)? {
        ColorValue::Color(c) => Ok(c),
        ColorValue::Auto(_) => Ok(Color::TRANSPARENT),
    }
}

/// `<border-type> [<color>] [<alpha>%]` in any order (Textual `_parse_border`). Missing type
/// is `solid`; missing color is `default_color`.
fn border_edge(name: &str, t: &[&str], default_color: Color) -> Result<BorderEdge, String> {
    let mut kind = BorderKind::Solid;
    let mut color = default_color;
    let mut alpha = None;
    for tok in t {
        if let Some(k) = parse_border_kind(tok) {
            kind = k;
        } else if let Some(a) = percent(tok) {
            alpha = Some(a);
        } else {
            color = parse_color_word(tok).map_err(|e| {
                format!("'{name}': expected '<border-type> [<color>] [<alpha>%]': {e}")
            })?;
        }
    }
    if let Some(a) = alpha {
        color = color.multiply_alpha(a);
    }
    Ok(BorderEdge { kind, color })
}

fn align_pair(name: &str, t: &[&str]) -> Result<(AlignH, AlignV), String> {
    let [h, v] = t else {
        return Err(format!("'{name}' expects two values (horizontal vertical)"));
    };
    Ok((
        enum_value(name, &[h], ALIGN_H)?,
        enum_value(name, &[v], ALIGN_V)?,
    ))
}

/// Parse `text-style` words (`bold italic`, `not underline`, `none`, `b reverse`) into a
/// [`Style`] with only attribute fields set. Implemented locally (not via `Style::parse`).
/// `none` sets every attribute to `Some(false)`; `not X` sets X to `Some(false)`.
pub fn parse_text_style(value: &str) -> Result<TextStyle, String> {
    let words = split_values(value);
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    text_style(&words)
}

fn text_style(t: &[&str]) -> Result<TextStyle, String> {
    let mut style = Style::default();
    let mut negate = false;
    for word in t {
        let w = word.to_ascii_lowercase();
        let flag = match w.as_str() {
            "not" => {
                negate = true;
                continue;
            }
            "none" => {
                style.bold = Some(false);
                style.dim = Some(false);
                style.italic = Some(false);
                style.underline = Some(false);
                style.strike = Some(false);
                style.reverse = Some(false);
                style.blink = Some(false);
                negate = false;
                continue;
            }
            "b" | "bold" => &mut style.bold,
            "d" | "dim" => &mut style.dim,
            "i" | "italic" => &mut style.italic,
            "u" | "underline" => &mut style.underline,
            "s" | "strike" => &mut style.strike,
            "r" | "reverse" => &mut style.reverse,
            "blink" => &mut style.blink,
            // Valid in Textual but not modeled by our Style: accept and ignore.
            "o" | "overline" | "uu" => {
                negate = false;
                continue;
            }
            _ => return Err(format!("invalid text style {word:?}")),
        };
        *flag = Some(!negate);
        negate = false;
    }
    Ok(style)
}

/// Child text style with unset attributes filled from the parent's.
pub(crate) fn inherit_text_style(
    child: Option<TextStyle>,
    parent: &Option<TextStyle>,
) -> Option<TextStyle> {
    match (child, parent) {
        (None, p) => p.clone(),
        (Some(c), None) => Some(c),
        (Some(mut c), Some(p)) => {
            c.bold = c.bold.or(p.bold);
            c.dim = c.dim.or(p.dim);
            c.italic = c.italic.or(p.italic);
            c.underline = c.underline.or(p.underline);
            c.strike = c.strike.or(p.strike);
            c.reverse = c.reverse.or(p.reverse);
            c.blink = c.blink.or(p.blink);
            Some(c)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(css: &str) -> Styles {
        Styles::parse_inline(css).unwrap_or_else(|e| panic!("{css}: {e}"))
    }

    fn err(css: &str) -> String {
        Styles::parse_inline(css).expect_err(css)
    }

    fn c(text: &str) -> Color {
        Color::parse(text).unwrap()
    }

    #[test]
    fn enums() {
        let st = s("display: none; visibility: hidden; layout: grid; position: absolute; box-sizing: content-box");
        assert_eq!(st.display, Some(Display::None));
        assert_eq!(st.visibility, Some(Visibility::Hidden));
        assert_eq!(st.layout, Some(LayoutKind::Grid));
        assert_eq!(st.position, Some(Position::Absolute));
        assert_eq!(st.box_sizing, Some(BoxSizing::ContentBox));
        assert_eq!(s("layout: horizontal").layout, Some(LayoutKind::Horizontal));
        assert_eq!(s("display: block").display, Some(Display::Block));
        assert!(err("display: flex").contains("display"));
        assert!(Styles::parse_inline("layout: vertical horizontal").is_err());
    }

    #[test]
    fn scalars() {
        let st = s("width: 50%; height: 1fr; min-width: 10; max-height: 50vh; min-height: auto; max-width: 20w");
        assert_eq!(st.width, Some(Scalar::percent(50.0)));
        assert_eq!(st.height, Some(Scalar::fr(1.0)));
        assert_eq!(st.min_width, Some(Scalar::cells(10.0)));
        assert_eq!(st.max_height.unwrap().unit, Unit::ViewHeight);
        assert_eq!(st.min_height, Some(Scalar::AUTO));
        assert_eq!(st.max_width.unwrap().unit, Unit::Width);
        assert!(Styles::parse_inline("width: big").is_err());
        assert!(Styles::parse_inline("width: 1 2").is_err());
    }

    #[test]
    fn spacing_props() {
        assert_eq!(s("margin: 1").margin, Some(Spacing::all(1)));
        assert_eq!(s("margin: 1 2").margin, Some(Spacing::new(1, 2, 1, 2)));
        assert_eq!(
            s("padding: 1 2 3 4").padding,
            Some(Spacing::new(1, 2, 3, 4))
        );
        assert!(Styles::parse_inline("padding: 1 2 3").is_err());
        assert_eq!(
            s("padding: 1; padding-left: 5; padding-top: 0").padding,
            Some(Spacing::new(0, 1, 1, 5))
        );
        assert_eq!(s("margin-bottom: 2").margin, Some(Spacing::new(0, 0, 2, 0)));
        assert_eq!(
            s("margin-right: -1").margin,
            Some(Spacing::new(0, -1, 0, 0))
        );
    }

    #[test]
    fn borders() {
        let st = s("border: heavy red");
        for e in [
            st.border_top,
            st.border_right,
            st.border_bottom,
            st.border_left,
        ] {
            assert_eq!(
                e,
                Some(BorderEdge {
                    kind: BorderKind::Heavy,
                    color: c("red")
                })
            );
        }
        let st = s("border-top: tall #ff0000 50%; border-left: none");
        assert_eq!(
            st.border_top,
            Some(BorderEdge {
                kind: BorderKind::Tall,
                color: Color::rgba(255, 0, 0, 0.5)
            })
        );
        assert_eq!(st.border_left.unwrap().kind, BorderKind::None);
        assert_eq!(st.border_right, None);
        // Kind only → default (white) color; color only → solid.
        assert_eq!(
            s("border: round").border_bottom.unwrap().color,
            Color::WHITE
        );
        assert_eq!(
            s("border: blue").border_bottom.unwrap().kind,
            BorderKind::Solid
        );
        assert_eq!(
            s("border-right: blank").border_right.unwrap().kind,
            BorderKind::Blank
        );
        assert!(Styles::parse_inline("border: wiggly").is_err());
        let all_kinds = [
            "none", "hidden", "ascii", "blank", "dashed", "double", "heavy", "hkey", "inner",
            "outer", "panel", "round", "solid", "tall", "tab", "thick", "block", "vkey", "wide",
        ];
        for k in all_kinds {
            assert!(parse_border_kind(k).is_some(), "{k}");
        }
        let mut st = Styles::new();
        st.set_declaration_with("border", "wide", c("green"))
            .unwrap();
        assert_eq!(st.border_top.unwrap().color, c("green"));
    }

    #[test]
    fn outlines() {
        let st = s("outline: solid green; outline-left: dashed blue");
        assert_eq!(st.outline_top.unwrap().kind, BorderKind::Solid);
        assert_eq!(
            st.outline_left,
            Some(BorderEdge {
                kind: BorderKind::Dashed,
                color: c("blue")
            })
        );
        assert_eq!(
            s("outline-bottom: wide").outline_bottom.unwrap().kind,
            BorderKind::Wide
        );
        assert_eq!(
            s("outline-top: wide").outline_top.unwrap().kind,
            BorderKind::Wide
        );
        assert_eq!(
            s("outline-right: wide").outline_right.unwrap().kind,
            BorderKind::Wide
        );
    }

    #[test]
    fn border_titles() {
        let st = s("border-title-align: center; border-subtitle-align: right; \
                    border-title-color: red; border-title-background: blue 50%; \
                    border-title-style: bold italic; border-subtitle-color: green; \
                    border-subtitle-background: black; border-subtitle-style: not bold");
        assert_eq!(st.border_title_align, Some(AlignH::Center));
        assert_eq!(st.border_subtitle_align, Some(AlignH::Right));
        assert_eq!(st.border_title_color, Some(c("red")));
        assert_eq!(
            st.border_title_background,
            Some(c("blue").multiply_alpha(0.5))
        );
        assert_eq!(st.border_title_style, Some(Style::new().bold().italic()));
        assert_eq!(st.border_subtitle_color, Some(c("green")));
        assert_eq!(st.border_subtitle_background, Some(c("black")));
        assert_eq!(st.border_subtitle_style.unwrap().bold, Some(false));
    }

    #[test]
    fn colors() {
        assert_eq!(s("color: red").color, Some(ColorValue::Color(c("red"))));
        assert_eq!(s("color: auto").color, Some(ColorValue::Auto(1.0)));
        assert_eq!(s("color: auto 87%").color, Some(ColorValue::Auto(0.87)));
        // `$text 90%` expands to `auto 87% 90%`: last alpha wins.
        assert_eq!(s("color: auto 87% 90%").color, Some(ColorValue::Auto(0.9)));
        assert_eq!(
            s("color: #0178D4 20%").color,
            Some(ColorValue::Color(Color::rgba(1, 120, 212, 0.2)))
        );
        assert_eq!(
            s("color: 50%").color,
            Some(ColorValue::Color(Color::WHITE.with_alpha(0.5)))
        );
        assert_eq!(
            s("background: rgb(10, 20, 30)").background,
            Some(Color::rgb(10, 20, 30))
        );
        assert_eq!(
            s("background: hsl(0, 100%, 50%) 50%").background,
            Some(Color::rgba(255, 0, 0, 0.5))
        );
        assert_eq!(
            s("background: transparent").background,
            Some(Color::TRANSPARENT)
        );
        assert_eq!(
            s("background: ansi_default").background,
            Some(Color::TRANSPARENT)
        );
        assert_eq!(
            s("background: #12345680").background.unwrap().a,
            128.0 / 255.0
        );
        assert!(Styles::parse_inline("background: auto").is_err());
        assert!(Styles::parse_inline("color: notacolor").is_err());
        let st = s("tint: red 10%; background-tint: blue 5%");
        assert_eq!(st.tint, Some(c("red").multiply_alpha(0.1)));
        assert_eq!(st.background_tint, Some(c("blue").multiply_alpha(0.05)));
        assert_eq!(
            parse_color_value("auto 60%").unwrap(),
            ColorValue::Auto(0.6)
        );
    }

    #[test]
    fn opacity() {
        assert_eq!(s("opacity: 50%").opacity, Some(0.5));
        assert_eq!(s("opacity: 0.25").opacity, Some(0.25));
        assert_eq!(s("opacity: 2").opacity, Some(1.0));
        assert_eq!(s("text-opacity: 60%").text_opacity, Some(0.6));
        assert!(Styles::parse_inline("opacity: half").is_err());
    }

    #[test]
    fn text_styles() {
        let st = s("text-style: bold italic underline");
        let ts = st.text_style.unwrap();
        assert_eq!(
            (ts.bold, ts.italic, ts.underline),
            (Some(true), Some(true), Some(true))
        );
        assert_eq!(ts.dim, None);
        assert_eq!(ts.fg, None);
        let ts = parse_text_style("none").unwrap();
        assert_eq!(
            [
                ts.bold,
                ts.dim,
                ts.italic,
                ts.underline,
                ts.strike,
                ts.reverse,
                ts.blink
            ],
            [Some(false); 7]
        );
        let ts = parse_text_style("bold not underline").unwrap();
        assert_eq!((ts.bold, ts.underline), (Some(true), Some(false)));
        let ts = parse_text_style("b reverse strike dim blink").unwrap();
        assert_eq!(
            (ts.bold, ts.reverse, ts.strike, ts.dim, ts.blink),
            (Some(true), Some(true), Some(true), Some(true), Some(true))
        );
        assert!(parse_text_style("overline uu").unwrap().is_empty());
        assert!(parse_text_style("sparkly").is_err());
        let st = s("link-style: underline; link-style-hover: bold not underline");
        assert_eq!(st.link_style.unwrap().underline, Some(true));
        assert_eq!(st.link_style_hover.unwrap().underline, Some(false));
    }

    #[test]
    fn text_props() {
        let st = s("text-align: center; text-wrap: nowrap; text-overflow: ellipsis");
        assert_eq!(st.text_align, Some(TextAlign::Center));
        assert_eq!(st.text_wrap, Some(TextWrap::NoWrap));
        assert_eq!(st.text_overflow, Some(TextOverflowKind::Ellipsis));
        assert_eq!(
            s("text-align: justify").text_align,
            Some(TextAlign::Justify)
        );
        assert_eq!(
            s("text-overflow: clip").text_overflow,
            Some(TextOverflowKind::Clip)
        );
    }

    #[test]
    fn alignment() {
        let st = s("align: center middle; content-align: right bottom");
        assert_eq!(st.get_align(), (AlignH::Center, AlignV::Middle));
        assert_eq!(st.get_content_align(), (AlignH::Right, AlignV::Bottom));
        let st = s("align-horizontal: right; align-vertical: bottom; \
                    content-align-horizontal: center; content-align-vertical: middle");
        assert_eq!(st.get_align(), (AlignH::Right, AlignV::Bottom));
        assert_eq!(st.get_content_align(), (AlignH::Center, AlignV::Middle));
        assert!(Styles::parse_inline("align: middle center").is_err());
        assert!(Styles::parse_inline("align: center").is_err());
    }

    #[test]
    fn dock_offset_layers() {
        assert_eq!(s("dock: bottom").dock, Some(Dock::Bottom));
        assert_eq!(s("dock: none").dock, None);
        let st = s("offset: 1 -2");
        assert_eq!(st.offset_x, Some(Scalar::cells(1.0)));
        assert_eq!(st.offset_y, Some(Scalar::cells(-2.0)));
        let st = s("offset: 50% 25%");
        assert_eq!(st.offset_x.unwrap().unit, Unit::Width);
        assert_eq!(st.offset_y.unwrap().unit, Unit::Height);
        assert_eq!(s("offset-x: 3").offset_x, Some(Scalar::cells(3.0)));
        assert_eq!(s("offset-y: 4").offset_y, Some(Scalar::cells(4.0)));
        assert!(Styles::parse_inline("offset: 1").is_err());
        assert_eq!(s("layer: above").layer.as_deref(), Some("above"));
        assert_eq!(
            s("layers: below above").layers,
            Some(vec!["below".to_string(), "above".to_string()])
        );
    }

    #[test]
    fn overflow_and_scrollbars() {
        let st = s("overflow: auto hidden");
        assert_eq!(
            (st.overflow_x, st.overflow_y),
            (Some(Overflow::Auto), Some(Overflow::Hidden))
        );
        let st = s("overflow: scroll");
        assert_eq!(
            (st.overflow_x, st.overflow_y),
            (Some(Overflow::Scroll), Some(Overflow::Scroll))
        );
        assert_eq!(s("overflow-y: auto").overflow_y, Some(Overflow::Auto));
        assert_eq!(s("overflow-x: scroll").overflow_x, Some(Overflow::Scroll));
        assert_eq!(
            s("scrollbar-gutter: stable").scrollbar_gutter,
            Some(ScrollbarGutter::Stable)
        );
        // Textual order: horizontal then vertical.
        let st = s("scrollbar-size: 1 3");
        assert_eq!(st.scrollbar_size_horizontal, Some(1));
        assert_eq!(st.scrollbar_size_vertical, Some(3));
        assert_eq!(
            s("scrollbar-size-vertical: 0").scrollbar_size_vertical,
            Some(0)
        );
        assert_eq!(
            s("scrollbar-size-horizontal: 2").scrollbar_size_horizontal,
            Some(2)
        );
        assert!(Styles::parse_inline("scrollbar-size: 1").is_err());
        let st = s(
            "scrollbar-color: red; scrollbar-color-hover: green; scrollbar-color-active: blue; \
                    scrollbar-background: black; scrollbar-background-hover: white; \
                    scrollbar-background-active: yellow; scrollbar-corner-color: gray",
        );
        assert_eq!(st.scrollbar_color, Some(c("red")));
        assert_eq!(st.scrollbar_color_hover, Some(c("green")));
        assert_eq!(st.scrollbar_color_active, Some(c("blue")));
        assert_eq!(st.scrollbar_background, Some(c("black")));
        assert_eq!(st.scrollbar_background_hover, Some(c("white")));
        assert_eq!(st.scrollbar_background_active, Some(c("yellow")));
        assert_eq!(st.scrollbar_corner_color, Some(c("gray")));
    }

    #[test]
    fn grid() {
        let st = s("grid-size: 3");
        assert_eq!(st.get_grid_size(), (3, 0));
        assert_eq!(s("grid-size: 3 2").get_grid_size(), (3, 2));
        let st = s("grid-columns: 1fr 2fr 10 50% auto");
        assert_eq!(
            st.grid_columns,
            Some(vec![
                Scalar::fr(1.0),
                Scalar::fr(2.0),
                Scalar::cells(10.0),
                Scalar {
                    value: 50.0,
                    unit: Unit::Width
                },
                Scalar::AUTO
            ])
        );
        assert_eq!(s("grid-rows: 25%").grid_rows.unwrap()[0].unit, Unit::Height);
        let st = s("grid-gutter: 1");
        assert_eq!(st.get_grid_gutter(), (1, 1));
        // Textual: first value → horizontal, second → vertical.
        let st = s("grid-gutter: 1 2");
        assert_eq!(st.grid_gutter_horizontal, Some(1));
        assert_eq!(st.grid_gutter_vertical, Some(2));
        let st = s("grid-gutter-horizontal: 4; grid-gutter-vertical: 5; grid-size-columns: 2; grid-size-rows: 3");
        assert_eq!(st.get_grid_gutter(), (4, 5));
        assert_eq!(st.get_grid_size(), (2, 3));
        let st = s("column-span: 2; row-span: 3");
        assert_eq!((st.column_span, st.row_span), (Some(2), Some(3)));
        assert!(Styles::parse_inline("column-span: 0").is_err());
        assert!(Styles::parse_inline("grid-gutter: 1 2 3").is_err());
    }

    #[test]
    fn links() {
        let st = s("link-color: red; link-background: blue; link-color-hover: green; link-background-hover: black");
        assert_eq!(st.link_color, Some(c("red")));
        assert_eq!(st.link_background, Some(c("blue")));
        assert_eq!(st.link_color_hover, Some(c("green")));
        assert_eq!(st.link_background_hover, Some(c("black")));
        // `$link-color` is `auto 87%` in Textual themes: maps to transparent (defer).
        assert_eq!(
            s("link-color: auto 87%").link_color,
            Some(Color::TRANSPARENT)
        );
        // `initial` leaves the property unset.
        assert_eq!(s("link-background: initial").link_background, None);
    }

    #[test]
    fn unknown_and_ignored_properties() {
        assert!(err("bogus: 1").contains("unknown property"));
        assert!(err("bogus: initial").contains("unknown property"));
        for p in IGNORED_PROPERTIES {
            assert!(
                Styles::parse_inline(&format!("{p}: whatever 1 2")).is_ok(),
                "{p}"
            );
        }
        assert!(is_known_property("width"));
        assert!(is_known_property("hatch"));
        assert!(is_known_property("border-title-style"));
        assert!(!is_known_property("bogus"));
        assert!(Styles::parse_inline("width:").is_err());
    }

    #[test]
    fn inline_parsing() {
        let st = s("width: 10; color: red !important;");
        assert_eq!(st.width, Some(Scalar::cells(10.0)));
        assert_eq!(st.color, Some(ColorValue::Color(c("red"))));
        assert_eq!(s(""), Styles::default());
        assert_eq!(
            s("background: rgb(1,2,3); width: 1").background,
            Some(Color::rgb(1, 2, 3))
        );
        assert!(Styles::parse_inline("width 10").is_err());
    }

    #[test]
    fn merge_overlays_set_fields() {
        let mut a = s("width: 1; height: 2; color: red");
        let b = s("width: 3; background: blue");
        a.merge(&b);
        assert_eq!(a.width, Some(Scalar::cells(3.0)));
        assert_eq!(a.height, Some(Scalar::cells(2.0)));
        assert_eq!(a.background, Some(c("blue")));
        assert_eq!(a.color, Some(ColorValue::Color(c("red"))));
        let before = a.clone();
        a.merge(&Styles::default());
        assert_eq!(a, before);
    }

    #[test]
    fn inheritance() {
        let parent = s(
            "color: red; text-style: bold; text-align: center; background: blue; \
                        width: 5; link-color: green; link-style: underline; text-wrap: nowrap; \
                        text-overflow: ellipsis; text-opacity: 50%",
        );
        let mut child = s("text-style: italic; link-style: not underline");
        child.inherit_from(&parent);
        assert_eq!(child.color, Some(ColorValue::Color(c("red"))));
        assert_eq!(child.text_align, Some(TextAlign::Center));
        assert_eq!(child.text_wrap, Some(TextWrap::NoWrap));
        assert_eq!(child.text_overflow, Some(TextOverflowKind::Ellipsis));
        assert_eq!(child.link_color, Some(c("green")));
        // Not inherited.
        assert_eq!(child.background, None);
        assert_eq!(child.width, None);
        assert_eq!(child.text_opacity, None);
        // Text style attributes combine.
        let ts = child.text_style.unwrap();
        assert_eq!((ts.bold, ts.italic), (Some(true), Some(true)));
        assert_eq!(child.link_style.unwrap().underline, Some(false));
        // Child values win.
        let mut child = s("color: blue; text-style: not bold");
        child.inherit_from(&parent);
        assert_eq!(child.color, Some(ColorValue::Color(c("blue"))));
        assert_eq!(child.text_style.unwrap().bold, Some(false));
    }

    #[test]
    fn split_values_keeps_functions() {
        assert_eq!(
            split_values(" rgb(1, 2, 3)  20% \"a b\" "),
            vec!["rgb(1, 2, 3)", "20%", "\"a b\""]
        );
        assert_eq!(strip_important("red !important"), ("red", true));
        assert_eq!(strip_important(" red "), ("red", false));
    }
}
