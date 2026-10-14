//! The [`Widget`] trait, render output ([`Content`]) and composition helpers ([`Child`]).

use crate::binding::Binding;
use crate::context::Ctx;
use crate::dom::{Dom, NodeId};
use crate::events::Event;
use crate::message::{Flow, Message};
use std::any::Any;
use textual_css::{
    AlignH, ColorValue, SelectorNode, Styles, Stylesheet, TextAlign, TextOverflowKind, TextWrap,
};
use textual_render::{Color, Justify, Offset, Size, Strip, Style, Text, TextOverflow};

/// What a widget renders inside its content region.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Content {
    #[default]
    Empty,
    /// Styled text: wrapped, justified (`text-align`) and aligned (`content-align`)
    /// by the compositor according to the widget's styles.
    Text(Text),
    /// Pre-rendered lines, drawn verbatim from the top-left of the content region
    /// (cropped / padded to fit). Used by widgets that lay out their own content.
    Lines(Vec<Strip>),
}

impl Content {
    /// Content from Textual markup (no variable lookup; use [`RenderCtx::markup`] for `$vars`).
    pub fn markup(s: &str) -> Content {
        Content::Text(Text::from_markup(s))
    }

    pub fn plain(s: &str) -> Content {
        Content::Text(Text::plain(s))
    }
}

impl From<Text> for Content {
    fn from(t: Text) -> Self {
        Content::Text(t)
    }
}

impl From<&str> for Content {
    fn from(s: &str) -> Self {
        Content::markup(s)
    }
}

impl From<String> for Content {
    fn from(s: String) -> Self {
        Content::markup(&s)
    }
}

impl From<Vec<Strip>> for Content {
    fn from(lines: Vec<Strip>) -> Self {
        Content::Lines(lines)
    }
}

/// A UI component. Every method has a default, so a widget only implements what it needs.
///
/// Handlers receive a [`Ctx`] for interacting with the app (posting messages, querying and
/// mutating other widgets, focus, timers, workers, screens). While a handler runs, the widget
/// is checked out of the DOM, so `ctx` can freely access every *other* widget.
pub trait Widget: Any {
    /// CSS type names, most specific first (e.g. `&["Button"]`). `"Widget"` is appended automatically.
    fn type_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Widget-level CSS (Textual's `DEFAULT_CSS`), registered once per first type name.
    /// Loses to app CSS on equal importance.
    fn default_css(&self) -> &'static str {
        ""
    }

    /// Child widgets created when this widget is mounted.
    fn compose(&mut self) -> Vec<Child> {
        Vec::new()
    }

    fn on_mount(&mut self, _ctx: &mut Ctx) {}

    fn on_unmount(&mut self, _ctx: &mut Ctx) {}

    fn on_event(&mut self, _event: &Event, _ctx: &mut Ctx) -> Flow {
        Flow::Continue
    }

    fn on_message(&mut self, _message: &Message, _ctx: &mut Ctx) -> Flow {
        Flow::Continue
    }

    /// Render content for the content region (`ctx.size`).
    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::Empty
    }

    /// Width for `width: auto`. Default: measured from [`Widget::render`] output.
    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        None
    }

    /// Height for `height: auto` at the given content width. Default: measured from render output.
    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        None
    }

    /// For self-scrolling widgets (like Textual's `ScrollView`): the full size of the
    /// content. When `Some`, layout shows scrollbars per `overflow` and `render` receives
    /// `ctx.scroll_offset` and must render only the visible window.
    fn virtual_size(&self, _ctx: &RenderCtx) -> Option<Size> {
        None
    }

    fn can_focus(&self) -> bool {
        false
    }

    /// Whether descendants may receive focus.
    fn can_focus_children(&self) -> bool {
        true
    }

    fn bindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    /// Run a named action (from a binding or `ctx.run_action`). Return `true` if handled.
    fn run_action(&mut self, _action: &str, _args: &[String], _ctx: &mut Ctx) -> bool {
        false
    }

    /// Commands offered in the command palette when this widget is the app root or the
    /// active screen (like Textual's `App.get_system_commands` / `Screen.COMMANDS`).
    fn commands(&self) -> Vec<crate::command::Command> {
        Vec::new()
    }
}

