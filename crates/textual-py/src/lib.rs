//! `textual_rs._native`: PyO3 bindings that run Python Textual-style apps on the textual-rs
//! engine. The Python package `textual_rs` (in `python/`) provides the Textual-compatible API
//! on top of these functions; see `docs/python-bindings.md` for the design.

mod adapter;
mod bridge;
mod ops;
mod runner;
mod widgets;

use pyo3::prelude::*;

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add(
        "WrongThreadError",
        py.get_type::<bridge::WrongThreadError>(),
    )?;
    m.add(
        "NoActiveAppError",
        py.get_type::<bridge::NoActiveAppError>(),
    )?;
    m.add_class::<runner::PilotHandle>()?;
    m.add_function(wrap_pyfunction!(runner::run_app, m)?)?;
    m.add_function(wrap_pyfunction!(runner::start_test, m)?)?;
    m.add_function(wrap_pyfunction!(widgets::register_class, m)?)?;
    macro_rules! ops {
        ($($f:ident),* $(,)?) => { $( m.add_function(wrap_pyfunction!(ops::$f, m)?)?; )* };
    }
    ops!(
        query,
        matches,
        node_object,
        type_names,
        is_mounted,
        parent,
        children,
        root,
        screen_of,
        active_screen,
        css_id,
        classes,
        has_class,
        update_classes,
        set_classes,
        set_inline_style,
        set_display,
        computed_style,
        set_disabled,
        is_disabled,
        set_node_text,
        refresh,
        set_loading,
        mount,
        remove,
        remove_children,
        focus,
        blur,
        focus_next,
        focused,
        hovered,
        region,
        app_size,
        scroll_offset,
        scroll_to,
        scroll_visible,
        post_message,
        run_action,
        post_call,
        on_app_thread,
        app_running,
        set_timer,
        cancel_timer,
        exit,
        bell,
        notify,
        set_app_text,
        set_theme,
        theme,
        theme_names,
        copy_to_clipboard,
        clipboard,
        push_screen,
        switch_screen,
        pop_screen,
        screen_stack,
        install_screen,
        uninstall_screen,
        is_screen_installed,
        named_screen,
        switch_mode,
        current_mode,
        maximize,
        widget_get,
        widget_set,
        widget_call,
        add_css,
        has_css,
        report_error,
        take_error,
    );
    Ok(())
}
