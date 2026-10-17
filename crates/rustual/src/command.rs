//! Command palette core (port of `textual/command.py`, `fuzzy.py`, `system_commands.py`):
//! the fuzzy [`Matcher`], [`Command`]s and [`CommandProvider`]s.
//!
//! `ctrl+p` (the app's `command_palette` action) opens a
//! [`CommandPalette`](crate::widgets::command_palette::CommandPalette) screen listing the
//! commands of every source:
//!
//! - [`SystemCommandsProvider`] (theme, quit, keys panel, maximize, screenshot, ...), always
//!   first registered;
//! - providers added with [`App::command_provider`];
//! - [`Widget::commands`](crate::Widget::commands) of the app root and of the active screen.
//!
//! ```
//! # use rustual::prelude::*;
//! # struct MyApp;
//! # impl Widget for MyApp {}
//! # let _app =
//! App::new(MyApp)
//!     .command_provider(|_ctx: &mut Ctx| vec![
//!         Command::new("Save", "Save the document", "save"),
//!         Command::callback("Hello", "Say hello", |ctx| ctx.notify("Hello!")),
//!     ])
//! # ;
//! ```
//!
//! [`CommandProvider`]s return all their commands up front and the palette does the fuzzy
//! matching; "discovery" (empty input) lists every command whose [`Command::discover`] flag is
//! set. Searches that produce results later (Textual's async `Provider.search` / `discover`)
//! use a [`SearchHook`] ([`App::command_search`]): it is told about each query and delivers
//! scored hits with [`Ctx::add_command_hits`].

use crate::dom::NodeId;

use crate::app::{App, AppState};
use crate::binding::Binding;
use crate::context::Ctx;
use crate::widget::Child;
use rustual_render::{Style, Text};
use std::fmt;
use std::rc::Rc;

// ---------------------------------------------------------------------------------- fuzzy

/// Upper bound on candidate offset combinations examined per match (Textual examines all,
/// which is exponential in the worst case).
const MAX_COMBINATIONS: usize = 10_000;

/// Fuzzy search (port of `fuzzy.py::FuzzySearch`). Finds all ways the query letters appear in
/// order in the candidate and keeps the best score.
#[derive(Debug, Clone, Default)]
pub struct FuzzySearch {
    pub case_sensitive: bool,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl FuzzySearch {
    pub fn new(case_sensitive: bool) -> Self {
        FuzzySearch { case_sensitive }
    }

    /// Score and matched character offsets (`(0.0, [])` for no match).
    pub fn match_offsets(&self, query: &str, candidate: &str) -> (f64, Vec<usize>) {
        let fold = |s: &str| -> Vec<char> {
            if self.case_sensitive {
                s.chars().collect()
            } else {
                // Per-character lowercasing keeps character offsets aligned with the candidate.
                s.chars()
                    .map(|c| c.to_lowercase().next().unwrap_or(c))
                    .collect()
            }
        };
        let query = fold(query);
        let candidate = fold(candidate);
        if query.is_empty() {
            return (0.0, Vec::new());
        }

        // Quick exit when the query exists as a substring.
        if let Some(loc) = find_sub(&candidate, &query, 0) {
            let offsets: Vec<usize> = (loc..loc + query.len()).collect();
            let factor = if candidate == query { 2.0 } else { 1.5 };
            return (score(&candidate, &offsets) * factor, offsets);
        }

        let mut letter_positions: Vec<Vec<usize>> = Vec::new();
        let mut position = 0;
        for (offset, letter) in query.iter().enumerate() {
            let last_index = candidate.len().saturating_sub(offset);
            let mut positions = Vec::new();
            let mut index = position;
            while let Some(loc) = (index..candidate.len()).find(|i| candidate[*i] == *letter) {
                positions.push(loc);
                index = loc + 1;
                if index >= last_index {
                    break;
                }
            }
            if positions.is_empty() {
                return (0.0, Vec::new());
            }
            position = positions[0] + 1;
            letter_positions.push(positions);
        }

        let mut best: (f64, Vec<usize>) = (0.0, Vec::new());
        let mut examined = 0usize;
        let mut offsets = Vec::with_capacity(query.len());
        collect_offsets(&letter_positions, 0, &mut offsets, &mut |o: &[usize]| {
            examined += 1;
            let s = score(&candidate, o);
            if s > best.0 {
                best = (s, o.to_vec());
            }
            examined < MAX_COMBINATIONS
        });
        best
    }
}

fn find_sub(hay: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|i| hay[*i..*i + needle.len()] == *needle)
}

