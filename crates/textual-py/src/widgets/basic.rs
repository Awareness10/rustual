//! Containers, Screen, Static, Label, Button, Input, Header, Footer.

use super::rich::RichHost;
use super::{arg, as_mut, as_ref, kw, new_py, none, to_py, Registry};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use textual::widgets::{self as w, button, input};
use textual::{Ctx, Message, Widget};

pub(crate) fn register(r: &mut Registry) {
    macro_rules! containers {
        ($($name:ident),*) => {
            $( r.kind(stringify!($name), |_, _, _| Ok(Box::new(w::$name::new()))); )*
        };
    }
    containers!(
        Container,
        Vertical,
        VerticalGroup,
        Horizontal,
        HorizontalGroup,
        Center,
        Right,
        Middle,
        CenterMiddle,
        Grid,
        ItemGrid,
        ScrollableContainer,
        VerticalScroll,
        HorizontalScroll
    );
    r.kind("Screen", |_, _, _| Ok(Box::new(textual::Screen::new())));
    r.kind("Static", |_, k, _| {
        let content: String = kw(k, "content")?.unwrap_or_default();
        let inner = if kw::<bool>(k, "markup")?.unwrap_or(true) {
            w::Static::new(content)
        } else {
            w::Static::plain(content)
        };
        Ok(Box::new(RichHost::new(Box::new(inner), kw(k, "rich")?)))
    });
    r.kind("Label", |_, k, _| {
        let content: String = kw(k, "content")?.unwrap_or_default();
        let inner = if kw::<bool>(k, "markup")?.unwrap_or(true) {
            w::Label::new(content)
        } else {
            w::Label::plain(content)
        };
        Ok(Box::new(RichHost::new(Box::new(inner), kw(k, "rich")?)))
    });
    r.kind("Button", |_, k, _| {
        let label: String = kw(k, "label")?.unwrap_or_default();
        let mut b = w::Button::new(label)
            .variant(variant(&kw::<String>(k, "variant")?.unwrap_or_default())?)
            .flat(kw(k, "flat")?.unwrap_or(false));
        if let Some(action) = kw::<String>(k, "action")? {
            b = b.action(&action);
        }
        Ok(Box::new(b))
    });
    r.kind("Input", |_, k, _| {
        let mut i = w::Input::new()
            .value(kw::<String>(k, "value")?.unwrap_or_default())
            .placeholder(kw::<String>(k, "placeholder")?.unwrap_or_default())
            .password(kw(k, "password")?.unwrap_or(false))
            .compact(kw(k, "compact")?.unwrap_or(false));
        if let Some(t) = kw::<String>(k, "type")? {
            i = i.input_type(match t.as_str() {
                "integer" => w::InputType::Integer,
                "number" => w::InputType::Number,
                "text" => w::InputType::Text,
                _ => return Err(PyValueError::new_err(format!("unknown input type {t:?}"))),
            });
        }
        if let Some(n) = kw::<usize>(k, "max_length")? {
            if n > 0 {
                i = i.max_length(n);
            }
        }
        if let Some(chars) = kw::<String>(k, "restrict")? {
            i = i.restrict_chars(&chars);
        }
        if let Some(b) = kw(k, "select_on_focus")? {
            i = i.select_on_focus(b);
        }
        Ok(Box::new(i))
    });
    r.kind("Header", |_, k, _| {
        let mut h = w::Header::new()
            .show_clock(kw(k, "show_clock")?.unwrap_or(false))
            .tall(kw(k, "tall")?.unwrap_or(false));
        if let Some(icon) = kw::<String>(k, "icon")? {
            h = h.icon(&icon);
        }
        Ok(Box::new(h))
    });
    r.kind("Footer", |_, k, _| {
        Ok(Box::new(
            w::Footer::new().compact(kw(k, "compact")?.unwrap_or(false)),
        ))
    });

    r.getter(get).setter(set).caller(call);

    r.message::<button::Pressed>("on_button_pressed", |py, m: &Message| {
        let _p = m.get::<button::Pressed>().expect("Pressed");
        new_py(py, "Button.Pressed", &PyDict::new(py))
    });
    r.message::<input::Changed>("on_input_changed", |py, m: &Message| {
        let p = m.get::<input::Changed>().expect("Changed");
        let d = PyDict::new(py);
        d.set_item("value", &p.value)?;
        new_py(py, "Input.Changed", &d)
    });
    r.message::<input::Submitted>("on_input_submitted", |py, m: &Message| {
        let p = m.get::<input::Submitted>().expect("Submitted");
        let d = PyDict::new(py);
        d.set_item("value", &p.value)?;
        new_py(py, "Input.Submitted", &d)
    });
    r.message::<input::Blurred>("on_input_blurred", |py, m: &Message| {
        let p = m.get::<input::Blurred>().expect("Blurred");
        let d = PyDict::new(py);
        d.set_item("value", &p.value)?;
        new_py(py, "Input.Blurred", &d)
    });
}

