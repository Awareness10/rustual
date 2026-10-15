//! Form controls: ToggleButton, Checkbox, RadioButton, RadioSet, Switch, OptionList, Select,
//! SelectionList, MaskedInput.
//!
//! `Select` and `SelectionList` are generic over their value type; from Python they hold
//! arbitrary Python objects ([`PyValue`], compared with Python `==`).

use super::{arg, as_mut, as_ref, kw, new_py, none, to_py, Registry};
use crate::bridge::node_to_int;
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use rustual::widgets::option_list::{self, OptionItem, OptionListError};
use rustual::widgets::select::{self, Select};
use rustual::widgets::selection_list::{self, Selection, SelectionList};
use rustual::widgets::{
    self as w, checkbox, masked_input::MaskedInput, radio_button, radio_set, switch, toggle_button,
};
use rustual::{Ctx, Message, Widget};

// ---------------------------------------------------------------------------- PyValue

/// A Python object used as a `Select` / `SelectionList` value.
pub(crate) struct PyValue(pub Py<PyAny>);

impl Clone for PyValue {
    fn clone(&self) -> Self {
        Python::attach(|py| PyValue(self.0.clone_ref(py)))
    }
}

impl PartialEq for PyValue {
    fn eq(&self, other: &Self) -> bool {
        if self.0.is(&other.0) {
            return true;
        }
        Python::attach(|py| self.0.bind(py).eq(other.0.bind(py)).unwrap_or(false))
    }
}

impl std::fmt::Debug for PyValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PyValue(..)")
    }
}

impl PyValue {
    fn from(v: &Bound<'_, PyAny>) -> Self {
        PyValue(v.clone().unbind())
    }
    fn py(&self, py: Python<'_>) -> Py<PyAny> {
        self.0.clone_ref(py)
    }
}

type PySelect = Select<PyValue>;
type PySelectionList = SelectionList<PyValue>;

fn list_err(e: OptionListError) -> PyErr {
    PyValueError::new_err(e.to_string())
}

// ---------------------------------------------------------------------------- registration

