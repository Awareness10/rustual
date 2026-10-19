from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Any, ClassVar

from .._bridge import _native
from ..message import Message
from ..widget import _NATIVE_CLASSES, AwaitComplete
from ._tree import Tree, TreeNode


@dataclass
class DirEntry:
    """The data of a ``DirectoryTree`` node."""

    path: Path
    loaded: bool = False


class DirectoryTree(Tree[DirEntry]):
    """A tree view of a directory (folders are loaded when expanded).

    Override :meth:`filter_paths` to hide paths.
    """

    _rs_kind: ClassVar[str] = "DirectoryTree"
    _rs_framework: ClassVar[bool] = True

    class _PathMessage(Message):
        def __init__(self, node: TreeNode[DirEntry] | None, path: Path) -> None:
            super().__init__()
            self._node = node
            self._node_id: int | None = None
            self.path = path

        @property
        def node(self) -> TreeNode[DirEntry]:
            if self._node is None and self._node_id is not None:
                self._node = self.control._get_node(self._node_id)
            return self._node  # type: ignore[return-value]

        @property
        def control(self) -> DirectoryTree:
            return self.sender  # type: ignore[return-value]

        @property
        def directory_tree(self) -> DirectoryTree:
            return self.control

    class FileSelected(_PathMessage):
        """A file was selected."""

    class DirectorySelected(_PathMessage):
        """A directory was selected."""

    def __init__(
        self,
        path: str | Path,
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(str(path), name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"path": str(path)}
        if type(self).filter_paths is not DirectoryTree.filter_paths:
            self._rs_kw["filter"] = self._keep_path

    def _keep_path(self, path: str) -> bool:
        return any(True for _ in self.filter_paths([Path(path)]))

    def filter_paths(self, paths: Iterable[Path]) -> Iterable[Path]:
        """Filter the paths shown in the tree (override in a subclass)."""
        return paths

    def _node_data(self, node: TreeNode[DirEntry]) -> DirEntry | None:
        info = self._call("node_path", node._id)
        if info is None:
            return None
        return DirEntry(Path(info[0]), info[1])

    @property
    def path(self) -> Path:
        return Path(self._get("path"))

    @path.setter
    def path(self, path: str | Path) -> None:
        self._call("set_path", str(path))
        self._tree_node_map().clear()

    def reload(self) -> AwaitComplete:
        """Reload the tree from disk."""
        self._call("reload")
        return AwaitComplete()

    def reload_node(self, node: TreeNode[DirEntry]) -> AwaitComplete:
        self._call("reload_node", node._id)
        return AwaitComplete()


def _path_message(cls: type) -> Any:
    def make(node_id: int, path: str) -> Any:
        message = cls(None, Path(path))
        message._node_id = node_id
        return message

    return make


_native.register_class("DirectoryTree.FileSelected", _path_message(DirectoryTree.FileSelected))
_native.register_class("DirectoryTree.DirectorySelected", _path_message(DirectoryTree.DirectorySelected))
_NATIVE_CLASSES["DirectoryTree"] = DirectoryTree
