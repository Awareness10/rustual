//! `CommandPalette`, `CommandList` and `SearchIcon` (port of the widgets in
//! `textual/command.py`): the modal screen opened with `ctrl+p` (see [`crate::command`]).
//!
//! An input at the top filters the commands with the fuzzy [`Matcher`]; matches are listed
//! best first with the matched letters highlighted (`command-palette--highlight`) and the help
//! text dimmed (`command-palette--help-text`). `up`/`down`/`pageup`/`pagedown` move the
//! highlight, `enter` or a click runs the command, `escape` or a click outside closes the
//! palette.
//!
//! Deviations: matching is synchronous (no busy indicator / batching); "No matches found" is
//! shown immediately; `run_on_select` is always on.

use crate::app::Pending;
use crate::binding::Binding;
use crate::command::{Command, CommandAction, Matcher};
use crate::context::Ctx;
use crate::events::Event;
use crate::message::{Flow, Message};
use crate::widget::{Child, Content, RenderCtx, Widget, WidgetExt};
use crate::widgets::input::{self, Input};
use crate::widgets::option_list::{self, OptionItem, OptionList};
use crate::widgets::{Horizontal, Vertical};
use std::time::Duration;
use textual_render::{Size, Style, Text};

const PALETTE_CSS: &str = "
CommandPalette {
    color: $foreground;
    background: $background 60%;
    align-horizontal: center;
}
CommandPalette > .command-palette--help-text {
    text-style: dim not bold;
}
CommandPalette > .command-palette--highlight {
    text-style: bold underline;
}
CommandPalette > Vertical {
    margin-top: 3;
    height: 100%;
    visibility: hidden;
    background: $surface;
    &:dark { background: $panel-darken-1; }
}
CommandPalette #--input {
    height: auto;
    visibility: visible;
    border: hkey black 50%;
}
CommandPalette #--input.--list-visible {
    border-bottom: none;
}
CommandPalette #--results {
    height: auto;
}
CommandPalette #--input Input, CommandPalette #--input Input:focus {
    border: blank;
    width: 1fr;
    padding-left: 0;
    background: transparent;
    background-tint: transparent;
}
";

const COMMAND_LIST_CSS: &str = "
CommandList {
    visibility: hidden;
    border-top: blank;
    border-bottom: hkey black;
    border-left: none;
    border-right: none;
    height: auto;
    max-height: 70vh;
    background: transparent;
    padding: 0;
    overflow-x: hidden;
    overflow-y: auto;
    color: $foreground;
}
CommandList.--visible {
    visibility: visible;
}
CommandList.--populating {
    border-bottom: none;
}
CommandList > .option-list--option {
    color: $foreground;
    text-style: bold;
}
CommandList > .option-list--option-highlighted {
    color: $block-cursor-blurred-foreground;
    background: $block-cursor-blurred-background;
    text-style: $block-cursor-blurred-text-style;
}
CommandList > .option-list--option-disabled {
    color: $text-disabled;
}
CommandList > .option-list--option-hover {
    background: $block-hover-background;
}
CommandList > .option-list--separator {
    color: $foreground 15%;
}
";

const SEARCH_ICON_CSS: &str = "
SearchIcon {
    color: #000;
    margin-left: 1;
    margin-top: 1;
    width: 2;
    height: 1;
}
";

/// The id of the disabled option shown when nothing matches.
const NO_MATCHES: &str = "--no-matches";

/// Posted to the app root when a command palette opens (Textual: `CommandPalette.Opened`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opened;

/// Posted to the app root when a command palette closes (Textual: `CommandPalette.Closed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Closed {
    /// Whether a command was chosen.
    pub option_selected: bool,
}

/// The search icon left of the palette input.
#[derive(Debug, Clone)]
pub struct SearchIcon {
    icon: String,
}

impl Default for SearchIcon {
    fn default() -> Self {
        SearchIcon {
            icon: "🔎".to_string(),
        }
    }
}

