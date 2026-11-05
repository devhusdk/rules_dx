"""Prod dep used only here must fail with a category error."""

import pytest
import test_helper


def test_greet():
    assert test_helper.check("world")
    assert pytest is not None
