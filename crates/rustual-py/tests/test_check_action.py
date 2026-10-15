from rustual.app import App, ComposeResult
from rustual.reactive import reactive
from rustual.widgets import Footer, Label


async def test_check_action_hides_and_disables_bindings():
    class MyApp(App):
        BINDINGS = [("n", "next", "Next"), ("p", "prev", "Previous"), ("x", "secret", "Secret")]
        page = reactive(0)

        def compose(self) -> ComposeResult:
            yield Label("")
            yield Footer()

        def check_action(self, action: str, parameters: tuple) -> bool | None:
            if action == "prev" and self.page == 0:
                return None  # shown dimmed, not runnable
            if action == "secret":
                return False  # hidden
            return True

        def action_next(self) -> None:
            self.page += 1

        def action_prev(self) -> None:
            self.page -= 1

        def action_secret(self) -> None:
            raise AssertionError("hidden action must not run")

    app = MyApp()
    async with app.run_test() as pilot:
        footer = pilot.screen_text().splitlines()[-1]
        assert "Next" in footer and "Previous" in footer and "Secret" not in footer
        await pilot.press("p", "x")
        assert app.page == 0
        await pilot.press("n", "n", "p")
        assert app.page == 1