/// Recursively enumerate increasing offset sequences; `visit` returns false to stop.
fn collect_offsets(
    letter_positions: &[Vec<usize>],
    index: usize,
    offsets: &mut Vec<usize>,
    visit: &mut dyn FnMut(&[usize]) -> bool,
) -> bool {
    for &offset in &letter_positions[index] {
        if offsets.last().is_none_or(|last| offset > *last) {
            offsets.push(offset);
            let keep_going = if index + 1 == letter_positions.len() {
                visit(offsets)
            } else {
                collect_offsets(letter_positions, index + 1, offsets, visit)
            };
            offsets.pop();
            if !keep_going {
                return false;
            }
        }
    }
    true
}

/// `FuzzySearch.score`: boosts first letters of words and fewer, longer runs.
fn score(candidate: &[char], positions: &[usize]) -> f64 {
    if positions.is_empty() {
        return 0.0;
    }
    let first_letter =
        |i: usize| is_word_char(candidate[i]) && (i == 0 || !is_word_char(candidate[i - 1]));
    let offset_count = positions.len();
    let mut score = (offset_count + positions.iter().filter(|p| first_letter(**p)).count()) as f64;
    let mut groups = 1;
    for w in positions.windows(2) {
        if w[1] != w[0] + 1 {
            groups += 1;
        }
    }
    let normalized_groups = (offset_count - (groups - 1)) as f64 / offset_count as f64;
    score *= 1.0 + normalized_groups * normalized_groups;
    score
}

/// A fuzzy matcher for one query (port of `fuzzy.py::Matcher`).
#[derive(Debug, Clone)]
pub struct Matcher {
    query: String,
    match_style: Style,
    search: FuzzySearch,
}

impl Matcher {
    /// Case-insensitive matcher; matches are highlighted in reverse video by default.
    pub fn new(query: &str) -> Self {
        Matcher {
            query: query.to_string(),
            match_style: Style::new().reverse(),
            search: FuzzySearch::new(false),
        }
    }

    pub fn case_sensitive(mut self, case_sensitive: bool) -> Self {
        self.search.case_sensitive = case_sensitive;
        self
    }

    pub fn match_style(mut self, style: Style) -> Self {
        self.match_style = style;
        self
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Match strength (0 = no match; higher is better; not normalized).
    pub fn score(&self, candidate: &str) -> f64 {
        self.search.match_offsets(&self.query, candidate).0
    }

    /// Character offsets of the best match.
    pub fn offsets(&self, candidate: &str) -> Vec<usize> {
        self.search.match_offsets(&self.query, candidate).1
    }

    /// `candidate` as plain text with the matched (non-space) characters styled.
    pub fn highlight(&self, candidate: &str) -> Text {
        let (score, offsets) = self.search.match_offsets(&self.query, candidate);
        if score <= 0.0 {
            return Text::plain(candidate);
        }
        let mut text = Text::new();
        let mut run = String::new();
        let mut run_hl = false;
        for (i, c) in candidate.chars().enumerate() {
            let hl = offsets.contains(&i) && !c.is_whitespace();
            if hl != run_hl && !run.is_empty() {
                let style = if run_hl {
                    self.match_style.clone()
                } else {
                    Style::new()
                };
                text.push(std::mem::take(&mut run), style);
            }
            run_hl = hl;
            run.push(c);
        }
        if !run.is_empty() {
            let style = if run_hl {
                self.match_style.clone()
            } else {
                Style::new()
            };
            text.push(run, style);
        }
        text
    }
}

// ---------------------------------------------------------------------------------- commands

/// What a [`Command`] does when chosen.
#[derive(Clone)]
pub enum CommandAction {
    /// An action string (e.g. `"app.bell"`, `"set_theme('nord')"`), run on behalf of the app root.
    Action(String),
    /// A callback, run with a [`Ctx`] for the app root after the palette has closed.
    Callback(Rc<dyn Fn(&mut Ctx)>),
}

impl fmt::Debug for CommandAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandAction::Action(a) => f.debug_tuple("Action").field(a).finish(),
            CommandAction::Callback(_) => f.write_str("Callback(..)"),
        }
    }
}