fn variant(s: &str) -> PyResult<w::ButtonVariant> {
    Ok(match s {
        "" | "default" => w::ButtonVariant::Default,
        "primary" => w::ButtonVariant::Primary,
        "success" => w::ButtonVariant::Success,
        "warning" => w::ButtonVariant::Warning,
        "error" => w::ButtonVariant::Error,
        _ => {
            return Err(PyValueError::new_err(format!(
                "invalid button variant {s:?}"
            )))
        }
    })
}

fn variant_name(v: w::ButtonVariant) -> &'static str {
    match v {
        w::ButtonVariant::Default => "default",
        w::ButtonVariant::Primary => "primary",
        w::ButtonVariant::Success => "success",
        w::ButtonVariant::Warning => "warning",
        w::ButtonVariant::Error => "error",
    }
}

fn get(py: Python<'_>, wd: &dyn Widget, attr: &str) -> Option<PyResult<Py<PyAny>>> {
    if let Some(s) = as_ref::<w::Static>(wd) {
        return match attr {
            "content" | "renderable" => Some(to_py(py, static_text(s))),
            _ => None,
        };
    }
    if let Some(l) = as_ref::<w::Label>(wd) {
        return match attr {
            "content" | "renderable" => Some(to_py(py, l.plain_text())),
            _ => None,
        };
    }
    if let Some(b) = as_ref::<w::Button>(wd) {
        return match attr {
            "label" => Some(to_py(py, b.label())),
            "variant" => Some(to_py(py, variant_name(b.get_variant()))),
            _ => None,
        };
    }
    if let Some(i) = as_ref::<w::Input>(wd) {
        return match attr {
            "value" => Some(to_py(py, i.get_value())),
            "placeholder" => Some(to_py(py, i.get_placeholder())),
            "password" => Some(to_py(py, i.is_password())),
            "cursor_position" => Some(to_py(py, i.cursor_position())),
            "selected_text" => Some(to_py(py, i.selected_text())),
            "is_valid" => Some(to_py(py, i.is_valid())),
            "selection" => {
                let s = i.selection();
                Some(to_py(py, (s.start, s.end)))
            }
            _ => None,
        };
    }
    if let Some(f) = as_ref::<w::Footer>(wd) {
        return match attr {
            "compact" => Some(to_py(py, f.is_compact())),
            _ => None,
        };
    }
    if let Some(h) = as_ref::<w::Header>(wd) {
        return match attr {
            "tall" => Some(to_py(py, h.is_tall())),
            _ => None,
        };
    }
    None
}

fn static_text(s: &w::Static) -> String {
    match s.content() {
        w::StaticContent::Markup(m) => m.clone(),
        w::StaticContent::Plain(p) => p.clone(),
        w::StaticContent::Text(t) => t.plain_text(),
    }
}

