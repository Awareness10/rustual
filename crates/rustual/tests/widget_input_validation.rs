//! `Input` / `MaskedInput` validation. Each expectation was observed with Textual 8.2.8 (same
//! widgets, same keys): which messages carry a `validation_result`, the `-valid` / `-invalid`
//! classes, `valid_empty`, `validate_on`, and the implicit validators of typed inputs.

use rustual::prelude::*;
use rustual::validation::{FailureKind, Function, Integer, Length, Number};

/// (message, input id, value, failure descriptions or `None` if validation didn't run)
type Logged = (&'static str, String, String, Option<Vec<String>>);

struct Form {
    children: Vec<Child>,
    log: Vec<Logged>,
}

impl Widget for Form {
    fn compose(&mut self) -> Vec<Child> {
        std::mem::take(&mut self.children)
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        let descriptions =
            |r: &Option<ValidationResult>| r.as_ref().map(|r| r.failure_descriptions());
        let id = |i: &Option<String>| i.clone().unwrap_or_default();
        if let Some(c) = m.get::<input::Changed>() {
            let entry = (
                "changed",
                id(&c.input_id),
                c.value.clone(),
                descriptions(&c.validation_result),
            );
            self.log.push(entry);
        }
        if let Some(s) = m.get::<input::Submitted>() {
            let entry = (
                "submitted",
                id(&s.input_id),
                s.value.clone(),
                descriptions(&s.validation_result),
            );
            self.log.push(entry);
        }
        if let Some(b) = m.get::<input::Blurred>() {
            let entry = (
                "blurred",
                id(&b.input_id),
                b.value.clone(),
                descriptions(&b.validation_result),
            );
            self.log.push(entry);
        }
        Flow::Continue
    }
}

async fn form(children: Vec<Child>) -> Pilot {
    App::new(Form {
        children,
        log: vec![],
    })
    .test(Size::new(60, 30))
    .await
}

/// The messages logged after the first `mark` ones.
fn since(p: &Pilot, mark: usize) -> Vec<Logged> {
    p.app::<Form>().unwrap().log[mark..].to_vec()
}

/// How many messages have been logged.
fn mark(p: &Pilot) -> usize {
    p.app::<Form>().unwrap().log.len()
}

fn entry(kind: &'static str, id: &str, value: &str, failures: Option<&[&str]>) -> Logged {
    (
        kind,
        id.to_string(),
        value.to_string(),
        failures.map(|f| f.iter().map(|s| s.to_string()).collect()),
    )
}

/// The `-valid` / `-invalid` classes of a node, sorted.
fn validity(p: &Pilot, sel: &str) -> Vec<&'static str> {
    let node = p.dom().get(p.query(sel)[0]).unwrap();
    ["-invalid", "-valid"]
        .into_iter()
        .filter(|c| node.has_class(c))
        .collect()
}

async fn focus(p: &mut Pilot, sel: &str) {
    let id = p.query(sel)[0];
    p.with_ctx(None, |ctx| ctx.focus(id));
    p.pause().await;
}

#[tokio::test]
async fn initial_values_are_validated_on_mount() {
    let p = form(compose![
        Input::new().value("abc").validator(Number::new()).id("a"),
        Input::new()
            .value("5")
            .input_type(InputType::Integer)
            .id("b"),
        Input::new()
            .validator(Length::new().minimum(2))
            .validate_on([ValidateOn::Submitted])
            .id("c"),
        Input::new()
            .validator(Length::new().minimum(2))
            .valid_empty(true)
            .id("d"),
        Input::new().value("x").id("e"),
    ])
    .await;
    // Textual: a ['-invalid'], b ['-valid'], c [], d [], e ['-valid'].
    assert_eq!(validity(&p, "#a"), ["-invalid"]);
    assert_eq!(validity(&p, "#b"), ["-valid"]);
    assert!(validity(&p, "#c").is_empty());
    assert!(validity(&p, "#d").is_empty());
    assert_eq!(validity(&p, "#e"), ["-valid"]);
    assert!(!p.query_one::<Input>("#a").unwrap().is_valid());
    assert!(p.query_one::<Input>("#c").unwrap().is_valid());
    assert_eq!(
        since(&p, 0),
        [
            entry("changed", "a", "abc", Some(&["Must be a valid number."])),
            entry("changed", "b", "5", Some(&[])),
            entry("changed", "e", "x", None),
        ]
    );
}

#[tokio::test]
async fn validate_on_submitted_only() {
    let mut p = form(compose![
        Input::new().id("first"),
        Input::new()
            .validator(Length::new().minimum(2))
            .validate_on([ValidateOn::Submitted])
            .id("c"),
        Button::new("other").id("other"),
    ])
    .await;
    focus(&mut p, "#c").await;
    let m = mark(&p);
    p.press(&["x"]).await;
    assert!(validity(&p, "#c").is_empty());
    p.press(&["enter"]).await;
    assert_eq!(validity(&p, "#c"), ["-invalid"]);
    focus(&mut p, "#other").await;
    assert_eq!(
        since(&p, m),
        [
            entry("changed", "c", "x", None),
            entry(
                "submitted",
                "c",
                "x",
                Some(&["Must be longer than 2 characters."])
            ),
            entry("blurred", "c", "x", None),
        ]
    );
}

