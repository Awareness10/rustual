//! TabbedContent / TabPane, Tabs / Tab, ContentSwitcher, Collapsible, Markdown,
//! MarkdownViewer, TextArea, Rule, Placeholder, LoadingIndicator, Link, Welcome, HelpPanel,
//! KeyPanel.
//!
//! Container widgets (`TabbedContent`, `TabPane`, `Tabs`, `ContentSwitcher`, `Collapsible`)
//! don't consume their children: the Python children are mounted as ordinary children and the
//! Rust widgets move them into place when they mount (as they do for `.with(...)` children).
//! Methods that need the DOM (pane / tab lookups) are callers, since getters have no `Ctx`;
//! they return node ids that the Python side turns into widgets.

use super::{arg, as_mut, as_ref, kw, new_py, none, to_py, Registry};
use crate::adapter::build_child;
use crate::bridge::node_to_int;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use rustual::widgets::markdown::{self, MarkdownTableOfContents};
use rustual::widgets::text_area::{self, Selection};
use rustual::widgets::{
    self as w, collapsible, link, tabbed_content, tabs, ContentSwitcher, IndentType, LineStyle,
    PlaceholderVariant, RuleOrientation, TabBehavior,
};
use rustual::{Ctx, NodeId, Widget};
use std::sync::Arc;

pub(crate) fn register(r: &mut Registry) {
    r.kind("TabbedContent", |_, k, _| {
        let mut t = w::TabbedContent::new();
        if let Some(titles) = kw::<Vec<String>>(k, "titles")? {
            t = t.titles(titles);
        }
        if let Some(initial) = kw::<String>(k, "initial")? {
            t = t.initial(initial);
        }
        Ok(Box::new(t))
    });
    r.kind("TabPane", |_, k, _| {
        Ok(Box::new(w::TabPane::new(
            kw::<String>(k, "title")?.unwrap_or_default(),
        )))
    });
    r.kind("Tabs", |_, k, _| {
        let mut t = w::Tabs::default();
        if let Some(active) = kw::<String>(k, "active")? {
            t = t.active(active);
        }
        Ok(Box::new(t))
    });
    r.kind("Tab", |_, k, _| {
        Ok(Box::new(w::Tab::new(
            kw::<String>(k, "label")?.unwrap_or_default(),
        )))
    });
    r.kind("ContentSwitcher", |_, k, _| {
        let mut s = ContentSwitcher::new();
        if let Some(initial) = kw::<String>(k, "initial")? {
            s = s.initial(initial);
        }
        Ok(Box::new(s))
    });
    r.kind("Collapsible", |_, k, _| {
        let mut c = w::Collapsible::new(kw::<String>(k, "title")?.unwrap_or("Toggle".into()))
            .collapsed(kw(k, "collapsed")?.unwrap_or(true));
        if let Some(s) = kw::<String>(k, "collapsed_symbol")? {
            c = c.collapsed_symbol(s);
        }
        if let Some(s) = kw::<String>(k, "expanded_symbol")? {
            c = c.expanded_symbol(s);
        }
        Ok(Box::new(c))
    });
    r.kind("Markdown", |_, k, _| {
        Ok(Box::new(
            w::Markdown::new(kw::<String>(k, "markdown")?.unwrap_or_default())
                .open_links(kw(k, "open_links")?.unwrap_or(true)),
        ))
    });
    r.kind("MarkdownViewer", |_, k, _| {
        Ok(Box::new(
            w::MarkdownViewer::new(kw::<String>(k, "markdown")?.unwrap_or_default())
                .open_links(kw(k, "open_links")?.unwrap_or(true))
                .show_table_of_contents(kw(k, "show_table_of_contents")?.unwrap_or(true)),
        ))
    });
    r.kind("MarkdownTableOfContents", |_, _, _| {
        Ok(Box::new(MarkdownTableOfContents::new()))
    });
    r.kind("TextArea", |_, k, _| {
        let text: String = kw(k, "text")?.unwrap_or_default();
        let mut t = w::TextArea::new(text)
            .soft_wrap(kw(k, "soft_wrap")?.unwrap_or(true))
            .read_only(kw(k, "read_only")?.unwrap_or(false))
            .show_cursor(kw(k, "show_cursor")?.unwrap_or(true))
            .show_line_numbers(kw(k, "show_line_numbers")?.unwrap_or(false))
            .line_number_start(kw(k, "line_number_start")?.unwrap_or(1))
            .max_checkpoints(kw(k, "max_checkpoints")?.unwrap_or(50))
            .compact(kw(k, "compact")?.unwrap_or(false))
            .highlight_cursor_line(kw(k, "highlight_cursor_line")?.unwrap_or(true))
            .placeholder(kw::<String>(k, "placeholder")?.unwrap_or_default());
        if let Some(b) = kw::<String>(k, "tab_behavior")? {
            t = t.tab_behavior(tab_behavior(&b)?);
        }
        if let Some(l) = kw::<String>(k, "language")? {
            t = t.language(&l);
        }
        if let Some(name) = kw::<String>(k, "theme")? {
            check_theme(&name)?;
            t = t.theme(&name);
        }
        if let Some(n) = kw::<usize>(k, "indent_width")? {
            t = t.indent_width(n);
        }
        if let Some(s) = kw::<String>(k, "indent_type")? {
            t = t.indent_type(indent_type(&s)?);
        }
        if let Some(b) = kw::<bool>(k, "cursor_blink")? {
            t = t.cursor_blink(b);
        }
        if let Some(b) = kw::<bool>(k, "match_cursor_bracket")? {
            t = t.match_cursor_bracket(b);
        }
        Ok(Box::new(t))
    });
    r.kind("Rule", |_, k, _| {
        let o = orientation(&kw::<String>(k, "orientation")?.unwrap_or("horizontal".into()))?;
        let s = line_style(&kw::<String>(k, "line_style")?.unwrap_or("solid".into()))?;
        Ok(Box::new(w::Rule::new(o, s)))
    });
    r.kind("Placeholder", |_, k, _| {
        let mut p = w::Placeholder::new().variant(placeholder_variant(
            &kw::<String>(k, "variant")?.unwrap_or_default(),
        )?);
        if let Some(label) = kw::<String>(k, "label")? {
            p = p.label(label);
        }
        Ok(Box::new(p))
    });
    r.kind("LoadingIndicator", |_, _, _| {
        Ok(Box::new(w::LoadingIndicator::new()))
    });
    r.kind("Link", |_, k, _| {
        let text: String = kw(k, "text")?.unwrap_or_default();
        let mut l = w::Link::new(text);
        if let Some(url) = kw::<String>(k, "url")? {
            l = l.url(url);
        }
        Ok(Box::new(l))
    });
    r.kind("Welcome", |_, k, _| {
        Ok(Box::new(
            w::Welcome::new().exit_on_close(kw(k, "exit_on_close")?.unwrap_or(false)),
        ))
    });
    r.kind("HelpPanel", |_, _, _| Ok(Box::new(w::HelpPanel::new())));
    r.kind("KeyPanel", |_, _, _| Ok(Box::new(w::KeyPanel::new())));

    r.getter(get).setter(set).caller(call);
    register_messages(r);
}

