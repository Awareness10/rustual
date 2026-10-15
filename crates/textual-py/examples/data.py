"""Data widgets: a DataTable, a Tree and a RichLog of selections, plus a ProgressBar,
Sparkline and Digits. Select a row / node (enter or click) to log it; press s to sort."""

from textual_rs import on
from textual_rs.app import App, ComposeResult
from textual_rs.containers import Horizontal, Vertical
from textual_rs.widgets import DataTable, Digits, Footer, Header, ProgressBar, RichLog, Sparkline, Tree

ROWS = [
    ("lane", "swimmer", "country", "time"),
    (4, "Joseph Schooling", "Singapore", 50.39),
    (2, "Michael Phelps", "United States", 51.14),
    (5, "Chad le Clos", "South Africa", 51.14),
    (6, "László Cseh", "Hungary", 51.14),
    (3, "Li Zhuhao", "China", 51.26),
    (8, "Mehdy Metella", "France", 51.58),
    (7, "Tom Shields", "United States", 51.73),
    (1, "Aleksandr Sadovnikov", "Russia", 51.84),
]


class DataApp(App):
    CSS = """
    #top { height: 1fr; }
    DataTable { width: 2fr; }
    Tree { width: 1fr; }
    #bottom { height: 12; }
    RichLog { width: 3fr; border: round $accent; }
    #side { width: 2fr; }
    Sparkline { height: 3; margin: 1 0; }
    Digits { width: auto; }
    """
    BINDINGS = [("s", "sort", "Sort by time"), ("p", "progress", "Advance")]

    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal(id="top"):
            table = DataTable(cursor_type="row", zebra_stripes=True)
            *_, self.time_column = table.add_columns(*ROWS[0])
            table.add_rows(ROWS[1:])
            yield table
            tree: Tree[str] = Tree("Dune", data="book")
            tree.root.expand()
            people = tree.root.add("Characters", expand=True)
            for name in ("Paul", "Jessica", "Chani"):
                people.add_leaf(name, data=name.lower())
            places = tree.root.add("Places", expand=True)
            places.add_leaf("Arrakis")
            places.add_leaf("Caladan")
            yield tree
        with Horizontal(id="bottom"):
            yield RichLog(markup=True, highlight=True)
            with Vertical(id="side"):
                yield ProgressBar(total=len(ROWS) - 1, show_eta=False)
                yield Sparkline([row[3] for row in ROWS[1:]])
                yield Digits(f"{ROWS[1][3]:.2f}")
        yield Footer()

    def on_mount(self) -> None:
        self.query_one(RichLog).write("[b]Ready.[/b] Select a row or a tree node.")
        self.query_one(ProgressBar).advance(3)

    @on(DataTable.RowSelected)
    def row_selected(self, event: DataTable.RowSelected) -> None:
        row = event.data_table.get_row(event.row_key)
        self.query_one(RichLog).write(f"[green]row[/green] {event.cursor_row}: {row}")
        self.query_one(Digits).update(f"{row[3]:.2f}")

    def on_tree_node_selected(self, event: Tree.NodeSelected) -> None:
        self.query_one(RichLog).write(f"[yellow]node[/yellow] {event.node.label} data={event.node.data!r}")

    def action_sort(self) -> None:
        self.query_one(DataTable).sort(self.time_column)

    def action_progress(self) -> None:
        self.query_one(ProgressBar).advance()


if __name__ == "__main__":
    DataApp().run()