impl Widget for SearchIcon {
    fn type_names(&self) -> &'static [&'static str] {
        &["SearchIcon"]
    }

    fn default_css(&self) -> &'static str {
        SEARCH_ICON_CSS
    }

    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::Text(Text::plain(self.icon.clone()))
    }
}

/// The palette's list of matching commands: a non-focusable [`OptionList`].
#[derive(Debug, Clone, Default)]
pub struct CommandList {
    list: OptionList,
}

impl CommandList {
    pub fn new() -> Self {
        CommandList::default()
    }

    /// The underlying option list.
    pub fn options(&self) -> &OptionList {
        &self.list
    }

    pub fn options_mut(&mut self) -> &mut OptionList {
        &mut self.list
    }

    /// Prompts of the listed options as plain text.
    pub fn prompts(&self) -> Vec<String> {
        self.list
            .options()
            .iter()
            .map(|o| o.plain_prompt())
            .collect()
    }
}

impl Widget for CommandList {
    fn type_names(&self) -> &'static [&'static str] {
        &["CommandList"]
    }

    fn default_css(&self) -> &'static str {
        COMMAND_LIST_CSS
    }

    fn can_focus(&self) -> bool {
        false
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        self.list.run_action(action, args, ctx)
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        self.list.on_event(event, ctx)
    }

    fn virtual_size(&self, ctx: &RenderCtx) -> Option<Size> {
        self.list.virtual_size(ctx)
    }

    fn content_width(&self, ctx: &RenderCtx) -> Option<i32> {
        self.list.content_width(ctx)
    }

    fn content_height(&self, ctx: &RenderCtx, width: i32) -> Option<i32> {
        self.list.content_height(ctx, width)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        self.list.render(ctx)
    }
}

/// The command palette screen (pushed as a modal by the `command_palette` action).
#[derive(Debug, Clone)]
pub struct CommandPalette {
    commands: Vec<Command>,
    placeholder: String,
    /// Indices into `commands` of the listed hits, in display order.
    hits: Vec<usize>,
    list_visible: bool,
}

impl CommandPalette {
    pub fn new(commands: Vec<Command>) -> Self {
        CommandPalette {
            commands,
            placeholder: "Search for commands…".to_string(),
            hits: Vec::new(),
            list_visible: false,
        }
    }

    pub fn placeholder(mut self, placeholder: &str) -> Self {
        self.placeholder = placeholder.to_string();
        self
    }

    /// All commands offered by this palette.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// Names of the commands currently listed, best match first.
    pub fn hit_names(&self) -> Vec<&str> {
        self.hits
            .iter()
            .map(|i| self.commands[*i].name.as_str())
            .collect()
    }

    fn list_id(ctx: &Ctx) -> Option<crate::dom::NodeId> {
        ctx.query_in(ctx.id(), "CommandList").into_iter().next()
    }

    /// Text attributes (no colors) of a component class of the palette.
    fn component_attrs(ctx: &Ctx, class: &str) -> Style {
        let rc = RenderCtx::new(&ctx.app.dom, &ctx.app.env, ctx.id(), Size::new(1, 1));
        let s = rc.component_style(class);
        Style {
            fg: None,
            bg: None,
            link: None,
            ..s
        }
    }

    fn prompt(&self, command: &Command, matcher: Option<&Matcher>, help_style: &Style) -> Text {
        let mut text = Text::plain("  ");
        text.append(match matcher {
            Some(m) => m.highlight(&command.name),
            None => Text::plain(command.name.clone()),
        });
        if !command.help.is_empty() {
            text.push("\n  ", Style::new());
            let mut help = Text::from_markup(&command.help);
            help.stylize_base(help_style);
            text.append(help);
        }
        text
    }