fn register_messages(r: &mut Registry) {
    r.message::<tabbed_content::TabActivated>("on_tabbed_content_tab_activated", |py, m| {
        let p = m
            .get::<tabbed_content::TabActivated>()
            .expect("TabActivated");
        let d = PyDict::new(py);
        d.set_item("pane_id", &p.pane_id)?;
        d.set_item("tab_id", &p.tab_id)?;
        new_py(py, "TabbedContent.TabActivated", &d)
    });
    r.message::<tabbed_content::Cleared>("on_tabbed_content_cleared", |py, _| {
        new_py(py, "TabbedContent.Cleared", &PyDict::new(py))
    });
    r.message::<tabs::TabActivated>("on_tabs_tab_activated", |py, m| {
        let p = m.get::<tabs::TabActivated>().expect("TabActivated");
        tab_message(py, "Tabs.TabActivated", &p.tab_id)
    });
    r.message::<tabs::Cleared>("on_tabs_cleared", |py, _| {
        new_py(py, "Tabs.Cleared", &PyDict::new(py))
    });
    r.message::<tabs::TabDisabled>("on_tabs_tab_disabled", |py, m| {
        let p = m.get::<tabs::TabDisabled>().expect("TabDisabled");
        tab_message(py, "Tabs.TabDisabled", &p.tab_id)
    });
    r.message::<tabs::TabEnabled>("on_tabs_tab_enabled", |py, m| {
        let p = m.get::<tabs::TabEnabled>().expect("TabEnabled");
        tab_message(py, "Tabs.TabEnabled", &p.tab_id)
    });
    r.message::<tabs::TabHidden>("on_tabs_tab_hidden", |py, m| {
        let p = m.get::<tabs::TabHidden>().expect("TabHidden");
        tab_message(py, "Tabs.TabHidden", &p.tab_id)
    });
    r.message::<tabs::TabShown>("on_tabs_tab_shown", |py, m| {
        let p = m.get::<tabs::TabShown>().expect("TabShown");
        tab_message(py, "Tabs.TabShown", &p.tab_id)
    });
    r.message::<collapsible::Expanded>("on_collapsible_expanded", |py, _| {
        new_py(py, "Collapsible.Expanded", &PyDict::new(py))
    });
    r.message::<collapsible::Collapsed>("on_collapsible_collapsed", |py, _| {
        new_py(py, "Collapsible.Collapsed", &PyDict::new(py))
    });
    r.message::<markdown::LinkClicked>("on_markdown_link_clicked", |py, m| {
        let p = m.get::<markdown::LinkClicked>().expect("LinkClicked");
        let d = PyDict::new(py);
        d.set_item("href", &p.href)?;
        new_py(py, "Markdown.LinkClicked", &d)
    });
    r.message::<markdown::TableOfContentsUpdated>(
        "on_markdown_table_of_contents_updated",
        |py, m| {
            let p = m
                .get::<markdown::TableOfContentsUpdated>()
                .expect("TableOfContentsUpdated");
            let d = PyDict::new(py);
            d.set_item("table_of_contents", p.table_of_contents.clone())?;
            new_py(py, "Markdown.TableOfContentsUpdated", &d)
        },
    );
    r.message::<markdown::TableOfContentsSelected>(
        "on_markdown_table_of_contents_selected",
        |py, m| {
            let p = m
                .get::<markdown::TableOfContentsSelected>()
                .expect("TableOfContentsSelected");
            let d = PyDict::new(py);
            d.set_item("block_id", &p.block_id)?;
            new_py(py, "Markdown.TableOfContentsSelected", &d)
        },
    );
    r.message::<markdown::NavigatorUpdated>("on_markdown_viewer_navigator_updated", |py, _| {
        new_py(py, "MarkdownViewer.NavigatorUpdated", &PyDict::new(py))
    });
    r.message::<text_area::Changed>("on_text_area_changed", |py, _| {
        new_py(py, "TextArea.Changed", &PyDict::new(py))
    });
    r.message::<text_area::SelectionChanged>("on_text_area_selection_changed", |py, m| {
        let p = m
            .get::<text_area::SelectionChanged>()
            .expect("SelectionChanged");
        let d = PyDict::new(py);
        d.set_item("selection", (p.selection.start, p.selection.end))?;
        new_py(py, "TextArea.SelectionChanged", &d)
    });
}