/// A command palette entry (Textual: `SystemCommand` / `Hit`).
#[derive(Debug, Clone)]
pub struct Command {
    /// Name, matched against the user's input.
    pub name: String,
    /// Help text shown (dimmed) under the name.
    pub help: String,
    pub action: CommandAction,
    /// Listed when the input is empty (Textual's `discover`). Default `true`.
    pub discover: bool,
    /// A score from an external search (see [`SearchHook`]), on the scale of
    /// [`Matcher::score`] (like Textual's, not limited to 0..1); the palette doesn't match such
    /// commands itself.
    pub score: Option<f64>,
    /// What the list shows instead of the name (e.g. with the matched letters highlighted).
    pub display: Option<Text>,
}

impl Command {
    /// A command that runs an action string.
    pub fn new(name: &str, help: &str, action: &str) -> Self {
        Command {
            name: name.to_string(),
            help: help.to_string(),
            action: CommandAction::Action(action.to_string()),
            discover: true,
            score: None,
            display: None,
        }
    }

    /// A command that runs a callback (with a `Ctx` for the app root).
    pub fn callback(name: &str, help: &str, f: impl Fn(&mut Ctx) + 'static) -> Self {
        Command {
            name: name.to_string(),
            help: help.to_string(),
            action: CommandAction::Callback(Rc::new(f)),
            discover: true,
            score: None,
            display: None,
        }
    }

    pub fn discover(mut self, discover: bool) -> Self {
        self.discover = discover;
        self
    }

    /// The score of an externally matched hit (see [`Command::score`]).
    pub fn score(mut self, score: f64) -> Self {
        self.score = Some(score);
        self
    }

    /// Show `display` in the list instead of the name.
    pub fn display(mut self, display: Text) -> Self {
        self.display = Some(display);
        self
    }
}

/// What a [`SearchHook`] is told about an open palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchEvent<'a> {
    /// The palette opened (Textual: providers' `startup`).
    Opened,
    /// The input changed to `query` (empty: discovery). Hits for it must carry `generation`
    /// (older generations are ignored).
    Query { query: &'a str, generation: u64 },
    /// The palette closed (Textual: providers' `shutdown`).
    Closed,
}

