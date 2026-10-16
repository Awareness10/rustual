//! A task manager: projects in a sidebar, tasks in a table, a modal form to add and edit tasks
//! (with validation), a confirmation dialog for deleting, a filter, palette commands, and
//! tasks saved to a file between runs.
//!
//! Run with `cargo run -p rustual --example tasks [file]` (default: `rustual-tasks.tsv` in the
//! temp directory). The footer lists the keys: `n` new, `e` / enter edit, space toggles done,
//! delete removes, `/` filters, `ctrl+s` saves, `ctrl+q` quits; `ctrl+p` has more commands.

use rustual::prelude::*;
use rustual::widgets::data_table::RowSelected;
use rustual::widgets::{input, list_view};
use std::path::{Path, PathBuf};

const CSS: &str = "
#body { height: 1fr; }
#projects { width: 26; border: round $primary; }
#main { width: 1fr; }
#filter { display: none; margin: 0 1; }
#filter.-visible { display: block; }
#tasks { height: 1fr; border: round $secondary; }
#empty { display: none; height: 1fr; content-align: center middle; color: $text-muted; }
#empty.-visible { display: block; }
";

const PROJECTS: [(&str, &str); 4] = [
    ("all", "All tasks"),
    ("inbox", "Inbox"),
    ("work", "Work"),
    ("home", "Home"),
];

// ------------------------------------------------------------------------------------ model

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Priority {
    Low,
    Normal,
    High,
}

impl Priority {
    const ALL: [Priority; 3] = [Priority::High, Priority::Normal, Priority::Low];

    fn name(self) -> &'static str {
        match self {
            Priority::Low => "low",
            Priority::Normal => "normal",
            Priority::High => "high",
        }
    }

    fn parse(s: &str) -> Priority {
        match s {
            "high" => Priority::High,
            "low" => Priority::Low,
            _ => Priority::Normal,
        }
    }

    fn markup(self) -> &'static str {
        match self {
            Priority::High => "[b $error]high[/]",
            Priority::Normal => "normal",
            Priority::Low => "[$text-muted]low[/]",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Task {
    id: u32,
    title: String,
    project: String,
    priority: Priority,
    due: String,
    done: bool,
}

/// Tab-separated, one task per line (`id, done, priority, project, due, title`).
fn load(path: &Path) -> Option<Vec<Task>> {
    let text = std::fs::read_to_string(path).ok()?;
    let tasks = text
        .lines()
        .filter_map(|line| {
            let mut f = line.splitn(6, '\t');
            Some(Task {
                id: f.next()?.parse().ok()?,
                done: f.next()? == "1",
                priority: Priority::parse(f.next()?),
                project: f.next()?.to_string(),
                due: f.next()?.to_string(),
                title: f.next()?.to_string(),
            })
        })
        .collect();
    Some(tasks)
}

fn save(path: &Path, tasks: &[Task]) -> std::io::Result<()> {
    let text: String = tasks
        .iter()
        .map(|t| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                t.id,
                u8::from(t.done),
                t.priority.name(),
                t.project,
                t.due,
                t.title.replace(['\t', '\n'], " ")
            )
        })
        .collect();
    std::fs::write(path, text)
}

fn sample_tasks() -> Vec<Task> {
    let task = |id, title: &str, project: &str, priority, due: &str, done| Task {
        id,
        title: title.into(),
        project: project.into(),
        priority,
        due: due.into(),
        done,
    };
    vec![
        task(
            1,
            "Write the release notes",
            "work",
            Priority::High,
            "2026-10-20",
            false,
        ),
        task(
            2,
            "Review the layout PR",
            "work",
            Priority::Normal,
            "2026-10-18",
            false,
        ),
        task(
            3,
            "Answer support email",
            "inbox",
            Priority::Normal,
            "",
            false,
        ),
        task(
            4,
            "Book flights for the conference",
            "inbox",
            Priority::High,
            "2026-11-02",
            false,
        ),
        task(5, "Water the plants", "home", Priority::Low, "", true),
        task(
            6,
            "Fix the leaking tap",
            "home",
            Priority::Normal,
            "2026-10-25",
            false,
        ),
        task(
            7,
            "Update the benchmark numbers",
            "work",
            Priority::Low,
            "",
            true,
        ),
    ]
}