pub(crate) fn register(r: &mut Registry) {
    r.kind("ToggleButton", |_, k, _| {
        let (label, value, first, compact) = toggle_args(k)?;
        Ok(Box::new(
            w::ToggleButton::new(label)
                .value(value)
                .button_first(first)
                .compact(compact),
        ))
    });
    r.kind("Checkbox", |_, k, _| {
        let (label, value, first, compact) = toggle_args(k)?;
        Ok(Box::new(
            w::Checkbox::new(label)
                .value(value)
                .button_first(first)
                .compact(compact),
        ))
    });
    r.kind("RadioButton", |_, k, _| {
        let (label, value, first, compact) = toggle_args(k)?;
        Ok(Box::new(
            w::RadioButton::new(label)
                .value(value)
                .button_first(first)
                .compact(compact),
        ))
    });
    r.kind("RadioSet", |_, k, _| {
        // Buttons are mounted as ordinary (Python) children.
        Ok(Box::new(
            w::RadioSet::default().compact(kw(k, "compact")?.unwrap_or(false)),
        ))
    });
    r.kind("Switch", |_, k, _| {
        Ok(Box::new(
            w::Switch::new()
                .value(kw(k, "value")?.unwrap_or(false))
                .animate(kw(k, "animate")?.unwrap_or(true)),
        ))
    });
    r.kind("OptionList", |_, k, _| {
        let mut list = w::OptionList::new().compact(kw(k, "compact")?.unwrap_or(false));
        if let Some(items) = kw::<Bound<'_, PyList>>(k, "options")? {
            add_option_items(&mut list, &items)?;
        }
        Ok(Box::new(list))
    });
    r.kind("Select", |_, k, _| {
        let options = select_options(kw(k, "options")?)?;
        let mut s = PySelect::new(options)
            .prompt(kw::<String>(k, "prompt")?.unwrap_or_else(|| "Select".into()))
            .allow_blank(kw(k, "allow_blank")?.unwrap_or(true));
        if let Some(v) = kw::<Bound<'_, PyAny>>(k, "value")? {
            s = s.value(PyValue::from(&v));
        }
        Ok(Box::new(s))
    });
    r.kind("SelectionList", |_, k, _| {
        let mut list = PySelectionList::new().compact(kw(k, "compact")?.unwrap_or(false));
        if let Some(items) = kw::<Bound<'_, PyList>>(k, "selections")? {
            list.add_options(selection_items(&items)?)
                .map_err(list_err)?;
        }
        Ok(Box::new(list))
    });
    r.kind("MaskedInput", |_, k, _| {
        let template: String = kw(k, "template")?.unwrap_or_default();
        let mut m =
            MaskedInput::try_new(&template).map_err(|e| PyValueError::new_err(e.to_string()))?;
        if let Some(p) = kw::<String>(k, "placeholder")? {
            m = m.placeholder(&p);
        }
        if let Some(v) = kw::<String>(k, "value")? {
            if !v.is_empty() {
                m = m.value(&v);
            }
        }
        m = m
            .valid_empty(kw(k, "valid_empty")?.unwrap_or(false))
            .select_on_focus(kw(k, "select_on_focus")?.unwrap_or(true))
            .compact(kw(k, "compact")?.unwrap_or(false));
        Ok(Box::new(m))
    });

    r.getter(get).setter(set).caller(call);

    r.message::<toggle_button::Changed>("on_toggle_button_changed", |py, m: &Message| {
        let p = m.get::<toggle_button::Changed>().expect("Changed");
        new_py(py, "ToggleButton.Changed", &value_kw(py, p.value)?)
    });
    r.message::<checkbox::Changed>("on_checkbox_changed", |py, m: &Message| {
        let p = m.get::<checkbox::Changed>().expect("Changed");
        new_py(py, "Checkbox.Changed", &value_kw(py, p.value)?)
    });
    r.message::<radio_button::Changed>("on_radio_button_changed", |py, m: &Message| {
        let p = m.get::<radio_button::Changed>().expect("Changed");
        new_py(py, "RadioButton.Changed", &value_kw(py, p.value)?)
    });
    r.message::<switch::Changed>("on_switch_changed", |py, m: &Message| {
        let p = m.get::<switch::Changed>().expect("Changed");
        new_py(py, "Switch.Changed", &value_kw(py, p.value)?)
    });
    r.message::<radio_set::Changed>("on_radio_set_changed", |py, m: &Message| {
        let p = m.get::<radio_set::Changed>().expect("Changed");
        let d = PyDict::new(py);
        d.set_item("index", p.pressed_index)?;
        d.set_item("pressed", node_to_int(p.pressed))?;
        new_py(py, "RadioSet.Changed", &d)
    });
    r.message::<option_list::OptionHighlighted>(
        "on_option_list_option_highlighted",
        |py, m: &Message| {
            let p = m.get::<option_list::OptionHighlighted>().expect("message");
            new_py(
                py,
                "OptionList.OptionHighlighted",
                &option_kw(py, p.option_index, &p.option_id)?,
            )
        },
    );
    r.message::<option_list::OptionSelected>(
        "on_option_list_option_selected",
        |py, m: &Message| {
            let p = m.get::<option_list::OptionSelected>().expect("message");
            new_py(
                py,
                "OptionList.OptionSelected",
                &option_kw(py, p.option_index, &p.option_id)?,
            )
        },
    );
    r.message::<select::Changed<PyValue>>("on_select_changed", |py, m: &Message| {
        let p = m.get::<select::Changed<PyValue>>().expect("Changed");
        let d = PyDict::new(py);
        d.set_item("value", p.value.as_ref().map(|v| v.py(py)))?;
        new_py(py, "Select.Changed", &d)
    });
    r.message::<selection_list::SelectionHighlighted<PyValue>>(
        "on_selection_list_selection_highlighted",
        |py, m: &Message| {
            let p = m
                .get::<selection_list::SelectionHighlighted<PyValue>>()
                .expect("message");
            let d = PyDict::new(py);
            d.set_item("index", p.selection_index)?;
            new_py(py, "SelectionList.SelectionHighlighted", &d)
        },
    );
    r.message::<selection_list::SelectionToggled<PyValue>>(
        "on_selection_list_selection_toggled",
        |py, m: &Message| {
            let p = m
                .get::<selection_list::SelectionToggled<PyValue>>()
                .expect("message");
            let d = PyDict::new(py);
            d.set_item("index", p.selection_index)?;
            new_py(py, "SelectionList.SelectionToggled", &d)
        },
    );
    r.message::<selection_list::SelectedChanged<PyValue>>(
        "on_selection_list_selected_changed",
        |py, m: &Message| {
            let p = m
                .get::<selection_list::SelectedChanged<PyValue>>()
                .expect("message");
            let d = PyDict::new(py);
            let selected: Vec<Py<PyAny>> = p.selected.iter().map(|v| v.py(py)).collect();
            d.set_item("selected", selected)?;
            new_py(py, "SelectionList.SelectedChanged", &d)
        },
    );
}

