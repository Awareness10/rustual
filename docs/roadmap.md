# Roadmap

Where rustual is heading: full parity with Python Textual 8.2.8, and production readiness.
Per-feature status lives in [`features.md`](../features.md); releases follow
[`RELEASING.md`](../RELEASING.md). Items are sized **S** (hours), **M** (a day) or **L** (several days).

Latest release: **v0.6.0** on [PyPI](https://pypi.org/project/rustual-py/),
[crates.io](https://crates.io/crates/rustual) and [GitHub](https://github.com/Awareness10/rustual/releases).

## Next up

Textual's own example apps run unchanged with only the imports swapped: 11 of the 15 are in the
side-by-side `rustual-examples`, and `color_command` and `breakpoints` now pass in tests too (`dictionary`
and `mother` need a web API / an LLM). Next, Textual's demo:

- [ ] **Example parity** (S): add `color_command`, `breakpoints` and `dictionary` to `rustual-examples`.
- [ ] **Textual's demo** (L): run `python -m textual`'s `DemoApp` with the imports swapped; fix what it exposes.
- [ ] **Real-terminal QA pass** (M): mouse, resize, true color, wide characters and performance in kitty,
  WezTerm, Alacritty, GNOME Terminal, tmux and Windows Terminal (so far: headless, a pyte emulator, SVG
  screenshots, and the side-by-side examples).

## Releases & production readiness

- [ ] **More wheel targets** (S): Linux aarch64 (manylinux + musllinux) and Windows arm64 in `release.yml`.
- [ ] **Kitty keyboard protocol** (S): enable crossterm's keyboard enhancement flags where supported, so keys
  like `ctrl+enter`, `shift+enter` and key releases are distinguishable.
- [ ] **Windows support check** (S): the test suite passes on Windows in CI; still run the examples in a real
  Windows terminal (suspend is unix-only).
- [ ] **Public API review** (M): naming consistency (`get_value` vs `value`, builder vs setter patterns,
  which methods take `ctx`), `#[non_exhaustive]` on public enums, hide internals that leaked as `pub`.
- [ ] **Rustdoc pass** (M): a crate-level tutorial and docs on every public widget. Done: all doc examples
  are compiled doc-tests (`no_run` only where they start a terminal) and `cargo doc` runs warning-free in
  CI.
- [ ] **Clipboard fallback** (S): copy now goes through the app (OSC 52 + per-app clipboard), but paste still
  falls back to a process-global clipboard shared by Input/TextArea; remove the global so parallel tests
  can't race.

## Core

- [ ] **Text selection & copy across widgets** (L): Textual's screen-wide mouse selection (`ALLOW_SELECT`,
  `text_select_all`, `selection` component styles), copy via OSC 52.
- [ ] **Inline mode** (L): `App.run(inline=True)` rendering below the prompt without the alternate screen.
- [ ] **Devtools** (L): `textual console`-style log sink, `log()` API, DOM inspector.
- [ ] **CSS hot reload** (S): watch `css_path` files and re-apply on change (Textual `--dev`).
- [ ] **Unsupported CSS properties** (M): `keyline`, `constrain`, `overlay: screen` (would simplify
  Select and the command palette), `expand`, `line-pad`, `pointer` (mouse cursor shapes via OSC 22).
- [ ] **Snapshot testing** (M): SVG snapshot compare helper (`pilot.assert_snapshot("name")` storing
  baselines), like pytest-textual-snapshot.
- [ ] **`@click` links** (S): `link-*-hover` styles; `@mouse.down` / `@mouse.up` handlers.
- [ ] **Screens & modes** (S): `remove_mode`, modes from screen names; unknown screen/mode names should be
  reported (currently return `None`/`false`).
- [ ] **Workers** (S): cancel a widget's workers when it is removed (results are currently dropped).
- [ ] **Loading** (S): refuse programmatic focus of loading widgets (Textual does); block their bindings.
- [ ] **Screenshots** (S): local-time filenames and default to the Downloads folder like Textual.
- [ ] **Command palette** (S): busy indicator while searches run; delay "No matches found" like Textual.
- [ ] **Mouse** (S): `MouseCapture` / `MouseRelease` events; click chain as an event field.
- [ ] **Animations** (S): first frame lands one frame late; colour `speed`; animated thumb drag;
  transitions from/to `auto` and `fr` sizes jump instead of animating.
- [ ] **Docks** (S): document (or optionally fix) that multiple widgets docked to the same edge overlap,
  as in Textual — this has already tripped up one example.
- [ ] **Scrolling helpers** (S): `scroll_to_widget(top=True)` / `scroll_visible(top=..)`, 3-cell margin when
  keeping a cursor visible (Textual), `center_scroll`.
- [ ] **Reactive-style ergonomics** (M): optional `Reactive<T>` helper with watch/validate/compute callbacks,
  to reduce hand-written setter boilerplate.
- [ ] **Performance** (M): incremental restyle/relayout (dirty flags) for very large DOMs; currently the
  whole tree is restyled every frame (~2 ms per frame at 1000 widgets in release, acceptable for now).

## Widgets

- [ ] **TextArea** (M): autocomplete suggestions, more themes, 3-cell scroll margin.
- [ ] **Syntax highlighting** (L): Markdown fences, TextArea and the `Syntax` renderable use a line-based
  keyword highlighter (python, rust, js, json, toml); replace with tree-sitter (or syntect) for multi-line
  constructs and more languages.
- [ ] **Markdown** (M): streaming / incremental `append` (`MarkdownStream`), horizontal scrolling in code
  fences, custom block classes (`parser_factory`).
- [ ] **DataTable** (S): re-measure row heights on column changes; wrap long cells in auto-height rows.
- [ ] **Input** (S): `Suggester` (inline completion).
- [ ] **Button** (S): the focus style (`b reverse`) should cover the padded label like Textual (an empty
  label shows one reversed cell; visible in `calculator` and `five_by_five`).
- [ ] **Select** (S): render the dropdown as an overlay instead of a modal screen (after `overlay: screen`).
- [ ] **ListView** (S): `-hovered` class semantics instead of `:hover` (which also matches descendants).
- [ ] **Footer** (S): clickable group descriptions.
- [ ] **Pretty** (S): per-container expansion like Rich (instead of all-or-nothing), indent guides.
- [ ] **DirectoryTree** (S): load directories in a worker for slow filesystems.
- [ ] **Welcome** (S): render the welcome text with the Markdown widget.
- [ ] **Renderables** (S): `Syntax` indent guides and `line_range`; more rich renderables when needed (`Rule`,
  `Tree`, `Align` with vertical alignment); renderables in tooltips and as `Widget::render` content.
- [ ] **Static** (S): empty content should still be one line tall with `height: auto` (Textual measures
  `""` as one line); visible in `code_browser`, where Textual's empty `#code` row is not hatched.
- [ ] **Placeholder** (S): per-app color cycling counter like Textual.

## Python bindings

- [ ] **Widget API gaps** (M): `scripts/api_coverage.py` lists every missing public name (58% of Textual's are
  present); see also the per-widget notes in [`features.md`](../features.md) (e.g. TextArea `register_language`,
  DataTable per-row updates, Tree label styling, reactive-style watchers on widget properties such as
  `Checkbox.value`, TabPane `Disabled`/`Enabled` messages).
- [ ] **`render_line` / Line API** (M): Python `ScrollView` subclasses rendering strips per line.
- [ ] **`styles` read access** (S): expose computed styles (currently write-only, plus display/visibility).
- [ ] **`App.suspend()`** (M): run another program in the terminal (`with app.suspend():`); the run loop must
  drop its input stream meanwhile, or crossterm's reader thread takes the program's keystrokes.
- [ ] **Compatibility leftovers** (S each), from scanning popular Textual apps: `App.active_bindings`
  (`ActiveBinding`), custom TextArea themes (`TextAreaTheme`, `register_theme`), Input `Suggester`,
  `textual.style.Style`, `MarkdownStream`.
- [ ] **`App.save_screenshot` / `export_screenshot`** (S): Textual's screenshot methods are missing on the
  Python `App` (only the `screenshot` action and `pilot.export_svg()` exist).
- [ ] **Type stubs** (S): a `_native.pyi` stub (the package already ships `py.typed`).
- [ ] **Python CI on Windows/macOS** (S): the Python test suite runs on Linux only (release wheels are
  smoke-tested on all platforms).

## Docs & examples

- [ ] **Guide** (M): port the essential parts of Textual's guide (app basics, CSS, layout, events, actions,
  screens, workers, testing) to Rust idioms.
- [ ] **Gallery script** (S): a script that regenerates `docs/assets/screenshots/` from the examples via
  `TEXTUAL_SCREENSHOT` (the README gallery's five captures are currently made by hand).
- [ ] **Bundled examples** (S): more demos in `rustual.demo` (the `rustual-py` launcher), e.g. ports
  of Textual's calculator and five-by-five.