/// A widget plus the node attributes to mount it with (id, classes, inline styles, children).
pub struct Child {
    pub widget: Box<dyn Widget>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub styles: Option<String>,
    pub disabled: bool,
    pub border_title: Option<String>,
    pub border_subtitle: Option<String>,
    pub children: Vec<Child>,
    /// Tooltip text (markup), see [`Child::tooltip`].
    pub tooltip: Option<String>,
}

impl Child {
    pub fn new(widget: Box<dyn Widget>) -> Child {
        Child {
            widget,
            id: None,
            classes: Vec::new(),
            styles: None,
            disabled: false,
            border_title: None,
            border_subtitle: None,
            children: Vec::new(),
            tooltip: None,
        }
    }

    /// A tooltip (markup) shown when the mouse rests over the widget (Textual's `tooltip`).
    pub fn tooltip(mut self, text: &str) -> Child {
        self.tooltip = Some(text.to_string());
        self
    }

    pub fn id(mut self, id: &str) -> Child {
        self.id = Some(id.to_string());
        self
    }

    /// Space-separated classes.
    pub fn classes(mut self, classes: &str) -> Child {
        self.classes
            .extend(classes.split_whitespace().map(String::from));
        self
    }

    /// Inline CSS declarations, e.g. `"width: 20; background: red;"`.
    pub fn style(mut self, css: &str) -> Child {
        self.styles = Some(match self.styles {
            Some(s) => format!("{s};{css}"),
            None => css.to_string(),
        });
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Child {
        self.disabled = disabled;
        self
    }

    pub fn border_title(mut self, t: &str) -> Child {
        self.border_title = Some(t.to_string());
        self
    }

    pub fn border_subtitle(mut self, t: &str) -> Child {
        self.border_subtitle = Some(t.to_string());
        self
    }

    /// Add children (like Textual's `with Container(): yield ...`).
    pub fn with(mut self, children: impl IntoIterator<Item = Child>) -> Child {
        self.children.extend(children);
        self
    }
}

impl<W: Widget> From<W> for Child {
    fn from(w: W) -> Self {
        Child::new(Box::new(w))
    }
}

/// Builder methods available on every widget value: `Button::new("OK").id("ok")`.
pub trait WidgetExt: Widget + Sized {
    fn child(self) -> Child {
        Child::new(Box::new(self))
    }
    fn id(self, id: &str) -> Child {
        self.child().id(id)
    }
    fn classes(self, classes: &str) -> Child {
        self.child().classes(classes)
    }
    fn style(self, css: &str) -> Child {
        self.child().style(css)
    }
    fn disabled(self, disabled: bool) -> Child {
        self.child().disabled(disabled)
    }
    fn border_title(self, t: &str) -> Child {
        self.child().border_title(t)
    }
    fn with(self, children: impl IntoIterator<Item = Child>) -> Child {
        self.child().with(children)
    }
    fn tooltip(self, text: &str) -> Child {
        self.child().tooltip(text)
    }
}

impl<W: Widget + Sized> WidgetExt for W {}

/// Build a `Vec<Child>` from widgets and/or `Child` values: `compose![Header::new(), Button::new("x").id("b")]`.
#[macro_export]
macro_rules! compose {
    ($($child:expr),* $(,)?) => {
        vec![$($crate::Child::from($child)),*]
    };
}

/// App-wide information available while rendering.
#[derive(Default)]
pub struct RenderEnv {
    pub stylesheet: Stylesheet,
    pub title: String,
    pub sub_title: String,
    /// Bindings active for the focused widget chain (focused → screen → app), deduplicated by key,
    /// in display order. Used by `Footer`.
    pub active_bindings: Vec<Binding>,
    pub dark: bool,
    /// Active notifications (oldest first), for toast widgets.
    pub notifications: Vec<crate::app::Notification>,
    /// The hovered / grabbed scrollbar, painted with the `-hover` / `-active` colors.
    pub scrollbar_highlight: Option<crate::scrollbar::ScrollbarHighlight>,
    /// Every binding of the focus chain (including hidden ones), grouped per node, app first;
    /// a key bound by an inner node hides outer bindings of the same key. Used by `KeyPanel`.
    pub all_bindings: Vec<Vec<Binding>>,
}

/// Read-only context for [`Widget::render`] and measurement.
pub struct RenderCtx<'a> {
    pub dom: &'a Dom,
    pub env: &'a RenderEnv,
    pub node: NodeId,
    /// Content region size (inside padding/border/scrollbars). During measurement, the available size.
    pub size: Size,
    /// Scroll position (for widgets with `virtual_size`).
    pub scroll_offset: Offset,
}