fn tab_message<'py>(py: Python<'py>, key: &str, tab_id: &str) -> PyResult<Bound<'py, PyAny>> {
    let d = PyDict::new(py);
    d.set_item("tab_id", tab_id)?;
    new_py(py, key, &d)
}

// ---------------------------------------------------------------------------- enums

fn tab_behavior(s: &str) -> PyResult<TabBehavior> {
    match s {
        "focus" => Ok(TabBehavior::Focus),
        "indent" => Ok(TabBehavior::Indent),
        _ => Err(PyValueError::new_err(format!("invalid tab_behavior {s:?}"))),
    }
}

fn tab_behavior_name(b: TabBehavior) -> &'static str {
    match b {
        TabBehavior::Focus => "focus",
        TabBehavior::Indent => "indent",
    }
}

fn indent_type(s: &str) -> PyResult<IndentType> {
    match s {
        "spaces" => Ok(IndentType::Spaces),
        "tabs" => Ok(IndentType::Tabs),
        _ => Err(PyValueError::new_err(format!("invalid indent_type {s:?}"))),
    }
}

fn check_theme(name: &str) -> PyResult<()> {
    if w::TextAreaTheme::get_builtin_theme(name).is_some() {
        Ok(())
    } else {
        Err(PyValueError::new_err(format!(
            "theme {name:?} does not exist"
        )))
    }
}

fn orientation(s: &str) -> PyResult<RuleOrientation> {
    match s {
        "horizontal" => Ok(RuleOrientation::Horizontal),
        "vertical" => Ok(RuleOrientation::Vertical),
        _ => Err(PyValueError::new_err(format!(
            "invalid rule orientation {s:?}"
        ))),
    }
}

fn orientation_name(o: RuleOrientation) -> &'static str {
    match o {
        RuleOrientation::Horizontal => "horizontal",
        RuleOrientation::Vertical => "vertical",
    }
}

fn line_style(s: &str) -> PyResult<LineStyle> {
    LineStyle::parse(s).ok_or_else(|| PyValueError::new_err(format!("invalid line style {s:?}")))
}

