"""DOM queries (Textual's ``textual.css.query``)."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Generic, Iterator, TypeVar, overload

if TYPE_CHECKING:
    from ..widget import Widget

QueryType = TypeVar("QueryType", bound="Widget")
ExpectType = TypeVar("ExpectType", bound="Widget")


class QueryError(Exception):
    pass


class NoMatches(QueryError):
    pass


class TooManyMatches(QueryError):
    pass


class WrongType(QueryError):
    pass


class InvalidQueryFormat(QueryError):
    pass


class DOMQuery(Generic[QueryType]):
    """The result of ``widget.query(selector)``: a list of widgets with bulk operations."""

    def __init__(self, nodes: list[Any], selector: str = "*") -> None:
        self._nodes = nodes
        self._selector = selector

    def __repr__(self) -> str:
        return f"DOMQuery({self._selector!r}, {len(self._nodes)} nodes)"

    def __len__(self) -> int:
        return len(self._nodes)

    def __bool__(self) -> bool:
        return bool(self._nodes)

    def __iter__(self) -> Iterator[QueryType]:
        return iter(list(self._nodes))

    def __reversed__(self) -> Iterator[QueryType]:
        return iter(list(reversed(self._nodes)))

    @overload
    def __getitem__(self, index: int) -> QueryType: ...
    @overload
    def __getitem__(self, index: slice) -> list[QueryType]: ...
    def __getitem__(self, index: Any) -> Any:
        return self._nodes[index]

    @property
    def nodes(self) -> list[QueryType]:
        return list(self._nodes)

    def first(self, expect_type: type[ExpectType] | None = None) -> Any:
        if not self._nodes:
            raise NoMatches(f"No nodes match {self._selector!r}")
        node = self._nodes[0]
        if expect_type is not None and not isinstance(node, expect_type):
            raise WrongType(f"Query value is wrong type; expected {expect_type}, got {type(node)}")
        return node

    def last(self, expect_type: type[ExpectType] | None = None) -> Any:
        if not self._nodes:
            raise NoMatches(f"No nodes match {self._selector!r}")
        node = self._nodes[-1]
        if expect_type is not None and not isinstance(node, expect_type):
            raise WrongType(f"Query value is wrong type; expected {expect_type}, got {type(node)}")
        return node

    def only_one(self, expect_type: type[ExpectType] | None = None) -> Any:
        if len(self._nodes) > 1:
            raise TooManyMatches(f"Expected one node matching {self._selector!r}, found {len(self._nodes)}")
        return self.first(expect_type)

    def results(self, filter_type: type[ExpectType] | None = None) -> Iterator[Any]:
        for node in list(self._nodes):
            if filter_type is None or isinstance(node, filter_type):
                yield node

    def filter(self, selector: str) -> DOMQuery[QueryType]:
        return DOMQuery([n for n in self._nodes if n.match(selector)], f"{self._selector} {selector}")

    def exclude(self, selector: str) -> DOMQuery[QueryType]:
        return DOMQuery([n for n in self._nodes if not n.match(selector)], self._selector)

    # -- bulk operations ------------------------------------------------------------------

    def set_class(self, add: bool, *class_names: str) -> DOMQuery[QueryType]:
        for node in self._nodes:
            node.set_class(add, *class_names)
        return self

    def set_classes(self, classes: str | Any) -> DOMQuery[QueryType]:
        for node in self._nodes:
            node.set_classes(classes)
        return self

    def add_class(self, *class_names: str) -> DOMQuery[QueryType]:
        for node in self._nodes:
            node.add_class(*class_names)
        return self

    def remove_class(self, *class_names: str) -> DOMQuery[QueryType]:
        for node in self._nodes:
            node.remove_class(*class_names)
        return self

    def toggle_class(self, *class_names: str) -> DOMQuery[QueryType]:
        for node in self._nodes:
            node.toggle_class(*class_names)
        return self

    def remove(self) -> Any:
        from ..widget import AwaitComplete

        for node in self._nodes:
            node.remove()
        return AwaitComplete()

    def set_styles(self, css: str | None = None, **update_styles: Any) -> DOMQuery[QueryType]:
        for node in self._nodes:
            if css is not None:
                node.styles.parse_inline(css)
            for k, v in update_styles.items():
                setattr(node.styles, k, v)
        return self

    def set(self, display: bool | None = None, visible: bool | None = None, disabled: bool | None = None, loading: bool | None = None) -> DOMQuery[QueryType]:
        for node in self._nodes:
            if display is not None:
                node.display = display
            if visible is not None:
                node.visible = visible
            if disabled is not None:
                node.disabled = disabled
            if loading is not None:
                node.loading = loading
        return self

    def refresh(self, *, repaint: bool = True, layout: bool = False, recompose: bool = False) -> DOMQuery[QueryType]:
        for node in self._nodes:
            node.refresh(repaint=repaint, layout=layout, recompose=recompose)
        return self

    def focus(self) -> DOMQuery[QueryType]:
        if self._nodes:
            self._nodes[0].focus()
        return self

    def blur(self) -> DOMQuery[QueryType]:
        for node in self._nodes:
            if node.has_focus:
                node.blur()
        return self
