# Changelog

All notable changes to rustual (the Rust crates and the `rustual-py` Python package, which share
one version). The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project uses [Semantic Versioning](https://semver.org/). Releases are made with
`scripts/release.py` (see [`RELEASING.md`](RELEASING.md)).

## Unreleased

### Added
- `layout: stream` (Textual's stream layout for long lists: full-width, auto-height children);
  child regions match Textual 8.2.8 exactly. Python `Widget.virtual_size`.

### Fixed
- The Python `Pilot` (`press`, `click`, `pause`, ...) could return while an animation such as
  smooth scrolling was still running (when frames were fast); it now waits for animations, like
  Textual's.

## 0.4.0 - 2026-10-16

### Added
- `hatch` CSS (`hatch: right $panel`, `hatch: "x" red 50%`, `hatch: none`) and Python
  `styles.hatch = ("cross", "red")`: hatched backgrounds as in Textual. A maximized widget's screen is
  now hatched around it, like Textual's.
- Rich-style renderables for Rust (`rustual::renderables`): ports of rich's `Table`, `Panel`,
  `Columns`, `Padding` and `Group`, plus `Syntax` (keyword highlighting, line numbers, highlighted
  lines, word wrap). Their layout matches rich 15 line for line. Show them with
  `Static::renderable` / `update_renderable` or `RichLog::write_renderable`; see the `renderables`
  example.
- ANSI themes `ansi-dark` / `ansi-light` (Textual's, with identical variables) that use the
  terminal's own colors: `ansi_*` / `ansi_default` colors are written as ANSI codes, `:ansi` /
  `:nocolor` pseudo-classes and the widgets' `:ansi` rules apply. `App::ansi_color` /
  `ctx.set_ansi_color` (Python `App(ansi_color=...)`, `app.ansi_color`) choose native ANSI colors
  or truecolor; `NO_COLOR` removes colors (`App::no_color`). `Theme::ansi` / Python `Theme(ansi=True)`.
- Command palette search hooks (`App::command_search`, `Ctx::add_command_hits`): hits delivered
  later (e.g. from async work), merged by score as they arrive; `App::system_commands(false)`.
  Python: `rustual.command` (`Provider`, `Hit`, `DiscoveryHit`, `SimpleProvider`,
  `CommandPalette`), `App.COMMANDS` / `Screen.COMMANDS`, `get_system_commands` / `SystemCommand`,
  `search_commands`, `fuzzy.Matcher`. Textual's `color_command` example runs unchanged.
- Breakpoints: `App::horizontal_breakpoints` / `vertical_breakpoints`, per-screen
  `Ctx::set_screen_breakpoints` (Python `HORIZONTAL_BREAKPOINTS` / `VERTICAL_BREAKPOINTS` on apps
  and screens).
- `DescendantFocus` / `DescendantBlur` and `ScreenResume` / `ScreenSuspend` events; per-screen
  titles (`Ctx::set_screen_title`, Python `Screen.title` / `TITLE`); runtime bindings (`Ctx::bind`);
  `Ctx::widget_style`, `Ctx::clear_notifications`.
- Python compatibility with real Textual apps (found by scanning 9 popular ones): `log`
  (`TEXTUAL_LOG` / `logging`), `lazy`, `getters`, `cache`, `design.ColorSystem`,
  `theme.BUILTIN_THEMES`, `markup.escape`, `types`, `errors`, `dom`, `timer`, `notifications`,
  `_color_constants`, `InvalidThemeError`, `rich_style` / `get_component_rich_style`, `data_bind`,
  `set_reactive`, `bind`, `clear_notifications`, `set_keymap`, `get_key_display`, `@dataclass`
  messages.
- `Header::time_format` / `set_time_format` (Python `Header(time_format=...)`, `header.time_format`):
  strftime formats for the header clock (default `%X`).

### Changed
- `ansi_*` colors are the terminal's own colors (`Color::ansi`): `ansi_default` is no longer
  transparent, and without native ANSI colors they are shown with Textual's terminal themes
  (MONOKAI / ALABASTER) instead of fixed RGB values. `Color` has a new `ansi` field.
- `dim` text is drawn as a color blended towards the background (like Textual) unless ANSI
  colors are native.
- The header clock shows the local time (it was UTC), like Textual. `clock_utc_offset` still pins a
  fixed offset. rustual now depends on `chrono` for time zones and formatting.

### Fixed
- Python `Content.from_markup(...).plain` / `str()` returned the markup instead of the text.

## 0.3.0 - 2026-10-15

### Added
- `rustual-py` command and `python -m rustual`: a launcher for the bundled example apps
  (stopwatch, counter, forms, data, content, workers). Try it without installing: `uvx rustual-py`.
  The examples moved from `crates/rustual-py/examples/` into the package (`rustual.demo`).

### Changed
- `ToDo.md` is now [`docs/roadmap.md`](docs/roadmap.md), with a "Next up" section.

### Fixed
- A worker that outlives its app (e.g. a thread still running after exit) no longer reports its
  failure into the next app.

## 0.2.0 - 2026-10-15

First release published to PyPI and crates.io.

### Added
- Python: `Widget.prevent()`, `auto_refresh`, `rustual.color` (`Color`, `Gradient`),
  `rustual.renderables.gradient` (`LinearGradient`, `VerticalGradient`), `rustual.highlight`
  (theme-aware syntax highlighting), `rustual.signal`, `App.current_theme`,
  `App.theme_changed_signal`, `MarkdownViewer.navigator`, `RenderableType`; containers accept
  `can_focus` / `can_focus_children`; `DataTable[...]` / `Tree[...]` are subscriptable.
- Footer shows the command palette key (`^p palette`) docked right, like Textual
  (`show_command_palette`).
- Core: message suppression scopes (`Ctx::prevent_messages`), read-only DOM access in
  `check_action`, `Dom::active_screen`.
- All documentation examples are compiled doc-tests; rustdoc is warning-free and checked in CI.
- crates.io metadata, a README and LICENSE per crate; CI runs a publish dry run.

### Changed
- Footer bindings are ordered like Textual 8 (focused widget first, then screen and app).
- A Tree's cursor starts on the first line (posting `NodeHighlighted`), as in Textual.
- Widgets with a transparent background paint their ancestors' background colour instead of
  showing what the parent drew underneath (Textual's opaque-widget behaviour).
- Python `Key` events use Textual's key names for punctuation (`plus`, `equals_sign`, ...).
- `Widget::check_action` takes a `&Dom` argument.

### Fixed
- Startup auto-focus no longer overrides focus set by a screen pushed from the app's `on_mount`.
- Setting `App.title` / `sub_title` / `theme` inside `compose()` no longer raises.
- Rich renderables returned from `render()` are measured at their natural height.

## 0.1.0 - 2026-10-15

First release: Python wheels for Linux (glibc and musl), Windows and macOS on GitHub releases.
