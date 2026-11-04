"""Conventional test-dir file that must stay a library (`test_` prefix negative)."""

import logins


def fixture_name():
    return "ada"


def fixture_digest():
    return logins.digest(fixture_name())