fn set(
    _py: Python<'_>,
    wd: &mut dyn Widget,
    ctx: &mut Ctx,
    attr: &str,
    v: &Bound<'_, PyAny>,
) -> Option<PyResult<()>> {
    if let Some(b) = as_mut::<w::Button>(wd) {
        return match attr {
            "label" => Some(v.extract::<String>().map(|s| b.set_label(s))),
            "variant" => Some((|| {
                let s: String = v.extract()?;
                b.set_variant(variant(&s)?, ctx);
                Ok(())
            })()),
            _ => None,
        };
    }
    if let Some(i) = as_mut::<w::Input>(wd) {
        return match attr {
            "value" => Some(v.extract::<String>().map(|s| i.set_value(s, ctx))),
            "placeholder" => Some(v.extract::<String>().map(|s| i.set_placeholder(s))),
            "password" => Some(v.extract::<bool>().map(|p| i.set_password(p))),
            "cursor_position" => Some(v.extract::<usize>().map(|p| i.set_cursor_position(p))),
            "max_length" => Some(v.extract::<usize>().map(|p| i.set_max_length(p))),
            "selection" => Some((|| {
                let (s, e): (usize, usize) = v.extract()?;
                i.set_selection(w::Selection::new(s, e));
                Ok(())
            })()),
            _ => None,
        };
    }
    if let Some(f) = as_mut::<w::Footer>(wd) {
        return match attr {
            "compact" => Some(v.extract::<bool>().map(|c| f.set_compact(c, ctx))),
            _ => None,
        };
    }
    if let Some(h) = as_mut::<w::Header>(wd) {
        return match attr {
            "tall" => Some(v.extract::<bool>().map(|t| h.set_tall(t, ctx))),
            _ => None,
        };
    }
    None
}

fn call(
    py: Python<'_>,
    wd: &mut dyn Widget,
    ctx: &mut Ctx,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    if let Some(h) = (wd as &mut dyn std::any::Any).downcast_mut::<RichHost>() {
        match method {
            "update_rich" => {
                return Some((|| {
                    h.set_rich(Some(args.get_item(0)?.unbind()));
                    none(py)
                })())
            }
            // A text update replaces any Rich renderable (then falls through to the widget).
            "update" => h.set_rich(None),
            _ => {}
        }
    }
    if let Some(s) = as_mut::<w::Static>(wd) {
        return match method {
            // update(content, markup)
            "update" => Some((|| {
                let content: String = arg(args, 0)?;
                if arg::<bool>(args, 1)? {
                    s.update(content)
                } else {
                    s.update_plain(content)
                }
                none(py)
            })()),
            _ => None,
        };
    }
    if let Some(l) = as_mut::<w::Label>(wd) {
        return match method {
            "update" => Some((|| {
                let content: String = arg(args, 0)?;
                if arg::<bool>(args, 1)? {
                    l.update(content)
                } else {
                    l.update_plain(content)
                }
                none(py)
            })()),
            _ => None,
        };
    }
    if let Some(b) = as_mut::<w::Button>(wd) {
        return match method {
            "press" => Some({
                b.press(ctx);
                none(py)
            }),
            _ => None,
        };
    }
    if let Some(i) = as_mut::<w::Input>(wd) {
        return match method {
            "clear" => Some({
                i.clear(ctx);
                none(py)
            }),
            "insert_text_at_cursor" => Some((|| {
                i.insert_text_at_cursor(&arg::<String>(args, 0)?, ctx);
                none(py)
            })()),
            "insert" => Some((|| {
                i.insert(&arg::<String>(args, 0)?, arg(args, 1)?, ctx);
                none(py)
            })()),
            "delete" => Some((|| {
                i.delete(arg(args, 0)?, arg(args, 1)?, ctx);
                none(py)
            })()),
            "replace" => Some((|| {
                i.replace(&arg::<String>(args, 0)?, arg(args, 1)?, arg(args, 2)?, ctx);
                none(py)
            })()),
            "select_all" => Some({
                i.select_all();
                none(py)
            }),
            "validate" => Some(to_py(py, i.validate(ctx))),
            "action_submit" => Some({
                i.action_submit(ctx);
                none(py)
            }),
            _ => None,
        };
    }
    None
}
