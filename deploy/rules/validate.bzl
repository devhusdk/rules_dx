"""Shared charset validators for dx deploy providers."""

load("@bazel_skylib//lib:shell.bzl", "shell")

ALNUM = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"

LOWER_ALNUM = "abcdefghijklmnopqrstuvwxyz0123456789"

NAME_CHARS = ALNUM + "._-"

VERSION_CHARS = NAME_CHARS + "+"

LAUNCHER_UNSAFE = ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/"]

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

def charset_schema_error(version, charsets):
    """Validates a schema version and its (what, chars, unsafe) charsets."""
    if version != 1:
        return charsets[0][0] + ": unsupported schema v" + str(version) + " (want v1)"
    for what, chars, unsafe in charsets:
        if type(chars) != "string" or chars == "":
            return what + ": want a non-empty charset (schema v1)"
        seen = {}
        for c in chars.elems():
            if c in seen:
                return what + ": duplicate charset char '" + c + "'"
            seen[c] = True
            if c in unsafe:
                return what + ": unsafe charset char '" + c + "' (must stay launcher-safe)"
    return ""

def genrule_arg(value):
    """Shell-quotes a string for a native.genrule cmd."""
    return shell.quote(value).replace("$", "$$")
