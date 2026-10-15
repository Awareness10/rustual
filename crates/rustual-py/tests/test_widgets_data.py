"""DataTable, ListView, Tree, DirectoryTree, Log, RichLog, Pretty, ProgressBar, Sparkline, Digits."""

from pathlib import Path

import pytest

from rustual import on
from rustual.app import App, ComposeResult
from rustual.coordinate import Coordinate
from rustual.widgets import (
    DataTable,
    Digits,
    DirectoryTree,
    Label,
    ListItem,
    ListView,
    Log,
    Pretty,
    ProgressBar,
    RichLog,
    Sparkline,
    Tree,
)
from rustual.widgets.data_table import CellKey, ColumnKey, DuplicateKey, RowDoesNotExist, RowKey
from rustual.widgets.directory_tree import DirEntry
from rustual.widgets.tree import RemoveRootError, TreeNode, UnknownNodeID

# ------------------------------------------------------------------------------------ DataTable


class TableApp(App):
    def __init__(self, cursor_type: str = "cell") -> None:
        super().__init__()
        self.cursor_type = cursor_type
        self.events: list = []

    def compose(self) -> ComposeResult:
        table = DataTable(cursor_type=self.cursor_type)
        # Filled before mounting (detached widget).
        table.add_columns("Name", ("Age", "age"))
        table.add_row("Ada", 36, key="ada")
        yield table

    def on_mount(self) -> None:
        table = self.query_one(DataTable)
        table.add_rows([("Bob", 25), ("Cy", 41)])
        table.focus()

    def on_data_table_cell_highlighted(self, event: DataTable.CellHighlighted) -> None:
        self.events.append(("cell_h", event.value, event.coordinate, event.cell_key))

    def on_data_table_cell_selected(self, event: DataTable.CellSelected) -> None:
        self.events.append(("cell_s", event.value, event.coordinate, event.control is self.query_one(DataTable)))

    @on(DataTable.RowSelected)
    def row_selected(self, event: DataTable.RowSelected) -> None:
        self.events.append(("row_s", event.cursor_row, event.row_key, event.data_table.id))

    def on_data_table_row_highlighted(self, event: DataTable.RowHighlighted) -> None:
        self.events.append(("row_h", event.cursor_row, event.row_key))

    def on_data_table_header_selected(self, event: DataTable.HeaderSelected) -> None:
        self.events.append(("header", event.column_key, event.column_index, event.label))


async def test_data_table_api():
    app = TableApp()
    async with app.run_test(size=(60, 12)) as pilot:
        table = app.query_one(DataTable)
        assert table.row_count == 3
        assert table.column_count == 2
        assert table.get_row("ada") == ["Ada", 36]
        assert table.get_row_at(2) == ["Cy", 41]
        assert table.get_cell("ada", "age") == 36
        assert table.get_cell_at(Coordinate(1, 0)) == "Bob"
        assert list(table.get_column("age")) == [36, 25, 41]
        assert table.get_row_index("ada") == 0
        assert table.get_column_index("age") == 1
        key = table.add_row("Dee", 3.5, key="dee")
        assert isinstance(key, RowKey) and key == "dee" and key.value == "dee"
        assert table.get_cell(key, "age") == 3.5
        auto = table.add_row("Eve", 1)
        assert auto.value is None
        assert table.get_row(auto) == ["Eve", 1]
        table.update_cell(auto, "age", 99, update_width=True)
        assert table.get_cell_at((4, 1)) == 99
        table.update_cell_at((4, 0), "[b]Eva[/b]")
        assert table.get_row(auto) == ["[b]Eva[/b]", 99]
        cell_key = table.coordinate_to_cell_key(Coordinate(0, 1))
        assert cell_key == CellKey(RowKey("ada"), ColumnKey("age"))
        assert table.get_cell_coordinate("ada", "age") == Coordinate(0, 1)
        assert "Ada" in pilot.screen_text() and "Age" in pilot.screen_text()
        assert [c.label for c in table.ordered_columns] == ["Name", "Age"]
        assert list(table.rows)[0] == "ada"

        with pytest.raises(DuplicateKey):
            table.add_row("x", 1, key="ada")
        with pytest.raises(RowDoesNotExist):
            table.get_row("nope")

        table.sort("age")
        assert [r[0] for r in (table.get_row_at(i) for i in range(table.row_count))] == [
            "Dee",
            "Bob",
            "Ada",
            "Cy",
            "[b]Eva[/b]",
        ]
        table.sort("age", reverse=True)
        assert table.get_row_at(0)[1] == 99
        table.sort(key=lambda row: str(row[1]))
        assert table.get_row_at(0)[0] == "Bob"
        table.sort(table.ordered_columns[1].key, key=lambda age: -age)
        assert table.get_row_at(0)[1] == 99

        table.remove_row("dee")
        assert table.row_count == 4
        table.zebra_stripes = True
        assert table.zebra_stripes is True
        table.clear()
        assert table.row_count == 0 and table.column_count == 2
        table.clear(columns=True)
        assert table.column_count == 0