    /// Re-filter the commands for `query` and refill the list (Textual: `_gather_commands`).
    fn refresh_list(&mut self, query: &str, ctx: &mut Ctx) {
        let help_style = Self::component_attrs(ctx, "command-palette--help-text");
        let options: Vec<OptionItem>;
        if query.is_empty() {
            self.hits = (0..self.commands.len())
                .filter(|i| self.commands[*i].discover)
                .collect();
            options = self
                .hits
                .iter()
                .map(|i| OptionItem::text(self.prompt(&self.commands[*i], None, &help_style)))
                .collect();
        } else {
            let matcher = Matcher::new(query)
                .match_style(Self::component_attrs(ctx, "command-palette--highlight"));
            let mut scored: Vec<(f64, usize)> = self
                .commands
                .iter()
                .enumerate()
                .map(|(i, c)| (matcher.score(&c.name), i))
                .filter(|(s, _)| *s > 0.0)
                .collect();
            scored.sort_by(|a, b| b.0.total_cmp(&a.0));
            self.hits = scored.into_iter().map(|(_, i)| i).collect();
            options = self
                .hits
                .iter()
                .map(|i| {
                    OptionItem::text(self.prompt(&self.commands[*i], Some(&matcher), &help_style))
                })
                .collect();
        }
        let no_matches = options.is_empty() && !query.is_empty();
        let Some(list) = Self::list_id(ctx) else {
            return;
        };
        ctx.with_id::<CommandList, _>(list, |l, ctx| {
            l.list.clear_options(ctx);
            if no_matches {
                let _ = l.list.add_option(
                    OptionItem::new("No matches found")
                        .id(NO_MATCHES)
                        .disabled(true),
                );
            } else {
                let _ = l.list.add_options(options);
                if l.list.option_count() > 0 {
                    l.list.set_highlighted(Some(0), ctx);
                }
            }
        });
        let visible = no_matches || !self.hits.is_empty();
        self.set_list_visible(visible, ctx);
    }

    fn set_list_visible(&mut self, visible: bool, ctx: &mut Ctx) {
        self.list_visible = visible;
        if let Some(list) = Self::list_id(ctx) {
            ctx.set_class_on(list, visible, "--visible");
        }
        if let Some(input) = ctx.query_in(ctx.id(), "#--input").into_iter().next() {
            ctx.set_class_on(input, visible, "--list-visible");
        }
    }

    fn with_list<R>(
        &self,
        ctx: &mut Ctx,
        f: impl FnOnce(&mut OptionList, &mut Ctx) -> R,
    ) -> Option<R> {
        let list = Self::list_id(ctx)?;
        ctx.with_id::<CommandList, _>(list, |l, ctx| f(&mut l.list, ctx))
    }

    fn highlighted(&self, ctx: &Ctx) -> Option<usize> {
        let list = Self::list_id(ctx)?;
        ctx.dom().widget::<CommandList>(list)?.list.highlighted()
    }

    /// Close without running anything (Textual: `_action_escape`).
    pub fn action_escape(&mut self, ctx: &mut Ctx) {
        let root = ctx.root();
        ctx.post_message_to(
            root,
            Closed {
                option_selected: false,
            },
        );
        ctx.pop_screen();
    }

    /// Run the listed command at `index` after closing the palette.
    fn run_hit(&mut self, index: usize, ctx: &mut Ctx) {
        let Some(command) = self.hits.get(index).map(|i| self.commands[*i].clone()) else {
            return;
        };
        let root = ctx.root();
        ctx.post_message_to(
            root,
            Closed {
                option_selected: true,
            },
        );
        ctx.pop_screen();
        match command.action {
            CommandAction::Action(action) => {
                ctx.app.queue.push_back(Pending::Action {
                    target: root,
                    action,
                });
            }
            CommandAction::Callback(f) => {
                ctx.app
                    .add_timer(root, Duration::ZERO, false, Box::new(move |_w, ctx| f(ctx)));
            }
        }
    }

