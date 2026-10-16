//! `HelpPanel` (port of `textual/widgets/_help_panel.py`): a panel docked on the right of the
//! screen showing context-sensitive help for the focused widget (its [`Widget::help`]
//! Markdown, or that of its nearest ancestor with help) above a [`KeyPanel`] listing the
//! active key bindings.
//!
//! Shown with the `app.show_help_panel` action (or [`Ctx::show_help_panel`], or the "Keys"
//! command of the command palette), hidden with `app.hide_help_panel`.
//!
//! Deviations from Textual:
//! - `split: right` is approximated with `dock: right` (as for [`KeyPanel`]).
//! - Textual watches `screen.focused`; there is no such watcher here. Instead the panel checks
//!   the focused widget's help at every layout pass and, when it changed, posts itself a
//!   message (through an [`AppHandle`]) that updates the help document. The update therefore
//!   lands one event-loop turn after the focus change (in Pilot tests: one more
//!   `pilot.pause()`).

use crate::app::AppHandle;
use crate::context::Ctx;
use crate::dom::{Dom, NodeId};
use crate::message::{Flow, Message};
use crate::widget::{Child, RenderCtx, Widget, WidgetExt};
use crate::widgets::{KeyPanel, Markdown};
use rustual_render::Size;
use std::cell::RefCell;

const DEFAULT_CSS: &str = "
HelpPanel {
    dock: right;
    width: 33%;
    min-width: 30;
    max-width: 60;
    border-left: vkey $foreground 30%;
    padding: 0 1;
    height: 1fr;
    padding-right: 1;
    layout: vertical;
    height: 100%;

    #widget-help {
        height: auto;
        max-height: 50%;
        width: 1fr;
        padding: 0;
        margin: 0;
        padding: 1 0;
        margin-top: 1;
        display: none;
        background: $panel;

        MarkdownBlock {
            padding-left: 2;
            padding-right: 2;
        }
    }

    &.-show-help #widget-help {
        display: block;
    }

    KeyPanel#keys-help {
        width: 1fr;
        height: 1fr;
        border-left: none;
        padding: 0;
    }
}

/* ANSI themes (Textual `:ansi`). */
HelpPanel {
    &:ansi {
        background: ansi_default;
        border-left: vkey ansi_black;

        Markdown, KeyPanel {
            background: ansi_default;
        }
        .bindings-table--divide {
            color: transparent;
        }
    }
}

/* ANSI themes (Textual `:ansi`). */
HelpPanel {
    #widget-help {
        &:ansi {
            background: ansi_default;
        }
    }
}
";

/// Posted by the panel to itself when the focused widget's help changed.
struct RefreshHelp;

/// What the panel shows: the focused node and its help text.
type HelpState = (Option<NodeId>, Option<String>);

/// Shows help for the focused widget and the active key bindings.
#[derive(Default)]
pub struct HelpPanel {
    handle: Option<AppHandle>,
    node: Option<NodeId>,
    /// The state last shown or requested (to post at most one refresh per change).
    requested: RefCell<Option<HelpState>>,
}

impl HelpPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// The help (Markdown) for `focused`: its own [`Widget::help`] or the nearest ancestor's
    /// (Textual: `ancestors_with_self` lookup in `HelpPanel.update_help`).
    pub fn help_for(dom: &Dom, focused: NodeId) -> Option<String> {
        dom.ancestors_with_self(focused).into_iter().find_map(|id| {
            dom.get(id)?
                .widget()?
                .help()
                .filter(|h| !h.trim().is_empty())
                .map(|h| dedent(h.trim_end()))
        })
    }

    /// The focused node (on this panel's screen) and its help.
    fn current(dom: &Dom, panel: NodeId) -> HelpState {
        let focused = dom
            .focused
            .filter(|f| dom.screen_of(*f).is_some() && dom.screen_of(*f) == dom.screen_of(panel));
        let help = focused.and_then(|f| Self::help_for(dom, f));
        (focused, help)
    }

    /// Update the help document from the focused widget (Textual: `update_help`).
    fn update_help(&mut self, ctx: &mut Ctx) {
        let node = ctx.id();
        let state = Self::current(ctx.dom(), node);
        *self.requested.borrow_mut() = Some(state.clone());
        let (_, help) = state;
        ctx.set_class(help.is_some(), "-show-help");
        let text = help.unwrap_or_default();
        if let Some(md) = ctx.query_in(node, "#widget-help").into_iter().next() {
            let unchanged = ctx.get::<Markdown>(md).map(|m| m.source() == text);
            if unchanged == Some(false) {
                ctx.with_id::<Markdown, _>(md, |m, c| m.update(text, c));
            }
        }
    }
}

/// Textual's `textwrap.dedent`: remove the common leading whitespace of non-blank lines.
fn dedent(s: &str) -> String {
    let indent = s
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    s.lines()
        .map(|l| {
            if l.len() >= indent {
                &l[indent..]
            } else {
                l.trim_start()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Widget for HelpPanel {
    fn type_names(&self) -> &'static [&'static str] {
        &["HelpPanel"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn compose(&mut self) -> Vec<Child> {
        vec![
            Markdown::new("").id("widget-help"),
            KeyPanel::new().id("keys-help"),
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.add_class("-textual-system");
        self.node = Some(ctx.id());
        self.handle = Some(ctx.app_handle());
        self.update_help(ctx);
    }

    fn on_message(&mut self, message: &Message, ctx: &mut Ctx) -> Flow {
        if message.is::<RefreshHelp>() {
            self.update_help(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }

    /// Not a scroll view: this is the per-layout hook that notices focus changes.
    fn virtual_size(&self, ctx: &RenderCtx) -> Option<Size> {
        if let (Some(node), Some(handle)) = (self.node, self.handle.as_ref()) {
            let state = Self::current(ctx.dom, node);
            let mut requested = self.requested.borrow_mut();
            if requested.as_ref() != Some(&state) {
                *requested = Some(state);
                handle.post_message(node, RefreshHelp);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::dedent;

    #[test]
    fn dedent_common_indent() {
        assert_eq!(dedent("  # A\n\n    b\n  c"), "# A\n\n  b\nc");
        assert_eq!(dedent("x\n  y"), "x\n  y");
    }
}