fn toggle_args(k: &Bound<'_, PyDict>) -> PyResult<(String, bool, bool, bool)> {
    Ok((
        kw::<String>(k, "label")?.unwrap_or_default(),
        kw(k, "value")?.unwrap_or(false),
        kw(k, "button_first")?.unwrap_or(true),
        kw(k, "compact")?.unwrap_or(false),
    ))
}

fn value_kw(py: Python<'_>, value: bool) -> PyResult<Bound<'_, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("value", value)?;
    Ok(d)
}

fn option_kw<'py>(
    py: Python<'py>,
    index: usize,
    id: &Option<String>,
) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("index", index)?;
    d.set_item("option_id", id)?;
    Ok(d)
}

/// Option items from Python: `(markup, id, disabled)` tuples, or `None` for a separator.
fn option_items(items: &Bound<'_, PyList>) -> PyResult<Vec<Option<OptionItem>>> {
    let mut out = Vec::new();
    for item in items.iter() {
        if item.is_none() {
            out.push(None);
            continue;
        }
        let (prompt, id, disabled): (String, Option<String>, bool) = item.extract()?;
        let mut o = OptionItem::new(prompt).disabled(disabled);
        if let Some(id) = id {
            o = o.id(id);
        }
        out.push(Some(o));
    }
    Ok(out)
}

fn add_option_items(list: &mut w::OptionList, items: &Bound<'_, PyList>) -> PyResult<()> {
    let items = option_items(items)?;
    let ids: Vec<&str> = items.iter().flatten().filter_map(|o| o.get_id()).collect();
    let mut seen = std::collections::HashSet::new();
    for id in ids {
        if list.get_option(id).is_ok() || !seen.insert(id) {
            return Err(list_err(OptionListError::DuplicateId(id.to_string())));
        }
    }
    for item in items {
        match item {
            Some(o) => list.add_option(o).map_err(list_err)?,
            None => list.add_separator(),
        }
    }
    Ok(())
}

