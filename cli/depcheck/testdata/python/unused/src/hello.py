"""Consistent lock, one unused declaration."""

import coverage
import pytest


def greet(name):
    assert pytest is not None
    assert coverage is not None
    return f"Hello, {name}!"
