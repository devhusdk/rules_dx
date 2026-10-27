"""Shared charset validators for dx deploy providers."""

load("@bazel_skylib//lib:shell.bzl", "shell")

ALNUM = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"

LOWER_ALNUM = "abcdefghijklmnopqrstuvwxyz0123456789"

NAME_CHARS = ALNUM + "._-"

VERSION_CHARS = NAME_CHARS + "+"

EMBED_UNSAFE = ["\"", "\\", "'", " ", "\n", "`", "$"]

def only_chars(value, allowed):
    """Returns True when every char of value is in allowed."""
    for c in value.elems():
        if c not in allowed:
            return False
    return True

def has_any(value, chars):
    """Returns True when value contains any of chars."""
    for c in value.elems():
        if c in chars:
            return True
    return False

def genrule_arg(value):
    """Shell-quotes a string for a native.genrule cmd."""
    return shell.quote(value).replace("$", "$$")
