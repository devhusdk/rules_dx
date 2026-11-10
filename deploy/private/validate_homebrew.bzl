"""Validators for the homebrew deploy rule."""

load(":validate.bzl", "LOWER_ALNUM", "NAME_CHARS", "only_chars")

def homebrew_formula_error(formula):
    """Validates one formula name value."""
    if type(formula) != "string" or formula == "":
        return ("homebrew_tap_deploy: invalid formula '" + str(formula) +
                "': want a non-empty name (for example 'my-tool')")
    if not only_chars(formula, LOWER_ALNUM + "-_"):
        return ("homebrew_tap_deploy: invalid formula '" + formula +
                "': want only [a-z0-9_-]")
    return ""

def homebrew_repo_error(field, value):
    """Validates one GitHub owner/repository value."""
    parts = value.split("/") if type(value) == "string" else []
    if len(parts) != 2 or parts[0] == "" or parts[1] == "" or not only_chars(value, NAME_CHARS + "/"):
        return ("homebrew_tap_deploy: invalid " + field + " '" + str(value) +
                "': want owner/repository (for example 'org/homebrew-tap')")
    return ""

def homebrew_text_error(field, value):
    """Validates one formula text value."""
    if type(value) != "string" or value == "":
        return "homebrew_tap_deploy: invalid " + field + " '" + str(value) + "': want a non-empty string"
    if "\n" in value or "\r" in value:
        return "homebrew_tap_deploy: invalid " + field + ": must be a single line"
    return ""
