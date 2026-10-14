# Writing widgets for textual-rs

This is how Textual widget concepts map to textual-rs. Read `crates/textual/src/widget.rs`,
`context.rs`, and `widgets/static_.rs` / `widgets/containers.rs` for working examples.

## Anatomy

```rust
// crates/textual/src/widgets/button.rs
use crate::prelude::*;          // Widget, Ctx, Content, RenderCtx, Event, Flow, Message, Binding, ...

/// Posted when the button is activated (Textual: `Button.Pressed`).
#[derive(Debug, Clone, PartialEq)]
pub struct Pressed {
    /// The pressed button's CSS id, if any.
    pub button_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Button {
    label: String,
    variant: ButtonVariant,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self { ... }
    pub fn variant(mut self, v: ButtonVariant) -> Self { self.variant = v; self }   // builder style
    pub fn label(&self) -> &str { ... }
    pub fn set_label(&mut self, label: impl Into<String>) { ... }
}

impl Widget for Button {
    fn type_names(&self) -> &'static [&'static str] { &["Button"] }   // CSS type, + parents if subclassing
    fn default_css(&self) -> &'static str { DEFAULT_CSS }              // ported from Textual's DEFAULT_CSS
    fn can_focus(&self) -> bool { true }
    fn bindings(&self) -> Vec<Binding> { vec![Binding::new("enter", "press", "Press button")] }
    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        match action { "press" => { self.press(ctx); true } _ => false }
    }
    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event { self.press(ctx); return Flow::Stop; }
        Flow::Continue
    }
    fn render(&self, ctx: &RenderCtx) -> Content { Content::Text(ctx.markup(&self.label)) }
}
```

Register the module in `widgets/mod.rs`: `pub mod button; pub use button::{Button, ButtonVariant};`
Users then match messages as `msg.get::<button::Pressed>()`.

## Mapping Textual → textual-rs

