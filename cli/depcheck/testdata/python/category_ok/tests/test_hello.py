"""Correctly categorized dev use plus multi-category prod use."""

import pytest
import test_helper
from hello import greet


def test_greet():
    assert test_helper.check("world")
    assert pytest is not None
    assert "Hello" in greet("world")
