"""Conventional test-dir file that stays a library."""

import logins


def fixture_name():
    return "ada"


def fixture_digest():
    return logins.digest(fixture_name())
