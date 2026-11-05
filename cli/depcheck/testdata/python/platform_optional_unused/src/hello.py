"""Unused optional still fails (optional is not proof)."""

import pytest


def greet(name):
    assert pytest is not None
    return f"Hello, {name}!"