impl<'a> RenderCtx<'a> {
    pub fn new(dom: &'a Dom, env: &'a RenderEnv, node: NodeId, size: Size) -> Self {
        let scroll_offset = dom.get(node).map(|n| n.scroll_offset).unwrap_or_default();
        RenderCtx {
            dom,
            env,
            node,
            size,
            scroll_offset,
        }
    }

    /// Computed styles of this widget.
    pub fn styles(&self) -> &'a Styles {
        static EMPTY: std::sync::OnceLock<Styles> = std::sync::OnceLock::new();
        self.dom
            .get(self.node)
            .map(|n| &n.styles)
            .unwrap_or_else(|| EMPTY.get_or_init(Styles::default))
    }

    pub fn is_focused(&self) -> bool {
        self.dom.focused == Some(self.node)
    }

    pub fn is_hovered(&self) -> bool {
        self.dom.hovered == Some(self.node)
    }

    pub fn is_disabled(&self) -> bool {
        self.dom.is_disabled(self.node)
    }

    pub fn has_class(&self, c: &str) -> bool {
        self.dom
            .get(self.node)
            .map(|n| n.has_class(c))
            .unwrap_or(false)
    }

    pub fn has_pseudo_class(&self, p: &str) -> bool {
        self.dom.node_ref(self.node).has_pseudo_class(p)
    }

    /// Theme variable as a color (`"primary"`, `"accent"`, ... without `$`).
    pub fn var_color(&self, name: &str) -> Option<Color> {
        let v = self.env.stylesheet.variables().get(name)?;
        parse_color_value(v)
    }

    /// Parse markup with `$variable` colors resolved from the theme.
    pub fn markup(&self, s: &str) -> Text {
        Text::from_markup_with(s, &|name| self.var_color(name))
    }

    /// Effective background behind this widget (its own background composited over ancestors').
    pub fn background(&self) -> Color {
        effective_background(self.dom, self.node)
    }

    /// The widget's base text style: color (resolving `auto`), background and `text-style`.
    pub fn base_style(&self) -> Style {
        style_from_styles(self.styles(), self.background())
    }

    /// Style for a component class (Textual's `get_component_rich_style`), matched as a
    /// virtual child of this widget with the given class, e.g. `"datatable--header"`.
    /// Only `color`, `background` and `text-style` are used. Colors are composited over this widget's background.
    pub fn component_style(&self, class: &str) -> Style {
        let virt = EitherNode::Component(ComponentNode {
            parent: self.dom.node_ref(self.node),
            class,
        });
        let mut styles = self.env.stylesheet.compute(virt);
        styles.inherit_from(self.styles());
        style_from_styles(&styles, self.background())
    }

    /// Text justification from `text-align`.
    pub fn justify(&self) -> Justify {
        text_align_to_justify(self.styles().get_text_align())
    }

    /// Overflow handling and no-wrap flag from `text-overflow` / `text-wrap`.
    pub fn text_overflow(&self) -> (TextOverflow, bool) {
        let s = self.styles();
        let overflow = match s.text_overflow.unwrap_or_default() {
            TextOverflowKind::Fold => TextOverflow::Fold,
            TextOverflowKind::Clip => TextOverflow::Clip,
            TextOverflowKind::Ellipsis => TextOverflow::Ellipsis,
        };
        (
            overflow,
            s.text_wrap.unwrap_or_default() == TextWrap::NoWrap,
        )
    }
}

