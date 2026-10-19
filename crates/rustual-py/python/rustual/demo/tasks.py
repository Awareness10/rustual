"""A task manager: projects in a sidebar, tasks in a table, a modal form to add and edit tasks
(with validation), a confirmation dialog for deleting, a filter, palette commands, and tasks
saved to a JSON file between runs.

Written against Textual's API: replace ``rustual`` with ``textual`` in the imports and it runs
under Textual unchanged."""

from __future__ import annotations

import json
import re
import tempfile
from dataclasses import asdict, dataclass, replace
from functools import partial
from pathlib import Path

from rustual import on
from rustual.app import App, ComposeResult
from rustual.binding import Binding
from rustual.command import Hit, Hits, Provider
from rustual.containers import Horizontal, Vertical
from rustual.markup import escape
from rustual.screen import ModalScreen
from rustual.widgets import (
    Button,
    DataTable,
    Footer,
    Header,
    Input,
    Label,
    ListItem,
    ListView,
    Select,
    Static,
)

PROJECTS = {"all": "All tasks", "inbox": "Inbox", "work": "Work", "home": "Home"}
PRIORITIES = ["high", "normal", "low"]
DEFAULT_PATH = Path(tempfile.gettempdir()) / "rustual-tasks.json"
DATE = re.compile(r"^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$")


@dataclass
class Task:
    id: int
    title: str
    project: str = "inbox"
    priority: str = "normal"
    due: str = ""
    done: bool = False

    @property
    def sort_key(self) -> tuple:
        # Open tasks first, then by priority and due date (tasks without one last).
        return (self.done, PRIORITIES.index(self.priority), self.due == "", self.due)


SAMPLE = [
    Task(1, "Write the release notes", "work", "high", "2026-10-20"),
    Task(2, "Review the layout PR", "work", "normal", "2026-10-18"),
    Task(3, "Answer support email", "inbox"),
    Task(4, "Book flights for the conference", "inbox", "high", "2026-11-02"),
    Task(5, "Water the plants", "home", "low", done=True),
    Task(6, "Fix the leaking tap", "home", "normal", "2026-10-25"),
    Task(7, "Update the benchmark numbers", "work", "low", done=True),
]


def load(path: Path) -> list[Task]:
    try:
        return [Task(**data) for data in json.loads(path.read_text())]
    except (OSError, ValueError, TypeError):
        return [replace(task) for task in SAMPLE]


class TaskForm(ModalScreen[Task | None]):
    """Add or edit a task; dismisses with the task, or ``None`` if cancelled."""

    DEFAULT_CSS = """
    TaskForm { align: center middle; }
    #dialog { width: 64; height: auto; padding: 1 2; border: thick $accent; background: $surface; }
    #dialog Label { margin-top: 1; }
    #heading { text-style: bold; margin-top: 0; }
    #error { color: $text-error; height: auto; }
    #buttons { height: auto; align-horizontal: right; margin-top: 1; }
    #buttons Button { margin-left: 2; }
    Input.-invalid { border: tall $error; }
    """
    BINDINGS = [
        Binding("escape", "cancel", "Cancel"),
        Binding("ctrl+s", "submit", "Save"),
    ]

    def __init__(self, task: Task, is_new: bool) -> None:
        super().__init__()
        self.editing = task
        self.is_new = is_new

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("New task" if self.is_new else "Edit task", id="heading")
            yield Label("Title")
            yield Input(self.editing.title, placeholder="What needs doing?", id="title")
            yield Label("Project")
            projects = [(name, key) for key, name in PROJECTS.items() if key != "all"]
            yield Select(projects, allow_blank=False, value=self.editing.project, id="project")
            yield Label("Priority")
            yield Select([(p, p) for p in PRIORITIES], allow_blank=False, value=self.editing.priority, id="priority")
            yield Label("Due date")
            yield Input(
                self.editing.due,
                placeholder="YYYY-MM-DD (optional)",
                restrict=r"[0-9-]*",
                max_length=10,
                id="due",
            )
            yield Label("", id="error")
            with Horizontal(id="buttons"):
                yield Button("Cancel", id="cancel")
                yield Button("Save", variant="primary", id="save")

    def action_cancel(self) -> None:
        self.dismiss(None)

    def action_submit(self) -> None:
        title = self.query_one("#title", Input).value.strip()
        due = self.query_one("#due", Input).value.strip()
        errors = []
        if not title:
            errors.append("The title can't be empty.")
        if due and not DATE.match(due):
            errors.append("The due date must look like 2026-10-31.")
        self.query_one("#title", Input).set_class(not title, "-invalid")
        self.query_one("#due", Input).set_class(bool(due) and not DATE.match(due), "-invalid")
        if errors:
            self.query_one("#error", Label).update(" ".join(errors))
            return
        self.dismiss(
            replace(
                self.editing,
                title=title,
                due=due,
                project=self.query_one("#project", Select).value,
                priority=self.query_one("#priority", Select).value,
            )
        )

    @on(Button.Pressed, "#save")
    @on(Input.Submitted)
    def _submit(self) -> None:
        self.action_submit()

    @on(Button.Pressed, "#cancel")
    def _cancel(self) -> None:
        self.dismiss(None)


