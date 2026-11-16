"""Passing conformance subject."""

import greet


def test_passing_case() -> None:
    """Greets the world."""
    assert greet.greet("world") == "hello world"
