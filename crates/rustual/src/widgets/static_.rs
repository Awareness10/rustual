//! `Static` and `Label`: widgets that display (markup) content.

use crate::renderables::{measure, render_lines, AnyRenderable, IntoRenderable, RenderOptions};
use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::Text;
use std::sync::Arc;

/// What a `Static` displays.
#[derive(Debug, Clone)]
pub enum StaticContent {
    /// Textual markup (with `$variable` support).
    Markup(String),
    /// Plain text, rendered literally.
    Plain(String),
    /// Pre-styled text.
    Text(Text),
    /// A Rich-style renderable ([`crate::renderables`]), laid out for the content width.
    Renderable(AnyRenderable),
}

impl PartialEq for StaticContent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Markup(a), Self::Markup(b)) | (Self::Plain(a), Self::Plain(b)) => a == b,
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Renderable(a), Self::Renderable(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// Width used for [`Static::plain_text`] of a renderable.
const PLAIN_TEXT_WIDTH: i32 = 80;

/// Displays content; call [`Static::update`] to change it. Content is markup by default.
#[derive(Debug, Clone)]
pub struct Static {
    content: StaticContent,
}

impl Static {
    pub fn new(markup: impl Into<String>) -> Self {
        Static {
            content: StaticContent::Markup(markup.into()),
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Static {
            content: StaticContent::Plain(text.into()),
        }
    }

    pub fn text(text: Text) -> Self {
        Static {
            content: StaticContent::Text(text),
        }
    }

    /// Show a Rich-style renderable (a [`Table`](crate::renderables::Table),
    /// [`Panel`](crate::renderables::Panel), [`Syntax`](crate::renderables::Syntax), ...).
    /// With `width: auto` the widget takes the renderable's measured width.
    pub fn renderable(renderable: impl IntoRenderable) -> Self {
        Static {
            content: StaticContent::Renderable(renderable.into_renderable()),
        }
    }

    pub fn update(&mut self, markup: impl Into<String>) {
        self.content = StaticContent::Markup(markup.into());
    }

    pub fn update_plain(&mut self, text: impl Into<String>) {
        self.content = StaticContent::Plain(text.into());
    }

    pub fn update_text(&mut self, text: Text) {
        self.content = StaticContent::Text(text);
    }

    /// Replace the content with a renderable.
    pub fn update_renderable(&mut self, renderable: impl IntoRenderable) {
        self.content = StaticContent::Renderable(renderable.into_renderable());
    }

    pub fn content(&self) -> &StaticContent {
        &self.content
    }

    /// The displayed text without styles (a renderable is rendered 80 cells wide, with
    /// trailing spaces removed).
    pub fn plain_text(&self) -> String {
        match &self.content {
            StaticContent::Markup(m) => Text::from_markup(m).plain_text(),
            StaticContent::Plain(p) => p.clone(),
            StaticContent::Text(t) => t.plain_text(),
            StaticContent::Renderable(r) => {
                render_lines(r.as_ref(), &RenderOptions::new(PLAIN_TEXT_WIDTH))
                    .iter()
                    .map(|l| l.plain_text().trim_end().to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
    }

    pub(crate) fn render_content(&self, ctx: &RenderCtx) -> Content {
        match &self.content {
            StaticContent::Markup(m) => Content::Text(ctx.markup(m)),
            StaticContent::Plain(p) => Content::Text(Text::plain(p.clone())),
            StaticContent::Text(t) => Content::Text(t.clone()),
            StaticContent::Renderable(r) => Content::Lines(render_lines(
                r.as_ref(),
                &RenderOptions::new(ctx.size.width),
            )),
        }
    }

    /// `width: auto` of a renderable: its measured maximum (Textual measures Rich renderables
    /// the same way).
    pub(crate) fn measured_width(&self, ctx: &RenderCtx) -> Option<i32> {
        match &self.content {
            StaticContent::Renderable(r) => {
                Some(measure(r.as_ref(), &RenderOptions::new(ctx.size.width)).maximum)
            }
            _ => None,
        }
    }
}

impl Widget for Static {
    fn type_names(&self) -> &'static [&'static str] {
        &["Static"]
    }

    fn default_css(&self) -> &'static str {
        "Static { height: auto; }"
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        self.render_content(ctx)
    }

    fn content_width(&self, ctx: &RenderCtx) -> Option<i32> {
        self.measured_width(ctx)
    }
}

/// A `Static` that sizes to its content (`width: auto; height: auto`).
/// Variants (classes) `success`, `error`, `warning`, `primary`, `secondary`, `accent` add a colored style.
#[derive(Debug, Clone)]
pub struct Label {
    inner: Static,
}

impl Label {
    pub fn new(markup: impl Into<String>) -> Self {
        Label {
            inner: Static::new(markup),
        }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Label {
            inner: Static::plain(text),
        }
    }

    pub fn update(&mut self, markup: impl Into<String>) {
        self.inner.update(markup)
    }

    pub fn update_plain(&mut self, text: impl Into<String>) {
        self.inner.update_plain(text)
    }

    pub fn plain_text(&self) -> String {
        self.inner.plain_text()
    }
}

impl Widget for Label {
    fn type_names(&self) -> &'static [&'static str] {
        &["Label", "Static"]
    }

    fn default_css(&self) -> &'static str {
        "
Label {
    width: auto;
    height: auto;
    min-height: 1;
    &.success { color: $text-success; background: $success-muted; }
    &.error { color: $text-error; background: $error-muted; }
    &.warning { color: $text-warning; background: $warning-muted; }
    &.primary { color: $text-primary; background: $primary-muted; }
    &.secondary { color: $text-secondary; background: $secondary-muted; }
    &.accent { color: $text-accent; background: $accent-muted; }
}
"
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        self.inner.render_content(ctx)
    }

    fn content_width(&self, ctx: &RenderCtx) -> Option<i32> {
        self.inner.measured_width(ctx)
    }
}
