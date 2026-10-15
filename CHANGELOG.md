# Changelog

All notable changes to rustual (the Rust crates and the `rustual-py` Python package, which share
one version). The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project uses [Semantic Versioning](https://semver.org/). Releases are made with
`scripts/release.py` (see [`RELEASING.md`](RELEASING.md)).

## Unreleased

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
