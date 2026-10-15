//! A small form: text and password inputs, a checkbox, a radio set, a switch and a submit
//! button. Run with `cargo run -p rustual --example form`; ctrl+q quits.

use rustual::prelude::*;

const CSS: &str = "
Screen {
    layout: vertical;
    overflow-y: auto;
    padding: 1 2;
}
.row {
    height: auto;
}
.row > Label {
    padding: 1 1 0 0;
    width: 12;
}
.row > Input {
    width: 1fr;
}
#plan {
    width: 30;
}
#submit {
    margin: 1 0;
}
#result {
    width: 1fr;
    height: auto;
    border: round $accent;
    padding: 0 1;
}
";

#[derive(Default)]
struct FormApp;

impl FormApp {
    fn summary(ctx: &mut Ctx) -> String {
        let name = ctx
            .query_one::<Input>("#name")
            .map(|i| i.get_value().to_string())
            .unwrap_or_default();
        let password_len = ctx
            .query_one::<Input>("#password")
            .map(|i| i.get_value().chars().count())
            .unwrap_or(0);
        let newsletter = ctx
            .query_one::<Checkbox>("#newsletter")
            .map(|c| c.get_value())
            .unwrap_or(false);
        let plan_id = ctx.query_id("#plan");
        let plan = plan_id
            .and_then(|id| {
                let set = ctx.dom().widget::<RadioSet>(id)?;
                let pressed = set.pressed_button()?;
                ctx.dom()
                    .widget::<RadioButton>(pressed)
                    .map(|b| b.plain_label())
            })
            .unwrap_or_else(|| "(none)".into());
        let dark = ctx
            .query_one::<Switch>("#dark")
            .map(|s| s.get_value())
            .unwrap_or(false);
        format!(
            "name: {name}\npassword: {} chars\nnewsletter: {newsletter}\nplan: {plan}\ndark mode: {dark}",
            password_len
        )
    }
}

impl Widget for FormApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            HorizontalGroup::new().classes("row").with(compose![
                Label::new("Name"),
                Input::new().placeholder("Your name").id("name"),
            ]),
            HorizontalGroup::new().classes("row").with(compose![
                Label::new("Password"),
                Input::new()
                    .placeholder("Password")
                    .password(true)
                    .id("password"),
            ]),
            HorizontalGroup::new().classes("row").with(compose![
                Label::new("Plan"),
                RadioSet::new(["Free", "Pro", "Enterprise"]).id("plan"),
            ]),
            HorizontalGroup::new().classes("row").with(compose![
                Label::new("Options"),
                Checkbox::new("Subscribe to newsletter").id("newsletter"),
                Switch::new().value(true).id("dark"),
            ]),
            Button::primary("Submit").id("submit"),
            Static::new("Fill in the form and press Submit").id("result"),
        ]
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(c) = msg.get::<switch::Changed>() {
            ctx.set_theme(if c.value {
                "textual-dark"
            } else {
                "textual-light"
            });
        }
        let submitted = msg.is::<button::Pressed>() || msg.is::<input::Submitted>();
        if submitted {
            let text = Self::summary(ctx);
            if let Some(s) = ctx.query_one::<Static>("#result") {
                s.update_plain(text);
            }
        }
        Flow::Continue
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    App::new(FormApp).css(CSS).run().await
}
