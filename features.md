# textual-rs feature status

Parity tracker against Python Textual 8.2.8. Percentages are rough estimates of how much of
Textual's behaviour for that item is implemented here. Status: ✅ done (≥ 90%) · 🟡 partial · ❌ missing.

**Overall: milestone 1 complete. ~70% of commonly used Textual functionality, ~45% of full parity.**

_Last updated: 2026-10-14._

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
| Bindings & actions | ✅ | 85 | priority, namespaces `app.`/`screen.`/`focused.`, args; no keymap overrides, no `check_action` |
| Builtin actions | 🟡 | 70 | quit, bell, toggle_dark, set_theme, focus, focus_next/previous, pop_screen, add/remove/toggle_class, notify, scroll_* |
| Timers (`set_timer`, `set_interval`) | ✅ | 95 | |
| Workers (async + blocking) | ✅ | 80 | result posted as message; no worker state events / groups / exclusive |
| Screens: push/pop/dismiss, modal | ✅ | 85 | modal dims the screen below; no `switch_screen`, no named screens / `MODES` |
| Notifications (toasts) | ✅ | 90 | `ctx.notify`, severity, timeout, click to dismiss |
| Animation (`animate`, easing, transitions) | 🟡 | 75 | all 33 easings, `ctx.animate_value`, `animate_style` (opacity/offset/width/height), smooth scroll, animation levels, Tabs/Switch animate; no CSS `transition`, no color animation, no `delay`/`speed` |
| Tooltips | ✅ | 85 | `Child::tooltip`, `ctx.set_tooltip`; 0.5s delay, ancestor inheritance, flip above / keep on screen; no Rich renderables, no `_disable_tooltips` |
| Command palette | ✅ | 80 | `ctrl+p`, fuzzy `Matcher` port, `CommandProvider` / `App::command_provider` / `Widget::commands`, system commands, theme picker, header icon; synchronous providers (no async search / busy indicator) |
| Text selection & copy (screen selection) | ❌ | 0 | Input has its own selection + internal clipboard |
| Clipboard (OSC 52) | ❌ | 0 | |
| `@click` markup links | 🟡 | 75 | click runs the action with the widget as owner; no `link-*-hover` styles, no `@mouse.down`/`@mouse.up` |
| Screenshots (SVG / ANSI / text export) | ✅ | 85 | `canvas.to_svg()`, `to_ansi()`, `pilot.export_svg()`, `TEXTUAL_SCREENSHOT=out.svg` env mode (+ `TEXTUAL_SIZE`, `TEXTUAL_PRESS`); no screenshot key binding yet |
| Maximize (`ctrl+m`), `loading` overlay, `disabled` | 🟡 | 40 | `disabled` done; no maximize; no `loading` reactive |
| Suspend (`ctrl+z`), inline mode, no-color / ANSI themes | ❌ | 0 | |
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
| hatch, keyline, pointer, constrain, overlay, expand, line-pad, transitions | ❌ | 0 | parsed and ignored |
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
| MaskedInput | ❌ | 0 | |
| TextArea | 🟡 | 80 | editing, selection (keys/mouse drag/double+triple click), undo/redo batching, soft wrap, line numbers, themes (css/monokai/dracula/vscode_dark/github_light), bracket matching, read-only, placeholder; no tree-sitter (simple per-line keyword highlighter for python/rust/js/json/toml), no suggestions, no OSC 52 clipboard, no mouse capture |
| Checkbox / ToggleButton | ✅ | 95 | |
| RadioButton / RadioSet | ✅ | 95 | |
| Switch | ✅ | 95 | slides over 0.3s |
| Select | ✅ | 85 | dropdown via a transparent modal screen; type-to-search |
| SelectionList | ❌ | 0 | |
| OptionList | ✅ | 90 | |
| ListView / ListItem | ✅ | 90 | |
| DataTable | ✅ | 85 | cursors, sorting, fixed rows/cols, zebra, hover; cells always left-aligned |
| Tree | ✅ | 90 | generic data payload, guides, keyboard + mouse |
| DirectoryTree | ❌ | 0 | |
| Tabs / Tab | ✅ | 95 | underline slides over 0.3s |
| TabbedContent / TabPane | ✅ | 90 | |
| ContentSwitcher | ✅ | 95 | |
| Collapsible | ✅ | 95 | |
| Header | ✅ | 90 | clock in UTC (offset configurable); icon opens the command palette |
| Footer | ✅ | 85 | click replays the key; no binding groups |
| Log | ✅ | 90 | |
| RichLog | ✅ | 80 | no highlighter |
| ProgressBar | ✅ | 90 | ETA, indeterminate; no gradient |
| LoadingIndicator | ✅ | 90 | no `loading` overlay integration |
| Sparkline | ✅ | 85 | no gradient |
| Digits | ✅ | 95 | |
| Placeholder | ✅ | 95 | |
| Rule | ✅ | 100 | |
| Toast / ToastRack | ✅ | 90 | |
| Containers (Container, Vertical, Horizontal, Grid, Center, Middle, ScrollableContainer, ...) | ✅ | 95 | |
| Markdown / MarkdownViewer | 🟡 | 75 | pulldown-cmark (GFM tables, strikethrough, task lists); all block types, inline styles, clickable links, TOC + goto_anchor, viewer with TOC tree and file history; no syntax highlighting (hook only), no streaming / incremental append, no `open_url` |
| Link | ✅ | 90 | opener stubbable via `link::set_url_opener`; posts `link::Opened` |
| Pretty | ❌ | 0 | |
| Tooltip | ✅ | 90 | runtime-positioned (no `constrain` CSS) |
| KeyPanel / HelpPanel | 🟡 | 60 | KeyPanel ✅ (~85%, `dock: right` instead of `split`); no HelpPanel (needs Markdown / `HELP`) |
| Welcome | ❌ | 0 | |

## Roadmap

1. **Essential widgets:** TextArea, Markdown/MarkdownViewer.
2. **Core:** animation (easing, `animate`, smooth scroll, Tabs/Switch/ProgressBar motion),
   scrollbar dragging + mouse capture, OSC 52 clipboard.
3. **Low-hanging fruit:** Pretty, SelectionList, MaskedInput, DirectoryTree, HelpPanel, Welcome,
   SVG/text screenshots, `switch_screen` + named screens/modes, scrollbar hover colors, maximize, `loading` overlay,
   suspend (`ctrl+z`).
