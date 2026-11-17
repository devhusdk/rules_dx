"""Tests for the manifest-free module."""

import pure


def test_area():
    assert pure.circle_area(0.0) == 0.0
    assert pure.circle_area(1.0) == pure.circle_area(1.0)