class Confirm(ModalScreen[bool]):
    """A yes / no question."""

    DEFAULT_CSS = """
    Confirm { align: center middle; }
    #confirm { width: 50; height: auto; padding: 1 2; border: thick $error; background: $surface; }
    #confirm Horizontal { height: auto; align-horizontal: right; margin-top: 1; }
    #confirm Button { margin-left: 2; }
    """
    BINDINGS = [
        Binding("y", "answer(True)", "Delete"),
        Binding("n,escape", "answer(False)", "Keep"),
    ]

    def __init__(self, question: str) -> None:
        super().__init__()
        self.question = question

    def compose(self) -> ComposeResult:
        with Vertical(id="confirm"):
            yield Label(self.question)
            with Horizontal():
                yield Button("No", id="no")
                yield Button("Delete", variant="error", id="yes")

    def action_answer(self, yes: bool) -> None:
        self.dismiss(yes)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        self.dismiss(event.button.id == "yes")


class TaskCommands(Provider):
    """Palette commands: bulk actions and switching projects."""

    async def search(self, query: str) -> Hits:
        app = self.app
        assert isinstance(app, TasksApp)
        commands = [
            ("Mark all done", app.action_mark_all, "Complete every task in this project"),
            ("Clear completed", app.action_clear_done, "Delete every completed task"),
            ("Save", app.action_save, "Write the tasks to disk"),
        ]
        commands += [
            (f"Show {name}", partial(app.show_project, key), "Switch project") for key, name in PROJECTS.items()
        ]
        matcher = self.matcher(query)
        for name, callback, help in commands:
            score = matcher.match(name)
            if score > 0:
                yield Hit(score, matcher.highlight(name), callback, help=help)