/// A search that delivers hits later, e.g. from async work: called with the palette's node,
/// the event and a `Ctx`; answers with [`Ctx::add_command_hits`]. Registered with
/// [`App::command_search`].
pub type SearchHook = Rc<dyn Fn(NodeId, SearchEvent<'_>, &mut Ctx)>;

/// A source of commands (Textual: `command.Provider`). Called each time the palette opens,
/// with a `Ctx` for the app root (the palette is not open yet, so `ctx.screen()` and
/// `ctx.focused()` describe the screen the palette was invoked from).
pub trait CommandProvider {
    fn commands(&self, ctx: &mut Ctx) -> Vec<Command>;
}

impl<F: Fn(&mut Ctx) -> Vec<Command>> CommandProvider for F {
    fn commands(&self, ctx: &mut Ctx) -> Vec<Command> {
        self(ctx)
    }
}

/// A fixed list of commands (Textual: `SimpleProvider`).
impl CommandProvider for Vec<Command> {
    fn commands(&self, _ctx: &mut Ctx) -> Vec<Command> {
        self.clone()
    }
}

/// The built-in commands (port of `system_commands.py` + `App.get_system_commands`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCommandsProvider;

impl CommandProvider for SystemCommandsProvider {
    fn commands(&self, ctx: &mut Ctx) -> Vec<Command> {
        let mut commands = vec![
            Command::new("Theme", "Change the current theme", "app.change_theme"),
            Command::new(
                "Quit",
                "Quit the application as soon as possible",
                "app.quit",
            ),
        ];
        let screen = ctx.screen();
        if ctx.query_in(screen, "KeyPanel").is_empty() {
            commands.push(Command::new(
                "Keys",
                "Show help for the focused widget and a summary of available keys",
                "app.show_help_panel",
            ));
        } else {
            commands.push(Command::new(
                "Keys",
                "Hide the keys and widget help panel",
                "app.hide_help_panel",
            ));
        }
        if ctx.maximized().is_some() {
            commands.push(Command::new(
                "Minimize",
                "Minimize the widget and restore to normal size",
                "app.minimize",
            ));
        } else if ctx
            .focused()
            .is_some_and(|f| crate::screens::allow_maximize(ctx.dom(), f))
        {
            commands.push(Command::new(
                "Maximize",
                "Maximize the focused widget",
                "app.maximize",
            ));
        }
        commands.push(Command::new(
            "Screenshot",
            "Save an SVG 'screenshot' of the current screen",
            "app.screenshot",
        ));
        // Not in Textual 8 (which dropped `toggle_dark` in favour of themes), kept for convenience.
        commands.push(Command::new(
            "Toggle dark mode",
            "Switch between the light and dark default themes",
            "app.toggle_dark",
        ));
        commands.push(Command::new(
            "Focus next",
            "Move focus to the next widget",
            "app.focus_next",
        ));
        commands.push(Command::new(
            "Focus previous",
            "Move focus to the previous widget",
            "app.focus_previous",
        ));
        commands.sort_by(|a, b| a.name.cmp(&b.name));
        commands
    }
}

/// The `ctrl+p` binding the runtime adds to the app root (Textual: `COMMAND_PALETTE_BINDING`).
pub(crate) fn palette_binding() -> Binding {
    Binding::new("ctrl+p", "command_palette", "palette")
        .hidden()
        .priority()
}

// ---------------------------------------------------------------------------------- opening

/// Whether a command palette is the active screen (Textual: `CommandPalette.is_open`).
pub(crate) fn is_open(state: &AppState) -> bool {
    state
        .active_screen()
        .and_then(|s| state.dom.get(s))
        .is_some_and(|n| n.has_class("--textual-command-palette"))
}

/// Gather commands from every source, in display order for discovery.
pub(crate) fn gather_commands(state: &mut AppState) -> Vec<Command> {
    let root = state.root;
    let screen = state.active_screen();
    let providers = state.ux.providers.clone();
    // The root widget may be checked out (palette opened from its own handler): then its
    // `commands()` are skipped, but providers still get a `Ctx` for the root.
    let mut out = state
        .dom
        .get(root)
        .and_then(|n| n.widget())
        .map(|w| w.commands())
        .unwrap_or_default();
    {
        let mut ctx = Ctx {
            app: state,
            node: root,
        };
        for p in &providers {
            out.extend(p.commands(&mut ctx));
        }
        if ctx.app.ux.system_commands {
            out.extend(SystemCommandsProvider.commands(&mut ctx));
        }
    }
    state.dirty = true;
    if let Some(screen) = screen.filter(|s| *s != root) {
        if let Some(w) = state.dom.get(screen).and_then(|n| n.widget()) {
            out.extend(w.commands());
        }
    }
    out
}

/// Push a command palette listing `commands`, asking `hooks` for more (no-op if one is open).
pub(crate) fn push_palette(
    state: &mut AppState,
    commands: Vec<Command>,
    placeholder: &str,
    hooks: Vec<SearchHook>,
) {
    if is_open(state) {
        return;
    }
    crate::tooltip::clear(state);
    let palette = crate::widgets::command_palette::CommandPalette::new(commands)
        .placeholder(placeholder)
        .search_hooks(hooks);
    let root = state.root;
    state.push_screen(
        root,
        Child::new(Box::new(palette)).id("--command-palette"),
        true,
    );
}

/// The `command_palette` action.
pub(crate) fn open_palette(state: &mut AppState) {
    if !state.ux.palette_enabled || is_open(state) {
        return;
    }
    let commands = gather_commands(state);
    let hooks = state.ux.search_hooks.clone();
    push_palette(state, commands, "Search for commands…", hooks);
}

/// The `change_theme` action: a palette listing every registered theme (Textual: `search_themes`).
pub(crate) fn open_theme_palette(state: &mut AppState) {
    let mut names: Vec<String> = state.themes.iter().map(|t| t.name.clone()).collect();
    names.sort();
    let commands = names
        .iter()
        .map(|n| Command::new(n, "", &format!("app.set_theme('{n}')")))
        .collect();
    push_palette(state, commands, "Search for themes…", Vec::new());
}

impl App {
    /// Register a command palette provider (Textual: `App.COMMANDS`).
    pub fn command_provider(mut self, provider: impl CommandProvider + 'static) -> Self {
        self.state.ux.providers.push(Rc::new(provider));
        self
    }

    /// Enable or disable the command palette and its `ctrl+p` binding
    /// (Textual: `ENABLE_COMMAND_PALETTE`). Enabled by default.
    pub fn enable_command_palette(mut self, enabled: bool) -> Self {
        self.state.ux.palette_enabled = enabled;
        self
    }

    /// Register a search that delivers hits later (see [`SearchHook`]); like Textual's async
    /// `Provider`s.
    pub fn command_search(
        mut self,
        hook: impl Fn(NodeId, SearchEvent<'_>, &mut Ctx) + 'static,
    ) -> Self {
        self.state.ux.search_hooks.push(Rc::new(hook));
        self
    }

    /// Include the built-in system commands (theme, quit, keys, ...) in the palette. On by
    /// default; turn off to provide them yourself.
    pub fn system_commands(mut self, enabled: bool) -> Self {
        self.state.ux.system_commands = enabled;
        self
    }
}

impl Ctx<'_> {
    /// Open the command palette (Textual: `App.action_command_palette`).
    pub fn open_command_palette(&mut self) {
        open_palette(self.app);
    }

    /// Open a command palette with the given commands only (Textual:
    /// `push_screen(CommandPalette(providers=...))`).
    pub fn open_command_palette_with(&mut self, commands: Vec<Command>, placeholder: &str) {
        push_palette(self.app, commands, placeholder, Vec::new());
    }

    /// Open a command palette with `commands` plus the hits of the registered
    /// [search hooks](App::command_search) (`builtin`: also every built-in source, as
    /// [`Ctx::open_command_palette`]).
    pub fn open_command_palette_searching(
        &mut self,
        commands: Vec<Command>,
        placeholder: &str,
        builtin: bool,
    ) {
        let mut commands = commands;
        if builtin {
            commands.extend(gather_commands(self.app));
        }
        let hooks = self.app.ux.search_hooks.clone();
        push_palette(self.app, commands, placeholder, hooks);
    }

    /// Deliver hits for a query of the palette `palette` (see [`SearchHook`]); hits for an
    /// older `generation` are dropped.
    pub fn add_command_hits(&mut self, palette: NodeId, generation: u64, hits: Vec<Command>) {
        use crate::widgets::command_palette::CommandPalette;
        if self.app.dom.widget::<CommandPalette>(palette).is_none() {
            // Not a palette, or a busy one (we are inside its own handler, e.g. a search hook
            // answering synchronously): keep the hits for the palette to pick up.
            if self.app.dom.get(palette).is_some() {
                self.app.ux.pending_hits.push((palette, generation, hits));
            }
            return;
        }
        self.with_id::<CommandPalette, _>(palette, |p, ctx| p.add_hits(generation, hits, ctx));
    }

    /// The built-in system commands for the current screen (theme, quit, keys, ...).
    pub fn system_commands(&mut self) -> Vec<Command> {
        SystemCommandsProvider.commands(self)
    }

    /// Open a palette listing the registered themes (Textual: `App.search_themes`).
    pub fn search_themes(&mut self) {
        open_theme_palette(self.app);
    }

    /// Whether the command palette is the active screen.
    pub fn is_command_palette_open(&self) -> bool {
        is_open(self.app)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substring_scores_higher_than_scattered() {
        let f = FuzzySearch::default();
        let (a, ao) = f.match_offsets("the", "Theme");
        let (b, _) = f.match_offsets("the", "Toggle hex mode");
        assert!(a > b, "{a} {b}");
        assert_eq!(ao, vec![0, 1, 2]);
        assert_eq!(f.match_offsets("xyz", "Theme").0, 0.0);
    }

    #[test]
    fn exact_match_doubles() {
        let f = FuzzySearch::default();
        let exact = f.match_offsets("quit", "quit").0;
        let sub = f.match_offsets("quit", "quit now").0;
        assert!(exact > sub);
    }

    #[test]
    fn first_letters_boost() {
        // "fn" matches the first letters of "Focus next".
        let f = FuzzySearch::default();
        let (s, o) = f.match_offsets("fn", "Focus next");
        assert_eq!(o, vec![0, 6]);
        assert!(s > f.match_offsets("fn", "Off and on").0);
    }

    #[test]
    fn highlight_marks_matches() {
        let t = Matcher::new("qt").highlight("Quit");
        assert_eq!(t.plain_text(), "Quit");
        assert_eq!(t.spans[0].text, "Q");
        assert_eq!(t.spans[0].style.reverse, Some(true));
        assert_eq!(t.spans.last().unwrap().text, "t");
    }
}