/// `YYYY-MM-DD` with a plausible month and day (empty is fine: no due date).
fn valid_date(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let parts: Vec<&str> = s.split('-').collect();
    let [y, m, d] = parts.as_slice() else {
        return false;
    };
    let num = |p: &str, len: usize| (p.len() == len).then(|| p.parse::<u32>().ok()).flatten();
    matches!(
        (num(y, 4), num(m, 2), num(d, 2)),
        (Some(_), Some(1..=12), Some(1..=31))
    )
}

// ------------------------------------------------------------------------------- task form

/// What the task form returns: the edited task, or `None` if cancelled.
struct FormResult(Option<Task>);

/// A modal form to add or edit a task.
struct TaskForm {
    task: Task,
    is_new: bool,
}

impl TaskForm {
    fn submit(&mut self, ctx: &mut Ctx) {
        let title = ctx
            .query_one::<Input>("#title")
            .map(|i| i.get_value().trim().to_string())
            .unwrap_or_default();
        let due = ctx
            .query_one::<Input>("#due")
            .map(|i| i.get_value().trim().to_string())
            .unwrap_or_default();
        let mut errors = Vec::new();
        if title.is_empty() {
            errors.push("The title can't be empty.");
        }
        if !valid_date(&due) {
            errors.push("The due date must look like 2026-10-31.");
        }
        for (id, bad) in [("#title", title.is_empty()), ("#due", !valid_date(&due))] {
            if let Some(node) = ctx.query_id(id) {
                ctx.set_class_on(node, bad, "-invalid");
            }
        }
        if !errors.is_empty() {
            ctx.with::<Label, _>("#error", |l, _| l.update(errors.join(" ")));
            return;
        }
        let project = ctx
            .query_one::<Select<String>>("#project")
            .and_then(|s| s.get_value().cloned())
            .unwrap_or_else(|| "inbox".into());
        let priority = ctx
            .query_one::<Select<String>>("#priority")
            .and_then(|s| s.get_value().cloned())
            .map_or(Priority::Normal, |p| Priority::parse(&p));
        let task = Task {
            title,
            due,
            project,
            priority,
            ..self.task.clone()
        };
        ctx.dismiss(FormResult(Some(task)));
    }
}

impl Widget for TaskForm {
    fn type_names(&self) -> &'static [&'static str] {
        &["TaskForm", "ModalScreen", "Screen"]
    }

    fn default_css(&self) -> &'static str {
        "
TaskForm { align: center middle; }
#dialog {
    width: 64; height: auto; padding: 1 2;
    border: thick $accent; background: $surface;
}
#dialog Label { margin-top: 1; }
#heading { text-style: bold; margin-top: 0; }
#error { color: $text-error; height: auto; }
#buttons { height: auto; align-horizontal: right; margin-top: 1; }
#buttons Button { margin-left: 2; }
"
    }

    fn compose(&mut self) -> Vec<Child> {
        let heading = if self.is_new { "New task" } else { "Edit task" };
        let projects: Vec<(String, String)> = PROJECTS[1..]
            .iter()
            .map(|(id, name)| (name.to_string(), id.to_string()))
            .collect();
        let priorities: Vec<(String, String)> = Priority::ALL
            .iter()
            .map(|p| (p.name().to_string(), p.name().to_string()))
            .collect();
        compose![Vertical::new().id("dialog").with(compose![
            Label::new(heading).id("heading"),
            Label::new("Title"),
            Input::new()
                .value(self.task.title.clone())
                .placeholder("What needs doing?")
                .id("title"),
            Label::new("Project"),
            Select::new(projects)
                .allow_blank(false)
                .value(self.task.project.clone())
                .id("project"),
            Label::new("Priority"),
            Select::new(priorities)
                .allow_blank(false)
                .value(self.task.priority.name().to_string())
                .id("priority"),
            Label::new("Due date"),
            Input::new()
                .value(self.task.due.clone())
                .placeholder("YYYY-MM-DD (optional)")
                .restrict_chars("0123456789-")
                .max_length(10)
                .id("due"),
            Label::new("").id("error"),
            Horizontal::new().id("buttons").with(compose![
                Button::new("Cancel").id("cancel"),
                Button::primary("Save").id("save"),
            ]),
        ])]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("escape", "cancel", "Cancel"),
            Binding::new("ctrl+s", "submit", "Save"),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "cancel" => ctx.dismiss(FormResult(None)),
            "submit" => self.submit(ctx),
            _ => return false,
        }
        true
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.get::<button::Pressed>().is_some() {
            match msg.sender_id() {
                Some("save") => self.submit(ctx),
                Some("cancel") => ctx.dismiss(FormResult(None)),
                _ => {}
            }
            return Flow::Stop;
        }
        if msg.is::<input::Submitted>() {
            self.submit(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }
}