fn line_style_name(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Ascii => "ascii",
        LineStyle::Blank => "blank",
        LineStyle::Dashed => "dashed",
        LineStyle::Double => "double",
        LineStyle::Heavy => "heavy",
        LineStyle::Hidden => "hidden",
        LineStyle::None => "none",
        LineStyle::Solid => "solid",
        LineStyle::Thick => "thick",
    }
}

fn placeholder_variant(s: &str) -> PyResult<PlaceholderVariant> {
    match s {
        "" | "default" => Ok(PlaceholderVariant::Default),
        "size" => Ok(PlaceholderVariant::Size),
        "text" => Ok(PlaceholderVariant::Text),
        _ => Err(PyValueError::new_err(format!(
            "invalid placeholder variant {s:?}"
        ))),
    }
}

// ---------------------------------------------------------------------------- getters

fn get(py: Python<'_>, wd: &dyn Widget, attr: &str) -> Option<PyResult<Py<PyAny>>> {
    if let Some(t) = as_ref::<w::TabPane>(wd) {
        return match attr {
            "title" => Some(to_py(py, t.title())),
            _ => None,
        };
    }
    if let Some(t) = as_ref::<w::Tab>(wd) {
        return match attr {
            "label" => Some(to_py(py, t.label())),
            "label_text" => Some(to_py(py, t.label_text())),
            _ => None,
        };
    }
    if let Some(s) = as_ref::<ContentSwitcher>(wd) {
        return match attr {
            "current" => Some(to_py(py, s.current())),
            _ => None,
        };
    }
    if let Some(c) = as_ref::<w::Collapsible>(wd) {
        return match attr {
            "title" => Some(to_py(py, c.title())),
            "collapsed" => Some(to_py(py, c.is_collapsed())),
            _ => None,
        };
    }
    if let Some(m) = as_ref::<w::Markdown>(wd) {
        return match attr {
            "source" => Some(to_py(py, m.source())),
            "table_of_contents" => Some(to_py(py, m.table_of_contents().to_vec())),
            "open_links" => Some(to_py(py, m.opens_links())),
            _ => None,
        };
    }
    if let Some(m) = as_ref::<w::MarkdownViewer>(wd) {
        return match attr {
            "show_table_of_contents" => Some(to_py(py, m.is_showing_table_of_contents())),
            "location" => Some(to_py(py, m.navigator().location())),
            "navigator_start" => Some(to_py(py, m.navigator().start())),
            "navigator_end" => Some(to_py(py, m.navigator().end())),
            _ => None,
        };
    }
    if let Some(t) = as_ref::<MarkdownTableOfContents>(wd) {
        return match attr {
            "table_of_contents" => Some(to_py(py, t.table_of_contents().to_vec())),
            _ => None,
        };
    }
    if let Some(t) = as_ref::<w::TextArea>(wd) {
        return text_area_get(py, t, attr);
    }
    if let Some(r) = as_ref::<w::Rule>(wd) {
        return match attr {
            "orientation" => Some(to_py(py, orientation_name(r.get_orientation()))),
            "line_style" => Some(to_py(py, line_style_name(r.get_line_style()))),
            _ => None,
        };
    }
    if let Some(p) = as_ref::<w::Placeholder>(wd) {
        return match attr {
            "variant" => Some(to_py(py, p.get_variant().name())),
            _ => None,
        };
    }
    if let Some(l) = as_ref::<w::Link>(wd) {
        return match attr {
            "text" => Some(to_py(py, l.text())),
            "url" => Some(to_py(py, l.get_url())),
            _ => None,
        };
    }
    None
}