async def test_data_table_cursor_and_messages():
    app = TableApp()
    async with app.run_test(size=(60, 12)) as pilot:
        table = app.query_one(DataTable)
        assert table.cursor_coordinate == Coordinate(0, 0)
        app.events.clear()
        await pilot.press("down")
        assert table.cursor_coordinate == Coordinate(1, 0)
        assert app.events[-1] == ("cell_h", "Bob", Coordinate(1, 0), CellKey(table.coordinate_to_cell_key((1, 0)).row_key, table.coordinate_to_cell_key((1, 0)).column_key))
        await pilot.press("right", "enter")
        assert app.events[-1] == ("cell_s", 25, Coordinate(1, 1), True)
        table.cursor_coordinate = Coordinate(2, 0)
        assert table.cursor_row == 2
        table.move_cursor(row=0, column=1)
        assert table.cursor_coordinate == (0, 1)

        table.cursor_type = "row"
        assert table.cursor_type == "row"
        await pilot.press("down")
        assert app.events[-1][0] == "row_h" and app.events[-1][1] == 1
        await pilot.press("enter")
        kind, row, key, table_id = app.events[-1]
        assert (kind, row) == ("row_s", 1)
        assert table.get_row(key) == ["Bob", 25]

        await pilot.click(DataTable, offset=(3, 0))
        assert app.events[-1][0] == "header"
        assert app.events[-1][2] == 0 and app.events[-1][3] == "Name"


# ------------------------------------------------------------------------------------ ListView


class ListApp(App):
    def __init__(self) -> None:
        super().__init__()
        self.events: list = []

    def compose(self) -> ComposeResult:
        yield ListView(
            ListItem(Label("one"), id="one"),
            ListItem(Label("two"), id="two"),
            ListItem(Label("three"), id="three"),
        )

    def on_mount(self) -> None:
        self.query_one(ListView).focus()

    def on_list_view_highlighted(self, event: ListView.Highlighted) -> None:
        self.events.append(("h", event.item.id if event.item else None))

    @on(ListView.Selected)
    def selected(self, event: ListView.Selected) -> None:
        self.events.append(("s", event.item.id, event.index, event.list_view is self.query_one(ListView)))


async def test_list_view():
    app = ListApp()
    async with app.run_test(size=(40, 12)) as pilot:
        lv = app.query_one(ListView)
        assert lv.index == 0
        assert lv.highlighted_child.id == "one"
        assert lv.highlighted_child.highlighted
        await pilot.press("down")
        assert lv.index == 1
        assert app.events[-1] == ("h", "two")
        await pilot.press("enter")
        assert app.events[-1] == ("s", "two", 1, True)
        await pilot.click("#three")
        assert lv.index == 2
        assert app.events[-1] == ("s", "three", 2, True)

        lv.append(ListItem(Label("four"), id="four"))
        await pilot.pause()
        assert len(lv) == 4
        lv.index = 3
        assert lv.highlighted_child.id == "four"
        assert "four" in pilot.screen_text()
        lv.extend([ListItem(Label("five")), ListItem(Label("six"))])
        lv.insert(0, [ListItem(Label("zero"), id="zero")])
        await pilot.pause()
        assert len(lv) == 7
        assert lv.children[0].id == "zero"
        lv.pop(0)
        await pilot.pause()
        assert len(lv) == 6
        lv.remove_items([0, 1])
        await pilot.pause()
        assert len(lv) == 4
        lv.clear()
        await pilot.pause()
        assert len(lv) == 0
        assert lv.index is None
        assert app.events[-1] == ("h", None)


# ------------------------------------------------------------------------------------ Tree


class TreeApp(App):
    def __init__(self) -> None:
        super().__init__()
        self.events: list = []

    def compose(self) -> ComposeResult:
        tree: Tree[str] = Tree("Dune", data="root")
        tree.root.expand()
        characters = tree.root.add("Characters", expand=True)
        characters.add_leaf("Paul", data="paul")
        characters.add_leaf("Jessica")
        tree.root.add("Places")
        yield tree

    def on_mount(self) -> None:
        self.query_one(Tree).focus()

    def on_tree_node_highlighted(self, event: Tree.NodeHighlighted) -> None:
        self.events.append(("h", event.node.label))

    def on_tree_node_selected(self, event: Tree.NodeSelected) -> None:
        self.events.append(("s", event.node.label, event.node.data))

    @on(Tree.NodeExpanded)
    def expanded(self, event: Tree.NodeExpanded) -> None:
        self.events.append(("e", event.node.label, event.control is self.query_one(Tree)))

    def on_tree_node_collapsed(self, event: Tree.NodeCollapsed) -> None:
        self.events.append(("c", event.node.label))


