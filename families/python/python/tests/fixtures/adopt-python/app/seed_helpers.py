"""Shared helpers that stay libraries."""

import envelopes


def make_payload():
    return {"seed": 7}


def wrap_seed():
    return envelopes.wrap("seed", make_payload())