// ---------------------------------------------------------------------------- confirmation

/// The answer of a [`Confirm`] dialog.
struct Confirmed(bool);

/// A yes / no question in a modal dialog.
struct Confirm {
    question: String,
}

impl Widget for Confirm {
    fn type_names(&self) -> &'static [&'static str] {
        &["Confirm", "ModalScreen", "Screen"]
    }

    fn default_css(&self) -> &'static str {
        "
Confirm { align: center middle; }
#confirm { width: 50; height: auto; padding: 1 2; border: thick $error; background: $surface; }
#confirm Horizontal { height: auto; align-horizontal: right; margin-top: 1; }
#confirm Button { margin-left: 2; }
"
    }

    fn compose(&mut self) -> Vec<Child> {
        compose![Vertical::new().id("confirm").with(compose![
            Label::new(self.question.clone()),
            Horizontal::new().with(compose![
                Button::new("No").id("no"),
                Button::error("Delete").id("yes"),
            ]),
        ])]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("y", "answer(True)", "Delete"),
            Binding::new("n,escape", "answer(False)", "Keep"),
        ]
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        if action == "answer" {
            ctx.dismiss(Confirmed(args.first().is_some_and(|a| a == "True")));
            return true;
        }
        false
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.get::<button::Pressed>().is_some() {
            ctx.dismiss(Confirmed(msg.sender_id() == Some("yes")));
            return Flow::Stop;
        }
        Flow::Continue
    }
}

// ------------------------------------------------------------------------------------- app

struct TasksApp {
    path: PathBuf,
    tasks: Vec<Task>,
    project: String,
    filter: String,
    /// The task a pending confirmation is about.
    deleting: Option<u32>,
    dirty: bool,
}

impl TasksApp {
    fn new(path: PathBuf) -> Self {
        let tasks = load(&path).unwrap_or_else(sample_tasks);
        TasksApp {
            path,
            tasks,
            project: "all".into(),
            filter: String::new(),
            deleting: None,
            dirty: false,
        }
    }

    fn visible(&self) -> Vec<&Task> {
        let filter = self.filter.to_lowercase();
        let mut tasks: Vec<&Task> = self
            .tasks
            .iter()
            .filter(|t| self.project == "all" || t.project == self.project)
            .filter(|t| filter.is_empty() || t.title.to_lowercase().contains(&filter))
            .collect();
        // Open tasks first, then by priority and due date.
        tasks.sort_by(|a, b| {
            (
                a.done,
                std::cmp::Reverse(a.priority),
                a.due.is_empty(),
                &a.due,
            )
                .cmp(&(
                    b.done,
                    std::cmp::Reverse(b.priority),
                    b.due.is_empty(),
                    &b.due,
                ))
        });
        tasks
    }

