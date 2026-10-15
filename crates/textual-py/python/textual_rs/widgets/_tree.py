from __future__ import annotations

from typing import Any, ClassVar, Generic, TypeVar

from .._bridge import _native
from ..message import Message
from ..widget import Widget, _NATIVE_CLASSES
from ._detached import DetachedNative, markup_of

TreeDataType = TypeVar("TreeDataType")
EventTreeDataType = TypeVar("EventTreeDataType")


class NodeID(int):
    """The id of a node within one tree."""


class RemoveRootError(Exception):
    """An attempt was made to remove the root node."""


class UnknownNodeID(Exception):
    """The node id isn't known to the tree."""


class AddNodeError(Exception):
    """A node couldn't be added (bad ``before`` / ``after``)."""


class TreeNodes(list):  # type: ignore[type-arg]
    """An immutable-by-convention list of tree nodes."""


class TreeNode(Generic[TreeDataType]):
    """A node of a :class:`Tree`. Node objects are unique per node id."""

    def __init__(self, tree: Tree[Any], id: int, data: Any = None) -> None:
        self._tree = tree
        self._id = NodeID(id)
        self._data = data

    def __repr__(self) -> str:
        return f"TreeNode({self.label!r}, {self.data!r})"

    def _info(self) -> tuple:
        info = self._tree._call("node_info", self._id)
        if info is None:
            raise UnknownNodeID(f"Unknown NodeID ({self._id}) in tree")
        return info

    # -- identity / structure --------------------------------------------------------------------

    @property
    def id(self) -> NodeID:
        return self._id

    @property
    def tree(self) -> Tree[TreeDataType]:
        return self._tree

    @property
    def parent(self) -> TreeNode[TreeDataType] | None:
        parent = self._info()[0]
        return None if parent is None else self._tree._get_node(parent)

    @property
    def children(self) -> TreeNodes:
        return TreeNodes(self._tree._get_node(c) for c in self._info()[1])

    @property
    def siblings(self) -> TreeNodes:
        parent = self.parent
        if parent is None:
            return TreeNodes([self])
        return TreeNodes(c for c in parent.children if c is not self)

    @property
    def next_sibling(self) -> TreeNode[TreeDataType] | None:
        parent = self.parent
        if parent is None:
            return None
        children = parent.children
        index = children.index(self)
        return children[index + 1] if index + 1 < len(children) else None

    @property
    def previous_sibling(self) -> TreeNode[TreeDataType] | None:
        parent = self.parent
        if parent is None:
            return None
        children = parent.children
        index = children.index(self)
        return children[index - 1] if index > 0 else None

    @property
    def line(self) -> int:
        return self._info()[6]

    @property
    def is_root(self) -> bool:
        return self._info()[0] is None

    @property
    def is_last(self) -> bool:
        parent = self.parent
        return parent is None or parent.children[-1] is self

    # -- state --------------------------------------------------------------------------------------

    @property
    def label(self) -> str:
        """The label as plain text."""
        return self._info()[3]

    @label.setter
    def label(self, new_label: Any) -> None:
        self.set_label(new_label)

    def set_label(self, label: Any) -> None:
        self._tree._call("set_label", self._id, markup_of(label))

    @property
    def data(self) -> TreeDataType | None:
        return self._tree._node_data(self)

    @data.setter
    def data(self, data: TreeDataType | None) -> None:
        self._data = data

    @property
    def is_expanded(self) -> bool:
        return self._info()[4]

    @property
    def is_collapsed(self) -> bool:
        return not self.is_expanded

    @property
    def allow_expand(self) -> bool:
        return self._info()[5]

    @allow_expand.setter
    def allow_expand(self, allow_expand: bool) -> None:
        self._tree._call("set_allow_expand", self._id, bool(allow_expand))

    def expand(self) -> TreeNode[TreeDataType]:
        self._tree._call("expand", self._id, False)
        return self

    def expand_all(self) -> TreeNode[TreeDataType]:
        self._tree._call("expand", self._id, True)
        return self

    def collapse(self) -> TreeNode[TreeDataType]:
        self._tree._call("collapse", self._id, False)
        return self

    def collapse_all(self) -> TreeNode[TreeDataType]:
        self._tree._call("collapse", self._id, True)
        return self

    def toggle(self) -> TreeNode[TreeDataType]:
        self._tree._call("toggle", self._id, False)
        return self

    def toggle_all(self) -> TreeNode[TreeDataType]:
        self._tree._call("toggle", self._id, True)
        return self

    # -- children -----------------------------------------------------------------------------------

    def add(
        self,
        label: Any,
        data: TreeDataType | None = None,
        *,
        before: int | TreeNode[TreeDataType] | None = None,
        after: int | TreeNode[TreeDataType] | None = None,
        expand: bool = False,
        allow_expand: bool = True,
    ) -> TreeNode[TreeDataType]:
        """Add a child node (collapsed unless ``expand``)."""
        if before is not None and after is not None:
            raise AddNodeError("Unable to add a node both before and after a node")
        index: int | None = None
        if before is not None or after is not None:
            children = self.children
            ref = before if before is not None else after
            if isinstance(ref, TreeNode):
                try:
                    index = children.index(ref)
                except ValueError:
                    which = "before" if before is not None else "after"
                    raise AddNodeError(f"The node specified for `{which}` is not a child of this node") from None
                if after is not None:
                    index += 1
            elif isinstance(ref, int):
                if before is not None:
                    index = ref if ref >= 0 else max(0, len(children) + ref)
                else:
                    index = ref + 1 + (len(children) if ref < 0 else 0)
            else:
                raise TypeError("`before` / `after` must be an index or a TreeNode")
        node_id = self._tree._call("add", self._id, index, markup_of(label), bool(expand), bool(allow_expand))
        node = self._tree._get_node(node_id)
        node._data = data
        return node

    def add_leaf(
        self,
        label: Any,
        data: TreeDataType | None = None,
        *,
        before: int | TreeNode[TreeDataType] | None = None,
        after: int | TreeNode[TreeDataType] | None = None,
    ) -> TreeNode[TreeDataType]:
        """Add a node that can't be expanded."""
        return self.add(label, data, before=before, after=after, expand=False, allow_expand=False)

    def remove(self) -> None:
        """Remove this node and its descendants."""
        if self.is_root:
            raise RemoveRootError("Attempt to remove the root node of a Tree.")
        self._tree._forget(self._tree._call("remove", self._id))

    def remove_children(self) -> None:
        self._tree._forget(self._tree._call("remove_children", self._id))

    def refresh(self) -> None:
        self._tree.refresh()

    def __class_getitem__(cls, item: Any) -> Any:
        return cls