#[tokio::test]
async fn valid_empty_skips_validators_and_can_change() {
    let mut p = form(compose![Input::new()
        .validator(Length::new().minimum(2))
        .valid_empty(true)
        .id("d")])
    .await;
    focus(&mut p, "#d").await;
    p.press(&["x"]).await;
    assert_eq!(validity(&p, "#d"), ["-invalid"]);
    p.press(&["backspace"]).await;
    assert_eq!(validity(&p, "#d"), ["-valid"]);
    assert_eq!(
        since(&p, 0),
        [
            entry(
                "changed",
                "d",
                "x",
                Some(&["Must be longer than 2 characters."])
            ),
            entry("changed", "d", "", None),
        ]
    );
    // Turning valid_empty off re-validates and posts Changed (Textual: _watch_valid_empty).
    let m = mark(&p);
    p.with::<Input, _>("#d", |i, ctx| i.set_valid_empty(false, ctx))
        .await;
    assert_eq!(validity(&p, "#d"), ["-invalid"]);
    assert!(!p.query_one::<Input>("#d").unwrap().is_valid_empty());
    assert_eq!(
        since(&p, m),
        [entry(
            "changed",
            "d",
            "",
            Some(&["Must be longer than 2 characters."])
        )]
    );
    // Setting the same value again does nothing.
    let m = mark(&p);
    p.with::<Input, _>("#d", |i, ctx| i.set_valid_empty(false, ctx))
        .await;
    assert!(since(&p, m).is_empty());
}

#[tokio::test]
async fn validate_runs_on_any_value_and_sets_classes() {
    let mut p = form(compose![
        Input::new().value("abc").validator(Number::new()).id("a"),
        Input::new().value("x").id("e"),
    ])
    .await;
    assert_eq!(validity(&p, "#a"), ["-invalid"]);
    let result = p
        .with::<Input, _>("#a", |i, ctx| i.validate_value("12", ctx))
        .await
        .unwrap();
    assert_eq!(result, Some(ValidationResult::success()));
    assert_eq!(validity(&p, "#a"), ["-valid"]);
    assert!(p.query_one::<Input>("#a").unwrap().is_valid());
    // Back to the real value.
    let result = p
        .with::<Input, _>("#a", |i, ctx| i.validate(ctx))
        .await
        .unwrap();
    assert_eq!(result.unwrap().failures[0].kind, FailureKind::NotANumber);
    assert_eq!(validity(&p, "#a"), ["-invalid"]);
    // No validators: None, and valid.
    let result = p
        .with::<Input, _>("#e", |i, ctx| i.validate_value("zz", ctx))
        .await;
    assert_eq!(result, Some(None));
    assert_eq!(validity(&p, "#e"), ["-valid"]);
}

#[tokio::test]
async fn typed_inputs_validate_until_validators_are_given() {
    let mut p = form(compose![
        Input::new().input_type(InputType::Integer).id("int"),
        Input::new().input_type(InputType::Number).id("num"),
        // Validators replace the implicit type validator (Textual adds it only without any).
        Input::new()
            .input_type(InputType::Integer)
            .validator(Length::new().maximum(1))
            .id("len"),
    ])
    .await;
    let set = |v: &'static str| move |i: &mut Input, ctx: &mut Ctx| i.set_value(v, ctx);
    p.with::<Input, _>("#int", set("1.5")).await;
    p.with::<Input, _>("#num", set("1.5")).await;
    p.with::<Input, _>("#len", set("12")).await;
    assert_eq!(
        since(&p, 0),
        [
            entry("changed", "int", "1.5", Some(&["Must be a valid integer."])),
            entry("changed", "num", "1.5", Some(&[])),
            entry(
                "changed",
                "len",
                "12",
                Some(&["Must be shorter than 1 characters."])
            ),
        ]
    );
    assert!(p
        .query_one::<Input>("#int")
        .unwrap()
        .get_validators()
        .is_empty());
    assert_eq!(
        p.query_one::<Input>("#len").unwrap().get_validators().len(),
        1
    );
}