    /// The task under the table cursor.
    fn current(&self, ctx: &mut Ctx) -> Option<u32> {
        let table = ctx.query_one::<DataTable>("#tasks")?;
        let row = table.cursor_row();
        let key = table.row_keys().get(row)?.0.clone();
        key.strip_prefix('t')?.parse().ok()
    }

    /// Rebuild the table, the project counts and the sub-title.
    fn refresh(&self, ctx: &mut Ctx) {
        let rows: Vec<(String, Vec<CellValue>)> = self
            .visible()
            .into_iter()
            .map(|t| {
                let check = if t.done { "[$success]✓[/]" } else { "·" };
                let title = if t.done {
                    format!(
                        "[strike $text-muted]{}[/]",
                        rustual::render::markup::escape(&t.title)
                    )
                } else {
                    rustual::render::markup::escape(&t.title)
                };
                let project = PROJECTS
                    .iter()
                    .find(|(id, _)| *id == t.project)
                    .map_or(t.project.as_str(), |(_, name)| name);
                (
                    format!("t{}", t.id),
                    vec![
                        CellValue::from(check),
                        title.into(),
                        project.into(),
                        t.priority.markup().into(),
                        t.due.as_str().into(),
                    ],
                )
            })
            .collect();
        let empty = rows.is_empty();
        ctx.with::<DataTable, _>("#tasks", |table, ctx| {
            let cursor = table.cursor_row();
            table.clear(false);
            for (key, cells) in rows {
                let _ = table.add_row_with(cells, Some(1), Some(&key), None);
            }
            let last = table.row_count().saturating_sub(1);
            table.move_cursor(Some(cursor.min(last)), None, ctx);
        });
        for id in ["#tasks", "#empty"] {
            if let Some(node) = ctx.query_id(id) {
                ctx.set_class_on(node, (id == "#empty") == empty, "-visible");
                ctx.set_display(node, (id == "#empty") == empty);
            }
        }
        for (id, name) in PROJECTS {
            let open = self
                .tasks
                .iter()
                .filter(|t| !t.done && (id == "all" || t.project == id))
                .count();
            let label = format!("{name} [$text-muted]({open})[/]");
            ctx.with::<Label, _>(&format!("#project-{id}"), |l, _| l.update(label));
        }
        let open = self.tasks.iter().filter(|t| !t.done).count();
        let unsaved = if self.dirty { " · unsaved" } else { "" };
        ctx.set_sub_title(&format!("{open} open of {}{unsaved}", self.tasks.len()));
    }

    fn edit(&mut self, id: Option<u32>, ctx: &mut Ctx) {
        let (task, is_new) = match id.and_then(|id| self.tasks.iter().find(|t| t.id == id)) {
            Some(t) => (t.clone(), false),
            None => {
                let next = self.tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                let project = if self.project == "all" {
                    "inbox"
                } else {
                    &self.project
                };
                let task = Task {
                    id: next,
                    title: String::new(),
                    project: project.to_string(),
                    priority: Priority::Normal,
                    due: String::new(),
                    done: false,
                };
                (task, true)
            }
        };
        ctx.push_modal(TaskForm { task, is_new });
    }

    fn save(&mut self, ctx: &mut Ctx) {
        match save(&self.path, &self.tasks) {
            Ok(()) => {
                self.dirty = false;
                ctx.notify(&format!(
                    "Saved {} tasks to {}",
                    self.tasks.len(),
                    self.path.display()
                ));
            }
            Err(e) => ctx.notify_with(&e.to_string(), "Couldn't save", Severity::Error, None),
        }
        self.refresh(ctx);
    }
}