| Textual | textual-rs |
|---|---|
| `DEFAULT_CSS` | `fn default_css()` — port verbatim where possible (TCSS is the same language) |
| `COMPONENT_CLASSES` + `get_component_rich_style("x--y")` | `ctx.component_style("x--y")` in `render` (rules like `Widget > .x--y { ... }` in default CSS) |
| `BINDINGS` / `action_*` | `fn bindings()` / `fn run_action(name, args, ctx) -> bool` |
| `compose()` yielding widgets | `fn compose(&mut self) -> Vec<Child>` with `compose![...]`, `.id()`, `.classes()`, `.with([...])` |
| `on_mount` | `fn on_mount(&mut self, ctx)` (children are mounted before their parent's on_mount) |
| `on_key`, `on_click`, `on_focus`, ... | `fn on_event(&mut self, &Event, ctx) -> Flow` (`Flow::Stop` = `event.stop()`/`prevent_default`) |
| `on_<widget>_<message>` / `@on` | `fn on_message(&mut self, &Message, ctx) -> Flow`, `msg.get::<T>()`, `msg.sender_id()` |
| `self.post_message(X())` | `ctx.post_message(X { .. })` (handled by self first, then bubbles to ancestors) |
| reactive attribute + `watch_x` | plain field + setter that does the side effects; the runtime repaints after any handler |
| `self.refresh(layout=True)` | not needed — every frame restyles + relayouts; `ctx.refresh()` exists for timers/workers |
| `self.query_one("#x", Label).update(..)` | `ctx.query_one::<Label>("#x").unwrap().update(..)`; with a ctx: `ctx.with::<Input, _>("#x", |w, ctx| ...)` |
| `self.set_interval(1, cb)` | `ctx.set_interval(Duration, |w: &mut Self, ctx| ...)` |
| `@work` / workers | `ctx.run_worker(async { Msg })` → `Msg` arrives in `on_message` |
| `self.add_class` / `set_class` | `ctx.add_class`, `ctx.set_class(on, "x")`; other nodes: `ctx.add_class_to(id, ..)` |
| `self.styles.x = ..` | `ctx.set_styles(|s| s.width = Some(Scalar::cells(10.0)))` |
| `self.mount(...)` / `remove()` | `ctx.mount(child)`, `ctx.mount_to(parent, child, index)`, `ctx.remove(id)` |
| `self.focus()` | `ctx.focus_self()` |
| `self.app.push_screen(S())` / `dismiss(r)` | `ctx.push_screen(S)`, `ctx.push_modal(S)`, `ctx.dismiss(r)` (pusher gets `r` as a message) |
| `ScrollView` (virtual size, render_line) | `fn virtual_size(&self, ctx) -> Option<Size>`; `render` uses `ctx.scroll_offset` & `ctx.size`; scroll with `ctx.scroll_to/scroll_by/scroll_to_show(region)` |
| `self.notify(...)` | `ctx.notify(..)` / `ctx.notify_with(..)` |

## Rendering

- `Content::Text(Text)` — the compositor wraps it to the content width and applies `text-align`,
  `content-align`, `text-wrap`, `text-overflow`. Use for label-like widgets.
- `Content::Lines(Vec<Strip>)` — exact lines (one `Strip` per row), drawn from the content
  region's top-left. Use when the widget draws its own layout (tables, lists, bars, input fields).
  Pad/crop strips with `Strip::set_cell_len(width, &style)`; `ctx.size` is the content size.
- Base style (color/background/text-style from CSS) is applied under your spans automatically.
  Span styles with `bg: None` keep the widget background.
- Theme colors: `ctx.var_color("primary")`, markup `[$accent]x[/]`, or (best) CSS rules.
- State-dependent styling: prefer CSS pseudo-classes (`:focus`, `:hover`, `:disabled`) and classes
  (`ctx.set_class(self.value, "-on")`) over hard-coded colors.

## Sizing

- `height: auto` widgets: implement `content_height(&self, ctx, width)` if the default measurement
  (rendering and counting lines) is wrong or expensive. `width: auto`: `content_width`.

## Testing

Write tests in `crates/textual/tests/widget_<name>.rs` using the Pilot:

```rust
#[tokio::test]
async fn button_press_posts_message() {
    struct T { pressed: Vec<Option<String>> }
    impl Widget for T {
        fn compose(&mut self) -> Vec<Child> { compose![Button::new("OK").id("ok")] }
        fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
            if let Some(p) = m.get::<button::Pressed>() { self.pressed.push(p.button_id.clone()); }
            Flow::Continue
        }
    }
    let mut pilot = App::new(T { pressed: vec![] }).test(Size::new(40, 10)).await;
    pilot.click("#ok").await;
    assert_eq!(pilot.app::<T>().unwrap().pressed, vec![Some("ok".into())]);
    assert!(pilot.screen_text().contains("OK"));
}
```

Pilot API: `press(&[keys])`, `type_text(s)`, `click(selector)`, `click_at(x, y)`, `hover`,
`scroll_down_at`, `resize`, `pause`, `wait(dur)`, `wait_for_signal`, `query_one::<W>(sel)`,
`with::<W, R>(sel, |w, ctx| ..)`, `app::<Root>()`, `focused_id()`, `screen_text()`, `canvas()`
(cell-level: `canvas.get(x, y).unwrap().bg`).

Also check `pilot.css_errors()` is empty in at least one test per widget (your DEFAULT_CSS must parse).

## Visual QA

Render any example to an image and look at it:

```sh
TEXTUAL_SCREENSHOT=/tmp/shot.svg TEXTUAL_SIZE=100x30 TEXTUAL_PRESS=tab,down,enter \
    cargo run -q -p textual --example table
rsvg-convert /tmp/shot.svg -o /tmp/shot.png   # then view the PNG
```

In tests: `pilot.export_svg()` / `pilot.export_ansi()`.
