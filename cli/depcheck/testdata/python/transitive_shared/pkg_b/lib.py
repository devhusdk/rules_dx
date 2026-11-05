"""Package B owns the pytest use; workspace scope counts it."""

import pytest


def label():
    assert pytest is not None
    return "shared-helper"
