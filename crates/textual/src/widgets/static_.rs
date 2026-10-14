//! `Static` and `Label`: widgets that display (markup) content.

use crate::widget::{Content, RenderCtx, Widget};
use textual_render::Text;

/// What a `Static` displays.
#[derive(Debug, Clone, PartialEq)]
pub enum StaticContent {
    /// Textual markup (with `$variable` support).
    Markup(String),
    /// Plain text, rendered literally.
    Plain(String),
    /// Pre-styled text.
    Text(Text),
}

/// Displays content; call [`Static::update`] to change it. Content is markup by default.
#[derive(Debug, Clone)]
pub struct Static {
    content: StaticContent,
}

impl Static {
    pub fn new(markup: impl Into<String>) -> Self {
        Static { content: StaticContent::Markup(markup.into()) }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Static { content: StaticContent::Plain(text.into()) }
    }

    pub fn text(text: Text) -> Self {
        Static { content: StaticContent::Text(text) }
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

    pub fn content(&self) -> &StaticContent {
        &self.content
    }

    /// The displayed text without styles.
    pub fn plain_text(&self) -> String {
        match &self.content {
            StaticContent::Markup(m) => Text::from_markup(m).plain_text(),
            StaticContent::Plain(p) => p.clone(),
            StaticContent::Text(t) => t.plain_text(),
        }
    }

    pub(crate) fn render_content(&self, ctx: &RenderCtx) -> Content {
        match &self.content {
            StaticContent::Markup(m) => Content::Text(ctx.markup(m)),
            StaticContent::Plain(p) => Content::Text(Text::plain(p.clone())),
            StaticContent::Text(t) => Content::Text(t.clone()),
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
}

/// A `Static` that sizes to its content (`width: auto; height: auto`).
/// Variants (classes) `success`, `error`, `warning`, `primary`, `secondary`, `accent` add a colored style.
#[derive(Debug, Clone)]
pub struct Label {
    inner: Static,
}

impl Label {
    pub fn new(markup: impl Into<String>) -> Self {
        Label { inner: Static::new(markup) }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Label { inner: Static::plain(text) }
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
}
