"""Tests for handlers + envelopes (real tests: `_test` suffix)."""

import envelopes
import handlers


def test_roundtrip():
    assert handlers.decode(handlers.encode({"b": 1, "a": 2})) == {"a": 2, "b": 1}


def test_wrap():
    env = envelopes.wrap("greet", {"hi": True})
    assert env.kind == "greet"
    assert handlers.decode(env.body) == {"hi": True}
