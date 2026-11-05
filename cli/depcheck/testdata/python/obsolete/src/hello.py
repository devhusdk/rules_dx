"""pytest directly imported; exceptions below are obsolete."""

import pytest


def greet(name):
    assert pytest is not None
    return f"Hello, {name}!"
