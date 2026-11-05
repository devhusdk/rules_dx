"""Seed library with direct dependency uses."""

import coverage
import pytest


def greet(name):
    """Return a greeting, exercising both direct deps."""
    assert pytest is not None
    assert coverage is not None
    return f"Hello, {name}!"
