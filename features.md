# textual-rs feature status

Parity tracker against Python Textual 8.2.8. Percentages are rough estimates of how much of
Textual's behaviour for that item is implemented here. Status: ✅ done (≥ 90%) · 🟡 partial · ❌ missing.

**Overall: ~90% of commonly used Textual functionality, ~70% of full parity.** 566 tests.

_Last updated: 2026-10-15._

## Core runtime

| Feature | Status | % | Notes |
|---|---|---|---|
| App, compose, mount/remove | ✅ | 95 | `App::new(root)`, `compose![...]`, `.with([...])`, `ctx.mount/remove` |
| Event loop (tokio + crossterm) | ✅ | 90 | raw mode, alt screen, synchronized output, frame diffing |
| Keyboard input & key names | ✅ | 90 | Textual-style names; no kitty keyboard protocol |
| Mouse: click, hover, wheel | ✅ | 90 | `:hover`, Enter/Leave, Click (+ `ctx.click_chain()` for double/triple clicks), wheel scrolls nearest scrollable |
| Mouse capture / drag | ✅ | 85 | `ctx.capture_mouse/release_mouse`; drags go to the MouseDown widget; scrollbar thumb drag + track paging; no `MouseCapture`/`MouseRelease` events |
| Focus chain, tab/shift+tab, auto-focus | ✅ | 90 | `can_focus_children`, focus on click, restore on screen pop |
| Messages (bubbling) | ✅ | 95 | typed payloads, `Flow::Stop`, `sender_id()` |
| Bindings & actions | ✅ | 90 | priority, namespaces `app.`/`screen.`/`focused.`, args; keymaps (`Binding::id`, `App::keymap`, `ctx.set_keymap/update_keymap`, clashing keys dropped); no `check_action` |
| Builtin actions | ✅ | 85 | quit, bell, toggle_dark, set_theme, focus, focus_next/previous, pop_screen, push_screen/switch_screen/switch_mode('name'), maximize, minimize, screenshot, suspend_process, add/remove/toggle_class, notify, scroll_* |
| Timers (`set_timer`, `set_interval`) | ✅ | 95 | |
| Workers (async + blocking) | ✅ | 90 | result posted as message; `run_worker_with`/`run_blocking_with` + `WorkerOptions { name, group, exclusive }` post `worker::StateChanged` (Pending/Running/Success/Error=panic/Cancelled); `cancel_workers(group)`, `WorkerHandle::state()`; errors don't exit the app; plain `run_worker` posts no state events |
| Screens: push/pop/dismiss, modal, switch, named, modes | ✅ | 90 | modal dims the screen below; `ctx.switch_screen`, `App::screen/modal_screen` (installed, kept when popped) + `push_screen_named/switch_screen_named/install_screen`; `App::mode` + `default_mode` + `ctx.switch_mode` (per-mode stacks); no ScreenSuspend/Resume events, no `remove_mode` |
| Notifications (toasts) | ✅ | 90 | `ctx.notify`, severity, timeout, click to dismiss |
| Animation (`animate`, easing, transitions) | ✅ | 90 | all 33 easings, `ctx.animate_value`, `animate_style` (opacity/offset/width/height), `animate_color` (background/color/tint/background-tint, `Color::blend`), `AnimateOptions` (`delay`, `speed`, easing, level) via `*_opts`, smooth scroll, animation levels, CSS transitions, Tabs/Switch animate; delayed style animations read their start value at request time |
| Tooltips | ✅ | 85 | `Child::tooltip`, `ctx.set_tooltip`; 0.5s delay, ancestor inheritance, flip above / keep on screen; no Rich renderables, no `_disable_tooltips` |
| Command palette | ✅ | 80 | `ctrl+p`, fuzzy `Matcher` port, `CommandProvider` / `App::command_provider` / `Widget::commands`, system commands, theme picker, header icon; synchronous providers (no async search / busy indicator) |
| Text selection & copy (screen selection) | ❌ | 0 | Input has its own selection + internal clipboard |
| Clipboard (OSC 52) | ✅ | 90 | `ctx.copy_to_clipboard` queues OSC 52, flushed after the next frame; Input/TextArea copy/cut use it; paste reads the app clipboard; `pilot.clipboard()` |
| `@click` markup links | 🟡 | 75 | click runs the action with the widget as owner; no `link-*-hover` styles, no `@mouse.down`/`@mouse.up` |
| Screenshots (SVG / ANSI / text export) | ✅ | 90 | `canvas.to_svg()`, `to_ansi()`, `pilot.export_svg()`, `TEXTUAL_SCREENSHOT=out.svg` env mode (+ `TEXTUAL_SIZE`, `TEXTUAL_PRESS`); `screenshot(filename?, path?)` action + "Screenshot" palette command (CWD, Textual filename scheme, UTC timestamp), `ctx.save_screenshot` |
| Maximize, `loading` overlay, `disabled` | ✅ | 85 | `ctx.maximize/maximize_exact/minimize`, `maximize`/`minimize` actions + palette commands, escape minimizes, `Widget::allow_maximize`, Footer/toasts kept; `ctx.set_loading(id, bool)` covers with a LoadingIndicator (blocks input, leaves focus chain); no hatch background, no custom loading widget |
| Suspend (`ctrl+z`), inline mode, no-color / ANSI themes | 🟡 | 30 | `suspend_process` action / `ctx.suspend_process` (unix SIGTSTP via `kill`, terminal restored + full redraw on resume; not bound by default, like Textual 8); no inline mode, no ANSI themes |
| Devtools console / logging | ❌ | 0 | |
| Testing: Pilot | ✅ | 90 | press, type, click, hover, scroll, resize, wait (paused time), query, canvas |
| Snapshot testing (SVG compare) | ❌ | 0 | text snapshots via `screen_text()` |

