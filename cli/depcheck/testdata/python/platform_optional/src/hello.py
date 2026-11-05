"""Platform/optional uses counted without exceptions."""

import sys

import optional_feat
import pytest
import win_only


def greet(name):
    assert pytest is not None
    return f"Hello, {name}!"


def platform():
    if sys.platform == "win32":
        return win_only.label()
    return "posix"


def feat():
    return optional_feat.label()