/// `(markup, value)` pairs.
fn select_options(items: Option<Bound<'_, PyList>>) -> PyResult<Vec<(String, PyValue)>> {
    let Some(items) = items else {
        return Ok(Vec::new());
    };
    items
        .iter()
        .map(|item| {
            let (prompt, value): (String, Bound<'_, PyAny>) = item.extract()?;
            Ok((prompt, PyValue::from(&value)))
        })
        .collect()
}

/// `(markup, value, initial_state, id, disabled)` tuples.
fn selection_items(items: &Bound<'_, PyList>) -> PyResult<Vec<Selection<PyValue>>> {
    items
        .iter()
        .map(|item| {
            let (prompt, value, initial, id, disabled): (
                String,
                Bound<'_, PyAny>,
                bool,
                Option<String>,
                bool,
            ) = item.extract()?;
            let mut s = Selection::new(prompt, PyValue::from(&value))
                .initial_state(initial)
                .disabled(disabled);
            if let Some(id) = id {
                s = s.id(id);
            }
            Ok(s)
        })
        .collect()
}

// ---------------------------------------------------------------------------- toggles

fn toggle_ref(wd: &dyn Widget) -> Option<&w::ToggleButton> {
    as_ref::<w::ToggleButton>(wd)
        .or_else(|| as_ref::<w::Checkbox>(wd).map(|c| &**c))
        .or_else(|| as_ref::<w::RadioButton>(wd).map(|r| &**r))
}

fn toggle_mut(wd: &mut dyn Widget) -> Option<&mut w::ToggleButton> {
    // Check the type first: the borrow checker can't see that the failed downcasts end.
    let any = &*wd as &dyn std::any::Any;
    if any.is::<w::ToggleButton>() {
        as_mut::<w::ToggleButton>(wd)
    } else if any.is::<w::Checkbox>() {
        as_mut::<w::Checkbox>(wd).map(|c| &mut **c)
    } else if any.is::<w::RadioButton>() {
        as_mut::<w::RadioButton>(wd).map(|r| &mut **r)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------- get

fn get(py: Python<'_>, wd: &dyn Widget, attr: &str) -> Option<PyResult<Py<PyAny>>> {
    if let Some(t) = toggle_ref(wd) {
        return match attr {
            "value" => Some(to_py(py, t.get_value())),
            "label" => Some(to_py(py, t.label())),
            _ => None,
        };
    }
    if let Some(s) = as_ref::<w::Switch>(wd) {
        return match attr {
            "value" => Some(to_py(py, s.get_value())),
            "slider_pos" => Some(to_py(py, s.slider_position())),
            _ => None,
        };
    }
    if let Some(s) = as_ref::<w::RadioSet>(wd) {
        return match attr {
            "pressed_index" => Some(to_py(py, s.pressed_index())),
            "pressed_button" => Some(to_py(py, s.pressed_button().map(node_to_int))),
            "selected" => Some(to_py(py, s.selected_index())),
            _ => None,
        };
    }
    if let Some(l) = as_ref::<w::OptionList>(wd) {
        return match attr {
            "highlighted" => Some(to_py(py, l.highlighted())),
            "option_count" => Some(to_py(py, l.option_count())),
            "options" => Some(to_py(
                py,
                l.options()
                    .iter()
                    .map(|o| (o.plain_prompt(), o.get_id(), o.is_disabled()))
                    .collect::<Vec<_>>(),
            )),
            _ => None,
        };
    }
    if let Some(s) = as_ref::<PySelect>(wd) {
        return match attr {
            "value" => Some(Ok(match s.get_value() {
                Some(v) => v.py(py),
                None => py.None(),
            })),
            "expanded" => Some(to_py(py, s.is_expanded())),
            "prompt" => Some(to_py(py, s.get_prompt())),
            _ => None,
        };
    }
    if let Some(l) = as_ref::<PySelectionList>(wd) {
        return match attr {
            "selected" => Some(Ok(PyList::new(py, l.selected().iter().map(|v| v.py(py)))
                .map(|l| l.into_any().unbind())
                .unwrap_or_else(|_| py.None()))),
            "highlighted" => Some(to_py(py, l.highlighted())),
            "option_count" => Some(to_py(py, l.option_count())),
            _ => None,
        };
    }
    if let Some(m) = as_ref::<MaskedInput>(wd) {
        return match attr {
            "value" => Some(to_py(py, m.get_value())),
            "template" => Some(to_py(py, m.template())),
            "placeholder" => Some(to_py(py, m.input().get_placeholder())),
            "cursor_position" => Some(to_py(py, m.cursor_position())),
            "is_valid" => Some(to_py(py, m.is_valid())),
            "mask" => Some(to_py(py, m.mask())),
            "selected_text" => Some(to_py(py, m.input().selected_text())),
            "selection" => {
                let s = m.input().selection();
                Some(to_py(py, (s.start, s.end)))
            }
            _ => None,
        };
    }
    None
}

// ---------------------------------------------------------------------------- set

fn set(
    _py: Python<'_>,
    wd: &mut dyn Widget,
    ctx: &mut Ctx,
    attr: &str,
    v: &Bound<'_, PyAny>,
) -> Option<PyResult<()>> {
    if let Some(t) = toggle_mut(wd) {
        return match attr {
            "value" => Some(v.extract::<bool>().map(|b| t.set_value(b, ctx))),
            "label" => Some(v.extract::<String>().map(|s| {
                t.set_label(s);
                ctx.refresh();
            })),
            _ => None,
        };
    }
    if let Some(s) = as_mut::<w::Switch>(wd) {
        return match attr {
            "value" => Some(v.extract::<bool>().map(|b| s.set_value(b, ctx))),
            _ => None,
        };
    }
    if let Some(l) = as_mut::<w::OptionList>(wd) {
        return match attr {
            "highlighted" => Some(
                v.extract::<Option<usize>>()
                    .map(|h| l.set_highlighted(h, ctx)),
            ),
            _ => None,
        };
    }
    if let Some(s) = as_mut::<PySelect>(wd) {
        return match attr {
            // `None` means blank (Select.NULL is converted by the Python side).
            "value" => {
                let value = (!v.is_none()).then(|| PyValue::from(v));
                Some(if s.set_value(value, ctx) {
                    Ok(())
                } else {
                    Err(PyValueError::new_err("illegal select value"))
                })
            }
            "prompt" => Some(v.extract::<String>().map(|p| s.set_prompt(p, ctx))),
            _ => None,
        };
    }
    if let Some(l) = as_mut::<PySelectionList>(wd) {
        return match attr {
            "highlighted" => Some(
                v.extract::<Option<usize>>()
                    .map(|h| l.set_highlighted(h, ctx)),
            ),
            _ => None,
        };
    }
    if let Some(m) = as_mut::<MaskedInput>(wd) {
        return match attr {
            "value" => Some((|| {
                let s: String = v.extract()?;
                if m.set_value(&s, ctx) {
                    Ok(())
                } else {
                    Err(PyValueError::new_err("Value does not match template!"))
                }
            })()),
            "template" => Some((|| {
                let s: String = v.extract()?;
                m.set_template(&s, ctx)
                    .map_err(|e| PyValueError::new_err(e.to_string()))
            })()),
            "placeholder" => Some(v.extract::<String>().map(|s| m.set_placeholder(&s))),
            _ => None,
        };
    }
    None
}

// ---------------------------------------------------------------------------- call

fn call(
    py: Python<'_>,
    wd: &mut dyn Widget,
    ctx: &mut Ctx,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    let is_form = toggle_ref(wd).is_some()
        || as_ref::<w::Switch>(wd).is_some()
        || as_ref::<w::RadioSet>(wd).is_some()
        || as_ref::<w::OptionList>(wd).is_some()
        || as_ref::<PySelect>(wd).is_some()
        || as_ref::<PySelectionList>(wd).is_some()
        || as_ref::<MaskedInput>(wd).is_some();
    if is_form && method == "run_action" {
        // run_action(name): run one of the widget's own actions (key bindings).
        return Some((|| {
            let name: String = arg(args, 0)?;
            to_py(py, wd.run_action(&name, &[], ctx))
        })());
    }
    if let Some(t) = toggle_mut(wd) {
        return match method {
            "toggle" => Some({
                t.toggle(ctx);
                none(py)
            }),
            _ => None,
        };
    }
    if let Some(s) = as_mut::<w::Switch>(wd) {
        return match method {
            "toggle" => Some({
                s.toggle(ctx);
                none(py)
            }),
            _ => None,
        };
    }
    if let Some(l) = as_mut::<w::OptionList>(wd) {
        return option_list_call(py, l, ctx, method, args);
    }
    if let Some(s) = as_mut::<PySelect>(wd) {
        return match method {
            "set_options" => Some((|| {
                let opts = select_options(Some(arg::<Bound<'_, PyList>>(args, 0)?))?;
                s.set_options(opts, ctx);
                none(py)
            })()),
            "show_overlay" => Some({
                s.show_overlay(ctx);
                none(py)
            }),
            _ => None,
        };
    }
    if let Some(l) = as_mut::<PySelectionList>(wd) {
        return selection_list_call(py, l, ctx, method, args);
    }
    if let Some(m) = as_mut::<MaskedInput>(wd) {
        return match method {
            "clear" => Some({
                m.clear(ctx);
                none(py)
            }),
            "insert_text_at_cursor" => Some((|| {
                m.insert_text_at_cursor(&arg::<String>(args, 0)?, ctx);
                none(py)
            })()),
            "validate" => Some(to_py(py, m.validate(ctx))),
            "matches_template" => Some((|| {
                let v: String = arg(args, 0)?;
                to_py(py, m.matches_template(&v))
            })()),
            _ => None,
        };
    }
    None
}

fn index_err(i: usize) -> PyErr {
    PyIndexError::new_err(format!("no option at index {i}"))
}

fn option_list_call(
    py: Python<'_>,
    l: &mut w::OptionList,
    ctx: &mut Ctx,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    Some((|| match method {
        "add_options" => {
            add_option_items(l, &arg::<Bound<'_, PyList>>(args, 0)?)?;
            ctx.refresh();
            none(py)
        }
        "clear_options" => {
            l.clear_options(ctx);
            none(py)
        }
        "remove_option_at_index" => {
            let i: usize = arg(args, 0)?;
            l.remove_option_at_index(i, ctx).map_err(|_| index_err(i))?;
            none(py)
        }
        "set_option_disabled" => {
            let i: usize = arg(args, 0)?;
            let r = if arg::<bool>(args, 1)? {
                l.disable_option_at_index(i, ctx)
            } else {
                l.enable_option_at_index(i, ctx)
            };
            r.map_err(|_| index_err(i))?;
            ctx.refresh();
            none(py)
        }
        "replace_option_prompt_at_index" => {
            let i: usize = arg(args, 0)?;
            l.replace_option_prompt_at_index(i, arg::<String>(args, 1)?)
                .map_err(|_| index_err(i))?;
            ctx.refresh();
            none(py)
        }
        "scroll_to_highlight" => {
            l.scroll_to_highlight(ctx);
            none(py)
        }
        _ => Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "no native method {method:?}"
        ))),
    })())
}

fn selection_list_call(
    py: Python<'_>,
    l: &mut PySelectionList,
    ctx: &mut Ctx,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    let value = || -> PyResult<PyValue> { Ok(PyValue::from(&args.get_item(0)?)) };
    Some((|| match method {
        "add_options" => {
            let items = selection_items(&arg::<Bound<'_, PyList>>(args, 0)?)?;
            l.add_options(items).map_err(list_err)?;
            ctx.refresh();
            none(py)
        }
        "clear_options" => {
            l.clear_options(ctx);
            none(py)
        }
        "remove_option_at_index" => {
            let i: usize = arg(args, 0)?;
            l.remove_option_at_index(i, ctx).map_err(|_| index_err(i))?;
            none(py)
        }
        "set_option_disabled" => {
            let i: usize = arg(args, 0)?;
            l.set_option_disabled(i, arg(args, 1)?)
                .map_err(|_| index_err(i))?;
            ctx.refresh();
            none(py)
        }
        "select" => to_py(py, l.select(&value()?, ctx)),
        "deselect" => to_py(py, l.deselect(&value()?, ctx)),
        "toggle" => {
            l.toggle(&value()?, ctx);
            none(py)
        }
        "select_all" => {
            l.select_all(ctx);
            none(py)
        }
        "deselect_all" => {
            l.deselect_all(ctx);
            none(py)
        }
        "toggle_all" => {
            l.toggle_all(ctx);
            none(py)
        }
        _ => Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "no native method {method:?}"
        ))),
    })())
}
