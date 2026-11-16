"""Failing conformance subject."""

import greet


def test_failing_case() -> None:
    """Expects a greeting that is not produced."""
    assert greet.greet("world") == "goodbye world"
