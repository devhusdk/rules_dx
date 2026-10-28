"""Annotated tuple with mapped, ignored, and dynamic entries."""

import os

pytest_plugins: tuple[str, ...] = (
    "http_fixtures",
    "mapped_plugin",
    "ignored_plugin",
    os.environ.get("EXTRA_PLUGIN", ""),
)


def test_plugins():
    assert pytest_plugins