## CSS (TCSS)

| Feature | Status | % | Notes |
|---|---|---|---|
| Tokenizer/parser, nesting `&`, `!important`, comments | ✅ | 95 | real Textual DEFAULT_CSS from 102 widgets parses cleanly |
| Selectors: type/class/id/universal, `>` and descendant, lists | ✅ | 95 | type selectors match "subclass" type names |
| Pseudo-classes | ✅ | 85 | focus, focus-within, hover, disabled, enabled, dark, light, first/last-child, even/odd, empty, can-focus; no `:inline`, `:ansi`, `:nocolor` |
| Variables (`$var`, theme vars) | ✅ | 100 | |
| Themes & design system | ✅ | 95 | 19 built-in themes; variables match Python's output within ±2 per channel; no ANSI themes |
| Cascade (specificity, origin, important) | ✅ | 95 | |
| Box model: width/height/min/max, margin, padding, box-sizing | ✅ | 95 | |
| Units: cells, %, fr, w, h, vw, vh, auto | ✅ | 95 | |
| Colors: names, hex, rgb/hsl, alpha `%`, `auto` contrast | ✅ | 90 | `ansi_default` → transparent |
| Borders (19 kinds), titles/subtitles, outline | ✅ | 95 | |
| Text: color, text-style, text-align, text-wrap, text-overflow, text-opacity | ✅ | 90 | |
| background, background-tint, tint, opacity | ✅ | 85 | |
| content-align, align | ✅ | 95 | |
| dock, layers, layer, offset, position: absolute | ✅ | 85 | |
| overflow, scrollbar-size, scrollbar colors, scrollbar-gutter | ✅ | 95 | hover/active colors, ⅛-cell thumb ends (`ScrollBarRender`), theme scrollbar variables |
| Grid: grid-size, grid-columns/rows, gutter, spans | ✅ | 90 | |
| CSS `transition` | ✅ | 90 | `transition: <prop> <duration> [<easing>] [<delay>], ...` → `Styles::transitions`; runtime overlay in `transition.rs` (deterministic under Pilot `wait`, skipped below `AnimationLevel::Full`); opacity, text-opacity, offset, width/height, min/max-*, margin, padding, color, background, tint, background-tint, border(-edge) colors, scrollbar-*, link-* colors; `_`/`-` both accepted in names; mismatched units resolve to cells (`auto` → cells from the layout; to `auto`/`fr` jumps); inline-styled properties are not transitioned |
| hatch, keyline, pointer, constrain, overlay, expand, line-pad | ❌ | 0 | parsed and ignored |
| CSS hot reload / `CSS_PATH` file watching | 🟡 | 30 | `App::css_path` loads once |