#[tokio::test]
async fn validators_merge_in_order_and_messages_carry_failures() {
    let mut p = form(compose![Input::new()
        .validator(Length::new().minimum(3))
        .validator(
            Function::new(|v| v.chars().all(char::is_alphabetic))
                .failure_description("Letters only.")
        )
        .id("i")])
    .await;
    focus(&mut p, "#i").await;
    p.type_text("a1").await;
    p.press(&["enter"]).await;
    let log = since(&p, 0);
    assert_eq!(
        log.last().unwrap(),
        &entry(
            "submitted",
            "i",
            "a1",
            Some(&["Must be longer than 3 characters.", "Letters only."])
        )
    );
    assert_eq!(
        log[0],
        entry(
            "changed",
            "i",
            "a",
            Some(&["Must be longer than 3 characters."])
        )
    );
}

#[tokio::test]
async fn validate_on_and_validators_can_change_at_runtime() {
    let mut p = form(compose![Input::new().id("i")]).await;
    p.with::<Input, _>("#i", |i, _| {
        i.set_validators([Box::new(Integer::new()) as Box<dyn Validator>]);
        i.set_validate_on([ValidateOn::Blur]);
    })
    .await;
    let input = p.query_one::<Input>("#i").unwrap();
    assert!(input.validates_on(ValidateOn::Blur));
    assert!(!input.validates_on(ValidateOn::Changed));
    assert!(!input.validates_on(ValidateOn::Submitted));
    p.with::<Input, _>("#i", |i, ctx| i.set_value("x", ctx))
        .await;
    assert_eq!(since(&p, 0), [entry("changed", "i", "x", None)]);
    assert!(validity(&p, "#i").is_empty());
}

#[tokio::test]
async fn blur_validates() {
    let mut p = form(compose![
        Input::new().validator(Integer::new()).id("i"),
        Button::new("b").id("b"),
    ])
    .await;
    focus(&mut p, "#i").await;
    p.press(&["x"]).await;
    let m = mark(&p);
    focus(&mut p, "#b").await;
    assert_eq!(
        since(&p, m),
        [entry(
            "blurred",
            "i",
            "x",
            Some(&["Must be a valid integer."])
        )]
    );
}

// ---------------------------------------------------------------------------------- MaskedInput

#[tokio::test]
async fn masked_input_template_is_an_implicit_validator() {
    let mut p = form(compose![
        MaskedInput::new("999").valid_empty(true).id("a"),
        MaskedInput::new("(999)").id("b"),
        MaskedInput::new("99")
            .validator(Function::new(|v| v != "12").failure_description("not 12"))
            .id("c"),
        MaskedInput::new("99")
            .validate_on([ValidateOn::Submitted])
            .id("d"),
    ])
    .await;
    // Textual: a '' [] valid, b '(' ['-invalid'], c / d [] valid; only b posts Changed.
    assert!(validity(&p, "#a").is_empty());
    assert_eq!(validity(&p, "#b"), ["-invalid"]);
    assert!(!p.query_one::<MaskedInput>("#b").unwrap().is_valid());
    assert!(p.query_one::<MaskedInput>("#c").unwrap().is_valid());
    assert_eq!(
        since(&p, 0),
        [entry(
            "changed",
            "b",
            "(",
            Some(&["Value does not match template!"])
        )]
    );

    focus(&mut p, "#c").await;
    let m = mark(&p);
    p.press(&["1", "2", "enter"]).await;
    assert_eq!(validity(&p, "#c"), ["-invalid"]);
    assert_eq!(
        since(&p, m),
        [
            entry(
                "changed",
                "c",
                "1",
                Some(&["Value does not match template!"])
            ),
            entry("changed", "c", "12", Some(&["not 12"])),
            entry("submitted", "c", "12", Some(&["not 12"])),
        ]
    );

    focus(&mut p, "#d").await;
    let m = mark(&p);
    p.press(&["1", "enter"]).await;
    assert_eq!(validity(&p, "#d"), ["-invalid"]);
    assert_eq!(
        since(&p, m),
        [
            entry("changed", "d", "1", None),
            entry(
                "submitted",
                "d",
                "1",
                Some(&["Value does not match template!"])
            ),
        ]
    );

    // valid_empty doesn't exempt the template (Textual: a.validate("") fails).
    let result = p
        .with::<MaskedInput, _>("#a", |m, ctx| m.validate_value("", ctx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result.failure_descriptions(),
        ["Value does not match template!"]
    );
    assert_eq!(result.failures[0].validator, "Template");
    assert!(!p.query_one::<MaskedInput>("#a").unwrap().is_valid());
}

#[tokio::test]
async fn masked_input_set_template_revalidates() {
    let mut p = form(compose![MaskedInput::new("99").value("12").id("m")]).await;
    assert_eq!(validity(&p, "#m"), ["-valid"]);
    let m = mark(&p);
    p.with::<MaskedInput, _>("#m", |m, ctx| m.set_template("999", ctx).unwrap())
        .await;
    assert_eq!(validity(&p, "#m"), ["-invalid"]);
    assert_eq!(
        since(&p, m),
        [entry(
            "changed",
            "m",
            "12",
            Some(&["Value does not match template!"])
        )]
    );
}
