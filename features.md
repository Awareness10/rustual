# rustual feature status

Parity tracker against Python Textual 8.2.8. Percentages are rough estimates of how much of
Textual's behaviour for that item is implemented here. Status: ✅ done (≥ 90%) · 🟡 partial · ❌ missing.

**Overall: ~90% of commonly used Textual functionality, ~70% of full parity.** Test counts and coverage: see the badges in the [README](README.md#testing) (updated by CI). CI tests Linux, macOS and Windows; Python wheels for all three are on [PyPI](https://pypi.org/project/rustual-py/) and [GitHub releases](https://github.com/Awareness10/rustual/releases).

_Last updated: 2026-10-17._

## Core runtime

| Feature | Status | % | Notes |
|---|---|---|---|
| App, compose, mount/remove | ✅ | 95 | `App::new(root)`, `compose![...]`, `.with([...])`, `ctx.mount/remove` |
| Event loop (tokio + crossterm) | ✅ | 90 | raw mode, alt screen, synchronized output, frame diffing |
| Keyboard input & key names | ✅ | 90 | Textual-style names; bindings may use Textual's character names (`slash`, `question_mark`, ...), which match the typed character; no kitty keyboard protocol |
| Mouse: click, hover, wheel | ✅ | 90 | `:hover`, Enter/Leave, Click (+ `ctx.click_chain()` for double/triple clicks), wheel scrolls nearest scrollable |
| Mouse capture / drag | ✅ | 85 | `ctx.capture_mouse/release_mouse`; drags go to the MouseDown widget; scrollbar thumb drag + track paging; no `MouseCapture`/`MouseRelease` events |
| Focus chain, tab/shift+tab, auto-focus | ✅ | 90 | `can_focus_children`, focus on click, restore on screen pop |
| Messages (bubbling) | ✅ | 95 | typed payloads, `Flow::Stop`, `sender_id()` |
| Bindings & actions | ✅ | 90 | priority, namespaces `app.`/`screen.`/`focused.`, args; keymaps (`Binding::id`, `App::keymap`, `ctx.set_keymap/update_keymap`, clashing keys dropped); runtime bindings (`ctx.bind`, Textual `bind`); `check_action` (hide / disable bindings, footer dims); `check_consume_key` (a focused input hides / skips printable-key bindings); modal screens stop the chain (only priority bindings reach the app) |
| Builtin actions | ✅ | 85 | quit, bell, toggle_dark, set_theme, focus, focus_next/previous, pop_screen, push_screen/switch_screen/switch_mode('name'), maximize, minimize, screenshot, suspend_process, add/remove/toggle_class, notify, scroll_* |
| Timers (`set_timer`, `set_interval`) | ✅ | 95 | |
| Workers (async + blocking) | ✅ | 90 | result posted as message; `run_worker_with`/`run_blocking_with` + `WorkerOptions { name, group, exclusive }` post `worker::StateChanged` (Pending/Running/Success/Error=panic/Cancelled); `cancel_workers(group)`, `WorkerHandle::state()`; errors don't exit the app; plain `run_worker` posts no state events |
| Screens: push/pop/dismiss, modal, switch, named, modes | ✅ | 90 | modal dims the screen below; `ctx.switch_screen`, `App::screen/modal_screen` (installed, kept when popped) + `push_screen_named/switch_screen_named/install_screen`; `App::mode` + `default_mode` + `ctx.switch_mode` (per-mode stacks); `ScreenSuspend` / `ScreenResume` events; per-screen titles (`ctx.set_screen_title`); no `remove_mode` |
| Notifications (toasts) | ✅ | 90 | `ctx.notify`, severity, timeout, click to dismiss |
| Animation (`animate`, easing, transitions) | ✅ | 90 | all 33 easings, `ctx.animate_value`, `animate_style` (opacity/offset/width/height), `animate_color` (background/color/tint/background-tint, `Color::blend`), `AnimateOptions` (`delay`, `speed`, easing, level) via `*_opts`, smooth scroll, animation levels, CSS transitions, Tabs/Switch animate; delayed style animations read their start value at request time |
| Tooltips | ✅ | 85 | `Child::tooltip`, `ctx.set_tooltip`; 0.5s delay, ancestor inheritance, flip above / keep on screen; markup content only (no renderables), no `_disable_tooltips` |
| Command palette | ✅ | 90 | `ctrl+p`, fuzzy `Matcher` port (Textual's score scale), `CommandProvider` / `App::command_provider` / `Widget::commands`, system commands (`App::system_commands` to replace them), theme picker, header icon; search hooks (`App::command_search`) deliver scored / discovery hits (synchronously or later), merged by score as they arrive (stale queries dropped), which is how Python's async `Provider`s run; no busy indicator, "No matches" shows without Textual's delay |
| Text selection & copy (screen selection) | ❌ | 0 | Input has its own selection + internal clipboard |
| Clipboard (OSC 52) | ✅ | 90 | `ctx.copy_to_clipboard` queues OSC 52, flushed after the next frame; Input/TextArea copy/cut use it; paste reads the app clipboard (falling back to a process-wide one); `pilot.clipboard()` |
| `@click` markup links | 🟡 | 75 | click runs the action with the widget as owner; no `link-*-hover` styles, no `@mouse.down`/`@mouse.up` |
| Screenshots (SVG / ANSI / text export) | ✅ | 90 | `canvas.to_svg()`, `to_ansi()`, `pilot.export_svg()`, `TEXTUAL_SCREENSHOT=out.svg` env mode (+ `TEXTUAL_SIZE`, `TEXTUAL_PRESS`); `screenshot(filename?, path?)` action + "Screenshot" palette command (CWD, Textual filename scheme, UTC timestamp), `ctx.save_screenshot` |
| Maximize, `loading` overlay, `disabled` | ✅ | 85 | `ctx.maximize/maximize_exact/minimize`, `maximize`/`minimize` actions + palette commands, escape minimizes, `Widget::allow_maximize`, Footer/toasts kept, hatched background around the maximized widget; `ctx.set_loading(id, bool)` covers with a LoadingIndicator (blocks input, leaves focus chain); no custom loading widget |
| Suspend (`ctrl+z`), inline mode | 🟡 | 50 | `suspend_process` action / `ctx.suspend_process` (unix SIGTSTP via `kill`, terminal restored + full redraw on resume; not bound by default, like Textual 8); no inline mode |
| ANSI themes, `ansi_color`, `NO_COLOR` | ✅ | 90 | `ansi-dark` / `ansi-light` (variables identical to Textual's `_generate_ansi`); `ansi_*` / `ansi_default` colors are written as terminal ANSI codes when native (`App::ansi_color`, else the theme's `ansi`; `ctx.set_ansi_color`), otherwise converted with Textual's MONOKAI / ALABASTER terminal themes, with `dim` turned into a blended color (`ANSIToTruecolor`); `NO_COLOR` gives default colors (native) or a gray scale (`NoColor` / `Monochrome`); widgets' `:ansi` rules ported; screenshots use Rich's SVG export theme; no `TEXTUAL_DIM_FACTOR` / `TEXTUAL_FILTERS` env vars |
| Devtools console / logging | ❌ | 0 | |
| Testing: Pilot | ✅ | 90 | press, type, click, hover, scroll, resize, wait (paused time), query, canvas; Python `Pilot` methods wait for running animations, like Textual's |
| Snapshot testing (SVG compare) | ❌ | 0 | text snapshots via `screen_text()` |

## CSS (TCSS)

| Feature | Status | % | Notes |
|---|---|---|---|
| Tokenizer/parser, nesting `&`, `!important`, comments | ✅ | 95 | real Textual DEFAULT_CSS from 102 widgets parses cleanly |
| Selectors: type/class/id/universal, `>` and descendant, lists | ✅ | 95 | type selectors match "subclass" type names |
| Pseudo-classes | ✅ | 90 | focus, focus-within, hover, disabled, enabled, dark, light, ansi, nocolor, first/last-child, even/odd, empty, can-focus; no `:inline` |
| Variables (`$var`, theme vars) | ✅ | 100 | |
| Themes & design system | ✅ | 95 | 22 built-in themes, including `ansi-dark` / `ansi-light`; variables match Python's output within ±2 per channel (ANSI themes exactly) |
| Cascade (specificity, origin, important) | ✅ | 95 | |
| Box model: width/height/min/max, margin, padding, box-sizing | ✅ | 95 | |
| Units: cells, %, fr, w, h, vw, vh, auto | ✅ | 95 | |
| Colors: names, hex, rgb/hsl, alpha `%`, `auto` contrast | ✅ | 95 | `ansi_*` / `ansi_default` are the terminal's own colors (see ANSI themes): opaque, alpha ignored, they win when composited |
| Borders (19 kinds), titles/subtitles, outline | ✅ | 95 | |
| Text: color, text-style, text-align, text-wrap, text-overflow, text-opacity | ✅ | 90 | `dim` is drawn as a color blended 66% towards the background unless ANSI colors are native (as Textual) |
| background, background-tint, tint, opacity | ✅ | 85 | |
| content-align, align | ✅ | 95 | |
| dock, layers, layer, offset, position: absolute | ✅ | 85 | |
| overflow, scrollbar-size, scrollbar colors, scrollbar-gutter | ✅ | 95 | hover/active colors, ⅛-cell thumb ends (`ScrollBarRender`), theme scrollbar variables |
| Grid: grid-size, grid-columns/rows, gutter, spans | ✅ | 90 | |
| CSS `transition` | ✅ | 90 | `transition: <prop> <duration> [<easing>] [<delay>], ...` → `Styles::transitions`; runtime overlay in `transition.rs` (deterministic under Pilot `wait`, skipped below `AnimationLevel::Full`); opacity, text-opacity, offset, width/height, min/max-*, margin, padding, color, background, tint, background-tint, border(-edge) colors, scrollbar-*, link-* colors; `_`/`-` both accepted in names; mismatched units resolve to cells (`auto` → cells from the layout; to `auto`/`fr` jumps); inline-styled properties are not transitioned |
| `hatch` | ✅ | 100 | `hatch: <left\|right\|cross\|horizontal\|vertical\|"c"> <color> [<alpha>%]` or `none`: blank cells of the padding box (including spaces between words) drawn as the hatch character in `background + color`; children cover it; maximized views are hatched (`Screen.-maximized-view`); Python `styles.hatch = ("right", "red")` |
| keyline, pointer, constrain, overlay, expand, line-pad | ❌ | 0 | parsed and ignored |
| CSS hot reload / `CSS_PATH` file watching | 🟡 | 30 | `App::css_path` loads once |

## Layout & rendering

| Feature | Status | % | Notes |
|---|---|---|---|
| Vertical / horizontal / grid layouts | ✅ | 95 | port of Textual's `layouts/*` |
| Breakpoints | ✅ | 100 | `App::horizontal_breakpoints` / `vertical_breakpoints` (Textual `HORIZONTAL_BREAKPOINTS` / `VERTICAL_BREAKPOINTS`), per-screen overrides (`ctx.set_screen_breakpoints`); classes follow the terminal size |
| Stream layout | ✅ | 95 | `layout: stream` (port of `layouts/stream.py`): full-width, auto-height children, collapsing vertical margins, only cell `min-height` / `max-height`, no offset / absolute; child regions match Textual 8.2.8 exactly in side-by-side tests; Textual's between-frame placement cache isn't ported (rustual lays out every frame, so stream is only ~3% faster than vertical here) |
| Scrolling: containers, self-scrolling (ScrollView) | ✅ | 90 | keyboard / scrollbar-track scrolling animates (out_cubic, 50 cells/s); wheel and drag are immediate |
| Compositor: clipping, layers, opacity, modal dimming | ✅ | 90 | full-frame paint + diff (~2 ms per frame at 1000 widgets, release build) |
| Content markup (`[b red on $panel]`, `[/]`, `\[`) | ✅ | 90 | tags with unknown style words (`[foo]`, `[on $unknown]`) are kept with no effect, as in Textual; Python Rich `Text` keeps its styles (theme names such as `repr.str` are resolved, as Textual does) |
| Unicode cell widths (CJK, emoji, ZWJ) | ✅ | 90 | |
| Rich renderables (tables, syntax, panels) | 🟡 | 80 | Rust: `rustual::renderables` ports rich 15's `Table` (columns, ratios, sections, footers, row styles, title/caption, grids, every `rich.box` style), `Panel`, `Columns`, `Padding`, `Group` and `Syntax` (keyword highlighter + TextArea themes; line numbers, highlighted lines, word wrap, padding, dedent), checked line by line against rich's output (`scripts/rich_golden.py`); shown with `Static::renderable` (auto width = rich's measurement) and `RichLog::write_renderable`. Not ported: `Syntax` indent guides / line ranges, other rich renderables (`Rule`, `Tree`, `Layout`, `Progress`, ...). Python: any Rich renderable via the `rich` package in `Static`, `render()` and `RichLog` |

## Widgets

| Widget | Status | % | Notes |
|---|---|---|---|
| Static | ✅ | 95 | markup / plain / Text content, Rich-style renderables (`Static::renderable`, `update_renderable`), `update` |
| Label | ✅ | 95 | variants via classes |
| Button | ✅ | 95 | variants, flat, action, active effect |
| Input | ✅ | 85 | password, restrict (char predicate or whole-value `restrict_value`), max length, integer/number, selection; no suggester; validators only in Python (`rustual.validation`) |
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
| Tree | ✅ | 92 | generic data payload, guides, keyboard + mouse; cursor starts on the first line (as Textual) |
| DirectoryTree | 🟡 | 85 | lazy loading, sorting, icons, filter predicate, reload keeping expansion; cursor starts on the root and the root is labelled with the folder name, as in Textual; synchronous I/O |
| Tabs / Tab | ✅ | 95 | underline slides over 0.3s |
| TabbedContent / TabPane | ✅ | 90 | |
| ContentSwitcher | ✅ | 95 | |
| Collapsible | ✅ | 95 | |
| Header | ✅ | 95 | shows the screen's own title / sub-title when set (Textual `Screen.title`); clock in local time with a strftime `time_format` (default `%X`; changeable at runtime, Python `Header(time_format=...)` / `header.time_format`), `clock_utc_offset` pins a fixed offset; icon opens the command palette |
| Footer | ✅ | 92 | key labels as Textual's (`^q`, `del`, `shift+^p`); shows its own screen's bindings (also beneath a modal); Textual 8 order (focused widget → screen → app); `^p palette` key docked right (`show_command_palette`); click replays the key; binding groups (`Binding::group(BindingGroup)`, compact groups) |
| Log | ✅ | 90 | |
| RichLog | ✅ | 90 | `highlight(true)`: Rich `ReprHighlighter` port (`pretty::tokenize_text`), ANSI-theme colors at write time; `write_renderable` renders tables / panels / syntax once at write time (measured width, shrunk to the region, at least `min_width`; `write_renderable_with` for `width` / `expand`), as Textual does; no custom highlighter object |
| ProgressBar | ✅ | 95 | ETA, indeterminate, gradients |
| LoadingIndicator | ✅ | 95 | used by the `loading` overlay |
| Sparkline | ✅ | 95 | bars blended from `sparkline--min-color` to `sparkline--max-color`, `summary_function`, bucketing like Textual |
| Digits | ✅ | 95 | |
| Placeholder | ✅ | 95 | |
| Rule | ✅ | 100 | |
| Toast / ToastRack | ✅ | 90 | |
| Containers (Container, Vertical, Horizontal, Grid, Center, Middle, ScrollableContainer, ...) | ✅ | 95 | |
| Markdown / MarkdownViewer | 🟡 | 85 | pulldown-cmark (GFM tables, strikethrough, task lists); all block types, inline styles, clickable links, TOC + goto_anchor, viewer with TOC tree and file history; fences highlighted by default with the TextArea keyword highlighter (python/rust/js/json/toml; monokai dark / github_light light; replaceable hook), `open_links` (default on: scheme links via `open_url`); tables sized like Textual's grid (proportional expand, shrink to the longest word, "…" for overlong words); `append` re-renders the whole document (no incremental `MarkdownStream`) |
| Link | ✅ | 90 | opener stubbable via `link::set_url_opener`; posts `link::Opened` |
| Pretty | 🟡 | 75 | `Debug` repr, one-line or expanded, hand-written repr highlighter; all-or-nothing expansion, no indent guides |
| Tooltip | ✅ | 90 | runtime-positioned (no `constrain` CSS) |
| KeyPanel / HelpPanel | ✅ | 85 | `dock: right` instead of `split`; HelpPanel shows `Widget::help` Markdown (nearest ancestor) + embedded KeyPanel, `app.show/hide_help_panel`; help refresh lands one loop turn after focus changes (no focus watcher) |
| Welcome | 🟡 | 85 | welcome text built from Statics (not the Markdown widget); OK exits by default in Rust (`exit_on_close`), not in Python (as Textual) |

## Python bindings (`rustual`)

Textual-compatible Python API on the Rust engine (`crates/rustual-py`, PyO3 + maturin + uv).
See [`docs/python-bindings.md`](docs/python-bindings.md).

| Feature | Status | % | Notes |
|---|---|---|---|
| App lifecycle | ✅ | 90 | `run()` with GIL released (returns `exit()` value), `run_async`, `run_test` + `Pilot`; handler exceptions exit cleanly and re-raise |
| Widget API | ✅ | 90 | compose (incl. `with` blocks), render (markup / Content / Rich renderables), DEFAULT_CSS, BINDINGS + `bind()`, queries + `getters` (`query_one`, `child_by_id`, `app`), classes, styles (write), `rich_style` / `get_component_rich_style` (partial, default), `data_bind`, `set_reactive`, display/visible/disabled/loading, mount/remove/recompose, focus, scrolling, timers, `animate()`, `log` (to `TEXTUAL_LOG` and Python `logging`); no `render_line` |
| Reactivity | ✅ | 90 | reactive/var with watch/validate/compute, init, always_update, layout, recompose, toggle_class, `watch()` |
| Messages & events | ✅ | 90 | custom messages (incl. `@dataclass` ones), bubbling, stop/prevent_default, `@on` with selectors, MRO dispatch, `key_*` methods, mouse/focus/enter/leave/resize/paste, `DescendantFocus` / `DescendantBlur`, `ScreenResume` / `ScreenSuspend` |
| Screens & modes | ✅ | 90 | Screen/ModalScreen, dismiss + callback, `push_screen_wait`, SCREENS/MODES, screen CSS, `title` / `sub_title` (`TITLE` / `SUB_TITLE`), `COMMANDS`, `HORIZONTAL_BREAKPOINTS` / `VERTICAL_BREAKPOINTS`, `is_active`, `focus_next` / `focus_previous` |
| Workers & async | ✅ | 90 | async handlers/workers on an app-thread asyncio loop; thread workers; widget calls from threads auto-marshalled |
| Themes, validation | ✅ | 90 | `Theme` (incl. `ansi=`) / `register_theme`, `App(ansi_color=...)`, `app.ansi_color` / `native_ansi_color` / `no_color`; `rustual.validation`; Input `restrict` regex |
| Bindings | ✅ | 90 | `BINDINGS` (tuples / `Binding`), MRO merge, `check_action` / `refresh_bindings` |
| Built-in widgets | ✅ | 85 | every built-in widget is bound (see below) |
| Packaging | ✅ | 90 | `rustual-py` on PyPI (`pip install rustual-py`, `uvx rustual-py`): abi3 wheels (CPython 3.10+) for manylinux_2_28 / musllinux_1_2 x86_64, Windows x86_64, macOS universal2 + sdist, also on GitHub releases; Rust crates on crates.io |
| Textual helper modules | ✅ | 80 | `rustual.color` (Color, Gradient), `renderables.gradient` (LinearGradient, VerticalGradient), `highlight` (Pygments, theme-aware), `signal`, `widget.prevent()`, `auto_refresh`, `App.current_theme` / `theme_changed_signal`, `command` (`Provider`, `Hit`, `DiscoveryHit`, `SimpleProvider`, `CommandPalette`), `fuzzy.Matcher`, `system_commands`, `getters`, `lazy` (`Lazy` / `Reveal` mount at once), `cache` (`LRUCache`, `FIFOCache`), `design.ColorSystem`, `theme.BUILTIN_THEMES`, `markup.escape`, `types`, `errors`, `dom`, `timer`, `notifications`, `geometry.clamp`, `_color_constants`; app `get_system_commands` / `SystemCommand`, `search_commands`, `clear_notifications`, `set_keymap` / `update_keymap`, `get_key_display`, `InvalidThemeError` |
| Not yet | ❌ | 0 | `render_line` / `Strip` (Line API), `App.suspend`, `App.active_bindings`, custom TextArea themes (`TextAreaTheme`), Input `Suggester`, `textual.style`, devtools console, inline mode |

Python widget coverage (gaps vs Textual's Python API; the Rust widget's own gaps are listed above):

| Widget(s) | % | Gaps |
|---|---|---|
| Static, Label, Button, Input, MaskedInput | 90 | Input `suggester`; MaskedInput lacks insert/delete/replace/selection setters |
| Checkbox, RadioButton, RadioSet, Switch | 85 | `value` is a property, not a reactive (no `watch_value` in subclasses) |
| Select, SelectionList, OptionList | 80 | `Option.disabled` must be changed via enable/disable methods; Select `type_to_search` always on |
| DataTable | 80 | `get_cell` returns plain values (Rich `Text` as markup); no per-row height/label updates |
| Tree, DirectoryTree | 80 | labels are `str`; no `render_label`; DirectoryTree loads synchronously |
| ListView, Log, RichLog, Pretty | 85 | RichLog pretty-printing via `str()`; Pretty uses `pprint` |
| ProgressBar, Sparkline, Digits | 90 | ProgressBar `gradient`/`clock` ignored |
| TabbedContent, Tabs, ContentSwitcher, Collapsible | 85 | TabPane `Disabled`/`Enabled`/`Focused` and Tab `Clicked`/`Relabelled` messages missing |
| Markdown, MarkdownViewer | 75 | no `MarkdownStream` / `parser_factory`; viewer navigation methods aren't coroutines |
| TextArea | 75 | no `register_language` / `register_theme` / `edit(Edit)`; keyword highlighter instead of tree-sitter |
| Header, Footer, Rule, Placeholder, LoadingIndicator, Link, Welcome, HelpPanel, KeyPanel | 90 | HelpPanel/KeyPanel `update_help` |

## Roadmap

See [`docs/roadmap.md`](docs/roadmap.md) for what's next and the backlog.