    /// Enter in the input (Textual: `_select_or_command`).
    fn select_or_command(&mut self, ctx: &mut Ctx) {
        if !self.list_visible {
            return;
        }
        match self.highlighted(ctx) {
            Some(h) => {
                if !self.hits.is_empty() {
                    self.run_hit(h, ctx);
                }
            }
            None => {
                self.action_cursor_down(ctx);
                if self.hits.len() == 1 {
                    self.run_hit(0, ctx);
                }
            }
        }
    }

    /// Down: show the list if hidden, else move the highlight (Textual: `_action_cursor_down`).
    pub fn action_cursor_down(&mut self, ctx: &mut Ctx) {
        if self.hits.is_empty() {
            return;
        }
        if !self.list_visible {
            self.set_list_visible(true, ctx);
            self.with_list(ctx, |l, ctx| l.set_highlighted(Some(0), ctx));
        } else {
            self.with_list(ctx, |l, ctx| l.action_cursor_down(ctx));
        }
    }
}

impl Widget for CommandPalette {
    fn type_names(&self) -> &'static [&'static str] {
        &["CommandPalette"]
    }

    fn default_css(&self) -> &'static str {
        PALETTE_CSS
    }

    fn compose(&mut self) -> Vec<Child> {
        vec![Vertical::new().id("--container").with([
            Horizontal::new().id("--input").with([
                Child::new(Box::new(SearchIcon::default())),
                Child::new(Box::new(
                    Input::new()
                        .placeholder(self.placeholder.clone())
                        .select_on_focus(false),
                ))
                .id("--command-input"),
            ]),
            Vertical::new()
                .id("--results")
                .with([Child::new(Box::new(CommandList::new())).id("--command-list")]),
        ])]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.add_class("--textual-command-palette");
        let root = ctx.root();
        ctx.post_message_to(root, Opened);
        self.refresh_list("", ctx);
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("ctrl+end,shift+end", "command_list('last')", "Go to bottom").hidden(),
            Binding::new("ctrl+home,shift+home", "command_list('first')", "Go to top").hidden(),
            Binding::new("down", "cursor_down", "Next command").hidden(),
            Binding::new("escape", "escape", "Exit the command palette"),
            Binding::new("pagedown", "command_list('page_down')", "Next page").hidden(),
            Binding::new("pageup", "command_list('page_up')", "Previous page").hidden(),
            Binding::new("up", "command_list('cursor_up')", "Previous command").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "escape" => self.action_escape(ctx),
            "cursor_down" => self.action_cursor_down(ctx),
            "command_list" => {
                let name = args.first().cloned().unwrap_or_default();
                self.with_list(ctx, |l, ctx| match name.as_str() {
                    "first" => l.action_first(ctx),
                    "last" => l.action_last(ctx),
                    "cursor_up" => l.action_cursor_up(ctx),
                    "cursor_down" => l.action_cursor_down(ctx),
                    "page_up" => l.action_page_up(ctx),
                    "page_down" => l.action_page_down(ctx),
                    "select" => l.action_select(ctx),
                    _ => {}
                });
            }
            _ => return false,
        }
        true
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        // A click on the palette's background (outside the input and list) closes it.
        if let Event::Click(m) = event {
            if ctx.app.node_at(m.screen_x, m.screen_y) == Some(ctx.id()) {
                self.action_escape(ctx);
                return Flow::Stop;
            }
        }
        Flow::Continue
    }

    fn on_message(&mut self, message: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(changed) = message.get::<input::Changed>() {
            let query = changed.value.trim().to_string();
            self.refresh_list(&query, ctx);
            return Flow::Stop;
        }
        if message.is::<input::Submitted>() {
            self.select_or_command(ctx);
            return Flow::Stop;
        }
        if let Some(selected) = message.get::<option_list::OptionSelected>() {
            if selected.option_id.as_deref() != Some(NO_MATCHES) {
                self.run_hit(selected.option_index, ctx);
            }
            return Flow::Stop;
        }
        if message.is::<option_list::OptionHighlighted>() {
            return Flow::Stop;
        }
        Flow::Continue
    }
}
