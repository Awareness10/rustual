//! A chat client: messages in a `layout: stream` log, assistant replies rendered as Markdown and
//! streamed in token by token (like an LLM client), input history, and the keys you'd expect.
//!
//! The assistant is simulated (canned replies picked by keyword: try "rust", "table" or
//! "plan"), so the example runs offline. Run with `cargo run -p rustual --example chat`.
//! Enter sends, up / down recall earlier prompts, escape stops a reply, `ctrl+l` clears,
//! `ctrl+n` starts a new chat, `ctrl+q` quits.

use rustual::prelude::*;
use rustual::widgets::input;
use std::collections::VecDeque;
use std::time::Duration;

const CSS: &str = "
#log { layout: stream; height: 1fr; padding: 0 1; }
.message { padding: 0 1; margin-top: 1; }
.user { margin-left: 16; background: $primary-muted; border-left: wide $primary; }
.assistant { margin-right: 8; background: $boost; border-left: wide $accent; }
.assistant.-streaming { border-left: wide $warning; }
#prompt { margin: 0 1 1 1; }
";

const WELCOME: &str =
    "**Hi!** I'm a simulated assistant. Ask me something — try *rust*, *table* or *plan*.";

/// A canned reply for a prompt.
fn reply(prompt: &str) -> String {
    let p = prompt.to_lowercase();
    if p.contains("rust") {
        "Here's a small Rust function that computes Fibonacci numbers iteratively:\n\n\
```rust\nfn fib(n: u64) -> u64 {\n    let (mut a, mut b) = (0, 1);\n    for _ in 0..n {\n        (a, b) = (b, a + b);\n    }\n    a\n}\n```\n\n\
It runs in **O(n)** time and constant space. `fib(10)` is `55`."
            .into()
    } else if p.contains("table") {
        "Some terminal UI libraries:\n\n\
| Library | Language | Style |\n|---|---|---|\n| Textual | Python | DOM + CSS |\n\
| rustual | Rust / Python | DOM + CSS |\n| Ratatui | Rust | immediate mode |\n\n\
*rustual* re-creates Textual's model in Rust."
            .into()
    } else if p.contains("plan") {
        "A plan for shipping a release:\n\n1. Freeze features on `dev`.\n2. Run the full test suite \
and fix what breaks.\n3. Update the **changelog**.\n4. Tag the release and let CI publish it.\n\n\
> Small, frequent releases are easier to review."
            .into()
    } else {
        format!(
            "You said:\n\n> {}\n\nI'm only a simulation, so I can't really answer — but I can \
show **Markdown**: *emphasis*, `inline code`, lists and more.",
            prompt.replace('\n', " ")
        )
    }
}

