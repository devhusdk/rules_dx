"""Shared helpers that must NOT be classified as tests (`test_` prefix negative)."""

import envelopes


def make_payload():
    return {"seed": 7}


def wrap_seed():
    return envelopes.wrap("seed", make_payload())