impl Widget for TasksApp {
    fn compose(&mut self) -> Vec<Child> {
        let projects: Vec<Child> = PROJECTS
            .iter()
            .map(|(id, name)| {
                ListItem::new()
                    .with(compose![Label::new(*name).id(&format!("project-{id}"))])
                    .id(id)
            })
            .collect();
        let mut table = DataTable::new()
            .cursor_type(CursorType::Row)
            .zebra_stripes(true);
        for (label, key) in [
            ("", "done"),
            ("Task", "title"),
            ("Project", "project"),
            ("Priority", "priority"),
            ("Due", "due"),
        ] {
            let _ = table.add_column_with(label, None, Some(key));
        }
        compose![
            Header::new(),
            Horizontal::new().id("body").with(compose![
                ListView::new()
                    .with(projects)
                    .id("projects")
                    .border_title("Projects"),
                Vertical::new().id("main").with(compose![
                    Input::new()
                        .placeholder("Filter tasks… (escape to close)")
                        .id("filter"),
                    table.id("tasks").border_title("Tasks"),
                    Static::new("Nothing to do here. Press [b]n[/] to add a task.").id("empty"),
                ]),
            ]),
            Footer::new(),
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_title("Tasks");
        self.refresh(ctx);
        if let Some(table) = ctx.query_id("#tasks") {
            ctx.focus(table);
        }
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("n", "new", "New"),
            Binding::new("e", "edit", "Edit"),
            Binding::new("space", "toggle", "Done"),
            Binding::new("delete", "delete", "Delete"),
            Binding::new("slash", "filter", "Filter"),
            Binding::new("escape", "close_filter", "Close filter").hidden(),
            Binding::new("ctrl+s", "save", "Save"),
            Binding::new("ctrl+q", "quit", "Quit").priority(),
        ]
    }

