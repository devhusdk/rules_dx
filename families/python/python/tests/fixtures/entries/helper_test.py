"""Thin entry fixture test."""

import helper


def test_suffix():
    assert helper.suffix("x") == "<x>"
