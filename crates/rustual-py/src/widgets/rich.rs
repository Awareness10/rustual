//! Rich renderables (`rich.table.Table`, `Syntax`, `Panel`, ...) rendered through Rich into
//! native strips, cached per width.

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyList;
use rustual::render::{Color, Span, Strip, Style};
use rustual::{Content, RenderCtx, Widget};
use std::cell::RefCell;

static RENDER_LINES: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

fn opt(b: bool) -> Option<bool> {
    b.then_some(true)
}

/// Render a Rich renderable at `width` cells.
pub(crate) fn rich_lines(
    py: Python<'_>,
    renderable: &Bound<'_, PyAny>,
    width: i32,
) -> PyResult<Vec<Strip>> {
    let f = RENDER_LINES.get_or_try_init(py, || -> PyResult<Py<PyAny>> {
        Ok(py
            .import("rustual._rich")?
            .getattr("render_lines")?
            .unbind())
    })?;
    // Rich code may call back into user code: no DOM access from there.
    let _guard = crate::bridge::push(crate::bridge::blocked_frame());
    let lines = f.bind(py).call1((renderable, width.max(1)))?;
    let mut out = Vec::new();
    for line in lines.cast::<PyList>()?.iter() {
        let mut spans = Vec::new();
        for seg in line.cast::<PyList>()?.iter() {
            #[allow(clippy::type_complexity)]
            let (text, style): (
                String,
                Option<(
                    Option<(u8, u8, u8)>,
                    Option<(u8, u8, u8)>,
                    bool,
                    bool,
                    bool,
                    bool,
                    bool,
                    bool,
                    Option<String>,
                )>,
            ) = seg.extract()?;
            let style = match style {
                None => Style::default(),
                Some((fg, bg, bold, dim, italic, underline, strike, reverse, link)) => Style {
                    fg: fg.map(|(r, g, b)| Color::rgb(r, g, b)),
                    bg: bg.map(|(r, g, b)| Color::rgb(r, g, b)),
                    bold: opt(bold),
                    dim: opt(dim),
                    italic: opt(italic),
                    underline: opt(underline),
                    strike: opt(strike),
                    reverse: opt(reverse),
                    blink: None,
                    link: link.map(Into::into),
                },
            };
            spans.push(Span::new(text, style));
        }
        out.push(Strip::new(spans));
    }
    Ok(out)
}

/// Hosts a text widget (`Static`, `Label`) and optionally a Rich renderable shown instead of
/// its text. Everything else delegates to the inner widget.
pub(crate) struct RichHost {
    pub inner: Box<dyn Widget>,
    rich: Option<Py<PyAny>>,
    cache: RefCell<Option<(i32, Vec<Strip>)>>,
}

impl RichHost {
    pub fn new(inner: Box<dyn Widget>, rich: Option<Py<PyAny>>) -> Self {
        RichHost {
            inner,
            rich,
            cache: RefCell::new(None),
        }
    }

    pub fn set_rich(&mut self, rich: Option<Py<PyAny>>) {
        self.rich = rich;
        self.cache.replace(None);
    }

    fn lines(&self, width: i32) -> Option<Vec<Strip>> {
        let rich = self.rich.as_ref()?;
        if let Some((w, lines)) = &*self.cache.borrow() {
            if *w == width {
                return Some(lines.clone());
            }
        }
        let lines = crate::bridge::call_py(|py| rich_lines(py, rich.bind(py), width))?;
        self.cache.replace(Some((width, lines.clone())));
        Some(lines)
    }
}

impl Widget for RichHost {
    fn type_names(&self) -> &'static [&'static str] {
        self.inner.type_names()
    }
    fn default_css(&self) -> &'static str {
        self.inner.default_css()
    }
    fn on_attach(&mut self, id: rustual::NodeId) {
        self.inner.on_attach(id)
    }
    fn compose(&mut self) -> Vec<rustual::Child> {
        self.inner.compose()
    }
    fn on_mount(&mut self, ctx: &mut rustual::Ctx) {
        self.inner.on_mount(ctx)
    }
    fn on_unmount(&mut self, ctx: &mut rustual::Ctx) {
        self.inner.on_unmount(ctx)
    }
    fn on_event(&mut self, event: &rustual::Event, ctx: &mut rustual::Ctx) -> rustual::Flow {
        self.inner.on_event(event, ctx)
    }
    fn on_message(&mut self, m: &rustual::Message, ctx: &mut rustual::Ctx) -> rustual::Flow {
        self.inner.on_message(m, ctx)
    }
    fn render(&self, ctx: &RenderCtx) -> Content {
        match self.lines(ctx.size.width) {
            Some(lines) => Content::Lines(lines),
            None => self.inner.render(ctx),
        }
    }
    fn content_width(&self, ctx: &RenderCtx) -> Option<i32> {
        match self.rich {
            Some(_) => self
                .lines(ctx.size.width)
                .map(|l| l.iter().map(Strip::cell_len).max().unwrap_or(0)),
            None => self.inner.content_width(ctx),
        }
    }
    fn content_height(&self, ctx: &RenderCtx, width: i32) -> Option<i32> {
        match self.rich {
            Some(_) => self.lines(width).map(|l| l.len() as i32),
            None => self.inner.content_height(ctx, width),
        }
    }
    fn can_focus(&self) -> bool {
        self.inner.can_focus()
    }
    fn bindings(&self) -> Vec<rustual::Binding> {
        self.inner.bindings()
    }
    fn run_action(&mut self, a: &str, args: &[String], ctx: &mut rustual::Ctx) -> bool {
        self.inner.run_action(a, args, ctx)
    }
    fn delegate(&self) -> Option<&dyn Widget> {
        Some(self.inner.as_ref())
    }
    fn delegate_mut(&mut self) -> Option<&mut dyn Widget> {
        Some(self.inner.as_mut())
    }
}

/// `Content` for a Rich renderable returned from a Python `render()`.
pub(crate) fn rich_content(
    py: Python<'_>,
    ctx: &RenderCtx,
    r: &Bound<'_, PyAny>,
) -> PyResult<Content> {
    Ok(Content::Lines(rich_lines(py, r, ctx.size.width)?))
}
