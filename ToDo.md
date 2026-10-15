# ToDo

What remains for textual-rs to reach full parity with Python Textual 8.2.8 and to be production-ready.
Status per feature lives in [`features.md`](features.md); this file is the actionable backlog.
Items are grouped by priority; each is sized **S** (hours), **M** (a day), or **L** (several days).

## 1. Before a first release

- [ ] **Real-terminal QA pass** (M): mouse, resize, true color, wide characters, and performance across kitty,
  WezTerm, Alacritty, GNOME Terminal, tmux, Windows Terminal. So far, all checks were headless (Pilot, a pyte
  terminal emulator, SVG screenshots).
- [ ] **Kitty keyboard protocol** (S): enable crossterm's keyboard enhancement flags where supported, so keys
  like `ctrl+enter`, `shift+enter` and key releases are distinguishable.
- [ ] **Windows support check** (M): build and run examples on Windows; suspend is unix-only.
- [ ] **Public API review** (M): naming consistency (`get_value` vs `value`, builder vs setter patterns,
  which methods take `ctx`), `#[non_exhaustive]` on public enums, hide internals that leaked as `pub`.
- [ ] **Rustdoc pass** (M): crate-level docs with a tutorial, docs on every public widget, doc-tests for
  the main APIs; `cargo doc` without warnings.
- [ ] **Publishable crates** (S): metadata, README per crate, LICENSE file, CI workflow
  (test + clippy + fmt on Linux/macOS/Windows), MSRV declared.
- [ ] **Clipboard fallback** (S): copy now goes through the app (OSC 52 + per-app clipboard), but paste still
  falls back to a process-global clipboard shared by Input/TextArea; remove the global so parallel tests
  can't race.

## 2. Core gaps

- [ ] **Text selection & copy across widgets** (L): Textual's screen-wide mouse selection (`ALLOW_SELECT`,
  `text_select_all`, `selection` component styles), copy via OSC 52.
- [ ] **Inline mode** (L): `App.run(inline=True)` rendering below the prompt without the alternate screen.
- [ ] **ANSI / no-color themes** (M): `textual-ansi` theme, `ansi_default` color, `:ansi` / `:nocolor`
  pseudo-classes, `NO_COLOR` env support.
- [ ] **Devtools** (L): `textual console`-style log sink, `log()` API, DOM inspector.
- [ ] **CSS hot reload** (S): watch `css_path` files and re-apply on change (Textual `--dev`).
- [ ] **Unsupported CSS properties** (M): `hatch`, `keyline`, `constrain`, `overlay: screen` (would simplify
  Select and the command palette), `expand`, `line-pad`, `pointer` (mouse cursor shapes via OSC 22),
  `layout: stream`.
- [ ] **Snapshot testing** (M): SVG snapshot compare helper (`pilot.assert_snapshot("name")` storing
  baselines), like pytest-textual-snapshot.
- [ ] **`@click` links** (S): `link-*-hover` styles; `@mouse.down` / `@mouse.up` handlers.
- [ ] **Screens & modes** (S): `ScreenSuspend` / `ScreenResume` events, `remove_mode`, modes from screen
  names; unknown screen/mode names should be reported (currently return `None`/`false`).
- [ ] **Actions** (S): `check_action` (dynamic enable/disable of bindings and footer dimming).
- [ ] **Workers** (S): cancel a widget's workers when it is removed (results are currently dropped).
- [ ] **Loading** (S): refuse programmatic focus of loading widgets (Textual does); block their bindings.
- [ ] **Screenshots** (S): local-time filenames and default to the Downloads folder like Textual.
- [ ] **Command palette** (M): async providers with incremental results, busy indicator, discovery hits.
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

## 3. Widget gaps

- [ ] **TextArea** (M): autocomplete suggestions, more themes, 3-cell scroll margin.
- [ ] **Syntax highlighting** (L): Markdown fences and TextArea use a line-based keyword highlighter (python,
  rust, js, json, toml); replace with tree-sitter (or syntect) for multi-line constructs and more languages.
- [ ] **Markdown** (M): streaming / incremental `append` (`MarkdownStream`), horizontal scrolling in code
  fences, custom block classes (`parser_factory`).
- [ ] **DataTable** (S): re-measure row heights on column changes; wrap long cells in auto-height rows.
- [ ] **Input** (S): `Suggester` (inline completion), pluggable validators (`validators=`).
- [ ] **Select** (S): render the dropdown as an overlay instead of a modal screen (after `overlay: screen`).
- [ ] **ListView** (S): `-hovered` class semantics instead of `:hover` (which also matches descendants).
- [ ] **Header** (S): local timezone clock (currently UTC with configurable offset), strftime formats.
- [ ] **Footer** (S): disabled-binding styling (needs `check_action`), clickable group descriptions.
- [ ] **Pretty** (S): per-container expansion like Rich (instead of all-or-nothing), indent guides.
- [ ] **DirectoryTree** (S): load directories in a worker for slow filesystems.
- [ ] **Welcome** (S): render the welcome text with the Markdown widget.
- [ ] **Rich renderables** (L): Rich-style `Table`, `Syntax`, `Panel`, `Columns` renderables usable in
  `Static` / `RichLog`.
- [ ] **Placeholder** (S): per-app color cycling counter like Textual.

## 4. Python bindings (`textual_rs`)

- [ ] **Remaining widget bindings** (M): bind every built-in widget (in progress: forms, data, content batches).
- [ ] **`check_action` / `refresh_bindings`** (S, needs the core `check_action` above).
- [ ] **Command palette providers** (M): Python `Provider` classes (`COMMANDS`) on the core `CommandProvider`.
- [ ] **`render_line` / Line API** (M): Python `ScrollView` subclasses rendering strips per line.
- [ ] **`styles` read access** (S): expose computed styles (currently write-only, plus display/visibility).
- [ ] **`App.suspend()`, devtools `log()`** (S each).
- [ ] **Wheels & CI** (S): build abi3 wheels for Linux/macOS/Windows with maturin in CI; publish.
- [ ] **Type stubs** (S): `py.typed` + stubs for `_native`.

## 5. Docs & examples

- [ ] **Guide** (M): port the essential parts of Textual's guide (app basics, CSS, layout, events, actions,
  screens, workers, testing) to Rust idioms.
- [ ] **Gallery** (S): SVG screenshots of every example in `docs/` generated by `TEXTUAL_SCREENSHOT`.
- [ ] **Port Textual's example apps** (M): calculator, code browser, five-by-five, dictionary, stopwatch
  tutorial — good end-to-end validation.