async def test_tree():
    app = TreeApp()
    async with app.run_test(size=(40, 12)) as pilot:
        tree = app.query_one(Tree)
        root = tree.root
        assert root.label == "Dune" and root.data == "root" and root.is_root
        assert root.is_expanded
        characters, places = root.children
        assert characters.label == "Characters" and characters.is_expanded
        paul, jessica = characters.children
        assert paul.data == "paul" and not paul.allow_expand
        assert paul.parent is characters
        assert paul.next_sibling is jessica and jessica.previous_sibling is paul
        assert jessica.is_last and not paul.is_last
        assert tree.get_node_by_id(paul.id) is paul
        text = pilot.screen_text()
        assert "Paul" in text and "Places" in text

        # Like Textual, the cursor starts on the first line (highlighting it on mount).
        assert tree.cursor_line == 0
        assert ("h", "Dune") in app.events
        await pilot.press("down", "down")
        assert tree.cursor_node is paul
        await pilot.press("enter")
        assert ("s", "Paul", "paul") in app.events

        places.add_leaf("Arrakis", data={"hot": True})
        places.expand()
        await pilot.pause()
        assert app.events[-1] == ("e", "Places", True)
        assert "Arrakis" in pilot.screen_text()
        places.collapse()
        await pilot.pause()
        assert app.events[-1] == ("c", "Places")
        places.toggle()
        assert places.is_expanded
        node = places.add("Caladan", before=0)
        assert [c.label for c in places.children] == ["Caladan", "Arrakis"]
        node.label = "[b]Caladan[/b]"
        assert node.label == "Caladan"
        node.remove()
        assert [c.label for c in places.children] == ["Arrakis"]
        with pytest.raises(UnknownNodeID):
            tree.get_node_by_id(node.id)
        with pytest.raises(RemoveRootError):
            root.remove()

        tree.move_cursor(places)
        assert tree.cursor_node is places
        tree.select_node(places)
        await pilot.pause()
        assert ("s", "Places", None) in app.events

        tree.clear()
        assert tree.root.children == [] and tree.root.data == "root"
        tree.reset("New", data=1)
        assert tree.root.label == "New" and tree.root.data == 1
        tree.root.add_leaf("x")
        tree.show_root = False
        assert tree.show_root is False


async def test_tree_add_json():
    class JsonApp(App):
        def compose(self) -> ComposeResult:
            tree = Tree("data")
            tree.add_json({"a": 1, "b": [True, None]})
            tree.root.expand_all()
            yield tree

    app = JsonApp()
    async with app.run_test(size=(40, 12)) as pilot:
        assert isinstance(app.query_one(Tree).root.children[0], TreeNode)
        text = pilot.screen_text()
        assert "a=1" in text and "None" in text


# ------------------------------------------------------------------------------------ DirectoryTree


async def test_directory_tree(tmp_path: Path):
    (tmp_path / "sub").mkdir()
    (tmp_path / "sub" / "inner.txt").write_text("x")
    (tmp_path / "a.py").write_text("x")
    (tmp_path / "skip.log").write_text("x")

    class Filtered(DirectoryTree):
        def filter_paths(self, paths):
            return [p for p in paths if p.suffix != ".log"]

    events: list = []

    class DirApp(App):
        def compose(self) -> ComposeResult:
            yield Filtered(tmp_path)

        def on_mount(self) -> None:
            self.query_one(DirectoryTree).focus()

        def on_directory_tree_file_selected(self, event: DirectoryTree.FileSelected) -> None:
            events.append(("file", event.path, event.node.data))

        def on_directory_tree_directory_selected(self, event: DirectoryTree.DirectorySelected) -> None:
            events.append(("dir", event.path))

    app = DirApp()
    async with app.run_test(size=(50, 12)) as pilot:
        dt = app.query_one(DirectoryTree)
        assert dt.path == tmp_path
        labels = [c.label for c in dt.root.children]
        assert labels == ["sub", "a.py"]
        assert dt.root.data == DirEntry(tmp_path, True)
        assert "skip.log" not in pilot.screen_text()
        await pilot.press("down", "down")
        await pilot.press("enter")
        assert events[-1] == ("dir", tmp_path / "sub")
        assert [c.label for c in dt.root.children[0].children] == ["inner.txt"]
        await pilot.press("down", "down", "enter")
        assert events[-1][0] == "file" and events[-1][1] == tmp_path / "a.py"
        assert events[-1][2].path == tmp_path / "a.py"
        (tmp_path / "b.txt").write_text("x")
        dt.reload()
        assert "b.txt" in [c.label for c in dt.root.children]


