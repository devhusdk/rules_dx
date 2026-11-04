"""Tests for the polyglot geometry helpers."""

from rectangles import area


def test_area():
    assert area(3, 4) == 12
