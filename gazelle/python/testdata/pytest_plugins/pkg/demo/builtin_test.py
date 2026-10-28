"""Built-in plugin aliases resolve to pytest, not to local modules."""

pytest_plugins = "capture, logging"


def test_builtin():
    assert pytest_plugins