pub fn text_align_to_justify(a: TextAlign) -> Justify {
    match a {
        TextAlign::Start | TextAlign::Left => Justify::Left,
        TextAlign::End | TextAlign::Right => Justify::Right,
        TextAlign::Center => Justify::Center,
        TextAlign::Justify => Justify::Full,
    }
}

pub fn align_h_to_justify(a: AlignH) -> Justify {
    match a {
        AlignH::Left => Justify::Left,
        AlignH::Center => Justify::Center,
        AlignH::Right => Justify::Right,
    }
}

/// Parse a CSS variable value as a color, supporting the `"<color> <alpha>%"` form.
pub fn parse_color_value(v: &str) -> Option<Color> {
    let v = v.trim();
    if let Ok(c) = Color::parse(v) {
        return Some(c);
    }
    let (color, alpha) = v.rsplit_once(char::is_whitespace)?;
    let alpha: f32 = alpha.strip_suffix('%')?.parse().ok()?;
    Color::parse(color.trim())
        .ok()
        .map(|c| c.multiply_alpha(alpha / 100.0))
}

/// Composite backgrounds from the root down to `node`.
pub fn effective_background(dom: &Dom, node: NodeId) -> Color {
    let mut chain = dom.ancestors_with_self(node);
    chain.reverse();
    let mut bg = Color::BLACK;
    for id in chain {
        if let Some(n) = dom.get(id) {
            bg = bg.composite(n.styles.get_background());
        }
    }
    bg
}

/// Convert computed styles to a span style over the given (opaque) background.
pub fn style_from_styles(styles: &Styles, background: Color) -> Style {
    let mut style = styles.text_style.clone().unwrap_or_default();
    style.fg = None;
    style.bg = None;
    let bg = styles.background.map(|b| background.composite(b));
    let under = bg.unwrap_or(background);
    style.fg = match styles.color {
        Some(ColorValue::Color(c)) => Some(under.composite(c)),
        Some(ColorValue::Auto(alpha)) => Some(under.composite(under.get_contrast_text(alpha))),
        None => None,
    };
    style.bg = bg;
    style
}

/// A virtual node used to match component-class rules (`Parent > .component-class`).
#[derive(Clone, Copy)]
struct ComponentNode<'a> {
    parent: crate::dom::NodeRef<'a>,
    class: &'a str,
}

#[derive(Clone, Copy)]
enum EitherNode<'a> {
    Component(ComponentNode<'a>),
    Real(crate::dom::NodeRef<'a>),
}

impl<'a> textual_css::SelectorNode for EitherNode<'a> {
    fn type_names(&self) -> &[&'static str] {
        match self {
            EitherNode::Component(_) => &[],
            EitherNode::Real(r) => r.type_names(),
        }
    }
    fn css_id(&self) -> Option<&str> {
        match self {
            EitherNode::Component(_) => None,
            EitherNode::Real(r) => r.css_id(),
        }
    }
    fn has_class(&self, class: &str) -> bool {
        match self {
            EitherNode::Component(c) => c.class == class,
            EitherNode::Real(r) => r.has_class(class),
        }
    }
    fn has_pseudo_class(&self, p: &str) -> bool {
        match self {
            EitherNode::Component(_) => false,
            EitherNode::Real(r) => r.has_pseudo_class(p),
        }
    }
    fn parent(&self) -> Option<Self> {
        match self {
            EitherNode::Component(c) => Some(EitherNode::Real(c.parent)),
            EitherNode::Real(r) => r.parent().map(EitherNode::Real),
        }
    }
}
