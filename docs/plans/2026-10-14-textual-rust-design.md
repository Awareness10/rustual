# textual-rs — Design

A Rust re-creation of Python [Textual](https://textual.textualize.io) (reference: Textual 8.2.8 and
Rich 15 sources unpacked in `reference/textual-src/textual` and `reference/rich-src/rich`).

## Decisions

| Topic | Decision |
|---|---|
| Scope (milestone 1) | Core runtime + TCSS + layout + compositor + ~14 widgets, examples, tests |
| API style | Idiomatic Rust expressing Textual concepts (traits, enums, arena DOM) |
| Runtime | tokio + crossterm (`event-stream`). Handlers are **sync**; async work runs in workers (`ctx.run_worker(future)`) whose output is posted back as a message |
| Rendering | Full-frame compositing into a cell `Canvas`, diffed against the previous frame |
| Restyle/relayout | Whenever anything is dirty: restyle all → layout → paint → diff. Simple first; optimize later |

## Crates

```
crates/textual-render   geometry, Color, Style, cells, Text + markup, Strip, BorderKind glyphs, Canvas (+diff)
crates/textual-css      tokenizer, parser (vars, nesting), selectors/specificity, Styles (typed), Stylesheet cascade, Theme
crates/textual          Dom, Widget trait, events, messages, bindings/actions, Ctx, layout, compositor,
                        driver (crossterm + headless), App runtime, Pilot (testing), widgets/, examples/
```

Dependency direction: `textual` → `textual-css` → `textual-render`.

## Core model

- **Dom** (`dom.rs`): `SlotMap<NodeId, Node>`. A `Node` owns `Box<dyn Widget>`, CSS id, classes, type names,
  inline + computed `Styles`, `NodeLayout`, scroll offset. Tree: `App root → Screen(s) → widgets`.
  `NodeRef` implements `textual_css::SelectorNode` (pseudo-classes: focus, hover, disabled, dark, first-child, ...).
- **Widget** (`widget.rs`): trait with defaulted methods: `type_names`, `default_css`, `compose -> Vec<Child>`,
  `on_mount`, `on_event -> Flow`, `on_message -> Flow`, `render(&RenderCtx) -> Content`, `content_width/height`,
  `virtual_size` (self-scrolling), `can_focus`, `bindings`, `run_action`. Downcasting via trait upcasting to `dyn Any`.
- **Child**: widget + mount attributes; `Button::new("OK").id("ok").classes("primary")`, containers via `.with([...])`,
  `compose![a, b, c]` macro.
- **Handlers & Ctx**: during a handler the widget is *checked out* of its node (`Option::take`), so `Ctx` has
  `&mut AppState` and can access every other widget (`ctx.query_one::<Label>("#status")`). Any handler
  invocation marks the app dirty.
- **Messages**: `Message { sender, sender_css_id, payload: Box<dyn Any + Send> }`. `ctx.post_message(T)` delivers
  to the posting widget first and bubbles up to the app root until a handler returns `Flow::Stop`.
- **Events**: `Event` enum (Key, Mouse*, Click, Enter/Leave, Focus/Blur, Resize, Paste). Key dispatch: priority
  bindings → focused widget (bubbling) → bindings along focus chain (focused → ancestors → screen → app) → builtins
  (tab/shift+tab focus cycling). Mouse events go to the topmost widget under the pointer and bubble.
- **Actions**: strings like `"quit"`, `"app.toggle_dark"`, `"set_theme('nord')"`. Run on the binding owner, or
  `app.`/`screen.` namespaces; builtin app actions: quit, bell, toggle_dark, focus_next, focus_previous, pop_screen,
  set_theme(name).
- **Screens**: root children; stack; modal screens paint over the screen below. `push_screen`, `pop_screen`,
  `dismiss(result)` posts `ScreenDismissed` to the pusher.
- **Timers / workers**: `ctx.set_interval(d, |w: &mut W, ctx| ..)`, `ctx.set_timer(..)`; `ctx.run_worker(async { Msg })`.
  `AppHandle` (Send + Clone) posts messages from anywhere.

## Pipeline per frame

1. **Styles**: for every node: `stylesheet.compute(node_ref)` → merge inline styles → `inherit_from(parent)`.
2. **Layout** (`layout/`): `layout_screen(dom, env, screen, size) -> Arrangement` (paint order). Box model per Textual.
3. **Paint** (`compositor.rs`): background, border + titles, content (`Content::Text` wrapped & aligned, or
   `Content::Lines`), scrollbars, outline, opacity → `Canvas`.
4. **Diff**: `canvas.diff(prev)` → driver writes changed runs with crossterm.

## CSS

TCSS as in Textual: type/class/id/universal selectors, pseudo-classes, descendant and child combinators, selector
lists, nesting with `&`, `$variables` (theme + source-defined), `!important`. Origin precedence: widget DEFAULT_CSS
< app CSS (DEFAULT loses regardless of specificity, like Textual). Theme generates the full variable set from
`design.py` (`$primary`, `$surface`, `$panel`, `$boost`, `$text-muted`, `$primary-lighten-2`, `$footer-*`, ...).
Colors accept `<color> <alpha>%`.

## Testing

- Unit tests per module (`cargo test -p <crate>`).
- `Pilot` drives a headless app: `let mut pilot = App::new(MyApp::default()).css(CSS).test(Size::new(80, 24)).await;`
  `pilot.press(&["tab", "enter"]).await; pilot.click("#ok").await; assert!(pilot.screen_text().contains("..."));`
- Snapshot-style tests compare `Canvas::to_text()` output.

## Conventions for contributors (incl. subagents)

- Do not change existing public signatures in contract files without the architect's approval; adding items is fine.
- Port behaviour from the reference sources where in doubt; cite the Textual file in a comment for non-obvious rules.
- Keep `cargo test` and `cargo clippy` clean for the crate you own. No `unsafe`.
