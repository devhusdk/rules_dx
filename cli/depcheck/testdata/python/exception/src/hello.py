"""pytest used directly; build-plugin via script (non-import)."""

import pytest


def greet(name):
    assert pytest is not None
    return f"Hello, {name}!"


def version():
    return "0.0.0"
