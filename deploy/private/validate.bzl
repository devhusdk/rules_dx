"""Shared charset validators for dx deploy providers."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load("//core:packaging.bzl", _only_chars = "only_chars")

ALNUM = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"

LOWER_ALNUM = "abcdefghijklmnopqrstuvwxyz0123456789"

NAME_CHARS = ALNUM + "._-"

VERSION_CHARS = NAME_CHARS + "+"

DIGITS = "0123456789"

SIZE_UNITS = [("GB", 1000 * 1000 * 1000), ("MB", 1000 * 1000), ("KB", 1000), ("B", 1)]

only_chars = _only_chars

def genrule_arg(value):
    """Shell-quotes a string for a native.genrule cmd."""
    return shell.quote(value).replace("$", "$$")

def size_budget_error(value):
    """Validates one size_budget such as '25MB'; '' means no budget."""
    if value == "":
        return ""
    for unit, _ in SIZE_UNITS:
        if value.endswith(unit):
            digits = value[:-len(unit)]
            if digits != "" and only_chars(digits, DIGITS) and int(digits) > 0:
                return ""
            break
    return ("size_budget '" + value + "': want a positive whole number and a unit, " +
            "for example '25MB' (B, KB, MB or GB, decimal)")

def parse_size_budget(value):
    """Returns the byte count of one valid size_budget; '' means no budget and gives 0."""
    if value == "":
        return 0
    for unit, scale in SIZE_UNITS:
        if value.endswith(unit):
            return int(value[:-len(unit)]) * scale
    return 0
