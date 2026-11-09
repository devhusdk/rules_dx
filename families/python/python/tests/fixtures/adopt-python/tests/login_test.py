"""Real tests for the login helpers (`_test` suffix)."""

import logins


def test_digest_stable():
    assert logins.digest("ada") == logins.digest("ada")


def test_digest_length():
    assert len(logins.digest("ada")) == 16