    fn commands(&self) -> Vec<Command> {
        let mut commands = vec![
            Command::new(
                "Mark all done",
                "Complete every task in this project",
                "mark_all",
            ),
            Command::new(
                "Clear completed",
                "Delete every completed task",
                "clear_done",
            ),
            Command::new("Save", "Write the tasks to disk", "save"),
        ];
        for (id, name) in PROJECTS {
            commands.push(Command::new(
                &format!("Show {name}"),
                "Switch project",
                &format!("project('{id}')"),
            ));
        }
        commands
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "new" => self.edit(None, ctx),
            "edit" => {
                if let Some(id) = self.current(ctx) {
                    self.edit(Some(id), ctx);
                }
            }
            "toggle" => {
                if let Some(id) = self.current(ctx) {
                    if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
                        t.done = !t.done;
                        self.dirty = true;
                    }
                    self.refresh(ctx);
                }
            }
            "delete" => {
                if let Some(t) = self
                    .current(ctx)
                    .and_then(|id| self.tasks.iter().find(|t| t.id == id))
                {
                    self.deleting = Some(t.id);
                    ctx.push_modal(Confirm {
                        question: format!("Delete “{}”?", t.title),
                    });
                }
            }
            "filter" => {
                if let Some(node) = ctx.query_id("#filter") {
                    ctx.set_class_on(node, true, "-visible");
                    ctx.focus(node);
                }
            }
            "close_filter" => {
                if let Some(node) = ctx.query_id("#filter") {
                    ctx.set_class_on(node, false, "-visible");
                }
                ctx.with::<Input, _>("#filter", |i, ctx| i.clear(ctx));
                self.filter.clear();
                self.refresh(ctx);
                if let Some(table) = ctx.query_id("#tasks") {
                    ctx.focus(table);
                }
            }
            "save" => self.save(ctx),
            "mark_all" => {
                for t in self.tasks.iter_mut() {
                    if self.project == "all" || t.project == self.project {
                        t.done = true;
                    }
                }
                self.dirty = true;
                self.refresh(ctx);
            }
            "clear_done" => {
                let before = self.tasks.len();
                self.tasks.retain(|t| !t.done);
                self.dirty = true;
                ctx.notify(&format!(
                    "Removed {} completed tasks",
                    before - self.tasks.len()
                ));
                self.refresh(ctx);
            }
            "project" => {
                let id = args.first().cloned().unwrap_or_default();
                if let Some(index) = PROJECTS.iter().position(|(p, _)| *p == id) {
                    ctx.with::<ListView, _>("#projects", |l, ctx| l.set_index(Some(index), ctx));
                }
            }
            _ => return false,
        }
        true
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(FormResult(task)) = msg.get::<FormResult>() {
            if let Some(task) = task.clone() {
                match self.tasks.iter_mut().find(|t| t.id == task.id) {
                    Some(existing) => *existing = task,
                    None => self.tasks.push(task),
                }
                self.dirty = true;
                self.refresh(ctx);
            }
            return Flow::Stop;
        }
        if let Some(Confirmed(yes)) = msg.get::<Confirmed>() {
            if let (true, Some(id)) = (*yes, self.deleting.take()) {
                self.tasks.retain(|t| t.id != id);
                self.dirty = true;
                ctx.notify("Task deleted");
                self.refresh(ctx);
            }
            return Flow::Stop;
        }
        if let Some(h) = msg.get::<list_view::Highlighted>() {
            if let Some(id) = &h.item_id {
                self.project = id.clone();
                self.refresh(ctx);
            }
            return Flow::Stop;
        }
        if msg.is::<RowSelected>() {
            self.run_action("edit", &[], ctx);
            return Flow::Stop;
        }
        if let Some(changed) = msg.get::<input::Changed>() {
            if msg.sender_id() == Some("filter") {
                self.filter = changed.value.clone();
                self.refresh(ctx);
                return Flow::Stop;
            }
        }
        Flow::Continue
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("rustual-tasks.tsv"));
    App::new(TasksApp::new(path)).css(CSS).run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(path: &Path) -> App {
        App::new(TasksApp::new(path.to_path_buf())).css(CSS)
    }

    fn temp_file(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rustual-tasks-test-{name}-{}.tsv",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[tokio::test]
    async fn add_edit_toggle_delete_and_save() {
        let path = temp_file("crud");
        let mut p = app(&path).test(Size::new(110, 30)).await;
        assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
        let footer = p
            .screen_text()
            .lines()
            .last()
            .unwrap_or_default()
            .to_string();
        assert!(
            footer.contains("n New") && footer.contains("del Delete") && footer.contains("^q Quit"),
            "{footer}"
        );
        let rows = |p: &Pilot| p.query_one::<DataTable>("#tasks").unwrap().row_count();
        assert_eq!(rows(&p), 7);

        // New task: an empty title is rejected, then saved.
        p.press(&["n"]).await;
        p.press(&["enter"]).await;
        assert!(p.screen_text().contains("The title can't be empty."));
        p.type_text("Buy milk").await;
        p.press(&["enter"]).await;
        assert_eq!(rows(&p), 8);
        assert!(p.screen_text().contains("Buy milk"));

        // Filter, toggle done, delete with confirmation.
        p.press(&["slash"]).await;
        p.type_text("milk").await;
        assert_eq!(rows(&p), 1);
        p.press(&["escape"]).await;
        assert_eq!(rows(&p), 8);
        p.press(&["delete"]).await;
        assert!(p.screen_text().contains("Delete “"));
        p.press(&["y"]).await;
        assert_eq!(rows(&p), 7);

        // Saved tasks load again.
        p.press(&["ctrl+s"]).await;
        assert_eq!(load(&path).map(|t| t.len()), Some(7));
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn projects_filter_the_table() {
        let mut p = app(&temp_file("projects")).test(Size::new(110, 30)).await;
        p.with::<TasksApp, _>("App", |_, ctx| {
            ctx.with::<ListView, _>("#projects", |l, ctx| l.set_index(Some(3), ctx));
        })
        .await;
        assert_eq!(p.query_one::<DataTable>("#tasks").unwrap().row_count(), 2);
        assert!(p.screen_text().contains("Home (1)"));
    }

    #[test]
    fn dates() {
        assert!(valid_date("") && valid_date("2026-10-31"));
        assert!(!valid_date("2026-13-01") && !valid_date("26-10-31") && !valid_date("2026-1-1"));
    }
}