## Layout & rendering

| Feature | Status | % | Notes |
|---|---|---|---|
| Vertical / horizontal / grid layouts | ✅ | 95 | port of Textual's `layouts/*` |
| Stream layout | ❌ | 0 | falls back to vertical |
| Scrolling: containers, self-scrolling (ScrollView) | ✅ | 90 | keyboard / scrollbar-track scrolling animates (out_cubic, 50 cells/s); wheel and drag are immediate |
| Compositor: clipping, layers, opacity, modal dimming | ✅ | 90 | full-frame paint + diff (~2 ms per frame at 1000 widgets, release build) |
| Content markup (`[b red on $panel]`, `[/]`, `\[`) | ✅ | 90 | |
| Unicode cell widths (CJK, emoji, ZWJ) | ✅ | 90 | |
| Rich renderables (tables, syntax, pretty) | ❌ | 5 | no Rich equivalent beyond styled text |

## Widgets

| Widget | Status | % | Notes |
|---|---|---|---|
| Static | ✅ | 95 | markup / plain / Text content, `update` |
| Label | ✅ | 95 | variants via classes |
| Button | ✅ | 95 | variants, flat, action, active effect |
| Input | ✅ | 85 | password, restrict (char predicate), max length, integer/number, selection; no suggester/validators API |
| MaskedInput | ✅ | 90 | full template engine (`A a N n X x 9 0 D d # H h B b`, case, escapes, blank char), validity classes; no `validators=` |
| TextArea | 🟡 | 80 | editing, selection (keys/mouse drag/double+triple click), undo/redo batching, soft wrap, line numbers, themes (css/monokai/dracula/vscode_dark/github_light), bracket matching, read-only, placeholder; no tree-sitter (simple per-line keyword highlighter for python/rust/js/json/toml), no suggestions; copy/cut use the OSC 52 clipboard |
| Checkbox / ToggleButton | ✅ | 95 | |
| RadioButton / RadioSet | ✅ | 95 | |
| Switch | ✅ | 95 | slides over 0.3s |
| Select | ✅ | 85 | dropdown via a transparent modal screen; type-to-search |
| SelectionList | ✅ | 90 | generic values, toggle/select/deselect (all), typed messages; no separators |
| OptionList | ✅ | 90 | |
| ListView / ListItem | ✅ | 90 | |
| DataTable | ✅ | 88 | cursors, sorting, fixed rows/cols, zebra, hover; per-cell alignment (`CellValue::right/center/justify`; numbers left by default, as Textual) |
| Tree | ✅ | 90 | generic data payload, guides, keyboard + mouse |
| DirectoryTree | 🟡 | 85 | lazy loading, sorting, icons, filter predicate, reload keeping expansion; synchronous I/O |
| Tabs / Tab | ✅ | 95 | underline slides over 0.3s |
| TabbedContent / TabPane | ✅ | 90 | |
| ContentSwitcher | ✅ | 95 | |
| Collapsible | ✅ | 95 | |
| Header | ✅ | 90 | clock in UTC (offset configurable); icon opens the command palette |
| Footer | ✅ | 90 | click replays the key; binding groups (`Binding::group(BindingGroup)`, compact groups) |
| Log | ✅ | 90 | |
| RichLog | ✅ | 90 | `highlight(true)`: Rich `ReprHighlighter` port (`pretty::tokenize_text`), ANSI-theme colors at write time; no custom highlighter object |
| ProgressBar | ✅ | 95 | ETA, indeterminate, gradients |
| LoadingIndicator | ✅ | 95 | used by the `loading` overlay |
| Sparkline | ✅ | 85 | no gradient |
| Digits | ✅ | 95 | |
| Placeholder | ✅ | 95 | |
| Rule | ✅ | 100 | |
| Toast / ToastRack | ✅ | 90 | |
| Containers (Container, Vertical, Horizontal, Grid, Center, Middle, ScrollableContainer, ...) | ✅ | 95 | |
| Markdown / MarkdownViewer | 🟡 | 85 | pulldown-cmark (GFM tables, strikethrough, task lists); all block types, inline styles, clickable links, TOC + goto_anchor, viewer with TOC tree and file history; fences highlighted by default with the TextArea keyword highlighter (python/rust/js/json/toml; monokai dark / github_light light; replaceable hook), `open_links` (default on: scheme links via `open_url`); no streaming / incremental append |
| Link | ✅ | 90 | opener stubbable via `link::set_url_opener`; posts `link::Opened` |
| Pretty | 🟡 | 75 | `Debug` repr, one-line or expanded, hand-written repr highlighter; all-or-nothing expansion, no indent guides |
| Tooltip | ✅ | 90 | runtime-positioned (no `constrain` CSS) |
| KeyPanel / HelpPanel | ✅ | 85 | `dock: right` instead of `split`; HelpPanel shows `Widget::help` Markdown (nearest ancestor) + embedded KeyPanel, `app.show/hide_help_panel`; help refresh lands one loop turn after focus changes (no focus watcher) |
| Welcome | 🟡 | 85 | welcome text built from Statics (not the Markdown widget); OK exits |