fn text_area_get(py: Python<'_>, t: &w::TextArea, attr: &str) -> Option<PyResult<Py<PyAny>>> {
    let doc = t.document();
    Some(match attr {
        "text" => to_py(py, t.text()),
        "selection" => {
            let s = t.selection();
            to_py(py, (s.start, s.end))
        }
        "cursor_location" => to_py(py, t.cursor_location()),
        "selected_text" => to_py(py, t.selected_text()),
        "language" => to_py(py, t.get_language()),
        "theme" => to_py(py, t.theme_name()),
        "read_only" => to_py(py, t.is_read_only()),
        "soft_wrap" => to_py(py, t.is_soft_wrap()),
        "show_line_numbers" => to_py(py, t.is_showing_line_numbers()),
        "indent_width" => to_py(py, t.get_indent_width()),
        "tab_behavior" => to_py(py, tab_behavior_name(t.get_tab_behavior())),
        "placeholder" => to_py(py, t.get_placeholder()),
        "matching_bracket_location" => to_py(py, t.matching_bracket_location()),
        "available_themes" => to_py(py, t.available_themes()),
        "available_languages" => to_py(py, t.available_languages()),
        "gutter_width" => to_py(py, t.gutter_width()),
        "cursor_at_first_line" => to_py(py, t.cursor_at_first_line()),
        "cursor_at_last_line" => to_py(py, t.cursor_at_last_line()),
        "cursor_at_start_of_line" => to_py(py, t.cursor_at_start_of_line()),
        "cursor_at_end_of_line" => to_py(py, t.cursor_at_end_of_line()),
        "cursor_at_start_of_text" => to_py(py, t.cursor_at_start_of_text()),
        "cursor_at_end_of_text" => to_py(py, t.cursor_at_end_of_text()),
        "can_undo" => to_py(py, t.history().can_undo()),
        "can_redo" => to_py(py, t.history().can_redo()),
        "line_count" => to_py(py, doc.line_count()),
        "lines" => to_py(py, doc.lines().to_vec()),
        "newline" => to_py(py, doc.newline().as_str()),
        "start" => to_py(py, doc.start()),
        "end" => to_py(py, doc.end()),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------- setters

fn set(
    _py: Python<'_>,
    wd: &mut dyn Widget,
    ctx: &mut Ctx,
    attr: &str,
    v: &Bound<'_, PyAny>,
) -> Option<PyResult<()>> {
    if let Some(t) = as_mut::<w::TabbedContent>(wd) {
        return match attr {
            "active" => Some((|| {
                let id: String = v.extract()?;
                if id.is_empty() {
                    return Err(PyValueError::new_err(
                        "'active' tab must not be empty string.",
                    ));
                }
                if t.get_pane(&id, ctx).is_none() || !t.set_active(&id, ctx) {
                    return Err(PyValueError::new_err(format!("no pane with id {id:?}")));
                }
                Ok(())
            })()),
            _ => None,
        };
    }
    if let Some(t) = as_mut::<w::Tabs>(wd) {
        return match attr {
            "active" => Some((|| {
                let id: String = v.extract()?;
                if !t.set_active(&id, ctx) {
                    return Err(PyValueError::new_err(format!("no tab with id {id:?}")));
                }
                Ok(())
            })()),
            _ => None,
        };
    }
    if let Some(t) = as_mut::<w::Tab>(wd) {
        return match attr {
            "label" => Some(v.extract::<String>().map(|s| {
                t.set_label(s);
                ctx.refresh();
            })),
            _ => None,
        };
    }
    if let Some(s) = as_mut::<ContentSwitcher>(wd) {
        return match attr {
            "current" => Some(
                v.extract::<Option<String>>()
                    .map(|c| s.set_current(c.as_deref(), ctx)),
            ),
            _ => None,
        };
    }
    if let Some(c) = as_mut::<w::Collapsible>(wd) {
        return match attr {
            "title" => Some(v.extract::<String>().map(|t| c.set_title(t, ctx))),
            "collapsed" => Some(v.extract::<bool>().map(|b| c.set_collapsed(b, ctx))),
            _ => None,
        };
    }
    if let Some(m) = as_mut::<w::Markdown>(wd) {
        return match attr {
            "open_links" => Some(v.extract::<bool>().map(|b| m.set_open_links(b))),
            _ => None,
        };
    }
    if let Some(m) = as_mut::<w::MarkdownViewer>(wd) {
        return match attr {
            "show_table_of_contents" => Some(
                v.extract::<bool>()
                    .map(|b| m.set_show_table_of_contents(b, ctx)),
            ),
            _ => None,
        };
    }
    if let Some(t) = as_mut::<w::TextArea>(wd) {
        return text_area_set(t, ctx, attr, v);
    }
    if let Some(r) = as_mut::<w::Rule>(wd) {
        return match attr {
            "orientation" => Some((|| {
                r.set_orientation(orientation(&v.extract::<String>()?)?, ctx);
                Ok(())
            })()),
            "line_style" => Some((|| {
                r.set_line_style(line_style(&v.extract::<String>()?)?);
                ctx.refresh();
                Ok(())
            })()),
            _ => None,
        };
    }
    if let Some(p) = as_mut::<w::Placeholder>(wd) {
        return match attr {
            "variant" => Some((|| {
                p.set_variant(placeholder_variant(&v.extract::<String>()?)?, ctx);
                Ok(())
            })()),
            _ => None,
        };
    }
    if let Some(l) = as_mut::<w::Link>(wd) {
        return match attr {
            "text" => Some(v.extract::<String>().map(|s| {
                l.set_text(s);
                ctx.refresh();
            })),
            "url" => Some(v.extract::<String>().map(|s| l.set_url(s))),
            _ => None,
        };
    }
    None
}

fn text_area_set(
    t: &mut w::TextArea,
    ctx: &mut Ctx,
    attr: &str,
    v: &Bound<'_, PyAny>,
) -> Option<PyResult<()>> {
    const ATTRS: &[&str] = &[
        "text",
        "selection",
        "cursor_location",
        "language",
        "theme",
        "read_only",
        "soft_wrap",
        "show_line_numbers",
        "indent_width",
        "indent_type",
        "tab_behavior",
        "placeholder",
        "cursor_blink",
        "match_cursor_bracket",
        "highlight_cursor_line",
        "max_checkpoints",
    ];
    if !ATTRS.contains(&attr) {
        return None;
    }
    Some((|| {
        match attr {
            "text" => t.set_text(&v.extract::<String>()?, ctx),
            "selection" => {
                let (s, e): ((usize, usize), (usize, usize)) = v.extract()?;
                t.set_selection(Selection::new(s, e), ctx);
            }
            "cursor_location" => t.move_cursor(v.extract()?, false, ctx),
            "language" => t.set_language(v.extract::<Option<String>>()?.as_deref()),
            "theme" => {
                let name: String = v.extract()?;
                if !t.set_theme(&name, ctx) {
                    return Err(PyValueError::new_err(format!(
                        "theme {name:?} does not exist"
                    )));
                }
            }
            "read_only" => t.set_read_only(v.extract()?, ctx),
            "soft_wrap" => t.set_soft_wrap(v.extract()?, ctx),
            "show_line_numbers" => t.set_show_line_numbers(v.extract()?, ctx),
            "indent_width" => t.set_indent_width(v.extract()?, ctx),
            "indent_type" => t.set_indent_type(indent_type(&v.extract::<String>()?)?),
            "tab_behavior" => t.set_tab_behavior(tab_behavior(&v.extract::<String>()?)?),
            "placeholder" => t.set_placeholder(v.extract::<String>()?),
            "cursor_blink" => t.set_cursor_blink(v.extract()?, ctx),
            "match_cursor_bracket" => t.set_match_cursor_bracket(v.extract()?),
            "highlight_cursor_line" => t.set_highlight_cursor_line(v.extract()?),
            _ => t.set_max_checkpoints(v.extract()?),
        }
        ctx.refresh();
        Ok(())
    })())
}

// ---------------------------------------------------------------------------- methods

fn node(py: Python<'_>, n: Option<NodeId>) -> PyResult<Py<PyAny>> {
    to_py(py, n.map(node_to_int))
}

fn opt_arg<'py, T: FromPyObjectOwned<'py>>(
    args: &Bound<'py, PyTuple>,
    i: usize,
) -> PyResult<Option<T>> {
    if i >= args.len() {
        return Ok(None);
    }
    let v = args.get_item(i)?;
    if v.is_none() {
        return Ok(None);
    }
    v.extract::<T>().map(Some).map_err(Into::into)
}

/// Index for an insertion `before` / `after` the item with the given id, among `ids`.
fn insert_index(
    ids: &[String],
    before: Option<String>,
    after: Option<String>,
    what: &str,
) -> PyResult<Option<usize>> {
    let find = |id: &str| {
        ids.iter()
            .position(|i| i == id)
            .ok_or_else(|| PyValueError::new_err(format!("there is no {what} with id {id:?}")))
    };
    match (before, after) {
        (Some(_), Some(_)) => Err(PyValueError::new_err(
            "only one of 'before' or 'after' may be given",
        )),
        (Some(b), None) => Ok(Some(find(&b)?)),
        (None, Some(a)) => Ok(Some(find(&a)? + 1)),
        (None, None) => Ok(None),
    }
}

fn call(
    py: Python<'_>,
    wd: &mut dyn Widget,
    ctx: &mut Ctx,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    if let Some(t) = as_mut::<w::TabbedContent>(wd) {
        return tabbed_content_call(py, t, ctx, method, args);
    }
    if let Some(t) = as_mut::<w::Tabs>(wd) {
        return tabs_call(py, t, ctx, method, args);
    }
    if let Some(s) = as_mut::<ContentSwitcher>(wd) {
        return match method {
            "visible_content" => Some(node(py, s.visible_content(ctx))),
            _ => None,
        };
    }
    if let Some(m) = as_mut::<w::Markdown>(wd) {
        return Some(match method {
            "update" => (|| {
                m.update(arg::<String>(args, 0)?, ctx);
                none(py)
            })(),
            "append" => (|| {
                m.append(&arg::<String>(args, 0)?, ctx);
                none(py)
            })(),
            "load" => (|| {
                let path: std::path::PathBuf = arg(args, 0)?;
                m.load(path, ctx).map_err(PyErr::from)?;
                none(py)
            })(),
            "goto_anchor" => (|| to_py(py, m.goto_anchor(&arg::<String>(args, 0)?, ctx)))(),
            _ => return None,
        });
    }
    if let Some(m) = as_mut::<w::MarkdownViewer>(wd) {
        return Some(match method {
            "go" => (|| to_py(py, m.go(&arg::<String>(args, 0)?, ctx)))(),
            "back" => to_py(py, m.back(ctx)),
            "forward" => to_py(py, m.forward(ctx)),
            "document" => node(py, m.document_id(ctx)),
            "update" => (|| {
                m.update(arg::<String>(args, 0)?, ctx);
                none(py)
            })(),
            _ => return None,
        });
    }
    if let Some(t) = as_mut::<w::TextArea>(wd) {
        return text_area_call(py, t, ctx, method, args);
    }
    if let Some(p) = as_mut::<w::Placeholder>(wd) {
        return match method {
            "cycle_variant" => Some({
                p.cycle_variant(ctx);
                none(py)
            }),
            _ => None,
        };
    }
    if let Some(l) = as_mut::<w::Link>(wd) {
        return match method {
            "action_open_link" => Some({
                l.action_open_link(ctx);
                none(py)
            }),
            _ => None,
        };
    }
    None
}

fn pane_ids(t: &w::TabbedContent, ctx: &Ctx) -> Vec<String> {
    t.panes(ctx)
        .into_iter()
        .filter_map(|p| ctx.css_id(p).map(String::from))
        .collect()
}

fn tabbed_content_call(
    py: Python<'_>,
    t: &mut w::TabbedContent,
    ctx: &mut Ctx,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    let id = || arg::<String>(args, 0);
    Some(match method {
        "active" => {
            // The active pane according to the tabs (updated immediately, unlike the
            // widget's own copy which follows the TabActivated message).
            let active = t
                .tabs_node()
                .and_then(|n| ctx.dom().widget::<w::Tabs>(n))
                .map(|tabs| tabbed_content::sans_prefix(tabs.active_id()).to_string())
                .unwrap_or_else(|| t.active().to_string());
            to_py(py, active)
        }
        // add_pane(pane, before_id, after_id)
        "add_pane" => (|| {
            let index = insert_index(
                &pane_ids(t, ctx),
                opt_arg(args, 1)?,
                opt_arg(args, 2)?,
                "pane",
            )?;
            let child = build_child(py, &args.get_item(0)?)?;
            t.insert_pane(child, index, ctx);
            none(py)
        })(),
        "remove_pane" => (|| {
            t.remove_pane(&id()?, ctx);
            none(py)
        })(),
        "clear_panes" => {
            t.clear_panes(ctx);
            none(py)
        }
        "get_pane" => (|| node(py, t.get_pane(&id()?, ctx)))(),
        "get_tab" => (|| node(py, t.get_tab(&id()?, ctx)))(),
        "active_pane" => node(py, t.active_pane(ctx)),
        "tab_count" => to_py(py, t.tab_count(ctx)),
        "pane_ids" => to_py(py, pane_ids(t, ctx)),
        "tabs_node" => node(py, t.tabs_node()),
        "switcher_node" => node(py, t.switcher_node()),
        "disable_tab" => (|| to_py(py, t.disable_tab(&id()?, ctx)))(),
        "enable_tab" => (|| to_py(py, t.enable_tab(&id()?, ctx)))(),
        "hide_tab" => (|| to_py(py, t.hide_tab(&id()?, ctx)))(),
        "show_tab" => (|| to_py(py, t.show_tab(&id()?, ctx)))(),
        _ => return None,
    })
}

fn tabs_call(
    py: Python<'_>,
    t: &mut w::Tabs,
    ctx: &mut Ctx,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    let id = || arg::<String>(args, 0);
    Some(match method {
        "active" => to_py(py, t.active_id()),
        // add_tab(tab, before_id, after_id)
        "add_tab" => (|| {
            let index = insert_index(&t.tab_ids(ctx), opt_arg(args, 1)?, opt_arg(args, 2)?, "tab")?;
            let child = build_child(py, &args.get_item(0)?)?;
            to_py(py, t.mount_tab(child, index, ctx))
        })(),
        "remove_tab" => (|| {
            t.remove_tab(&id()?, ctx);
            none(py)
        })(),
        "clear" => {
            t.clear(ctx);
            none(py)
        }
        "get_tab" => (|| node(py, t.get_tab(&id()?, ctx)))(),
        "active_tab" => node(py, t.active_tab(ctx)),
        "tab_count" => to_py(py, t.tab_count(ctx)),
        "tab_ids" => to_py(py, t.tab_ids(ctx)),
        "next_tab" => {
            t.next_tab(ctx);
            none(py)
        }
        "previous_tab" => {
            t.previous_tab(ctx);
            none(py)
        }
        "disable" => (|| to_py(py, t.disable(&id()?, ctx)))(),
        "enable" => (|| to_py(py, t.enable(&id()?, ctx)))(),
        "hide" => (|| to_py(py, t.hide(&id()?, ctx)))(),
        "show" => (|| to_py(py, t.show(&id()?, ctx)))(),
        _ => return None,
    })
}

fn edit_result(py: Python<'_>, r: text_area::EditResult) -> PyResult<Py<PyAny>> {
    to_py(py, (r.end_location, r.replaced_text))
}

fn text_area_call(
    py: Python<'_>,
    t: &mut w::TextArea,
    ctx: &mut Ctx,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    Some(match method {
        "load_text" => (|| {
            t.load_text(&arg::<String>(args, 0)?, ctx);
            none(py)
        })(),
        // insert(text, location | None)
        "insert" => (|| {
            let r = t.insert(&arg::<String>(args, 0)?, opt_arg(args, 1)?, ctx);
            edit_result(py, r)
        })(),
        "delete" => (|| {
            let r = t.delete(arg(args, 0)?, arg(args, 1)?, ctx);
            edit_result(py, r)
        })(),
        "replace" => (|| {
            let r = t.replace(&arg::<String>(args, 0)?, arg(args, 1)?, arg(args, 2)?, ctx);
            edit_result(py, r)
        })(),
        "clear" => {
            let r = t.clear(ctx);
            edit_result(py, r)
        }
        "undo" => {
            t.undo(ctx);
            none(py)
        }
        "redo" => {
            t.redo(ctx);
            none(py)
        }
        "select_line" => (|| {
            t.select_line(arg(args, 0)?, ctx);
            none(py)
        })(),
        "select_all" => {
            t.select_all(ctx);
            none(py)
        }
        // move_cursor(location, select)
        "move_cursor" => (|| {
            t.move_cursor(arg(args, 0)?, arg(args, 1)?, ctx);
            none(py)
        })(),
        // move_cursor_relative(rows, columns, select)
        "move_cursor_relative" => (|| {
            t.move_cursor_relative(arg(args, 0)?, arg(args, 1)?, arg(args, 2)?, ctx);
            none(py)
        })(),
        "get_text_range" => (|| to_py(py, t.get_text_range(arg(args, 0)?, arg(args, 1)?)))(),
        "get_line" => (|| {
            let i: usize = arg(args, 0)?;
            let doc = t.document();
            if i >= doc.line_count() {
                return Err(pyo3::exceptions::PyIndexError::new_err(
                    "line index out of range",
                ));
            }
            to_py(py, doc.get_line(i))
        })(),
        "get_index_from_location" => {
            (|| to_py(py, t.document().get_index_from_location(arg(args, 0)?)))()
        }
        "get_location_from_index" => {
            (|| to_py(py, t.document().get_location_from_index(arg(args, 0)?)))()
        }
        "scroll_cursor_visible" => {
            t.scroll_cursor_visible(ctx);
            none(py)
        }
        "copy" => to_py(py, t.action_copy()),
        "cut" => {
            t.action_cut(ctx);
            none(py)
        }
        "paste" => {
            t.action_paste(ctx);
            none(py)
        }
        _ => return None,
    })
}

// ---------------------------------------------------------------------------- functions

/// Route `open_url` (links in `Link` / `Markdown`) through a Python callable taking the URL
/// (`None` restores the system opener).
#[pyfunction]
#[pyo3(signature = (opener=None))]
pub fn set_url_opener(opener: Option<Py<PyAny>>) {
    link::set_url_opener(opener.map(|f| {
        Arc::new(move |url: &str| {
            Python::attach(|py| {
                f.bind(py)
                    .call1((url,))
                    .map(drop)
                    .map_err(|e| std::io::Error::other(e.to_string()))
            })
        }) as Arc<dyn Fn(&str) -> std::io::Result<()> + Send + Sync>
    }));
}

pub(crate) fn add_functions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(set_url_opener, m)?)
}