class Tree(DetachedNative, Widget, Generic[TreeDataType]):
    """A tree of expandable nodes with a cursor."""

    _rs_kind: ClassVar[str] = "Tree"
    _rs_framework: ClassVar[bool] = True
    can_focus = True

    ICON_NODE = "▶ "
    ICON_NODE_EXPANDED = "▼ "

    class _NodeMessage(Message):
        def __init__(self, node: TreeNode[Any] | None = None) -> None:
            super().__init__()
            self._node = node
            self._node_id: int | None = None

        @property
        def node(self) -> TreeNode[Any]:
            if self._node is None and self._node_id is not None:
                self._node = self.control._get_node(self._node_id)  # type: ignore[union-attr]
            return self._node  # type: ignore[return-value]

        @property
        def control(self) -> Tree[Any]:
            sender = self.sender
            if sender is None and self._node is not None:
                return self._node.tree
            return sender  # type: ignore[return-value]

    class NodeSelected(_NodeMessage):
        """A node was selected (enter / click)."""

    class NodeExpanded(_NodeMessage):
        """A node was expanded."""

    class NodeCollapsed(_NodeMessage):
        """A node was collapsed."""

    class NodeHighlighted(_NodeMessage):
        """The cursor moved to a node."""

    def __init__(
        self,
        label: Any,
        data: TreeDataType | None = None,
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"label": markup_of(label)}
        self._tree_nodes: dict[int, TreeNode[TreeDataType]] = {}
        self._get_node(0)._data = data

    def __class_getitem__(cls, item: Any) -> Any:
        return cls

    # -- nodes --------------------------------------------------------------------------------------

    def _tree_node_map(self) -> dict[int, TreeNode[TreeDataType]]:
        nodes = self.__dict__.get("_tree_nodes")
        if nodes is None:
            nodes = self.__dict__["_tree_nodes"] = {}
        return nodes

    def _get_node(self, node_id: int) -> TreeNode[TreeDataType]:
        nodes = self._tree_node_map()
        node = nodes.get(node_id)
        if node is None:
            node = nodes[node_id] = TreeNode(self, node_id)
        return node

    def _forget(self, node_ids: list[int]) -> None:
        nodes = self._tree_node_map()
        for node_id in node_ids:
            nodes.pop(node_id, None)

    def _node_data(self, node: TreeNode[TreeDataType]) -> TreeDataType | None:
        return node._data

    @property
    def root(self) -> TreeNode[TreeDataType]:
        return self._get_node(self._get("root"))

    def get_node_by_id(self, node_id: int) -> TreeNode[TreeDataType]:
        if self._call("node_info", node_id) is None:
            raise UnknownNodeID(f"Unknown NodeID ({node_id}) in tree")
        return self._get_node(node_id)

    def get_node_at_line(self, line_no: int) -> TreeNode[TreeDataType] | None:
        node_id = self._call("node_at_line", line_no)
        return None if node_id is None else self._get_node(node_id)

    def add_json(self, json_data: object, node: TreeNode[Any] | None = None) -> None:
        """Add JSON data (dicts / lists / scalars) under ``node`` (the root by default)."""
        from ..content import escape

        if node is None:
            node = self.root

        def add_node(name: str, node: TreeNode[Any], data: object) -> None:
            if isinstance(data, dict):
                node.set_label(f"[b]{{}}[/b] {escape(name)}")
                for key, value in data.items():
                    add_node(key, node.add(""), value)
            elif isinstance(data, list):
                node.set_label(f"[b]\\[][/b] {escape(name)}")
                for index, value in enumerate(data):
                    add_node(str(index), node.add(""), value)
            else:
                node.allow_expand = False
                if name:
                    node.set_label(f"[b]{escape(name)}[/b]=[i]{escape(repr(data))}[/i]")
                else:
                    node.set_label(escape(repr(data)))

        add_node("", node, json_data)

    def clear(self) -> Tree[TreeDataType]:
        """Remove every node under the root (the root keeps its label and data)."""
        old_root = self.root
        root_id = self._call("clear")
        self._tree_node_map().clear()
        self._get_node(root_id)._data = old_root._data
        return self

    def reset(self, label: Any, data: TreeDataType | None = None) -> Tree[TreeDataType]:
        """Clear the tree and give the root a new label and data."""
        root_id = self._call("reset", markup_of(label))
        self._tree_node_map().clear()
        self._get_node(root_id)._data = data
        return self

    # -- options --------------------------------------------------------------------------------------

    def _option(name: str) -> property:  # type: ignore[misc]
        def fget(self: Tree[Any]) -> Any:
            return self._get(name)

        def fset(self: Tree[Any], value: Any) -> None:
            self._call(f"set_{name}", value)

        return property(fget, fset)

    show_root = _option("show_root")
    show_guides = _option("show_guides")
    guide_depth = _option("guide_depth")
    auto_expand = _option("auto_expand")
    del _option

    # -- cursor ---------------------------------------------------------------------------------------

    @property
    def cursor_line(self) -> int:
        return self._get("cursor_line")

    @cursor_line.setter
    def cursor_line(self, line: int) -> None:
        self._call("set_cursor_line", int(line))

    @property
    def cursor_node(self) -> TreeNode[TreeDataType] | None:
        node_id = self._get("cursor_node")
        return None if node_id is None else self._get_node(node_id)

    @property
    def last_line(self) -> int:
        return self._get("last_line")

    def move_cursor(self, node: TreeNode[TreeDataType] | None, animate: bool = False) -> None:
        self._call("move_cursor", None if node is None else node._id)

    def move_cursor_to_line(self, line: int, animate: bool = False) -> None:
        if self.get_node_at_line(line) is None:
            raise IndexError(f"No line no. {line} in the tree")
        self.cursor_line = line

    def select_node(self, node: TreeNode[TreeDataType] | None) -> None:
        self._call("select_node", None if node is None else node._id)

    def unselect(self) -> None:
        self._call("unselect")

    def scroll_to_line(self, line: int, animate: bool = True) -> None:
        self._call("scroll_to_line", int(line))

    def scroll_to_node(self, node: TreeNode[TreeDataType], animate: bool = True) -> None:
        self._call("scroll_to_node", node._id)


def _node_message(cls: type) -> Any:
    def make(node_id: int) -> Any:
        message = cls()
        message._node_id = node_id
        return message

    return make


for _name in ("NodeSelected", "NodeExpanded", "NodeCollapsed", "NodeHighlighted"):
    _native.register_class(f"Tree.{_name}", _node_message(getattr(Tree, _name)))

_NATIVE_CLASSES["Tree"] = Tree