## Python bindings (`textual_rs`)

Textual-compatible Python API on the Rust engine (`crates/textual-py`, PyO3 + maturin + uv).
See [`docs/python-bindings.md`](docs/python-bindings.md).

| Feature | Status | % | Notes |
|---|---|---|---|
| App lifecycle | ✅ | 90 | `run()` with GIL released (returns `exit()` value), `run_async`, `run_test` + `Pilot`; handler exceptions exit cleanly and re-raise |
| Widget API | ✅ | 85 | compose (incl. `with` blocks), render (markup / Content / Rich renderables), DEFAULT_CSS, BINDINGS, queries, classes, styles (write), display/visible/disabled/loading, mount/remove/recompose, focus, scrolling, timers, `animate()`; no `render_line` |
| Reactivity | ✅ | 90 | reactive/var with watch/validate/compute, init, always_update, layout, recompose, toggle_class, `watch()` |
| Messages & events | ✅ | 90 | custom messages, bubbling, stop/prevent_default, `@on` with selectors, MRO dispatch, `key_*` methods, mouse/focus/enter/leave/resize/paste |
| Screens & modes | ✅ | 85 | Screen/ModalScreen, dismiss + callback, `push_screen_wait`, SCREENS/MODES, screen CSS |
| Workers & async | ✅ | 90 | async handlers/workers on an app-thread asyncio loop; thread workers; widget calls from threads auto-marshalled |
| Themes, validation | ✅ | 90 | `Theme`/`register_theme`; `textual_rs.validation`; Input `restrict` regex |
| Built-in widgets | 🟡 | 20 | Static, Label, Button, Input, Header, Footer, containers bound; remaining widgets in progress |
| Not yet | ❌ | 0 | `check_action`, command palette providers, devtools/`log`, `App.suspend`, inline mode |

## Roadmap

See [`ToDo.md`](ToDo.md) for the prioritized backlog.
