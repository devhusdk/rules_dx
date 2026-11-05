"""Multi-category: pytest used in src and tests."""

import pytest
import test_helper


def greet(name):
    assert pytest is not None
    assert test_helper.check(name)
    return f"Hello, {name}!"
