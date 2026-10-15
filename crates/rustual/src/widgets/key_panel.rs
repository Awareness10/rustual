//! `KeyPanel` and `BindingsTable` (port of `textual/widgets/_key_panel.py`): a panel docked
//! on the right of the screen listing every key binding active for the focused widget
//! (including hidden ones), app bindings first.
//!
//! Shown on its own with the `app.show_keys` action (or `ctx.show_key_panel()`), hidden with
//! `app.hide_keys`; `app.toggle_keys` toggles. The [`HelpPanel`](crate::widgets::HelpPanel)
//! (`app.show_help_panel`, the "Keys" command of the command palette) embeds one below the
//! focused widget's help; an embedded panel is not docked.
//!
//! Deviations: Textual's `split: right` is approximated with `dock: right`; the panel is
//! re-rendered every frame from [`RenderEnv::all_bindings`](crate::RenderEnv) rather than
//! recomposed on a bindings-changed signal; no binding group titles or binding tooltips.

use crate::binding::{format_key, Binding};
use crate::context::Ctx;
use crate::widget::{Child, Content, RenderCtx, Widget};
use rustual_render::{Justify, Span, Strip, Style, Text, TextOverflow};

const KEY_PANEL_CSS: &str = "
KeyPanel {
    width: 33%;
    min-width: 30;
    max-width: 60;
    border-left: vkey $foreground 30%;
    padding: 0 1;
    height: 1fr;
    padding-right: 1;
    align: center top;
    layout: vertical;
    overflow-x: hidden;
    overflow-y: auto;

    &.-docked {
        dock: right;
    }

    &> BindingsTable > .bindings-table--key {
        color: $text-accent;
        text-style: bold;
    }

    &> BindingsTable > .bindings-table--description {
        color: $foreground;
    }

    &> BindingsTable > .bindings-table--divider {
        color: transparent;
    }

    &> BindingsTable > .bindings-table--header {
        color: $text-primary;
        text-style: underline;
    }
}
";

const BINDINGS_TABLE_CSS: &str = "
BindingsTable {
    width: auto;
    height: auto;
}
";

/// Rows of the table: groups (one per DOM node, app first) of `(keys, description)`.
fn table_rows(bindings: &[Vec<Binding>]) -> Vec<Vec<(String, String)>> {
    let mut out = Vec::new();
    for group in bindings {
        // Merge bindings with the same action (Textual: `action_to_bindings`).
        let mut rows: Vec<(String, Vec<String>, String)> = Vec::new();
        for b in group {
            // Textual's `ctrl+c` "help_quit" binding is a system binding (not listed).
            if b.keys == ["ctrl+c"] && b.action == "quit" {
                continue;
            }
            let displays: Vec<String> = match &b.key_display {
                Some(d) => vec![d.clone()],
                None => b.keys.iter().map(|k| format_key(k)).collect(),
            };
            match rows.iter_mut().find(|(a, _, _)| *a == b.action) {
                Some((_, keys, _)) => {
                    for d in displays {
                        if !keys.contains(&d) {
                            keys.push(d);
                        }
                    }
                }
                None => rows.push((b.action.clone(), displays, b.description.clone())),
            }
        }
        if !rows.is_empty() {
            out.push(
                rows.into_iter()
                    .map(|(_, keys, desc)| (keys.join(" "), desc))
                    .collect(),
            );
        }
    }
    out
}

/// The table of bindings shown in a [`KeyPanel`].
#[derive(Debug, Clone, Default)]
pub struct BindingsTable;

impl BindingsTable {
    fn lines(&self, ctx: &RenderCtx) -> Vec<Strip> {
        let groups = table_rows(&ctx.env.all_bindings);
        let key_style = ctx.component_style("bindings-table--key");
        let desc_style = ctx.component_style("bindings-table--description");
        let key_width = groups
            .iter()
            .flatten()
            .map(|(k, _)| Text::plain(k.clone()).cell_len())
            .max()
            .unwrap_or(0)
            + 2;
        let desc_width = (ctx.size.width - key_width).max(1);
        let mut lines = Vec::new();
        for (gi, group) in groups.iter().enumerate() {
            if gi > 0 {
                lines.push(Strip::default());
            }
            for (keys, desc) in group {
                // Keys right-justified with one cell of padding either side.
                let pad = (key_width - 2 - Text::plain(keys.clone()).cell_len()).max(0);
                let mut key_strip = Strip::blank(pad + 1, Style::new());
                key_strip.push(Span::new(keys.clone(), key_style.clone()));
                key_strip.push(Span::new(" ", Style::new()));
                let mut desc_text = Text::from_markup(desc);
                desc_text.stylize_base(&desc_style);
                let desc_lines =
                    desc_text.wrap(desc_width, Justify::Left, TextOverflow::Fold, false);
                for (i, d) in desc_lines.into_iter().enumerate() {
                    let mut line = if i == 0 {
                        key_strip.clone()
                    } else {
                        Strip::blank(key_width, Style::new())
                    };
                    for span in &d.spans {
                        line.push(span.clone());
                    }
                    lines.push(line);
                }
            }
        }
        lines
    }
}

impl Widget for BindingsTable {
    fn type_names(&self) -> &'static [&'static str] {
        &["BindingsTable"]
    }

    fn default_css(&self) -> &'static str {
        BINDINGS_TABLE_CSS
    }

    fn content_width(&self, ctx: &RenderCtx) -> Option<i32> {
        let groups = table_rows(&ctx.env.all_bindings);
        let key_width = groups
            .iter()
            .flatten()
            .map(|(k, _)| Text::plain(k.clone()).cell_len())
            .max()
            .unwrap_or(0)
            + 2;
        let desc_width = groups
            .iter()
            .flatten()
            .map(|(_, d)| Text::from_markup(d).cell_len())
            .max()
            .unwrap_or(0);
        Some((key_width + desc_width).min(ctx.size.width.max(1)))
    }

    fn content_height(&self, ctx: &RenderCtx, width: i32) -> Option<i32> {
        let mut sized = RenderCtx::new(ctx.dom, ctx.env, ctx.node, ctx.size);
        sized.size.width = width;
        Some(self.lines(&sized).len() as i32)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        Content::Lines(self.lines(ctx))
    }
}

/// A scrollable panel listing the active key bindings, docked on the right.
#[derive(Debug, Clone, Default)]
pub struct KeyPanel;

impl KeyPanel {
    pub fn new() -> Self {
        KeyPanel
    }
}

impl Widget for KeyPanel {
    fn type_names(&self) -> &'static [&'static str] {
        &["KeyPanel"]
    }

    fn default_css(&self) -> &'static str {
        KEY_PANEL_CSS
    }

    fn compose(&mut self) -> Vec<Child> {
        vec![Child::new(Box::new(BindingsTable)).id("bindings-table")]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.add_class("-textual-system");
        // Docked on the right unless embedded in a `HelpPanel` (Textual: `split: initial`).
        let embedded = ctx
            .parent()
            .and_then(|p| ctx.dom().get(p))
            .is_some_and(|n| n.is_type("HelpPanel"));
        ctx.set_class(!embedded, "-docked");
    }
}