/// Split a reply into the chunks a streaming API would deliver (words with their spacing).
fn tokens(text: &str) -> VecDeque<String> {
    let mut out = VecDeque::new();
    let mut current = String::new();
    for c in text.chars() {
        current.push(c);
        if c.is_whitespace() {
            out.push_back(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push_back(current);
    }
    out
}

/// A reply being streamed into the Markdown widget `node`.
struct Streaming {
    node: NodeId,
    pending: VecDeque<String>,
    timer: TimerId,
}

struct Chat {
    history: Vec<String>,
    /// Position while browsing `history` with up / down (`None`: not browsing).
    history_index: Option<usize>,
    streaming: Option<Streaming>,
    messages: usize,
}

impl Chat {
    fn new() -> Self {
        Chat {
            history: Vec::new(),
            history_index: None,
            streaming: None,
            messages: 0,
        }
    }

    fn scroll_to_end(ctx: &mut Ctx) {
        if let Some(log) = ctx.query_id("#log") {
            ctx.scroll_node_to(log, 0, i32::MAX);
        }
    }

    fn status(&self, ctx: &mut Ctx) {
        let state = if self.streaming.is_some() {
            " · typing…"
        } else {
            ""
        };
        ctx.set_sub_title(&format!("{} messages{state}", self.messages));
    }

    fn send(&mut self, text: String, ctx: &mut Ctx) {
        self.stop(ctx);
        let Some(log) = ctx.query_id("#log") else {
            return;
        };
        self.history.push(text.clone());
        self.history_index = None;
        self.messages += 2;
        ctx.mount_to(
            log,
            Static::new(rustual::render::markup::escape(&text)).classes("message user"),
            None,
        );
        let node = ctx.mount_to(
            log,
            Child::from(Markdown::new("")).classes("message assistant -streaming"),
            None,
        );
        let timer = ctx.set_interval(Duration::from_millis(30), |w: &mut Chat, ctx| w.tick(ctx));
        self.streaming = Some(Streaming {
            node,
            pending: tokens(&reply(&text)),
            timer,
        });
        self.status(ctx);
        Self::scroll_to_end(ctx);
    }

    /// Deliver the next chunk of the reply being streamed.
    fn tick(&mut self, ctx: &mut Ctx) {
        let Some(stream) = self.streaming.as_mut() else {
            return;
        };
        match stream.pending.pop_front() {
            Some(chunk) => {
                let node = stream.node;
                ctx.with_id::<Markdown, _>(node, |m, ctx| m.append(&chunk, ctx));
                Self::scroll_to_end(ctx);
            }
            None => self.stop(ctx),
        }
    }

    /// Stop streaming (the reply so far stays).
    fn stop(&mut self, ctx: &mut Ctx) {
        if let Some(stream) = self.streaming.take() {
            ctx.cancel_timer(stream.timer);
            ctx.set_class_on(stream.node, false, "-streaming");
            if !stream.pending.is_empty() {
                ctx.with_id::<Markdown, _>(stream.node, |m, ctx| m.append(" *(stopped)*", ctx));
            }
        }
        self.status(ctx);
    }

    fn recall(&mut self, older: bool, ctx: &mut Ctx) {
        if self.history.is_empty() {
            return;
        }
        let last = self.history.len() - 1;
        self.history_index = match (self.history_index, older) {
            (None, true) => Some(last),
            (Some(i), true) => Some(i.saturating_sub(1)),
            (Some(i), false) if i < last => Some(i + 1),
            _ => None,
        };
        let text = self
            .history_index
            .map(|i| self.history[i].clone())
            .unwrap_or_default();
        ctx.with::<Input, _>("#prompt", |input, ctx| {
            input.set_value(text, ctx);
            input.action_end(false);
        });
    }
}

impl Widget for Chat {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Header::new(),
            VerticalScroll::new().id("log").with(compose![
                Child::from(Markdown::new(WELCOME)).classes("message assistant")
            ]),
            Input::new()
                .placeholder("Message the assistant… (enter to send)")
                .id("prompt"),
            Footer::new(),
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_title("Chat");
        self.messages = 1;
        self.status(ctx);
        if let Some(prompt) = ctx.query_id("#prompt") {
            ctx.focus(prompt);
        }
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("escape", "stop", "Stop"),
            Binding::new("ctrl+l", "clear", "Clear"),
            Binding::new("ctrl+n", "new_chat", "New chat"),
            Binding::new("up", "history(True)", "Previous").hidden(),
            Binding::new("down", "history(False)", "Next").hidden(),
            Binding::new("ctrl+q", "quit", "Quit").priority(),
        ]
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "stop" => self.stop(ctx),
            "clear" | "new_chat" => {
                self.stop(ctx);
                if let Some(log) = ctx.query_id("#log") {
                    ctx.remove_children(log);
                    if action == "new_chat" {
                        ctx.mount_to(
                            log,
                            Child::from(Markdown::new(WELCOME)).classes("message assistant"),
                            None,
                        );
                        self.history.clear();
                        ctx.notify("Started a new chat");
                    }
                }
                self.messages = usize::from(action == "new_chat");
                self.status(ctx);
            }
            "history" => self.recall(args.first().is_some_and(|a| a == "True"), ctx),
            _ => return false,
        }
        true
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(submitted) = msg.get::<input::Submitted>() {
            let text = submitted.value.trim().to_string();
            if !text.is_empty() {
                ctx.with::<Input, _>("#prompt", |i, ctx| i.clear(ctx));
                self.send(text, ctx);
            }
            return Flow::Stop;
        }
        Flow::Continue
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    App::new(Chat::new()).css(CSS).run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn markdowns(p: &Pilot) -> Vec<String> {
        p.query(".assistant")
            .into_iter()
            .filter_map(|id| {
                p.dom()
                    .widget::<Markdown>(id)
                    .map(|m| m.source().to_string())
            })
            .collect()
    }

    #[tokio::test(start_paused = true)]
    async fn sends_streams_stops_and_recalls() {
        let mut p = App::new(Chat::new()).css(CSS).test(Size::new(90, 30)).await;
        assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
        let footer = p
            .screen_text()
            .lines()
            .last()
            .unwrap_or_default()
            .to_string();
        assert!(
            footer.contains("esc Stop")
                && footer.contains("^l Clear")
                && footer.contains("^q Quit"),
            "{footer}"
        );

        p.type_text("show me some rust").await;
        p.press(&["enter"]).await;
        assert!(p.screen_text().contains("show me some rust"));
        // The reply streams in over time.
        p.wait(Duration::from_millis(90)).await;
        let partial = markdowns(&p)[1].clone();
        assert!(
            !partial.is_empty() && !partial.contains("fn fib"),
            "{partial:?}"
        );
        p.wait(Duration::from_secs(5)).await;
        assert!(markdowns(&p)[1].contains("fn fib(n: u64)"));
        assert!(p.screen_text().contains("4 messages") || p.screen_text().contains("3 messages"));

        // Escape stops a reply part way.
        p.type_text("make a plan").await;
        p.press(&["enter"]).await;
        p.wait(Duration::from_millis(60)).await;
        p.press(&["escape"]).await;
        p.wait(Duration::from_secs(5)).await;
        let stopped = markdowns(&p)[2].clone();
        assert!(
            stopped.ends_with("*(stopped)*") && !stopped.contains("changelog"),
            "{stopped:?}"
        );

        // Up recalls earlier prompts.
        p.press(&["up"]).await;
        assert_eq!(
            p.query_one::<Input>("#prompt").unwrap().get_value(),
            "make a plan"
        );
        p.press(&["up"]).await;
        assert_eq!(
            p.query_one::<Input>("#prompt").unwrap().get_value(),
            "show me some rust"
        );

        // Clear empties the log.
        p.press(&["ctrl+l"]).await;
        assert!(markdowns(&p).is_empty());
    }

    #[test]
    fn tokens_keep_the_text() {
        let text = reply("a table please");
        assert_eq!(tokens(&text).into_iter().collect::<String>(), text);
    }
}
