"""Shared charset validators for dx deploy providers."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load("//libs/starlark:packaging.bzl", _has_any = "has_any", _only_chars = "only_chars")

ALNUM = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"

LOWER_ALNUM = "abcdefghijklmnopqrstuvwxyz0123456789"

NAME_CHARS = ALNUM + "._-"

VERSION_CHARS = NAME_CHARS + "+"

EMBED_UNSAFE = ["\"", "\\", "'", " ", "\n", "`", "$"]

only_chars = _only_chars

has_any = _has_any

def genrule_arg(value):
    """Shell-quotes a string for a native.genrule cmd."""
    return shell.quote(value).replace("$", "$$")