# ------------------------------------------------------------------------------------ logs


async def test_log_and_rich_log():
    class LogApp(App):
        CSS = "Log, RichLog { height: 1fr; }"

        def compose(self) -> ComposeResult:
            log = Log(max_lines=3, id="log")
            log.write("before mount\n")
            yield log
            rich_log = RichLog(markup=True, id="rich")
            rich_log.write("[b]bold[/b] start")
            yield rich_log

    app = LogApp()
    async with app.run_test(size=(80, 20)) as pilot:
        log = app.query_one(Log)
        assert log.lines == ["before mount"]
        log.write("par")
        log.write("tial\nnext")
        assert log.lines == ["before mount", "partial", "next"]
        log.write_lines(["a", "b"])
        assert log.lines == ["next", "a", "b"]
        assert log.line_count == 3
        log.write_line(42)
        assert log.lines[-1] == "42"
        assert log.max_lines == 3 and log.auto_scroll is True
        log.clear()
        assert log.lines == []

        rich_log = app.query_one(RichLog)
        assert rich_log.lines == ["bold start"]
        rich_log.write({"a": 1})
        assert rich_log.lines[-1] == "{'a': 1}"
        rich_log.markup = False
        rich_log.write("[b]raw[/b]")
        assert rich_log.lines[-1] == "[b]raw[/b]"
        from rustual.content import Content

        rich_log.write(Content.from_markup("[i]styled[/i]"))
        assert rich_log.lines[-1] == "styled"
        assert "bold start" in pilot.screen_text()
        rich_log.clear()
        assert rich_log.lines == []


async def test_rich_log_rich_renderable():
    rich_table = pytest.importorskip("rich.table")

    class RichApp(App):
        def compose(self) -> ComposeResult:
            yield RichLog()

    app = RichApp()
    async with app.run_test(size=(60, 20)) as pilot:
        table = rich_table.Table("Col A", "Col B")
        table.add_row("x", "y")
        log = app.query_one(RichLog)
        log.write(table)
        assert any("Col A" in line for line in log.lines)
        assert "Col A" in pilot.screen_text()


# ------------------------------------------------------------------------------------ display widgets


async def test_display_widgets():
    class DisplayApp(App):
        def compose(self) -> ComposeResult:
            yield Pretty({"key": [1, 2]})
            yield ProgressBar(total=10, show_eta=False)
            yield Sparkline([1, 2, 3, 4], summary_function=max)
            yield Digits("3.14")

    app = DisplayApp()
    async with app.run_test(size=(60, 16)) as pilot:
        text = pilot.screen_text()
        assert "{'key': [1, 2]}" in text
        assert "0%" in text
        pretty = app.query_one(Pretty)
        pretty.update([None, True])
        await pilot.pause()
        assert "[None, True]" in pilot.screen_text()

        bar = app.query_one(ProgressBar)
        assert bar.total == 10 and bar.progress == 0 and bar.percentage == 0
        bar.advance(5)
        assert bar.progress == 5 and bar.percentage == 0.5
        await pilot.pause()
        assert "50%" in pilot.screen_text()
        bar.update(total=20, progress=15)
        assert bar.percentage == 0.75
        bar.update(advance=5)
        assert bar.progress == 20
        bar.total = None
        assert bar.percentage is None

        spark = app.query_one(Sparkline)
        assert spark.data == [1, 2, 3, 4]
        spark.data = [5, 1]
        assert spark.data == [5.0, 1.0]
        assert "█" in pilot.screen_text()

        digits = app.query_one(Digits)
        assert digits.value == "3.14"
        digits.update("42")
        assert digits.value == "42"
        with pytest.raises(TypeError):
            digits.update(42)  # type: ignore[arg-type]


async def test_prefilled_before_mount():
    table = DataTable()
    table.add_column("A", key="a")
    key = table.add_row(1)
    assert table.get_cell(key, "a") == 1
    tree = Tree("r")
    leaf = tree.root.add_leaf("leaf", data=5)
    assert tree.root.children == [leaf]
    bar = ProgressBar(total=4)
    bar.advance(2)

    class PrefilledApp(App):
        def compose(self) -> ComposeResult:
            yield table
            yield tree
            yield bar

    app = PrefilledApp()
    async with app.run_test(size=(40, 12)):
        assert table.row_count == 1 and table.get_row(key) == [1]
        assert tree.root.children[0] is leaf and leaf.data == 5
        assert bar.progress == 2