class TasksApp(App):
    TITLE = "Tasks"
    COMMANDS = App.COMMANDS | {TaskCommands}
    CSS = """
    #body { height: 1fr; }
    #projects { width: 26; border: round $primary; }
    #main { width: 1fr; }
    #filter { display: none; margin: 0 1; }
    #filter.-visible { display: block; }
    #tasks { height: 1fr; border: round $secondary; }
    #empty { display: none; height: 1fr; content-align: center middle; color: $text-muted; }
    #main.-empty #tasks { display: none; }
    #main.-empty #empty { display: block; }
    """
    BINDINGS = [
        Binding("n", "new", "New"),
        Binding("e", "edit", "Edit"),
        Binding("space", "toggle", "Done"),
        Binding("delete", "delete", "Delete"),
        Binding("slash", "filter", "Filter"),
        Binding("escape", "close_filter", "Close filter", show=False),
        Binding("ctrl+s", "save", "Save"),
        Binding("ctrl+q", "quit", "Quit", priority=True),
    ]

    def __init__(self, path: Path = DEFAULT_PATH) -> None:
        super().__init__()
        self.path = path
        self.tasks = load(path)
        self.project = "all"
        self.filter = ""
        self.dirty = False

    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal(id="body"):
            projects = ListView(
                *[ListItem(Label(name, id=f"label-{key}"), id=key) for key, name in PROJECTS.items()], id="projects"
            )
            projects.border_title = "Projects"
            yield projects
            with Vertical(id="main"):
                yield Input(placeholder="Filter tasks… (escape to close)", id="filter")
                table = DataTable(cursor_type="row", zebra_stripes=True, id="tasks")
                table.border_title = "Tasks"
                yield table
                yield Static("Nothing to do here. Press [b]n[/] to add a task.", id="empty")
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#tasks", DataTable)
        for label, key in [
            ("", "done"),
            ("Task", "title"),
            ("Project", "project"),
            ("Priority", "priority"),
            ("Due", "due"),
        ]:
            table.add_column(label, key=key)
        self.refresh_tasks()
        table.focus()

    # ------------------------------------------------------------------------------ helpers

    @property
    def visible(self) -> list[Task]:
        needle = self.filter.lower()
        tasks = [
            t for t in self.tasks if (self.project == "all" or t.project == self.project) and needle in t.title.lower()
        ]
        return sorted(tasks, key=lambda t: t.sort_key)

    @property
    def current(self) -> Task | None:
        table = self.query_one("#tasks", DataTable)
        if not table.row_count:
            return None
        key = table.coordinate_to_cell_key(table.cursor_coordinate).row_key.value
        return next((t for t in self.tasks if f"t{t.id}" == key), None)

    def refresh_tasks(self) -> None:
        table = self.query_one("#tasks", DataTable)
        cursor = table.cursor_row
        table.clear()
        # DataTable cells use Rich markup (as in Textual), so named colors rather than $variables.
        for t in self.visible:
            title = escape(t.title)
            table.add_row(
                "[green]✓[/]" if t.done else "·",
                f"[strike dim]{title}[/]" if t.done else title,
                PROJECTS.get(t.project, t.project),
                {"high": "[bold red]high[/]", "low": "[dim]low[/]"}.get(t.priority, t.priority),
                t.due,
                key=f"t{t.id}",
            )
        if table.row_count:
            table.move_cursor(row=min(cursor, table.row_count - 1))
        self.query_one("#main").set_class(not table.row_count, "-empty")
        for key, name in PROJECTS.items():
            count = sum(1 for t in self.tasks if not t.done and key in ("all", t.project))
            self.query_one(f"#label-{key}", Label).update(f"{name} [$text-muted]({count})[/]")
        open_count = sum(1 for t in self.tasks if not t.done)
        self.sub_title = f"{open_count} open of {len(self.tasks)}" + (" · unsaved" if self.dirty else "")

    def changed(self) -> None:
        self.dirty = True
        self.refresh_tasks()

    def show_project(self, key: str) -> None:
        self.query_one("#projects", ListView).index = list(PROJECTS).index(key)

    def open_form(self, task: Task, is_new: bool) -> None:
        def done(result: Task | None) -> None:
            if result is None:
                return
            if is_new:
                self.tasks.append(result)
            else:
                self.tasks = [result if t.id == result.id else t for t in self.tasks]
            self.changed()

        self.push_screen(TaskForm(task, is_new), done)

    # ------------------------------------------------------------------------------ actions

    def action_new(self) -> None:
        next_id = max((t.id for t in self.tasks), default=0) + 1
        project = "inbox" if self.project == "all" else self.project
        self.open_form(Task(next_id, "", project), is_new=True)

    def action_edit(self) -> None:
        if (task := self.current) is not None:
            self.open_form(task, is_new=False)

    def action_toggle(self) -> None:
        if (task := self.current) is not None:
            task.done = not task.done
            self.changed()

    def action_delete(self) -> None:
        task = self.current
        if task is None:
            return

        def done(yes: bool | None) -> None:
            if yes:
                self.tasks.remove(task)
                self.notify("Task deleted")
                self.changed()

        self.push_screen(Confirm(f"Delete “{task.title}”?"), done)

    def action_filter(self) -> None:
        filter_input = self.query_one("#filter", Input)
        filter_input.add_class("-visible")
        filter_input.focus()

    def action_close_filter(self) -> None:
        filter_input = self.query_one("#filter", Input)
        filter_input.remove_class("-visible")
        filter_input.clear()
        self.filter = ""
        self.refresh_tasks()
        self.query_one("#tasks").focus()

    def action_save(self) -> None:
        try:
            self.path.write_text(json.dumps([asdict(t) for t in self.tasks], indent=2))
        except OSError as error:
            self.notify(str(error), title="Couldn't save", severity="error")
            return
        self.dirty = False
        self.notify(f"Saved {len(self.tasks)} tasks to {self.path}")
        self.refresh_tasks()

    def action_mark_all(self) -> None:
        for t in self.tasks:
            if self.project in ("all", t.project):
                t.done = True
        self.changed()

    def action_clear_done(self) -> None:
        before = len(self.tasks)
        self.tasks = [t for t in self.tasks if not t.done]
        self.notify(f"Removed {before - len(self.tasks)} completed tasks")
        self.changed()

    # ----------------------------------------------------------------------------- messages

    def on_list_view_highlighted(self, event: ListView.Highlighted) -> None:
        if event.item is not None and event.item.id:
            self.project = event.item.id
            self.refresh_tasks()

    def on_data_table_row_selected(self) -> None:
        self.action_edit()

    @on(Input.Changed, "#filter")
    def _filter(self, event: Input.Changed) -> None:
        self.filter = event.value
        self.refresh_tasks()


if __name__ == "__main__":
    TasksApp().run()
